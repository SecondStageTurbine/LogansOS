// SPDX-License-Identifier: MPL-2.0
//! Pure, host-testable CPU separable-Gaussian blur — the OT-63 GPU P1 Slice-1
//! "blur floor" (ADR-077 v2 §1, Slice 1).
//!
//! This is BOTH the cap-native software floor (real frosted glass on any
//! machine, GPU or not — the ADR-024 Reduce-Transparency / I-14(b) fallback)
//! AND the **bit-for-bit reference** the later Gen9 GPU kernel must match
//! (ADR-077 v2 §5.1: GPU-on ≡ CPU-floor, provable because the arithmetic is
//! exact fixed-point integer, never float).
//!
//! ## What this module is
//! - A two-pass **separable** blur (H then V) over a **linear XRGB8888 LE**
//!   pixel buffer. Byte order in memory is `[B, G, R, X]` (DRM `XR24`): the
//!   little-endian u32 `0x??RRGGBB` lands as bytes `BB GG RR XX`. R, G, B are
//!   blurred **independently**; the X byte is **read-ignored** and written
//!   `0xFF` (ADR-077 v2 §1.2).
//! - **Exact Q16 fixed-point** arithmetic: weights sum to exactly `65536`, the
//!   accumulator is `i32`, and the per-pass result is
//!   `clamp((acc + 32768) >> 16, 0, 255)` — deterministic **round-half-up**, no
//!   float. That is what makes the kill-switch and GPU equalities *provable*.
//! - **Explicit bounds checks** (ADR-077 v2 amendment 2): every entry validates
//!   dimensions, stride, weight length, and buffer lengths up front
//!   ([`validate_blur`]) and every sample uses clamp-to-edge addressing. A
//!   walker/thread mask is an ABI concern, never the memory-safety boundary —
//!   the same discipline the GPU kernel must carry.
//! - The **damage halo** rules (ADR-077 v2 amendment 3): a blur of radius `r`
//!   spreads, so the committed output damage is the changed rect expanded by
//!   `r` ([`expand_output_rect`]) and the source read is a further `r` halo —
//!   the changed rect expanded by `2r` ([`source_halo_rect`]).
//! - The **kill-switch** selector ([`glass_blur_enabled`]) and the equality
//!   core ([`apply_blur_or_floor`]): blur-disabled ≡ the solid
//!   Reduce-Transparency floor (the backdrop passes through unblurred),
//!   bit-for-bit; blur-enabled ≡ the two-pass blur (ADR-077 v2 §5.1).
//!
//! ## Host mounting
//! Mounted on the host by `tools/atrium-logic-tests` via `#[path]` (like
//! `logic.rs`) — so keep it free of `crate::` paths, `alloc`, and
//! `cfg(feature = ...)`. It operates only on caller-provided byte slices; the
//! kernel wrapper ([`super::buffer::SharedPixelBuffer::blur_backdrop`]) owns the
//! buffers and the toggle.
//!
//! The literal **8×8 `r=1` byte oracle** (source → H-intermediate → dest, all
//! three asserted, hand-recomputable, memory order `BB GG RR FF`) lives in the
//! host suite and is what the PM independently recomputes (ADR-028 discipline).

// This module is a self-contained pure API whose FULL surface is exercised by
// the host KATs (`tools/atrium-logic-tests::blur_tests`) — the r=8 production
// weights, the `source_halo_rect` incremental rule, and the r=1 oracle path are
// each proven there. In a given kernel build only the subset a scenario uses is
// live (Slice 1's `atrium_glass_blur` drives the r=1 path), so allow dead_code
// here exactly as the mounting test crate does — the KATs, not this build's call
// graph, are the liveness proof.
#![allow(dead_code)]

/// One Q16 unit (weights sum to exactly this).
pub const Q16_ONE: i32 = 65_536;
/// The round-half-up bias added before the `>> 16` (`0.5` in Q16).
pub const Q16_HALF: i32 = 32_768;

/// Bytes per XRGB8888 pixel.
pub const BYTES_PER_PIXEL: usize = 4;
/// Channel byte offsets within a pixel (memory order `[B, G, R, X]`).
pub const OFF_B: usize = 0;
pub const OFF_G: usize = 1;
pub const OFF_R: usize = 2;
pub const OFF_X: usize = 3;
/// The X byte written to every output pixel (ADR-077 v2 §1.2: X is opaque).
pub const OUT_X_BYTE: u8 = 0xFF;

/// Fixed vibrancy tint applied after the backdrop has been blurred.  The
/// compositor's glass material stays integer-only for the same reason as the
/// blur: GPU-on, GPU-off, and forced-degrade must all produce the same bytes.
/// Channels are in the in-memory XRGB order `[B, G, R]`; `alpha` is the tint
/// contribution in `[0, 255]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GlassTint {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub alpha: u8,
}

/// The pinned G1 glass material: a cool, lightly blue-white vibrancy layer at
/// 25%.  This is a compositor material constant, not a client-controlled
/// shader/input.
pub const G1_VIBRANCY_TINT: GlassTint = GlassTint {
    b: 0x80,
    g: 0xC0,
    r: 0xFF,
    alpha: 64,
};

/// The pinned production radius (ADR-077 v2 §1.2: `r = 8` → a 17-tap kernel).
pub const BLUR_RADIUS_R8: usize = 8;

/// The `r = 1` Q16 weights used by the **byte oracle** (`w = [16384, 32768,
/// 16384]`, sum `65536`). Small radius makes the hand-laid oracle tractable
/// while still exercising clamp edges + the round-half-up boundary
/// (ADR-077 v2 §1.5).
pub const WEIGHTS_R1_Q16: [i32; 3] = [16_384, 32_768, 16_384];

/// The pinned production `r = 8`, σ = 4.0 Q16 Gaussian weights (17 taps), summing
/// to **exactly** `65536` (the center tap absorbs the rounding residual — here
/// the residual is 0, so the samples are the pure rounded Gaussian). Symmetric.
/// Generated offline by `tools/atrium-logic-tests/oracle_gen/blur_weights.py`:
/// `raw[k] = exp(-k²/(2σ²))`, normalize to Q16, round, fix Σ==65536 on center.
/// Provenance discipline (ADR-077 v2 §1.2): the generator + its output are
/// pinned; a host KAT re-asserts Σ==65536 and symmetry so drift fails the build.
pub const WEIGHTS_R8_Q16: [i32; 17] = [
    915, 1462, 2195, 3095, 4100, 5103, 5966, 6552, // k = -8 ..= -1
    6760, // k = 0 (center)
    6552, 5966, 5103, 4100, 3095, 2195, 1462, 915, // k = +1 ..= +8
];

/// Blur geometry over a linear XRGB8888 buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BlurDims {
    /// Pixel width.
    pub width: usize,
    /// Pixel height.
    pub height: usize,
    /// Row pitch in **bytes** (`>= width * 4`).
    pub stride: usize,
    /// Kernel radius; the weight table must be `2 * radius + 1` long.
    pub radius: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlurError {
    /// `width` or `height` is zero.
    ZeroDim,
    /// `stride < width * 4`.
    StrideTooSmall,
    /// `weights.len() != 2 * radius + 1`.
    WeightLenMismatch,
    /// A source or destination slice is shorter than the buffer requires.
    BufferTooSmall,
    /// Checked size arithmetic overflowed `usize`.
    Overflow,
}

/// The minimum byte length a buffer of these dims requires: the last row only
/// needs `width * 4` bytes, so the tight bound is `(height - 1) * stride +
/// width * 4` (cross-check §2). All arithmetic is checked.
pub fn required_len(dims: BlurDims) -> Result<usize, BlurError> {
    if dims.width == 0 || dims.height == 0 {
        return Err(BlurError::ZeroDim);
    }
    let row_bytes = dims
        .width
        .checked_mul(BYTES_PER_PIXEL)
        .ok_or(BlurError::Overflow)?;
    if dims.stride < row_bytes {
        return Err(BlurError::StrideTooSmall);
    }
    let full = dims
        .height
        .checked_sub(1)
        .and_then(|h1| h1.checked_mul(dims.stride))
        .and_then(|body| body.checked_add(row_bytes))
        .ok_or(BlurError::Overflow)?;
    Ok(full)
}

/// Validate dims + weights against the src/dst lengths. Returns the required
/// minimum length on success. This is the single up-front memory-safety gate
/// (ADR-077 v2 amendment 2): after it passes, every index the passes compute is
/// provably in range, so the sample loop needs no per-tap length re-check.
pub fn validate_blur(
    dims: BlurDims,
    weights: &[i32],
    src_len: usize,
    dst_len: usize,
) -> Result<usize, BlurError> {
    if weights.len() != 2 * dims.radius + 1 {
        return Err(BlurError::WeightLenMismatch);
    }
    let need = required_len(dims)?;
    if src_len < need || dst_len < need {
        return Err(BlurError::BufferTooSmall);
    }
    Ok(need)
}

/// Clamp-to-edge sample index along one axis: an out-of-range tap replicates the
/// border pixel (ADR-077 v2 §1.2 edge rule). `dim` is the axis length (`>= 1`).
#[inline]
fn clamp_index(center: usize, offset: isize, dim: usize) -> usize {
    let raw = center as isize + offset;
    if raw < 0 {
        0
    } else if raw as usize >= dim {
        dim - 1
    } else {
        raw as usize
    }
}

/// One separable pass with a caller-chosen tap axis.
///
/// `horizontal = true` gathers `2r+1` taps along X (clamp-to-edge in X) — Pass
/// H, `src → mid`. `horizontal = false` gathers along Y — Pass V, `mid → dst`.
/// Per output pixel: R, G, B each `clamp((Σ w[k]·sample + 32768) >> 16, 0,255)`;
/// the X byte is written [`OUT_X_BYTE`] regardless of the source X.
fn blur_pass(
    src: &[u8],
    dst: &mut [u8],
    dims: BlurDims,
    weights: &[i32],
    horizontal: bool,
) -> Result<(), BlurError> {
    validate_blur(dims, weights, src.len(), dst.len())?;
    let BlurDims {
        width,
        height,
        stride,
        radius,
    } = dims;
    let r = radius as isize;
    for y in 0..height {
        for x in 0..width {
            let base = y * stride + x * BYTES_PER_PIXEL;
            for c in [OFF_B, OFF_G, OFF_R] {
                let mut acc: i32 = 0;
                for (k, &wk) in weights.iter().enumerate() {
                    let offset = k as isize - r;
                    // Sample index: clamp-to-edge along the pass axis; the other
                    // axis is fixed at (x for V / y for H).
                    let sample_off = if horizontal {
                        let sx = clamp_index(x, offset, width);
                        y * stride + sx * BYTES_PER_PIXEL + c
                    } else {
                        let sy = clamp_index(y, offset, height);
                        sy * stride + x * BYTES_PER_PIXEL + c
                    };
                    acc += wk * src[sample_off] as i32;
                }
                // Round-half-up then clamp to [0, 255]. `acc` peaks at
                // 65536 * 255 = 16_711_680, well inside i32 (cross-check §1.4).
                let out = ((acc + Q16_HALF) >> 16).clamp(0, 255) as u8;
                dst[base + c] = out;
            }
            dst[base + OFF_X] = OUT_X_BYTE;
        }
    }
    Ok(())
}

/// Pass H (horizontal): `src → mid`, clamp-to-edge in X.
pub fn blur_pass_h(
    src: &[u8],
    mid: &mut [u8],
    dims: BlurDims,
    weights: &[i32],
) -> Result<(), BlurError> {
    blur_pass(src, mid, dims, weights, true)
}

/// Pass V (vertical): `mid → dst`, clamp-to-edge in Y.
pub fn blur_pass_v(
    mid: &[u8],
    dst: &mut [u8],
    dims: BlurDims,
    weights: &[i32],
) -> Result<(), BlurError> {
    blur_pass(mid, dst, dims, weights, false)
}

/// Copy the backdrop through unblurred (the solid Reduce-Transparency floor at
/// this stage: no blur applied). `dst` becomes `src` bit-for-bit over the
/// live `width * 4` bytes of every row (stride padding is left untouched).
pub fn copy_backdrop(src: &[u8], dst: &mut [u8], dims: BlurDims) -> Result<(), BlurError> {
    // `radius`/weights are irrelevant to a passthrough; validate geometry only.
    let need = required_len(dims)?;
    if src.len() < need || dst.len() < need {
        return Err(BlurError::BufferTooSmall);
    }
    let row_bytes = dims.width * BYTES_PER_PIXEL;
    for y in 0..dims.height {
        let base = y * dims.stride;
        dst[base..base + row_bytes].copy_from_slice(&src[base..base + row_bytes]);
    }
    Ok(())
}

/// The two-layer kill-switch selector (ADR-077 v2 §5.1), mirroring
/// [`super::logic::atrium_syscalls_enabled`]: the glass blur runs only when the
/// compile-time feature AND the runtime toggle are both on. Slice-1 default is
/// **off** — the backdrop passes through as the solid floor.
#[inline]
pub const fn glass_blur_enabled(feature_on: bool, runtime_on: bool) -> bool {
    feature_on && runtime_on
}

/// The kill-switch equality core (ADR-077 v2 §5.1): when `enabled`, run the
/// two-pass H→V blur (`src → mid → dst`); when disabled, pass the backdrop
/// through unblurred (`dst = src`, the solid Reduce-Transparency floor) —
/// **bit-for-bit** what the composite produced before the blur stage existed.
/// `mid` is a scratch buffer of the same geometry, used only when `enabled`.
pub fn apply_blur_or_floor(
    src: &[u8],
    mid: &mut [u8],
    dst: &mut [u8],
    dims: BlurDims,
    weights: &[i32],
    enabled: bool,
) -> Result<(), BlurError> {
    if enabled {
        blur_pass_h(src, mid, dims, weights)?;
        blur_pass_v(mid, dst, dims, weights)?;
        Ok(())
    } else {
        copy_backdrop(src, dst, dims)
    }
}

/// Apply the fixed G1 vibrancy layer to a blurred (or Reduce-Transparency
/// floor) backdrop.  This is deliberately a separate pure stage from the
/// blur: the live compositor sequence is capture -> blur/floor -> tint ->
/// ordinary G1 composition, and the compute fence never implies scanout.
///
/// Integer source-over form, per RGB channel:
/// `round_half_up((src * (255 - a) + tint * a) / 255)`.  The source X byte is
/// ignored and output X is always [`OUT_X_BYTE`], preserving the XRGB contract.
pub fn apply_vibrancy_tint(
    src: &[u8],
    dst: &mut [u8],
    dims: BlurDims,
    tint: GlassTint,
) -> Result<(), BlurError> {
    let need = required_len(dims)?;
    if src.len() < need || dst.len() < need {
        return Err(BlurError::BufferTooSmall);
    }
    let row_bytes = dims.width * BYTES_PER_PIXEL;
    let alpha = tint.alpha as u32;
    let inverse = 255u32 - alpha;
    for y in 0..dims.height {
        let row = y * dims.stride;
        for x in 0..row_bytes / BYTES_PER_PIXEL {
            let off = row + x * BYTES_PER_PIXEL;
            for (channel, tint_channel) in [(OFF_B, tint.b), (OFF_G, tint.g), (OFF_R, tint.r)] {
                let mixed = src[off + channel] as u32 * inverse + tint_channel as u32 * alpha + 127;
                dst[off + channel] = (mixed / 255) as u8;
            }
            dst[off + OFF_X] = OUT_X_BYTE;
        }
    }
    Ok(())
}

// === Damage halo (ADR-077 v2 amendment 3) ===

/// An integer pixel rectangle (surface/buffer coordinates).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BlurRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Expand `rect` outward by `margin` on every side, clamped to the surface
/// `[0, surf_w) × [0, surf_h)`. The workhorse for both halo rules. Saturating on
/// the low side, min-clamped on the high side; a `rect` already outside the
/// surface yields a zero-area rect.
fn expand_clamped(rect: BlurRect, margin: u32, surf_w: u32, surf_h: u32) -> BlurRect {
    let x0 = rect.x.saturating_sub(margin);
    let y0 = rect.y.saturating_sub(margin);
    // right/bottom edges (exclusive), saturating then clamped to the surface.
    let x1 = rect
        .x
        .saturating_add(rect.width)
        .saturating_add(margin)
        .min(surf_w);
    let y1 = rect
        .y
        .saturating_add(rect.height)
        .saturating_add(margin)
        .min(surf_h);
    BlurRect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    }
}

/// The **output** region affected when the backdrop cell `rect` changes: a blur
/// of radius `r` spreads a changed source pixel up to `r` away, so the affected
/// output — and therefore the committed damage — is `rect` expanded by `r`,
/// clamped to the surface (ADR-077 v2 amendment 3).
pub fn expand_output_rect(rect: BlurRect, radius: u32, surf_w: u32, surf_h: u32) -> BlurRect {
    expand_clamped(rect, radius, surf_w, surf_h)
}

/// The **source** region that must be READ to recompute [`expand_output_rect`]:
/// each affected output pixel itself gathers taps `±r`, so the source halo is
/// the output rect expanded by another `r` — i.e. `rect` expanded by `2r`,
/// clamped to the surface (ADR-077 v2 amendment 3). Backdrop damage behind a
/// glass surface invalidates that surface even with no client damage; that
/// scene-level rule is the compositor slice's, but the halo math is pinned here.
pub fn source_halo_rect(rect: BlurRect, radius: u32, surf_w: u32, surf_h: u32) -> BlurRect {
    let two_r = radius.saturating_mul(2);
    expand_clamped(rect, two_r, surf_w, surf_h)
}
