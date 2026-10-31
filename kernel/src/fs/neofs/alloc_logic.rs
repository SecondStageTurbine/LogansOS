// SPDX-License-Identifier: MPL-2.0
//! NeoFS block allocation (`docs/specs/neofs-format.md`): the free map, the
//! bump allocator past `next_free_block`, and the reservation of an inode
//! block with its generation. Pure, over [`Space`], so
//! `tools/neofs-tree-logic-tests` runs the kernel's own allocator on a model
//! volume.
//!
//! Every reservation either takes everything it asks for or changes nothing:
//! [`reserve_block_and_generation`] checks the generation before it takes a
//! block (Chad, relay #123: it used to take the block first, and keep it
//! when the generation was exhausted).

/// Why an allocation was refused. The kernel maps these onto `NeoFsError`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocError {
    /// Not enough free blocks.
    NoSpace,
    /// The superblock or the free map is inconsistent (a block past the map,
    /// or the generation counter exhausted).
    Invalid,
}

/// The allocator's view of a mounted volume.
pub trait Space {
    fn next_free_block(&self) -> u64;
    fn set_next_free_block(&mut self, block: u64);
    fn total_blocks(&self) -> u64;
    fn next_generation(&self) -> u32;
    fn set_next_generation(&mut self, generation: u32);
    /// The free map's bytes (bit set = free), its dirty flag, and the blocks
    /// freed but not yet reusable, when the volume has one.
    fn free_map(&mut self) -> Option<(&mut [u8], &mut bool, &dyn Deferred)>;
    /// Make reusable the freed blocks whose freeing commit is now durable
    /// (#180's barrier rule 2).
    fn release_deferred(&mut self);
}

/// The blocks a commit freed that may not be allocated until that commit is
/// durable (#180, `commit_logic::DeferredFrees`).
pub trait Deferred {
    fn is_deferred(&self, block: u64) -> bool;
}

pub fn free_map_get(bytes: &[u8], block: u64) -> Result<bool, AllocError> {
    let byte_index = usize::try_from(block / 8).map_err(|_| AllocError::Invalid)?;
    let Some(byte) = bytes.get(byte_index) else {
        return Err(AllocError::Invalid);
    };
    Ok((*byte & (1u8 << ((block % 8) as u32))) != 0)
}

pub fn free_map_set(bytes: &mut [u8], block: u64, free: bool) -> Result<(), AllocError> {
    let byte_index = usize::try_from(block / 8).map_err(|_| AllocError::Invalid)?;
    let Some(byte) = bytes.get_mut(byte_index) else {
        return Err(AllocError::Invalid);
    };
    let mask = 1u8 << ((block % 8) as u32);
    if free {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
    Ok(())
}

/// `count` contiguous free blocks below `next_free_block` from the free map,
/// marked used; `None` when the volume has no free map or no such run. A
/// block freed by a commit that isn't durable yet isn't free (#180).
pub fn allocate_from_free_map<S: Space>(s: &mut S, count: u64) -> Result<Option<u64>, AllocError> {
    let total = s.total_blocks();
    let limit = s.next_free_block().min(total);
    if s.free_map().is_none() {
        return Ok(None);
    }
    if count == 0 || count > total {
        return Err(AllocError::NoSpace);
    }
    s.release_deferred();
    let Some((bytes, dirty, deferred)) = s.free_map() else {
        return Ok(None);
    };
    let mut run_start = 0u64;
    let mut run_len = 0u64;
    for block in 1..limit {
        if free_map_get(bytes, block)? && !deferred.is_deferred(block) {
            if run_len == 0 {
                run_start = block;
            }
            run_len += 1;
            if run_len == count {
                for allocated in run_start..run_start + count {
                    free_map_set(bytes, allocated, false)?;
                }
                *dirty = true;
                return Ok(Some(run_start));
            }
        } else {
            run_len = 0;
        }
    }
    Ok(None)
}

/// `count` blocks from `next_free_block`.
pub fn allocate_bump_blocks<S: Space>(s: &mut S, count: u64) -> Result<u64, AllocError> {
    let block = s.next_free_block();
    let end = block.checked_add(count).ok_or(AllocError::NoSpace)?;
    if block == 0 || end > s.total_blocks() {
        return Err(AllocError::NoSpace);
    }
    s.set_next_free_block(end);
    Ok(block)
}

pub fn allocate_one_block<S: Space>(s: &mut S) -> Result<u64, AllocError> {
    if let Some(block) = allocate_from_free_map(s, 1)? {
        return Ok(block);
    }
    allocate_bump_blocks(s, 1)
}

pub fn allocate_contiguous_blocks<S: Space>(s: &mut S, count: u64) -> Result<u64, AllocError> {
    if count == 0 {
        return Err(AllocError::Invalid);
    }
    if let Some(block) = allocate_from_free_map(s, count)? {
        return Ok(block);
    }
    allocate_bump_blocks(s, count)
}

/// One block for a new inode, and the generation it records. The generation
/// is checked first: when it can't advance, no block is taken.
pub fn reserve_block_and_generation<S: Space>(s: &mut S) -> Result<(u64, u32), AllocError> {
    let generation = s.next_generation();
    let next = generation.checked_add(1).ok_or(AllocError::Invalid)?;
    let block = allocate_one_block(s)?;
    s.set_next_generation(next);
    Ok((block, generation))
}

/// Give back the run `base..base + count`. The bump allocator rewinds when
/// nothing was allocated after it; otherwise its blocks are marked free in
/// the free map (they lie below `next_free_block`, so a free bit there is
/// valid). Returns false when the volume has no free map and can't rewind:
/// the blocks stay used until its next sweep.
pub fn release_run<S: Space>(s: &mut S, base: u64, count: u64) -> bool {
    let end = base + count;
    if s.next_free_block() == end {
        // Right for a bump run, and for a free-map run ending there too: its
        // bits stay clear, and clear is what blocks at or past
        // `next_free_block` must be.
        s.set_next_free_block(base);
        return true;
    }
    let total = s.total_blocks();
    let Some((bytes, dirty, _)) = s.free_map() else {
        return false;
    };
    for block in base..end {
        if block < total {
            let _ = free_map_set(bytes, block, true);
        }
    }
    *dirty = true;
    true
}

/// Before a commit frees `blocks` (the version it supersedes): every block
/// is in range, not already free, and named once. Checked over the whole
/// list before any bit changes, so a refusal leaves the map as it was (the
/// #206 AAG, C1: a crafted inode naming one block twice).
pub fn check_frees(bytes: &[u8], total_blocks: u64, blocks: &[u64]) -> Result<(), AllocError> {
    let mut sorted = blocks.to_vec();
    sorted.sort_unstable();
    if sorted.windows(2).any(|w| w[0] == w[1]) {
        return Err(AllocError::Invalid);
    }
    for &block in &sorted {
        if block == 0 || block >= total_blocks || free_map_get(bytes, block)? {
            return Err(AllocError::Invalid);
        }
    }
    Ok(())
}

/// Mark `block` in a mount's live set (one byte per block). A block reached
/// a second time, from two inodes, two directory entries or one inode twice,
/// is refused: freeing one owner's copy would free the other's (C1).
pub fn mark_live_once(live: &mut [u8], block: u64) -> Result<(), AllocError> {
    let index = usize::try_from(block).map_err(|_| AllocError::Invalid)?;
    let slot = live.get_mut(index).ok_or(AllocError::Invalid)?;
    if *slot != 0 {
        return Err(AllocError::Invalid);
    }
    *slot = 1;
    Ok(())
}
