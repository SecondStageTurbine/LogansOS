// SPDX-License-Identifier: MPL-2.0
//! OT-83: shared helpers for the ring-3 fault probes.
//!
//! Each `fault-probe-<vector>` binary prints a line, raises exactly one CPU
//! exception in ring 3, and — only if the exception did NOT kill it — prints a
//! FAIL line and exits [`SURVIVED`]. The kernel scenario expects the process to
//! die with 128 + the vector's signal instead, then runs `fault-probe-ok` to
//! show the system is still up and a second process runs.

#![no_std]

use libneo::{sys_exit, sys_write};

/// Exit code a probe uses if its exception returned control to it.
pub const SURVIVED: i32 = 99;

/// Write `msg` to the console.
pub fn say(msg: &[u8]) {
    let _ = sys_write(1, msg.as_ptr(), msg.len());
}

/// Called after the faulting instruction: reaching it is the failure.
pub fn survived(tag: &[u8]) -> ! {
    say(tag);
    say(b" FAIL: the exception returned to ring 3\n");
    sys_exit(SURVIVED);
}

/// The probes' panic handler body.
pub fn panic_exit() -> ! {
    sys_exit(255);
}
