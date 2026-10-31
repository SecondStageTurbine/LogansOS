// SPDX-License-Identifier: MPL-2.0
//! K1a-0 (CP-88): re-parenting the children of caps whose edges are going.
//!
//! When a close or a process teardown removes caps, every remaining edge that
//! names one of them is pointed at its nearest ancestor that is not also
//! going, so the remaining caps keep a path to every ancestor a revoke may
//! start from. Pure and generic over the cap id, so the host KAT crate
//! (`tools/cap-graph-logic-tests`) mounts it via `#[path]` and checks it
//! against a model graph.

use alloc::collections::BTreeMap;

/// The nearest ancestor of `p` that is not in `dying` (`cap -> its parent`),
/// or `None` if the chain ends at a root inside `dying`. Bounded: each step
/// leaves one dying cap behind, so a cycle (never produced by the kernel)
/// ends the walk instead of looping.
pub fn resolve<K: Ord + Copy>(dying: &BTreeMap<K, Option<K>>, mut p: K) -> Option<K> {
    for _ in 0..=dying.len() {
        match dying.get(&p) {
            Some(Some(up)) => p = *up,
            Some(None) => return None,
            None => return Some(p),
        }
    }
    None
}

/// Re-point one edge whose parent is dying. Returns the new parent when the
/// edge changed.
pub fn reparent<K: Ord + Copy>(
    parent: &mut Option<K>,
    dying: &BTreeMap<K, Option<K>>,
) -> Option<Option<K>> {
    let p = (*parent)?;
    if !dying.contains_key(&p) {
        return None;
    }
    *parent = resolve(dying, p);
    Some(*parent)
}
