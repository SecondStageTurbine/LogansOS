// SPDX-License-Identifier: MPL-2.0
//! Pure structural reuse predictor (Phase 7 BUILD-P3) — the op-graph lookahead.
//!
//! This file has **no kernel dependencies** (only `alloc`) so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and unit-test the prediction logic.
//!
//! ANSPEC-001 §9.11. The signal is **structural, not statistical**: from the op
//! sequence the executor is already running — specifically the *remaining* ops'
//! operand value-indices — a value is **predicted-hot** iff an op within the
//! lookahead window `W` consumes it ("needed by the next op" at `W = 1`), and a
//! value has a **future use** iff *any* remaining op consumes it. No learned
//! model, no access-counter sampling: the dependency graph the pass holds is
//! the entire input. `W = 0` yields the empty hot set — the §9.10 reactive
//! path, bit for bit.
//!
//! Both sets are over **value indices** (the executor's value-table slots), not
//! tensor ids: a value index whose tensor has not been produced yet simply has
//! no tensor to mark — the executor resolves indices against its table, so a
//! not-yet-produced operand can never pin or protect anything.

extern crate alloc;

use alloc::collections::BTreeSet;

/// Value indices consumed by an op within the next `w` remaining ops — the
/// predicted-hot set. `remaining_op_inputs[k]` is the operand value-index list
/// of the k-th upcoming op (the op after the one being placed is index 0).
pub fn hot_values(remaining_op_inputs: &[&[u32]], w: usize) -> BTreeSet<u32> {
    let mut hot = BTreeSet::new();
    for inputs in remaining_op_inputs.iter().take(w) {
        for value in *inputs {
            hot.insert(*value);
        }
    }
    hot
}

/// Value indices consumed by *any* remaining op — the future-use set. A
/// device-resident value absent from this set (and not a declared model
/// output) is dead to the pass: demoting it can never force a re-migration,
/// which is what makes it the only safe proactive-demotion candidate (the
/// thrash guard, realized structurally).
pub fn future_use_values(remaining_op_inputs: &[&[u32]]) -> BTreeSet<u32> {
    let mut used = BTreeSet::new();
    for inputs in remaining_op_inputs {
        for value in *inputs {
            used.insert(*value);
        }
    }
    used
}
