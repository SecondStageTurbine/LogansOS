// SPDX-License-Identifier: MPL-2.0
//! Pure claim-decision logic for the migration reservation (Phase 7 BUILD-P4a).
//!
//! This file has **no kernel dependencies** so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and unit-test the claim truth
//! table. The kernel glue — the reservation table, the park/wake dance, the
//! atomic migrate+accounting commit — lives in `reservation.rs`.
//!
//! ANSPEC-001 §9.12. The claim gate is the exact §9.10/§9.11 evictability
//! predicate: a tensor is claimable only when **nothing else can reach it** —
//! the owning container holds the only strong reference and no `Weak` exists
//! (a `Weak::upgrade` could otherwise mint a reference without the container
//! lock). A contended claim is **skipped, never blocked** (the Linux
//! `MIGRATE_SYNC_LIGHT` bail-don't-block lesson): the mover finds other work;
//! only an *accessor* ever parks.

/// Why a claim was admitted or refused.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ClaimDecision {
    /// Unreferenced and unreserved — the mover may claim and migrate.
    Admit,
    /// Another mover holds the reservation — skip (never block).
    SkippedContended,
    /// A reference exists (strong > 1 or any weak) — migrating could race a
    /// live reader or a `Weak::upgrade`; skip.
    SkippedReferenced,
}

/// The claim truth table. `already_reserved` is the reservation-table lookup;
/// `strong`/`weak` are the `Arc` counts read under the owning container's lock.
pub fn decide_claim(already_reserved: bool, strong: usize, weak: usize) -> ClaimDecision {
    if already_reserved {
        // Checked FIRST: a reserved tensor's counts belong to the owner.
        return ClaimDecision::SkippedContended;
    }
    if strong != 1 || weak != 0 {
        return ClaimDecision::SkippedReferenced;
    }
    ClaimDecision::Admit
}
