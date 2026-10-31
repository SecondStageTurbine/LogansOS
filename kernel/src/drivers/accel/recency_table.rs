// SPDX-License-Identifier: MPL-2.0
//! Pure per-tensor last-use recency table (Phase 7 BUILD-P2) — the one new
//! input LRU eviction needs.
//!
//! This file has **no kernel dependencies** (only `alloc`) so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and unit-test the tick arithmetic.
//! The kernel glue — the global table behind a lock, the touch/forget wrappers
//! the executor and `Tensor::migrate`/`Drop` call — lives in `accounting.rs`,
//! because recency **extends §9.5**: it is measurement (when was this resident
//! tensor last used), not decision. The decision (which tensor to evict) lives
//! in `ai/evict_policy.rs` and only *reads* these ticks.
//!
//! A tensor is resident on exactly one device at a time (migration replaces the
//! backing storage — see `ai/tensor.rs`), so the table keys on `tensor_id` alone
//! and records the device the tensor was last used on. `forget` is O(log n) and
//! never scans; migrate-off and `Tensor::drop` both forget, so the table is
//! bounded by the set of live accelerator-resident tensors.

extern crate alloc;

use alloc::collections::BTreeMap;

/// Where (and when) a tensor was last used. The tick is a table-local monotonic
/// counter — strictly increasing across touches, never wall-clock.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RecencyEntry {
    pub device_abi: u32,
    pub tick: u64,
}

/// The recency map: `tensor_id → (device, last-use tick)`. Bounded by the live
/// device-resident tensor set (entries are removed on migrate-off and drop).
#[derive(Default)]
pub struct RecencyTable {
    map: BTreeMap<u64, RecencyEntry>,
    next_tick: u64,
}

impl RecencyTable {
    pub const fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            next_tick: 1,
        }
    }

    /// Record a use of `tensor_id` while resident on `device_abi`; returns the
    /// tick assigned. Ticks are strictly increasing across all touches (a
    /// re-touch moves the tensor to "most recently used"). Saturating: the
    /// counter cannot wrap (u64 ticks outlive any realistic run).
    pub fn touch(&mut self, device_abi: u32, tensor_id: u64) -> u64 {
        let tick = self.next_tick;
        self.next_tick = self.next_tick.saturating_add(1);
        self.map
            .insert(tensor_id, RecencyEntry { device_abi, tick });
        tick
    }

    /// Drop the entry for `tensor_id` (migrate-off-device / tensor drop).
    /// No-op if absent — forget is idempotent.
    pub fn forget(&mut self, tensor_id: u64) {
        self.map.remove(&tensor_id);
    }

    /// The last-use tick of `tensor_id` *on `device_abi`* — `None` if the tensor
    /// was never touched there (or was last used on a different device). The
    /// eviction policy treats `None` as tick 0: never-used is the coldest.
    pub fn tick_of(&self, device_abi: u32, tensor_id: u64) -> Option<u64> {
        self.map
            .get(&tensor_id)
            .filter(|entry| entry.device_abi == device_abi)
            .map(|entry| entry.tick)
    }

    /// Number of live entries (boundedness checks/tests).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
