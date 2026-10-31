// SPDX-License-Identifier: MPL-2.0
//! Phase 7 BUILD-P4b: the async residency daemon — the kernel manages memory
//! while it works. ANSPEC-001 §9.12 (daemon subsection).
//!
//! A dedicated kernel thread (the kswapd/TPP model, on `sched/kthread.rs`)
//! that, concurrently with live dispatch, **bg-demotes** cold managed tensors
//! when a device's free bytes fall below the low watermark and **prefetches**
//! explicitly published-hot ones ahead of need — so the executor's critical
//! path stops paying for reclaim. Every movement goes through the **proven
//! P4a reservation** (`ai::reservation::try_migrate` on a [`TensorSlot`]):
//! the daemon claims-or-skips, never blocks, never touches what a pass holds.
//!
//! **Strictly additive (Q5):** the synchronous P2/P3 placement path is the
//! always-complete direct-reclaim backstop. The daemon dormant (never started,
//! or paused) is the default configuration and is bit-for-bit P3; awake, it
//! can only make placements cheaper, never make one fail. A daemon and a
//! synchronous reclaim racing for the same victim are safe by construction —
//! both go through the P4a claim; the loser skips.
//!
//! **The demotion surface is the managed-slot registry** (ADR-040): tensors a
//! tenant parks in registered [`TensorSlot`]s. The executor's value table
//! stays private and synchronous-only — the P4a soundness lemma keeps it that
//! way by design. Tenants signal reuse with [`publish_hot`]; the daemon never
//! prefetches on its own initiative (the structural anti-oscillation
//! guarantee) and never demotes a published-hot tensor.
//!
//! **Teardown is drain, never kill (P4a seam 1):** stop completes the current
//! cycle — every reservation ends Resident, every parked waiter is woken by
//! the commit/rollback that ends its reservation — then the thread exits.

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::ai::reservation::{self, MigrateOutcome};
use crate::ai::tensor::Device;
use crate::drivers::accel::{self, accounting};
use crate::sched::kthread::{self, KthreadExitState, KthreadHandle};
use crate::sched::{ThreadWakeToken, block_current_with_token_and_guard, wake_thread_token};
use crate::serial_println;
use crate::sync::SwitchMutex;

#[path = "watermark.rs"]
mod watermark;
pub use watermark::{WatermarkState, next_state, prefetch_admissible, watermarks};

/// The owner id the daemon stamps on its reservations (#4 diagnostics).
pub const DAEMON_OWNER: u64 = 0xDAE0;

/// The slot type moved to the arena in BUILD-O1a (the arena owns ownership);
/// re-exported so scenario code is source-identical.
pub use super::arena::ManagedSlot;
use super::arena::SlotRef;

/// Daemon control: the park token and the pending-kick flag, under one lock so
/// a kick that races the park can never be lost (token installed under the
/// same guard the kicker takes — the wake-token discipline).
struct DaemonControl {
    waiter: Option<ThreadWakeToken>,
    kicked: bool,
}

static CONTROL: SwitchMutex<DaemonControl> = SwitchMutex::new(DaemonControl {
    waiter: None,
    kicked: false,
});

/// Daemon enablement, independent of the thread being alive: the gate's
/// baseline phase pauses the daemon to measure the backstop-only cost.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Per-device noted max placement-need — the watermark `H_dev` (P4a seam 3:
/// per-device, replacing P3's pass-global figure). MONOTONIC-LATCH NOTE
/// (ADR-040): the hint only grows; one outsized placement permanently raises
/// the low watermark and can make the daemon over-drain cold tensors. Bounded
/// by capacity and policy-quality-only (never correctness); a decaying or
/// windowed estimate is the named follow-on.
static NEED_HINTS: SwitchMutex<Vec<(u32, u64)>> = SwitchMutex::new(Vec::new());

/// Per-device hysteresis state (the pure `watermark::next_state` machine,
/// wired into `run_cycle`): the daemon begins reclaiming only when woken below
/// LOW, continues to HIGH, and HOLDS inside the band.
static WATERMARK_STATES: SwitchMutex<Vec<(u32, WatermarkState)>> = SwitchMutex::new(Vec::new());

fn device_state(abi: u32) -> WatermarkState {
    WATERMARK_STATES
        .lock()
        .iter()
        .find(|(d, _)| *d == abi)
        .map(|(_, state)| *state)
        .unwrap_or(WatermarkState::Sleeping)
}

fn set_device_state(abi: u32, state: WatermarkState) {
    let mut states = WATERMARK_STATES.lock();
    match states.iter_mut().find(|(d, _)| *d == abi) {
        Some((_, existing)) => *existing = state,
        None => states.push((abi, state)),
    }
}

// --- §9.12 daemon discriminator counters -------------------------------------

static BG_DEMOTES: AtomicU64 = AtomicU64::new(0);
static PREFETCHES: AtomicU64 = AtomicU64::new(0);
static PREFETCH_REFUSALS: AtomicU64 = AtomicU64::new(0);
static MISPREDICT_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static DAEMON_SKIPS: AtomicU64 = AtomicU64::new(0);
static KICKS: AtomicU64 = AtomicU64::new(0);
/// Discriminator #6: the daemon-alive heartbeat — instrument the silence.
static HEARTBEAT: AtomicU64 = AtomicU64::new(0);

/// `hot` and `prefetched` are a two-bit tenant/daemon protocol. A tenant can
/// consume a prefetched tensor (`mark_used`) and then withdraw its prediction
/// (`clear_hot`); rollback must not observe the withdrawal without also
/// observing the consumed-prefetch clear.
const PREDICTION_ORDER: Ordering = Ordering::SeqCst;

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn reset_counters() {
    BG_DEMOTES.store(0, Ordering::Relaxed);
    PREFETCHES.store(0, Ordering::Relaxed);
    PREFETCH_REFUSALS.store(0, Ordering::Relaxed);
    MISPREDICT_ROLLBACKS.store(0, Ordering::Relaxed);
    DAEMON_SKIPS.store(0, Ordering::Relaxed);
    KICKS.store(0, Ordering::Relaxed);
}

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn bg_demotes() -> u64 {
    BG_DEMOTES.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn prefetches() -> u64 {
    PREFETCHES.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn prefetch_refusals() -> u64 {
    PREFETCH_REFUSALS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn mispredict_rollbacks() -> u64 {
    MISPREDICT_ROLLBACKS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn heartbeat() -> u64 {
    HEARTBEAT.load(Ordering::Relaxed)
}

// --- Tenant / placement-path API ---------------------------------------------

/// Register a managed slot (the legacy 'static path — adopted by the arena
/// as a permanent, never-freed entry; BUILD-O1a). Walk order is unchanged:
/// the arena iterates in index order, which for never-freed entries IS
/// registration order (the ISC-2 equivalence).
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn register(slot: &'static ManagedSlot) {
    super::arena::register_static(slot);
}

/// Publish a reuse prediction for a managed slot's tensor: the daemon may
/// prefetch it to its home device. The ONLY prefetch trigger (structural
/// anti-oscillation: no publish, no prefetch).
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn publish_hot(slot: &ManagedSlot) {
    slot.hot.store(true, PREDICTION_ORDER);
}

/// Withdraw the prediction. A prefetched tensor whose prediction is withdrawn
/// before use is a misprediction: the daemon demotes it back (byte-preserving)
/// and counts the rollback.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn clear_hot(slot: &ManagedSlot) {
    slot.hot.store(false, PREDICTION_ORDER);
}

/// Signal that a prefetched tensor was actually consumed (the prediction paid
/// off): its later demotion is ordinary cold reclaim, never a misprediction.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn mark_used(slot: &ManagedSlot) {
    slot.prefetched.store(false, PREDICTION_ORDER);
}

/// Keyed tenant prediction signals (BUILD-O1b): the production surfaces hold
/// `SlotKey`s, not slot references — these resolve through the arena's
/// generation check (stale key ⇒ `false`, never the wrong slot — I-13).
#[allow(dead_code)] // gate/production tenant API — feature-gated consumers
pub fn publish_hot_key(key: super::arena::SlotKey) -> bool {
    match super::arena::slot_ref(key) {
        Some(slot_ref) => {
            publish_hot(slot_ref.managed());
            true
        }
        None => false,
    }
}

#[allow(dead_code)] // gate/production tenant API — feature-gated consumers
pub fn clear_hot_key(key: super::arena::SlotKey) -> bool {
    match super::arena::slot_ref(key) {
        Some(slot_ref) => {
            clear_hot(slot_ref.managed());
            true
        }
        None => false,
    }
}

#[allow(dead_code)] // gate/production tenant API — feature-gated consumers
pub fn mark_used_key(key: super::arena::SlotKey) -> bool {
    match super::arena::slot_ref(key) {
        Some(slot_ref) => {
            mark_used(slot_ref.managed());
            true
        }
        None => false,
    }
}

/// Note a placement's room-need on `device` (called from the placement
/// consult): the latch step — a live workload's need always raises (or
/// restores) the hint immediately, decay or no decay.
pub fn note_need(device: Device, need: u64) {
    let abi = device.as_abi();
    let decay = DECAY_ENABLED.load(Ordering::Acquire);
    let mut hints = NEED_HINTS.lock();
    match hints.iter_mut().find(|(d, _)| *d == abi) {
        // BUILD-C1: decay is DEMAND-driven — one step per note, then the
        // latch. Each demand event erodes an outlier toward the recent max
        // (the ADR-040 latch hazard, fixed) while quiet windows freeze the
        // band: the gate's first C1 run caught wake-driven decay collapsing
        // low during kick-heavy drain waits where no notes flow.
        Some((_, hint)) => *hint = watermark::latch(watermark::decay_step(*hint, decay), need),
        None => hints.push((abi, need)),
    }
}

/// BUILD-C1 (the ADR-040 monotonic-latch fix): hints decay an eighth per
/// demand note toward recent demand, so one outsized placement no longer
/// inflates the watermark band forever. Kill-switch (Rule 1): decay disabled
/// reproduces the original latch bit-for-bit (`watermark::decay_step` is the
/// identity — unit-test equality in the host suite).
static DECAY_ENABLED: AtomicBool = AtomicBool::new(true);

/// The decaying-H kill-switch (BUILD-C1, Rule 1).
#[allow(dead_code)] // Rule-1 kill-switch entry — gate + host-equality consumer
pub fn set_decay_enabled(enabled: bool) {
    DECAY_ENABLED.store(enabled, Ordering::Release);
}

/// The device's noted max placement-need — the watermark `H_dev`, also read by
/// the P3 synchronous demote seam (the seam-3 unification).
pub fn need_hint(device: Device) -> u64 {
    let abi = device.as_abi();
    NEED_HINTS
        .lock()
        .iter()
        .find(|(d, _)| *d == abi)
        .map(|(_, hint)| *hint)
        .unwrap_or(0)
}

/// Wake the daemon (placement found free < low, a tenant changed the pool, or
/// teardown). Non-blocking; the synchronous backstop never waits on it.
pub fn kick() {
    KICKS.fetch_add(1, Ordering::Relaxed);
    let waiter = {
        let mut control = CONTROL.lock();
        control.kicked = true;
        control.waiter.take()
    };
    if let Some(token) = waiter {
        let _ = wake_thread_token(token, 0);
    }
}

/// Enable/disable the daemon's actions without stopping the thread (the
/// gate's baseline phase). Disabled ⇒ the loop wakes, does nothing, sleeps —
/// the system is bit-for-bit P3.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
    if enabled {
        kick();
    }
}

#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// Whether the daemon thread is currently parked (its wake token installed).
/// The gate's setup quiesces on this after disabling — a mid-cycle daemon must
/// finish and park before a baseline measurement begins.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn is_parked() -> bool {
    CONTROL.lock().waiter.is_some()
}

// --- The daemon ---------------------------------------------------------------

/// Start the daemon thread (dormant until [`set_enabled`]). The returned
/// handle's [`stop`] is the only correct teardown.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn start() -> KthreadHandle {
    kthread::spawn_owned("residency-daemon", |stop: kthread::KthreadStopToken| {
        daemon_loop(&stop);
        KthreadExitState::Success
    })
}

/// Teardown (P4a seam 1): request stop, kick the parked daemon, and join. The
/// loop finishes its current cycle — every reservation it took has already
/// committed or rolled back (each `try_migrate` is atomic), so no waiter can
/// be left parked and no slot left Reserved.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn stop(handle: KthreadHandle) {
    handle.request_stop();
    kick();
    let _ = handle.join();
}

fn daemon_loop(stop: &kthread::KthreadStopToken) {
    loop {
        if stop.is_stop_requested() {
            // Drain semantics: each try_migrate below is atomic (commit or
            // rollback wakes its waiters before returning), so reaching this
            // point with no cycle in flight IS the drained state.
            return;
        }
        if is_enabled() {
            run_cycle();
        }
        HEARTBEAT.fetch_add(1, Ordering::Relaxed);

        // Park until kicked or stopped — token installed under the same lock
        // `kick` takes, so a racing kick either finds the token (wakes us) or
        // sets `kicked` before we test it (we skip the park).
        let control = CONTROL.lock();
        if stop.is_stop_requested() {
            return;
        }
        if control.kicked {
            let mut control = control;
            control.kicked = false;
            continue;
        }
        let _ = block_current_with_token_and_guard(control, |state, token| {
            debug_assert!(state.waiter.is_none(), "daemon double-parked");
            state.waiter = Some(token);
        });
    }
}

/// One reclaim/prefetch cycle over every capacity-bounded device.
fn run_cycle() {
    for index in 0..accel::device_count() {
        let Some(driver) = accel::get_device(index) else {
            continue;
        };
        let Some(capacity) = driver.capacity_bytes() else {
            continue;
        };
        let device = driver.descriptor().device;
        let abi = device.as_abi();
        let (low, high) = watermarks(capacity, need_hint(device));
        if low == 0 {
            continue; // no placement has touched this device yet
        }

        // ── Demote: hysteresis-gated (the pure state machine, wired): enter
        // Reclaiming only when free < low, continue to high, HOLD inside the
        // band — a kick arriving with free in [low, high) does nothing (the
        // anti-oscillation property). Victims are the coldest non-hot managed
        // tensors via claim-or-skip, never block.
        loop {
            let free = capacity.saturating_sub(accounting::device_resident_total(device));
            let state = next_state(device_state(abi), free, low, high);
            set_device_state(abi, state);
            if state != WatermarkState::Reclaiming {
                break;
            }
            let Some(victim) = coldest_demotable(device) else {
                break;
            };
            let resident_before = accounting::device_resident_total(device);
            match reservation::try_migrate(victim.managed().slot(), Device::Cpu, DAEMON_OWNER) {
                MigrateOutcome::Migrated => {
                    BG_DEMOTES.fetch_add(1, Ordering::Relaxed);
                    serial_println!(
                        "residency: device={:?} → cpu reason=bg-demote (daemon, resident={}/{} low={})",
                        device,
                        resident_before,
                        capacity,
                        low,
                    );
                    crate::log::flush();
                }
                MigrateOutcome::SkippedReferenced | MigrateOutcome::SkippedContended => {
                    // In use or contended right now — skip; the next kick
                    // revisits. Never block (MIGRATE_SYNC_LIGHT).
                    DAEMON_SKIPS.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                MigrateOutcome::NotPresent | MigrateOutcome::Failed => break,
            }
        }

        // ── Prefetch: published-hot, CPU-resident managed tensors, behind the
        // thrash-guard hard gate (free after transfer stays ≥ low).
        let keys = super::arena::live_keys();
        for key in keys {
            let Some(slot_ref) = super::arena::slot_ref(key) else {
                continue;
            };
            let managed = slot_ref.managed();
            if managed.home != device || !managed.hot.load(PREDICTION_ORDER) {
                continue;
            }
            let Some(tensor) = managed.guarded_clone() else {
                continue;
            };
            let bytes = tensor.byte_len() as u64;
            let on_cpu = tensor.device() == Device::Cpu;
            drop(tensor); // never hold a reference across the claim
            if !on_cpu {
                continue;
            }
            let free = capacity.saturating_sub(accounting::device_resident_total(device));
            if !prefetch_admissible(free, bytes, low) {
                PREFETCH_REFUSALS.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            // Publish the prefetch marker before migration can become visible
            // to a tenant. If the tenant then consumes the tensor, mark_used's
            // clear wins and rollback cannot count it as a misprediction.
            managed.prefetched.store(true, PREDICTION_ORDER);
            if reservation::try_migrate(managed.slot(), device, DAEMON_OWNER)
                == MigrateOutcome::Migrated
            {
                PREFETCHES.fetch_add(1, Ordering::Relaxed);
                serial_println!(
                    "residency: cpu → device={:?} reason=prefetch (daemon, headroom ok, bytes={})",
                    device,
                    bytes,
                );
                crate::log::flush();
            } else {
                managed.prefetched.store(false, PREDICTION_ORDER);
                DAEMON_SKIPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        // ── Misprediction rollback: a still-`prefetched` (never consumed)
        // tensor whose prediction was withdrawn goes straight back to CPU —
        // byte-preserving, counted. The withdrawal itself is the trigger
        // (BUILD-C1): the original coupling to the high watermark made the
        // rollback timing depend on the H estimate, which the decaying hint
        // would have turned into a flake; a withdrawn-and-unconsumed prefetch
        // has no claim to device residency at any pressure level. A consumed
        // (`mark_used`) or re-published tensor is never touched by this sweep.
        let keys = super::arena::live_keys();
        for key in keys {
            let Some(slot_ref) = super::arena::slot_ref(key) else {
                continue;
            };
            let managed = slot_ref.managed();
            if managed.home != device
                || managed.hot.load(PREDICTION_ORDER)
                || !managed.prefetched.load(PREDICTION_ORDER)
            {
                continue;
            }
            let Some(tensor) = managed.guarded_clone() else {
                continue;
            };
            let resident_here = tensor.device() == device;
            drop(tensor);
            if !resident_here {
                managed.prefetched.store(false, PREDICTION_ORDER);
                continue;
            }
            if reservation::try_migrate(managed.slot(), Device::Cpu, DAEMON_OWNER)
                == MigrateOutcome::Migrated
            {
                managed.prefetched.store(false, PREDICTION_ORDER);
                MISPREDICT_ROLLBACKS.fetch_add(1, Ordering::Relaxed);
                serial_println!(
                    "residency: device={:?} → cpu reason=mispredict-rollback (daemon, prediction withdrawn)",
                    device,
                );
                crate::log::flush();
            }
        }
    }
}

/// The coldest (lowest §9.5 recency tick) managed tensor resident on `device`
/// that is not published-hot — chosen by the SAME pure LRU ordering the
/// synchronous demote seam uses (`evict_policy::coldest_eligible`; BUILD-C1
/// unification — the former ad-hoc min-scan is gone). Candidates are built
/// through the shared `evict::candidate_from` join; `evictable` is passed
/// `true` because the reservation claim is the authoritative re-check (the
/// daemon's own transient clone would otherwise inflate the count).
fn coldest_demotable(device: Device) -> Option<SlotRef> {
    let keys = super::arena::live_keys();
    let mut candidates = alloc::vec::Vec::new();
    let mut slot_of: alloc::vec::Vec<(u64, SlotRef)> = alloc::vec::Vec::new();
    for key in keys {
        let Some(slot_ref) = super::arena::slot_ref(key) else {
            continue;
        };
        let managed = slot_ref.managed();
        if managed.home != device || managed.hot.load(PREDICTION_ORDER) {
            continue;
        }
        let Some(tensor) = managed.guarded_clone() else {
            continue;
        };
        if tensor.device() != device {
            continue;
        }
        candidates.push(crate::ai::evict::candidate_from(
            &tensor, device, true, false, true,
        ));
        slot_of.push((tensor.id(), slot_ref.clone()));
    }
    let victim = crate::ai::evict::coldest_eligible(&candidates)?;
    slot_of
        .iter()
        .find(|(id, _)| *id == victim)
        .map(|(_, slot)| slot.clone())
}
