// SPDX-License-Identifier: MPL-2.0
//! A9-D: the busy emblem, the LogansOS ring turning slowly round its still
//! star while Arthur thinks.
//!
//! The frames are pre-rendered by `build.rs` from the two layers
//! `tools/branding/generate.py` writes (`assets/busy-emblem-*-88.rgba`):
//! `FRAMES` frames of `SIDE` x `SIDE` straight-alpha `0xAARRGGBB`, frame `k`
//! with the ring turned clockwise by `k * 360 / FRAMES` degrees. The WM blends
//! one per [`FRAME_NS`] on its own clock, so the emblem keeps turning while
//! the client is blocked.

use crate::Rect;

include!(concat!(env!("OUT_DIR"), "/busy_emblem_frames.rs"));

/// 12 frames a second: one turn every `FRAMES / 12` = 3 s.
pub const FRAME_NS: u64 = 1_000_000_000 / 12;

/// Frame `index` (taken modulo [`FRAMES`]), row-major, stride [`SIDE`].
pub fn frame(index: usize) -> &'static [u32] {
    let start = (index % FRAMES) * SIDE * SIDE;
    &PIXELS[start..start + SIDE * SIDE]
}

/// The frame to show `elapsed_ns` after the emblem appeared.
pub const fn frame_at(elapsed_ns: u64) -> usize {
    ((elapsed_ns / FRAME_NS) % FRAMES as u64) as usize
}

/// Where the emblem goes when the client reserves `area`: centred in it, then
/// clipped to `area` and to `clip`. Returns the destination and the offset of
/// its top-left pixel inside a frame, or `None` when nothing is visible.
pub fn place(area: Rect, clip: Rect) -> Option<(Rect, usize)> {
    // Centre with signed arithmetic: an area smaller than the emblem crops
    // it evenly on both sides.
    let left = area.x as i64 + (area.w as i64 - SIDE as i64) / 2;
    let top = area.y as i64 + (area.h as i64 - SIDE as i64) / 2;
    let x0 = left.max(0) as usize;
    let y0 = top.max(0) as usize;
    let whole = Rect::new(
        x0,
        y0,
        (left + SIDE as i64 - x0 as i64).max(0) as usize,
        (top + SIDE as i64 - y0 as i64).max(0) as usize,
    );
    let dest = whole.intersect(&area).intersect(&clip);
    if dest.is_empty() {
        return None;
    }
    let sx = (dest.x as i64 - left) as usize;
    let sy = (dest.y as i64 - top) as usize;
    Some((dest, sy * SIDE + sx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn alpha(p: u32) -> f64 {
        f64::from(p >> 24)
    }

    /// Offset from the frame centre, in pixels.
    fn offset(i: usize) -> (f64, f64) {
        let c = SIDE as f64 / 2.0;
        ((i % SIDE) as f64 + 0.5 - c, (i / SIDE) as f64 + 0.5 - c)
    }

    #[test]
    fn frames_have_the_promised_shape() {
        assert_eq!(PIXELS.len(), FRAMES * SIDE * SIDE);
        assert!((24..=36).contains(&FRAMES), "24-36 angles");
        assert!((40..=48).contains(&SIDE), "40-48 px");
        for k in 0..FRAMES {
            let f = frame(k);
            // Opaque somewhere (the star's core), transparent in the corners.
            assert!(
                f.iter().any(|&p| p >> 24 == 0xFF),
                "frame {k} has no opaque pixel"
            );
            for corner in [0, SIDE - 1, SIDE * (SIDE - 1), SIDE * SIDE - 1] {
                assert_eq!(f[corner] >> 24, 0, "frame {k} corner {corner}");
            }
            // A transparent pixel carries no colour.
            assert!(f.iter().all(|&p| p >> 24 != 0 || p == 0));
        }
        assert_eq!(frame(FRAMES), frame(0));
    }

    /// The star does not turn: the centre (well inside the ring) is the same
    /// in every frame.
    #[test]
    fn the_star_stays_still() {
        let still = |i: usize| {
            let (dx, dy) = offset(i);
            (dx * dx + dy * dy).sqrt() < 0.2 * SIDE as f64
        };
        let first = frame(0);
        assert!(
            (0..SIDE * SIDE)
                .filter(|&i| still(i))
                .any(|i| first[i] >> 24 == 0xFF)
        );
        for k in 1..FRAMES {
            let f = frame(k);
            for i in (0..SIDE * SIDE).filter(|&i| still(i)) {
                assert_eq!(f[i], first[i], "frame {k} pixel {i}");
            }
        }
    }

    /// The ring turns clockwise by 360/FRAMES degrees a frame. Independent of
    /// how build.rs rotates: the ring band's alpha, histogrammed by screen
    /// angle (y down, so a growing angle is clockwise), must match frame 0's
    /// histogram best at exactly the frame's turn.
    #[test]
    fn the_ring_turns_clockwise_one_step_a_frame() {
        const BINS: usize = FRAMES * 2;
        let histogram = |f: &[u32]| {
            let mut h = [0.0f64; BINS];
            for (i, &p) in f.iter().enumerate() {
                let (dx, dy) = offset(i);
                if (dx * dx + dy * dy).sqrt() < 0.3 * SIDE as f64 {
                    continue;
                }
                let turn = (dy.atan2(dx) + 2.0 * PI) % (2.0 * PI);
                h[((turn / (2.0 * PI) * BINS as f64) as usize).min(BINS - 1)] += alpha(p);
            }
            h
        };
        let base = histogram(frame(0));
        for k in 1..FRAMES {
            let h = histogram(frame(k));
            let error = |shift: usize| {
                (0..BINS)
                    .map(|b| (h[(b + shift) % BINS] - base[b]).powi(2))
                    .sum::<f64>()
            };
            let best = (0..BINS)
                .min_by(|&a, &b| error(a).total_cmp(&error(b)))
                .unwrap();
            assert_eq!(best, 2 * k % BINS, "frame {k}");
        }
    }

    #[test]
    fn frame_at_runs_at_twelve_a_second_and_wraps() {
        assert_eq!(frame_at(0), 0);
        assert_eq!(frame_at(FRAME_NS - 1), 0);
        assert_eq!(frame_at(FRAME_NS), 1);
        assert_eq!(frame_at(FRAME_NS * FRAMES as u64), 0);
        assert_eq!(frame_at(FRAME_NS * (FRAMES as u64 + 5)), 5);
        // One turn takes 3 s (to within the per-frame rounding).
        assert_eq!(FRAME_NS * FRAMES as u64, 2_999_999_988);
    }

    #[test]
    fn place_centres_in_the_area_and_clips() {
        let big = Rect::new(0, 0, 1000, 1000);
        // Room to spare: centred, whole.
        assert_eq!(
            place(Rect::new(100, 200, SIDE + 10, SIDE + 20), big),
            Some((Rect::new(105, 210, SIDE, SIDE), 0))
        );
        // The clip cuts the left 4 columns and top 3 rows.
        assert_eq!(
            place(
                Rect::new(100, 200, SIDE, SIDE),
                Rect::new(104, 203, 1000, 1000)
            ),
            Some((Rect::new(104, 203, SIDE - 4, SIDE - 3), 3 * SIDE + 4))
        );
        // An area shorter than the emblem crops it evenly, top and bottom.
        assert_eq!(
            place(Rect::new(10, 10, SIDE, SIDE - 10), big),
            Some((Rect::new(10, 10, SIDE, SIDE - 10), 5 * SIDE))
        );
        // Near the origin the centring would go negative.
        assert_eq!(
            place(Rect::new(0, 0, 4, 4), big),
            Some((Rect::new(0, 0, 4, 4), (SIDE / 2 - 2) * SIDE + SIDE / 2 - 2))
        );
        assert_eq!(
            place(Rect::new(10, 10, SIDE, SIDE), Rect::new(500, 500, 10, 10)),
            None
        );
        assert_eq!(place(Rect::new(10, 10, 0, 0), big), None);
    }
}
