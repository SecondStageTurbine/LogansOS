// SPDX-License-Identifier: MPL-2.0
//! Wide (8.8 fixed-point) colour helpers and the ordered dither.

/// A colour in wide units: each channel is an 8-bit value × 256.
pub type Wide = [u32; 3];

/// The 4×4 Bayer ordered-dither thresholds (0..15).
pub const BAYER4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// White (255) in wide units.
pub const WHITE_W: u32 = 255 * 256;

/// Expand an XRGB8888 token to wide units.
pub const fn widen(rgb: u32) -> Wide {
    [
        ((rgb >> 16) & 0xFF) * 256,
        ((rgb >> 8) & 0xFF) * 256,
        (rgb & 0xFF) * 256,
    ]
}

/// Dither one wide pixel at `(x, y)` to XRGB8888 (top byte 0).
pub fn dither(w: Wide, x: usize, y: usize) -> u32 {
    let t = BAYER4[y & 3][x & 3] * 16 + 8;
    let c = |v: u32| ((v + t) >> 8).min(255);
    (c(w[0]) << 16) | (c(w[1]) << 8) | c(w[2])
}

/// `top` over `below` at 8-bit coverage `c`: `(top·c + below·(255−c) + 127) / 255`.
pub fn mix(top: Wide, below: Wide, c: u32) -> Wide {
    let m = |t: u32, b: u32| (t * c + b * (255 - c) + 127) / 255;
    [
        m(top[0], below[0]),
        m(top[1], below[1]),
        m(top[2], below[2]),
    ]
}

/// Lighten toward white at 8-bit alpha `a`: `w + ((65280 − w)·a + 127) / 255`.
pub fn lighten(w: Wide, a: u32) -> Wide {
    let l = |v: u32| v + ((WHITE_W.saturating_sub(v)) * a + 127) / 255;
    [l(w[0]), l(w[1]), l(w[2])]
}
