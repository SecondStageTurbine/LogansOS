// SPDX-License-Identifier: MPL-2.0
//! Pure admission-control policy for the inference scheduler (Phase 7 BUILD-2).
//!
//! This file has **no kernel dependencies** (it is `#![no_std]`-clean and uses no
//! `alloc`) so the host suite (`tools/ai-ops-tests`) can path-`mod` it and
//! unit-test the priority / quota / reservation decision logic. The kernel glue —
//! the per-device queues, the decision trace, the in-flight-principal wiring, and
//! reading the §9.5 accounting table for quota — lives in `admission.rs`.
//!
//! The resource shape (ANSPEC-001 §9.6) is **admission control, not preemption**:
//! accelerators run to completion (one request per device at a time — temporal
//! isolation); the policy decides *who runs next*, never interrupting a running
//! dispatch. So this is two pure decisions: (1) when a request arrives and the
//! device is busy/reserved by another principal, defer it; (2) when the device
//! frees, choose the next pending request by `priority → under-quota → FIFO`.

/// A pending inference request — the schedulable unit (THINK Q1). Principals are
/// `u32` (a `ProcessId`) here to keep this file kernel-free.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Request {
    pub principal: u32,
    /// Higher admits first.
    pub priority: u32,
    /// Monotonic submission order — FIFO tiebreak (earlier wins).
    pub submit_seq: u64,
    /// This request holds a device reservation while it runs, deferring others.
    pub reserved: bool,
}

/// The admission decision for a newly-arriving request.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Decision {
    Admit,
    Defer(DeferReason),
}

/// Why a request was deferred — the cross-tenant clause: *deferred because of
/// another principal's claim*. Carries the other principal so the trace can name it.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DeferReason {
    /// Another principal holds an active device reservation.
    Reservation { holder: u32 },
    /// The device is busy running another principal (run-to-completion).
    DeviceBusy { running: u32 },
    /// This principal is over quota; an under-quota principal is preferred.
    #[allow(dead_code)] // host-suite variant — §9.6 truth table
    OverQuota { admitted: u32 },
    /// A higher-priority principal's request is preferred.
    #[allow(dead_code)] // host-suite variant — §9.6 truth table
    LowerPriority { admitted: u32 },
}

/// The device admission state the arrival policy reasons over.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct DeviceState {
    /// Principal of the request currently running on the device, if any.
    pub running: Option<u32>,
    /// Principal holding the device reservation, if any.
    pub reservation: Option<u32>,
}

/// Decide whether an arriving request is admitted now or deferred. Under
/// run-to-completion with temporal isolation, a reservation or a running request
/// **by another principal** defers the arrival; a principal's own reservation or
/// running request never blocks its own. Reservation outranks plain device-busy
/// (it is the explicit cross-tenant claim).
pub fn admit_arrival(req: &Request, state: &DeviceState) -> Decision {
    if let Some(holder) = state.reservation
        && holder != req.principal
    {
        return Decision::Defer(DeferReason::Reservation { holder });
    }
    if let Some(running) = state.running
        && running != req.principal
    {
        return Decision::Defer(DeferReason::DeviceBusy { running });
    }
    Decision::Admit
}

/// Choose the index of the next request to admit from `pending` (the device is now
/// free, no blocking reservation). Policy order, best first:
///   1. **highest `priority`**,
///   2. then **under quota before over quota** (`over_quota(principal)` reads the
///      §9.5 accounting snapshot — no parallel bookkeeping),
///   3. then **earliest `submit_seq`** (FIFO).
///
/// Returns `None` only for an empty queue.
pub fn choose_next<F: Fn(u32) -> bool>(pending: &[Request], over_quota: F) -> Option<usize> {
    pending
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.priority
                .cmp(&b.priority)
                // under-quota (over == false) ranks higher: if b is over and a is under,
                // `true.cmp(&false)` == Greater ⇒ a wins.
                .then_with(|| over_quota(b.principal).cmp(&over_quota(a.principal)))
                // earlier submit_seq ranks higher: if a.seq < b.seq, `b.cmp(a)` == Greater ⇒ a wins.
                .then_with(|| b.submit_seq.cmp(&a.submit_seq))
        })
        .map(|(index, _)| index)
}

/// The reason `loser` was deferred in favour of `winner` when the device freed and
/// `choose_next` picked `winner` — for the `ADMITTED`/`DEFERRED` trace pairing.
/// Quota outranks priority as an explanation (it is the accounting-grounded claim).
#[allow(dead_code)] // host-suite API — §9.6 admission truth table (path-mod consumers)
pub fn losing_reason<F: Fn(u32) -> bool>(
    winner: &Request,
    loser: &Request,
    over_quota: F,
) -> DeferReason {
    if over_quota(loser.principal) && !over_quota(winner.principal) {
        DeferReason::OverQuota {
            admitted: winner.principal,
        }
    } else {
        DeferReason::LowerPriority {
            admitted: winner.principal,
        }
    }
}
