// SPDX-License-Identifier: MPL-2.0
//! F7 (drag smoothness): `Rect::minus` against a pixel model. A moved frame
//! repaints the new reach, then the old reach minus it; the pieces must
//! cover exactly the old pixels outside the new rect, each once.

use super::atrium_wm::Rect;

fn pixels(r: &Rect) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            v.push((x, y));
        }
    }
    v
}

fn check(a: Rect, b: Rect) {
    let pieces = a.minus(&b);
    let mut got: Vec<(usize, usize)> = pieces.iter().flat_map(pixels).collect();
    let n = got.len();
    got.sort_unstable();
    got.dedup();
    assert_eq!(got.len(), n, "{a:?} - {b:?}: a pixel covered twice");
    let mut want: Vec<(usize, usize)> = pixels(&a)
        .into_iter()
        .filter(|&(x, y)| !b.contains(x, y))
        .collect();
    want.sort_unstable();
    assert_eq!(got, want, "{a:?} - {b:?}");
    for p in pieces {
        assert!(p.is_empty() || a.covers(&p), "{a:?} - {b:?}: {p:?} outside");
        if p.is_empty() {
            assert_eq!(p, Rect::default());
        }
    }
}

#[test]
fn minus_covers_exactly_the_rest_once() {
    let a = Rect::new(10, 20, 30, 25);
    // Every placement of a 12x9 rect around and over `a`, plus a few sizes.
    for (w, h) in [(12, 9), (30, 25), (50, 60), (1, 1)] {
        for y in 0..60 {
            for x in 0..60 {
                check(a, Rect::new(x, y, w, h));
            }
        }
    }
}

#[test]
fn minus_hand_laid_examples() {
    let a = Rect::new(0, 0, 10, 10);
    // Disjoint: `a` whole.
    assert_eq!(
        a.minus(&Rect::new(20, 20, 5, 5)),
        [a, Rect::default(), Rect::default(), Rect::default()]
    );
    // Covered: nothing left.
    assert_eq!(a.minus(&Rect::new(0, 0, 10, 10)), [Rect::default(); 4]);
    // A frame moved by (+3, +2): the top band and the left strip are left.
    assert_eq!(
        a.minus(&Rect::new(3, 2, 10, 10)),
        [
            Rect::new(0, 0, 10, 2),
            Rect::default(),
            Rect::new(0, 2, 3, 8),
            Rect::default(),
        ]
    );
    // A hole in the middle: four bands.
    assert_eq!(
        a.minus(&Rect::new(4, 4, 2, 2)),
        [
            Rect::new(0, 0, 10, 4),
            Rect::new(0, 6, 10, 4),
            Rect::new(0, 4, 4, 2),
            Rect::new(6, 4, 4, 2),
        ]
    );
    // An empty subtrahend takes nothing.
    assert_eq!(a.minus(&Rect::default())[0], a);
}
