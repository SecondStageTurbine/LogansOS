// SPDX-License-Identifier: MPL-2.0
//! Eviction + proactive-demotion glue (ANSPEC-001 §9.10–§9.11; built P2→C1).
//!
//! When residency-aware placement (§9.9) targets a capacity-bounded accelerator
//! that is out of room, the executor consults this module *before* allocating on
//! the device: it snapshots the pass's device-resident tensors with their §9.5
//! recency ticks, runs the pure LRU policy ([`evict_policy`], host-testable),
//! **migrates each victim back to CPU** through the existing migration mechanism
//! (`Tensor::migrate` — no parallel residency path), and emits the `eviction:`
//! decision trace. If nothing evictable can make room, it signals the host
//! fallback (`no-room-cpu-fallback`) — the op still runs, never thrashes, never
//! fails. ANSPEC-001 §9.10.
//!
//! **Evictability is read, not enforced.** A tensor is evictable iff the pass's
//! value table holds the *only* `Arc` to it (`Arc::strong_count == 1`) and it is
//! not pinned (an operand of the op being placed). Operands are automatically
//! protected — the executing node holds live clones — and externally-held
//! tensors (model weights, caller inputs, shared/deduped tensors) can never be
//! victims: another holder keeps the storage alive, so migrating them could not
//! free device memory anyway. `Arc::get_mut` is the final mutation gate (it also
//! refuses if any `Weak` exists; since BUILD-O2 the dedup registry holds
//! SlotKeys, not Weaks — candidacy no longer pins); a victim it refuses
//! is skipped and the post-eviction room re-check decides the outcome.
//!
//! Result-invariant (dispatch ISC-4): eviction changes where a tensor lives,
//! never its bytes — migration copies the device storage to host byte-for-byte.

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::ai::tensor::{Device, Tensor};
use crate::drivers::accel::{AccelDriver, AccelMemoryKind, accounting};
use crate::serial_println;

#[path = "evict_policy.rs"]
mod evict_policy;
pub use evict_policy::{
    ResidencyCandidate, ResidencyReason, coldest_eligible, decide_eviction, select_demotion,
};

/// Join a live tensor with its §9.5 recency tick into a policy candidate —
/// the ONE construction site (BUILD-C1), shared by the executor's value-table
/// snapshot below and the residency daemon's managed-slot walk. `evictable`
/// is the caller's judgment: the executor computes it from the refcount gate
/// + pin set; the daemon passes `true` and lets the reservation claim (the
///   authoritative re-check, under the locks) decide.
pub fn candidate_from(
    tensor: &Tensor,
    device: Device,
    evictable: bool,
    predicted_hot: bool,
    demote_eligible: bool,
) -> ResidencyCandidate {
    ResidencyCandidate {
        tensor_id: tensor.id(),
        bytes: tensor.byte_len() as u64,
        // A missing recency entry would be a tracking gap; defaulting to 0
        // (coldest) fails toward eviction, which is byte-preserving — benign.
        last_use_tick: accounting::recency_tick(device, tensor.id()).unwrap_or(0),
        evictable,
        predicted_hot,
        demote_eligible,
    }
}

/// Eviction accounting for the gate (dispatch ISC-5): victims migrated to CPU,
/// their bytes, and no-room CPU fallbacks, across a measured forward pass.
/// Reset before a measured run; read after (mirrors the migration counters).
/// BUILD-P3 adds the proactive counters: predicted-hot tensors retained, and
/// cold tensors proactively demoted to bank headroom.
static EVICTIONS: AtomicU64 = AtomicU64::new(0);
static EVICTED_BYTES: AtomicU64 = AtomicU64::new(0);
static NO_ROOM_FALLBACKS: AtomicU64 = AtomicU64::new(0);
static RETAINED: AtomicU64 = AtomicU64::new(0);
static DEMOTIONS: AtomicU64 = AtomicU64::new(0);

/// Reset the eviction + proactive-residency counters before a measured pass.
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn reset_counters() {
    EVICTIONS.store(0, Ordering::Relaxed);
    EVICTED_BYTES.store(0, Ordering::Relaxed);
    NO_ROOM_FALLBACKS.store(0, Ordering::Relaxed);
    RETAINED.store(0, Ordering::Relaxed);
    DEMOTIONS.store(0, Ordering::Relaxed);
}

/// Predicted-hot tensors retained (eviction priority refused) in the last pass.
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn retained_count() -> u64 {
    RETAINED.load(Ordering::Relaxed)
}

/// Cold tensors proactively demoted to bank headroom in the last pass.
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn demotion_count() -> u64 {
    DEMOTIONS.load(Ordering::Relaxed)
}

/// Victims migrated to CPU during the last measured pass.
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn eviction_count() -> u64 {
    EVICTIONS.load(Ordering::Relaxed)
}

/// Victim bytes migrated to CPU during the last measured pass.
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn evicted_bytes() -> u64 {
    EVICTED_BYTES.load(Ordering::Relaxed)
}

/// Placements rerouted to the host fallback because nothing evictable could
/// make room, during the last measured pass.
#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn no_room_fallback_count() -> u64 {
    NO_ROOM_FALLBACKS.load(Ordering::Relaxed)
}

/// What the room consult decided for the placement.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RoomOutcome {
    /// The device has room (possibly after evictions) — proceed with placement.
    Ready,
    /// Nothing evictable makes room — place the op on the host fallback.
    CpuFallback,
}

/// Bytes the placement will take from the device when this op dispatches there:
/// the resident output tensor, plus — for arena-backed devices (accel_sim) —
/// the transient dispatch staging the driver allocates from the *same* arena
/// (every input plus the output staging copy). A room check that ignored the
/// staging would pass and then OOM mid-dispatch.
pub fn room_needed(memory: AccelMemoryKind, operand_bytes_total: u64, output_bytes: u64) -> u64 {
    match memory {
        AccelMemoryKind::DeviceArena => output_bytes
            .saturating_add(operand_bytes_total)
            .saturating_add(output_bytes),
        AccelMemoryKind::HostRam | AccelMemoryKind::DmaBuffer => output_bytes,
    }
}

/// Snapshot the pass's tensors resident on `device`, with §9.5 recency and the
/// BUILD-P3 prediction flags joined on (`hot`/`demote_ok` are sets of *value
/// indices* — the executor's slot numbers — computed structurally from the
/// remaining op graph by `ai::predictor`).
///
/// Evictable requires `weak_count == 0` as well as `strong_count == 1`:
/// `Arc::get_mut` refuses when any `Weak` exists (defensive since BUILD-O2:
/// the §9.7 dedup registry stopped holding Weaks — idle deduped weights ARE
/// claimable now; the screen keeps any future Weak-holder honest), and a
/// victim selected then refused would break the
/// no-partial-eviction rule — earlier victims already migrated, placement
/// falling back anyway. Screening here keeps the selected set all-or-nothing.
///
/// Coherence note: the snapshot→select→migrate sequence is non-atomic across
/// the accounting/recency locks, which is sound because accelerator requests
/// run to completion one-per-device (temporal isolation — see
/// `accounting.rs` INFLIGHT) and this executor pass is the only mutator of
/// its value table. **BUILD-P3 (Path A) added the proactive retain/cold-demote
/// decisions WITHOUT relaxing this single-mutator property.**
///
/// **BUILD-P4a (§9.12) relaxed it for the daemon path and re-proved it:** a
/// concurrent mover may now migrate a tensor via `ai::reservation` — safely,
/// because a *referenced* tensor is never claimable (the same
/// `strong == 1 && weak == 0` gate this snapshot uses), an arriving accessor
/// parks on the reservation and re-resolves on wake, and the storage swap
/// commits with the §9.5 updates in one critical section under the documented
/// lock order (`sync::lock_order`). **BUILD-O1a (ADR-042) re-proved the gate
/// as I-4 unique-strong-owner-at-rest: the slot arena holds the only resting
/// `Arc` and every use mints-and-drops through `guarded_clone`, so the same
/// predicate now states "no use in flight" for arena-owned tensors (the
/// mint-vs-claim-vs-free stress gate).** This synchronous path is unchanged: with
/// no concurrent mover the reservation table is empty and behavior is P3
/// bit-for-bit. The alloc/dispatch OOM backstops in `exec.rs` remain the
/// safety net for what no model can see.
fn snapshot_candidates(
    values: &[Option<Arc<Tensor>>],
    device: Device,
    pinned: &[u64],
    hot: &BTreeSet<u32>,
    demote_ok: &BTreeSet<u32>,
) -> (Vec<ResidencyCandidate>, BTreeMap<u64, usize>) {
    let mut candidates: Vec<ResidencyCandidate> = Vec::new();
    let mut value_index_of: BTreeMap<u64, usize> = BTreeMap::new();
    for (index, slot) in values.iter().enumerate() {
        let Some(tensor) = slot.as_ref() else {
            continue;
        };
        if tensor.device() != device {
            continue;
        }
        let id = tensor.id();
        if value_index_of.insert(id, index).is_some() {
            // The same physical tensor at a second value index: never evictable
            // (strong_count ≥ 2 by construction) and already snapshotted —
            // a duplicate candidate would double-count its bytes toward `freed`.
            // NOTE this also means only the FIRST slot's prediction flags are
            // recorded; that is safe precisely BECAUSE the strong-count makes
            // the candidate non-evictable/non-demotable regardless of flags.
            // If aliased tensors ever become evictable, this dedup must merge
            // flags (hot-if-any-slot-hot) instead of dropping them.
            continue;
        }
        let evictable =
            Arc::strong_count(tensor) == 1 && Arc::weak_count(tensor) == 0 && !pinned.contains(&id);
        let value_index = u32::try_from(index).unwrap_or(u32::MAX);
        candidates.push(candidate_from(
            tensor,
            device,
            evictable,
            hot.contains(&value_index),
            demote_ok.contains(&value_index),
        ));
    }
    (candidates, value_index_of)
}

/// Migrate one victim to CPU through the existing mechanism. Returns the bytes
/// freed, or `None` if the victim could not be acquired/migrated (unreachable
/// for a screened candidate; kept as defense in depth — the caller aborts its
/// round rather than continue toward a placement that may now fall back).
fn migrate_victim(values: &mut [Option<Arc<Tensor>>], index: usize) -> Option<u64> {
    let slot = values.get_mut(index)?.as_mut()?;
    let tensor = Arc::get_mut(slot)?;
    let bytes = tensor.byte_len() as u64;
    if tensor.migrate(Device::Cpu).is_err() {
        return None;
    }
    Some(bytes)
}

/// Make room on `driver`'s device for `need` bytes, evicting LRU unpinned
/// tensors from the pass's value table if necessary — retaining predicted-hot
/// tensors (BUILD-P3, §9.11) when a colder victim can make room instead.
/// `pinned` is the current op's operand tensor ids (pin protection — never
/// evict what you're about to use; the output does not exist yet); `hot` is
/// the predicted-hot value-index set from the structural lookahead (empty at
/// `W = 0` → the §9.10 reactive decision, bit for bit). Returns whether
/// placement can proceed on the device or must fall back to the host.
///
/// Devices without a declared capacity never evict (`Ready` immediately): the
/// §9.9 path, bit for bit.
pub fn ensure_room(
    values: &mut [Option<Arc<Tensor>>],
    driver: &dyn AccelDriver,
    pinned: &[u64],
    need: u64,
    hot: &BTreeSet<u32>,
) -> RoomOutcome {
    let Some(capacity) = driver.capacity_bytes() else {
        return RoomOutcome::Ready;
    };
    let device = driver.descriptor().device;
    let resident = accounting::device_resident_total(device);
    if resident.saturating_add(need) <= capacity {
        return RoomOutcome::Ready;
    }

    let empty = BTreeSet::new();
    let (candidates, value_index_of) = snapshot_candidates(values, device, pinned, hot, &empty);

    let decision = decide_eviction(capacity, resident, need, &candidates);
    match decision.reason {
        ResidencyReason::NoEviction => RoomOutcome::Ready,
        ResidencyReason::NoRoomCpuFallback => {
            trace_no_room(device, resident, capacity, need, &candidates);
            NO_ROOM_FALLBACKS.fetch_add(1, Ordering::Relaxed);
            RoomOutcome::CpuFallback
        }
        ResidencyReason::RetainHot | ResidencyReason::Lru => {
            // The §9.11 governance line: predicted-hot tensors plain LRU would
            // have evicted first, kept resident because reuse is imminent.
            for (id, bytes) in &decision.retained {
                RETAINED.fetch_add(1, Ordering::Relaxed);
                serial_println!(
                    "residency: tensor={} device={:?} RETAINED reason=reuse-soon (next-use<=W, would-evict={})",
                    id,
                    device,
                    bytes,
                );
                crate::log::flush();
            }
            for victim_id in decision.victims {
                let Some(&index) = value_index_of.get(&victim_id) else {
                    break;
                };
                let resident_before = accounting::device_resident_total(device);
                // Any refusal or migration failure ABORTS the round —
                // continuing would evict further tensors toward a placement
                // that may now fall back (partial-eviction thrash).
                let Some(bytes) = migrate_victim(values, index) else {
                    break;
                };
                EVICTIONS.fetch_add(1, Ordering::Relaxed);
                EVICTED_BYTES.fetch_add(bytes, Ordering::Relaxed);
                serial_println!(
                    "eviction: tensor={} device={:?} → cpu reason=lru (resident={}/{}, need={})",
                    victim_id,
                    device,
                    resident_before,
                    capacity,
                    need,
                );
                crate::log::flush();
            }
            // The victim set was precomputed as sufficient; re-check in case a
            // victim was refused (Weak) or its migration failed.
            if accounting::device_resident_total(device).saturating_add(need) <= capacity {
                RoomOutcome::Ready
            } else {
                trace_no_room(
                    device,
                    accounting::device_resident_total(device),
                    capacity,
                    need,
                    &candidates,
                );
                NO_ROOM_FALLBACKS.fetch_add(1, Ordering::Relaxed);
                RoomOutcome::CpuFallback
            }
        }
    }
}

/// Proactive cold-demote (BUILD-P3, §9.11): after a placement settles, if the
/// device's residency has crossed the headroom threshold (`resident > C − H`),
/// migrate the coldest **dead-to-the-pass** tensor to CPU — banking room so
/// the *next* placement avoids an eviction round-trip. At most one demotion
/// per seam (bounded); never a predicted-hot tensor, never a tensor the pass
/// still uses, never a declared model output (the thrash guard: a proactive
/// move the graph says will be undone is refused, not weighed). Runs only
/// between ops — headroom banks for a next placement; after the last op none
/// exists. Synchronous, same pass, same single-mutator discipline.
/// Run the demote seam against every capacity-bounded registered device.
/// Devices without a declared capacity are skipped (nothing to bank against),
/// so the sweep is a no-op on capacity-free systems. The headroom target is
/// the **per-device** noted max placement-need (BUILD-P4b, seam 3 — the same
/// `H_dev` the daemon's watermarks use; value-identical to P3's pass-global
/// figure on single-device workloads since the consult notes need before any
/// seam runs).
pub fn bank_headroom_all(
    values: &mut [Option<Arc<Tensor>>],
    hot: &BTreeSet<u32>,
    demote_ok: &BTreeSet<u32>,
) {
    for index in 0..crate::drivers::accel::device_count() {
        let Some(driver) = crate::drivers::accel::get_device(index) else {
            continue;
        };
        if driver.capacity_bytes().is_some() {
            let headroom = crate::ai::residency_daemon::need_hint(driver.descriptor().device);
            if headroom != 0 {
                bank_headroom(values, driver.as_ref(), hot, demote_ok, headroom);
            }
        }
    }
}

pub fn bank_headroom(
    values: &mut [Option<Arc<Tensor>>],
    driver: &dyn AccelDriver,
    hot: &BTreeSet<u32>,
    demote_ok: &BTreeSet<u32>,
    headroom: u64,
) {
    let Some(capacity) = driver.capacity_bytes() else {
        return;
    };
    if headroom == 0 {
        return;
    }
    let device = driver.descriptor().device;
    let resident = accounting::device_resident_total(device);
    let (candidates, value_index_of) = snapshot_candidates(values, device, &[], hot, demote_ok);
    let Some(victim_id) = select_demotion(capacity, resident, headroom, &candidates) else {
        return;
    };
    let Some(&index) = value_index_of.get(&victim_id) else {
        return;
    };
    if migrate_victim(values, index).is_none() {
        return;
    }
    DEMOTIONS.fetch_add(1, Ordering::Relaxed);
    serial_println!(
        "residency: tensor={} device={:?} → cpu reason=cold-demote (resident={}/{}, headroom-target={})",
        victim_id,
        device,
        resident,
        capacity,
        headroom,
    );
    crate::log::flush();
}

fn trace_no_room(
    device: Device,
    resident: u64,
    capacity: u64,
    need: u64,
    candidates: &[ResidencyCandidate],
) {
    let evictable: u64 = candidates
        .iter()
        .filter(|c| c.evictable)
        .map(|c| c.bytes)
        .sum();
    serial_println!(
        "eviction: device={:?} reason=no-room-cpu-fallback (resident={}/{}, need={}, evictable={})",
        device,
        resident,
        capacity,
        need,
        evictable,
    );
    crate::log::flush();
}
