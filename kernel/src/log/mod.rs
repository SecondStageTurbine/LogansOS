// SPDX-License-Identifier: MPL-2.0
//! Kernel log fan-out.
//!
//! Serial remains the primary kernel log path. Optional sinks hang off this
//! module so existing `serial_println!` call sites do not need to know about
//! scenario-only capture destinations.

use core::fmt;

pub mod block_sink;

pub fn write_fmt(args: fmt::Arguments<'_>) {
    block_sink::write_fmt(args);
}

pub fn flush() {
    block_sink::flush();
}

/// OT-90: the clean shutdown is done with the block sink (K3 flushed it, or
/// the deadline passed); no later print or flush touches it
/// (`block_sink::close`).
#[cfg(feature = "power-clean")]
pub fn close_sink() {
    block_sink::close();
}

/// Stage-only section on this CPU (OT-83 F1, CP-71; see `block_sink`).
pub fn enter_stage_only() -> bool {
    block_sink::enter_stage_only()
}

/// Leave the section entered by [`enter_stage_only`].
pub fn leave_stage_only(was_stage_only: bool) {
    block_sink::leave_stage_only(was_stage_only);
}

/// Normal context: write out what a stage-only section left staged; never
/// waits.
pub fn flush_pending() {
    block_sink::flush_pending();
}

/// Panic path: stage only, never block or flush (`block_sink`).
#[cfg(feature = "fb-progress-console")]
pub fn write_fmt_panicking(args: fmt::Arguments<'_>) {
    block_sink::write_fmt_panicking(args);
}

/// Panic path: one bounded attempt to write the staged lines out.
#[cfg(feature = "fb-progress-console")]
pub fn panic_flush() {
    block_sink::panic_flush();
}
