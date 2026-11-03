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
//! The scan is still linear in the number of cached blocks, and without
//! `neofs-cache-evict` the cache never shrinks: an index by block number is
//! the follow-up.

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
