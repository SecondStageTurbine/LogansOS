// SPDX-License-Identifier: MPL-2.0
//! Pure watermark hysteresis for the residency daemon (Phase 7 BUILD-P4b).
//!
//! This file has **no kernel dependencies** so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and unit-test the state machine.
//! ANSPEC-001 §9.12 (daemon subsection): the daemon wakes to reclaim when a
//! device's free bytes fall below the **low** watermark and goes back to sleep
//! once free reaches the **high** watermark — the TPP/kswapd hysteresis that
//! prevents single-byte oscillation on the boundary.
//!
//! `low = H_dev` (the device's noted max placement-need — banking less can
//! never prevent the next synchronous reclaim) and `high = min(2·H_dev, C)`.
//! On a tiny arena where `2·H ≥ C` (accel_sim: H ≈ 0.56·C) the band
//! degenerates to "drain every cold managed tensor before sleeping" — the
//! correct behavior at that scale, with oscillation still structurally
//! impossible because prefetch happens only on an explicit hot-publish, never
//! on the daemon's own initiative (ADR-040).

/// The daemon's per-device reclaim state.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum WatermarkState {
    Sleeping,
    Reclaiming,
}

/// The low/high pair for a device with `capacity` bytes and a noted max
/// placement-need of `need_hint` bytes. `need_hint == 0` (nothing placed yet)
/// yields `(0, 0)` — the daemon never reclaims a device no placement has
/// touched.
pub fn watermarks(capacity: u64, need_hint: u64) -> (u64, u64) {
    if need_hint == 0 {
        return (0, 0);
    }
    let low = need_hint.min(capacity);
    let high = need_hint.saturating_mul(2).min(capacity);
    (low, high)
}

/// One hysteresis step: wake below low, sleep at-or-above high, hold
/// otherwise. Free bytes in the band `[low, high)` never flip the state —
/// the anti-oscillation property the host suite asserts byte-by-byte.
pub fn next_state(prev: WatermarkState, free: u64, low: u64, high: u64) -> WatermarkState {
    match prev {
        WatermarkState::Sleeping if free < low => WatermarkState::Reclaiming,
        WatermarkState::Reclaiming if free >= high => WatermarkState::Sleeping,
        unchanged => unchanged,
    }
}

/// Prefetch admissibility (the §9.11 thrash guard as a hard gate): a prefetch
/// of `bytes` is admitted only when the device's free bytes stay at or above
/// the low watermark **after** the transfer — a prefetch must never cause the
/// reclaim it exists to avoid.
pub fn prefetch_admissible(free: u64, bytes: u64, low: u64) -> bool {
    free >= bytes && free - bytes >= low
}

/// The latch step for the per-device need hint: a placement's need can only
/// raise the hint (BUILD-P4b semantics, retained under decay — any live
/// workload immediately restores its band).
pub fn latch(prev_hint: u64, need: u64) -> u64 {
    prev_hint.max(need)
}

/// One decay step (BUILD-C1, the monotonic-latch fix from ADR-040): the hint
/// loses an eighth per demand note, decaying toward recent demand so one
/// outsized placement no longer inflates the watermark band forever. With
/// decay disabled (the Rule-1 kill-switch) this is the identity — the hint
/// behaves as the original latch, bit for bit.
pub fn decay_step(hint: u64, decay_enabled: bool) -> u64 {
    if !decay_enabled {
        return hint;
    }
    hint - hint / 8
}
