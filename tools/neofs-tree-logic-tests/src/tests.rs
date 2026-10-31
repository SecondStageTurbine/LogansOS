// SPDX-License-Identifier: MPL-2.0
//! The tree KATs. The oracle bytes are laid out by hand from
//! `docs/specs/neofs-format.md` §4.3; the resolver below is written from the
//! kernel reader (`resolve_logical_block` in `kernel/src/fs/neofs/inode.rs`),
//! not from the writer.

use std::collections::HashMap;

use super::tree_logic::*;

const MAX_FILE_BLOCKS: u64 = 2 * 1024 * 1024 * 1024 / 4096;

/// A 9-block file, data at blocks 100..109, its one pointer block at 200:
/// direct slots hold 100..107, the single-indirect root is 200, and block 200
/// holds one pointer, 108, then zeros.
#[test]
fn single_indirect_oracle() {
    let t = Tree {
        n: 9,
        data: 100,
        ptr: 200,
    };
    assert_eq!(pointer_blocks(9), 1);
    assert_eq!(t.direct(), [100, 101, 102, 103, 104, 105, 106, 107]);
    assert_eq!(t.roots(), (200, 0, 0));
    let mut expect = [0u8; 4096];
    expect[..8].copy_from_slice(&[0x6c, 0, 0, 0, 0, 0, 0, 0]); // 108
    assert_eq!(t.pointer_block(0), Some((200, expect)));
    assert_eq!(t.pointer_block(1), None);
}

/// A file 513 blocks into the double level (1,033 blocks): pointer block 0 is
/// the single root, 1 the double root with two leaves (2 and 3); leaf 2 holds
/// the 512 blocks from logical 520, leaf 3 the last one.
#[test]
fn double_indirect_oracle() {
    let (data, ptr) = (1000u64, 5000u64);
    let t = Tree { n: 1033, data, ptr };
    assert_eq!(pointer_blocks(1033), 4);
    assert_eq!(t.roots(), (ptr, ptr + 1, 0));
    let mut root = [0u8; 4096];
    root[..8].copy_from_slice(&(ptr + 2).to_le_bytes());
    root[8..16].copy_from_slice(&(ptr + 3).to_le_bytes());
    assert_eq!(t.pointer_block(1), Some((ptr + 1, root)));
    let (block, leaf) = t.pointer_block(2).unwrap();
    assert_eq!(block, ptr + 2);
    assert_eq!(&leaf[..8], &(data + 520).to_le_bytes());
    assert_eq!(&leaf[4088..], &(data + 1031).to_le_bytes());
    let (_, last) = t.pointer_block(3).unwrap();
    assert_eq!(&last[..8], &(data + 1032).to_le_bytes());
    assert!(last[8..].iter().all(|&b| b == 0));
}

#[test]
fn pointer_block_counts_at_every_boundary() {
    let single_end = 8 + 512;
    let double_end = single_end + 512 * 512;
    for (n, k) in [
        (0, 0),
        (1, 0),
        (8, 0),
        (9, 1),
        (single_end, 1),
        (single_end + 1, 3), // single root, double root, one leaf
        (single_end + 512, 3),
        (single_end + 513, 4),
        (double_end, 1 + 1 + 512),
        (double_end + 1, 1 + 513 + 3), // triple root, one level-2, one leaf
        (MAX_FILE_BLOCKS, 1 + 513 + 1 + 1 + 511),
    ] {
        assert_eq!(pointer_blocks(n), k, "n={n}");
    }
}

/// The kernel reader's walk (`resolve_logical_block`): direct, then single,
/// double, triple, each pointer read from the block the level above names.
fn resolve(t: &Tree, blocks: &HashMap<u64, [u8; 4096]>, logical: u64) -> u64 {
    let ptr_at = |block: u64, index: u64| -> u64 {
        let bytes = blocks
            .get(&block)
            .unwrap_or_else(|| panic!("no pointer block {block}"));
        let at = index as usize * 8;
        let p = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        assert_ne!(p, 0, "a used pointer is zero (block {block} index {index})");
        p
    };
    let (single, double, triple) = t.roots();
    if logical < 8 {
        return t.direct()[logical as usize];
    }
    let mut index = logical - 8;
    if index < 512 {
        return ptr_at(single, index);
    }
    index -= 512;
    if index < 512 * 512 {
        let leaf = ptr_at(double, index / 512);
        return ptr_at(leaf, index % 512);
    }
    index -= 512 * 512;
    let level2 = ptr_at(triple, index / (512 * 512));
    let rest = index % (512 * 512);
    let leaf = ptr_at(level2, rest / 512);
    ptr_at(leaf, rest % 512)
}

fn check(n: u64, data: u64, ptr: u64) {
    let t = Tree { n, data, ptr };
    let k = pointer_blocks(n);
    let mut blocks = HashMap::new();
    for i in 0..k {
        let (block, bytes) = t.pointer_block(i).unwrap();
        assert_eq!(block, ptr + i, "pointer blocks are numbered in order");
        // Every pointer is a data block of this file or another pointer block
        // of it, and the used ones come first.
        let mut ended = false;
        for e in bytes.chunks(8) {
            let p = u64::from_le_bytes(e.try_into().unwrap());
            if p == 0 {
                ended = true;
                continue;
            }
            assert!(!ended, "n={n}: a pointer after a zero in block {block}");
            assert!(
                (data..data + n).contains(&p) || (ptr..ptr + k).contains(&p),
                "n={n}: pointer {p} outside the file"
            );
        }
        assert!(blocks.insert(block, bytes).is_none());
    }
    assert_eq!(t.pointer_block(k), None);
    let (single, double, triple) = t.roots();
    for (root, used) in [
        (single, n > 8),
        (double, n > 520),
        (triple, n > 520 + 512 * 512),
    ] {
        assert_eq!(root != 0, used, "n={n}: root {root}");
    }
    for logical in 0..n {
        assert_eq!(
            resolve(&t, &blocks, logical),
            data + logical,
            "n={n} logical={logical}"
        );
    }
}

#[test]
fn every_logical_block_resolves_through_the_reader_walk() {
    for n in [
        0,
        1,
        7,
        8,
        9,
        100,
        519,
        520,
        521,
        1032,
        1033,
        2048 + 7,
        20_000,
    ] {
        check(n, 77, 1 << 30);
    }
}

#[test]
fn the_triple_level_resolves_too() {
    let double_end = 8 + 512 + 512 * 512;
    for n in [
        double_end,
        double_end + 1,
        double_end + 513,
        double_end + 100_000,
    ] {
        check(n, 1_000, 1 << 32);
    }
}

#[test]
fn the_largest_file_resolves() {
    check(MAX_FILE_BLOCKS, 16_400, 16_400 + MAX_FILE_BLOCKS);
}

/// The in-place form writes the same bytes, and clears what a previous
/// block left in the buffer.
#[test]
fn pointer_block_into_matches_and_clears() {
    let t = Tree {
        n: 1033,
        data: 1000,
        ptr: 5000,
    };
    let mut buf = [0xffu8; 4096];
    for i in 0..pointer_blocks(1033) {
        let block = t.pointer_block_into(i, &mut buf).unwrap();
        assert_eq!(Some((block, buf)), t.pointer_block(i));
    }
    assert_eq!(t.pointer_block_into(4, &mut buf), None);
}
