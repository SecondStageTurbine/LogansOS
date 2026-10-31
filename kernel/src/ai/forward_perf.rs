// SPDX-License-Identifier: MPL-2.0
//! Arthur perf lane (OT-7 wall-clock) — an honest TSC-based measurement of the
//! real forward pass, replacing the Milestone-A `ms=0`.
//!
//! Milestone A shipped correct on silicon but its internal timing reported
//! `ms=0` because the PIT tick (100 Hz ⇒ 10 ms granularity) is far coarser than
//! a single forward. This module wires the OT-7 `lfence;rdtsc` idiom into the
//! **two disjoint real forward entry points**, so every forward is timed at
//! cycle granularity, and calibrates a TSC→time conversion against the PIT so
//! cycles become milliseconds:
//!
//! 1. [`super::exec::run`] — the graph forward, which the `sys_infer_run`
//!    syscall path and every graph-backed `generate`/`next_logits` caller
//!    funnel through.
//! 2. [`super::qwen_inventory::InventoryDecodeState::advance`] — the
//!    inventory-backed per-token forward behind `sys_generate` (persistent-KV
//!    and reprefill modes alike). Inventory models carry no graph nodes, so
//!    this path never enters `exec::run`; without its own bracket the real
//!    Arthur generation path would be entirely untimed.
//!
//! The two are mutually exclusive per forward — a model is graph-backed or
//! inventory-backed — so the accumulators cannot double-count a single pass.
//!
//! ## Honesty (the OT-7 caveat — load-bearing)
//! Under **QEMU/TCG the TSC is virtualized** — the cycle counts are NOT real
//! silicon magnitudes. The QEMU number proves the **wiring** (nonzero, plausible,
//! scales with `thread_count`); the **real tokens/sec is the bare-metal read**
//! (the next Arthur OptiPlex session). The `SHOWCASE perf …` breadcrumb is
//! labeled accordingly.
//!
//! ## Correctness-neutral (kill-switch clean)
//! `read_tsc` is a pure register read with no memory effect, so instrumenting the
//! forward does **not** change the logits/token-ids — the fast-on ≡ scalar
//! bit-for-bit kill-switch is unaffected by measurement. Both the module
//! (`ai/mod.rs`) and every call site are `#[cfg(feature = "forward-perf")]`, so
//! builds without it are byte-identical (no `rdtsc`), and the showcase pulls it
//! in. Each timed entry point keeps the error path exactly as it was: a forward
//! that fails is propagated unchanged and never recorded as a pass.

use core::sync::atomic::{AtomicU64, Ordering};

/// Calibrated TSC frequency (ticks/second); 0 until [`calibrate_tsc_hz`] runs.
static TSC_HZ: AtomicU64 = AtomicU64::new(0);
/// Total TSC cycles spent across all recorded forward passes.
static FORWARD_CYCLES: AtomicU64 = AtomicU64::new(0);
/// Number of forward passes recorded (one per generated token's logits).
static FORWARD_PASSES: AtomicU64 = AtomicU64::new(0);

/// The OT-7 latency-oracle idiom: `lfence; rdtsc` reads only the time-stamp
/// counter into EDX:EAX; it touches no memory and no stack. Serialized by the
/// leading `lfence` so it cannot float over the forward it brackets.
#[inline]
pub fn read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: reads TSC into EDX:EAX only; no memory/stack effect (nostack).
    unsafe {
        core::arch::asm!(
            "lfence",
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nostack, preserves_flags)
        );
    }
    ((hi as u64) << 32) | lo as u64
}

/// Record one forward pass's cycle cost.
///
/// Called around both graph execution ([`super::exec::run`]) and the
/// inventory per-token forward
/// ([`super::qwen_inventory::InventoryDecodeState::advance`]). Those paths are
/// disjoint, so a pass is counted exactly once. Only a forward that produced
/// logits is recorded — a failed forward is propagated, never counted.
#[inline]
pub fn record_forward(cycles: u64) {
    FORWARD_CYCLES.fetch_add(cycles, Ordering::Relaxed);
    FORWARD_PASSES.fetch_add(1, Ordering::Relaxed);
}

/// `(total_forward_cycles, forward_passes)` — the accumulated measurement.
pub fn report() -> (u64, u64) {
    (
        FORWARD_CYCLES.load(Ordering::Relaxed),
        FORWARD_PASSES.load(Ordering::Relaxed),
    )
}

/// Calibrated TSC frequency in Hz (0 if uncalibrated / calibration bailed).
pub fn tsc_hz() -> u64 {
    TSC_HZ.load(Ordering::Relaxed)
}

/// Per-forward-pass milliseconds, derived from the accumulators + [`tsc_hz`].
/// 0 when uncalibrated or nothing measured (the report labels that state).
pub fn per_forward_ms() -> u64 {
    let (cycles, passes) = report();
    let hz = tsc_hz();
    if passes == 0 || hz == 0 {
        return 0;
    }
    // (cycles / passes) cycles ÷ hz cycles/s × 1000 ms/s.
    (cycles / passes).saturating_mul(1000) / hz
}

/// Forward passes per second (compute throughput). 0 when uncalibrated/empty.
pub fn passes_per_sec() -> u64 {
    let (cycles, passes) = report();
    let hz = tsc_hz();
    if cycles == 0 || hz == 0 {
        return 0;
    }
    passes.saturating_mul(hz) / cycles
}

/// Calibrate TSC-Hz against the 100 Hz PIT (`arch::x86_64::timer::TICKS`).
///
/// Requires interrupts enabled + the PIT ticking (call from a scenario/boot
/// point, never inside the IRQ-off forward). Idempotent — the first successful
/// non-zero calibration wins. Bails (leaving `tsc_hz()==0`) if the PIT is not
/// advancing, so a stuck timer degrades to an honest "uncalibrated" report
/// rather than a hang.
pub fn calibrate_tsc_hz() {
    if TSC_HZ.load(Ordering::Relaxed) != 0 {
        return;
    }
    const PIT_HZ: u64 = 100; // arch::x86_64::timer TICKS is 100 Hz nominal.
    const CAL_TICKS: u64 = 20; // ~200 ms calibration window.
    const SPIN_CAP: u64 = 2_000_000_000; // bail rather than hang if PIT is dead.

    let ticks = || crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);

    // Align to a tick edge so the window is a whole number of ticks.
    let edge = ticks();
    let mut spins: u64 = 0;
    while ticks() == edge {
        core::hint::spin_loop();
        spins += 1;
        if spins > SPIN_CAP {
            return;
        }
    }

    let base = ticks();
    let t0 = read_tsc();
    let target = base.saturating_add(CAL_TICKS);
    spins = 0;
    while ticks() < target {
        core::hint::spin_loop();
        spins += 1;
        if spins > SPIN_CAP.saturating_mul(CAL_TICKS) {
            return;
        }
    }
    let t1 = read_tsc();
    let elapsed_ticks = ticks().saturating_sub(base);
    if elapsed_ticks == 0 {
        return;
    }
    // tsc_hz = tsc_delta ÷ (elapsed_ticks / PIT_HZ) = tsc_delta × PIT_HZ ÷ ticks.
    let hz = t1.saturating_sub(t0).saturating_mul(PIT_HZ) / elapsed_ticks;
    TSC_HZ.store(hz, Ordering::Relaxed);
}
