// SPDX-License-Identifier: MPL-2.0
//! The block cache's lookup (pure; `tools/neofs-cache-logic-tests` holds it
//! to `iter().position` on hand-written inputs).
//!
//! A plain indexed loop, not `iter().position`: this scan is the hot path of
//! a large read-back (the installer's W5 verify spends most of its time in
//! it, by guest RIP samples), and in the debug kernel the iterator form costs
//! three calls per entry spread over several code pages. Its layout alone
//! moved `install_fresh_4k` from about 145 s to 175 s locally under TCG
//! (#57). Both helpers are `#[inline(always)]`, which holds in the debug
//! kernel too, so the loop compiles into the caller as one small body.
//!
//! The scan is linear in the number of cached blocks, and without
//! `neofs-cache-evict` the cache never shrinks, so a large read-back is
//! quadratic. `BlockIndex` (with `neofs-cache-index`) finds a block by binary
//! search instead, and always returns the slot the scan would: the cache's
//! `Vec` keeps its order, which the drain's tie-break depends on
//! (`docs/THINK-2026-11-08-neofs-cache-index.md`).

use alloc::vec::Vec;

/// A cached entry's block number.
pub(super) trait BlockKey {
    fn block_key(&self) -> u64;
}

/// The index of the first entry for `block`, as `iter().position` would
/// return it.
#[inline(always)]
pub(super) fn find_block<T: BlockKey>(entries: &[T], block: u64) -> Option<usize> {
    let mut idx = 0;
    while idx < entries.len() {
        if entries[idx].block_key() == block {
            return Some(idx);
        }
        idx += 1;
    }
    None
}

/// Block number to the slot of its first entry in the cache's `Vec`, kept in
/// step with every change to the `Vec` (`pushed`, `removed`, `rebuild`).
/// `find` then equals `find_block` on the same entries.
///
/// One `Vec` sorted by block number, not a `BTreeMap`: the kernel heap
/// (`linked_list_allocator`) keeps one address-ordered free list, and a
/// tree's thousands of small nodes sit between the cache's 4 KiB buffers, so
/// freeing the cache (a read-only mount's drop) left one hole per buffer and
/// every free walked them all: 38 s of `install_fresh_4k`'s boot after its
/// verify, against 1 s with no index. A sorted `Vec` is a few allocations.
/// The installer and the verify insert in nearly ascending block order, so an
/// insert is nearly always an append.
// Unused without `neofs-cache-index` (the kill-switch build).
#[allow(dead_code)]
pub(super) struct BlockIndex {
    /// `(block, slot)`, sorted by block, one entry per block.
    slots: Vec<(u64, usize)>,
}

#[allow(dead_code)]
impl BlockIndex {
    pub(super) const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    fn position(&self, block: u64) -> Result<usize, usize> {
        self.slots.binary_search_by_key(&block, |&(b, _)| b)
    }

    /// The slot of the first entry for `block`.
    pub(super) fn find(&self, block: u64) -> Option<usize> {
        self.position(block).ok().map(|at| self.slots[at].1)
    }

    /// After `entries.push(..)`: index the new last entry, unless an earlier
    /// entry already holds its block (that one stays first).
    pub(super) fn pushed<T: BlockKey>(&mut self, entries: &[T]) {
        if let Some(last) = entries.len().checked_sub(1) {
            let block = entries[last].block_key();
            if let Err(at) = self.position(block) {
                self.slots.insert(at, (block, last));
            }
        }
    }

    /// After `entries.remove(slot)` of an entry for `block`: every later slot
    /// moved down by one, and a later entry for `block`, if any, is now the
    /// first.
    pub(super) fn removed<T: BlockKey>(&mut self, entries: &[T], slot: usize, block: u64) {
        if let Ok(at) = self.position(block)
            && self.slots[at].1 == slot
        {
            self.slots.remove(at);
        }
        for (_, indexed) in self.slots.iter_mut() {
            if *indexed > slot {
                *indexed -= 1;
            }
        }
        if let Err(at) = self.position(block)
            && let Some(first) = find_block(entries, block)
        {
            self.slots.insert(at, (block, first));
        }
    }

    /// Index `entries` from scratch (after a `retain`).
    pub(super) fn rebuild<T: BlockKey>(&mut self, entries: &[T]) {
        self.slots.clear();
        self.slots.extend(
            entries
                .iter()
                .enumerate()
                .map(|(slot, entry)| (entry.block_key(), slot)),
        );
        // Stable: of equal blocks the first slot stays first, and is kept.
        self.slots.sort_by_key(|&(block, _)| block);
        self.slots.dedup_by_key(|&mut (block, _)| block);
    }

    /// Whether the index is exactly what `rebuild` would make of `entries`.
    pub(super) fn matches<T: BlockKey>(&self, entries: &[T]) -> bool {
        let mut fresh = Self::new();
        fresh.rebuild(entries);
        fresh.slots == self.slots
    }
}
