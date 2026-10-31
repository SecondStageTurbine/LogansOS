// SPDX-License-Identifier: MPL-2.0
//! `alloc_logic` on a model volume. The case Chad found in relay #123: a
//! reservation refused because the generation counter is exhausted must
//! leave the allocation exactly as it was, with and without a free map.

use std::collections::BTreeSet;

use crate::alloc_logic::{self, AllocError, Deferred, Space};

/// The model's freed-but-not-durable blocks (#180's `DeferredFrees`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Pending(BTreeSet<u64>);

impl Deferred for Pending {
    fn is_deferred(&self, block: u64) -> bool {
        self.0.contains(&block)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Volume {
    next_free_block: u64,
    total_blocks: u64,
    next_generation: u32,
    free_map: Option<(Vec<u8>, bool)>,
    deferred: Pending,
    /// Blocks `release_deferred` makes reusable (the model's durable point).
    durable: BTreeSet<u64>,
}

impl Space for Volume {
    fn next_free_block(&self) -> u64 {
        self.next_free_block
    }
    fn set_next_free_block(&mut self, block: u64) {
        self.next_free_block = block;
    }
    fn total_blocks(&self) -> u64 {
        self.total_blocks
    }
    fn next_generation(&self) -> u32 {
        self.next_generation
    }
    fn set_next_generation(&mut self, generation: u32) {
        self.next_generation = generation;
    }
    fn free_map(&mut self) -> Option<(&mut [u8], &mut bool, &dyn Deferred)> {
        let deferred = &self.deferred;
        self.free_map
            .as_mut()
            .map(|(b, d)| (b.as_mut_slice(), d, deferred as &dyn Deferred))
    }
    fn release_deferred(&mut self) {
        for block in std::mem::take(&mut self.durable) {
            self.deferred.0.remove(&block);
        }
    }
}

/// Chad's input: 1,000 blocks, `next_free_block` 100, the generation at
/// `u32::MAX`; with a free map, every block below 100 is used.
fn chad_volume(free_map: bool) -> Volume {
    Volume {
        next_free_block: 100,
        total_blocks: 1000,
        next_generation: u32::MAX,
        free_map: free_map.then(|| (vec![0u8; 1000 / 8], false)),
        deferred: Pending::default(),
        durable: BTreeSet::new(),
    }
}

/// The large writer's order: the 11-block run (10 data, 1 pointer), then the
/// inode block and its generation; on a refusal, the run is given back.
fn large_write(v: &mut Volume) -> Result<(u64, u32), AllocError> {
    let base = alloc_logic::allocate_contiguous_blocks(v, 11)?;
    match alloc_logic::reserve_block_and_generation(v) {
        Ok(r) => Ok(r),
        Err(e) => {
            alloc_logic::release_run(v, base, 11);
            Err(e)
        }
    }
}

#[test]
fn an_exhausted_generation_takes_no_block() {
    for free_map in [false, true] {
        let before = chad_volume(free_map);
        let mut v = before.clone();
        assert_eq!(
            alloc_logic::reserve_block_and_generation(&mut v),
            Err(AllocError::Invalid)
        );
        assert_eq!(v, before, "free_map={free_map}");
    }
}

#[test]
fn chads_large_write_leaves_the_allocation_unchanged() {
    for free_map in [false, true] {
        let before = chad_volume(free_map);
        let mut v = before.clone();
        assert_eq!(large_write(&mut v), Err(AllocError::Invalid));
        assert_eq!(v, before, "free_map={free_map}");
    }
}

/// The order Chad reproduced (block first, then the generation): the same
/// write leaves `next_free_block` at 112 and the inode block used. Kept to
/// show the KAT above tells the two orders apart.
#[test]
fn the_old_order_leaked_the_inode_block() {
    fn old_reserve(v: &mut Volume) -> Result<(u64, u32), AllocError> {
        let block = alloc_logic::allocate_one_block(v)?;
        let generation = v.next_generation;
        v.next_generation = generation.checked_add(1).ok_or(AllocError::Invalid)?;
        Ok((block, generation))
    }
    for free_map in [false, true] {
        let mut v = chad_volume(free_map);
        let base = alloc_logic::allocate_contiguous_blocks(&mut v, 11).unwrap();
        assert_eq!(old_reserve(&mut v), Err(AllocError::Invalid));
        alloc_logic::release_run(&mut v, base, 11);
        assert_eq!(v.next_free_block, 112, "free_map={free_map}");
    }
}

#[test]
fn an_exhausted_generation_takes_no_free_map_block() {
    // A hole of free blocks below `next_free_block`: the free-map path would
    // take block 10.
    let mut before = chad_volume(true);
    for block in 10..20 {
        alloc_logic::free_map_set(&mut before.free_map.as_mut().unwrap().0, block, true).unwrap();
    }
    let mut v = before.clone();
    assert_eq!(
        alloc_logic::reserve_block_and_generation(&mut v),
        Err(AllocError::Invalid)
    );
    assert_eq!(v, before);
}

#[test]
fn a_reservation_takes_one_block_and_one_generation() {
    let mut v = Volume {
        next_generation: 7,
        ..chad_volume(false)
    };
    assert_eq!(
        alloc_logic::reserve_block_and_generation(&mut v),
        Ok((100, 7))
    );
    assert_eq!((v.next_free_block, v.next_generation), (101, 8));
    // The last generation is still handed out; only the one after it can't be.
    let mut v = Volume {
        next_generation: u32::MAX - 1,
        ..chad_volume(true)
    };
    assert_eq!(
        alloc_logic::reserve_block_and_generation(&mut v),
        Ok((100, u32::MAX - 1))
    );
    assert_eq!(v.next_generation, u32::MAX);
}

#[test]
fn no_space_for_the_inode_block_changes_nothing() {
    let before = Volume {
        next_free_block: 1000,
        next_generation: 3,
        ..chad_volume(false)
    };
    let mut v = before.clone();
    assert_eq!(
        alloc_logic::reserve_block_and_generation(&mut v),
        Err(AllocError::NoSpace)
    );
    assert_eq!(v, before);
}

#[test]
fn a_run_that_cannot_rewind_without_a_free_map_is_reported() {
    let mut v = chad_volume(false);
    v.next_free_block = 120;
    assert!(!alloc_logic::release_run(&mut v, 100, 11));
    assert_eq!(v.next_free_block, 120);
    let mut v = chad_volume(true);
    v.next_free_block = 120;
    assert!(alloc_logic::release_run(&mut v, 100, 11));
    let (bytes, dirty) = v.free_map.as_ref().unwrap();
    assert!(*dirty);
    assert!((100..111).all(|b| alloc_logic::free_map_get(bytes, b).unwrap()));
    assert!(!alloc_logic::free_map_get(bytes, 111).unwrap());
}

/// #180, through the moved allocator: a block a commit freed is skipped
/// until that commit is durable, then taken.
#[test]
fn a_deferred_block_is_skipped_until_durable() {
    let mut v = Volume {
        next_generation: 3,
        ..chad_volume(true)
    };
    let map = &mut v.free_map.as_mut().unwrap().0;
    alloc_logic::free_map_set(map, 10, true).unwrap();
    alloc_logic::free_map_set(map, 11, true).unwrap();
    v.deferred.0.insert(10);
    // Block 10 is free but deferred: 11 is taken.
    assert_eq!(
        alloc_logic::reserve_block_and_generation(&mut v),
        Ok((11, 3))
    );
    // Its commit is durable now: 10 is taken.
    v.durable.insert(10);
    assert_eq!(
        alloc_logic::reserve_block_and_generation(&mut v),
        Ok((10, 4))
    );
    assert!(v.deferred.0.is_empty());
    // A deferred block also breaks a contiguous run.
    let mut v = chad_volume(true);
    let map = &mut v.free_map.as_mut().unwrap().0;
    for block in 20..23 {
        alloc_logic::free_map_set(map, block, true).unwrap();
    }
    v.deferred.0.insert(21);
    assert_eq!(alloc_logic::allocate_from_free_map(&mut v, 2), Ok(None));
}

// ---- #206 AAG, C1: frees are checked whole, and a block has one owner. ----

/// A map of 64 blocks with `used` marked used and everything else free.
fn map_with_used(used: &[u64]) -> Vec<u8> {
    let mut bytes = vec![0xFFu8; 8];
    for &b in used {
        alloc_logic::free_map_set(&mut bytes, b, false).unwrap();
    }
    bytes
}

#[test]
fn check_frees_accepts_a_clean_list() {
    let map = map_with_used(&[5, 6, 7, 9]);
    assert_eq!(alloc_logic::check_frees(&map, 64, &[5, 7, 9]), Ok(()));
    assert_eq!(alloc_logic::check_frees(&map, 64, &[]), Ok(()));
}

/// The crafted inode with `direct_blocks = [D, D]`: its superseded list
/// names D twice. Refused, and nothing in the map changes (the caller sets
/// bits only after the check).
#[test]
fn check_frees_refuses_a_block_named_twice() {
    let map = map_with_used(&[5, 6]);
    assert_eq!(
        alloc_logic::check_frees(&map, 64, &[5, 6, 6]),
        Err(AllocError::Invalid)
    );
    assert_eq!(map, map_with_used(&[5, 6]));
}

#[test]
fn check_frees_refuses_an_already_free_block_anywhere_in_the_list() {
    let map = map_with_used(&[5, 6]);
    // The free block last: the old code had set 5 and 6 before refusing it.
    assert_eq!(
        alloc_logic::check_frees(&map, 64, &[5, 6, 20]),
        Err(AllocError::Invalid)
    );
    assert_eq!(map, map_with_used(&[5, 6]));
}

#[test]
fn check_frees_refuses_block_zero_and_blocks_past_the_volume() {
    let map = map_with_used(&[0, 5]);
    assert_eq!(
        alloc_logic::check_frees(&map, 64, &[0]),
        Err(AllocError::Invalid)
    );
    assert_eq!(
        alloc_logic::check_frees(&map, 64, &[64]),
        Err(AllocError::Invalid)
    );
}

/// Mount's live walk: the crafted images an owner-per-block rule rejects,
/// one inode naming D twice, and two directory entries naming one inode.
#[test]
fn mark_live_once_refuses_a_second_owner() {
    let mut live = vec![0u8; 64];
    // Inode X at 10 with direct blocks [D, D], D = 11.
    assert_eq!(alloc_logic::mark_live_once(&mut live, 10), Ok(()));
    assert_eq!(alloc_logic::mark_live_once(&mut live, 11), Ok(()));
    assert_eq!(
        alloc_logic::mark_live_once(&mut live, 11),
        Err(AllocError::Invalid)
    );
    // Two entries naming inode 20: the second visit is refused.
    let mut live = vec![0u8; 64];
    assert_eq!(alloc_logic::mark_live_once(&mut live, 20), Ok(()));
    assert_eq!(
        alloc_logic::mark_live_once(&mut live, 20),
        Err(AllocError::Invalid)
    );
    assert_eq!(
        alloc_logic::mark_live_once(&mut live, 64),
        Err(AllocError::Invalid)
    );
}
