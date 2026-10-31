// SPDX-License-Identifier: MPL-2.0
//! The NeoFS commit order on a disk with a volatile write cache. Pure: the
//! kernel's block cache (`cache.rs`) and allocator (`inode.rs`) use it, and
//! `tools/neofs-commit-logic-tests` cuts power at every step of it on a
//! write-back disk model.
//!
//! A commit writes new data, inode, directory and free-map blocks (copy on
//! write), then three superblock copies that name the new root. Until a flush
//! returns, the device may persist any subset of the writes in any order. Two
//! rules keep some generation mountable whatever lands:
//!
//! 1. **The barrier.** No superblock copy is written while a non-superblock
//!    block written since the last flush may still be volatile: the drain
//!    flushes first. A superblock that lands then names a root that is on the
//!    media. A failed flush stops the drain with the superblocks unwritten,
//!    so the old copies stay.
//! 2. **Deferred reuse.** A block a commit frees (the version a write,
//!    truncate or directory change supersedes, and the free map it moves
//!    off) is still named by older generations, and mount validates a
//!    candidate's file contents. It is not handed out again until every
//!    superblock copy on the media is at least the freeing generation, so no
//!    generation mount could pick still names it.

use alloc::collections::BTreeMap;

/// A cached block's role. Write-back drains the lowest order first.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Role {
    Data,
    Inode,
    RootDirectory,
    FreeMap,
    Superblock,
}

impl Role {
    pub fn writeback_order(self) -> u8 {
        match self {
            Self::Data => 0,
            Self::Inode => 1,
            Self::RootDirectory => 2,
            Self::FreeMap => 3,
            Self::Superblock => 4,
        }
    }
}

/// The drain's next step.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DrainStep {
    /// Write the dirty block at this index.
    Write(usize),
    /// Flush the device.
    Flush,
    /// Nothing to do.
    Done,
}

/// The next drain step, given the dirty blocks (index and role), whether a
/// non-superblock block written since the last successful flush may still be
/// volatile, and whether a superblock commit is pending (a copy dirtied or
/// written and not yet flushed).
///
/// The lowest-order dirty block goes first. A superblock is never written
/// while `data_unflushed` (rule 1): flush first. With nothing dirty, a
/// pending superblock commit is flushed; data written mid-commit with no
/// superblock yet waits for that commit's barrier.
pub fn next_drain_step<I>(dirty: I, data_unflushed: bool, superblock_pending: bool) -> DrainStep
where
    I: IntoIterator<Item = (usize, Role)>,
{
    let next = dirty
        .into_iter()
        .min_by_key(|&(_, role)| role.writeback_order());
    match next {
        Some((_, Role::Superblock)) if data_unflushed => DrainStep::Flush,
        Some((idx, _)) => DrainStep::Write(idx),
        None if superblock_pending => DrainStep::Flush,
        None => DrainStep::Done,
    }
}

/// The superblock copies a device holds: index `i` is present when copy `i`
/// fits (`docs/specs/neofs-format.md`: copies at 0, 1 MiB and 64 MiB).
pub const COPIES: usize = 3;

/// What each superblock copy's media holds, as far as flushes prove it.
#[derive(Clone, Debug)]
pub struct SuperblockDurability {
    present: [bool; COPIES],
    /// The generation written to each copy since the last successful flush.
    written: [Option<u64>; COPIES],
    /// The generation each copy holds on the media for certain.
    durable: [u64; COPIES],
}

impl SuperblockDurability {
    /// After mounting generation `mounted`. Copies that hold an older
    /// generation are below it, which only makes the tracker stricter: every
    /// block freed after the mount carries a newer generation.
    pub fn new(present: [bool; COPIES], mounted: u64) -> Self {
        Self {
            present,
            written: [None; COPIES],
            durable: [mounted; COPIES],
        }
    }

    /// Copy `copy` was written with `generation` (not yet flushed).
    pub fn written(&mut self, copy: usize, generation: u64) {
        if let Some(slot) = self.written.get_mut(copy) {
            *slot = Some(generation);
        }
    }

    /// A flush returned success: every copy written before it is on the media.
    pub fn flushed(&mut self) {
        for (written, durable) in self.written.iter_mut().zip(self.durable.iter_mut()) {
            if let Some(generation) = written.take() {
                *durable = generation;
            }
        }
    }

    /// The lowest generation any present copy may hold on the media. A cut
    /// leaves a copy at a generation at least this, or unwritten since.
    pub fn durable_generation(&self) -> u64 {
        let mut low = u64::MAX;
        for copy in 0..COPIES {
            if self.present[copy] {
                // A copy written but not flushed may hold either generation:
                // the older one bounds it.
                low = low.min(self.durable[copy]);
            }
        }
        if low == u64::MAX { 0 } else { low }
    }
}

/// Blocks freed by a commit and not yet reusable (rule 2): block → the
/// generation that freed it.
#[derive(Clone, Debug, Default)]
pub struct DeferredFrees {
    pending: BTreeMap<u64, u64>,
}

impl DeferredFrees {
    pub fn new() -> Self {
        Self::default()
    }

    /// `block` was freed by the commit that published `generation`.
    pub fn defer(&mut self, block: u64, generation: u64) {
        self.pending.insert(block, generation);
    }

    /// Forget `block`: the commit that freed it was not published.
    pub fn cancel(&mut self, block: u64) {
        self.pending.remove(&block);
    }

    /// Drop the blocks whose freeing generation is durable on every copy.
    pub fn release(&mut self, durable_generation: u64) {
        self.pending
            .retain(|_, &mut generation| generation > durable_generation);
    }

    /// Whether `block` must not be allocated yet.
    pub fn is_deferred(&self, block: u64) -> bool {
        self.pending.contains_key(&block)
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}
