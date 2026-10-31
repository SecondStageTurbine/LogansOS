// SPDX-License-Identifier: MPL-2.0
//! Pure residency-aware op-placement policy (Phase 7 BUILD-P1).
//!
//! This file has **no kernel dependencies** (it is `#![no_std]`-clean and uses no
//! `alloc`) so the host suite (`tools/ai-ops-tests`) can path-`mod` it and
//! unit-test the decision logic. The kernel glue — snapshotting per-device operand
//! residency, the AccelDriver registry (`supported_ops`) and the §9.5 accounting
//! table, then dispatching to the chosen device and emitting the decision trace —
//! lives in `placement.rs`.
//!
//! ANSPEC-001 §9.9. The policy chooses *where an op runs* from inputs only the
//! kernel sees globally: per-device operand residency + op coverage + the
//! principal's §9.5/§9.6 quota. It is **result-invariant** (ISC-4): it changes the
//! run device, never the bytes computed. Its first rule attacks migration thrash —
//! prefer the device already holding the most operand bytes, so a forward pass does
//! not bounce tensors device↔host on every op.

/// Why the policy chose a device — the cross-cutting reasoning the application did
/// not request (the governance line). Ordered by the §1 rules of the dispatch.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PlacementReason {
    /// All operand bytes already reside on the chosen accelerator (zero migration).
    OperandsResident,
    /// Operands split across devices; chose the one holding the most bytes
    /// (migration minimized), with a deterministic lowest-id tiebreak.
    MinMigration,
    /// No accelerator covers the op, or operands are host-resident with no
    /// accelerator benefit → run on the host fallback (accel_cpu).
    CpuFallback,
    /// An accelerator held the operands but the principal is over its quota there
    /// (§9.5/§9.6) → spilled to the host fallback.
    OverQuota,
}

/// One device the op could run on, as the kernel glue snapshotted it.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DeviceCandidate {
    /// `Device::as_abi()` — kept as a raw `u32` to keep this file kernel-free.
    pub device_abi: u32,
    /// Operand bytes already resident on this device (host-class residency folds
    /// `Device::Cpu` into the accel_cpu candidate — see `placement.rs`).
    pub resident_bytes: u64,
    /// This device's driver covers this op (its `supported_ops`).
    pub covers_op: bool,
    /// The principal is over its §9.5/§9.6 dispatch quota on this device.
    pub over_quota: bool,
    /// The universal host-memory fallback (accel_cpu): covers the full op set and
    /// is always selectable (the safety floor), regardless of quota.
    pub is_cpu_fallback: bool,
}

/// The decision: the device to run on, the reasoning, and the operand bytes the
/// plan still has to stage to that device (`0` ⇒ fully resident, migration avoided).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Placement {
    pub device_abi: u32,
    pub reason: PlacementReason,
    pub migrated_bytes: u64,
}

/// Decide where an op runs.
///
/// `total_operand_bytes` is the sum of every operand's bytes (across all devices);
/// each candidate's `resident_bytes` is the subset already on it. Deterministic:
/// among *selectable* candidates the one with the most resident bytes wins, and the
/// lowest `device_abi` breaks ties.
///
/// **Selectable** = `covers_op` AND (`!over_quota` OR `is_cpu_fallback`). The host
/// fallback is always present and always selectable, so the policy never returns
/// "nowhere to run." It is a strict improvement over fixed dispatch: when no
/// accelerator holds operands, the max-bytes winner is the host fallback itself, so
/// host-resident work stays on the host (today's behavior) rather than thrashing to
/// an accelerator that holds nothing.
pub fn decide(candidates: &[DeviceCandidate], total_operand_bytes: u64) -> Placement {
    // Best selectable candidate: max resident bytes, lowest abi tiebreak.
    let mut best: Option<&DeviceCandidate> = None;
    for c in candidates {
        let selectable = c.covers_op && (!c.over_quota || c.is_cpu_fallback);
        if !selectable {
            continue;
        }
        best = Some(match best {
            None => c,
            Some(b) if c.resident_bytes > b.resident_bytes => c,
            Some(b) if c.resident_bytes == b.resident_bytes && c.device_abi < b.device_abi => c,
            Some(b) => b,
        });
    }

    // Was there a real accelerator that covers the op and actually holds operands,
    // but is over quota? This is what distinguishes `OverQuota` (we *would* have
    // placed there) from a plain `CpuFallback` (nothing to place there anyway).
    let blocked_by_quota = candidates
        .iter()
        .any(|c| !c.is_cpu_fallback && c.covers_op && c.over_quota && c.resident_bytes > 0);

    match best {
        Some(chosen) => {
            let migrated = total_operand_bytes.saturating_sub(chosen.resident_bytes);
            let reason = if chosen.is_cpu_fallback {
                if blocked_by_quota {
                    PlacementReason::OverQuota
                } else {
                    PlacementReason::CpuFallback
                }
            } else if migrated == 0 && chosen.resident_bytes > 0 {
                PlacementReason::OperandsResident
            } else {
                PlacementReason::MinMigration
            };
            Placement {
                device_abi: chosen.device_abi,
                reason,
                migrated_bytes: migrated,
            }
        }
        None => {
            // Only reachable if the glue supplied no fallback (it never does);
            // degenerate safety so the policy is total.
            let fallback = candidates.iter().find(|c| c.is_cpu_fallback);
            let dev = fallback.map(|f| f.device_abi).unwrap_or(0);
            let resident = fallback.map(|f| f.resident_bytes).unwrap_or(0);
            Placement {
                device_abi: dev,
                reason: PlacementReason::CpuFallback,
                migrated_bytes: total_operand_bytes.saturating_sub(resident),
            }
        }
    }
}
