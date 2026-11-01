// SPDX-License-Identifier: MPL-2.0
//! Host known-answer tests for the Lane 2b image-decode API
//! (`neo-image-decode`, re-exported by `libneo` as `libneo::image_decode`).
//!
//! The images are fetched raw from upstream repositories at pinned commits;
//! `tests/data/PROVENANCE.md` records each URL, commit, SHA-256, decoded size
//! and the expected RGBA SHA-256. The expected PNG hashes were produced with an
//! independent decoder (Python's `zlib` plus a direct PNG unfilter), and the
//! JPEG hashes pin the vendored `zune-jpeg` output because independent JPEG
//! decoders differ by ±1–2 in the IDCT/colour paths (see PROVENANCE.md).

#[cfg(test)]
mod tests {
    use neo_image_decode::{ImageDecodeError, ImageFormat, decode, decode_with_limit};
    use sha2::{Digest, Sha256};

    const PNG_RGB8: &[u8] = include_bytes!("../tests/data/png/basn2c08.png");
    const PNG_RGBA8: &[u8] = include_bytes!("../tests/data/png/basn6a08.png");
    const PNG_PALETTE8: &[u8] = include_bytes!("../tests/data/png/basn3p08.png");
    const PNG_GRAY8: &[u8] = include_bytes!("../tests/data/png/basn0g08.png");
    const PNG_GA8: &[u8] = include_bytes!("../tests/data/png/basn4a08.png");
    const PNG_RGB16: &[u8] = include_bytes!("../tests/data/png/basn2c16.png");

    const JPEG_444: &[u8] = include_bytes!("../tests/data/jpeg/tiny_non_interleaved_444.jpg");
    const JPEG_422: &[u8] = include_bytes!("../tests/data/jpeg/non_interleaved_422_64x64.jpg");
    const JPEG_PROGRESSIVE: &[u8] =
        include_bytes!("../tests/data/jpeg/progressive_restart_420.jpg");

    fn hex_sha256(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(64);
        for byte in Sha256::digest(bytes) {
            use core::fmt::Write;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    fn assert_decoded(image: &[u8], width: u32, height: u32, expected_sha256: &str) {
        let decoded = decode(image).expect("image decodes");
        assert_eq!(decoded.size(), (width, height));
        assert_eq!(decoded.rgba.len(), (width * height * 4) as usize);
        assert_eq!(
            hex_sha256(&decoded.rgba),
            expected_sha256,
            "decoded RGBA hash for {width}x{height}"
        );
    }

    #[test]
    fn png_rgb8_decodes_to_rgba() {
        assert_decoded(
            PNG_RGB8,
            32,
            32,
            "23a53c674ec50d5a5eb9c3f679b6b19ba5304ae99dff76801bec4939e0f0c99e",
        );
    }

    #[test]
    fn png_rgba8_decodes_to_rgba() {
        assert_decoded(
            PNG_RGBA8,
            32,
            32,
            "2eb6a2cb3166e9c188add371157e9f81caa18fdf34d218844ed930b53b7431d2",
        );
    }

    #[test]
    fn png_palette8_decodes_to_rgba() {
        assert_decoded(
            PNG_PALETTE8,
            32,
            32,
            "b1c3302eceae6738c36edafa98c8054824d9440f3ba53a3f17cc81d29acc32cc",
        );
    }

    #[test]
    fn png_gray8_decodes_to_rgba() {
        assert_decoded(
            PNG_GRAY8,
            32,
            32,
            "982faa277e83f73ca15b491e67eb41fa25526418ed23e057a9986c4f620eb158",
        );
    }

    #[test]
    fn png_gray_alpha8_decodes_to_rgba() {
        assert_decoded(
            PNG_GA8,
            32,
            32,
            "76b94a71d3c183a362c2cf6a46ebb50adc9d3a25a89bc0afc46fda6dbb002509",
        );
    }

    #[test]
    fn png_rgb16_is_stripped_to_8_bit() {
        assert_decoded(
            PNG_RGB16,
            32,
            32,
            "7c4b73e829f02793549b4480e25f0c0b332abcb24ac059dbad855fd1d726c17a",
        );
    }

    #[test]
    fn jpeg_444_decodes_to_rgba() {
        assert_decoded(
            JPEG_444,
            16,
            16,
            "59bfa308579730d52c4ec6c9366102ecaf570e6a7cd03aba307d568bd1bea6db",
        );
    }

    #[test]
    fn jpeg_422_decodes_to_rgba() {
        assert_decoded(
            JPEG_422,
            64,
            64,
            "4500aa1466440ae556a15710b241a21ba43e89b49f198aadac4c6744b602ef8f",
        );
    }

    #[test]
    fn jpeg_progressive_decodes_to_rgba() {
        assert_decoded(
            JPEG_PROGRESSIVE,
            256,
            256,
            "df194d1583400202406fd9894bd39349e753c818ea8ebf0d00f967ed757b6c00",
        );
    }

    #[test]
    fn format_sniffing_is_exact() {
        assert_eq!(ImageFormat::from_magic(PNG_RGB8), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_magic(JPEG_444), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_magic(b"not an image"), None);
        assert_eq!(ImageFormat::from_magic(&[0xff]), None);
    }

    #[test]
    fn empty_and_unknown_inputs_are_rejected() {
        assert!(matches!(decode(b""), Err(ImageDecodeError::EmptyInput)));
        assert!(matches!(
            decode(b"GIF89a"),
            Err(ImageDecodeError::UnknownFormat)
        ));
    }

    #[test]
    fn size_cap_rejects_before_allocating() {
        // 32 x 32 x 4 = 4096 bytes; a 64-byte cap must reject both formats.
        for image in [PNG_RGBA8, JPEG_422] {
            match decode_with_limit(image, 64) {
                Err(ImageDecodeError::TooLarge { limit_bytes, .. }) => {
                    assert_eq!(limit_bytes, 64);
                }
                other => panic!("expected TooLarge, got {other:?}"),
            }
        }
    }

    #[test]
    fn cap_at_exact_size_is_accepted() {
        // 16 x 16 x 4 = 1024 bytes exactly (JPEG), and 32 x 32 x 4 = 4096 (PNG).
        for (image, exact) in [(JPEG_444, 1024), (PNG_RGBA8, 4096)] {
            let decoded = decode_with_limit(image, exact).expect("cap equal to the pixels");
            assert_eq!(decoded.rgba.len(), exact);
            assert!(matches!(
                decode_with_limit(image, exact - 1),
                Err(ImageDecodeError::TooLarge { .. })
            ));
        }
    }

    /// A fixed-budget mutation smoke test (the THINK's host fuzz guardrail):
    /// deterministic single-byte flips and truncations must return an error or a
    /// valid capped image, never panic or allocate past the cap. This is not
    /// coverage-guided fuzzing; it is the cheap CI-budget floor.
    #[test]
    fn mutation_smoke_never_panics_or_over_allocates() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for fixture in [
            PNG_RGBA8,
            PNG_PALETTE8,
            PNG_RGB16,
            JPEG_444,
            JPEG_422,
            JPEG_PROGRESSIVE,
        ] {
            for _ in 0..64 {
                let mut mutated = fixture.to_vec();
                match next() % 3 {
                    0 => {
                        let cut = (next() as usize) % mutated.len();
                        mutated.truncate(cut);
                    }
                    1 => {
                        let index = (next() as usize) % mutated.len();
                        mutated[index] ^= (next() as u8) | 1;
                    }
                    _ => {
                        let index = (next() as usize) % mutated.len();
                        mutated[index] = next() as u8;
                    }
                }
                if let Ok(image) = decode(&mutated) {
                    assert_eq!(
                        image.rgba.len(),
                        (image.width as usize) * (image.height as usize) * 4
                    );
                    assert!(image.rgba.len() <= neo_image_decode::DEFAULT_MAX_DECODE_BYTES);
                }
            }
        }
    }

    #[test]
    fn huge_png_header_is_rejected_before_allocation() {
        // Keep a real PNG's IDAT but claim a 1,000,000 x 1,000,000 canvas. The
        // IHDR precheck must answer TooLarge without ever asking the decoder to
        // allocate for the claim (the default cap is 64 MiB).
        let mut hostile = PNG_RGBA8.to_vec();
        hostile[16..20].copy_from_slice(&1_000_000u32.to_be_bytes());
        hostile[20..24].copy_from_slice(&1_000_000u32.to_be_bytes());
        match decode(&hostile) {
            Err(ImageDecodeError::TooLarge {
                width,
                height,
                limit_bytes,
            }) => {
                assert_eq!((width, height), (1_000_000, 1_000_000));
                assert_eq!(limit_bytes, neo_image_decode::DEFAULT_MAX_DECODE_BYTES);
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    /// Lane 2c regression KAT: the fuzzer minimised this 24-byte input to a PNG
    /// signature plus an `IHDR` whose width and height are both `u32::MAX`.
    /// `(2^32 - 1)^2 * 4` is larger than `u64::MAX`; the size arithmetic must
    /// answer `TooLarge`, not overflow (which panicked in the fuzz build) and not
    /// wrap to a small "valid" length (which would be a cap bypass).
    #[test]
    fn png_ihdr_at_u32_max_is_too_large_not_an_overflow() {
        let hostile = include_bytes!("../tests/data/png/hostile_ihdr_overflow.png");
        assert_eq!(hostile.len(), 24);
        match decode(hostile) {
            Err(ImageDecodeError::TooLarge {
                width,
                height,
                limit_bytes,
            }) => {
                assert_eq!((width, height), (u32::MAX, u32::MAX));
                assert_eq!(limit_bytes, neo_image_decode::DEFAULT_MAX_DECODE_BYTES);
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }
}
