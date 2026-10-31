// SPDX-License-Identifier: MPL-2.0
//! OT-22: the `SYS_WAIT_ANY` item, as user space lays it out. Host-tested
//! from `tools/atrium-logic-tests` against hand-laid bytes and against the
//! kernel's decoder (`kernel/src/sched/wait_any_logic.rs`).

/// At most this many items per wait.
pub const WAIT_ITEMS_MAX: usize = 8;
/// A channel the caller could `sys_recv`/`sys_try_recv` without parking.
pub const WAIT_KIND_CHANNEL: u32 = 1;
/// An input focus or pointer-route leaf with an event queued (tick-paced).
pub const WAIT_KIND_INPUT: u32 = 2;
/// A pending SAK chord (input-system authority; the handle is ignored).
pub const WAIT_KIND_SAK: u32 = 3;

/// One `sys_wait_any` item: 8 bytes, `handle` then `kind`, little-endian.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WaitItem {
    pub handle: u32,
    pub kind: u32,
}
