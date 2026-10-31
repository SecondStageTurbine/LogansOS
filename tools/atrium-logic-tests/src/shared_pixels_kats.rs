// SPDX-License-Identifier: MPL-2.0
//! S1-F2 (Codex cross-check): the WM reads a client's shared pixels only
//! through `SharedPixels`, which copies with atomic loads into WM-owned
//! storage and never forms a reference over the client's pages.

use super::atrium_wm::{Rect, SharedPixels};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

fn surface(w: usize, h: usize, f: impl Fn(usize) -> u32) -> Vec<AtomicU32> {
    (0..w * h).map(|i| AtomicU32::new(f(i))).collect()
}

/// The copy lands exactly on the clipped rect of the WM-owned buffer and
/// touches nothing else; a rect partly off the surface is clipped; a
/// too-small destination copies nothing.
#[test]
fn snapshot_copies_the_clipped_rect_only() {
    let (w, h) = (8, 4);
    let src = surface(w, h, |i| 0x100 + i as u32);
    // SAFETY: `src` holds w*h aligned AtomicU32 (u32-sized) for the test.
    let view = unsafe { SharedPixels::new(src.as_ptr() as *const u32, w, h) };
    let mut dst = vec![0xDEAD_u32; w * h];
    let r = view.snapshot(Rect::new(6, 2, 10, 10), &mut dst);
    assert_eq!(r, Rect::new(6, 2, 2, 2));
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let want = if x >= 6 && y >= 2 {
                0x100 + i as u32
            } else {
                0xDEAD
            };
            assert_eq!(dst[i], want, "({x},{y})");
        }
    }
    assert!(view.snapshot(Rect::new(8, 0, 4, 4), &mut dst).is_empty());
    let mut small = vec![0u32; w * h - 1];
    assert!(view.snapshot(Rect::new(0, 0, w, h), &mut small).is_empty());
    assert!(small.iter().all(|&p| p == 0));
}

/// A hostile client rewriting its pages while the WM copies them: every
/// copied pixel is one the writer actually stored (no torn word, no panic),
/// and the WM's copy is stable once taken — later writes never reach it.
#[test]
fn snapshot_under_a_concurrent_writer() {
    const A: u32 = 0x00AA_AAAA;
    const B: u32 = 0x0055_5555;
    let (w, h) = (64, 32);
    let src = Arc::new(surface(w, h, |_| A));
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (src, stop) = (Arc::clone(&src), Arc::clone(&stop));
        std::thread::spawn(move || {
            let mut flip = false;
            while !stop.load(Ordering::Relaxed) {
                let v = if flip { A } else { B };
                for p in src.iter() {
                    p.store(v, Ordering::Relaxed);
                }
                flip = !flip;
            }
        })
    };
    // SAFETY: `src` outlives `view` (joined below) and holds w*h pixels.
    let view = unsafe { SharedPixels::new(src.as_ptr() as *const u32, w, h) };
    let mut dst = vec![0u32; w * h];
    for _ in 0..200 {
        assert_eq!(
            view.snapshot(Rect::new(0, 0, w, h), &mut dst),
            Rect::new(0, 0, w, h)
        );
        assert!(dst.iter().all(|&p| p == A || p == B));
    }
    let taken = dst.clone();
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert_eq!(dst, taken, "the WM's copy never changes under the writer");
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();
}

/// DI-1b resize witness: `load` reads one pixel now, at row-major
/// `y * width + x`, and nothing outside the surface.
#[test]
fn load_reads_one_pixel_and_refuses_outside() {
    let (w, h) = (8, 4);
    let src = surface(w, h, |i| 0x100 + i as u32);
    // SAFETY: `src` holds w*h aligned AtomicU32 (u32-sized) for the test.
    let view = unsafe { SharedPixels::new(src.as_ptr() as *const u32, w, h) };
    assert_eq!(view.load(0, 0), Some(0x100));
    assert_eq!(view.load(7, 3), Some(0x100 + 31));
    assert_eq!(view.load(3, 1), Some(0x100 + 11));
    assert_eq!(view.load(8, 0), None);
    assert_eq!(view.load(0, 4), None);
    src[11].store(0xABC, Ordering::Relaxed);
    assert_eq!(view.load(3, 1), Some(0xABC), "read now, not cached");
}
