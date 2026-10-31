// SPDX-License-Identifier: MPL-2.0
//! OT-34 F1: the pure barrier-decision policy, extracted host-testable.
//!
//! The BSP's fork-join join loop, on each spin, observes `(done, pending)` for an
//! AP slot plus whether it has passed the grace and hard-limit thresholds, and
//! must decide what to do. The load-bearing soundness rule (F1, the AAG bounce):
//! the hard limit may only SKIP/steal an **unclaimed** slot (`pending == true`, an
//! AP never started). A **claimed** slot (`pending == false`) that is not yet
//! `done` past the hard limit is an executor wedged mid pure-compute — an
//! impossible ring-0 state — and the BSP must **HALT** (fail-stop), NEVER return.
//! Returning would end `run_generic`, drop the `&mut out` borrow, and let the
//! caller free/reuse `out` while the still-live executor is about to write it
//! (write-after-free), plus a late `done` store could trip a later job's barrier.
//!
//! Encoding the decision as a pure function lets a host KAT prove
//! "claimed + unfinished + past-limit ⇒ Halt, never a skip" without a real AP.

/// The BSP's join action for one AP slot, derived from the observed slot state.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BarrierAction {
    /// The partition is retired (`done`); stop waiting on this slot.
    Done,
    /// The slot is unclaimed (`pending == true`) and past the grace window — the
    /// BSP may steal it (claim + compute it inline). Safe at any spin count: an
    /// unclaimed slot has no live executor, so nothing writes `out` behind the BSP.
    Steal,
    /// A CLAIMED slot (`pending == false`) is still not `done` past the hard limit:
    /// an executor wedged mid pure-compute. FAIL-STOP — the BSP must halt, never
    /// return (returning frees `out` under the live executor → UAF).
    Halt,
    /// Keep spinning (waiting on a claimed executor, or before the grace window).
    Spin,
}

/// Decide the BSP's join action for one slot from the observed `(done, pending)`
/// and the two spin thresholds.
///
/// The F1 invariant this encodes: there is NO input for which a claimed-but-
/// unfinished slot (`!done && !pending`) yields [`BarrierAction::Steal`] or any
/// action the caller maps to a return-that-frees-`out`. A claimed slot yields only
/// [`BarrierAction::Spin`] (keep waiting for its executor) or, past the hard limit,
/// [`BarrierAction::Halt`] (fail-stop). Stealing is reserved for the unclaimed
/// case, where no executor is live.
pub const fn barrier_action(
    done: bool,
    pending: bool,
    past_grace: bool,
    past_limit: bool,
) -> BarrierAction {
    if done {
        return BarrierAction::Done;
    }
    if pending {
        // Unclaimed: no live executor yet. Stealable once past the grace window.
        // (Past the hard limit while still unclaimed cannot happen in practice —
        // the BSP would have stolen it first — but stealing stays the safe action.)
        if past_grace {
            return BarrierAction::Steal;
        }
        return BarrierAction::Spin;
    }
    // Claimed (an executor won the claim CAS) but not `done`: only that executor
    // can retire the slot. Past the hard limit ⇒ wedged ⇒ fail-stop; else wait.
    if past_limit {
        return BarrierAction::Halt;
    }
    BarrierAction::Spin
}

/// Whether an unclaimed slot is past its steal grace. With a calibrated TSC
/// (`tsc_hz != 0`) the grace is time since the job was published,
/// `grace_us`; otherwise it is the spin count, as before. A spin's length
/// varies by an order of magnitude across CPUs (`PAUSE` is ~10 cycles on some
/// cores and ~140 on others), so a spin-count grace sized for one is far too
/// long on another, and every job then waits it out on a busy AP (Arthur
/// latency round 2). This only decides when an UNCLAIMED slot may be stolen;
/// a claimed slot never reaches it (see [`barrier_action`]).
pub const fn past_steal_grace(
    elapsed_cycles: u64,
    tsc_hz: u64,
    spins: u64,
    grace_us: u64,
    grace_spins: u64,
) -> bool {
    if tsc_hz == 0 {
        return spins >= grace_spins;
    }
    elapsed_cycles >= steal_grace_cycles(tsc_hz, grace_us)
}

/// `grace_us` in TSC cycles at `tsc_hz`, rounded UP and never 0 for a
/// non-zero grace (Chad, relay #52 item 3): multiplied before dividing, in
/// u128, so a rate below 1 MHz (leaf 15 can report 500 kHz) or a
/// non-integral MHz rate keeps the whole grace instead of truncating it to
/// nothing, and saturated to `u64::MAX` past the u64 range.
pub const fn steal_grace_cycles(tsc_hz: u64, grace_us: u64) -> u64 {
    let product = (tsc_hz as u128) * (grace_us as u128);
    let cycles = product.div_ceil(1_000_000);
    if cycles > u64::MAX as u128 {
        u64::MAX
    } else {
        cycles as u64
    }
}
