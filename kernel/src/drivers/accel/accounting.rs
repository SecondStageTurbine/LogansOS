// SPDX-License-Identifier: MPL-2.0
//! Phase 7 BUILD-1: per-`(device, principal)` accelerator accounting — the
//! governance **measurement floor**.
//!
//! You cannot arbitrate what you cannot measure. This module records, per
//! principal, how much of each `Device::Accel(n)` that principal used —
//! `resident_bytes` (exact), `dispatch_count` (exact), `dispatch_time` (real,
//! non-deterministic). It is **measurement only** (ANSPEC-001 §9.5): no
//! scheduling, quota, reservation, isolation, or any decision on the numbers —
//! that is BUILD-2+. The pure arithmetic lives in [`accounting_table`]; this is
//! the kernel glue (the global table, principal resolution, the dispatch
//! wrapper, the trace).
//!
//! Hooks live at the `AccelDriver`/residency boundary, not per-backend, so every
//! accelerator (`accel_cpu`, `accel_sim`, `virtio_accel`, `accel_intel`) is
//! covered uniformly: residency is captured at `DeviceStorage` alloc/drop;
//! dispatch is captured by [`dispatch_and_wait`], the common submit→complete
//! wrapper the inference executor uses.

extern crate alloc;

use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;

use crate::ai::tensor::Device;
use crate::arch::x86_64::timer::TICKS;
use crate::sched::current_pid;
use crate::sched::process::ProcessId;
use crate::serial_println;
use crate::sync::SwitchMutex;

#[path = "accounting_table.rs"]
mod accounting_table;
pub use accounting_table::{AccelUsage, AccountingTable};

#[path = "recency_table.rs"]
mod recency_table;
pub use recency_table::RecencyTable;

use super::{AccelDriver, AccelResult, OpBatch};

/// The global accounting table. `SwitchMutex` matches the accel registry's
/// locking discipline; the accounting paths never run in interrupt context.
/// Ordered (BUILD-P4a, §9.12): level `ACCT_TABLE` in the global lock order —
/// acquirable under the reservation lock (the atomic migrate+accounting
/// commit), never while holding `RECENCY`.
static TABLE: SwitchMutex<AccountingTable> =
    SwitchMutex::new_ordered(AccountingTable::new(), crate::sync::lock_order::ACCT_TABLE);

/// The global last-use recency table (Phase 7 BUILD-P2) — the §9.5 extension
/// LRU eviction reads. Same locking discipline as `TABLE`; the two are never
/// held at once (no nesting, no inversion). Ordered: level `ACCT_RECENCY`.
static RECENCY: SwitchMutex<RecencyTable> =
    SwitchMutex::new_ordered(RecencyTable::new(), crate::sync::lock_order::ACCT_RECENCY);

/// Explicit-principal override channel — the "thread the originating `ProcessId`"
/// seam (ISC-3). When **unset** (`PRINCIPAL_NONE`), accounting attributes to the
/// running thread's pid via [`current_pid`], which is `KERNEL_PID` (the system
/// principal) for kernel-internal dispatch — so accounting **never panics on
/// "no process."** The inference syscall and the accounting scenario set this
/// (via [`PrincipalGuard`]) to the owning process. The capability-principal
/// mapping is a labelled BUILD-3 seam; `ProcessId` is the unit here.
const PRINCIPAL_NONE: u32 = u32::MAX;
static PRINCIPAL_OVERRIDE: AtomicU32 = AtomicU32::new(PRINCIPAL_NONE);

/// Per-device in-flight principal (Phase 7 BUILD-2, ISC-1). The admission
/// scheduler sets the principal of the request **currently executing on a device**
/// here (via [`InflightGuard`]); the accounting hooks read it *per device*, so a
/// request's residency/dispatch is attributed to *its* principal — read from the
/// in-flight request, not a single global. Because accelerators run to completion
/// (one request per device at a time, temporal isolation), one slot per device is
/// exact, and concurrent requests on *different* devices are each correct. This
/// resolves the BUILD-1 single-active-principal stand-in: that global override
/// ([`PrincipalGuard`]) remains only as the lowest-precedence fallback.
static INFLIGHT: SwitchMutex<BTreeMap<u32, ProcessId>> =
    SwitchMutex::new_ordered(BTreeMap::new(), crate::sync::lock_order::ACCT_INFLIGHT);

/// The principal accelerator work on `device` is currently attributed to, in
/// precedence order: the in-flight request's principal on that device (set by the
/// admission scheduler), else the global override (BUILD-1 `PrincipalGuard`), else
/// the running thread's process (`KERNEL_PID` for kernel threads). Never panics.
pub fn current_principal(device: Device) -> ProcessId {
    if let Some(principal) = INFLIGHT.lock().get(&device.as_abi()).copied() {
        return principal;
    }
    match PRINCIPAL_OVERRIDE.load(Ordering::Relaxed) {
        PRINCIPAL_NONE => current_pid(),
        pid => ProcessId(pid),
    }
}

/// Scoped per-device in-flight principal (RAII). The admission scheduler enters
/// this with the executing request's `{device, principal}` so the accounting hooks
/// attribute that request's work to *its* principal; the prior value is restored on
/// drop (so the device returns to "no in-flight request"). This is the load-bearing
/// per-request attribution path — the principal travels with the request through the
/// dispatch path, not a global.
pub struct InflightGuard {
    device_abi: u32,
    prev: Option<ProcessId>,
}

impl InflightGuard {
    pub fn enter(device: Device, principal: ProcessId) -> Self {
        let device_abi = device.as_abi();
        let prev = INFLIGHT.lock().insert(device_abi, principal);
        Self { device_abi, prev }
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let mut map = INFLIGHT.lock();
        match self.prev {
            Some(principal) => {
                map.insert(self.device_abi, principal);
            }
            None => {
                map.remove(&self.device_abi);
            }
        }
    }
}

/// Scoped principal override (RAII). Entered with the owning `ProcessId`; the
/// prior value is restored on drop, so nested/guarded regions compose. The
/// inference syscall enters it with the calling process; the accounting scenario
/// enters it per simulated principal. Outside any guard, attribution falls back
/// to [`current_pid`].
///
/// BUILD-1 note: a single global override is correct for the synchronous,
/// one-principal-at-a-time inference of this build (the QEMU gate runs principals
/// sequentially). A per-request principal that survives concurrent cross-tenant
/// dispatch is BUILD-2 scheduler state, not measurement.
pub struct PrincipalGuard {
    prev: u32,
}

impl PrincipalGuard {
    pub fn enter(principal: ProcessId) -> Self {
        Self {
            prev: PRINCIPAL_OVERRIDE.swap(principal.0, Ordering::Relaxed),
        }
    }
}

impl Drop for PrincipalGuard {
    fn drop(&mut self) {
        PRINCIPAL_OVERRIDE.store(self.prev, Ordering::Relaxed);
    }
}

// --- Residency hooks (called from `DeviceStorage` alloc/drop) ---------------

/// `+bytes` resident for `principal` on `device`. The owning principal is
/// captured at alloc time and carried by the `DeviceStorage`, so the matching
/// [`resident_sub`] decrements the same principal regardless of the context at
/// free time.
pub fn resident_add(device: Device, principal: ProcessId, bytes: u64) {
    MUTATION_EPOCH.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    TABLE
        .lock()
        .add_resident(device.as_abi(), principal.0, bytes);
}

/// Monotone count of residency mutations (adds + subs), BUILD-O1a: lets the
/// §9.12 per-migration delta probe tell an unconfounded measurement window
/// (only its own mutations landed) from one interleaved with concurrent
/// frees/allocs — strict equality is enforced in the former and counted
/// unmeasured in the latter, so the probe never false-positives under
/// designed concurrency yet stays armed at full strength when quiescent.
static MUTATION_EPOCH: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

pub fn mutation_epoch() -> u64 {
    MUTATION_EPOCH.load(core::sync::atomic::Ordering::Relaxed)
}

/// `-bytes` resident for `principal` on `device`, on free/drop/migrate-off.
pub fn resident_sub(device: Device, principal: ProcessId, bytes: u64) {
    MUTATION_EPOCH.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    TABLE
        .lock()
        .sub_resident(device.as_abi(), principal.0, bytes);
}

// --- Recency hooks (§9.5 extension, Phase 7 BUILD-P2) ------------------------

/// Record a use of `tensor_id` while resident on `device`: executor op touch,
/// device-storage allocation, or migrate-onto-device. Monotonic table tick.
pub fn recency_touch(device: Device, tensor_id: u64) {
    RECENCY.lock().touch(device.as_abi(), tensor_id);
}

/// Drop `tensor_id`'s recency entry (migrate-off-device / tensor drop).
/// Idempotent; keeps the table bounded by live device-resident tensors.
pub fn recency_forget(tensor_id: u64) {
    RECENCY.lock().forget(tensor_id);
}

/// Last-use tick of `tensor_id` on `device` — `None` if never used there.
/// The eviction policy treats `None` as coldest.
pub fn recency_tick(device: Device, tensor_id: u64) -> Option<u64> {
    RECENCY.lock().tick_of(device.as_abi(), tensor_id)
}

/// Residency across **all** principals on `device` (BUILD-P2): the eviction
/// policy's `R`, read — never recomputed — from the §9.5 table.
pub fn device_resident_total(device: Device) -> u64 {
    TABLE.lock().device_residency(device.as_abi()).current
}

/// High-water residency on `device` — the gate's proof that `R ≤ C` held
/// throughout a run.
pub fn device_peak_resident(device: Device) -> u64 {
    TABLE.lock().device_residency(device.as_abi()).peak
}

// --- Dispatch hook (the common submit→complete wrapper) ---------------------

/// Dispatch a batch and wait for completion, accounting `dispatch_count += 1` and
/// `dispatch_time += elapsed` to the current principal on the driver's device.
/// This is the single trait-boundary dispatch hook: the inference executor and
/// the accounting scenario both route through it, so every backend is accounted
/// uniformly without per-backend code. The op is known *done* when `wait`
/// returns, so the count/time are recorded then.
pub fn dispatch_and_wait(
    driver: &dyn AccelDriver,
    batch: &mut OpBatch<'_, '_>,
    timeout: Option<Duration>,
) -> AccelResult<()> {
    let device = driver.descriptor().device;
    let start = TICKS.load(Ordering::Acquire);
    let token = driver.dispatch(batch)?;
    // A failed wait returns Err here without recording a phantom dispatch.
    driver.wait(token, timeout)?;
    let elapsed = TICKS.load(Ordering::Acquire).saturating_sub(start);
    // Resolve the principal (may take the INFLIGHT/scheduler locks) BEFORE taking
    // the TABLE lock — never hold TABLE across that resolution (lock-order discipline).
    let principal = current_principal(device);
    TABLE
        .lock()
        .record_dispatch(device.as_abi(), principal.0, elapsed);
    Ok(())
}

// --- Observation / lifecycle ------------------------------------------------

/// Current usage for `(device, principal)` — all-zero if never seen. The
/// correctness oracles assert `resident_bytes`/`dispatch_count` exactly against
/// this and `dispatch_time_ticks` only as monotonic/non-negative.
pub fn usage(device: Device, principal: ProcessId) -> AccelUsage {
    TABLE.lock().usage(device.as_abi(), principal.0)
}

/// Drop a principal's entries (process teardown) — keeps the table bounded.
pub fn forget_principal(principal: ProcessId) {
    TABLE.lock().forget_principal(principal.0);
}

/// The governance trace gate (ISC-4): emit one `accel-acct:` line per
/// `(device, principal)`. This is the build's success metric — measurement made
/// observable, no decision attached.
pub fn trace() {
    let entries = TABLE.lock().entries();
    for ((device_abi, pid), usage) in entries {
        serial_println!(
            "accel-acct: principal=PID{} device=0x{:08x} resident_bytes={} dispatches={} time_ticks={}",
            pid,
            device_abi,
            usage.resident_bytes,
            usage.dispatch_count,
            usage.dispatch_time_ticks,
        );
    }
    crate::log::flush();
}

/// Test seam (`i2c_hid_absent`, I-14(a)): run `f` while this thread holds
/// `INFLIGHT`, `TABLE` and `RECENCY`, taken in lockdep order (22 < 25 < 28),
/// so an ISR that wanted any of them would never get it. Production never
/// holds `TABLE` and `RECENCY` together; the order still permits it.
#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn debug_with_accounting_held<R>(f: impl FnOnce() -> R) -> R {
    let _inflight = INFLIGHT.lock();
    let _table = TABLE.lock();
    let _recency = RECENCY.lock();
    f()
}
