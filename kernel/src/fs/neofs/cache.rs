// SPDX-License-Identifier: MPL-2.0
extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::block::{BlockDevice, BlockError};
use crate::sched::kthread::{KthreadExitState, KthreadHandle, KthreadJoinResult, KthreadStopToken};
use crate::sched::{self, ThreadWakeToken, WakeTokenResult};
use crate::sync::{IrqSafeMutex, SwitchMutex};

use super::NeoFsError;
pub(super) use super::commit_logic::Role as CachedBlockRole;
use super::commit_logic::{COPIES, DrainStep, SuperblockDurability, next_drain_step};
use super::format::{
    FS_BLOCK_SIZE, SUPERBLOCK_COPY_OFFSETS, SUPERBLOCK_GENERATION_OFFSET, block_offset,
};
use super::inode::{read_bytes_at, read_fs_block, write_fs_block};

const BLOCK_FLUSH_BUSY_RETRIES: usize = 10_000;

/// OT-92: how many of the latest evictions the cache remembers by block
/// number (`CacheInner::recent_evicted`), so a miss whose read raced other
/// blocks' evictions can still be inserted.
#[cfg(feature = "neofs-cache-evict")]
const RECENT_EVICTED: usize = 64;

/// OT-92: how many times a partial write re-reads a block that was evicted
/// while it read it, before it gives up with an error. A retry needs that
/// very block (or more than `RECENT_EVICTED` blocks) evicted during one
/// device read, so this is never reached in practice; it bounds the loop.
const MISS_RETRIES: usize = 16;
const WORKER_DRAIN_BLOCKS_PER_CYCLE: usize = 1;

/// OT-92: the most blocks the cache keeps (1 MiB of 4 KiB blocks; a kernel
/// without a model has a 4 MiB heap). Past it, an insert evicts the least
/// recently used clean block. Dirty blocks are never evicted: they leave by
/// the drain, in the commit order, and `DIRTY_THROTTLE_BLOCKS` bounds them
/// between mutations. So the cache holds at most this plus the dirty blocks
/// one mutation adds past the throttle; that is not a constant, since a
/// mutation dirties as many blocks as it writes (truncating an empty file to
/// 300 blocks dirties 300, and the next insert's scan then walks 301).
/// Kill-switch: without `neofs-cache-evict` (`None`) the cache never evicts
/// (the previous layer).
pub(super) const CACHE_CAPACITY_BLOCKS: Option<usize> = if cfg!(feature = "neofs-cache-evict") {
    Some(256)
} else {
    None
};

/// OT-92: a mutation that starts with at least this many dirty blocks first
/// drains the mount as fsync does (`NeoFsState::throttle_dirty`). It bounds
/// the dirty blocks a mutation starts with, not those it adds: one large
/// mutation can still dirty more than the capacity (see
/// `CACHE_CAPACITY_BLOCKS`).
pub(super) const DIRTY_THROTTLE_BLOCKS: Option<usize> = if cfg!(feature = "neofs-cache-evict") {
    Some(128)
} else {
    None
};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct CacheStats {
    pub cached_blocks: u64,
    pub clean_blocks: u64,
    pub dirty_blocks: u64,
    pub hits: u64,
    pub misses: u64,
    pub dirty_marks: u64,
    pub data_dirty_marks: u64,
    pub inode_dirty_marks: u64,
    pub root_dirty_marks: u64,
    pub free_map_dirty_marks: u64,
    pub superblock_dirty_marks: u64,
    pub drain_batches: u64,
    pub drained_blocks: u64,
    pub flushes: u64,
    pub flush_required: bool,
    pub writeback_hold: bool,
    /// OT-92: clean blocks evicted (and, with `neofs-cache-evict`, blocks
    /// `forget_range` dropped). Always 0 with it off.
    pub evictions: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct WriteBackWorkerStopReport {
    pub had_worker: bool,
    pub exited_success: bool,
    pub exit_code: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub(super) struct DrainReport {
    pub(super) drained_blocks: u64,
}

struct CachedBlock {
    block: u64,
    bytes: Vec<u8>,
    dirty: bool,
    role: CachedBlockRole,
    /// OT-92: `CacheInner::use_clock` at the last hit or insert.
    last_use: u64,
}

struct CacheInner {
    blocks: Vec<CachedBlock>,
    hits: u64,
    misses: u64,
    dirty_marks: u64,
    data_dirty_marks: u64,
    inode_dirty_marks: u64,
    root_dirty_marks: u64,
    free_map_dirty_marks: u64,
    superblock_dirty_marks: u64,
    drain_batches: u64,
    drained_blocks: u64,
    flushes: u64,
    flush_required: bool,
    /// A non-superblock block was written since the last successful flush,
    /// so it may still be in the device's volatile cache: no superblock copy
    /// may be written until a flush (`commit_logic`, rule 1).
    data_unflushed: bool,
    /// The generation each superblock copy holds on the media for certain.
    durability: SuperblockDurability,
    writeback_hold: bool,
    foreground_writers: u32,
    worker_waiter: Option<ThreadWakeToken>,
    /// OT-92: ticks on every hit and insert; the LRU order.
    use_clock: u64,
    /// OT-92: blocks removed from the cache, ever. A miss reads the device
    /// outside the lock; if this moved meanwhile, the block may have been
    /// written, drained and evicted in between, so the bytes read may be
    /// older than the media and are not inserted (`insert_read_locked`).
    evictions: u64,
    /// OT-92: the block numbers of the last `RECENT_EVICTED` evictions,
    /// indexed by `evictions % RECENT_EVICTED` (`evicted_since`).
    #[cfg(feature = "neofs-cache-evict")]
    recent_evicted: [u64; RECENT_EVICTED],
    /// `blocks` by block number (`cache_logic::BlockIndex`), changed in the
    /// same critical section as `blocks` (`push_block`, `evict_locked`,
    /// `forget_range`).
    #[cfg(all(
        feature = "neofs-cache-index",
        not(feature = "neo-test-neofs-cache-index-off")
    ))]
    index: super::cache_logic::BlockIndex,
}

pub(super) struct NeoFsBlockCache {
    device: Arc<dyn BlockDevice>,
    /// Scheduler/context-switch special: the writeback worker parks through
    /// `block_current_with_token_and_guard`, so this cache lock must provide a
    /// release token while the worker-waiter token is installed.
    ///
    /// IRQ-safe (OT-74): it is shared by syscall paths, which run with IF=0
    /// (SYSCALL masks IF), and the preemptible `neofs-writeback` kthread. As a
    /// plain spinlock, a timer preemption of the worker inside a critical
    /// section left the lock held while an IF=0 syscall spun on it — on one
    /// CPU, forever (`arthur_reversibility`'s step-4 hang). Holding it with
    /// interrupts masked means the holder can never be preempted.
    inner: IrqSafeMutex<CacheInner>,
    /// One drain at a time: the barrier's "flushed since the last write"
    /// holds only if no other drain writes between its flush and its
    /// superblock write. Held across block I/O, in thread context with IF=1
    /// (the worker, fsync's `drain_all`); the worker only tries it.
    drain: SwitchMutex<()>,
    /// `durability.durable_generation()`, published for the allocator's
    /// deferred-free check without taking `inner` (`commit_logic`, rule 2).
    durable_generation: Arc<AtomicU64>,
    /// OT-90 P3a: the shutdown's bounded drain ran, so the writeback worker
    /// stops without its own (unbounded) `drain_all`.
    #[cfg(feature = "power-clean")]
    shutdown_drained: core::sync::atomic::AtomicBool,
    /// OT-100 R3 (THINK §4.2): why the volume went read-only
    /// (`persist_logic::Lost` code; 0: it hasn't). Stored once, by the drain
    /// worker on a fatal error or by the user's switch, with `Release`; every
    /// `is_writable()` loads it with `Acquire`, so every mutation entry point
    /// starts refusing the moment it is set.
    #[cfg(feature = "usb-persistence")]
    lost: core::sync::atomic::AtomicU8,
}

pub(super) struct WriteBackPauseGuard<'a> {
    cache: &'a NeoFsBlockCache,
}

impl NeoFsBlockCache {
    /// A cache over `device`, mounted at generation `mounted_generation`.
    pub(super) fn new(device: Arc<dyn BlockDevice>, mounted_generation: u64) -> Self {
        let geometry = device.geometry();
        let device_bytes = geometry
            .block_count
            .saturating_mul(geometry.logical_block_size as u64);
        let mut present = [false; COPIES];
        for (copy, offset) in SUPERBLOCK_COPY_OFFSETS.iter().copied().enumerate() {
            present[copy] = super::format::superblock_copy_on_device(offset, device_bytes);
        }
        Self {
            device,
            drain: SwitchMutex::new(()),
            durable_generation: Arc::new(AtomicU64::new(mounted_generation)),
            #[cfg(feature = "power-clean")]
            shutdown_drained: core::sync::atomic::AtomicBool::new(false),
            #[cfg(feature = "usb-persistence")]
            lost: core::sync::atomic::AtomicU8::new(0),
            inner: IrqSafeMutex::new(CacheInner {
                blocks: Vec::new(),
                hits: 0,
                misses: 0,
                dirty_marks: 0,
                data_dirty_marks: 0,
                inode_dirty_marks: 0,
                root_dirty_marks: 0,
                free_map_dirty_marks: 0,
                superblock_dirty_marks: 0,
                drain_batches: 0,
                drained_blocks: 0,
                flushes: 0,
                flush_required: false,
                data_unflushed: false,
                durability: SuperblockDurability::new(present, mounted_generation),
                writeback_hold: false,
                foreground_writers: 0,
                worker_waiter: None,
                use_clock: 0,
                evictions: 0,
                #[cfg(feature = "neofs-cache-evict")]
                recent_evicted: [0; RECENT_EVICTED],
                #[cfg(all(
                    feature = "neofs-cache-index",
                    not(feature = "neo-test-neofs-cache-index-off")
                ))]
                index: super::cache_logic::BlockIndex::new(),
            }),
        }
    }

    pub(super) fn read_block(&self, block: u64) -> Result<[u8; FS_BLOCK_SIZE], NeoFsError> {
        let epoch = {
            let mut inner = self.inner.lock();
            if let Some(idx) = find_cached_block(&inner, block) {
                inner.hits = inner.hits.saturating_add(1);
                touch(&mut inner, idx);
                let mut out = [0u8; FS_BLOCK_SIZE];
                out.copy_from_slice(&inner.blocks[idx].bytes);
                return Ok(out);
            }
            inner.misses = inner.misses.saturating_add(1);
            inner.evictions
        };

        #[cfg(feature = "neo-test-neofs-dir-ops")]
        super::inode::dir_ops_probe::note_device_read();
        let bytes = read_fs_block(&self.device, block)?;
        let mut inner = self.inner.lock();
        if let Some(idx) = find_cached_block(&inner, block) {
            let mut out = [0u8; FS_BLOCK_SIZE];
            out.copy_from_slice(&inner.blocks[idx].bytes);
            return Ok(out);
        }
        insert_read_locked(&mut inner, block, epoch, || bytes.to_vec());
        Ok(bytes)
    }

    pub(super) fn read_block_prefix(&self, block: u64, out: &mut [u8]) -> Result<(), NeoFsError> {
        if out.len() > FS_BLOCK_SIZE {
            return Err(NeoFsError::InvalidDeviceGeometry);
        }
        if out.is_empty() {
            return Ok(());
        }
        validate_fs_block(&self.device, block)?;
        let epoch = {
            let mut inner = self.inner.lock();
            if let Some(idx) = find_cached_block(&inner, block) {
                inner.hits = inner.hits.saturating_add(1);
                touch(&mut inner, idx);
                out.copy_from_slice(&inner.blocks[idx].bytes[..out.len()]);
                return Ok(());
            }
            inner.misses = inner.misses.saturating_add(1);
            inner.evictions
        };

        #[cfg(feature = "neo-test-neofs-dir-ops")]
        super::inode::dir_ops_probe::note_device_read();
        let mut bytes = alloc::vec![0u8; FS_BLOCK_SIZE];
        read_bytes_at(&self.device, block_offset(block)?, &mut bytes)?;

        let mut inner = self.inner.lock();
        if let Some(idx) = find_cached_block(&inner, block) {
            out.copy_from_slice(&inner.blocks[idx].bytes[..out.len()]);
            return Ok(());
        }
        out.copy_from_slice(&bytes[..out.len()]);
        insert_read_locked(&mut inner, block, epoch, || bytes);
        Ok(())
    }

    pub(super) fn read_block_prefix_uncached(
        &self,
        block: u64,
        out: &mut [u8],
    ) -> Result<(), NeoFsError> {
        if out.len() > FS_BLOCK_SIZE {
            return Err(NeoFsError::InvalidDeviceGeometry);
        }
        if out.is_empty() {
            return Ok(());
        }
        validate_fs_block(&self.device, block)?;
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        super::inode::dir_ops_probe::note_device_read();
        read_bytes_at(&self.device, block_offset(block)?, out)
    }

    pub(super) fn write_block(
        &self,
        block: u64,
        input: &[u8],
        role: CachedBlockRole,
    ) -> Result<(), NeoFsError> {
        if input.len() != FS_BLOCK_SIZE {
            return Err(NeoFsError::InvalidDeviceGeometry);
        }
        validate_fs_block(&self.device, block)?;
        let mut inner = self.inner.lock();
        let idx = find_or_insert_full_block(&mut inner, block, input.to_vec());
        inner.blocks[idx].bytes.copy_from_slice(input);
        mark_dirty(&mut inner, idx, role);
        evict_locked(&mut inner);
        let waiter = take_worker_waiter_if_work_locked(&mut inner);
        drop(inner);
        wake_worker(waiter);
        Ok(())
    }

    pub(super) fn write_bytes_at(
        &self,
        offset: u64,
        input: &[u8],
        role: CachedBlockRole,
    ) -> Result<(), NeoFsError> {
        if input.is_empty() {
            return Ok(());
        }

        validate_device_range(&self.device, offset, input.len())?;
        let mut copied = 0usize;
        while copied < input.len() {
            let cursor = offset + copied as u64;
            let block = cursor / FS_BLOCK_SIZE as u64;
            let in_block = (cursor % FS_BLOCK_SIZE as u64) as usize;
            let take = cmp::min(input.len() - copied, FS_BLOCK_SIZE - in_block);

            // A partial write merges into the block's current bytes. With
            // eviction (OT-92) the block can leave the cache between the
            // check and the merge, or be written, drained and evicted while
            // the device read runs: then read it again rather than merge
            // into bytes that may be older than the media. Only an eviction
            // of this block (`evicted_since`) forces the re-read, and at most
            // `MISS_RETRIES` times.
            let mut attempts = 0usize;
            let mut inner = loop {
                attempts += 1;
                if attempts > MISS_RETRIES {
                    return Err(NeoFsError::BlockDevice);
                }
                let needs_read = {
                    let mut inner = self.inner.lock();
                    if find_cached_block(&inner, block).is_some() {
                        None
                    } else {
                        inner.misses = inner.misses.saturating_add(1);
                        Some(inner.evictions)
                    }
                };
                let read = match needs_read {
                    Some(epoch) => {
                        #[cfg(feature = "neo-test-neofs-dir-ops")]
                        super::inode::dir_ops_probe::note_device_read();
                        let mut bytes = alloc::vec![0u8; FS_BLOCK_SIZE];
                        read_bytes_at(&self.device, block_offset(block)?, &mut bytes)?;
                        Some((epoch, bytes))
                    }
                    None => None,
                };

                let mut inner = self.inner.lock();
                if find_cached_block(&inner, block).is_some() {
                    break inner;
                }
                match read {
                    Some((epoch, bytes)) if !evicted_since(&inner, epoch, block) => {
                        push_block(&mut inner, block, bytes);
                        break inner;
                    }
                    _ => {}
                }
            };
            let idx = find_cached_block(&inner, block).ok_or(NeoFsError::BlockDevice)?;
            touch(&mut inner, idx);
            inner.blocks[idx].bytes[in_block..in_block + take]
                .copy_from_slice(&input[copied..copied + take]);
            mark_dirty(&mut inner, idx, role);
            evict_locked(&mut inner);
            let waiter = take_worker_waiter_if_work_locked(&mut inner);
            drop(inner);
            wake_worker(waiter);
            copied += take;
        }
        Ok(())
    }

    /// #106 relay 403 (CP-63): read the blocks the superblock copies sit in
    /// and keep them cached for the mount's life ([`pinned_block`]). A
    /// superblock copy is 512 bytes of a 4 KiB block, so writing it merges
    /// into the block's bytes: on a miss, `write_bytes_at` reads the device
    /// first, and every commit writes the copies with the namespace lock
    /// (`entries`, spinning) held. Pinned, the write is always a hit. It
    /// pins exactly the copies the writers publish (the one rule,
    /// `format::superblock_copy_on_device`); a writable mount has already
    /// refused a device where such a copy's block is not whole
    /// (`superblock_copies_pinnable`), and this refuses it again.
    #[cfg(feature = "neofs-dir-ops")]
    pub(super) fn pin_superblock_blocks(&self) -> Result<(), NeoFsError> {
        let geometry = self.device.geometry();
        let device_bytes = geometry
            .block_count
            .saturating_mul(geometry.logical_block_size as u64);
        for offset in SUPERBLOCK_COPY_OFFSETS {
            if !super::format::superblock_copy_on_device(offset, device_bytes) {
                continue;
            }
            let block = offset / FS_BLOCK_SIZE as u64;
            if block_offset(block)? + FS_BLOCK_SIZE as u64 > device_bytes {
                return Err(NeoFsError::InvalidDeviceGeometry);
            }
            let mut bytes = alloc::vec![0u8; FS_BLOCK_SIZE];
            read_bytes_at(&self.device, block_offset(block)?, &mut bytes)?;
            let mut inner = self.inner.lock();
            if find_cached_block(&inner, block).is_none() {
                push_block(&mut inner, block, bytes);
            }
        }
        Ok(())
    }

    /// NFS-W1: drop any cached copy of blocks `first..first + count`, which
    /// were just allocated to a file written past the cache. Only free blocks
    /// are ever allocated, so a cached copy (even a dirty one) holds no data
    /// anything references, and a later drain must not write it over the
    /// file's bytes.
    #[cfg(any(feature = "neofs-large-write", feature = "neo-test-neofs-dir-ops"))]
    pub(super) fn forget_range(&self, first: u64, count: u64) {
        let end = first.saturating_add(count);
        let mut inner = self.inner.lock();
        // Off (the previous layer): drop them, and nothing else changes.
        #[cfg(not(feature = "neofs-cache-evict"))]
        {
            inner.blocks.retain(|cached| {
                pinned_block(cached.block) || cached.block < first || cached.block >= end
            });
            #[cfg(all(
                feature = "neofs-cache-index",
                not(feature = "neo-test-neofs-cache-index-off")
            ))]
            {
                let inner = &mut *inner;
                inner.index.rebuild(&inner.blocks);
                check_index(inner, None);
            }
        }
        // On: each one counts as an eviction, so a miss that read it before
        // it was dropped doesn't insert its old bytes.
        #[cfg(feature = "neofs-cache-evict")]
        {
            let mut idx = 0;
            while idx < inner.blocks.len() {
                let block = inner.blocks[idx].block;
                if block >= first && block < end && !pinned_block(block) {
                    inner.blocks.remove(idx);
                    #[cfg(all(
                        feature = "neofs-cache-index",
                        not(feature = "neo-test-neofs-cache-index-off")
                    ))]
                    {
                        let inner = &mut *inner;
                        inner.index.removed(&inner.blocks, idx, block);
                        check_index(inner, None);
                    }
                    record_eviction(&mut inner, block);
                } else {
                    idx += 1;
                }
            }
        }
    }

    /// OT-92's kill-switch leg: `forget_range` from the scenario.
    #[cfg(all(feature = "neo-test", feature = "neofs-large-write"))]
    pub(super) fn debug_forget_range(&self, first: u64, count: u64) {
        self.forget_range(first, count);
    }

    pub(super) fn drain_all(&self) -> Result<DrainReport, NeoFsError> {
        let _worker_pause = self.pause_worker();
        let mut total = DrainReport::default();
        loop {
            // OT-100 R3 (THINK §4.2, CP-113): once the volume is lost, no
            // drain writes again, a foreground one included.
            #[cfg(feature = "usb-persistence")]
            if self.lost().is_some() {
                return Err(NeoFsError::ReadOnly);
            }
            let report = {
                let _drain = self.drain.lock();
                drain_pass(
                    &self.device,
                    &self.inner,
                    &self.durable_generation,
                    None,
                    false,
                )?
            };
            total.drained_blocks = total.drained_blocks.saturating_add(report.drained_blocks);
            let done = {
                let inner = self.inner.lock();
                !inner.flush_required && !inner.blocks.iter().any(|block| block.dirty)
            };
            if done {
                return Ok(total);
            }
            if report.drained_blocks == 0 {
                sched::yield_now();
            }
        }
    }

    /// OT-90 P3a, THINK §5 K2: `drain_all` with a bound. Passes of at most
    /// `SHUTDOWN_PASS_BLOCKS` blocks, through the same barrier order, until
    /// nothing is dirty and no flush is owed, or until `stall_ticks` timer
    /// ticks pass with no block drained. Returns the blocks drained and the
    /// blocks still dirty (0: clean); an error is a failed write or flush.
    #[cfg(feature = "power-clean")]
    pub(super) fn shutdown_drain(&self, stall_ticks: u64) -> Result<(u64, u64), NeoFsError> {
        const SHUTDOWN_PASS_BLOCKS: usize = 64;
        let ticks =
            || crate::arch::x86_64::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
        let _worker_pause = self.pause_worker();
        let mut drained = 0u64;
        let mut last_progress = ticks();
        loop {
            let report = {
                let _drain = self.drain.lock();
                drain_pass(
                    &self.device,
                    &self.inner,
                    &self.durable_generation,
                    Some(SHUTDOWN_PASS_BLOCKS),
                    false,
                )?
            };
            drained = drained.saturating_add(report.drained_blocks);
            let (done, dirty) = {
                let inner = self.inner.lock();
                let dirty = inner.blocks.iter().filter(|block| block.dirty).count() as u64;
                (!inner.flush_required && dirty == 0, dirty)
            };
            if done {
                return Ok((drained, 0));
            }
            if report.drained_blocks != 0 {
                last_progress = ticks();
            } else if ticks().saturating_sub(last_progress) > stall_ticks {
                return Ok((drained, dirty.max(1)));
            } else {
                sched::yield_now();
            }
        }
    }

    /// OT-90 P3a: the blocks dirty now (also OT-100 R3's "unsaved blocks
    /// dropped" count).
    #[cfg(any(feature = "power-clean", feature = "usb-persistence"))]
    pub(super) fn dirty_count(&self) -> u64 {
        self.inner
            .lock()
            .blocks
            .iter()
            .filter(|block| block.dirty)
            .count() as u64
    }

    /// OT-90 P3a: the shutdown drained; the worker's stop skips its own drain.
    #[cfg(feature = "power-clean")]
    pub(super) fn mark_shutdown_drained(&self) {
        self.shutdown_drained
            .store(true, core::sync::atomic::Ordering::SeqCst);
    }

    pub(super) fn drain_worker_cycle(&self) -> Result<DrainReport, NeoFsError> {
        {
            let inner = self.inner.lock();
            if inner.writeback_hold || inner.foreground_writers != 0 {
                return Ok(DrainReport::default());
            }
        }
        // A foreground drain is running: it does this cycle's work.
        let Some(_drain) = self.drain.try_lock() else {
            return Ok(DrainReport::default());
        };
        drain_pass(
            &self.device,
            &self.inner,
            &self.durable_generation,
            Some(WORKER_DRAIN_BLOCKS_PER_CYCLE),
            false,
        )
    }

    /// The cut `phase3_neofs_crash_mid_overwrite_recovery` models: every
    /// non-superblock dirty block written and the barrier flush returned, no
    /// superblock written. Returns the flushes it issued.
    #[cfg(any(
        feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery",
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub(super) fn debug_drain_to_barrier(&self) -> Result<u64, NeoFsError> {
        let _worker_pause = self.pause_worker();
        let _drain = self.drain.lock();
        let before = self.inner.lock().flushes;
        drain_pass(
            &self.device,
            &self.inner,
            &self.durable_generation,
            None,
            true,
        )?;
        Ok(self.inner.lock().flushes.saturating_sub(before))
    }

    /// The handle the allocator reads the durable superblock generation
    /// through (`commit_logic`, rule 2).
    pub(super) fn durable_generation_handle(&self) -> Arc<AtomicU64> {
        self.durable_generation.clone()
    }

    /// OT-92: whether a mutation should drain before it starts.
    pub(super) fn dirty_over_throttle(&self) -> bool {
        let Some(limit) = DIRTY_THROTTLE_BLOCKS else {
            return false;
        };
        let inner = self.inner.lock();
        inner.blocks.iter().filter(|block| block.dirty).count() >= limit
    }

    pub(super) fn pause_worker(&self) -> WriteBackPauseGuard<'_> {
        let mut inner = self.inner.lock();
        inner.foreground_writers = inner.foreground_writers.saturating_add(1);
        WriteBackPauseGuard { cache: self }
    }

    fn park_worker_if_idle(&self, stop: &KthreadStopToken) {
        if stop.is_stop_requested() {
            return;
        }

        // Park with IF already off: an `IrqSafeGuard` handed to the context
        // switch is forgotten, so it cannot restore IF itself — the enclosing
        // `without_interrupts` restores the worker's IF=1 on resume (the same
        // shape as virtio-blk's completion park).
        crate::arch::x86_64::without_interrupts(|| {
            let guard = self.inner.lock();
            if stop.is_stop_requested() || worker_has_work_locked(&guard) {
                return;
            }

            sched::block_current_with_token_and_guard(guard, |inner, token| {
                debug_assert!(
                    inner.worker_waiter.is_none(),
                    "NeoFS writeback worker waiter already installed"
                );
                inner.worker_waiter = Some(token);
            });
        });
    }

    fn wake_worker_for_stop(&self) {
        let waiter = self.inner.lock().worker_waiter.take();
        wake_worker(waiter);
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_stats(&self) -> CacheStats {
        let inner = self.inner.lock();
        stats_locked(&inner)
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_set_writeback_hold(&self, hold: bool) {
        let mut inner = self.inner.lock();
        let was_held = inner.writeback_hold;
        inner.writeback_hold = hold;
        let waiter = if was_held && !hold {
            take_worker_waiter_if_work_locked(&mut inner)
        } else {
            None
        };
        drop(inner);
        wake_worker(waiter);
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_wait_clean(&self, max_yields: usize) -> bool {
        for _ in 0..max_yields {
            let clean = {
                let inner = self.inner.lock();
                !inner.flush_required && !inner.blocks.iter().any(|block| block.dirty)
            };
            if clean {
                return true;
            }
            sched::yield_now();
            sched::reap_nested();
        }
        false
    }
}

impl Drop for WriteBackPauseGuard<'_> {
    fn drop(&mut self) {
        let mut inner = self.cache.inner.lock();
        inner.foreground_writers = inner.foreground_writers.saturating_sub(1);
        let waiter = if inner.foreground_writers == 0 {
            take_worker_waiter_if_work_locked(&mut inner)
        } else {
            None
        };
        drop(inner);
        wake_worker(waiter);
    }
}

pub(super) fn spawn_writeback_worker(cache: Arc<NeoFsBlockCache>) -> KthreadHandle {
    crate::sched::kthread::spawn_owned(
        "neofs-writeback",
        move |stop: crate::sched::kthread::KthreadStopToken| loop {
            if stop.is_stop_requested() {
                // OT-90 P3a: after the shutdown's bounded drain, stop without
                // another (unbounded) one.
                #[cfg(feature = "power-clean")]
                if cache
                    .shutdown_drained
                    .load(core::sync::atomic::Ordering::SeqCst)
                {
                    return KthreadExitState::Success;
                }
                return match cache.drain_all() {
                    Ok(_) => KthreadExitState::Success,
                    Err(_) => KthreadExitState::Failed(1),
                };
            }
            if cache.drain_worker_cycle().is_err() {
                // OT-100 R3: the volume is lost; it goes read-only.
                #[cfg(feature = "usb-persistence")]
                cache.lose_to_failure();
                return KthreadExitState::Failed(2);
            }
            cache.park_worker_if_idle(&stop);
        },
    )
}

#[cfg(feature = "usb-persistence")]
impl NeoFsBlockCache {
    /// Why the volume went read-only, if it did.
    pub(super) fn lost(&self) -> Option<crate::persist_logic::Lost> {
        crate::persist_logic::Lost::from_code(self.lost.load(Ordering::Acquire))
    }

    /// THINK §4.2: the drain hit a fatal error. Classify it from the
    /// device's last failure and flip the mount read-only: store the reason
    /// first (`Release`), then report once. No superblock is written: the
    /// stick keeps its last committed generation, which the next boot mounts.
    pub(super) fn lose_to_failure(&self) {
        let failure = self
            .device
            .last_failure()
            .unwrap_or(crate::persist_logic::BlockFailure::Other);
        self.lose(crate::persist_logic::classify(failure));
    }

    /// The "try" switch (THINK §2.3, §4.2): drain first, so the flip is
    /// clean, then go read-only with the user's reason.
    pub(super) fn lose_by_user(&self) {
        let _ = self.drain_all();
        self.lose(crate::persist_logic::Lost::User);
    }

    fn lose(&self, lost: crate::persist_logic::Lost) {
        if self
            .lost
            .compare_exchange(0, lost.code(), Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let dropped = self.dirty_count();
        crate::serial_println!(
            "persist: volume lost ({}); read-only - {} unsaved blocks dropped",
            lost.reason(),
            dropped
        );
        crate::serial_println!(
            "persist: notice: Your changes could not be saved: {}. LogansOS is now read-only until you restart.",
            lost.notice()
        );
    }
}

pub(super) fn stop_writeback_worker(
    cache: &NeoFsBlockCache,
    worker: &SwitchMutex<Option<KthreadHandle>>,
) -> WriteBackWorkerStopReport {
    let Some(handle) = worker.lock().take() else {
        return WriteBackWorkerStopReport {
            had_worker: false,
            exited_success: false,
            exit_code: 0,
        };
    };

    handle.request_stop();
    cache.wake_worker_for_stop();
    let join = handle.join();
    sched::reap_nested();
    match join {
        KthreadJoinResult::Exited(KthreadExitState::Success) => WriteBackWorkerStopReport {
            had_worker: true,
            exited_success: true,
            exit_code: 0,
        },
        KthreadJoinResult::Exited(KthreadExitState::Failed(code)) => WriteBackWorkerStopReport {
            had_worker: true,
            exited_success: false,
            exit_code: code,
        },
        KthreadJoinResult::Detached => WriteBackWorkerStopReport {
            had_worker: true,
            exited_success: false,
            exit_code: 0xd37a,
        },
        KthreadJoinResult::AlreadyJoined => WriteBackWorkerStopReport {
            had_worker: true,
            exited_success: false,
            exit_code: 0xa11d,
        },
        KthreadJoinResult::JoinAlreadyInProgress => WriteBackWorkerStopReport {
            had_worker: true,
            exited_success: false,
            exit_code: 0x001a,
        },
    }
}

fn worker_has_work_locked(inner: &CacheInner) -> bool {
    !inner.writeback_hold
        && inner.foreground_writers == 0
        && (inner.flush_required || inner.blocks.iter().any(|block| block.dirty))
}

fn take_worker_waiter_if_work_locked(inner: &mut CacheInner) -> Option<ThreadWakeToken> {
    if worker_has_work_locked(inner) {
        inner.worker_waiter.take()
    } else {
        None
    }
}

fn wake_worker(waiter: Option<ThreadWakeToken>) {
    if let Some(token) = waiter {
        match sched::wake_thread_token(token, 0) {
            WakeTokenResult::Woken | WakeTokenResult::Gone | WakeTokenResult::Stale => {}
        }
    }
}

impl super::cache_logic::BlockKey for CachedBlock {
    #[inline(always)]
    fn block_key(&self) -> u64 {
        self.block
    }
}

/// The hot path of a large read-back: see `cache_logic`. With
/// `neofs-cache-index`, the index. Without it, or with the kill-switch leg
/// (`neo-test-neofs-cache-index-off`, which compiles every part of the index
/// out, its upkeep and checks too: the previous layer), the scan. The
/// crosscheck scenarios run both on every lookup, and compare the index with a
/// rebuild after every change.
fn find_cached_block(inner: &CacheInner, block: u64) -> Option<usize> {
    #[cfg(all(
        feature = "neofs-cache-index",
        not(feature = "neo-test-neofs-cache-index-off")
    ))]
    {
        let found = inner.index.find(block);
        #[cfg(feature = "neofs-cache-index-crosscheck")]
        assert_eq!(
            found,
            super::cache_logic::find_block(&inner.blocks, block),
            "neofs cache index: lookup of block {} differs from the scan",
            block
        );
        found
    }
    #[cfg(not(all(
        feature = "neofs-cache-index",
        not(feature = "neo-test-neofs-cache-index-off")
    )))]
    {
        super::cache_logic::find_block(&inner.blocks, block)
    }
}

/// After a change to the index (a push, a removal, a rebuild; never a hit,
/// which changes nothing in it). Debug-assertion kernels check only what the
/// change touched, in O(log n): one entry per cached block, and a pushed
/// block's slot. The crosscheck scenarios also compare the whole index with a
/// rebuild (O(n log n), so not per change elsewhere: it would make the write
/// path quadratic again in the debug build `install_fresh_4k` times).
#[cfg(all(
    feature = "neofs-cache-index",
    not(feature = "neo-test-neofs-cache-index-off")
))]
#[inline(always)]
fn check_index(inner: &CacheInner, pushed: Option<(u64, usize)>) {
    debug_assert_eq!(
        inner.index.len(),
        inner.blocks.len(),
        "neofs cache index: not one entry per cached block"
    );
    if let Some((block, slot)) = pushed {
        debug_assert_eq!(
            inner.index.find(block),
            Some(slot),
            "neofs cache index: pushed block {} not at its slot",
            block
        );
    }
    #[cfg(feature = "neofs-cache-index-crosscheck")]
    assert!(
        inner.index.matches(&inner.blocks),
        "neofs cache index out of step with the cache ({} blocks)",
        inner.blocks.len()
    );
    let _ = (inner, pushed);
}

fn find_or_insert_full_block(inner: &mut CacheInner, block: u64, bytes: Vec<u8>) -> usize {
    if let Some(idx) = find_cached_block(inner, block) {
        return idx;
    }

    inner.misses = inner.misses.saturating_add(1);
    push_block(inner, block, bytes)
}

/// Append a clean block as the most recently used; returns its index.
fn push_block(inner: &mut CacheInner, block: u64, bytes: Vec<u8>) -> usize {
    inner.use_clock = inner.use_clock.wrapping_add(1);
    let last_use = inner.use_clock;
    inner.blocks.push(CachedBlock {
        block,
        bytes,
        dirty: false,
        role: CachedBlockRole::Data,
        last_use,
    });
    #[cfg(all(
        feature = "neofs-cache-index",
        not(feature = "neo-test-neofs-cache-index-off")
    ))]
    {
        let indexed = inner.index.pushed(&inner.blocks);
        // One entry per block: every push follows a miss under this lock.
        debug_assert!(indexed, "neofs cache: block {} pushed twice", block);
        let _ = indexed;
        check_index(inner, Some((block, inner.blocks.len() - 1)));
    }
    inner.blocks.len() - 1
}

fn touch(inner: &mut CacheInner, idx: usize) {
    inner.use_clock = inner.use_clock.wrapping_add(1);
    inner.blocks[idx].last_use = inner.use_clock;
}

/// Insert what a miss read from the device, unless the block may have been
/// evicted since the miss (`evicted_since`): the bytes may then predate a
/// write that was drained and evicted meanwhile. Skipping costs only a
/// later miss.
fn insert_read_locked(
    inner: &mut CacheInner,
    block: u64,
    epoch: u64,
    bytes: impl FnOnce() -> Vec<u8>,
) {
    if evicted_since(inner, epoch, block) {
        return;
    }
    push_block(inner, block, bytes());
    evict_locked(inner);
}

/// OT-92: evict least recently used clean blocks until the cache is within
/// `CACHE_CAPACITY_BLOCKS`. A clean block matches the device (a read returns
/// the last write, flushed or not), so dropping it loses nothing. If every
/// block is dirty the cache stays over until the drain cleans some.
fn evict_locked(inner: &mut CacheInner) {
    let Some(capacity) = CACHE_CAPACITY_BLOCKS else {
        return;
    };
    while inner.blocks.len() > capacity {
        let Some(victim) = inner
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| !block.dirty && !pinned_block(block.block))
            .min_by_key(|(_, block)| block.last_use)
            .map(|(idx, _)| idx)
        else {
            return;
        };
        // `remove`, not `swap_remove`: the drain takes the first dirty block
        // of the lowest role, so keep the others in insertion order.
        let block = inner.blocks.remove(victim).block;
        #[cfg(all(
            feature = "neofs-cache-index",
            not(feature = "neo-test-neofs-cache-index-off")
        ))]
        {
            inner.index.removed(&inner.blocks, victim, block);
            check_index(inner, None);
        }
        record_eviction(inner, block);
    }
}

/// Count an eviction of `block` and, with `neofs-cache-evict`, remember it
/// (`evicted_since`). Without it nothing calls this: `evict_locked` returns
/// at once and `forget_range` keeps the previous layer's `retain`.
fn record_eviction(inner: &mut CacheInner, block: u64) {
    #[cfg(feature = "neofs-cache-evict")]
    {
        inner.recent_evicted[(inner.evictions % RECENT_EVICTED as u64) as usize] = block;
    }
    #[cfg(not(feature = "neofs-cache-evict"))]
    let _ = block;
    inner.evictions = inner.evictions.wrapping_add(1);
}

/// Whether `block` may have been evicted since `epoch` (the eviction count a
/// miss read before its device read). Without `neofs-cache-evict` nothing
/// is ever evicted, so never. With it: when more than `RECENT_EVICTED`
/// evictions happened, the record no longer covers them all, so assume so.
fn evicted_since(inner: &CacheInner, epoch: u64, block: u64) -> bool {
    let since = inner.evictions.wrapping_sub(epoch);
    if since == 0 {
        return false;
    }
    #[cfg(feature = "neofs-cache-evict")]
    {
        if since > RECENT_EVICTED as u64 {
            return true;
        }
        (1..=since).any(|back| {
            let at = inner.evictions.wrapping_sub(back) % RECENT_EVICTED as u64;
            inner.recent_evicted[at as usize] == block
        })
    }
    #[cfg(not(feature = "neofs-cache-evict"))]
    {
        let _ = block;
        true
    }
}

fn mark_dirty(inner: &mut CacheInner, idx: usize, role: CachedBlockRole) {
    inner.blocks[idx].dirty = true;
    inner.blocks[idx].role = role;
    inner.dirty_marks = inner.dirty_marks.saturating_add(1);
    match role {
        CachedBlockRole::Data => inner.data_dirty_marks = inner.data_dirty_marks.saturating_add(1),
        CachedBlockRole::Inode => {
            inner.inode_dirty_marks = inner.inode_dirty_marks.saturating_add(1)
        }
        CachedBlockRole::RootDirectory => {
            inner.root_dirty_marks = inner.root_dirty_marks.saturating_add(1)
        }
        CachedBlockRole::FreeMap => {
            inner.free_map_dirty_marks = inner.free_map_dirty_marks.saturating_add(1)
        }
        CachedBlockRole::Superblock => {
            inner.superblock_dirty_marks = inner.superblock_dirty_marks.saturating_add(1);
            inner.flush_required = true;
        }
    }
}

/// One drain pass under the cache's drain lock (`commit_logic`): the
/// lowest-order dirty block first; before any superblock copy, a flush if a
/// non-superblock write may still be volatile (rule 1); and once nothing is
/// dirty, the flush that makes the superblocks durable. A flush error stops
/// the pass with the superblocks unwritten. `max_blocks` bounds the steps
/// (writes and flushes) for the worker. `to_barrier` (a test's cut) leaves
/// every superblock copy dirty and ends with the barrier flush.
fn drain_pass(
    device: &Arc<dyn BlockDevice>,
    inner_lock: &IrqSafeMutex<CacheInner>,
    durable_generation: &AtomicU64,
    max_blocks: Option<usize>,
    to_barrier: bool,
) -> Result<DrainReport, NeoFsError> {
    let mut drained_blocks = 0u64;
    let mut steps = 0usize;
    loop {
        if max_blocks.map(|max| steps >= max).unwrap_or(false) {
            break;
        }
        let step = {
            let inner = inner_lock.lock();
            let step = next_drain_step(
                inner
                    .blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| block.dirty)
                    .filter(|(_, block)| !to_barrier || block.role != CachedBlockRole::Superblock)
                    .map(|(idx, block)| (idx, block.role)),
                inner.data_unflushed,
                if to_barrier {
                    inner.data_unflushed
                } else {
                    inner.flush_required
                },
            );
            match step {
                DrainStep::Write(idx) => {
                    let block = &inner.blocks[idx];
                    Some(Some((block.block, block.bytes.to_vec(), block.role)))
                }
                DrainStep::Flush => Some(None),
                DrainStep::Done => None,
            }
        };
        let Some(step) = step else {
            break;
        };
        steps += 1;

        let Some((block, bytes, role)) = step else {
            flush_device_with_retry(device)?;
            let mut inner = inner_lock.lock();
            inner.flushes = inner.flushes.saturating_add(1);
            inner.data_unflushed = false;
            inner.durability.flushed();
            durable_generation.store(inner.durability.durable_generation(), Ordering::Release);
            if !inner.blocks.iter().any(|block| block.dirty) {
                inner.flush_required = false;
            }
            if to_barrier {
                break;
            }
            continue;
        };

        write_fs_block(device, block, &bytes)?;
        let mut inner = inner_lock.lock();
        if role == CachedBlockRole::Superblock {
            if let Some(copy) = superblock_copy_of_block(block) {
                inner
                    .durability
                    .written(copy, superblock_generation(&bytes));
            }
        } else {
            inner.data_unflushed = true;
        }
        if let Some(idx) = find_cached_block(&inner, block)
            && inner.blocks[idx].dirty
            && inner.blocks[idx].role == role
            && inner.blocks[idx].bytes.as_slice() == bytes.as_slice()
        {
            inner.blocks[idx].dirty = false;
            drained_blocks = drained_blocks.saturating_add(1);
        }
    }

    if drained_blocks != 0 {
        let mut inner = inner_lock.lock();
        inner.drain_batches = inner.drain_batches.saturating_add(1);
        inner.drained_blocks = inner.drained_blocks.saturating_add(drained_blocks);
    }

    Ok(DrainReport { drained_blocks })
}

/// #106 relay 403: a block that stays cached from mount on (the superblock
/// copies' blocks, [`NeoFsBlockCache::pin_superblock_blocks`]): never an
/// eviction victim, and never dropped by `forget_range` (nothing allocates
/// them, and the scenario's cold cache keeps them). Off (`neofs-dir-ops`
/// disabled): none, the previous layer.
#[cfg(feature = "neofs-dir-ops")]
fn pinned_block(block: u64) -> bool {
    superblock_copy_of_block(block).is_some()
}

#[cfg(not(feature = "neofs-dir-ops"))]
fn pinned_block(_block: u64) -> bool {
    false
}

/// The superblock copy (0..3) whose 4 KiB block is `block`, if any.
fn superblock_copy_of_block(block: u64) -> Option<usize> {
    SUPERBLOCK_COPY_OFFSETS
        .iter()
        .position(|&offset| offset / FS_BLOCK_SIZE as u64 == block)
}

/// The generation in a superblock copy's block (the copy starts the block).
fn superblock_generation(bytes: &[u8]) -> u64 {
    let mut field = [0u8; 8];
    field.copy_from_slice(&bytes[SUPERBLOCK_GENERATION_OFFSET..SUPERBLOCK_GENERATION_OFFSET + 8]);
    u64::from_le_bytes(field)
}

/// A writable NeoFS mount requires a device with a flush (`NeoFs::mount`), so
/// a device without one is an error here, never a skipped barrier.
fn flush_device_with_retry(device: &Arc<dyn BlockDevice>) -> Result<(), NeoFsError> {
    if !device.supports_flush() {
        return Err(NeoFsError::NoDurableFlush);
    }
    for _ in 0..BLOCK_FLUSH_BUSY_RETRIES {
        match device.flush() {
            Ok(()) => return Ok(()),
            Err(BlockError::Unsupported) => {
                sched::yield_now();
                sched::reap_nested();
            }
            Err(_) => return Err(NeoFsError::BlockDevice),
        }
    }
    Err(NeoFsError::BlockDevice)
}

fn stats_locked(inner: &CacheInner) -> CacheStats {
    let dirty_blocks = inner.blocks.iter().filter(|block| block.dirty).count() as u64;
    let cached_blocks = inner.blocks.len() as u64;
    CacheStats {
        cached_blocks,
        clean_blocks: cached_blocks.saturating_sub(dirty_blocks),
        dirty_blocks,
        hits: inner.hits,
        misses: inner.misses,
        dirty_marks: inner.dirty_marks,
        data_dirty_marks: inner.data_dirty_marks,
        inode_dirty_marks: inner.inode_dirty_marks,
        root_dirty_marks: inner.root_dirty_marks,
        free_map_dirty_marks: inner.free_map_dirty_marks,
        superblock_dirty_marks: inner.superblock_dirty_marks,
        drain_batches: inner.drain_batches,
        drained_blocks: inner.drained_blocks,
        flushes: inner.flushes,
        flush_required: inner.flush_required,
        writeback_hold: inner.writeback_hold,
        evictions: inner.evictions,
    }
}

fn validate_device_range(
    device: &Arc<dyn BlockDevice>,
    offset: u64,
    input_len: usize,
) -> Result<(), NeoFsError> {
    let geometry = device.geometry();
    let logical = geometry.logical_block_size as usize;
    if logical == 0 || logical > FS_BLOCK_SIZE || !FS_BLOCK_SIZE.is_multiple_of(logical) {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    let device_bytes = geometry
        .block_count
        .checked_mul(geometry.logical_block_size as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    let end = offset
        .checked_add(input_len as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    if end > device_bytes {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }

    let first_block = offset / FS_BLOCK_SIZE as u64;
    let last_byte = end
        .checked_sub(1)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    let last_block = last_byte / FS_BLOCK_SIZE as u64;
    block_offset(first_block)?;
    block_offset(last_block)?;
    Ok(())
}

fn validate_fs_block(device: &Arc<dyn BlockDevice>, block: u64) -> Result<(), NeoFsError> {
    let geometry = device.geometry();
    let logical = geometry.logical_block_size as usize;
    if logical == 0 || logical > FS_BLOCK_SIZE || !FS_BLOCK_SIZE.is_multiple_of(logical) {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    let device_bytes = geometry
        .block_count
        .checked_mul(geometry.logical_block_size as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    let offset = block_offset(block)?;
    let end = offset
        .checked_add(FS_BLOCK_SIZE as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    if end > device_bytes {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    Ok(())
}
