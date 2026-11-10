// SPDX-License-Identifier: MPL-2.0
//! OT-tick-1 (`tick_preempt_ring3`, relay 381): a role per spawn, read from
//! the channel at handle 0.
//!
//! - **1, a spinner:** spins in ring 3 forever with no syscall, the shape of
//!   an idle WM or Arthur without `desktop-park`. neo-init kills it.
//! - **2, the compute process:** for [`RUN_NS`], reads the monotonic clock in a
//!   loop and keeps the worst gap between two consecutive reads: how long the
//!   process went without the CPU. It sends the worst gap (ns) and the number
//!   of reads, 16 bytes little-endian, on its channel and exits 0.

use libneo::{CLOCK_MONOTONIC, Handle, sys_clock_gettime, sys_recv, sys_send};

const PARENT: Handle = Handle(0);
/// How long the compute process measures.
const RUN_NS: u64 = 5_000_000_000;

pub fn run() -> i32 {
    let mut role = [0u8; 1];
    if sys_recv(PARENT, &mut role) != 1 {
        return 1;
    }
    match role[0] {
        1 => spin(),
        2 => compute(),
        _ => 2,
    }
}

fn spin() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

fn compute() -> i32 {
    // A kernel without the clock (ENOSYS) would never end the run: say so
    // at once (exit 4) instead.
    let now = || sys_clock_gettime(CLOCK_MONOTONIC).ok();
    let Some(start) = now() else {
        return 4;
    };
    let mut prev = start;
    let mut worst = 0u64;
    let mut reads = 0u64;
    while prev.saturating_sub(start) < RUN_NS {
        let Some(t) = now() else {
            return 4;
        };
        worst = worst.max(t.saturating_sub(prev));
        prev = t;
        reads += 1;
    }
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&worst.to_le_bytes());
    out[8..].copy_from_slice(&reads.to_le_bytes());
    if sys_send(PARENT, &out) != 16 {
        return 3;
    }
    0
}
