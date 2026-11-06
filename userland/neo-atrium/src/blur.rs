// SPDX-License-Identifier: MPL-2.0
//! The frosted-glass blur: three passes of a separable integer box filter
//! (≈ a Gaussian of σ ≈ R·√(3·…)), edge-clamped, computed once per glass
//! shape and cached. Every call increments [`blur_calls`] so the gate can prove
//! that typing never re-blurs anything.

use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

/// Box radius of one blur pass (three passes ⇒ σ ≈ 8.7 px).
pub const BLUR_RADIUS: usize = 8;
/// How far the blur source extends beyond the glass box.
pub const BLUR_MARGIN: usize = 3 * BLUR_RADIUS;

static BLUR_CALLS: AtomicU32 = AtomicU32::new(0);

// Host tests run in parallel threads; count per thread there so one test's
// scenes cannot perturb another's count. The WM (no_std) uses the global.
#[cfg(test)]
std::thread_local! {
    static TEST_BLUR_CALLS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

fn count_blur() {
    BLUR_CALLS.fetch_add(1, Ordering::Relaxed);
    #[cfg(test)]
    TEST_BLUR_CALLS.with(|c| c.set(c.get() + 1));
}

/// How many region blurs have run in this process (in host tests: this thread).
pub fn blur_calls() -> u32 {
    #[cfg(test)]
    return TEST_BLUR_CALLS.with(|c| c.get());
    #[cfg(not(test))]
    BLUR_CALLS.load(Ordering::Relaxed)
}

/// One box pass of radius `r` over `src` (length `n`, stride `step`, starting
/// at `base`), edge-clamped, written to `dst` at the same positions.
fn box_line(src: &[u32], dst: &mut [u32], base: usize, n: usize, step: usize, r: usize) {
    if n == 0 {
        return;
    }
    let at = |i: isize| -> u32 {
        let i = i.clamp(0, n as isize - 1) as usize;
        src[base + i * step]
    };
    let width = (2 * r + 1) as u32;
    let mut sum: u32 = (-(r as isize)..=r as isize).map(at).sum();
    for i in 0..n {
        dst[base + i * step] = (sum + r as u32) / width;
        sum += at(i as isize + r as isize + 1);
        sum -= at(i as isize - r as isize);
    }
}

/// Blur one channel plane (`w × h`, tightly packed) in place: `passes` ×
/// (horizontal box, then vertical box) of radius `r`.
pub fn blur_plane(plane: &mut [u32], w: usize, h: usize, r: usize, passes: usize) {
    // F7 (drag smoothness): the glass's own radius takes the fast path, the
    // same arithmetic in the same order (bit-identical, see the tests).
    if r == BLUR_RADIUS {
        return blur_plane_fixed::<BLUR_RADIUS>(plane, w, h, passes);
    }
    blur_plane_generic(plane, w, h, r, passes);
}

/// [`blur_plane`] for any radius: one [`box_line`] per row, then per column.
fn blur_plane_generic(plane: &mut [u32], w: usize, h: usize, r: usize, passes: usize) {
    let mut tmp = vec![0u32; plane.len()];
    for _ in 0..passes {
        for y in 0..h {
            box_line(plane, &mut tmp, y * w, w, 1, r);
        }
        for x in 0..w {
            box_line(&tmp, plane, x, h, w, r);
        }
    }
}

/// [`blur_plane`] for a compile-time radius `R`: the divide by the box width
/// is by a constant (a multiply, not a `div` per pixel, which emulated CPUs
/// run as a slow helper), and the vertical pass walks rows, keeping one
/// running sum per column, instead of striding down each column. Each output
/// is the same `(sum + R) / (2R + 1)` of the same edge-clamped sum as
/// [`box_line`]'s.
fn blur_plane_fixed<const R: usize>(plane: &mut [u32], w: usize, h: usize, passes: usize) {
    if w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0u32; plane.len()];
    let mut sums = vec![0u32; w];
    for _ in 0..passes {
        for y in 0..h {
            let row = y * w;
            box_row_fixed::<R>(&plane[row..row + w], &mut tmp[row..row + w]);
        }
        box_cols_fixed::<R>(&tmp, plane, w, h, &mut sums);
    }
}

#[inline(always)]
fn box_div<const R: usize>(sum: u32) -> u32 {
    (sum + R as u32) / (2 * R as u32 + 1)
}

/// One horizontal box pass of radius `R` over `src` into `dst` (one row).
fn box_row_fixed<const R: usize>(src: &[u32], dst: &mut [u32]) {
    let n = src.len();
    let at = |i: isize| -> u32 { src[i.clamp(0, n as isize - 1) as usize] };
    let r = R as isize;
    let mut sum: u32 = (-r..=r).map(at).sum();
    for (i, out) in dst.iter_mut().enumerate().take(n) {
        *out = box_div::<R>(sum);
        sum += at(i as isize + r + 1);
        sum -= at(i as isize - r);
    }
}

/// One vertical box pass of radius `R` over `src` (`w × h`) into `dst`, row
/// by row, with `sums` holding each column's running sum.
fn box_cols_fixed<const R: usize>(
    src: &[u32],
    dst: &mut [u32],
    w: usize,
    h: usize,
    sums: &mut [u32],
) {
    let r = R as isize;
    let row = |y: isize| -> usize { y.clamp(0, h as isize - 1) as usize * w };
    sums.fill(0);
    for dy in -r..=r {
        let base = row(dy);
        for (sum, &v) in sums.iter_mut().zip(&src[base..base + w]) {
            *sum += v;
        }
    }
    for y in 0..h {
        let out = y * w;
        let (add, sub) = (row(y as isize + r + 1), row(y as isize - r));
        for x in 0..w {
            dst[out + x] = box_div::<R>(sums[x]);
            sums[x] += src[add + x];
            sums[x] -= src[sub + x];
        }
    }
}

/// A cached blur of the backdrop over one glass box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Blurred {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    /// Wide values, three planes of `w × h`.
    pub planes: [Vec<u32>; 3],
}

impl Blurred {
    pub fn at(&self, x: usize, y: usize) -> [u32; 3] {
        let i = (y - self.y) * self.w + (x - self.x);
        [self.planes[0][i], self.planes[1][i], self.planes[2][i]]
    }
}

/// Blur `source(x, y)` (wide) over the box `(x, y, w, h)` of a
/// `screen_w × screen_h` output, sourcing [`BLUR_MARGIN`] pixels around it.
pub fn blur_region(
    source: impl Fn(usize, usize) -> [u32; 3],
    screen_w: usize,
    screen_h: usize,
    (x, y, w, h): (usize, usize, usize, usize),
) -> Blurred {
    count_blur();
    let sx = x.saturating_sub(BLUR_MARGIN);
    let sy = y.saturating_sub(BLUR_MARGIN);
    let ex = (x + w + BLUR_MARGIN).min(screen_w);
    let ey = (y + h + BLUR_MARGIN).min(screen_h);
    let (sw, sh) = (ex - sx, ey - sy);
    let mut planes = [
        vec![0u32; sw * sh],
        vec![0u32; sw * sh],
        vec![0u32; sw * sh],
    ];
    for yy in 0..sh {
        for xx in 0..sw {
            let p = source(sx + xx, sy + yy);
            for c in 0..3 {
                planes[c][yy * sw + xx] = p[c];
            }
        }
    }
    for plane in &mut planes {
        blur_plane(plane, sw, sh, BLUR_RADIUS, 3);
    }
    let crop = |plane: &Vec<u32>| -> Vec<u32> {
        let mut out = Vec::with_capacity(w * h);
        for yy in 0..h {
            let row = (y - sy + yy) * sw + (x - sx);
            out.extend_from_slice(&plane[row..row + w]);
        }
        out
    };
    Blurred {
        x,
        y,
        w,
        h,
        planes: [crop(&planes[0]), crop(&planes[1]), crop(&planes[2])],
    }
}

#[cfg(test)]
mod fast_path_tests {
    use super::*;

    /// A deterministic plane of wide-range values (xorshift, no `rand`).
    fn plane(w: usize, h: usize, seed: u64) -> Vec<u32> {
        let mut x = seed | 1;
        (0..w * h)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                // Up to the wide range's top (255 << 8) and a little past it.
                (x % 70_000) as u32
            })
            .collect()
    }

    /// F7: the fast path is bit-identical to the per-line reference for the
    /// glass radius, on every shape the WM can hand it: wider and narrower
    /// than the box, a single row or column, and a frame-sized region.
    #[test]
    fn fixed_radius_path_equals_the_reference() {
        for (w, h) in [
            (1, 1),
            (1, 40),
            (40, 1),
            (3, 5),
            (16, 17),
            (17, 16),
            (33, 34),
            (97, 61),
            (704, 490),
        ] {
            for passes in [1, 3] {
                let src = plane(w, h, (w * 1000 + h) as u64);
                let mut fast = src.clone();
                let mut reference = src.clone();
                blur_plane(&mut fast, w, h, BLUR_RADIUS, passes);
                blur_plane_generic(&mut reference, w, h, BLUR_RADIUS, passes);
                assert_eq!(fast, reference, "{w}x{h}, {passes} passes");
            }
        }
    }

    /// Another radius still takes the reference path, unchanged.
    #[test]
    fn other_radii_take_the_reference_path() {
        let src = plane(50, 30, 7);
        let mut a = src.clone();
        let mut b = src.clone();
        blur_plane(&mut a, 50, 30, 3, 2);
        blur_plane_generic(&mut b, 50, 30, 3, 2);
        assert_eq!(a, b);
    }
}
