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
