// SPDX-License-Identifier: MPL-2.0
//! The `prefill-perf` overlay's pointer window (A9-A; Janus, #91 RETURN
//! items 1-2): `ptr in` and `rx` for one generation, counted so that `rx`
//! can never exceed `in`.
//!
//! - The window is ONE published word, `open`: the generation's id while it
//!   runs, 0 otherwise. Opening and closing are each a single store, so no
//!   reader ever sees a half-open window.
//! - `in` counts the records the kernel mints for the WM's pointer route
//!   while the window is open ([`PointerWindow::note_minted`], called under
//!   `DELIVERY` as each record is queued). `rx` counts the WM's dequeues of
//!   records minted in the window ([`PointerWindow::note_rx`]): the window
//!   keeps the first tag it minted, and a dequeued record with an older tag
//!   (minted before the window opened) is not counted. Both are counted at
//!   the kernel's own pointer route, so every dequeue `rx` counts is of a
//!   record `in` counted.
//! - Each count is packed with its window's id (`id << 32 | count`), so an
//!   increment that lands as the next window opens can only restart that
//!   window's count, never carry into it; nothing needs clearing.
//!
//! A WM route reclaim mid-window restarts tags at 1: records from the new
//! route older than the window's first tag are then not counted in `rx`,
//! which can only undercount it. Relaxed atomics: the mint and the dequeue
//! are each taken under `DELIVERY`, which orders them; nothing here takes a
//! lock. Pure, so `tools/ai-ops-tests` drives it on the host.

use core::sync::atomic::{AtomicU64, Ordering};

pub struct PointerWindow {
    /// The open window's id, or 0 while closed.
    open: AtomicU64,
    /// The most recently opened id: the window the overlay reports.
    last: AtomicU64,
    /// `id << 32 | records minted`.
    minted: AtomicU64,
    /// `id << 32 | minted records the WM dequeued`.
    rx: AtomicU64,
    /// `id << 32 | the first tag minted in the window`.
    first: AtomicU64,
}

const COUNT: u64 = 0xffff_ffff;

impl PointerWindow {
    pub const fn new() -> Self {
        Self {
            open: AtomicU64::new(0),
            last: AtomicU64::new(0),
            minted: AtomicU64::new(0),
            rx: AtomicU64::new(0),
            first: AtomicU64::new(0),
        }
    }

    /// Open a new window (a generation begins). Ids run 1..=u32::MAX, then
    /// wrap to 1; 0 means closed.
    pub fn open(&self) {
        let id = match (self.last.load(Ordering::Relaxed) + 1) & COUNT {
            0 => 1,
            id => id,
        };
        self.last.store(id, Ordering::Relaxed);
        self.open.store(id, Ordering::Relaxed);
    }

    /// Close the window (the generation ended). Its counts stay readable
    /// until the next one opens.
    pub fn close(&self) {
        self.open.store(0, Ordering::Relaxed);
    }

    /// The kernel minted the pointer record `tag` for the WM. Tag 0 (the
    /// one-off ROUTE_EXPIRED record) is never consumed and is not counted.
    pub fn note_minted(&self, tag: u16) {
        let id = self.open.load(Ordering::Relaxed);
        if id == 0 || tag == 0 {
            return;
        }
        let _ = self
            .first
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                (v >> 32 != id).then_some(id << 32 | u64::from(tag))
            });
        bump(&self.minted, id);
    }

    /// The WM dequeued the pointer record `tag`.
    pub fn note_rx(&self, tag: u16) {
        let id = self.open.load(Ordering::Relaxed);
        if id == 0 {
            return;
        }
        let first = self.first.load(Ordering::Relaxed);
        if first >> 32 != id || u64::from(tag) < first & COUNT {
            return;
        }
        bump(&self.rx, id);
    }

    /// `(in, rx)` for the last window opened.
    pub fn counts(&self) -> (u64, u64) {
        let id = self.last.load(Ordering::Relaxed);
        (count(&self.minted, id), count(&self.rx, id))
    }
}

fn bump(cell: &AtomicU64, id: u64) {
    let _ = cell.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
        Some(if v >> 32 == id {
            id << 32 | ((v & COUNT) + 1).min(COUNT)
        } else {
            id << 32 | 1
        })
    });
}

fn count(cell: &AtomicU64, id: u64) -> u64 {
    let v = cell.load(Ordering::Relaxed);
    if v >> 32 == id { v & COUNT } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum Step {
        Mint(u16),
        Rx(u16),
    }

    /// Run `steps` with the window opened before step `open_at` and closed
    /// before step `close_at` (each a store boundary of
    /// `begin_generation`/`end_generation`), on a window already used once.
    fn run(steps: &[Step], open_at: usize, close_at: usize) -> (u64, u64) {
        let w = PointerWindow::new();
        // An earlier generation, so stale ids and counts are in the cells.
        w.open();
        w.note_minted(1);
        w.note_rx(1);
        w.close();
        for (i, step) in steps.iter().enumerate() {
            if i == open_at {
                w.open();
            }
            if i == close_at {
                w.close();
            }
            match *step {
                Step::Mint(t) => w.note_minted(t),
                Step::Rx(t) => w.note_rx(t),
            }
        }
        if close_at >= steps.len() {
            w.close();
        }
        w.counts()
    }

    /// Janus's items 1-2: a dequeue at every boundary of the window's
    /// stores. For every placement of the open and close among the mints
    /// and dequeues of a record queued before the window (tag 7) and two
    /// inside it (8, 9), `rx <= in`: `in=0 rx=1` is never read.
    #[test]
    fn rx_never_exceeds_in_at_any_store_boundary() {
        use Step::*;
        let scripts: [&[Step]; 3] = [
            &[Mint(7), Mint(8), Rx(7), Rx(8), Mint(9), Rx(9)],
            &[Mint(7), Rx(7), Mint(8), Mint(9), Rx(8), Rx(9)],
            &[Mint(7), Mint(8), Mint(9), Rx(7), Rx(8), Rx(9)],
        ];
        for steps in scripts {
            for open_at in 0..=steps.len() {
                for close_at in open_at..=steps.len() {
                    let (inn, rx) = run(steps, open_at, close_at);
                    assert!(
                        rx <= inn,
                        "{steps:?} open {open_at} close {close_at}: in {inn} rx {rx}"
                    );
                }
            }
        }
    }

    /// The three diagnoses still read: a record queued before the window and
    /// dequeued inside it is not `rx`; records minted and dequeued inside it
    /// are both `in` and `rx`; minted but not dequeued is `in` only.
    #[test]
    fn counts_only_the_window_s_own_records() {
        use Step::*;
        // Opened after 7 was minted, closed at the end.
        assert_eq!(run(&[Mint(7), Rx(7)], 1, 2), (0, 0));
        assert_eq!(run(&[Mint(7), Mint(8), Rx(7), Rx(8)], 1, 4), (1, 1));
        assert_eq!(run(&[Mint(8), Mint(9)], 0, 2), (2, 0));
        // After the close nothing is counted.
        assert_eq!(run(&[Mint(8), Rx(8)], 0, 1), (1, 0));
    }

    /// A count from an earlier window never carries into the next: a new
    /// window starts at zero without any clearing.
    #[test]
    fn a_new_window_starts_at_zero() {
        let w = PointerWindow::new();
        w.open();
        w.note_minted(3);
        w.note_rx(3);
        assert_eq!(w.counts(), (1, 1));
        w.close();
        assert_eq!(w.counts(), (1, 1), "readable after close");
        w.open();
        assert_eq!(w.counts(), (0, 0));
        // Tag 0 (ROUTE_EXPIRED) is never counted.
        w.note_minted(0);
        w.note_rx(0);
        assert_eq!(w.counts(), (0, 0));
    }
}
