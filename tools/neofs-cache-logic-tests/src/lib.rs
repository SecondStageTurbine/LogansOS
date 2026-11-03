// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/fs/neofs/cache_logic.rs`, the block cache's
//! lookup. The real module is mounted through `#[path]`. Each expected index
//! is written out by hand and also checked against `iter().position`, the
//! form the kernel used before.

#![allow(dead_code)]

#[path = "../../../kernel/src/fs/neofs/cache_logic.rs"]
mod cache_logic;

#[cfg(test)]
mod tests {
    use super::cache_logic::{BlockKey, find_block};

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
}
