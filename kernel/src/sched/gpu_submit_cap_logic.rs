// SPDX-License-Identifier: MPL-2.0
//! Pure `GpuSubmitCap` authority rules, kept kernel-free for the host KAT.
//!
//! OT-63 Rung 1 Slice 2. `GpuSubmitCap` is the scarce capability that gates
//! **raw scanout present** — either a Software framebuffer blit or a hardware
//! present/flip submission to the iGPU. Renderers hand a surface to a trusted
//! compositor, and only a holder of a `GpuSubmitCap` may drive scanout when the
//! gate is armed. Unlike `NetDialCap` there is no
//! host/port scope — Rung 1 has exactly one iGPU and one present queue, so the
//! authority is binary (held vs not) under the kill-switch.

/// Kill-switch truth table: disabled enforcement is the ambient pre-feature
/// path (submit is ungated, bit-for-bit today's behaviour); enabled enforcement
/// requires a matching `GpuSubmitCap` held on every present backend. Mirrors
/// `net_dial_cap_logic`.
///
/// The paired runtime expression is the scanout gate at `SYS_FB_PRESENT` and
/// `SharedPixelBuffer::present`; edits to either expression must preserve this
/// table.
pub const fn authority_allows(enforcement_enabled: bool, matching_cap_held: bool) -> bool {
    !enforcement_enabled || matching_cap_held
}
