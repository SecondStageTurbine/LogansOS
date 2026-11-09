// SPDX-License-Identifier: MPL-2.0
//! PR #58 re-check: `Rect::uncovered_px` (the drag frames' over-present
//! count) against a pixel model, and on the two damages a drag frame can
//! present: the fix's strips (nothing outside) and the control's box.

use super::atrium_wm::Rect;

fn model(r: Rect, cover: &[Rect]) -> u64 {
    let mut n = 0;
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            n += u64::from(!cover.iter().any(|c| c.contains(x, y)));
        }
    }
    n
}

/// A 2-px outline's four edges (as `pointer::outline_edges`).
fn edges(f: Rect) -> [Rect; 4] {
    [
        Rect::new(f.x, f.y, f.w, 2),
        Rect::new(f.x, f.y + f.h - 2, f.w, 2),
        Rect::new(f.x, f.y, 2, f.h),
        Rect::new(f.x + f.w - 2, f.y, 2, f.h),
    ]
}

#[test]
fn uncovered_matches_the_pixel_model() {
    let cover = [
        Rect::new(2, 3, 5, 4),
        Rect::new(4, 1, 6, 3),
        Rect::new(0, 8, 3, 2),
        Rect::new(6, 5, 1, 7),
        Rect::default(),
    ];
    for r in [
        Rect::new(0, 0, 12, 12),
        Rect::new(3, 2, 4, 4),
        Rect::new(9, 9, 3, 3),
        Rect::new(5, 5, 1, 1),
        Rect::default(),
    ] {
        assert_eq!(r.uncovered_px(&cover), model(r, &cover), "{r:?}");
    }
    assert_eq!(Rect::new(1, 1, 4, 4).uncovered_px(&[]), 16);
}

#[test]
fn a_drag_frame_presents_nothing_outside_its_strips_but_the_box_does() {
    let (old, new) = (Rect::new(100, 100, 300, 200), Rect::new(130, 115, 300, 200));
    let sprite = Rect::new(250, 110, 12, 18);
    let mut needed = vec![sprite];
    needed.extend(edges(old));
    needed.extend(edges(new));
    // The fix presents each strip and the sprite as itself.
    for r in &needed {
        assert_eq!(r.uncovered_px(&needed), 0, "{r:?}");
    }
    // The control presents the box around them all.
    let boxed = sprite.union(&old).union(&new);
    let outside = boxed.uncovered_px(&needed);
    assert_eq!(outside, model(boxed, &needed));
    assert!(outside > 50_000, "{outside}");
}
