// SPDX-License-Identifier: MPL-2.0
//! Phase 7 BUILD-P4a: the migration-reservation primitive — move-while-
//! referenced is safe. ANSPEC-001 §9.12.
//!
//! BUILD-O1a (ADR-042) builds the slot arena on this primitive unchanged:
//! `guarded_clone` is the arena's mint gate and `take` its drain step, and
//! the I-4 predicate this module enforces now reads, for arena-owned
//! tensors, as exactly "no use in flight" (unique-strong-owner-at-rest).
//!
//! Every residency decision through P3 was safe because the executor pass is
//! the only mutator of its value table. A concurrent mover (the P4b daemon)
//! needs a primitive that makes the relaxation safe. This module provides it.
//!
//! **The shape (the `PG_isolated` analogue):** a tensor eligible for
//! background movement lives in a [`TensorSlot`]. The mover's claim — under
//! the slot's ordered lock — checks the §9.10 evictability gate
//! (`strong == 1 && weak == 0`, not already reserved; a contended claim is
//! **skipped, never blocked**) and then **takes the `Arc` out of the slot**,
//! leaving a `Reserved` marker. The migration itself runs with **no locks
//! held**: the mover owns the only reference (protection by possession —
//! `Arc::get_mut` cannot fail), so the long copy never monopolizes the
//! container. An accessor that hits the `Reserved` marker **parks** via
//! `block_current_with_token_and_guard` (token installed while the
//! reservation lock is held — no lost-wakeup window) and **re-resolves from
//! scratch** on wake (the fault-replay analogue). The commit restores the
//! migrated tensor to the slot, removes the reservation, and wakes the
//! waiters — from any observer's view the storage swap and the §9.5 updates
//! are atomic, because every observer is parked-or-parking until the commit.
//!
//! **The soundness lemma (refcount-preserving, Q1):** references are minted
//! only from an owning container or via `Weak::upgrade`. The gate's
//! `weak == 0` makes any `Weak`-bearing tensor unclaimable, so an upgrade can
//! never race a claim; taking the `Arc` out of the slot makes the container
//! unable to mint references for the duration; and a container whose accessor
//! skips this protocol can still never produce a use-after-migrate — it can
//! only make the gate see `strong > 1` and the claim skip. Zero new `unsafe`.
//!
//! Accounting note: the per-migration §9.5 delta is asserted at the commit
//! boundary (#5). Within a migration the same transient source+target overlap
//! exists that every synchronous migrate has had since P2 (new storage is
//! accounted before old storage frees); the reservation does not change it.
//!
//! Discriminators (§9.12, instrumented in-build): the storage generation
//! counter lives on `Tensor` (#1); this module counts parks/retries and keeps
//! the parked gauge for quiesce (#2); the lock order is asserted by the debug
//! lockdep (#3, `sync::lock_order`); claim contention is counted and a double
//! claim is structurally impossible — the `Arc` is simply not there (#4); the
//! per-migration delta is asserted at commit (#5).

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::ai::tensor::{Device, Tensor};
use crate::drivers::accel::accounting;
use crate::sched::{ThreadWakeToken, block_current_with_token_and_guard, wake_thread_token};
use crate::sync::SwitchMutex;

#[path = "reservation_logic.rs"]
mod reservation_logic;
pub use reservation_logic::{ClaimDecision, decide_claim};

/// What a [`TensorSlot`] holds.
enum SlotState {
    Empty,
    Resident(Arc<Tensor>),
    /// The tensor is out with a mover; `tensor_id` keys the reservation entry
    /// an accessor parks on.
    Reserved {
        tensor_id: u64,
    },
}

/// A tensor-owning container that participates in the reservation protocol —
/// the surface a background mover and racing accessors share. Ordered at
/// level `TENSOR_SLOT` (15), below the reservation lock (20).
pub struct TensorSlot {
    inner: SwitchMutex<SlotState>,
}

impl TensorSlot {
    pub const fn new() -> Self {
        Self {
            inner: SwitchMutex::new_ordered(SlotState::Empty, crate::sync::lock_order::TENSOR_SLOT),
        }
    }

    /// Install a tensor (replacing any resident one). Panics if reserved —
    /// callers must quiesce movers before repopulating a slot.
    pub fn put(&self, tensor: Arc<Tensor>) {
        let mut state = self.inner.lock();
        assert!(
            !matches!(*state, SlotState::Reserved { .. }),
            "TensorSlot::put while a migration reservation is held"
        );
        *state = SlotState::Resident(tensor);
    }

    /// Remove and return the resident tensor; `None` if empty or currently
    /// reserved (the caller retries after the mover commits).
    pub fn take(&self) -> Option<Arc<Tensor>> {
        let mut state = self.inner.lock();
        match core::mem::replace(&mut *state, SlotState::Empty) {
            SlotState::Resident(tensor) => Some(tensor),
            other @ SlotState::Reserved { .. } => {
                *state = other;
                None
            }
            SlotState::Empty => None,
        }
    }
}

impl Default for TensorSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Reservation lifecycle (diagnostics #4: exactly one owner in either state).
/// The production path ([`try_migrate`]) enters `Migrating` directly — its
/// swap begins as soon as the claim commits, so a separate `Claimed` hop would
/// be a fiction. `Claimed` exists for the held-open probe seam
/// ([`debug_claim_only`]), which claims without swapping. Observers must treat
/// the two identically (an accessor parks on either; `migrating_owner`
/// collapses both) — P4b must not distinguish them.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum ReservationState {
    /// Held open without a swap — produced only by the debug probe seam.
    #[allow(dead_code)] // constructed by the (feature-consumed) probe seam
    Claimed { owner: u64 },
    /// The storage swap is in flight (the production path's only state).
    Migrating { owner: u64 },
}

struct Reservation {
    state: ReservationState,
    waiters: Vec<ThreadWakeToken>,
    /// `debug_claim_only` parks the taken `Arc` here so `debug_release` can
    /// restore it; `try_migrate` keeps the `Arc` on its own stack instead.
    held: Option<Arc<Tensor>>,
}

/// The reservation table, keyed by `tensor_id` (a tensor is resident on
/// exactly one device — the P2 recency-table insight). Ordered at level
/// `RESERVATION` (20): acquired under an owning slot (15); released before the
/// lock-free migrate; reacquired (with the slot) for the commit.
static RESERVATIONS: SwitchMutex<BTreeMap<u64, Reservation>> =
    SwitchMutex::new_ordered(BTreeMap::new(), crate::sync::lock_order::RESERVATION);

// --- §9.12 discriminator counters (reset before a measured run) -------------

static CLAIMS_ADMITTED: AtomicU64 = AtomicU64::new(0);
static CLAIMS_CONTENDED: AtomicU64 = AtomicU64::new(0);
static CLAIMS_REFERENCED: AtomicU64 = AtomicU64::new(0);
static MIGRATIONS_COMMITTED: AtomicU64 = AtomicU64::new(0);
static MIGRATIONS_ROLLED_BACK: AtomicU64 = AtomicU64::new(0);
static ACCESSOR_PARKS: AtomicU64 = AtomicU64::new(0);
static ACCESSOR_RETRIES: AtomicU64 = AtomicU64::new(0);
/// #2 quiesce gauge: > 0 only while an accessor is parked. Balanced by the
/// post-wake decrement in `guarded_clone`; a parked thread torn down by force
/// (zombify) instead of woken would leak the gauge positive — acceptable for
/// the one-scenario-per-boot harness, but a P4b daemon teardown path must
/// drain its waiters by waking them, never by killing them parked.
static CURRENTLY_PARKED: AtomicU64 = AtomicU64::new(0);
/// #5: commits whose observed §9.5 delta did not equal the moved bytes —
/// counted only over unconfounded windows (see the epoch bracket in
/// `try_migrate`); the companions below prove the probe still fires.
static DELTA_VIOLATIONS: AtomicU64 = AtomicU64::new(0);
/// Commits whose delta bracket was unconfounded and enforced (BUILD-O1a).
static DELTAS_ENFORCED: AtomicU64 = AtomicU64::new(0);
/// Commits whose bracket was confounded by concurrent accounting mutations
/// (designed concurrency — e.g. an arena free of another slot mid-copy).
static DELTAS_UNMEASURED: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn deltas_enforced() -> u64 {
    DELTAS_ENFORCED.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn deltas_unmeasured() -> u64 {
    DELTAS_UNMEASURED.load(Ordering::Relaxed)
}

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn reset_counters() {
    CLAIMS_ADMITTED.store(0, Ordering::Relaxed);
    CLAIMS_CONTENDED.store(0, Ordering::Relaxed);
    CLAIMS_REFERENCED.store(0, Ordering::Relaxed);
    MIGRATIONS_COMMITTED.store(0, Ordering::Relaxed);
    MIGRATIONS_ROLLED_BACK.store(0, Ordering::Relaxed);
    ACCESSOR_PARKS.store(0, Ordering::Relaxed);
    ACCESSOR_RETRIES.store(0, Ordering::Relaxed);
    DELTA_VIOLATIONS.store(0, Ordering::Relaxed);
    DELTAS_ENFORCED.store(0, Ordering::Relaxed);
    DELTAS_UNMEASURED.store(0, Ordering::Relaxed);
    // CURRENTLY_PARKED is a live gauge, never reset.
}

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn claims_admitted() -> u64 {
    CLAIMS_ADMITTED.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn claims_contended() -> u64 {
    CLAIMS_CONTENDED.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn claims_referenced() -> u64 {
    CLAIMS_REFERENCED.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn migrations_committed() -> u64 {
    MIGRATIONS_COMMITTED.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn migrations_rolled_back() -> u64 {
    MIGRATIONS_ROLLED_BACK.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn accessor_parks() -> u64 {
    ACCESSOR_PARKS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn accessor_retries() -> u64 {
    ACCESSOR_RETRIES.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn currently_parked() -> u64 {
    CURRENTLY_PARKED.load(Ordering::Relaxed)
}
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn delta_violations() -> u64 {
    DELTA_VIOLATIONS.load(Ordering::Relaxed)
}

/// The mover's outcome vocabulary.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MigrateOutcome {
    Migrated,
    /// A reference exists (an accessor holds a clone) — skip, never block.
    SkippedReferenced,
    /// Another mover holds the reservation — skip, never block.
    SkippedContended,
    NotPresent,
    /// Claim succeeded but the migration failed — rolled back: the tensor is
    /// restored to the slot unchanged, the entry removed, waiters woken.
    Failed,
}

/// The owner currently holding `tensor_id`'s reservation, if any (#4).
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn migrating_owner(tensor_id: u64) -> Option<u64> {
    RESERVATIONS
        .lock()
        .get(&tensor_id)
        .map(|reservation| match reservation.state {
            ReservationState::Claimed { owner } | ReservationState::Migrating { owner } => owner,
        })
}

/// Claim the tensor in `slot` and migrate it to `target`.
///
/// Phase 1 (claim, slot + reservation locks): gate-check, take the `Arc` out,
/// leave `Reserved`, insert the entry. Phase 2 (no locks): migrate — the mover
/// owns the only reference. Phase 3 (commit, slot + reservation locks):
/// restore the tensor, remove the entry, take the waiters; wake them after the
/// locks drop. Rollback on failure is the same phase 3 with the unmigrated
/// tensor.
pub fn try_migrate(slot: &TensorSlot, target: Device, owner: u64) -> MigrateOutcome {
    // ── Phase 1: claim. ──
    let mut tensor = {
        let mut state = slot.inner.lock();
        let (tensor_id, strong, weak) = match &*state {
            SlotState::Empty => return MigrateOutcome::NotPresent,
            SlotState::Reserved { .. } => {
                CLAIMS_CONTENDED.fetch_add(1, Ordering::Relaxed);
                return MigrateOutcome::SkippedContended;
            }
            SlotState::Resident(tensor) => (
                tensor.id(),
                Arc::strong_count(tensor),
                Arc::weak_count(tensor),
            ),
        };
        let mut reservations = RESERVATIONS.lock();
        match decide_claim(reservations.contains_key(&tensor_id), strong, weak) {
            ClaimDecision::SkippedContended => {
                CLAIMS_CONTENDED.fetch_add(1, Ordering::Relaxed);
                return MigrateOutcome::SkippedContended;
            }
            ClaimDecision::SkippedReferenced => {
                CLAIMS_REFERENCED.fetch_add(1, Ordering::Relaxed);
                return MigrateOutcome::SkippedReferenced;
            }
            ClaimDecision::Admit => {}
        }
        CLAIMS_ADMITTED.fetch_add(1, Ordering::Relaxed);
        reservations.insert(
            tensor_id,
            Reservation {
                state: ReservationState::Migrating { owner },
                waiters: Vec::new(),
                held: None,
            },
        );
        let SlotState::Resident(tensor) =
            core::mem::replace(&mut *state, SlotState::Reserved { tensor_id })
        else {
            unreachable!("checked Resident above under the held slot lock");
        };
        tensor
        // both guards drop here — the copy runs lock-free
    };

    // ── Phase 2: migrate, owning the only reference. ──
    let tensor_id = tensor.id();
    let from_device = tensor.device();
    let bytes = tensor.byte_len() as u64;
    // The epoch read brackets the total snapshots (BUILD-O1a): if anything
    // beyond this migration's own mutations landed inside the window, the
    // device-total equality is confounded (e.g. the arena freeing ANOTHER
    // slot's device-resident tensor mid-copy — designed concurrency, not a
    // violation) and the strict check is counted unmeasured instead. In a
    // quiescent window — every P4a/P4b gate, and most O1a windows — the
    // probe enforces at full pre-O1a strength.
    let epoch_before = accounting::mutation_epoch();
    let (src_before, dst_before) = delta_snapshot(from_device, target);
    let migrate_result = match Arc::get_mut(&mut tensor) {
        Some(inner) => inner.migrate(target),
        // Unreachable by the lemma (the gate saw strong==1/weak==0 and the
        // Arc left the only container); defense in depth: roll back.
        None => Err(crate::ai::tensor::TensorError::PermissionDenied),
    };
    let outcome = match migrate_result {
        Ok(()) => {
            let (src_after, dst_after) = delta_snapshot(from_device, target);
            let epoch_after = accounting::mutation_epoch();
            // This migration's own accounted mutations: one per accel side
            // (CPU residency is unaccounted — see `delta_snapshot`). A
            // same-device migrate no-ops inside `Tensor::migrate` (zero
            // mutations land), so its window counts as unmeasured — safe,
            // but it inflates `deltas_unmeasured` beyond genuine confounds.
            let own = u64::from(matches!(from_device, Device::Accel(_)))
                + u64::from(matches!(target, Device::Accel(_)));
            if epoch_after.wrapping_sub(epoch_before) == own {
                let src_ok = match (src_before, src_after) {
                    (Some(before), Some(after)) => after == before.saturating_sub(bytes),
                    _ => true,
                };
                let dst_ok = match (dst_before, dst_after) {
                    (Some(before), Some(after)) => after == before.saturating_add(bytes),
                    _ => true,
                };
                if !src_ok || !dst_ok {
                    DELTA_VIOLATIONS.fetch_add(1, Ordering::Relaxed);
                }
                DELTAS_ENFORCED.fetch_add(1, Ordering::Relaxed);
            } else {
                DELTAS_UNMEASURED.fetch_add(1, Ordering::Relaxed);
            }
            MIGRATIONS_COMMITTED.fetch_add(1, Ordering::Relaxed);
            MigrateOutcome::Migrated
        }
        Err(_) => {
            MIGRATIONS_ROLLED_BACK.fetch_add(1, Ordering::Relaxed);
            MigrateOutcome::Failed
        }
    };

    // ── Phase 3: commit (or rollback — identical motion): restore the tensor,
    // end the reservation, wake the waiters after the locks drop. ──
    let waiters = {
        let mut state = slot.inner.lock();
        let mut reservations = RESERVATIONS.lock();
        debug_assert!(
            matches!(&*state, SlotState::Reserved { tensor_id: id } if *id == tensor_id),
            "slot state changed under an active reservation"
        );
        *state = SlotState::Resident(tensor);
        reservations
            .remove(&tensor_id)
            .map(|reservation| {
                // try_migrate keeps its Arc on the stack; a `held` Arc here
                // would mean a debug_claim_only entry was routed through this
                // commit — its tensor would be silently dropped. Refuse.
                debug_assert!(
                    reservation.held.is_none(),
                    "try_migrate commit found a held tensor (probe entry routed through commit)"
                );
                reservation.waiters
            })
            .unwrap_or_default()
    };
    for token in waiters {
        let _ = wake_thread_token(token, 0);
    }
    outcome
}

fn delta_snapshot(from: Device, to: Device) -> (Option<u64>, Option<u64>) {
    let src = match from {
        device @ Device::Accel(_) => Some(accounting::device_resident_total(device)),
        Device::Cpu => None,
    };
    let dst = match to {
        device @ Device::Accel(_) => Some(accounting::device_resident_total(device)),
        Device::Cpu => None,
    };
    (src, dst)
}

/// Mint a new reference to the tensor in `slot`, parking while a mover holds
/// its reservation and re-resolving from scratch on wake (#1 retry counter,
/// #2 gauge). Returns `None` when the slot is empty.
pub fn guarded_clone(slot: &TensorSlot) -> Option<Arc<Tensor>> {
    // NOTE deliberately NOT wrapped in `without_interrupts`: spinning on a
    // contended lock with interrupts disabled deadlocks a CPU whose preempted
    // thread holds that lock (gate run 4 caught exactly this — the accessor
    // spun IRQ-off on the slot while the preempted mover held it on the same
    // CPU, and the tick could never fire). The park itself is atomic without
    // it: `block_current_with_token_and_guard` disables interrupts internally
    // and installs the token while the reservation guard is held — the bare
    // call is the production pattern (virtio_blk, kthread, admission).
    loop {
        // Some(result) = resolved; None = parked-and-woke, re-resolve.
        let resolved = (|| {
            let state = slot.inner.lock();
            let tensor_id = match &*state {
                SlotState::Empty => return Some(None),
                SlotState::Resident(tensor) => return Some(Some(tensor.clone())),
                SlotState::Reserved { tensor_id } => *tensor_id,
            };
            let reservations = RESERVATIONS.lock();
            if !reservations.contains_key(&tensor_id) {
                // The mover committed between our two lock acquisitions; the
                // slot will be Resident on retry.
                return None;
            }
            // K1a-0 F2 witness: a CAP_GRAPH holder must never park (its
            // waiters spin with interrupts off).
            #[cfg(feature = "neo-test")]
            assert!(
                !crate::sched::process::cap_graph_held_here(),
                "K1a-0 F2: a reservation park while this CPU holds CAP_GRAPH"
            );
            ACCESSOR_PARKS.fetch_add(1, Ordering::Relaxed);
            CURRENTLY_PARKED.fetch_add(1, Ordering::Relaxed);
            // Release the slot (outer) so the mover's commit can take it,
            // then park on the reservation guard — the token is installed
            // while the lock is held and released atomically with the
            // context switch: the commit's wake can never be lost.
            drop(state);
            let _ = block_current_with_token_and_guard(reservations, |table, token| {
                table
                    .get_mut(&tensor_id)
                    // Held continuously from contains_key to here.
                    .expect("reservation vanished while its lock was held")
                    .waiters
                    .push(token);
            });
            CURRENTLY_PARKED.fetch_sub(1, Ordering::Relaxed);
            ACCESSOR_RETRIES.fetch_add(1, Ordering::Relaxed);
            None
        })();
        match resolved {
            Some(result) => return result,
            None => {
                crate::sched::yield_now();
                continue;
            }
        }
    }
}

// --- Test seams (#4 double-claim probe) --------------------------------------

/// Claim only — the reservation (and the taken `Arc`) are left in place so a
/// second claim can be probed. Production code uses [`try_migrate`].
#[allow(dead_code)] // P4a probe seam — the double-claim discriminator's test surface
pub fn debug_claim_only(slot: &TensorSlot, owner: u64) -> ClaimDecision {
    let mut state = slot.inner.lock();
    let (tensor_id, strong, weak) = match &*state {
        SlotState::Empty => return ClaimDecision::SkippedReferenced,
        SlotState::Reserved { .. } => {
            CLAIMS_CONTENDED.fetch_add(1, Ordering::Relaxed);
            return ClaimDecision::SkippedContended;
        }
        SlotState::Resident(tensor) => (
            tensor.id(),
            Arc::strong_count(tensor),
            Arc::weak_count(tensor),
        ),
    };
    let mut reservations = RESERVATIONS.lock();
    let decision = decide_claim(reservations.contains_key(&tensor_id), strong, weak);
    match decision {
        ClaimDecision::Admit => {
            CLAIMS_ADMITTED.fetch_add(1, Ordering::Relaxed);
            let SlotState::Resident(tensor) =
                core::mem::replace(&mut *state, SlotState::Reserved { tensor_id })
            else {
                unreachable!("checked Resident above under the held slot lock");
            };
            reservations.insert(
                tensor_id,
                Reservation {
                    state: ReservationState::Claimed { owner },
                    waiters: Vec::new(),
                    held: Some(tensor),
                },
            );
        }
        ClaimDecision::SkippedContended => {
            CLAIMS_CONTENDED.fetch_add(1, Ordering::Relaxed);
        }
        ClaimDecision::SkippedReferenced => {
            CLAIMS_REFERENCED.fetch_add(1, Ordering::Relaxed);
        }
    }
    decision
}

/// Release a `debug_claim_only` reservation: restore the held tensor to the
/// slot, remove the entry, wake the waiters (rollback semantics).
#[allow(dead_code)] // P4a probe seam — the double-claim discriminator's test surface
pub fn debug_release(slot: &TensorSlot, tensor_id: u64) {
    let waiters = {
        let mut state = slot.inner.lock();
        let mut reservations = RESERVATIONS.lock();
        let Some(reservation) = reservations.remove(&tensor_id) else {
            return;
        };
        if let Some(tensor) = reservation.held {
            *state = SlotState::Resident(tensor);
        }
        reservation.waiters
    };
    for token in waiters {
        let _ = wake_thread_token(token, 0);
    }
}

/// Test seam (`i2c_hid_absent`, I-14(a)): run `f` while this thread holds
/// `RESERVATIONS`, so an ISR that wanted it would never get it.
#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn debug_with_reservations_held<R>(f: impl FnOnce() -> R) -> R {
    let _reservations = RESERVATIONS.lock();
    f()
}
