// SPDX-License-Identifier: MPL-2.0
//! Pure tensor-sharing/ownership policy (Phase 7 BUILD-3).
//!
//! This file has **no kernel dependencies** (it is `#![no_std]`-clean and uses no
//! `alloc`) so the host suite (`tools/ai-ops-tests`) can path-`mod` it and unit-test
//! the load-bearing accounting rule. The kernel glue — the registries, the dedup
//! content-hash table, the cross-process reader scan, the §9.5 residency calls, and
//! the ownership trace — lives in `sharing.rs`.
//!
//! The governance fact (ANSPEC-001 §9.7): N principals can reference **one physical
//! copy** of an immutable tensor (explicit RO capability grant, or implicit
//! content-hash dedup). The single non-trivial *policy* decision is the residency
//! attribution rule (ISC-4): a shared tensor's bytes are physical reality counted
//! **once** (to the owning/first principal); every other principal is a *reader*,
//! contributing **zero** additional resident bytes. This is what keeps §9.5's
//! `resident_bytes` honest under sharing — no inflation from N references.

/// The accounted facts about one shared physical tensor: who owns it (the first
/// principal, who carries its residency) and its physical byte size (counted once).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SharedRecord {
    /// `ProcessId.0` of the owning/first principal — the one the physical bytes are
    /// attributed to in §9.5.
    pub owner: u32,
    /// Physical byte size of the one copy. Counted exactly once, regardless of how
    /// many principals reference it.
    pub bytes: u64,
}

/// Resident bytes attributed to `principal` for this shared tensor: the full physical
/// size if it is the owner, otherwise **zero** (readers add no residency). This is the
/// no-double-count rule of ISC-4.
pub const fn attributed_resident_bytes(record: &SharedRecord, principal: u32) -> u64 {
    if principal == record.owner {
        record.bytes
    } else {
        0
    }
}

/// Total resident bytes a shared tensor contributes across an arbitrary set of
/// referencing principals: exactly `bytes` once (the owner's), no matter how many
/// readers (including zero, or the owner appearing once). The invariant the gate
/// asserts — sharing must never inflate total residency beyond the physical copy.
#[allow(dead_code)] // host-suite API — §9.7 count-once accounting truth table
pub fn total_resident_bytes(record: &SharedRecord, principals: &[u32]) -> u64 {
    let mut total = 0u64;
    let mut i = 0;
    while i < principals.len() {
        total = total.saturating_add(attributed_resident_bytes(record, principals[i]));
        i += 1;
    }
    total
}
