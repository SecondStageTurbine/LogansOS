// SPDX-License-Identifier: MPL-2.0
//! OT-95/OT-96: bounded egress passes and table rounds, as pure logic
//! (`net-tick`, which `wire-rx` requires). The net tick, the loopback pump
//! and the receive pass each do work bounded by a budget per call, whatever
//! the backlog or what other CPUs queue meanwhile (Chad #173 F2, #175 item 4,
//! #189). Kernel-free, so `tools/net-rx-logic-tests` measures each call's
//! work on the host.

extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

/// What one [`egress_pass`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EgressPass {
    /// Segments taken off the front (at most the budget).
    pub taken: usize,
    /// Of those, sent (the rest went back to the front).
    pub sent: usize,
    /// Whether segments it didn't take remain.
    pub more: bool,
}

/// OT-96 F2 / #175 item 4, the whole call bounded (Chad #189): one egress
/// pass over the shared queue, which `locked` lends under its lock. Under
/// the lock it pops at most `budget` segments off the front; outside it,
/// sends each unless `keep` says it stays queued; then, under the lock
/// again, pushes the kept ones back onto the front, in order. Nothing else
/// in the queue is touched: no snapshot, no rebuild, nothing proportional to
/// the backlog or to what other CPUs queued meanwhile. The put-back goes in
/// whatever the queue's length then: other CPUs may have filled it to the
/// admission threshold meanwhile (`push_capped`), so the queue can pass the
/// threshold by what passes have out (Chad #241 1c).
pub fn egress_pass<T>(
    mut locked: impl FnMut(&mut dyn FnMut(&mut VecDeque<T>)),
    budget: usize,
    mut keep: impl FnMut(&T) -> bool,
    mut send: impl FnMut(T),
) -> EgressPass {
    let mut batch = Vec::with_capacity(budget);
    let mut more = false;
    locked(&mut |queue| {
        while batch.len() < budget {
            match queue.pop_front() {
                Some(item) => batch.push(item),
                None => break,
            }
        }
        more = !queue.is_empty();
    });
    let taken = batch.len();
    let mut kept = Vec::with_capacity(taken);
    let mut sent = 0;
    for item in batch {
        if keep(&item) {
            kept.push(item);
        } else {
            send(item);
            sent += 1;
        }
    }
    if !kept.is_empty() {
        let mut kept = Some(kept);
        locked(&mut |queue| {
            if let Some(kept) = kept.take() {
                for item in kept.into_iter().rev() {
                    queue.push_front(item);
                }
            }
        });
        more = true;
    }
    EgressPass { taken, sent, more }
}

/// OT-96 / #175 item 4: queue `item` unless the queue already holds `cap`;
/// `false` when it was refused (the caller drops it and counts it). An
/// admission threshold: [`egress_pass`]'s put-back doesn't check it.
pub fn push_capped<T>(queue: &mut VecDeque<T>, cap: usize, item: T) -> bool {
    if queue.len() >= cap {
        return false;
    }
    queue.push_back(item);
    true
}

/// #175 item 4: `f` over at most `budget` entries of `map`, the ones after
/// `cursor`; the cursor moves to the last one visited, or back to the start
/// once a round reaches the end. A sweep of the whole table takes
/// `len / budget` rounds, rounded up, and visits each entry once.
pub fn round<K: Ord + Copy, V, R>(
    map: &BTreeMap<K, V>,
    cursor: &mut Option<K>,
    budget: usize,
    mut f: impl FnMut(&K, &V) -> R,
) -> Vec<R> {
    use core::ops::Bound::{Excluded, Unbounded};
    let start = match *cursor {
        Some(key) => (Excluded(key), Unbounded),
        None => (Unbounded, Unbounded),
    };
    let mut out = Vec::new();
    let mut last = None;
    for (key, value) in map.range(start).take(budget) {
        out.push(f(key, value));
        last = Some(*key);
    }
    *cursor = if out.len() < budget { None } else { last };
    out
}
