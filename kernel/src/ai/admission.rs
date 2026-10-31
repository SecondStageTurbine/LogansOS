// SPDX-License-Identifier: MPL-2.0
//! Phase 7 BUILD-2: the admission-control inference scheduler — the first
//! enforceable **cross-tenant decision**.
//!
//! When two mutually-untrusting principals contend for one accelerator, the kernel
//! admits one and **defers** the other, by policy the applications did not request.
//! That deferral is structurally impossible above the kernel: a per-process
//! userspace runtime sees only its own work and can never legitimately defer
//! *another* principal. This is sovereignty of decision-making, minimal first
//! instance.
//!
//! Resource shape (ANSPEC-001 §9.6): **admission control, not preemption**.
//! Accelerators run to completion — one request per device at a time (temporal
//! isolation). The scheduler decides *who runs next* when a device is busy or a
//! principal is over quota; it **never** interrupts a running dispatch. Policy is
//! minimal (priority + per-principal quota + reservation) — enough to defer one
//! principal for another, no more (no fair-share, aging, work-stealing, preemption).
//!
//! The pure policy lives in [`admission_policy`]; this is the kernel glue — the
//! per-device queues, the decision trace, and reading the §9.5 accounting table for
//! quota (no parallel bookkeeping). Per-request principal attribution travels
//! through `accounting::InflightGuard` (BUILD-1 seam resolved).

// The scheduler's live entry point (`run_scheduled`) is wired into `sys_inf_run`
// (BUILD-2L). The lower-level `submit`/`complete`/`pending_len` primitives are
// exercised directly only by the feature-gated multi-tenant gate scenarios and the
// host policy unit tests, so suppress dead-code noise rather than gate them.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::ai::tensor::Device;
use crate::drivers::accel::accounting;
use crate::drivers::accel::accounting::InflightGuard;
use crate::errno::ECANCELED;
use crate::sched::process::{KERNEL_PID, ProcessId};
use crate::sched::{
    ThreadWakeToken, WakeTokenResult, block_current_with_token_and_guard, wake_thread_token,
};
use crate::serial_println;
use crate::sync::SwitchMutex;

#[path = "admission_policy.rs"]
mod admission_policy;
pub use admission_policy::{Decision, DeferReason, DeviceState, Request};

/// A principal is "over quota" on a device once its **accounted** (§9.5) dispatch
/// count on that device reaches this threshold; an over-quota principal yields to an
/// under-quota one when the device frees. Minimal fixed budget for BUILD-2 — an
/// elastic / configurable quota is later-build policy. Read from accounting, not a
/// second counter (ISC-4).
pub const DISPATCH_QUOTA: u64 = 4;

/// CP-66 (J3-P): cooperative cancellation of the admission section is on. The
/// `neo-test-admission-park-kill-control` build turns it off to show the
/// pre-fix behaviour: a force-kill zombifies a parked or preempted caller raw
/// (its `Arc`s leak) and `complete` admits the dead principal (the device
/// wedges).
pub const CANCEL_ON: bool = !cfg!(feature = "neo-test-admission-park-kill-control");

/// A request withdrawn because its process is being torn down: it was never
/// admitted (or, from [`run_scheduled`], never ran). The caller unwinds and
/// exits at the syscall boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cancelled;

/// A queued request plus the wake token of the thread parked on it (BUILD-2L).
/// `wake_token` is `Some` only for a caller that **parked** on `submit_and_park`
/// (the live path); the non-parking `submit` primitive (BUILD-2 gate scenario)
/// enqueues with `None`, so `complete`'s wake is a no-op for it.
struct PendingRequest {
    req: Request,
    wake_token: Option<ThreadWakeToken>,
}

struct DeviceQueue {
    running: Option<ProcessId>,
    reservation: Option<ProcessId>,
    pending: Vec<PendingRequest>,
}

impl DeviceQueue {
    fn new() -> Self {
        Self {
            running: None,
            reservation: None,
            pending: Vec::new(),
        }
    }

    fn state(&self) -> DeviceState {
        DeviceState {
            running: self.running.map(|p| p.0),
            reservation: self.reservation.map(|p| p.0),
        }
    }
}

/// Per-`Device::Accel(n)` admission state. `SwitchMutex` per the kernel's accel
/// locking discipline; the scheduler never runs in interrupt context.
/// Ordered (BUILD-P4a, §9.12): level `ADMISSION_QUEUES` — first in the global
/// order `QUEUES(10) → SLOT(15) → RESERVATION(20) → INFLIGHT(22) → TABLE(25) →
/// RECENCY(28) → scheduler`; see `sync::lock_order` and the debug lockdep in
/// `sync::switch_mutex`.
static QUEUES: SwitchMutex<BTreeMap<u32, DeviceQueue>> =
    SwitchMutex::new_ordered(BTreeMap::new(), crate::sync::lock_order::ADMISSION_QUEUES);
static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);

/// Quota predicate — reads the §9.5 accounting table (the single source of truth).
/// Lock order: callers hold `QUEUES` while calling this, which takes the accounting
/// `TABLE` lock; the accounting paths never take `QUEUES`, so `QUEUES → TABLE` is a
/// one-way order with no inversion.
fn over_quota(device: Device, principal: u32) -> bool {
    accounting::usage(device, ProcessId(principal)).dispatch_count >= DISPATCH_QUOTA
}

/// Submit an inference request for `device`; decide admit/defer by policy; emit the
/// cross-tenant decision trace. On `Admit`, the device becomes running this
/// principal (and reserved, if `reserved`); on `Defer`, the request is queued — the
/// caller runs the work (under `accounting::InflightGuard`) only when admitted, and
/// calls [`complete`] when it finishes to release the device to the next request.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn submit(device: Device, principal: ProcessId, priority: u32, reserved: bool) -> Decision {
    let submit_seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let req = Request {
        principal: principal.0,
        priority,
        submit_seq,
        reserved,
    };
    let mut queues = QUEUES.lock();
    let queue = queues
        .entry(device.as_abi())
        .or_insert_with(DeviceQueue::new);
    let decision = admission_policy::admit_arrival(&req, &queue.state());
    match decision {
        Decision::Admit => {
            queue.running = Some(principal);
            if reserved {
                queue.reservation = Some(principal);
            }
            drop(queues);
            trace_admitted(device, principal, priority);
            crate::log::flush_pending();
        }
        Decision::Defer(reason) => {
            queue.pending.push(PendingRequest {
                req,
                wake_token: None,
            });
            drop(queues);
            trace_deferred(device, principal, reason);
            crate::log::flush_pending();
        }
    }
    decision
}

/// Live-path admission (BUILD-2L): submit a request and **block-on-defer**. Decides
/// admit/defer under the `QUEUES` lock; on `Admit` returns immediately (the device is
/// now running this principal — single-tenant fast path: empty queue ⇒ no parking, no
/// latency). On `Defer`, the calling thread **parks** (no busy-wait) until [`complete`]
/// admits it on the holder's completion, then returns. Run-to-completion: the caller
/// MUST call [`complete`] after its work either way. This is the entry `run_scheduled`
/// (and thus `sys_inf_run`) routes through.
///
/// Lost-wakeup safety: the defer decision and the park are performed under a single
/// `QUEUES` acquisition — the pending request (carrying the wake token) is installed
/// by the block primitive's `install` closure *while `QUEUES` is still held*, and the
/// guard is released atomically with the context switch. A concurrent `complete`
/// cannot run between "decided defer" and "parked", so it can never wake a token that
/// is not yet stored. The parked thread holds **no** lock (`QUEUES` is released on the
/// switch), so it cannot block the admitter that must take `QUEUES` to wake it
/// (the `QUEUES → TABLE`/`QUEUES → scheduler` order is never inverted).
///
/// Cancellation (CP-66): returns `Err(Cancelled)` without admitting or parking
/// when the calling thread's process is being torn down, and when a parked
/// request is withdrawn by [`cancel_pid`]. The teardown flag is read under
/// `QUEUES`, which `cancel_pid` takes only after publishing it, so a request
/// is either pending when `cancel_pid` scans or sees the flag here. The
/// process `Arc` used for the read is dropped before parking (CP-30).
pub fn submit_and_park(
    device: Device,
    principal: ProcessId,
    priority: u32,
    reserved: bool,
) -> Result<(), Cancelled> {
    let submit_seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let req = Request {
        principal: principal.0,
        priority,
        submit_seq,
        reserved,
    };
    let caller = crate::sched::current_pid();
    let caller_proc = (CANCEL_ON && caller != KERNEL_PID)
        .then(|| crate::sched::process::get(caller))
        .map(|found| found.ok_or(()));
    let mut queues = QUEUES.lock();
    let torn_down = match &caller_proc {
        None => false,
        Some(Err(())) => true,
        Some(Ok(proc)) => proc.is_exiting(),
    };
    drop(caller_proc);
    if torn_down {
        drop(queues);
        return Err(Cancelled);
    }
    let state = queues
        .get(&device.as_abi())
        .map(|q| q.state())
        .unwrap_or_default();
    match admission_policy::admit_arrival(&req, &state) {
        Decision::Admit => {
            let queue = queues
                .entry(device.as_abi())
                .or_insert_with(DeviceQueue::new);
            queue.running = Some(principal);
            if reserved {
                queue.reservation = Some(principal);
            }
            drop(queues);
            trace_admitted(device, principal, priority);
            crate::log::flush_pending();
            Ok(())
        }
        Decision::Defer(reason) => {
            // Emit the cross-tenant decision trace before parking (still holding
            // QUEUES — stage-only, so it never waits on the log sink; flushed
            // once we resume).
            trace_deferred(device, principal, reason);
            let device_abi = device.as_abi();
            // Atomic: install the pending request (with this thread's wake token)
            // while QUEUES is held, then release QUEUES + scheduler lock on the
            // context switch. Resumes here once `complete` admits + wakes us, or
            // `cancel_pid` withdraws the request (the ECANCELED verdict).
            let verdict = block_current_with_token_and_guard(queues, |queue_map, token| {
                let queue = queue_map.entry(device_abi).or_insert_with(DeviceQueue::new);
                queue.pending.push(PendingRequest {
                    req,
                    wake_token: Some(token),
                });
            });
            crate::log::flush_pending();
            if verdict == ECANCELED {
                Err(Cancelled)
            } else {
                Ok(())
            }
        }
    }
}

/// The live inference admission path (BUILD-2L) — what `sys_inf_run` calls. Submits
/// `principal`'s request for `device`, **parks if deferred** until admitted, runs
/// `work` under the §9.5 in-flight-principal attribution ([`InflightGuard`]), then
/// completes (releasing the device to the next pending request, waking its parked
/// caller). Returns `work`'s result. Run-to-completion is enforced structurally: the
/// device stays "running this principal" from admission until `complete` here.
///
/// CP-66: the whole section — parked behind a holder, or holding the device
/// and preempted inside a dispatch — is marked on the thread, so a force-kill
/// never zombifies it from a queue (its stack owns the request's `Arc`s). A
/// withdrawn request returns `Err(Cancelled)` without running `work`; a killed
/// holder runs to completion and calls `complete`. Either way the caller
/// unwinds, and the syscall boundary exits the thread.
pub fn run_scheduled<R>(
    device: Device,
    principal: ProcessId,
    priority: u32,
    reserved: bool,
    work: impl FnOnce() -> R,
) -> Result<R, Cancelled> {
    crate::sched::scheduler::set_current_admission_section(true);
    if let Err(cancelled) = submit_and_park(device, principal, priority, reserved) {
        crate::sched::scheduler::set_current_admission_section(false);
        return Err(cancelled);
    }
    let result = {
        let _inflight = InflightGuard::enter(device, principal);
        work()
    };
    complete(device, principal);
    crate::sched::scheduler::set_current_admission_section(false);
    Ok(result)
}

/// Mark the request `principal` was running on `device` complete. Clears the device
/// (and the reservation, if this principal held it), then chooses + admits the next
/// pending request by policy (priority → under-quota → FIFO), emitting its
/// `ADMITTED` trace. Returns the newly-admitted principal, or `None` if the queue is
/// empty. Run-to-completion: this is the only point a new request starts.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn complete(device: Device, principal: ProcessId) -> Option<ProcessId> {
    let mut queues = QUEUES.lock();
    let queue = queues.get_mut(&device.as_abi())?;
    if queue.running == Some(principal) {
        queue.running = None;
    }
    if queue.reservation == Some(principal) {
        queue.reservation = None;
    }
    loop {
        // `choose_next` is pure (operates on `Request`); snapshot the request
        // fields, pick the winner index, then remove the owning `PendingRequest`
        // (with its token).
        let reqs: Vec<Request> = queue.pending.iter().map(|p| p.req).collect();
        let index = admission_policy::choose_next(&reqs, |p| over_quota(device, p))?;
        let next = queue.pending.remove(index);
        let next_principal = ProcessId(next.req.principal);
        queue.running = Some(next_principal);
        if next.req.reserved {
            queue.reservation = Some(next_principal);
        }
        let wake_token = next.wake_token;
        trace_admitted(device, next_principal, next.req.priority);
        // Wake the parked caller (live path) while still holding QUEUES: this
        // takes the scheduler lock (QUEUES → scheduler), the same order the park
        // path took, so no inversion. A non-parking enqueuer (BUILD-2 scenario)
        // has no token — no-op.
        let woken = match wake_token {
            Some(token) => wake_thread_token(token, 0) == WakeTokenResult::Woken,
            None => true,
        };
        // CP-66 backstop: a principal whose thread is gone can never call
        // `complete`; admitting it would wedge the device. Take it back and
        // choose again. (`cancel_pid` withdraws a dying process's requests
        // first, so this is defence in depth.)
        if woken || !CANCEL_ON {
            drop(queues);
            crate::log::flush_pending();
            return Some(next_principal);
        }
        trace_withdrawn(device.as_abi(), next_principal, "admitted principal gone");
        queue.running = None;
        if queue.reservation == Some(next_principal) {
            queue.reservation = None;
        }
    }
}

/// CP-66 (J3-P): withdraw every request of `pid`, which is being torn down
/// (callers publish `Process::exiting` first). Each pending request is removed
/// and its parked thread woken with the `ECANCELED` verdict, so it unwinds its
/// own frames (dropping the request's `Arc`s) and exits at the syscall
/// boundary instead of being zombified raw; `complete` can never admit it.
/// A holder of `pid` is left to finish (run-to-completion) and `complete`.
/// Returns how many of `pid`'s threads are in the admission section — the
/// cancelled waiters plus a running holder — which the kill paths count as
/// handled. A no-op in the control build.
pub fn cancel_pid(pid: ProcessId) -> usize {
    if !CANCEL_ON {
        return 0;
    }
    let mut queues = QUEUES.lock();
    let mut handled = 0usize;
    for (&abi, queue) in queues.iter_mut() {
        let mut i = 0;
        while i < queue.pending.len() {
            if queue.pending[i].req.principal != pid.0 {
                i += 1;
                continue;
            }
            let withdrawn = queue.pending.remove(i);
            if let Some(token) = withdrawn.wake_token {
                // QUEUES → scheduler, the order `complete` uses.
                if wake_thread_token(token, ECANCELED) == WakeTokenResult::Woken {
                    handled += 1;
                }
            }
            trace_withdrawn(abi, pid, "process torn down");
        }
        if queue.running == Some(pid) {
            handled += 1;
        }
    }
    drop(queues);
    handled
}

/// Test/observation: pending-queue depth for a device.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn pending_len(device: Device) -> usize {
    QUEUES
        .lock()
        .get(&device.as_abi())
        .map(|q| q.pending.len())
        .unwrap_or(0)
}

/// Test/observation: the principal the device is running, if any.
#[cfg(feature = "neo-test-admission-park-kill")]
pub fn running(device: Device) -> Option<ProcessId> {
    QUEUES.lock().get(&device.as_abi()).and_then(|q| q.running)
}

/// Every admission trace is printed stage-only (OT-83 F1, CP-71): it may run
/// with `QUEUES` held and at IF=0 (the exception kill's `cancel_pid`), where a
/// `SINK` holder preempted on this CPU could never release it. So the line is
/// staged or dropped (counted), never waited for or flushed here; callers call
/// `log::flush_pending` once `QUEUES` is released, and the reaper drains
/// what is left.
#[cfg(not(feature = "neo-test-admission-park-kill-sink-held-control"))]
fn trace_line(args: core::fmt::Arguments<'_>) {
    crate::arch::x86_64::without_interrupts(|| {
        let was = crate::log::enter_stage_only();
        serial_println!("{}", args);
        crate::log::leave_stage_only(was);
    });
}

/// Control: the pre-fix trace, which waits on `SINK` to flush.
#[cfg(feature = "neo-test-admission-park-kill-sink-held-control")]
fn trace_line(args: core::fmt::Arguments<'_>) {
    serial_println!("{}", args);
    crate::log::flush();
}

fn trace_admitted(device: Device, principal: ProcessId, priority: u32) {
    trace_line(format_args!(
        "accel-sched: infer principal=PID{} device=0x{:08x} priority={} ADMITTED",
        principal.0,
        device.as_abi(),
        priority,
    ));
}

fn trace_withdrawn(device_abi: u32, principal: ProcessId, why: &str) {
    trace_line(format_args!(
        "accel-sched: infer principal=PID{} device=0x{:08x} WITHDRAWN — {}",
        principal.0, device_abi, why,
    ));
}

fn trace_deferred(device: Device, principal: ProcessId, reason: DeferReason) {
    let (other, why) = match reason {
        DeferReason::Reservation { holder } => (holder, "reservation active"),
        DeferReason::DeviceBusy { running } => (running, "device busy"),
        DeferReason::OverQuota { admitted } => (admitted, "over quota"),
        DeferReason::LowerPriority { admitted } => (admitted, "lower priority"),
    };
    trace_line(format_args!(
        "accel-sched: infer principal=PID{} device=0x{:08x} DEFERRED — PID{} {}",
        principal.0,
        device.as_abi(),
        other,
        why,
    ));
}
