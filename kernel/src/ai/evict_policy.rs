// SPDX-License-Identifier: MPL-2.0
//! Pure LRU eviction policy under device-memory pressure (Phase 7 BUILD-P2).
//!
//! This file has **no kernel dependencies** (only `alloc`) so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and unit-test the selection logic.
//! The kernel glue — snapshotting the executor's device-resident tensors with
//! their §9.5-extension recency ticks, migrating the chosen victims to CPU, and
//! emitting the `eviction:` decision trace — lives in `evict.rs`.
//!
//! ANSPEC-001 §9.10. The policy decides *which resident tensors leave a full
//! device* so a placement (§9.9) can land: LRU among the **evictable** resident
//! tensors (lowest last-use tick first), or — when evicting every evictable
//! tensor still cannot make room — the CPU-fallback signal. It is
//! **result-invariant** (dispatch ISC-4): eviction changes where a tensor
//! lives, never its bytes.
//!
//! **No partial eviction.** The policy precomputes sufficiency: it returns
//! either a victim set that *does* make room, or `NoRoomCpuFallback` with zero
//! victims. Migrating tensors off-device without ending up with room is thrash
//! with no benefit — the one thing the dispatch forbids ("do not loop").

extern crate alloc;

use alloc::vec::Vec;

/// One device-resident tensor, as the kernel glue snapshotted it.
///
/// `evictable = false` folds both protections into one bit at the policy
/// boundary: **pinned** (an operand/output of the op being placed — you cannot
/// evict what you are about to use) and **externally held** (model weights,
/// caller inputs, shared/deduped tensors — another holder keeps the storage
/// alive, so migrating it could not free device memory anyway).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[allow(dead_code)] // host-suite API — the §9.10 reference selector + its I-2 equality matrix (path-mod consumers)
pub struct EvictCandidate {
    pub tensor_id: u64,
    pub bytes: u64,
    /// Last-use tick from the §9.5 recency extension; 0 = never used (coldest).
    pub last_use_tick: u64,
    pub evictable: bool,
}

/// Why the policy returned what it returned — the trace vocabulary.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[allow(dead_code)] // host-suite API — the §9.10 reference selector + its I-2 equality matrix (path-mod consumers)
pub enum EvictReason {
    /// `resident + need ≤ capacity` — no eviction required (the P1 path).
    NoEviction,
    /// The returned victims, evicted coldest-first, make room.
    Lru,
    /// Evicting every evictable tensor still cannot make room — place the op on
    /// the host fallback instead. **No victims are evicted** in this case.
    NoRoomCpuFallback,
}

/// Select the tensors to evict from a device with `capacity` total bytes,
/// `resident` bytes currently resident, to fit `need` more bytes.
///
/// Victims are chosen LRU-first among `evictable` candidates — ascending
/// `last_use_tick`, with `tensor_id` as the deterministic tiebreak — and
/// selection stops as soon as the freed bytes make room. Deterministic: equal
/// inputs always produce the same victim list.
#[allow(dead_code)] // host-suite API — the §9.10 reference selector + its I-2 equality matrix (path-mod consumers)
pub fn select_victims(
    capacity: u64,
    resident: u64,
    need: u64,
    candidates: &[EvictCandidate],
) -> (Vec<u64>, EvictReason) {
    if resident.saturating_add(need) <= capacity {
        return (Vec::new(), EvictReason::NoEviction);
    }

    let mut evictable: Vec<&EvictCandidate> = candidates.iter().filter(|c| c.evictable).collect();
    evictable.sort_by_key(|c| (c.last_use_tick, c.tensor_id));

    let mut victims: Vec<u64> = Vec::new();
    let mut freed: u64 = 0;
    for candidate in evictable {
        if resident.saturating_sub(freed).saturating_add(need) <= capacity {
            break;
        }
        freed = freed.saturating_add(candidate.bytes);
        victims.push(candidate.tensor_id);
    }

    if resident.saturating_sub(freed).saturating_add(need) <= capacity {
        (victims, EvictReason::Lru)
    } else {
        // Even the full evictable set cannot make room: evict nothing, signal
        // the host fallback (the op still runs — never thrash, never fail).
        (Vec::new(), EvictReason::NoRoomCpuFallback)
    }
}

// ─── Phase 7 BUILD-P3: the proactive residency decision (ANSPEC-001 §9.11) ───

/// One device-resident tensor with the BUILD-P3 prediction flags joined on.
///
/// `predicted_hot`: an op within the lookahead window `W` consumes it —
/// evicting it just forces a re-migration next op (net-negative), so it loses
/// eviction priority. `demote_eligible`: dead to the pass — no remaining op
/// consumes it AND it is not a declared model output. The asymmetry is
/// deliberate (failure-cost): retention is an optimization whose failure mode
/// is benign (retain too few → fall through to plain LRU, still correct), so
/// it uses the narrow `W`-window; demotion is a proactive eviction whose
/// failure mode is the thrash this build exists to eliminate, so its
/// eligibility is the conservative full-remaining-scan — the thrash guard,
/// realized structurally.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ResidencyCandidate {
    pub tensor_id: u64,
    pub bytes: u64,
    pub last_use_tick: u64,
    pub evictable: bool,
    pub predicted_hot: bool,
    pub demote_eligible: bool,
}

/// Why the proactive decision returned what it returned — the §9.11 vocabulary.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ResidencyReason {
    /// Room available — no eviction (the §9.9 path).
    NoEviction,
    /// Room was made AND at least one predicted-hot tensor that plain LRU
    /// would have evicted first was retained — a colder victim took its place.
    RetainHot,
    /// Room was made by plain LRU (no retention changed the outcome, or the
    /// retain pass could not make room and the decision fell through to the
    /// §9.10-exact behavior, predicted-hot included).
    Lru,
    /// Nothing evictable can make room — host fallback, zero victims.
    NoRoomCpuFallback,
}

/// The retain-aware eviction decision.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct EvictionDecision {
    /// Tensors to migrate to CPU, coldest-first, precomputed sufficient.
    pub victims: Vec<u64>,
    /// Predicted-hot tensors plain LRU would have evicted before the chosen
    /// victims — `(tensor_id, bytes)`, the `RETAINED` trace lines.
    pub retained: Vec<(u64, u64)>,
    pub reason: ResidencyReason,
}

fn lru_take(
    pool: &mut Vec<&ResidencyCandidate>,
    capacity: u64,
    resident: u64,
    need: u64,
) -> (Vec<u64>, u64) {
    pool.sort_by_key(|c| (c.last_use_tick, c.tensor_id));
    let mut victims = Vec::new();
    let mut freed: u64 = 0;
    for candidate in pool.iter() {
        if resident.saturating_sub(freed).saturating_add(need) <= capacity {
            break;
        }
        freed = freed.saturating_add(candidate.bytes);
        victims.push(candidate.tensor_id);
    }
    (victims, freed)
}

/// Decide which tensors leave a device that needs `need` more bytes, with
/// predicted-hot retention (Phase 7 BUILD-P3, §9.11).
///
/// Phase 1 selects LRU-first among evictable **non-predicted-hot** candidates.
/// If that makes room, any predicted-hot tensor that plain LRU ordering would
/// have taken *before* a chosen victim is reported as retained
/// (`RetainHot`). If retention leaves no way to make room, the decision falls
/// through to the §9.10-exact behavior — LRU among *all* evictable candidates,
/// predicted-hot included (`Lru`, nothing retained) — and only when even that
/// cannot make room does it signal the host fallback with zero victims.
/// With an empty predicted-hot set this is `select_victims` exactly — the
/// strict-superset guarantee (W=0 → P2 verbatim), asserted by the host suite.
pub fn decide_eviction(
    capacity: u64,
    resident: u64,
    need: u64,
    candidates: &[ResidencyCandidate],
) -> EvictionDecision {
    if resident.saturating_add(need) <= capacity {
        return EvictionDecision {
            victims: Vec::new(),
            retained: Vec::new(),
            reason: ResidencyReason::NoEviction,
        };
    }

    // Phase 1: retain predicted-hot — select among cold evictable only.
    let mut cold: Vec<&ResidencyCandidate> = candidates
        .iter()
        .filter(|c| c.evictable && !c.predicted_hot)
        .collect();
    let (victims, freed) = lru_take(&mut cold, capacity, resident, need);
    if resident.saturating_sub(freed).saturating_add(need) <= capacity {
        // Room made. Which hot tensors did retention actually save? Those that
        // plain LRU would have evicted before the warmest chosen victim.
        //
        // Proof sketch (do not "simplify" the bound): every chosen victim is
        // non-hot, and every reported-retained hot tensor is strictly colder
        // (by `(tick, id)`) than the warmest victim. Plain LRU processes
        // strictly coldest-first, so it reaches each such hot tensor BEFORE
        // that victim — and since making room required evicting through that
        // warmer victim, LRU had not yet made room when it reached the hot
        // tensor, hence would have evicted it. Equal ticks cannot flip the
        // comparison: the `tensor_id` component of the tuple key matches
        // `lru_take`'s sort key, so the order is the same total order.
        let warmest_victim = candidates
            .iter()
            .filter(|c| victims.contains(&c.tensor_id))
            .map(|c| (c.last_use_tick, c.tensor_id))
            .max();
        let retained: Vec<(u64, u64)> = match warmest_victim {
            Some(bound) => candidates
                .iter()
                .filter(|c| {
                    c.evictable && c.predicted_hot && (c.last_use_tick, c.tensor_id) < bound
                })
                .map(|c| (c.tensor_id, c.bytes))
                .collect(),
            None => Vec::new(),
        };
        let reason = if retained.is_empty() {
            ResidencyReason::Lru
        } else {
            ResidencyReason::RetainHot
        };
        return EvictionDecision {
            victims,
            retained,
            reason,
        };
    }

    // Phase 2: retention cannot make room — fall through to §9.10 exactly
    // (LRU among ALL evictable, predicted-hot included).
    let mut all: Vec<&ResidencyCandidate> = candidates.iter().filter(|c| c.evictable).collect();
    let (victims, freed) = lru_take(&mut all, capacity, resident, need);
    if resident.saturating_sub(freed).saturating_add(need) <= capacity {
        EvictionDecision {
            victims,
            retained: Vec::new(),
            reason: ResidencyReason::Lru,
        }
    } else {
        EvictionDecision {
            victims: Vec::new(),
            retained: Vec::new(),
            reason: ResidencyReason::NoRoomCpuFallback,
        }
    }
}

/// Proactive cold-demote (Phase 7 BUILD-P3, §9.11): when residency has crossed
/// the headroom threshold (`resident > capacity − headroom`), pick the coldest
/// **demote-eligible** tensor — dead to the pass, never predicted-hot, still
/// passing the §9.10 evictability gate — to migrate to CPU, banking room so
/// the next placement avoids an eviction round-trip. One victim per call
/// (bounded); `None` when below threshold or when nothing is safely demotable
/// (the thrash guard refuses rather than demote something the pass still
/// needs).
pub fn select_demotion(
    capacity: u64,
    resident: u64,
    headroom: u64,
    candidates: &[ResidencyCandidate],
) -> Option<u64> {
    if resident <= capacity.saturating_sub(headroom) {
        return None;
    }
    coldest_eligible(candidates)
}

/// The coldest safely-demotable candidate — the LRU ordering shared by the
/// synchronous demote seam (via [`select_demotion`]) and the residency
/// daemon's victim choice (BUILD-C1: the daemon's former ad-hoc min-scan is
/// this function).
pub fn coldest_eligible(candidates: &[ResidencyCandidate]) -> Option<u64> {
    candidates
        .iter()
        .filter(|c| c.evictable && c.demote_eligible && !c.predicted_hot)
        .min_by_key(|c| (c.last_use_tick, c.tensor_id))
        .map(|c| c.tensor_id)
}
