// SPDX-License-Identifier: MPL-2.0
//! The freeze watchdog's startup timeline line (feature `freeze-watchdog`):
//! when the input path and Arthur reached each milestone, in timer ticks
//! since boot, so one photo of the overlay shows where a "typing shows
//! nothing" window was spent.
//!
//! Pure: host-tested by `tools/fb-progress-logic-tests` through `#[path]`.

use core::fmt::{self, Write};

/// One snapshot of the timeline. A tick of 0 means "not yet".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    /// The timer tick the line is drawn at.
    pub now: u64,
    /// Keyboard-route activations (the WM's Home route first, then each
    /// handoff), and the ticks of the first and the latest.
    pub routes: u64,
    pub route_first: u64,
    pub route_last: u64,
    /// Key presses any keyboard produced, and the first one's tick.
    pub keys: u64,
    pub key_first: u64,
    /// Key presses dropped because no route was active (or the route
    /// changed under them).
    pub keys_dropped: u64,
    /// The first key press a client (the WM, or Arthur) took.
    pub rx_first: u64,
    /// Arthur's latest spawn.
    pub arthur: u64,
    /// Arthur's vocab: its open, and its close (the read done). From there
    /// to `model` is the parse (the model is taken right after it).
    pub vocab_open: u64,
    pub vocab_closed: u64,
    /// The latest model handle a process got.
    pub model: u64,
}

/// The tick a "first" milestone stores: tick 0 is reserved for "not yet".
pub const fn mark(now: u64) -> u64 {
    if now == 0 { 1 } else { now }
}

struct At(u64);

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            f.write_str("-")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// `start tk now=N | route#C first=F last=L | key#K first=F drop=D rx=R |
/// arthur=A vocab=O..C model=M`, every tick a timer tick since boot
/// (`-` = not yet).
pub fn write_line(out: &mut impl Write, t: &Timeline) -> fmt::Result {
    write!(
        out,
        "start tk now={} | route#{} first={} last={} | key#{} first={} drop={} rx={} | arthur={} vocab={}..{} model={}",
        t.now,
        t.routes,
        At(t.route_first),
        At(t.route_last),
        t.keys,
        At(t.key_first),
        t.keys_dropped,
        At(t.rx_first),
        At(t.arthur),
        At(t.vocab_open),
        At(t.vocab_closed),
        At(t.model),
    )
}
