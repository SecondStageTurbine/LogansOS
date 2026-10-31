// SPDX-License-Identifier: MPL-2.0
//! Exact-area scanline rasterizer: outline segments -> 8-bit coverage.
//!
//! Each line segment deposits, per pixel row it crosses, the signed area it
//! sweeps to its right into an accumulation buffer as *differences*; a running
//! prefix sum along the row then yields every pixel's covered area exactly
//! (non-zero winding via `|sum|`, saturated at 1). This is the accumulation
//! scheme popularised by font-rs (Apache-2.0); this file is an independent
//! implementation of the algorithm, not a copy.
//!
//! Curves are flattened into line segments with a subdivision count bounded by
//! the curve's second difference so the polyline stays within [`FLATTEN_TOL`]
//! pixels of the true curve. No libm: only `+ - * /`, comparisons and casts,
//! so host and target produce bit-identical coverage.

use alloc::vec;
use alloc::vec::Vec;

/// Maximum distance (pixels) between a flattened curve and its polyline.
pub const FLATTEN_TOL: f32 = 0.005;
const MAX_SEGMENTS: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// `floor` without libm. Valid for |x| < 2^31, which bounds every coordinate
/// a glyph bitmap can have.
pub fn floor(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t > x { t - 1.0 } else { t }
}

pub fn ceil(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t < x { t + 1.0 } else { t }
}

pub struct Rasterizer {
    width: usize,
    height: usize,
    /// Row stride is `width + 2`: a segment touching x == width deposits into
    /// columns `width` and `width + 1`, which are never read back.
    acc: Vec<f32>,
}

impl Rasterizer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            acc: vec![0.0; (width + 2) * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Deposit one directed edge (y grows downward). Points are clamped into
    /// the bitmap; callers size the bitmap from the outline's bounds.
    pub fn line(&mut self, a: Point, b: Point) {
        let w = self.width as f32;
        let h = self.height as f32;
        let a = Point::new(a.x.clamp(0.0, w), a.y.clamp(0.0, h));
        let b = Point::new(b.x.clamp(0.0, w), b.y.clamp(0.0, h));
        if a.y == b.y {
            return;
        }
        let (dir, top, bottom) = if a.y < b.y {
            (1.0f32, a, b)
        } else {
            (-1.0f32, b, a)
        };
        let dxdy = (bottom.x - top.x) / (bottom.y - top.y);
        let mut x = top.x;
        let mut row = floor(top.y) as usize;
        while row < self.height && (row as f32) < bottom.y {
            let y0 = (row as f32).max(top.y);
            let y1 = ((row + 1) as f32).min(bottom.y);
            let dy = y1 - y0;
            if dy > 0.0 {
                let x_next = if y1 == bottom.y {
                    bottom.x
                } else {
                    x + dxdy * dy
                };
                self.span(row, x, x_next, dy * dir);
                x = x_next;
            }
            row += 1;
        }
    }

    /// One edge piece inside pixel row `row`, from x `xa` to `xb`, sweeping
    /// signed height `d`. Writes the per-column differences of
    /// `contrib(c) = d * (area of pixel c right of the piece) / height`.
    fn span(&mut self, row: usize, xa: f32, xb: f32, d: f32) {
        let base = row * (self.width + 2);
        let (x0, x1) = if xa < xb { (xa, xb) } else { (xb, xa) };
        let c0f = floor(x0);
        let c0 = c0f as usize;
        let c1 = (ceil(x1) as usize).saturating_sub(1).max(c0);
        if c1 == c0 {
            let mid = 0.5 * (x0 + x1) - c0f;
            self.acc[base + c0] += d * (1.0 - mid);
            self.acc[base + c0 + 1] += d * mid;
            return;
        }
        // Proportion of the piece's height per unit of x.
        let s = 1.0 / (x1 - x0);
        let x0f = x0 - c0f;
        let x1f = x1 - c1 as f32;
        let first = d * 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
        self.acc[base + c0] += first;
        let mut prev = first;
        for c in c0 + 1..c1 {
            let contrib = d * s * ((c - c0) as f32 + 0.5 - x0f);
            self.acc[base + c] += contrib - prev;
            prev = contrib;
        }
        let last = d * (1.0 - 0.5 * s * x1f * x1f);
        self.acc[base + c1] += last - prev;
        self.acc[base + c1 + 1] += d - last;
    }

    pub fn quad(&mut self, p0: Point, p1: Point, p2: Point) {
        let ddx = p0.x - 2.0 * p1.x + p2.x;
        let ddy = p0.y - 2.0 * p1.y + p2.y;
        // Uniform n-way subdivision deviates at most |dd| / (4 n^2) from the
        // curve; pick the smallest n meeting FLATTEN_TOL (squared to avoid sqrt).
        let bound = (ddx * ddx + ddy * ddy) / (16.0 * FLATTEN_TOL * FLATTEN_TOL);
        let n = segments_for(bound);
        let mut prev = p0;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let mt = 1.0 - t;
            let p = Point::new(
                mt * mt * p0.x + 2.0 * mt * t * p1.x + t * t * p2.x,
                mt * mt * p0.y + 2.0 * mt * t * p1.y + t * t * p2.y,
            );
            let p = if i == n { p2 } else { p };
            self.line(prev, p);
            prev = p;
        }
    }

    pub fn cubic(&mut self, p0: Point, p1: Point, p2: Point, p3: Point) {
        let ax = p0.x - 2.0 * p1.x + p2.x;
        let ay = p0.y - 2.0 * p1.y + p2.y;
        let bx = p1.x - 2.0 * p2.x + p3.x;
        let by = p1.y - 2.0 * p2.y + p3.y;
        let dd = (ax * ax + ay * ay).max(bx * bx + by * by);
        // Cubic: deviation <= 3 |dd|max / (4 n^2).
        let bound = dd * 9.0 / (16.0 * FLATTEN_TOL * FLATTEN_TOL);
        let n = segments_for(bound);
        let mut prev = p0;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let mt = 1.0 - t;
            let (a, b, c, e) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
            let p = Point::new(
                a * p0.x + b * p1.x + c * p2.x + e * p3.x,
                a * p0.y + b * p1.y + c * p2.y + e * p3.y,
            );
            let p = if i == n { p3 } else { p };
            self.line(prev, p);
            prev = p;
        }
    }

    /// Resolve the accumulation buffer into row-major 8-bit coverage
    /// (`round(min(|area|, 1) * 255)`).
    pub fn coverage(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.width * self.height];
        for row in 0..self.height {
            let base = row * (self.width + 2);
            let mut sum = 0.0f32;
            for col in 0..self.width {
                sum += self.acc[base + col];
                let area = if sum < 0.0 { -sum } else { sum };
                let area = if area > 1.0 { 1.0 } else { area };
                out[row * self.width + col] = (area * 255.0 + 0.5) as u8;
            }
        }
        out
    }
}

/// Smallest n in 1..=MAX_SEGMENTS with n^4 >= bound.
fn segments_for(bound: f32) -> u32 {
    let mut n = 1u32;
    while n < MAX_SEGMENTS && ((n * n * n * n) as f32) < bound {
        n += 1;
    }
    n
}
