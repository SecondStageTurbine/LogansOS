// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/fs/neofs/cache_logic.rs`, the block cache's
//! lookup. The real module is mounted through `#[path]`. Each expected index
//! is written out by hand and also checked against `iter().position`, the
//! form the kernel used before. `BlockIndex` is checked against the scan on
//! hand-written sequences and on fixed-seed random ones (a model check: the
//! same operations on a `Vec` with and without the index).

#![allow(dead_code)]

extern crate alloc;

#[path = "../../../kernel/src/fs/neofs/cache_logic.rs"]
mod cache_logic;

#[cfg(test)]
mod tests {
    use super::cache_logic::{BlockIndex, BlockKey, find_block};

    struct Entry(u64);

    impl BlockKey for Entry {
        fn block_key(&self) -> u64 {
            self.0
        }
    }

    fn entries(blocks: &[u64]) -> Vec<Entry> {
        blocks.iter().map(|&b| Entry(b)).collect()
    }

    fn check(blocks: &[u64], block: u64, expected: Option<usize>) {
        let got = find_block(&entries(blocks), block);
        assert_eq!(got, expected, "find_block({blocks:?}, {block})");
        let reference = blocks.iter().position(|&b| b == block);
        assert_eq!(got, reference, "iter().position({blocks:?}, {block})");
    }

    #[test]
    fn finds_each_entry_by_index() {
        check(&[10, 20, 30], 10, Some(0));
        check(&[10, 20, 30], 20, Some(1));
        check(&[10, 20, 30], 30, Some(2));
    }

    #[test]
    fn a_missing_block_is_none() {
        check(&[10, 20, 30], 99, None);
    }

    #[test]
    fn an_empty_cache_is_none() {
        check(&[], 20, None);
    }

    #[test]
    fn a_repeated_block_returns_the_first() {
        check(&[7, 5, 5], 5, Some(1));
    }

    /// A cache: the entries and their index, changed as `cache.rs` changes
    /// them (push, `remove`, `retain` then rebuild).
    struct Model {
        entries: Vec<Entry>,
        index: BlockIndex,
    }

    impl Model {
        fn new() -> Self {
            Self {
                entries: Vec::new(),
                index: BlockIndex::new(),
            }
        }

        fn push(&mut self, block: u64) -> bool {
            self.entries.push(Entry(block));
            self.index.pushed(&self.entries)
        }

        fn remove(&mut self, slot: usize) {
            let block = self.entries.remove(slot).0;
            self.index.removed(&self.entries, slot, block);
        }

        fn retain_outside(&mut self, first: u64, end: u64) {
            self.entries.retain(|e| e.0 < first || e.0 >= end);
            self.index.rebuild(&self.entries);
        }

        /// The index agrees with a rebuild, and with the scan for every
        /// block below `limit`.
        fn agrees(&self, limit: u64) {
            assert!(self.index.matches(&self.entries));
            for block in 0..limit {
                assert_eq!(
                    self.index.find(block),
                    find_block(&self.entries, block),
                    "block {block} in {:?}",
                    self.entries.iter().map(|e| e.0).collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn index_follows_pushes() {
        let mut m = Model::new();
        for b in [10, 20, 30] {
            m.push(b);
        }
        assert_eq!(m.index.find(10), Some(0));
        assert_eq!(m.index.find(20), Some(1));
        assert_eq!(m.index.find(30), Some(2));
        assert_eq!(m.index.find(99), None);
    }

    #[test]
    fn removing_a_slot_moves_the_later_ones_down() {
        let mut m = Model::new();
        for b in [10, 20, 30] {
            m.push(b);
        }
        m.remove(1);
        assert_eq!(m.index.find(10), Some(0));
        assert_eq!(m.index.find(20), None);
        assert_eq!(m.index.find(30), Some(1));
        m.remove(0);
        assert_eq!(m.index.find(30), Some(0));
        m.remove(0);
        assert_eq!(m.index.find(30), None);
        assert!(m.index.matches(&m.entries));
    }

    #[test]
    fn retain_then_rebuild_matches() {
        let mut m = Model::new();
        for b in [5, 12, 13, 14, 40, 15] {
            m.push(b);
        }
        m.retain_outside(12, 15);
        assert_eq!(
            m.entries.iter().map(|e| e.0).collect::<Vec<_>>(),
            [5, 40, 15]
        );
        assert_eq!(m.index.find(40), Some(1));
        assert_eq!(m.index.find(15), Some(2));
        assert_eq!(m.index.find(13), None);
    }

    /// The cache never holds a block twice (every push follows a miss), but
    /// the index keeps the scan's first-match answer if it ever does.
    #[test]
    fn a_repeated_block_keeps_the_first_slot_and_hands_over_on_removal() {
        let mut m = Model::new();
        for b in [7, 5, 5] {
            m.push(b);
        }
        assert_eq!(m.index.find(5), Some(1));
        m.remove(1);
        assert_eq!(m.index.find(5), Some(1));
        m.remove(0);
        assert_eq!(m.index.find(5), Some(0));
        assert!(m.index.matches(&m.entries));
    }

    /// The cache's invariant: one entry per block. A second push of a block
    /// is reported (the kernel asserts on it), and the index keeps the first.
    #[test]
    fn a_double_insert_is_reported_and_the_first_slot_kept() {
        let mut m = Model::new();
        assert!(m.push(5));
        assert!(m.push(9));
        assert!(!m.push(5));
        assert_eq!(m.index.find(5), Some(0));
        assert_eq!(m.index.len(), 2);
        assert_ne!(m.index.len(), m.entries.len());
    }

    #[test]
    fn one_entry_per_block_keeps_the_lengths_equal() {
        let mut m = Model::new();
        for b in [3, 1, 4, 15, 9, 2, 6] {
            assert!(m.push(b));
            assert_eq!(m.index.len(), m.entries.len());
        }
        m.remove(2);
        m.retain_outside(9, 16);
        assert_eq!(m.index.len(), m.entries.len());
    }

    #[test]
    fn an_empty_index_finds_nothing() {
        let m = Model::new();
        assert_eq!(m.index.find(0), None);
        assert!(m.index.matches(&m.entries));
    }

    /// xorshift64*: a fixed, dependency-free sequence.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Random pushes (no repeats, as in the cache, and with repeats), removals
    /// and range retains; after every step every block agrees with the scan.
    #[test]
    fn model_check_against_the_scan() {
        for seed in 1..=64u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
            let repeats = seed % 2 == 0;
            let mut m = Model::new();
            for _ in 0..400 {
                match rng.below(10) {
                    0..=5 => {
                        let block = rng.below(64);
                        if repeats || find_block(&m.entries, block).is_none() {
                            m.push(block);
                        }
                    }
                    6..=8 if !m.entries.is_empty() => {
                        let slot = rng.below(m.entries.len() as u64) as usize;
                        m.remove(slot);
                    }
                    9 => {
                        let first = rng.below(64);
                        m.retain_outside(first, first + rng.below(8));
                    }
                    _ => {}
                }
                m.agrees(66);
            }
        }
    }
}
