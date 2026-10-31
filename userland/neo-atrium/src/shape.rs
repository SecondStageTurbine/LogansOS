// SPDX-License-Identifier: MPL-2.0
//! Coverage-correct rounded rectangles.
//!
//! A corner pixel's coverage is `round_half_up(255·A)`, `A` the *exact* area
//! of the unit pixel square inside the quarter disc, from the closed-form
//! integral of `√(r² − t²)`. No supersampling, so edges are as smooth as 8-bit
//! coverage allows and the tables are oracle-checkable by hand.

use alloc::vec::Vec;

/// `∫₀ᵗ √(r² − u²) du`.
fn disc_integral(t: f64, r: f64) -> f64 {
    let inside = libm::sqrt((r * r - t * t).max(0.0));
    0.5 * (t * inside + r * r * libm::asin((t / r).clamp(-1.0, 1.0)))
}

/// Exact area of the square `[x0, x0+1] × [y0, y0+1]` (x0, y0 ≥ 0) inside the
/// disc of radius `r` centred on the origin.
pub fn pixel_disc_area(x0: f64, y0: f64, r: f64) -> f64 {
    let (x1, y1) = (x0 + 1.0, y0 + 1.0);
    if x0 >= r || y0 >= r {
        return 0.0;
    }
    // h(t) = √(r² − t²) crosses y1 at ta and y0 at tb.
    let ta = if y1 < r {
        libm::sqrt(r * r - y1 * y1)
    } else {
        0.0
    };
    let tb = libm::sqrt(r * r - y0 * y0);
    let a = ta.clamp(x0, x1);
    let b = tb.clamp(x0, x1);
    let full = a - x0;
    let partial = disc_integral(b, r) - disc_integral(a, r) - y0 * (b - a);
    (full + partial).clamp(0.0, 1.0)
}

/// `round_half_up(255·area)`.
pub fn coverage_byte(area: f64) -> u8 {
    libm::floor(255.0 * area + 0.5) as u8
}

/// The `r × r` corner mask for the top-left corner: index `j·r + i` is the
/// coverage of the pixel `i` columns and `j` rows in from the corner.
pub fn corner_mask(r: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(r * r);
    for j in 0..r {
        for i in 0..r {
            let x0 = (r - i - 1) as f64;
            let y0 = (r - j - 1) as f64;
            out.push(coverage_byte(pixel_disc_area(x0, y0, r as f64)));
        }
    }
    out
}

/// A rounded rectangle with a precomputed corner mask.
#[derive(Clone, Debug)]
pub struct RoundRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub r: usize,
    mask: Vec<u8>,
}

impl RoundRect {
    pub fn new(x: usize, y: usize, w: usize, h: usize, r: usize) -> Self {
        let r = r.min(w / 2).min(h / 2);
        Self {
            x,
            y,
            w,
            h,
            r,
            mask: corner_mask(r),
        }
    }

    /// Coverage (0..=255) of pixel `(px, py)`.
    pub fn coverage(&self, px: usize, py: usize) -> u32 {
        if px < self.x || py < self.y || px >= self.x + self.w || py >= self.y + self.h {
            return 0;
        }
        let (dx, dy) = (px - self.x, py - self.y);
        let r = self.r;
        let i = if dx < r {
            Some(dx)
        } else if dx >= self.w - r {
            Some(self.w - 1 - dx)
        } else {
            None
        };
        let j = if dy < r {
            Some(dy)
        } else if dy >= self.h - r {
            Some(self.h - 1 - dy)
        } else {
            None
        };
        match (i, j) {
            (Some(i), Some(j)) => self.mask[j * r + i] as u32,
            _ => 255,
        }
    }

    /// The same shape inset by one pixel on every side (radius `r − 1`).
    pub fn inset1(&self) -> RoundRect {
        RoundRect::new(
            self.x + 1,
            self.y + 1,
            self.w.saturating_sub(2),
            self.h.saturating_sub(2),
            self.r.saturating_sub(1),
        )
    }
}
