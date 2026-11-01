// SPDX-License-Identifier: MPL-2.0
//! Lane 2b: a small image-decode crate (bytes in, RGBA + size out).
//!
//! This crate links the vendored `png` and `zune-jpeg` crates (see
//! `third_party/*/VENDORED.md`) and is the single entry point the OS's future
//! image consumers will use:
//!
//! ```
//! # fn demo(bytes: &[u8]) -> Result<(), neo_image_decode::ImageDecodeError> {
//! let image = neo_image_decode::decode(bytes)?;
//! // image.width x image.height pixels, row-major RGBA8.
//! # let _ = image;
//! # Ok(())
//! # }
//! ```
//!
//! **Host only.** The vendored `png` 0.18 decoder requires `std`, and the OS
//! userland target has none. `libneo` re-exports this crate as
//! `libneo::image_decode` behind its off-by-default `image-decode` feature; no
//! OS-target build enables it. Keeping the implementation in its own std crate
//! (rather than inside `libneo`) also keeps a Cargo feature from linking std
//! into every `no_std` userland binary when a host test asks for it. When the
//! OS gets its own `std` (OT-61), the same API can move behind it without
//! changing callers.
//!
//! **The size cap is the guard against a hostile image.** `decode` uses
//! [`DEFAULT_MAX_DECODE_BYTES`]; callers that know their budget use
//! [`decode_with_limit`]. A header whose `width * height * 4` exceeds the cap is
//! rejected *before* the pixel buffer is allocated, the PNG decoder's own
//! allocation budget is bounded, and the JPEG header's dimensions are checked
//! before the scan is decoded.

use core::fmt;

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Default cap on the decoded RGBA buffer: 64 MiB (≈ 4096 × 4096 × 4).
pub const DEFAULT_MAX_DECODE_BYTES: usize = 64 * 1024 * 1024;

/// The container format sniffed from the first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

impl ImageFormat {
    /// Sniff a PNG signature or a JPEG `SOI` marker. `None` for anything else.
    pub fn from_magic(bytes: &[u8]) -> Option<Self> {
        const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        if bytes.starts_with(&PNG_SIGNATURE) {
            Some(Self::Png)
        } else if bytes.starts_with(&[0xff, 0xd8]) {
            Some(Self::Jpeg)
        } else {
            None
        }
    }
}

/// A decoded image: row-major RGBA8 pixels (`rgba.len() == width * height * 4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DecodedImage {
    /// `(width, height)`.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Why a decode failed.
#[derive(Debug)]
pub enum ImageDecodeError {
    /// No bytes.
    EmptyInput,
    /// Not a PNG or JPEG.
    UnknownFormat,
    /// A header declared a zero dimension or an unusable size.
    InvalidDimensions,
    /// `width * height * 4` exceeded the caller's cap; nothing was allocated for
    /// the pixels.
    TooLarge {
        width: u32,
        height: u32,
        limit_bytes: usize,
    },
    /// A decoder produced a colour layout this API does not map to RGBA.
    UnsupportedColorType(&'static str),
    /// A decoder produced an unexpected buffer size.
    DecoderOutput(&'static str),
    Png(png::DecodingError),
    Jpeg(zune_jpeg::errors::DecodeErrors),
}

impl fmt::Display for ImageDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "empty image input"),
            Self::UnknownFormat => write!(f, "unknown image format (expected PNG or JPEG)"),
            Self::InvalidDimensions => write!(f, "image has a zero or unusable dimension"),
            Self::TooLarge {
                width,
                height,
                limit_bytes,
            } => write!(
                f,
                "{width}x{height} RGBA needs {} bytes, over the {limit_bytes}-byte cap",
                u64::from(*width) * u64::from(*height) * 4
            ),
            Self::UnsupportedColorType(what) => write!(f, "unsupported image colour type: {what}"),
            Self::DecoderOutput(what) => write!(f, "unexpected decoder output: {what}"),
            Self::Png(err) => write!(f, "PNG decode failed: {err}"),
            Self::Jpeg(err) => write!(f, "JPEG decode failed: {err}"),
        }
    }
}

/// Decode `bytes` (PNG or JPEG, sniffed) to RGBA8 with the default 64 MiB cap.
pub fn decode(bytes: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    decode_with_limit(bytes, DEFAULT_MAX_DECODE_BYTES)
}

/// Decode `bytes` (PNG or JPEG, sniffed) to RGBA8, rejecting an image whose
/// `width * height * 4` exceeds `max_rgba_bytes`.
pub fn decode_with_limit(
    bytes: &[u8],
    max_rgba_bytes: usize,
) -> Result<DecodedImage, ImageDecodeError> {
    if bytes.is_empty() {
        return Err(ImageDecodeError::EmptyInput);
    }
    match ImageFormat::from_magic(bytes) {
        Some(ImageFormat::Png) => decode_png(bytes, max_rgba_bytes),
        Some(ImageFormat::Jpeg) => decode_jpeg(bytes, max_rgba_bytes),
        None => Err(ImageDecodeError::UnknownFormat),
    }
}

/// `width * height * 4`, or `TooLarge` when it exceeds `limit`. Rejects zero
/// dimensions before any multiplication can be mistaken for a valid empty image.
fn checked_rgba_len(width: u32, height: u32, limit: usize) -> Result<usize, ImageDecodeError> {
    if width == 0 || height == 0 {
        return Err(ImageDecodeError::InvalidDimensions);
    }
    let bytes = u64::from(width) * u64::from(height) * 4;
    let limit = u64::try_from(limit).unwrap_or(u64::MAX);
    if bytes > limit {
        return Err(ImageDecodeError::TooLarge {
            width,
            height,
            limit_bytes: usize::try_from(limit).unwrap_or(usize::MAX),
        });
    }
    usize::try_from(bytes).map_err(|_| ImageDecodeError::TooLarge {
        width,
        height,
        limit_bytes: usize::try_from(limit).unwrap_or(usize::MAX),
    })
}

/// The width and height from a well-formed PNG `IHDR`, without asking the
/// decoder to parse (and potentially allocate for) anything first. The signature
/// is checked by `ImageFormat::from_magic`; this reads the first chunk.
fn png_ihdr_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // 8-byte signature + 4-byte length + 4-byte type + 13-byte IHDR body.
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" || bytes[8..12] != [0, 0, 0, 13] {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

fn decode_png(bytes: &[u8], limit: usize) -> Result<DecodedImage, ImageDecodeError> {
    // Reject an over-cap header before the decoder allocates for it, and keep
    // the decoder's own budget at least a small working floor so a tiny image
    // with an exactly-fitting cap (e.g. 4x4 with a 64-byte cap) can still parse.
    if let Some((width, height)) = png_ihdr_dimensions(bytes) {
        checked_rgba_len(width, height, limit)?;
    }
    let mut decoder = png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),
        png::Limits {
            // The decoder's own allocation budget (row buffers, ancillary
            // chunks). The pixel buffer is allocated below, after the cap check.
            bytes: limit.max(64 * 1024),
        },
    );
    // Expand palette/grayscale and strip 16-bit samples to 8 bits. The output is
    // still one of four layouts; `png_to_rgba` maps each to RGBA8.
    decoder.set_transformations(png::Transformations::normalize_to_color8());

    let mut reader = decoder.read_info().map_err(ImageDecodeError::Png)?;
    let (canvas_w, canvas_h) = (reader.info().width, reader.info().height);
    checked_rgba_len(canvas_w, canvas_h, limit)?;

    let out_len = reader
        .output_buffer_size()
        .ok_or(ImageDecodeError::TooLarge {
            width: canvas_w,
            height: canvas_h,
            limit_bytes: limit,
        })?;
    if out_len > limit {
        return Err(ImageDecodeError::TooLarge {
            width: canvas_w,
            height: canvas_h,
            limit_bytes: limit,
        });
    }

    let mut raw = vec![0u8; out_len];
    let out = reader.next_frame(&mut raw).map_err(ImageDecodeError::Png)?;
    let (width, height) = (out.width, out.height);
    let rgba_len = checked_rgba_len(width, height, limit)?;

    let bpp = out.color_type.samples();
    if bpp == 0 {
        return Err(ImageDecodeError::UnsupportedColorType("zero samples"));
    }
    let row_len = (width as usize)
        .checked_mul(bpp)
        .ok_or(ImageDecodeError::InvalidDimensions)?;
    if out.line_size < row_len {
        return Err(ImageDecodeError::DecoderOutput(
            "PNG line shorter than a row",
        ));
    }
    let used = out
        .line_size
        .checked_mul(height as usize)
        .ok_or(ImageDecodeError::InvalidDimensions)?;
    if used > raw.len() {
        return Err(ImageDecodeError::DecoderOutput(
            "PNG frame larger than its buffer",
        ));
    }

    let rgba = png_to_rgba(
        &raw[..used],
        out.color_type,
        out.line_size,
        width as usize,
        height as usize,
        rgba_len,
    )?;
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

fn png_to_rgba(
    data: &[u8],
    color_type: png::ColorType,
    line_size: usize,
    width: usize,
    height: usize,
    rgba_len: usize,
) -> Result<Vec<u8>, ImageDecodeError> {
    let mut out = Vec::with_capacity(rgba_len);
    for y in 0..height {
        let start = y * line_size;
        let row = &data[start..start + width * color_type.samples()];
        match color_type {
            png::ColorType::Grayscale => {
                for &g in row {
                    out.extend_from_slice(&[g, g, g, 0xff]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for px in row.chunks_exact(2) {
                    out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
                }
            }
            png::ColorType::Rgb => {
                for px in row.chunks_exact(3) {
                    out.extend_from_slice(&[px[0], px[1], px[2], 0xff]);
                }
            }
            png::ColorType::Rgba => out.extend_from_slice(row),
            png::ColorType::Indexed => {
                return Err(ImageDecodeError::UnsupportedColorType(
                    "indexed PNG was not expanded",
                ));
            }
        }
    }
    if out.len() != rgba_len {
        return Err(ImageDecodeError::DecoderOutput(
            "PNG expansion did not fill the RGBA buffer",
        ));
    }
    Ok(out)
}

fn decode_jpeg(bytes: &[u8], limit: usize) -> Result<DecodedImage, ImageDecodeError> {
    // JPEG dimensions are `u16`; accept the full format range here and let the
    // explicit `checked_rgba_len` below decide against the caller's cap. That
    // turns an over-cap image into `TooLarge` (a size-policy answer) rather than
    // a decoder `LargeDimensions` error, and no scan buffer is allocated until
    // after the cap check.
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGBA)
        .set_max_width(65_535)
        .set_max_height(65_535);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(ImageDecodeError::Jpeg)?;
    let info = decoder.info().ok_or(ImageDecodeError::DecoderOutput(
        "JPEG headers did not decode",
    ))?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    let rgba_len = checked_rgba_len(width, height, limit)?;

    let pixels = decoder.decode().map_err(ImageDecodeError::Jpeg)?;
    let colorspace = decoder
        .output_colorspace()
        .ok_or(ImageDecodeError::DecoderOutput(
            "JPEG output colours unknown",
        ))?;
    let rgba = jpeg_to_rgba(
        pixels,
        colorspace,
        width as usize,
        height as usize,
        rgba_len,
    )?;
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

fn jpeg_to_rgba(
    pixels: Vec<u8>,
    colorspace: ColorSpace,
    width: usize,
    height: usize,
    rgba_len: usize,
) -> Result<Vec<u8>, ImageDecodeError> {
    let pixels_per_image = width
        .checked_mul(height)
        .ok_or(ImageDecodeError::InvalidDimensions)?;
    let expected = match colorspace {
        ColorSpace::Luma => pixels_per_image,
        ColorSpace::LumaA => pixels_per_image * 2,
        ColorSpace::RGB | ColorSpace::BGR => pixels_per_image * 3,
        ColorSpace::RGBA | ColorSpace::BGRA | ColorSpace::ARGB => pixels_per_image * 4,
        _ => {
            return Err(ImageDecodeError::UnsupportedColorType(
                "JPEG output colours are not mapped to RGBA",
            ));
        }
    };
    if pixels.len() != expected {
        return Err(ImageDecodeError::DecoderOutput(
            "JPEG output length does not match its colours",
        ));
    }
    if colorspace == ColorSpace::RGBA {
        return Ok(pixels);
    }

    let mut out = Vec::with_capacity(rgba_len);
    match colorspace {
        ColorSpace::Luma => {
            for &g in &pixels {
                out.extend_from_slice(&[g, g, g, 0xff]);
            }
        }
        ColorSpace::LumaA => {
            for px in pixels.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
        }
        ColorSpace::RGB => {
            for px in pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 0xff]);
            }
        }
        ColorSpace::BGR => {
            for px in pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[2], px[1], px[0], 0xff]);
            }
        }
        ColorSpace::BGRA => {
            for px in pixels.chunks_exact(4) {
                out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        }
        ColorSpace::ARGB => {
            for px in pixels.chunks_exact(4) {
                out.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
            }
        }
        _ => unreachable!("colours checked above"),
    }
    if out.len() != rgba_len {
        return Err(ImageDecodeError::DecoderOutput(
            "JPEG conversion did not fill the RGBA buffer",
        ));
    }
    Ok(out)
}
