// SPDX-License-Identifier: MPL-2.0
//! The Threshold ambient backdrop: "intelligence as weather" — a calm Void
//! ground lit from within by two soft lights and the two flare-star ring arcs,
//! smooth and low-frequency so it is cheap to recompute for any damage rect.

use crate::wide::Wide;

/// Void (design-system §1).
pub const VOID: u32 = 0x000A_0E27;
/// The bottom of the vertical rise (as in S1).
pub const THRESHOLD: u32 = 0x0012_1C42;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Light {
    pub cx: i64,
    pub cy: i64,
    pub r: i64,
    /// Peak added colour, 8-bit units.
    pub peak: [u32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ring {
    pub cx: i64,
    pub cy: i64,
    pub r: i64,
    pub peak: [u32; 3],
}

/// Half-width of a ring's profile in 1/16 px (1.5 px).
pub const RING_HALF_WIDTH_16: i64 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Backdrop {
    pub width: usize,
    pub height: usize,
    pub lights: [Light; 2],
    pub rings: [Ring; 2],
}

impl Backdrop {
    /// The S3 backdrop for a `width × height` output. All geometry is integer
    /// and derived from the output size, so any GOP mode gets the same picture.
    pub fn new(width: usize, height: usize) -> Self {
        let (w, h) = (width as i64, height as i64);
        let (cx, cy) = (w * 62 / 100, h * 48 / 100);
        Self {
            width,
            height,
            lights: [
                // Electric Blue light, behind the workfield.
                Light {
                    cx,
                    cy,
                    r: h * 70 / 100,
                    peak: [16, 40, 86],
                },
                // Low-saturation violet depth, lower left (Threshold §12).
                Light {
                    cx: w * 12 / 100,
                    cy: h * 96 / 100,
                    r: h * 60 / 100,
                    peak: [26, 10, 44],
                },
            ],
            rings: [
                Ring {
                    cx,
                    cy,
                    r: h * 58 / 100,
                    peak: [22, 48, 96],
                },
                Ring {
                    cx,
                    cy,
                    r: h * 80 / 100,
                    peak: [16, 34, 70],
                },
            ],
        }
    }

    /// The backdrop at integer pixel `(x, y)`, in wide units.
    pub fn pixel(&self, x: usize, y: usize) -> Wide {
        let span = self.height.saturating_sub(1).max(1) as u64;
        let t = (y as u64).min(span);
        let mut out = [0u64; 3];
        let void = crate::wide::widen(VOID);
        let thr = crate::wide::widen(THRESHOLD);
        for c in 0..3 {
            out[c] = (void[c] as u64 * (span - t) + thr[c] as u64 * t + span / 2) / span;
        }
        let (xi, yi) = (x as i64, y as i64);
        for l in &self.lights {
            let d2 = (xi - l.cx).pow(2) + (yi - l.cy).pow(2);
            let r2 = l.r * l.r;
            if r2 > 0 && d2 < r2 {
                let q = (((r2 - d2) as u64) << 16) / r2 as u64;
                let f = (q * q) >> 16;
                for (o, p) in out.iter_mut().zip(l.peak) {
                    *o += (p as u64 * 256 * f) >> 16;
                }
            }
        }
        for g in &self.rings {
            if g.r <= 0 {
                continue;
            }
            let d2 = (xi - g.cx).pow(2) + (yi - g.cy).pow(2);
            // Truncating division (toward zero), as specified.
            let s = (8 * (d2 - g.r * g.r)) / g.r;
            let a = (RING_HALF_WIDTH_16 - s.abs()).max(0) as u64;
            if a > 0 {
                let f = (a << 16) / RING_HALF_WIDTH_16 as u64;
                for (o, p) in out.iter_mut().zip(g.peak) {
                    *o += (p as u64 * 256 * f) >> 16;
                }
            }
        }
        [
            out[0].min(65535) as u32,
            out[1].min(65535) as u32,
            out[2].min(65535) as u32,
        ]
    }
}
