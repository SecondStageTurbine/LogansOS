// SPDX-License-Identifier: MPL-2.0
//! Pure `GpuComputeCap` authority rules, kept kernel-free for the host KAT.
//!
//! OT-63 GPU P1 Slice 2. The sibling of [`super::gpu_submit_cap_logic`]: where
//! `GpuSubmitCap` gates a **hardware present/flip submit** (the scanout path),
//! `GpuComputeCap` gates a **compute-pipeline submit** to the iGPU (the blur
//! kernel dispatch). One iGPU, one manifest, so the authority is binary
//! (held vs not) under the kill-switch — mirrors `gpu_submit_cap_logic` /
//! `net_dial_cap_logic`.
//!
//! ## Amendment 4 — this is DEFINED here, not WIRED in Slices 2–4
//! ADR-077 v2 amendment 4 makes the pre-VT-d compute service **wholly
//! kernel-internal**: a `Handle::GpuCompute` + a service syscall is already a
//! userland compute queue, forbidden before the graphics VT-d fence (Slice 5).
//! So Slices 2–4 gate the compute path with a **feature/runtime kill-switch**
//! (`GPU_COMPUTE_ENABLE`, `super::super::drivers::accel::gpu_compute`), NOT a
//! process capability. This truth table is the **Slice-5 target**: when a
//! userland compute queue is finally granted (after the VT-d narrowing), the
//! runtime `gpu-compute-cap` gate wires to exactly this table, exactly as the
//! present path wires `gpu_submit_cap_logic`. Until then it is pure logic proven
//! by the host KAT, with no runtime caller — the kernel-internal service is
//! gated only by `GPU_COMPUTE_ENABLE`.

/// Kill-switch truth table (Slice-5 target): disabled enforcement is the
/// ambient path (compute submit ungated by a *process* cap — in Slices 2–4 the
/// only caller is the trusted kernel); enabled enforcement (Slice 5+) requires a
/// matching `GpuComputeCap` held. Mirrors [`super::gpu_submit_cap_logic::authority_allows`].
///
/// The paired runtime expression does not exist yet (amendment 4 — no userland
/// compute queue before Slice 5); this table pins the semantics so the Slice-5
/// wiring is a scoping change, not a re-derivation.
pub const fn authority_allows(enforcement_enabled: bool, matching_cap_held: bool) -> bool {
    !enforcement_enabled || matching_cap_held
}
