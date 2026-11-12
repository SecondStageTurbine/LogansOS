// SPDX-License-Identifier: MPL-2.0
extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::block::{BlockDevice, BlockError, DmaBuffer};
use crate::fs::{FileRights, Inode, InodeKind, VfsDirEntry, VfsError};
#[cfg(any(
    feature = "neo-test-arthur-real-brain-boot-15b",
    feature = "neo-test-arthur-real-brain-boot-3b"
))]
use crate::serial_println;
use crate::sync::SwitchMutex;

use super::NeoFsError;
use super::alloc_logic;
#[cfg(feature = "neo-test")]
use super::cache::{CacheStats, WriteBackWorkerStopReport};
use super::cache::{
    CachedBlockRole, NeoFsBlockCache, spawn_writeback_worker, stop_writeback_worker,
};
use super::commit_logic::DeferredFrees;
use super::format::{
    DIRECT_BLOCKS, DIRENT_SIZE, DOUBLE_INDIRECT_BLOCKS, DirEntry, FS_BLOCK_SIZE, FreeMapRef,
    INDIRECT_POINTERS_PER_BLOCK, INODE_SIZE, InodeRecord, MAX_FILE_BLOCKS, MAX_FILE_SIZE,
    SINGLE_INDIRECT_BLOCKS, SUPERBLOCK_COPY_OFFSETS, SUPERBLOCK_SIZE, Superblock, block_offset,
    file_data_blocks, free_map_block_count_for_total, free_map_byte_len, hash,
    parse_directory_entries, parse_inode, write_dir_entry, write_inode, write_superblock,
};

// F0: mkdir_at, unlink_at, rename_at, stat_at.
#[cfg(feature = "neofs-dir-ops")]
mod dir_ops;
#[cfg(feature = "neo-test-neofs-dir-ops")]
pub(super) use dir_ops::STOP_NEXT_BEFORE_PUBLISH;

struct NeoFsMetadata {
    superblock: Superblock,
    free_map: Option<FreeMapState>,
    /// Blocks a commit freed that an older generation still names: not
    /// allocated until every superblock copy is at least the freeing
    /// generation (`commit_logic`, rule 2).
    deferred: DeferredFrees,
    /// The cache's durable superblock generation.
    durable_generation: Arc<AtomicU64>,
}

const BLOCK_IO_BUSY_RETRIES: usize = 10_000;

struct RootState {
    inode_block: u64,
    record: InodeRecord,
}

struct FreeMapState {
    bytes: Vec<u8>,
    dirty: bool,
    /// Where the map the current superblock names lives (`start`, `count`).
    /// A publish writes the map to new blocks, then frees these.
    on_disk: (u64, u64),
}

struct DirectoryFrame {
    name: Option<String>,
    /// The inode block this version of the directory lives in.
    inode_block: u64,
    record: InodeRecord,
    entries: Vec<DirEntry>,
}

type RuntimeIndirectBlock = (u64, Vec<u8>);
type RuntimeTripleLevel = (u64, Vec<u8>, Vec<RuntimeIndirectBlock>);

struct RuntimeFileBlockLayout {
    direct_blocks: [u64; DIRECT_BLOCKS],
    single_indirect_block: u64,
    double_indirect_block: u64,
    triple_indirect_block: u64,
    data_blocks: Vec<u64>,
    indirect_blocks: Vec<RuntimeIndirectBlock>,
}

impl RuntimeFileBlockLayout {
    /// Every block the layout reserved: the data blocks, then the indirect
    /// blocks. (A release marks each free, or rewinds the bump allocator
    /// when it was the last one taken.)
    fn reserved_blocks(&self) -> Vec<u64> {
        let mut blocks = self.data_blocks.clone();
        blocks.extend(self.indirect_blocks.iter().map(|(block, _)| *block));
        blocks
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct FreeMapStats {
    pub free_blocks: u64,
    pub map_blocks: u64,
    pub next_free_block: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GcStats {
    pub live_blocks: u64,
    pub reclaimed_blocks: u64,
    pub free_blocks: u64,
    pub map_start_block: u64,
}

pub(super) struct NeoFsState {
    pub(super) device: Arc<dyn BlockDevice>,
    cache: Arc<NeoFsBlockCache>,
    writable: bool,
    /// Process/kthread filesystem state; IRQ handlers never inspect NeoFS
    /// metadata directly, and block completions rendezvous through drivers.
    metadata: SwitchMutex<NeoFsMetadata>,
    /// Process/kthread root inode state; no interrupt-context acquirer.
    root: SwitchMutex<RootState>,
    /// Process/kthread directory entry state; no interrupt-context acquirer.
    entries: SwitchMutex<Vec<DirEntry>>,
    /// Process/kthread lifecycle handle; stop/join coordination lives in the
    /// kthread control block, not in an interrupt handler.
    writeback_worker: SwitchMutex<Option<crate::sched::kthread::KthreadHandle>>,
    /// NFS-W1: a large write's commit failed part-way. Nothing more is
    /// published through this mount (`publish_superblock`).
    #[cfg(feature = "neofs-large-write")]
    commit_failed: core::sync::atomic::AtomicBool,
    /// OT-90 P3a (K2): set by the shutdown before it drains. Every mutator
    /// counts itself in `writes_in_flight` and then checks this, and the
    /// shutdown sets this and then waits for that count to reach zero (both
    /// SeqCst): a mutator either sees the freeze and refuses, or the drain
    /// waits for it, so no write lands after the drain unseen.
    #[cfg(feature = "power-clean")]
    frozen: core::sync::atomic::AtomicBool,
    #[cfg(feature = "power-clean")]
    writes_in_flight: core::sync::atomic::AtomicU32,
}

/// OT-90 P3a: one mutator counted in [`NeoFsState`]'s `writes_in_flight`.
#[cfg(feature = "power-clean")]
pub(super) struct WriteOp<'a>(&'a core::sync::atomic::AtomicU32);

#[cfg(feature = "power-clean")]
impl Drop for WriteOp<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, core::sync::atomic::Ordering::SeqCst);
    }
}

/// OT-90 P3a (K2): what a mount's shutdown did.
#[cfg(feature = "power-clean")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShutdownReport {
    /// The mount was writable (a read-only one has nothing to drain).
    pub writable: bool,
    /// Every mutator in flight at the freeze finished within the bound.
    pub writers_settled: bool,
    /// Blocks the drain wrote.
    pub drained_blocks: u64,
    /// Blocks still dirty when it stopped (0 when clean).
    pub dirty_blocks: u64,
    /// The drain returned an error (a write or a flush failed).
    pub drain_error: bool,
    /// The writeback worker's exit code (0: it exited cleanly).
    pub worker_exit: u32,
}

#[cfg(feature = "power-clean")]
impl ShutdownReport {
    /// THINK §5 K2: clean only when the drain finished with nothing dirty
    /// and no error, after every writer settled.
    pub fn clean(&self) -> bool {
        !self.writable
            || (self.writers_settled
                && !self.drain_error
                && self.dirty_blocks == 0
                && self.worker_exit == 0)
    }
}

impl NeoFsState {
    pub(super) fn new(
        device: Arc<dyn BlockDevice>,
        superblock: Superblock,
        writable: bool,
        root_inode_block: u64,
        root_record: InodeRecord,
        entries: Vec<DirEntry>,
        free_map_bytes: Option<Vec<u8>>,
    ) -> Self {
        let on_disk = superblock
            .free_map
            .map_or((0, 0), |map| (map.start_block, map.block_count));
        let free_map = free_map_bytes.map(|bytes| FreeMapState {
            bytes,
            dirty: false,
            on_disk,
        });
        let cache = Arc::new(NeoFsBlockCache::new(device.clone(), superblock.generation));
        let durable_generation = cache.durable_generation_handle();
        let writeback_worker = if writable {
            Some(spawn_writeback_worker(cache.clone()))
        } else {
            None
        };
        Self {
            device,
            cache,
            writable,
            metadata: SwitchMutex::new(NeoFsMetadata {
                superblock,
                free_map,
                deferred: DeferredFrees::new(),
                durable_generation,
            }),
            root: SwitchMutex::new(RootState {
                inode_block: root_inode_block,
                record: root_record,
            }),
            entries: SwitchMutex::new(entries),
            writeback_worker: SwitchMutex::new(writeback_worker),
            #[cfg(feature = "neofs-large-write")]
            commit_failed: core::sync::atomic::AtomicBool::new(false),
            #[cfg(feature = "power-clean")]
            frozen: core::sync::atomic::AtomicBool::new(false),
            #[cfg(feature = "power-clean")]
            writes_in_flight: core::sync::atomic::AtomicU32::new(0),
        }
    }

    pub(super) fn is_writable(&self) -> bool {
        self.writable
    }

    /// OT-90 P3a: count a mutator in, unless the mount is frozen for
    /// shutdown (then `ReadOnly`, as an unwritable mount answers).
    #[cfg(feature = "power-clean")]
    fn begin_write(&self) -> Result<WriteOp<'_>, NeoFsError> {
        use core::sync::atomic::Ordering::SeqCst;
        self.writes_in_flight.fetch_add(1, SeqCst);
        let op = WriteOp(&self.writes_in_flight);
        if self.frozen.load(SeqCst) {
            return Err(NeoFsError::ReadOnly);
        }
        Ok(op)
    }

    /// OT-90 P3a, THINK §5 K2: freeze writes, wait for the mutators in
    /// flight, drain the cache with a no-progress bound of `stall_ticks`
    /// timer ticks, then stop the writeback worker (without its own unbounded
    /// drain). Never hangs on a stuck device: a drain that makes no progress
    /// for the bound stops and reports what is still dirty.
    #[cfg(feature = "power-clean")]
    pub(super) fn shutdown(&self, stall_ticks: u64) -> ShutdownReport {
        use core::sync::atomic::Ordering::SeqCst;
        let mut report = ShutdownReport {
            writable: self.writable,
            writers_settled: true,
            drained_blocks: 0,
            dirty_blocks: 0,
            drain_error: false,
            worker_exit: 0,
        };
        if !self.writable {
            return report;
        }
        self.frozen.store(true, SeqCst);
        let ticks =
            || crate::arch::x86_64::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
        let start = ticks();
        while self.writes_in_flight.load(SeqCst) != 0 {
            if ticks().saturating_sub(start) > stall_ticks {
                report.writers_settled = false;
                break;
            }
            crate::sched::yield_now();
        }
        match self.cache.shutdown_drain(stall_ticks) {
            Ok((drained, dirty)) => {
                report.drained_blocks = drained;
                report.dirty_blocks = dirty;
            }
            Err(_) => {
                report.drain_error = true;
                report.dirty_blocks = self.cache.dirty_count();
            }
        }
        self.cache.mark_shutdown_drained();
        let stop = stop_writeback_worker(&self.cache, &self.writeback_worker);
        report.worker_exit = if !stop.had_worker || stop.exited_success {
            0
        } else {
            stop.exit_code.max(1)
        };
        report
    }

    pub(super) fn total_blocks(&self) -> u64 {
        self.metadata.lock().superblock.total_blocks
    }

    pub(super) fn generation(&self) -> u64 {
        self.metadata.lock().superblock.generation
    }

    pub(super) fn root_snapshot(&self) -> (u64, InodeRecord) {
        let root = self.root.lock();
        (root.inode_block, root.record.clone())
    }

    pub(super) fn resolve_path(&self, path: &str) -> Result<(u64, InodeRecord), NeoFsError> {
        let parts = split_relative_path(path)?;
        let mut entries = self.entries.lock().clone();
        for (idx, part) in parts.iter().enumerate() {
            let entry = entries
                .iter()
                .find(|entry| entry.name == *part)
                .cloned()
                .ok_or(NeoFsError::NotFound)?;
            let (record, _bytes) = self.read_cached_inode(entry.child_block)?;
            if idx + 1 == parts.len() {
                return Ok((entry.child_block, record));
            }
            if record.kind != super::format::NeoFsKind::Directory {
                return Err(NeoFsError::NotFound);
            }
            entries = self.read_cached_directory_entries(&record)?;
        }
        Err(NeoFsError::NotFound)
    }

    pub(super) fn current_child_generation(&self, name: &str) -> Result<u32, VfsError> {
        let (_block, record) = self.resolve_path(name).map_err(|_| VfsError::Stale)?;
        Ok(record.generation)
    }

    pub(super) fn read_cached_inode(
        &self,
        block: u64,
    ) -> Result<(InodeRecord, [u8; INODE_SIZE]), NeoFsError> {
        read_inode_from_cache(&self.cache, block, self.total_blocks())
    }

    pub(super) fn create_regular(&self, name: String) -> Result<(u64, InodeRecord), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;

        let parts = split_relative_path(&name)?;
        let (leaf, parent_parts) = split_leaf(&parts)?;
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut root_entries = self.entries.lock();
        let mut frames = self.load_directory_chain(&root_entries, parent_parts)?;
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        if parent.entries.iter().any(|entry| entry.name == leaf) {
            return Err(NeoFsError::InvalidDirectory);
        }
        check_directory_capacity(parent.entries.len() + 1)?;

        let (inode_block, generation) = self.reserve_inode_block_and_generation()?;
        let record = InodeRecord {
            kind: super::format::NeoFsKind::Regular,
            generation,
            file_size: 0,
            block_count: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            content_hash: hash(&[]),
            available_rights: FileRights::from_bits(
                FileRights::READ.bits() | FileRights::WRITE.bits(),
            ),
            directory_entry_count: 0,
            fs_block_count: self.total_blocks(),
        };
        write_inode_to_cache(
            &self.cache,
            inode_block,
            self.total_blocks(),
            &record,
            CachedBlockRole::Inode,
        )?;

        parent.entries.push(DirEntry {
            name: leaf.into(),
            child_block: inode_block,
            kind: super::format::NeoFsKind::Regular,
        });
        self.commit_directory_frames(&mut root_entries, &mut frames, &[])?;
        Ok((inode_block, record))
    }

    /// NFS-W1: create the regular file `name` holding `bytes`, any size to
    /// the format's 2 GiB limit, in one piece.
    ///
    /// - The data blocks and the pointer blocks (`tree_logic`) are allocated
    ///   as one contiguous run and written in runs of [`LARGE_WRITE_RUN`]
    ///   straight to the mount's device: through the write guard for an
    ///   armed volume, or through the installer's claim. The cache is
    ///   bypassed (and forgets any copy of those blocks).
    /// - The content BLAKE3 is streamed over the runs.
    /// - The device is flushed, then the inode and its directory entry are
    ///   committed once, through the ordinary path. Nothing references the
    ///   new blocks before that commit, so a stop or a cut before it leaves
    ///   the old tree, and the allocation (held only in memory until the
    ///   commit publishes it) is gone after a remount.
    ///
    /// Memory: one run buffer and its DMA copy (2 × 64 KiB) plus one pointer
    /// block (4 KiB), whatever the file's size. The namespace lock is held
    /// for the whole write, IF on (CP-56): it parks, it never spins.
    #[cfg(feature = "neofs-large-write")]
    pub(super) fn create_file_from_bytes(
        &self,
        name: String,
        bytes: &[u8],
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        use super::tree_logic::{Tree, pointer_blocks};
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        if bytes.len() > MAX_FILE_SIZE {
            return Err(NeoFsError::FileTooLarge);
        }
        let parts = split_relative_path(&name)?;
        let (leaf, parent_parts) = split_leaf(&parts)?;
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut root_entries = self.entries.lock();
        let mut frames = self.load_directory_chain(&root_entries, parent_parts)?;
        {
            let parent = frames.last().ok_or(NeoFsError::InvalidDirectory)?;
            if parent.entries.iter().any(|entry| entry.name == leaf) {
                return Err(NeoFsError::InvalidDirectory);
            }
            check_directory_capacity(parent.entries.len() + 1)?;
        }

        let n = file_data_blocks(bytes.len() as u64)?;
        let k = pointer_blocks(n);
        let run = if n + k == 0 {
            None
        } else {
            Some(allocate_run(&mut self.metadata.lock(), n + k)?)
        };
        let base = run.map_or(0, |r| r.base);
        #[cfg(feature = "neo-test-neofs-large-write")]
        LAST_LARGE_WRITE_RUN.store(base << 32 | (n + k), core::sync::atomic::Ordering::Release);
        // Every error from here to the commit gives the run (and the inode
        // block, once reserved) back: nothing references them yet, and a
        // later commit on this mount would otherwise publish them as used
        // (Chad's NFS-W1 review, item 2). The writeback pause ends with
        // `_writeback_pause` on every path.
        let staged = self.write_large_run(
            &Tree {
                n,
                data: base,
                ptr: base + n,
            },
            bytes,
        );
        let staged = staged.and_then(|content_hash| {
            let inode = self.reserve_inode_block_and_generation()?;
            Ok((content_hash, inode))
        });
        let (content_hash, (inode_block, generation)) = match staged {
            Ok(v) => v,
            Err(e) => {
                if let Some(run) = run {
                    release_run(&mut self.metadata.lock(), run);
                }
                return Err(e);
            }
        };
        let tree = Tree {
            n,
            data: base,
            ptr: base + n,
        };
        let (single, double, triple) = tree.roots();
        let record = InodeRecord {
            kind: super::format::NeoFsKind::Regular,
            generation,
            file_size: bytes.len() as u64,
            block_count: n,
            direct_blocks: tree.direct(),
            single_indirect_block: single,
            double_indirect_block: double,
            triple_indirect_block: triple,
            content_hash,
            available_rights: FileRights::from_bits(
                FileRights::READ.bits() | FileRights::WRITE.bits(),
            ),
            directory_entry_count: 0,
            fs_block_count: self.total_blocks(),
        };
        if let Err(e) = write_inode_to_cache(
            &self.cache,
            inode_block,
            self.total_blocks(),
            &record,
            CachedBlockRole::Inode,
        ) {
            let mut metadata = self.metadata.lock();
            release_run(
                &mut metadata,
                Run {
                    base: inode_block,
                    count: 1,
                },
            );
            if let Some(run) = run {
                release_run(&mut metadata, run);
            }
            return Err(e);
        }
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        parent.entries.push(DirEntry {
            name: leaf.into(),
            child_block: inode_block,
            kind: super::format::NeoFsKind::Regular,
        });
        // A failed commit may have written part of the new tree: the mount
        // is marked failed, and nothing more commits through it.
        if let Err(e) = self.commit_directory_frames(&mut root_entries, &mut frames, &[]) {
            self.commit_failed
                .store(true, core::sync::atomic::Ordering::Release);
            crate::serial_println!(
                "[ kernel ] neofs: large write commit failed {:?}; this mount commits nothing more",
                e
            );
            return Err(e);
        }
        crate::serial_println!(
            "[ kernel ] neofs: large write name={} bytes={} data_blocks={} pointer_blocks={} first_block={}",
            name,
            bytes.len(),
            n,
            k,
            base
        );
        Ok((inode_block, record))
    }

    /// NFS-W1: the data in runs of [`LARGE_WRITE_RUN`], then the pointer
    /// blocks, straight to the device, then a flush. Returns the content
    /// BLAKE3.
    #[cfg(feature = "neofs-large-write")]
    fn write_large_run(
        &self,
        tree: &super::tree_logic::Tree,
        bytes: &[u8],
    ) -> Result<[u8; 32], NeoFsError> {
        let k = super::tree_logic::pointer_blocks(tree.n);
        self.cache.forget_range(tree.data, tree.n + k);
        let mut hasher = blake3::Hasher::new();
        let mut run = vec![0u8; LARGE_WRITE_RUN];
        let mut done = 0usize;
        while done < bytes.len() {
            let take = cmp::min(bytes.len() - done, LARGE_WRITE_RUN);
            hasher.update(&bytes[done..done + take]);
            let padded = take.div_ceil(FS_BLOCK_SIZE) * FS_BLOCK_SIZE;
            run[..take].copy_from_slice(&bytes[done..done + take]);
            run[take..padded].fill(0);
            #[cfg(feature = "neo-test-neofs-large-write")]
            if LARGE_WRITE_FAIL_FIRST_WRITE.swap(false, core::sync::atomic::Ordering::AcqRel) {
                crate::serial_println!(
                    "[ kernel ] neofs: large write failed its first data write (test hook)"
                );
                return Err(NeoFsError::BlockDevice);
            }
            write_fs_blocks(
                &self.device,
                tree.data + (done / FS_BLOCK_SIZE) as u64,
                &run[..padded],
            )?;
            done += take;
        }
        let mut pointers = alloc::boxed::Box::new([0u8; FS_BLOCK_SIZE]);
        for i in 0..k {
            let block = tree
                .pointer_block_into(i, &mut pointers)
                .ok_or(NeoFsError::InvalidInode)?;
            write_fs_blocks(&self.device, block, &pointers[..])?;
        }
        if self.device.supports_flush() {
            retry_busy_block_io(|| self.device.flush())?;
        }
        #[cfg(feature = "neo-test-neofs-large-write")]
        if LARGE_WRITE_STOP_BEFORE_COMMIT.swap(false, core::sync::atomic::Ordering::AcqRel) {
            crate::serial_println!(
                "[ kernel ] neofs: large write stopped before commit (test hook)"
            );
            return Err(NeoFsError::BlockDevice);
        }
        Ok(*hasher.finalize().as_bytes())
    }

    /// OT-87 R4.3: an empty directory at `name` (its parent must exist), for
    /// the installer's `/deploy/A/`. The same shape as `neofs-mkfs`'s empty
    /// `subdir`: a directory inode with no data blocks.
    #[cfg(feature = "installer")]
    pub(super) fn create_directory(&self, name: String) -> Result<(u64, InodeRecord), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        let parts = split_relative_path(&name)?;
        let (leaf, parent_parts) = split_leaf(&parts)?;
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut root_entries = self.entries.lock();
        let mut frames = self.load_directory_chain(&root_entries, parent_parts)?;
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        if parent.entries.iter().any(|entry| entry.name == leaf) {
            return Err(NeoFsError::InvalidDirectory);
        }
        check_directory_capacity(parent.entries.len() + 1)?;

        let (inode_block, generation) = self.reserve_inode_block_and_generation()?;
        let record = InodeRecord {
            kind: super::format::NeoFsKind::Directory,
            generation,
            file_size: 0,
            block_count: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            content_hash: hash(&[]),
            available_rights: FileRights::from_bits(
                FileRights::READ.bits() | FileRights::WRITE.bits(),
            ),
            directory_entry_count: 0,
            fs_block_count: self.total_blocks(),
        };
        write_inode_to_cache(
            &self.cache,
            inode_block,
            self.total_blocks(),
            &record,
            CachedBlockRole::Inode,
        )?;
        parent.entries.push(DirEntry {
            name: leaf.into(),
            child_block: inode_block,
            kind: super::format::NeoFsKind::Directory,
        });
        self.commit_directory_frames(&mut root_entries, &mut frames, &[])?;
        Ok((inode_block, record))
    }

    /// OT-87 R4.3: grow the volume to `total_blocks` (the installer's W4: the
    /// seed, then the whole partition). The free map grows to cover it and is
    /// published at a new contiguous run, with the superblock copies, in one
    /// generation; the old map's blocks are left unreferenced (a leak of a
    /// block, not an inconsistency: they are neither live nor marked free).
    /// The device must already hold `total_blocks`.
    #[cfg(feature = "installer")]
    pub(super) fn grow(&self, total_blocks: u64) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        let geometry = self.device.geometry();
        let device_blocks = geometry
            .block_count
            .checked_mul(geometry.logical_block_size as u64)
            .ok_or(NeoFsError::InvalidDeviceGeometry)?
            / FS_BLOCK_SIZE as u64;
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let _entries = self.entries.lock();
        let mut metadata = self.metadata.lock();
        let current = metadata.superblock.total_blocks;
        if total_blocks < current || total_blocks > device_blocks {
            return Err(NeoFsError::InvalidSuperblock);
        }
        if total_blocks == current {
            return Ok(());
        }
        let block_count = super::format::free_map_block_count_for_total(total_blocks)?;
        let len = super::format::free_map_byte_len(block_count)?;
        let free_map = metadata
            .free_map
            .as_mut()
            .ok_or(NeoFsError::UnsupportedType)?;
        free_map.bytes.resize(len, 0);
        free_map.dirty = true;
        let old = metadata
            .superblock
            .free_map
            .ok_or(NeoFsError::UnsupportedType)?;
        metadata.superblock.total_blocks = total_blocks;
        metadata.superblock.free_map = Some(FreeMapRef { block_count, ..old });
        bump_superblock_generation(&mut metadata.superblock)?;
        self.publish_superblock(&mut metadata)
    }

    pub(super) fn commit_regular_write(
        &self,
        name: &str,
        old_inode_block: u64,
        old_record: &InodeRecord,
        offset: usize,
        input: &[u8],
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;

        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();

        let new_content = build_cow_content_cached(&self.cache, old_record, offset, input)?;
        let required_blocks = if new_content.is_empty() {
            0
        } else {
            new_content.len().div_ceil(FS_BLOCK_SIZE)
        };
        if required_blocks > DIRECT_BLOCKS {
            return Err(NeoFsError::FileTooLarge);
        }

        // The version this write replaces: freed by the publish below.
        let old_tree_blocks = if cfg!(feature = "neofs-leak-superseded") {
            Vec::new()
        } else {
            collect_regular_tree_blocks_cached(&self.cache, old_inode_block, old_record)?
        };
        let data_blocks = self.reserve_blocks(required_blocks)?;
        let mut reserved = data_blocks.clone();
        self.or_release(
            write_content_blocks_cached(&self.cache, &data_blocks, &new_content),
            &reserved,
        )?;

        let (new_inode_block, generation) =
            self.or_release(self.reserve_inode_block_and_generation(), &reserved)?;
        reserved.push(new_inode_block);
        let mut new_record = old_record.clone();
        new_record.generation = generation;
        new_record.file_size = new_content.len() as u64;
        new_record.block_count = required_blocks as u64;
        new_record.direct_blocks = [0; DIRECT_BLOCKS];
        new_record.single_indirect_block = 0;
        new_record.double_indirect_block = 0;
        new_record.triple_indirect_block = 0;
        for (idx, block) in data_blocks.iter().copied().enumerate() {
            new_record.direct_blocks[idx] = block;
        }
        new_record.content_hash = hash(&new_content);
        // The volume's size now, not when the old record was read: after a
        // grow, the new blocks may lie past the old end.
        new_record.fs_block_count = self.total_blocks();
        self.or_release(
            write_inode_to_cache(
                &self.cache,
                new_inode_block,
                self.total_blocks(),
                &new_record,
                CachedBlockRole::Inode,
            ),
            &reserved,
        )?;

        self.commit_child_update(
            name,
            (old_inode_block, old_record.generation),
            new_inode_block,
            super::format::NeoFsKind::Regular,
            &old_tree_blocks,
            &reserved,
        )?;
        Ok((new_inode_block, new_record))
    }

    pub(super) fn commit_regular_truncate(
        &self,
        name: &str,
        old_inode_block: u64,
        old_record: &InodeRecord,
        new_len: u64,
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        if new_len > MAX_FILE_SIZE as u64 {
            return Err(NeoFsError::FileTooLarge);
        }

        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();

        let new_len_usize = usize::try_from(new_len).map_err(|_| NeoFsError::FileTooLarge)?;
        let copy_len = cmp::min(old_record.file_size, new_len);
        let copy_len_usize = usize::try_from(copy_len).map_err(|_| NeoFsError::FileTooLarge)?;
        let mut new_content = read_logical_prefix_cached(&self.cache, old_record, copy_len_usize)?;
        if new_content.len() < new_len_usize {
            new_content.resize(new_len_usize, 0);
        }
        let old_tree_blocks =
            collect_regular_tree_blocks_cached(&self.cache, old_inode_block, old_record)?;

        let required_blocks = file_data_blocks(new_len)?;
        let layout = self.allocate_file_block_layout(required_blocks)?;
        let mut reserved = layout.reserved_blocks();
        self.or_release(
            write_content_blocks_cached(&self.cache, &layout.data_blocks, &new_content)
                .and_then(|()| write_indirect_blocks_cached(&self.cache, &layout.indirect_blocks)),
            &reserved,
        )?;

        let (new_inode_block, generation) =
            self.or_release(self.reserve_inode_block_and_generation(), &reserved)?;
        reserved.push(new_inode_block);
        let mut new_record = old_record.clone();
        new_record.generation = generation;
        new_record.file_size = new_len;
        new_record.block_count = required_blocks;
        new_record.direct_blocks = layout.direct_blocks;
        new_record.single_indirect_block = layout.single_indirect_block;
        new_record.double_indirect_block = layout.double_indirect_block;
        new_record.triple_indirect_block = layout.triple_indirect_block;
        new_record.content_hash = hash(&new_content);
        // The volume's size now, not when the old record was read: after a
        // grow, the new blocks may lie past the old end.
        new_record.fs_block_count = self.total_blocks();
        self.or_release(
            write_inode_to_cache(
                &self.cache,
                new_inode_block,
                self.total_blocks(),
                &new_record,
                CachedBlockRole::Inode,
            ),
            &reserved,
        )?;

        self.commit_child_update(
            name,
            (old_inode_block, old_record.generation),
            new_inode_block,
            super::format::NeoFsKind::Regular,
            &old_tree_blocks,
            &reserved,
        )?;
        Ok((new_inode_block, new_record))
    }

    /// W1's byte oracle reads the device: everything cached is written first.
    #[cfg(any(
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub(super) fn debug_drain_all(&self) -> Result<(), NeoFsError> {
        let _entries = self.entries.lock();
        self.cache.drain_all()?;
        Ok(())
    }

    /// W1's cut scenario: everything a replace writes, but no publish.
    #[cfg(feature = "neo-test-neofs-file-replace")]
    pub(super) fn debug_replace_before_publish(
        &self,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), NeoFsError> {
        let (old_inode_block, old_record) = self.resolve_path(name)?;
        self.commit_regular_replace(name, old_inode_block, &old_record, bytes, true)
            .map(|_| ())
    }

    /// OT-67 W1: `bytes` become the file's whole content in one commit.
    /// Truncate's copy-on-write path with the caller's bytes: a new tree
    /// through the indirect layout, a new inode, one publish (the directory
    /// entry's compare-and-swap) that frees the old tree, deferred.
    /// `stop_before_publish` (the cut scenario) writes everything but the
    /// publish.
    #[cfg(feature = "neofs-file-replace")]
    pub(super) fn commit_regular_replace(
        &self,
        name: &str,
        old_inode_block: u64,
        old_record: &InodeRecord,
        bytes: &[u8],
        stop_before_publish: bool,
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        if bytes.len() > MAX_FILE_SIZE {
            return Err(NeoFsError::FileTooLarge);
        }

        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();

        let old_tree_blocks =
            collect_regular_tree_blocks_cached(&self.cache, old_inode_block, old_record)?;
        let required_blocks = file_data_blocks(bytes.len() as u64)?;
        let layout = self.allocate_file_block_layout(required_blocks)?;
        let mut reserved = layout.reserved_blocks();
        self.or_release(
            write_content_blocks_cached(&self.cache, &layout.data_blocks, bytes)
                .and_then(|()| write_indirect_blocks_cached(&self.cache, &layout.indirect_blocks)),
            &reserved,
        )?;

        let (new_inode_block, generation) =
            self.or_release(self.reserve_inode_block_and_generation(), &reserved)?;
        reserved.push(new_inode_block);
        let mut new_record = old_record.clone();
        new_record.generation = generation;
        new_record.file_size = bytes.len() as u64;
        new_record.block_count = required_blocks;
        new_record.direct_blocks = layout.direct_blocks;
        new_record.single_indirect_block = layout.single_indirect_block;
        new_record.double_indirect_block = layout.double_indirect_block;
        new_record.triple_indirect_block = layout.triple_indirect_block;
        new_record.content_hash = hash(bytes);
        // The volume's size now, not when the old record was read: after a
        // grow, the new blocks may lie past the old end (as in truncate).
        new_record.fs_block_count = self.total_blocks();
        self.or_release(
            write_inode_to_cache(
                &self.cache,
                new_inode_block,
                self.total_blocks(),
                &new_record,
                CachedBlockRole::Inode,
            ),
            &reserved,
        )?;
        if stop_before_publish {
            return Ok((new_inode_block, new_record));
        }

        self.commit_child_update(
            name,
            (old_inode_block, old_record.generation),
            new_inode_block,
            super::format::NeoFsKind::Regular,
            &old_tree_blocks,
            &reserved,
        )?;
        Ok((new_inode_block, new_record))
    }

    /// OT-92: before a mutation, drain the mount if the cache holds
    /// `DIRTY_THROTTLE_BLOCKS` dirty blocks or more, so dirty blocks (which
    /// eviction never takes) stay bounded. The drain is fsync's: under the
    /// entries lock, so it never interleaves with a namespace commit, and
    /// before this mutation takes any lock. Off (`neofs-cache-evict`
    /// disabled): never drains.
    fn throttle_dirty(&self) -> Result<(), NeoFsError> {
        if self.cache.dirty_over_throttle() {
            let _entries = self.entries.lock();
            self.cache.drain_all()?;
        }
        Ok(())
    }

    pub(super) fn fsync_regular(&self) -> Result<(), NeoFsError> {
        if !self.writable {
            return Ok(());
        }

        // Conservative BUILD-3E fsync: mount-wide drain, not per-file dirty
        // attribution. NeoFS does not yet attribute dirty cache state per file,
        // so fsync serializes with namespace mutation and drains the whole mount
        // through the same ordered path as write-back/GC.
        let _entries = self.entries.lock();
        self.cache.drain_all()?;
        Ok(())
    }

    pub(super) fn debug_regular_blocks(
        &self,
        name: &str,
    ) -> Result<([u64; DIRECT_BLOCKS], u64), NeoFsError> {
        let (_block, inode) = self.resolve_path(name)?;
        if inode.kind != super::format::NeoFsKind::Regular {
            return Err(NeoFsError::UnsupportedType);
        }
        Ok((inode.direct_blocks, inode.block_count))
    }

    fn read_cached_directory_entries(
        &self,
        record: &InodeRecord,
    ) -> Result<Vec<DirEntry>, NeoFsError> {
        if record.kind != super::format::NeoFsKind::Directory {
            return Err(NeoFsError::InvalidDirectory);
        }
        let content = read_verified_logical_content_cached(&self.cache, record)?;
        parse_directory_entries(record, &content)
    }

    fn load_directory_chain(
        &self,
        root_entries: &[DirEntry],
        parent_parts: &[&str],
    ) -> Result<Vec<DirectoryFrame>, NeoFsError> {
        let (root_inode_block, root_record) = self.root_snapshot();
        let mut frames = Vec::new();
        frames.push(DirectoryFrame {
            name: None,
            inode_block: root_inode_block,
            record: root_record,
            entries: root_entries.to_vec(),
        });

        for part in parent_parts {
            let current = frames.last().ok_or(NeoFsError::InvalidDirectory)?;
            let entry = current
                .entries
                .iter()
                .find(|entry| entry.name == *part)
                .cloned()
                .ok_or(NeoFsError::NotFound)?;
            let (record, _bytes) = self.read_cached_inode(entry.child_block)?;
            if record.kind != super::format::NeoFsKind::Directory {
                return Err(NeoFsError::InvalidDirectory);
            }
            let entries = self.read_cached_directory_entries(&record)?;
            frames.push(DirectoryFrame {
                name: Some((*part).into()),
                inode_block: entry.child_block,
                record,
                entries,
            });
        }
        Ok(frames)
    }

    /// Point `path`'s entry at `new_child_block` and commit, freeing
    /// `superseded` (the old child's blocks) in the same publish. The entry
    /// must still name the version the caller read: its inode block AND its
    /// generation. The block alone isn't enough: first-fit reuse can hand a
    /// later version the same inode block, and then `superseded` would name
    /// that version's live blocks (#206 AAG, U1). `NotFound` when it moved.
    fn commit_child_update(
        &self,
        path: &str,
        // The version the caller read: its inode block and generation.
        (expected_old_block, expected_generation): (u64, u32),
        new_child_block: u64,
        kind: super::format::NeoFsKind,
        superseded: &[u64],
        reserved: &[u64],
    ) -> Result<(), NeoFsError> {
        let parts = split_relative_path(path)?;
        let (leaf, parent_parts) = split_leaf(&parts)?;
        let mut root_entries = self.entries.lock();
        let mut frames = match self.current_child_frames(
            &root_entries,
            parent_parts,
            leaf,
            expected_old_block,
            expected_generation,
        ) {
            Ok(frames) => frames,
            Err(err) => {
                // Refused before anything changed: the caller's new tree
                // and inode were never published, so they go straight back
                // to free (#206 AAG, U3).
                self.release_unpublished(reserved);
                return Err(err);
            }
        };
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        let entry = parent
            .entries
            .iter_mut()
            .find(|entry| entry.name == leaf)
            .ok_or(NeoFsError::NotFound)?;
        entry.child_block = new_child_block;
        entry.kind = kind;
        self.commit_directory_frames(&mut root_entries, &mut frames, superseded)
    }

    /// The directory chain down to `leaf`'s parent, when `leaf` still names
    /// the version the caller read: the same inode block and generation.
    /// `NotFound` otherwise.
    fn current_child_frames(
        &self,
        root_entries: &[DirEntry],
        parent_parts: &[&str],
        leaf: &str,
        expected_old_block: u64,
        expected_generation: u32,
    ) -> Result<Vec<DirectoryFrame>, NeoFsError> {
        let frames = self.load_directory_chain(root_entries, parent_parts)?;
        let parent = frames.last().ok_or(NeoFsError::InvalidDirectory)?;
        let entry = parent
            .entries
            .iter()
            .find(|entry| entry.name == leaf)
            .ok_or(NeoFsError::NotFound)?;
        if entry.child_block != expected_old_block {
            return Err(NeoFsError::NotFound);
        }
        let (current, _bytes) = self.read_cached_inode(expected_old_block)?;
        if current.generation != expected_generation {
            return Err(NeoFsError::NotFound);
        }
        Ok(frames)
    }

    /// `result`, giving back `reserved` first when it is an error: a write,
    /// truncate or replace that fails between reserving its blocks and its
    /// commit (a cache write, the inode's reservation) releases what it holds.
    ///
    /// **Not covered** (#213 AAG): a failure partway through reserving the
    /// blocks themselves (`reserve_blocks` or `allocate_file_block_layout`
    /// running out of space after taking some), and anything from
    /// `commit_directory_frames` on, the directory writes and the publish. A
    /// failed publish is #206's U3 note. Those leak until a sweep.
    fn or_release<T>(
        &self,
        result: Result<T, NeoFsError>,
        reserved: &[u64],
    ) -> Result<T, NeoFsError> {
        if result.is_err() {
            self.release_unpublished(reserved);
        }
        result
    }

    /// Give back blocks a commit reserved and never published, newest first
    /// so the bump allocator can rewind. They were free in every published
    /// map and nothing references them, so no deferral is needed.
    fn release_unpublished(&self, reserved: &[u64]) {
        if cfg!(feature = "neofs-keep-refused-reservations") {
            return;
        }
        let mut metadata = self.metadata.lock();
        for block in reserved.iter().rev().copied() {
            let _ = alloc_logic::release_run(&mut *metadata, block, 1);
        }
    }

    /// Write a new version of every directory in `frames`, then publish the
    /// new root. The publish frees what it supersedes: `superseded` (a
    /// replaced child's blocks) and every old directory version here.
    fn commit_directory_frames(
        &self,
        root_entries: &mut Vec<DirEntry>,
        frames: &mut [DirectoryFrame],
        superseded: &[u64],
    ) -> Result<(), NeoFsError> {
        let mut superseded = superseded.to_vec();
        if !cfg!(feature = "neofs-leak-superseded") {
            for frame in frames.iter() {
                collect_directory_blocks(frame.inode_block, &frame.record, &mut superseded)?;
            }
        }
        let mut child_update: Option<(String, u64, super::format::NeoFsKind)> = None;
        for idx in (0..frames.len()).rev() {
            if let Some((name, child_block, kind)) = child_update.take() {
                let entry = frames[idx]
                    .entries
                    .iter_mut()
                    .find(|entry| entry.name == name)
                    .ok_or(NeoFsError::NotFound)?;
                entry.child_block = child_block;
                entry.kind = kind;
            }

            if idx == 0 {
                self.commit_root_entries(&frames[idx].entries, &superseded)?;
                *root_entries = frames[idx].entries.clone();
                return Ok(());
            }

            let (dir_block, _record) = self
                .write_directory_tree_without_publish(&frames[idx].record, &frames[idx].entries)?;
            let name = frames[idx]
                .name
                .clone()
                .ok_or(NeoFsError::InvalidDirectory)?;
            child_update = Some((name, dir_block, super::format::NeoFsKind::Directory));
        }
        Err(NeoFsError::InvalidDirectory)
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_cache_stats(&self) -> CacheStats {
        self.cache.debug_stats()
    }

    #[cfg(all(feature = "neo-test", feature = "neofs-large-write"))]
    pub(super) fn debug_forget_range(&self, first: u64, count: u64) {
        self.cache.debug_forget_range(first, count);
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_set_writeback_hold(&self, hold: bool) {
        self.cache.debug_set_writeback_hold(hold);
    }

    /// Take `entries`, set `gate` to `ENTRIES_GATE_HELD`, and keep the lock
    /// for `ticks` ticks while yielding with interrupts on: the state of a
    /// `sys_write` preempted inside `commit_child_update`. Just before the
    /// release it swaps `gate` to `ENTRIES_GATE_RELEASED`. Returns the tick
    /// just before the release, and whether a caller had already claimed the
    /// gate (`ENTRIES_GATE_CALLING`), i.e. began its call while the lock was
    /// still held.
    #[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
    pub(super) fn debug_hold_entries_yielding(
        &self,
        ticks: u64,
        gate: &core::sync::atomic::AtomicU8,
    ) -> (u64, bool) {
        use core::sync::atomic::Ordering;
        let tick = || crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
        let entries = self.entries.lock();
        let until = tick().saturating_add(ticks);
        gate.store(crate::fs::ENTRIES_GATE_HELD, Ordering::SeqCst);
        crate::arch::x86_64::with_interrupts_enabled(|| {
            while tick() < until {
                crate::sched::yield_now();
            }
        });
        let released = tick();
        let before = gate.swap(crate::fs::ENTRIES_GATE_RELEASED, Ordering::SeqCst);
        drop(entries);
        (released, before == crate::fs::ENTRIES_GATE_CALLING)
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_wait_clean_cache(&self, max_yields: usize) -> bool {
        self.cache.debug_wait_clean(max_yields)
    }

    #[cfg(feature = "neo-test")]
    pub(super) fn debug_stop_writeback_worker(&self) -> WriteBackWorkerStopReport {
        stop_writeback_worker(&self.cache, &self.writeback_worker)
    }

    #[cfg(any(
        feature = "neo-test-phase3-neofs-3c",
        feature = "neo-test-neofs-large-write",
        feature = "neo-test-neofs-overwrite-frees",
        feature = "neo-test-neofs-grow-write",
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub(super) fn debug_free_map_stats(&self) -> Result<FreeMapStats, NeoFsError> {
        let metadata = self.metadata.lock();
        let free_map = metadata
            .free_map
            .as_ref()
            .ok_or(NeoFsError::UnsupportedType)?;
        let map_ref = metadata
            .superblock
            .free_map
            .ok_or(NeoFsError::InvalidSuperblock)?;
        Ok(FreeMapStats {
            free_blocks: count_free_blocks(&free_map.bytes, metadata.superblock.total_blocks)?,
            map_blocks: map_ref.block_count,
            next_free_block: metadata.superblock.next_free_block,
        })
    }

    #[cfg(feature = "neo-test-phase3-neofs-3c")]
    pub(super) fn debug_run_quiescent_gc(&self) -> Result<GcStats, NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        let _entries = self.entries.lock();
        self.cache.drain_all()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut metadata = self.metadata.lock();
        self.run_gc_locked(&mut metadata)
    }

    #[cfg(feature = "neo-test-phase3-neofs-3c")]
    pub(super) fn debug_inject_gc_before_superblock(&self) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        let _entries = self.entries.lock();
        let _writeback_pause = self.cache.pause_worker();
        let mut metadata = self.metadata.lock();
        let previous_free = {
            let free_map = metadata
                .free_map
                .as_ref()
                .ok_or(NeoFsError::UnsupportedType)?;
            count_free_blocks(&free_map.bytes, metadata.superblock.total_blocks)?
        };
        self.cache.drain_all()?;
        let (live, _live_count) = collect_live_blocks(&self.device, &metadata.superblock)?;
        let new_map = build_swept_free_map(&metadata.superblock, &live)?;
        let free_map = metadata
            .free_map
            .as_mut()
            .ok_or(NeoFsError::UnsupportedType)?;
        free_map.bytes = new_map;
        free_map.dirty = true;
        let block_count = metadata
            .superblock
            .free_map
            .ok_or(NeoFsError::InvalidSuperblock)?
            .block_count;
        let start_block = allocate_contiguous_blocks(&mut metadata, block_count)?;
        let map_bytes = metadata
            .free_map
            .as_ref()
            .ok_or(NeoFsError::InvalidSuperblock)?
            .bytes
            .clone();
        if count_free_blocks(&map_bytes, metadata.superblock.total_blocks)? <= previous_free {
            return Err(NeoFsError::NoSpace);
        }
        write_free_map_blocks_cached(&self.cache, start_block, block_count, &map_bytes)?;
        Ok(())
    }

    #[cfg(any(
        feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery",
        feature = "neo-test-phase3-neofs-crash-mid-create-recovery",
        feature = "neo-test-phase3-neofs-superblock-rotation-recovery"
    ))]
    pub(super) fn debug_write_invalid_superblock_copy(
        &self,
        slot: usize,
    ) -> Result<(), NeoFsError> {
        let _writeback_pause = self.cache.pause_worker();
        let mut superblock = self.metadata.lock().superblock.clone();
        superblock.generation = superblock
            .generation
            .checked_add(100)
            .ok_or(NeoFsError::InvalidSuperblock)?;
        superblock.root_inode_block = superblock
            .total_blocks
            .checked_sub(1)
            .ok_or(NeoFsError::InvalidSuperblock)?;
        superblock.root_inode_hash = [0xa5; 32];
        write_superblock_copy_cached(&self.cache, slot, &superblock)
    }

    #[cfg(feature = "neo-test-phase3-neofs-crash-mid-create-recovery")]
    pub(super) fn debug_inject_create_before_superblock(
        &self,
        name: alloc::string::String,
    ) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        let _writeback_pause = self.cache.pause_worker();
        let entries = self.entries.lock();
        if entries.iter().any(|entry| entry.name == name) {
            return Err(NeoFsError::InvalidDirectory);
        }
        if entries.len() >= FS_BLOCK_SIZE / DIRENT_SIZE {
            return Err(NeoFsError::NoSpace);
        }

        let (inode_block, generation) = self.reserve_inode_block_and_generation()?;
        let record = InodeRecord {
            kind: super::format::NeoFsKind::Regular,
            generation,
            file_size: 0,
            block_count: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            content_hash: hash(&[]),
            available_rights: FileRights::from_bits(
                FileRights::READ.bits() | FileRights::WRITE.bits(),
            ),
            directory_entry_count: 0,
            fs_block_count: self.total_blocks(),
        };
        write_inode_to_cache(
            &self.cache,
            inode_block,
            self.total_blocks(),
            &record,
            CachedBlockRole::Inode,
        )?;

        let mut new_entries = entries.clone();
        new_entries.push(DirEntry {
            name,
            child_block: inode_block,
            kind: super::format::NeoFsKind::Regular,
        });
        self.write_root_tree_without_publish(&new_entries)?;
        Ok(())
    }

    #[cfg(feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery")]
    pub(super) fn debug_inject_overwrite_before_superblock(
        &self,
        name: &str,
        offset: usize,
        input: &[u8],
    ) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }

        let _writeback_pause = self.cache.pause_worker();
        let entries = self.entries.lock();
        let entry_index = entries
            .iter()
            .position(|entry| entry.name == name)
            .ok_or(NeoFsError::NotFound)?;
        let old_inode_block = entries[entry_index].child_block;
        let (old_record, _bytes) = self.read_cached_inode(old_inode_block)?;
        let new_content = build_cow_content_cached(&self.cache, &old_record, offset, input)?;
        let required_blocks = if new_content.is_empty() {
            0
        } else {
            (new_content.len() + FS_BLOCK_SIZE - 1) / FS_BLOCK_SIZE
        };
        if required_blocks > DIRECT_BLOCKS {
            return Err(NeoFsError::FileTooLarge);
        }
        let data_blocks = self.reserve_blocks(required_blocks)?;
        write_content_blocks_cached(&self.cache, &data_blocks, &new_content)?;

        let (new_inode_block, generation) = self.reserve_inode_block_and_generation()?;
        let mut new_record = old_record;
        new_record.generation = generation;
        new_record.file_size = new_content.len() as u64;
        new_record.block_count = required_blocks as u64;
        new_record.direct_blocks = [0; DIRECT_BLOCKS];
        new_record.single_indirect_block = 0;
        new_record.double_indirect_block = 0;
        new_record.triple_indirect_block = 0;
        for (idx, block) in data_blocks.iter().copied().enumerate() {
            new_record.direct_blocks[idx] = block;
        }
        new_record.content_hash = hash(&new_content);
        // The volume's size now, not when the old record was read: after a
        // grow, the new blocks may lie past the old end.
        new_record.fs_block_count = self.total_blocks();
        write_inode_to_cache(
            &self.cache,
            new_inode_block,
            self.total_blocks(),
            &new_record,
            CachedBlockRole::Inode,
        )?;

        let mut new_entries = entries.clone();
        new_entries[entry_index].child_block = new_inode_block;
        self.write_root_tree_without_publish(&new_entries)?;
        Ok(())
    }

    /// The scenario's cut after the barrier flush (`NeoFsBlockCache::
    /// debug_drain_to_barrier`). Returns the flushes it issued.
    #[cfg(any(
        feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery",
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub(super) fn debug_drain_to_barrier(&self) -> Result<u64, NeoFsError> {
        self.cache.debug_drain_to_barrier()
    }

    /// The generation is checked before the block is taken, so a refusal
    /// changes nothing (`alloc_logic::reserve_block_and_generation`).
    fn reserve_inode_block_and_generation(&self) -> Result<(u64, u32), NeoFsError> {
        let mut metadata = self.metadata.lock();
        Ok(alloc_logic::reserve_block_and_generation(&mut *metadata)?)
    }

    fn reserve_blocks(&self, count: usize) -> Result<Vec<u64>, NeoFsError> {
        let mut metadata = self.metadata.lock();
        let mut blocks = Vec::with_capacity(count);
        for _ in 0..count {
            blocks.push(allocate_one_block(&mut metadata)?);
        }
        Ok(blocks)
    }

    fn reserve_indirect_block(&self) -> Result<u64, NeoFsError> {
        self.reserve_blocks(1)?
            .pop()
            .ok_or(NeoFsError::InvalidInode)
    }

    fn allocate_file_block_layout(
        &self,
        block_count: u64,
    ) -> Result<RuntimeFileBlockLayout, NeoFsError> {
        if block_count > MAX_FILE_BLOCKS {
            return Err(NeoFsError::FileTooLarge);
        }
        let block_count_usize =
            usize::try_from(block_count).map_err(|_| NeoFsError::FileTooLarge)?;
        let data_blocks = self.reserve_blocks(block_count_usize)?;

        let mut direct_blocks = [0u64; DIRECT_BLOCKS];
        for (idx, slot) in direct_blocks
            .iter_mut()
            .enumerate()
            .take(block_count_usize.min(DIRECT_BLOCKS))
        {
            *slot = data_blocks[idx];
        }

        let mut indirect_blocks = Vec::new();
        let mut single_indirect_block = 0;
        let mut double_indirect_block = 0;
        let mut triple_indirect_block = 0;
        let mut cursor = DIRECT_BLOCKS as u64;

        let single_count = block_count
            .saturating_sub(cursor)
            .min(SINGLE_INDIRECT_BLOCKS as u64);
        if single_count != 0 {
            let root = self.reserve_indirect_block()?;
            let mut bytes = vec![0u8; FS_BLOCK_SIZE];
            for idx in 0..single_count {
                put_indirect_pointer(&mut bytes, idx, data_blocks[(cursor + idx) as usize])?;
            }
            indirect_blocks.push((root, bytes));
            single_indirect_block = root;
            cursor += single_count;
        }

        let double_count = block_count
            .saturating_sub(cursor)
            .min(DOUBLE_INDIRECT_BLOCKS as u64);
        if double_count != 0 {
            let root = self.reserve_indirect_block()?;
            let leaves = self.allocate_indirect_tree(
                &data_blocks[cursor as usize..(cursor + double_count) as usize],
            )?;
            let mut root_bytes = vec![0u8; FS_BLOCK_SIZE];
            for (idx, (leaf_block, leaf_bytes)) in leaves.into_iter().enumerate() {
                put_indirect_pointer(&mut root_bytes, idx as u64, leaf_block)?;
                indirect_blocks.push((leaf_block, leaf_bytes));
            }
            indirect_blocks.push((root, root_bytes));
            double_indirect_block = root;
            cursor += double_count;
        }

        let triple_count = block_count.saturating_sub(cursor);
        if triple_count != 0 {
            let root = self.reserve_indirect_block()?;
            let level2 = self.allocate_triple_indirect_tree(
                &data_blocks[cursor as usize..(cursor + triple_count) as usize],
            )?;
            let mut root_bytes = vec![0u8; FS_BLOCK_SIZE];
            for (idx, (level2_block, level2_bytes, leaves)) in level2.into_iter().enumerate() {
                put_indirect_pointer(&mut root_bytes, idx as u64, level2_block)?;
                indirect_blocks.push((level2_block, level2_bytes));
                indirect_blocks.extend(leaves);
            }
            indirect_blocks.push((root, root_bytes));
            triple_indirect_block = root;
        }

        Ok(RuntimeFileBlockLayout {
            direct_blocks,
            single_indirect_block,
            double_indirect_block,
            triple_indirect_block,
            data_blocks,
            indirect_blocks,
        })
    }

    fn allocate_indirect_tree(
        &self,
        data_blocks: &[u64],
    ) -> Result<Vec<RuntimeIndirectBlock>, NeoFsError> {
        let mut leaves = Vec::new();
        for chunk in data_blocks.chunks(INDIRECT_POINTERS_PER_BLOCK) {
            let leaf = self.reserve_indirect_block()?;
            let mut leaf_bytes = vec![0u8; FS_BLOCK_SIZE];
            for (idx, data_block) in chunk.iter().copied().enumerate() {
                put_indirect_pointer(&mut leaf_bytes, idx as u64, data_block)?;
            }
            leaves.push((leaf, leaf_bytes));
        }
        Ok(leaves)
    }

    fn allocate_triple_indirect_tree(
        &self,
        data_blocks: &[u64],
    ) -> Result<Vec<RuntimeTripleLevel>, NeoFsError> {
        let mut level2_blocks = Vec::new();
        for level2_chunk in data_blocks.chunks(DOUBLE_INDIRECT_BLOCKS) {
            let level2 = self.reserve_indirect_block()?;
            let leaves = self.allocate_indirect_tree(level2_chunk)?;
            let mut level2_bytes = vec![0u8; FS_BLOCK_SIZE];
            for (idx, (leaf_block, _)) in leaves.iter().enumerate() {
                put_indirect_pointer(&mut level2_bytes, idx as u64, *leaf_block)?;
            }
            level2_blocks.push((level2, level2_bytes, leaves));
        }
        Ok(level2_blocks)
    }

    /// Publish `entries` as the new root, freeing `superseded`: the blocks
    /// of every version this commit replaces. They are free in the map this
    /// publish writes, and not reusable until this generation is durable on
    /// every copy (#180 rule 2), so a cut that brings an older generation
    /// back finds its blocks untouched.
    fn commit_root_entries(
        &self,
        entries: &[DirEntry],
        superseded: &[u64],
    ) -> Result<(), NeoFsError> {
        let (root_inode_block, root_record) = self.write_root_tree_without_publish(entries)?;
        let root_hash = hash(&write_inode(&root_record));
        let mut metadata = self.metadata.lock();
        metadata.superblock.root_inode_block = root_inode_block;
        metadata.superblock.root_inode_hash = root_hash;
        bump_superblock_generation(&mut metadata.superblock)?;
        let generation = metadata.superblock.generation;
        let freed = free_superseded(&mut metadata, superseded, generation)?;
        if let Err(e) = self.publish_superblock(&mut metadata) {
            // Nothing was published: the old tree is still the live one.
            unfree(&mut metadata, &freed)?;
            return Err(e);
        }
        let mut root = self.root.lock();
        root.inode_block = root_inode_block;
        root.record = root_record;
        Ok(())
    }

    fn write_root_tree_without_publish(
        &self,
        entries: &[DirEntry],
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        let old_root = self.root.lock().record.clone();
        self.write_directory_tree_without_publish(&old_root, entries)
    }

    fn write_directory_tree_without_publish(
        &self,
        old_record: &InodeRecord,
        entries: &[DirEntry],
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        let root_dir_block = self.reserve_blocks(1)?[0];
        let (root_inode_block, generation) = self.reserve_inode_block_and_generation()?;
        let mut root_dir_data = vec![0u8; FS_BLOCK_SIZE];
        for (idx, entry) in entries.iter().enumerate() {
            check_directory_capacity(idx + 1)?;
            let bytes = write_dir_entry(entry)?;
            root_dir_data[idx * DIRENT_SIZE..(idx + 1) * DIRENT_SIZE].copy_from_slice(&bytes);
        }
        let root_size = entries.len() * DIRENT_SIZE;
        self.cache.write_block(
            root_dir_block,
            &root_dir_data,
            CachedBlockRole::RootDirectory,
        )?;

        let mut direct_blocks = [0u64; DIRECT_BLOCKS];
        direct_blocks[0] = root_dir_block;
        let root_record = InodeRecord {
            kind: super::format::NeoFsKind::Directory,
            generation,
            file_size: root_size as u64,
            block_count: 1,
            direct_blocks,
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            content_hash: hash(&root_dir_data[..root_size]),
            available_rights: old_record.available_rights,
            directory_entry_count: entries.len() as u32,
            fs_block_count: self.total_blocks(),
        };
        write_inode_to_cache(
            &self.cache,
            root_inode_block,
            self.total_blocks(),
            &root_record,
            CachedBlockRole::RootDirectory,
        )?;
        Ok((root_inode_block, root_record))
    }

    fn publish_superblock(&self, metadata: &mut NeoFsMetadata) -> Result<(), NeoFsError> {
        #[cfg(feature = "neofs-large-write")]
        if self
            .commit_failed
            .load(core::sync::atomic::Ordering::Acquire)
        {
            return Err(NeoFsError::BlockDevice);
        }
        if metadata
            .free_map
            .as_ref()
            .map(|free_map| free_map.dirty)
            .unwrap_or(false)
        {
            let block_count = metadata
                .superblock
                .free_map
                .ok_or(NeoFsError::InvalidSuperblock)?
                .block_count;
            let start_block = allocate_contiguous_blocks(metadata, block_count)?;
            // The map this publish moves off is free in the map it writes.
            let (old_start, old_count) = metadata
                .free_map
                .as_ref()
                .ok_or(NeoFsError::InvalidSuperblock)?
                .on_disk;
            let old_map: Vec<u64> = if cfg!(feature = "neofs-leak-superseded") {
                Vec::new()
            } else {
                (old_start..old_start + old_count).collect()
            };
            let generation = metadata.superblock.generation;
            let freed = free_superseded(metadata, &old_map, generation)?;
            let map_bytes = metadata
                .free_map
                .as_ref()
                .ok_or(NeoFsError::InvalidSuperblock)?
                .bytes
                .clone();
            let map_hash = hash(&map_bytes);
            let published =
                write_free_map_blocks_cached(&self.cache, start_block, block_count, &map_bytes)
                    .and_then(|()| {
                        metadata.superblock.free_map = Some(FreeMapRef {
                            start_block,
                            block_count,
                            hash: map_hash,
                        });
                        write_superblock_copies_cached(
                            &self.cache,
                            &self.device,
                            &metadata.superblock,
                        )
                    });
            if let Err(e) = published {
                unfree(metadata, &freed)?;
                return Err(e);
            }
            if let Some(free_map) = metadata.free_map.as_mut() {
                free_map.dirty = false;
                free_map.on_disk = (start_block, block_count);
            }
            return Ok(());
        }

        write_superblock_copies_cached(&self.cache, &self.device, &metadata.superblock)?;
        Ok(())
    }

    #[cfg(feature = "neo-test-phase3-neofs-3c")]
    fn run_gc_locked(&self, metadata: &mut NeoFsMetadata) -> Result<GcStats, NeoFsError> {
        let previous_free = {
            let free_map = metadata
                .free_map
                .as_ref()
                .ok_or(NeoFsError::UnsupportedType)?;
            count_free_blocks(&free_map.bytes, metadata.superblock.total_blocks)?
        };
        let (live, live_count) = collect_live_blocks(&self.device, &metadata.superblock)?;
        let new_map = build_swept_free_map(&metadata.superblock, &live)?;
        let free_map = metadata
            .free_map
            .as_mut()
            .ok_or(NeoFsError::UnsupportedType)?;
        free_map.bytes = new_map;
        free_map.dirty = true;
        bump_superblock_generation(&mut metadata.superblock)?;
        self.publish_superblock(metadata)?;
        let free_blocks = count_free_blocks(
            &metadata
                .free_map
                .as_ref()
                .ok_or(NeoFsError::UnsupportedType)?
                .bytes,
            metadata.superblock.total_blocks,
        )?;
        let map_start_block = metadata
            .superblock
            .free_map
            .ok_or(NeoFsError::InvalidSuperblock)?
            .start_block;
        Ok(GcStats {
            live_blocks: live_count,
            reclaimed_blocks: free_blocks.saturating_sub(previous_free),
            free_blocks,
            map_start_block,
        })
    }
}

fn split_relative_path(path: &str) -> Result<Vec<&str>, NeoFsError> {
    if path.is_empty() {
        return Err(NeoFsError::InvalidDirectory);
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() {
            return Err(NeoFsError::InvalidDirectory);
        }
        parts.push(part);
    }
    Ok(parts)
}

fn split_leaf<'a>(parts: &'a [&'a str]) -> Result<(&'a str, &'a [&'a str]), NeoFsError> {
    let (leaf, parents) = parts.split_last().ok_or(NeoFsError::InvalidDirectory)?;
    Ok((*leaf, parents))
}

fn check_directory_capacity(entry_count: usize) -> Result<(), NeoFsError> {
    if entry_count > FS_BLOCK_SIZE / DIRENT_SIZE {
        Err(NeoFsError::NoSpace)
    } else {
        Ok(())
    }
}

impl Drop for NeoFsState {
    fn drop(&mut self) {
        #[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
        drop_pause_point();
        let _ = stop_writeback_worker(&self.cache, &self.writeback_worker);
    }
}

/// Ticks the next `NeoFsState` drop yields IF-on before stopping its writeback
/// worker (0 = no pause), standing in for the stop-and-join park.
#[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
static DROP_PAUSE_TICKS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
#[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
static DROP_PAUSED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
pub(crate) fn pause_next_state_drop_for_test(ticks: u64) {
    DROP_PAUSE_TICKS.store(ticks, core::sync::atomic::Ordering::Release);
}

#[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
pub(crate) fn state_drop_paused_for_test() -> bool {
    DROP_PAUSED.load(core::sync::atomic::Ordering::Acquire)
}

#[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
fn drop_pause_point() {
    use core::sync::atomic::Ordering;
    let ticks = DROP_PAUSE_TICKS.swap(0, Ordering::AcqRel);
    if ticks == 0 {
        return;
    }
    let tick = || crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
    let until = tick().saturating_add(ticks);
    DROP_PAUSED.store(true, Ordering::Release);
    crate::arch::x86_64::with_interrupts_enabled(|| {
        while tick() < until {
            crate::sched::yield_now();
        }
    });
    DROP_PAUSED.store(false, Ordering::Release);
}

pub struct NeoInode {
    state: Arc<NeoFsState>,
    name: Option<String>,
    /// Process/kthread inode location state; NeoFS IRQ work is delegated below
    /// the filesystem layer to block drivers.
    inode_block: SwitchMutex<u64>,
    /// Process/kthread inode record state; no ISR path acquires it.
    pub(super) record: SwitchMutex<InodeRecord>,
    /// Content-hash validation cache for this inode snapshot. NeoFS reads use
    /// bounded range I/O, so large files must not be materialized just to prove
    /// the existing content-hash contract before serving a chunk.
    verified_content: SwitchMutex<Option<VerifiedContent>>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct VerifiedContent {
    generation: u32,
    file_size: u64,
    block_count: u64,
    content_hash: [u8; 32],
}

impl VerifiedContent {
    fn from_record(record: &InodeRecord) -> Self {
        Self {
            generation: record.generation,
            file_size: record.file_size,
            block_count: record.block_count,
            content_hash: record.content_hash,
        }
    }
}

impl NeoInode {
    pub(super) fn new(
        state: Arc<NeoFsState>,
        name: Option<String>,
        inode_block: u64,
        record: InodeRecord,
    ) -> Self {
        Self {
            state,
            name,
            inode_block: SwitchMutex::new(inode_block),
            record: SwitchMutex::new(record),
            verified_content: SwitchMutex::new(None),
        }
    }

    pub(super) fn snapshot(&self) -> InodeRecord {
        self.record.lock().clone()
    }
}

impl Inode for NeoInode {
    fn kind(&self) -> InodeKind {
        self.record.lock().kind.as_vfs_kind()
    }

    fn available_rights(&self) -> FileRights {
        let record = self.record.lock();
        match record.kind.as_vfs_kind() {
            InodeKind::Regular => {
                let mut bits = record.available_rights.bits() & FileRights::KNOWN_BUILD4A.bits();
                if !self.state.is_writable() {
                    bits &= !FileRights::WRITE.bits();
                }
                FileRights::from_bits(bits)
            }
            InodeKind::Model => FileRights::READ,
            InodeKind::Directory => {
                FileRights::from_bits(FileRights::LOOKUP.bits() | FileRights::READDIR.bits())
            }
            InodeKind::Symlink => FileRights::empty(),
        }
    }

    fn current_generation(&self) -> Result<Option<u32>, VfsError> {
        match self.name.as_deref() {
            Some(name) => self.state.current_child_generation(name).map(Some),
            None => Ok(Some(self.state.root_snapshot().1.generation)),
        }
    }

    #[cfg(feature = "neofs-dir-ops")]
    fn mkdir_at(&self, name: &str) -> Result<(), VfsError> {
        dir_ops::mkdir(self, name)
    }

    #[cfg(feature = "neofs-dir-ops")]
    fn unlink_at(&self, name: &str) -> Result<(), VfsError> {
        dir_ops::unlink(self, name)
    }

    #[cfg(feature = "neofs-dir-ops")]
    fn rename_at(&self, name: &str, to: &dyn Inode, to_name: &str) -> Result<(), VfsError> {
        dir_ops::rename(self, name, to, to_name)
    }

    #[cfg(feature = "neofs-dir-ops")]
    fn stat_at(&self, name: &str) -> Result<super::dir_ops_logic::StatAtV1, VfsError> {
        dir_ops::stat(self, name)
    }

    #[cfg(feature = "neofs-dir-ops")]
    fn reload_dir(&self) -> Result<Option<u32>, VfsError> {
        dir_ops::reload(self)
    }

    #[cfg(feature = "neofs-dir-ops")]
    fn neofs_volume(&self) -> Option<(usize, Option<&str>)> {
        Some((Arc::as_ptr(&self.state) as usize, self.name.as_deref()))
    }

    fn fsync(&self) -> Result<(), VfsError> {
        if self.record.lock().kind.as_vfs_kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        self.state.fsync_regular().map_err(|_| VfsError::Invalid)
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.record.lock().file_size)
    }

    /// OT-87 R2: the record's content hash once its bytes hashed to it. A
    /// generation already verified (the read path's cache) is not hashed
    /// again; the read-path skip of the real-brain scenarios does not apply.
    #[cfg(any(feature = "model-from-neofs", feature = "installer"))]
    fn verified_content_hash(&self) -> Result<Option<crate::fs::VerifiedHash>, VfsError> {
        let record = self.record.lock().clone();
        if !matches!(
            record.kind.as_vfs_kind(),
            InodeKind::Regular | InodeKind::Model
        ) {
            return Err(VfsError::TypeForbidden);
        }
        ensure_verified_content(&self.state.cache, &record, &self.verified_content)
            .map_err(|_| VfsError::Invalid)?;
        Ok(Some(crate::fs::VerifiedHash {
            blake3: record.content_hash,
            generation: record.generation,
        }))
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        let record = self.record.lock().clone();
        if !matches!(
            record.kind.as_vfs_kind(),
            InodeKind::Regular | InodeKind::Model
        ) {
            return Err(VfsError::TypeForbidden);
        }
        if should_verify_before_range_read(record.kind.as_vfs_kind()) {
            ensure_verified_content(&self.state.cache, &record, &self.verified_content)
                .map_err(|_| VfsError::Invalid)?;
        }
        #[cfg(any(
            feature = "neo-test-arthur-real-brain-boot-15b",
            feature = "neo-test-arthur-real-brain-boot-3b"
        ))]
        if offset == 0 && out.len() > FS_BLOCK_SIZE && record.kind.as_vfs_kind() == InodeKind::Model
        {
            log_stage_b_model_indirect_summary(&record);
        }
        read_inode_range_cached(&self.state.cache, &record, offset, out)
            .map_err(|_| VfsError::Invalid)
    }

    fn readdir_at(&self, cursor: u64) -> Result<Option<VfsDirEntry>, VfsError> {
        let record = self.record.lock().clone();
        if record.kind.as_vfs_kind() != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        let idx = usize::try_from(cursor).map_err(|_| VfsError::Invalid)?;
        let entry = if self.name.is_none() {
            self.state.entries.lock().get(idx).cloned()
        } else {
            let content = read_verified_logical_content_cached(&self.state.cache, &record)
                .map_err(|_| VfsError::Invalid)?;
            let entries =
                parse_directory_entries(&record, &content).map_err(|_| VfsError::Invalid)?;
            entries.get(idx).cloned()
        };
        Ok(entry.map(|entry| VfsDirEntry {
            name: entry.name,
            kind: entry.kind.as_vfs_kind(),
            next_cursor: (idx + 1) as u64,
        }))
    }

    fn write_at(&self, offset: usize, input: &[u8]) -> Result<usize, VfsError> {
        if input.is_empty() {
            return Ok(0);
        }
        if !self.state.is_writable() {
            return Err(VfsError::InsufficientRights);
        }

        let old_record = self.record.lock().clone();
        if old_record.kind.as_vfs_kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        if !old_record.available_rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }

        let end = offset.checked_add(input.len()).ok_or(VfsError::Invalid)?;
        if end > MAX_FILE_SIZE {
            return Err(VfsError::Invalid);
        }

        let old_inode_block = *self.inode_block.lock();
        let name = self.name.as_deref().ok_or(VfsError::Stale)?;
        let (new_inode_block, new_record) = self
            .state
            .commit_regular_write(name, old_inode_block, &old_record, offset, input)
            .map_err(|_| VfsError::Invalid)?;
        *self.inode_block.lock() = new_inode_block;
        *self.record.lock() = new_record;
        *self.verified_content.lock() = None;
        Ok(input.len())
    }

    fn truncate(&self, new_len: u64) -> Result<(), VfsError> {
        if !self.state.is_writable() {
            return Err(VfsError::InsufficientRights);
        }
        if new_len > MAX_FILE_SIZE as u64 {
            return Err(VfsError::Invalid);
        }

        let old_record = self.record.lock().clone();
        if old_record.kind.as_vfs_kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        if !old_record.available_rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }
        if old_record.file_size == new_len {
            return Ok(());
        }

        let old_inode_block = *self.inode_block.lock();
        let name = self.name.as_deref().ok_or(VfsError::Stale)?;
        let (new_inode_block, new_record) = self
            .state
            .commit_regular_truncate(name, old_inode_block, &old_record, new_len)
            .map_err(|_| VfsError::Invalid)?;
        *self.inode_block.lock() = new_inode_block;
        *self.record.lock() = new_record;
        *self.verified_content.lock() = None;
        Ok(())
    }

    /// OT-67 W1. A lost compare-and-swap (another handle replaced the file
    /// first) is `Stale`: this handle's version is gone.
    #[cfg(feature = "neofs-file-replace")]
    fn replace(&self, bytes: &[u8]) -> Result<(), VfsError> {
        if !self.state.is_writable() {
            return Err(VfsError::InsufficientRights);
        }
        let old_record = self.record.lock().clone();
        if old_record.kind.as_vfs_kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        if !old_record.available_rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }
        let old_inode_block = *self.inode_block.lock();
        let name = self.name.as_deref().ok_or(VfsError::Stale)?;
        let (new_inode_block, new_record) = self
            .state
            .commit_regular_replace(name, old_inode_block, &old_record, bytes, false)
            .map_err(|e| match e {
                NeoFsError::NotFound => VfsError::Stale,
                NeoFsError::BlockDevice
                | NeoFsError::NoBlockDevice
                | NeoFsError::NoDurableFlush => VfsError::Io,
                _ => VfsError::Invalid,
            })?;
        *self.inode_block.lock() = new_inode_block;
        *self.record.lock() = new_record;
        *self.verified_content.lock() = None;
        Ok(())
    }
}

#[cfg(any(
    feature = "neo-test-arthur-real-brain-boot-15b",
    feature = "neo-test-arthur-real-brain-boot-3b"
))]
fn log_stage_b_model_indirect_summary(inode: &InodeRecord) {
    let blocks = inode.block_count;
    let direct = cmp::min(blocks, DIRECT_BLOCKS as u64);
    let mut remaining = blocks.saturating_sub(direct);
    let single = cmp::min(remaining, SINGLE_INDIRECT_BLOCKS as u64);
    remaining = remaining.saturating_sub(single);
    let double = cmp::min(remaining, DOUBLE_INDIRECT_BLOCKS as u64);
    remaining = remaining.saturating_sub(double);
    let triple = remaining;
    serial_println!(
        "neofs_model_read_indirect file_size={} data_blocks={} direct={} single={} double={} triple={} roots=[{},{},{}]",
        inode.file_size,
        blocks,
        direct,
        single,
        double,
        triple,
        inode.single_indirect_block,
        inode.double_indirect_block,
        inode.triple_indirect_block
    );
}

pub(super) fn read_inode_from_block(
    device: &Arc<dyn BlockDevice>,
    block: u64,
    total_blocks: u64,
) -> Result<(InodeRecord, [u8; INODE_SIZE]), NeoFsError> {
    let mut inode_bytes = [0u8; INODE_SIZE];
    read_block_prefix(device, block, &mut inode_bytes)?;
    let inode = parse_inode(&inode_bytes, total_blocks)?;
    Ok((inode, inode_bytes))
}

fn read_inode_from_cache(
    cache: &Arc<NeoFsBlockCache>,
    block: u64,
    total_blocks: u64,
) -> Result<(InodeRecord, [u8; INODE_SIZE]), NeoFsError> {
    let mut inode_bytes = [0u8; INODE_SIZE];
    cache.read_block_prefix(block, &mut inode_bytes)?;
    let inode = parse_inode(&inode_bytes, total_blocks)?;
    Ok((inode, inode_bytes))
}

pub(super) fn write_inode_to_block(
    device: &Arc<dyn BlockDevice>,
    block: u64,
    total_blocks: u64,
    inode: &InodeRecord,
) -> Result<(), NeoFsError> {
    if block == 0 || block >= total_blocks {
        return Err(NeoFsError::InvalidInode);
    }

    let inode_bytes = write_inode(inode);
    let mut block_bytes = vec![0u8; FS_BLOCK_SIZE];
    block_bytes[..INODE_SIZE].copy_from_slice(&inode_bytes);
    write_fs_block(device, block, &block_bytes)
}

fn write_inode_to_cache(
    cache: &Arc<NeoFsBlockCache>,
    block: u64,
    total_blocks: u64,
    inode: &InodeRecord,
    role: CachedBlockRole,
) -> Result<(), NeoFsError> {
    if block == 0 || block >= total_blocks {
        return Err(NeoFsError::InvalidInode);
    }

    let inode_bytes = write_inode(inode);
    let mut block_bytes = vec![0u8; FS_BLOCK_SIZE];
    block_bytes[..INODE_SIZE].copy_from_slice(&inode_bytes);
    cache.write_block(block, &block_bytes, role)
}

pub(super) fn read_verified_logical_content(
    device: &Arc<dyn BlockDevice>,
    inode: &InodeRecord,
) -> Result<Vec<u8>, NeoFsError> {
    let content = read_logical_content(device, inode)?;
    if hash(&content) != inode.content_hash {
        return Err(NeoFsError::ContentHashMismatch);
    }
    Ok(content)
}

fn read_verified_logical_content_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
) -> Result<Vec<u8>, NeoFsError> {
    let content = read_logical_content_cached(cache, inode)?;
    if hash(&content) != inode.content_hash {
        return Err(NeoFsError::ContentHashMismatch);
    }
    Ok(content)
}

fn read_logical_prefix_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
    len: usize,
) -> Result<Vec<u8>, NeoFsError> {
    let file_size = usize::try_from(inode.file_size).map_err(|_| NeoFsError::FileTooLarge)?;
    if len > file_size {
        return Err(NeoFsError::InvalidInode);
    }
    let mut content = vec![0u8; len];
    let copied = read_inode_range_cached(cache, inode, 0, &mut content)?;
    if copied != len {
        return Err(NeoFsError::InvalidInode);
    }
    Ok(content)
}

fn ensure_verified_content(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
    verified: &SwitchMutex<Option<VerifiedContent>>,
) -> Result<(), NeoFsError> {
    let expected = VerifiedContent::from_record(inode);
    if *verified.lock() == Some(expected) {
        return Ok(());
    }
    verify_inode_content_hash_cached(cache, inode)?;
    *verified.lock() = Some(expected);
    Ok(())
}

fn verify_inode_content_hash_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
) -> Result<(), NeoFsError> {
    validate_readable_inode(inode)?;
    let mut hasher = blake3::Hasher::new();
    let mut block_buf = vec![0u8; FS_BLOCK_SIZE];
    let mut remaining = inode.file_size as usize;
    for logical in 0..inode.block_count {
        if remaining == 0 {
            break;
        }
        let take = cmp::min(remaining, FS_BLOCK_SIZE);
        let block = resolve_logical_block_cached(cache, inode, logical)?;
        cache.read_block_prefix(block, &mut block_buf[..take])?;
        hasher.update(&block_buf[..take]);
        remaining -= take;
    }
    if remaining != 0 {
        return Err(NeoFsError::InvalidInode);
    }
    if *hasher.finalize().as_bytes() != inode.content_hash {
        return Err(NeoFsError::ContentHashMismatch);
    }
    Ok(())
}

fn read_inode_range_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
    offset: usize,
    out: &mut [u8],
) -> Result<usize, NeoFsError> {
    validate_readable_inode(inode)?;
    if out.is_empty() {
        return Ok(0);
    }
    let file_size = usize::try_from(inode.file_size).map_err(|_| NeoFsError::FileTooLarge)?;
    if offset >= file_size {
        return Ok(0);
    }
    let target_len = cmp::min(out.len(), file_size - offset);
    let mut copied = 0usize;
    let mut block_buf = vec![0u8; FS_BLOCK_SIZE];
    let cache_data_blocks = should_cache_range_read_blocks(inode.kind.as_vfs_kind());
    while copied < target_len {
        let absolute = offset + copied;
        let logical = (absolute / FS_BLOCK_SIZE) as u64;
        let in_block = absolute % FS_BLOCK_SIZE;
        let take = cmp::min(target_len - copied, FS_BLOCK_SIZE - in_block);
        let block = resolve_logical_block_cached(cache, inode, logical)?;
        if cache_data_blocks {
            cache.read_block_prefix(block, &mut block_buf)?;
        } else {
            cache.read_block_prefix_uncached(block, &mut block_buf)?;
        }
        out[copied..copied + take].copy_from_slice(&block_buf[in_block..in_block + take]);
        copied += take;
    }
    Ok(copied)
}

pub(super) fn read_logical_content(
    device: &Arc<dyn BlockDevice>,
    inode: &InodeRecord,
) -> Result<Vec<u8>, NeoFsError> {
    if inode.kind.as_vfs_kind() == InodeKind::Directory
        && inode.file_size != inode.directory_entry_count as u64 * DIRENT_SIZE as u64
    {
        return Err(NeoFsError::InvalidDirectory);
    }
    read_inode_content_with(inode, |logical, out| {
        let block = resolve_logical_block_device(device, inode, logical)?;
        read_block_prefix(device, block, out)
    })
}

fn read_logical_content_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
) -> Result<Vec<u8>, NeoFsError> {
    if inode.kind.as_vfs_kind() == InodeKind::Directory
        && inode.file_size != inode.directory_entry_count as u64 * DIRENT_SIZE as u64
    {
        return Err(NeoFsError::InvalidDirectory);
    }

    read_inode_content_with(inode, |logical, out| {
        let block = resolve_logical_block_cached(cache, inode, logical)?;
        cache.read_block_prefix(block, out)
    })
}

fn read_inode_content_with<F>(
    inode: &InodeRecord,
    mut read_logical_block: F,
) -> Result<Vec<u8>, NeoFsError>
where
    F: FnMut(u64, &mut [u8]) -> Result<(), NeoFsError>,
{
    validate_readable_inode(inode)?;
    let mut out = Vec::with_capacity(inode.file_size as usize);
    let mut remaining = inode.file_size as usize;
    for logical in 0..inode.block_count {
        if remaining == 0 {
            break;
        }
        let take = cmp::min(remaining, FS_BLOCK_SIZE);
        let start = out.len();
        out.resize(start + take, 0);
        read_logical_block(logical, &mut out[start..start + take])?;
        remaining -= take;
    }
    if remaining != 0 {
        return Err(NeoFsError::InvalidInode);
    }
    Ok(out)
}

pub(super) fn validate_readable_inode(inode: &InodeRecord) -> Result<(), NeoFsError> {
    if inode.file_size > MAX_FILE_SIZE as u64 {
        return Err(NeoFsError::FileTooLarge);
    }
    let expected_blocks = file_data_blocks(inode.file_size)?;
    if inode.block_count != expected_blocks || inode.block_count > MAX_FILE_BLOCKS {
        return Err(NeoFsError::InvalidInode);
    }
    if inode.fs_block_count == 0 {
        return Err(NeoFsError::InvalidInode);
    }
    Ok(())
}

fn should_verify_before_range_read(kind: InodeKind) -> bool {
    let _ = kind;
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    if kind == InodeKind::Model {
        return false;
    }
    true
}

fn should_cache_range_read_blocks(kind: InodeKind) -> bool {
    let _ = kind;
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    if kind == InodeKind::Model {
        return false;
    }
    true
}

fn resolve_logical_block_device(
    device: &Arc<dyn BlockDevice>,
    inode: &InodeRecord,
    logical: u64,
) -> Result<u64, NeoFsError> {
    resolve_logical_block_with(inode, logical, |block, out| {
        read_block_prefix(device, block, out)
    })
}

fn resolve_logical_block_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode: &InodeRecord,
    logical: u64,
) -> Result<u64, NeoFsError> {
    resolve_logical_block_with(inode, logical, |block, out| {
        cache.read_block_prefix(block, out)
    })
}

fn resolve_logical_block_with<F>(
    inode: &InodeRecord,
    logical: u64,
    mut read_indirect_block: F,
) -> Result<u64, NeoFsError>
where
    F: FnMut(u64, &mut [u8]) -> Result<(), NeoFsError>,
{
    validate_readable_inode(inode)?;
    if logical >= inode.block_count {
        return Err(NeoFsError::InvalidInode);
    }

    let direct = DIRECT_BLOCKS as u64;
    if logical < direct {
        return validate_fs_block(inode.direct_blocks[logical as usize], inode.fs_block_count);
    }

    let pointers = INDIRECT_POINTERS_PER_BLOCK as u64;
    let single_blocks = SINGLE_INDIRECT_BLOCKS as u64;
    let double_blocks = DOUBLE_INDIRECT_BLOCKS as u64;
    let mut index = logical - direct;

    if index < single_blocks {
        return read_pointer_from_indirect(
            &mut read_indirect_block,
            inode.single_indirect_block,
            index,
            inode.fs_block_count,
        );
    }
    index -= single_blocks;

    if index < double_blocks {
        let leaf_block = read_pointer_from_indirect(
            &mut read_indirect_block,
            inode.double_indirect_block,
            index / pointers,
            inode.fs_block_count,
        )?;
        return read_pointer_from_indirect(
            &mut read_indirect_block,
            leaf_block,
            index % pointers,
            inode.fs_block_count,
        );
    }
    index -= double_blocks;

    let level2_block = read_pointer_from_indirect(
        &mut read_indirect_block,
        inode.triple_indirect_block,
        index / (pointers * pointers),
        inode.fs_block_count,
    )?;
    let level2_index = index % (pointers * pointers);
    let leaf_block = read_pointer_from_indirect(
        &mut read_indirect_block,
        level2_block,
        level2_index / pointers,
        inode.fs_block_count,
    )?;
    read_pointer_from_indirect(
        &mut read_indirect_block,
        leaf_block,
        level2_index % pointers,
        inode.fs_block_count,
    )
}

fn read_pointer_from_indirect<F>(
    read_indirect_block: &mut F,
    block: u64,
    index: u64,
    fs_block_count: u64,
) -> Result<u64, NeoFsError>
where
    F: FnMut(u64, &mut [u8]) -> Result<(), NeoFsError>,
{
    validate_fs_block(block, fs_block_count)?;
    let mut bytes = vec![0u8; FS_BLOCK_SIZE];
    read_indirect_block(block, &mut bytes)?;
    pointer_from_indirect_bytes(&bytes, index, fs_block_count)
}

fn pointer_from_indirect_bytes(
    bytes: &[u8],
    index: u64,
    fs_block_count: u64,
) -> Result<u64, NeoFsError> {
    if index >= INDIRECT_POINTERS_PER_BLOCK as u64 {
        return Err(NeoFsError::InvalidInode);
    }
    let offset = index as usize * core::mem::size_of::<u64>();
    if bytes.len() != FS_BLOCK_SIZE || offset + 8 > bytes.len() {
        return Err(NeoFsError::InvalidInode);
    }
    let mut pointer = [0u8; 8];
    pointer.copy_from_slice(&bytes[offset..offset + 8]);
    validate_fs_block(u64::from_le_bytes(pointer), fs_block_count)
}

fn validate_fs_block(block: u64, fs_block_count: u64) -> Result<u64, NeoFsError> {
    if block == 0 || block >= fs_block_count {
        return Err(NeoFsError::InvalidInode);
    }
    Ok(block)
}

pub(super) fn read_fs_block(
    device: &Arc<dyn BlockDevice>,
    block: u64,
) -> Result<[u8; FS_BLOCK_SIZE], NeoFsError> {
    let mut out = [0u8; FS_BLOCK_SIZE];
    read_block_prefix(device, block, &mut out)?;
    Ok(out)
}

fn read_fs_block_vec(device: &Arc<dyn BlockDevice>, block: u64) -> Result<Vec<u8>, NeoFsError> {
    let mut out = vec![0u8; FS_BLOCK_SIZE];
    read_block_prefix(device, block, &mut out)?;
    Ok(out)
}

fn read_block_prefix(
    device: &Arc<dyn BlockDevice>,
    block: u64,
    out: &mut [u8],
) -> Result<(), NeoFsError> {
    if out.len() > FS_BLOCK_SIZE {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    if out.is_empty() {
        return Ok(());
    }
    read_bytes_at(device, block_offset(block)?, out)
}

pub(super) fn write_fs_block(
    device: &Arc<dyn BlockDevice>,
    block: u64,
    input: &[u8],
) -> Result<(), NeoFsError> {
    if input.len() != FS_BLOCK_SIZE {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    write_bytes_at(device, block_offset(block)?, input)
}

pub(super) fn write_superblock_copy(
    device: &Arc<dyn BlockDevice>,
    slot: usize,
    superblock: &Superblock,
) -> Result<(), NeoFsError> {
    let offset = SUPERBLOCK_COPY_OFFSETS
        .get(slot)
        .copied()
        .ok_or(NeoFsError::InvalidSuperblock)?;
    write_bytes_at(device, offset, &write_superblock(superblock))
}

fn write_superblock_copy_cached(
    cache: &Arc<NeoFsBlockCache>,
    slot: usize,
    superblock: &Superblock,
) -> Result<(), NeoFsError> {
    let offset = SUPERBLOCK_COPY_OFFSETS
        .get(slot)
        .copied()
        .ok_or(NeoFsError::InvalidSuperblock)?;
    cache.write_bytes_at(
        offset,
        &write_superblock(superblock),
        CachedBlockRole::Superblock,
    )
}

pub(super) fn read_bytes_at(
    device: &Arc<dyn BlockDevice>,
    offset: u64,
    out: &mut [u8],
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
        .checked_add(out.len() as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    if end > device_bytes {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }

    let mut copied = 0usize;
    while copied < out.len() {
        let cursor = offset + copied as u64;
        let lba = cursor / logical as u64;
        let in_block = (cursor % logical as u64) as usize;
        if in_block == 0 {
            let aligned = cmp::min(out.len() - copied, FS_BLOCK_SIZE);
            let take = aligned - (aligned % logical);
            if take != 0 {
                let dma = DmaBuffer::alloc_zeroed(take).map_err(|_| NeoFsError::BlockDevice)?;
                read_device_block_with_retry(device, lba, &dma)?;
                let slice = unsafe { dma.as_slice() };
                out[copied..copied + take].copy_from_slice(&slice[..take]);
                copied += take;
                continue;
            }
        }
        let take = cmp::min(out.len() - copied, logical - in_block);
        let dma = DmaBuffer::alloc_zeroed(logical).map_err(|_| NeoFsError::BlockDevice)?;
        read_device_block_with_retry(device, lba, &dma)?;
        let slice = unsafe { dma.as_slice() };
        out[copied..copied + take].copy_from_slice(&slice[in_block..in_block + take]);
        copied += take;
    }
    Ok(())
}

fn write_bytes_at(
    device: &Arc<dyn BlockDevice>,
    offset: u64,
    input: &[u8],
) -> Result<(), NeoFsError> {
    if input.is_empty() {
        return Ok(());
    }

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
        .checked_add(input.len() as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    if end > device_bytes {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }

    let mut copied = 0usize;
    while copied < input.len() {
        let cursor = offset + copied as u64;
        let lba = cursor / logical as u64;
        let in_block = (cursor % logical as u64) as usize;
        let take = cmp::min(input.len() - copied, logical - in_block);
        let dma = DmaBuffer::alloc_zeroed(logical).map_err(|_| NeoFsError::BlockDevice)?;
        if in_block != 0 || take != logical {
            read_device_block_with_retry(device, lba, &dma)?;
        }
        unsafe {
            dma.as_mut_slice()[in_block..in_block + take]
                .copy_from_slice(&input[copied..copied + take]);
        }
        write_device_block_with_retry(device, lba, &dma)?;
        copied += take;
    }
    Ok(())
}

/// NFS-W1: the bytes of one run of [`LARGE_WRITE_RUN`] at most.
#[cfg(feature = "neofs-large-write")]
pub(super) const LARGE_WRITE_RUN: usize = 64 * 1024;

/// NFS-W1: the large-write test hook: the next large write stops after its
/// blocks are durable and before its commit.
#[cfg(feature = "neo-test-neofs-large-write")]
pub(crate) static LARGE_WRITE_STOP_BEFORE_COMMIT: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// NFS-W1: the next large write fails its first data write (Chad's review,
/// item 2: the run must be given back).
#[cfg(feature = "neo-test-neofs-large-write")]
pub(crate) static LARGE_WRITE_FAIL_FIRST_WRITE: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// NFS-W1: the last large write's run, `base << 32 | count`.
#[cfg(feature = "neo-test-neofs-large-write")]
pub(crate) static LAST_LARGE_WRITE_RUN: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// NFS-W1: blocks a large write reserved, to give back if it fails.
#[cfg(feature = "neofs-large-write")]
#[derive(Clone, Copy)]
struct Run {
    base: u64,
    count: u64,
}

#[cfg(feature = "neofs-large-write")]
fn allocate_run(metadata: &mut NeoFsMetadata, count: u64) -> Result<Run, NeoFsError> {
    Ok(Run {
        base: allocate_contiguous_blocks(metadata, count)?,
        count,
    })
}

/// Give `run` back: the bump allocator rewinds when nothing was allocated
/// after it; otherwise the blocks are marked free in the free map (they lie
/// below `next_free_block`, so a free bit there is valid). A volume without a
/// free map that can't rewind keeps them used until its next sweep.
#[cfg(feature = "neofs-large-write")]
fn release_run(metadata: &mut NeoFsMetadata, run: Run) {
    if !alloc_logic::release_run(metadata, run.base, run.count) {
        crate::serial_println!(
            "[ kernel ] neofs: {} blocks from {} kept used (no free map)",
            run.count,
            run.base
        );
    }
}

/// NFS-W1: write whole filesystem blocks from `first`, as one device request,
/// past the cache.
#[cfg(feature = "neofs-large-write")]
fn write_fs_blocks(
    device: &Arc<dyn BlockDevice>,
    first: u64,
    bytes: &[u8],
) -> Result<(), NeoFsError> {
    let logical = device.geometry().logical_block_size as u64;
    if logical == 0
        || logical > FS_BLOCK_SIZE as u64
        || !(FS_BLOCK_SIZE as u64).is_multiple_of(logical)
        || bytes.is_empty()
        || !bytes.len().is_multiple_of(FS_BLOCK_SIZE)
    {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    let geometry = device.geometry();
    let device_bytes = geometry
        .block_count
        .checked_mul(logical)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    let end = block_offset(first)?
        .checked_add(bytes.len() as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    if end > device_bytes {
        return Err(NeoFsError::InvalidDeviceGeometry);
    }
    let dma = DmaBuffer::alloc_zeroed(bytes.len()).map_err(|_| NeoFsError::BlockDevice)?;
    // SAFETY: the buffer was just allocated and is not shared.
    unsafe { dma.as_mut_slice() }.copy_from_slice(bytes);
    let lba = block_offset(first)? / logical;
    retry_busy_block_io(|| device.write_block(lba, core::slice::from_ref(&dma)))
}

fn read_device_block_with_retry(
    device: &Arc<dyn BlockDevice>,
    lba: u64,
    dma: &Arc<DmaBuffer>,
) -> Result<(), NeoFsError> {
    retry_busy_block_io(|| device.read_block(lba, core::slice::from_ref(dma)))
}

fn write_device_block_with_retry(
    device: &Arc<dyn BlockDevice>,
    lba: u64,
    dma: &Arc<DmaBuffer>,
) -> Result<(), NeoFsError> {
    retry_busy_block_io(|| device.write_block(lba, core::slice::from_ref(dma)))
}

fn retry_busy_block_io<F>(mut op: F) -> Result<(), NeoFsError>
where
    F: FnMut() -> Result<(), BlockError>,
{
    for _ in 0..BLOCK_IO_BUSY_RETRIES {
        match op() {
            Ok(()) => return Ok(()),
            Err(BlockError::Unsupported) => {
                crate::sched::yield_now();
                crate::sched::reap_nested();
            }
            Err(_) => return Err(NeoFsError::BlockDevice),
        }
    }
    Err(NeoFsError::BlockDevice)
}

fn build_cow_content(
    device: &Arc<dyn BlockDevice>,
    old_record: &InodeRecord,
    offset: usize,
    input: &[u8],
) -> Result<Vec<u8>, NeoFsError> {
    let end = offset
        .checked_add(input.len())
        .ok_or(NeoFsError::FileTooLarge)?;
    if end > MAX_FILE_SIZE {
        return Err(NeoFsError::FileTooLarge);
    }
    let mut content = read_verified_logical_content(device, old_record)?;
    if end > content.len() {
        content.resize(end, 0);
    }
    content[offset..end].copy_from_slice(input);
    Ok(content)
}

fn build_cow_content_cached(
    cache: &Arc<NeoFsBlockCache>,
    old_record: &InodeRecord,
    offset: usize,
    input: &[u8],
) -> Result<Vec<u8>, NeoFsError> {
    let end = offset
        .checked_add(input.len())
        .ok_or(NeoFsError::FileTooLarge)?;
    if end > MAX_FILE_SIZE {
        return Err(NeoFsError::FileTooLarge);
    }
    let mut content = read_verified_logical_content_cached(cache, old_record)?;
    if end > content.len() {
        content.resize(end, 0);
    }
    content[offset..end].copy_from_slice(input);
    Ok(content)
}

/// A directory version's blocks: its inode block and its entry block (a
/// directory is one block: 64 entries). Not `validate_readable_inode`: an
/// empty directory records one block for no bytes.
fn collect_directory_blocks(
    inode_block: u64,
    record: &InodeRecord,
    out: &mut Vec<u64>,
) -> Result<(), NeoFsError> {
    if record.kind != super::format::NeoFsKind::Directory
        || record.block_count > DIRECT_BLOCKS as u64
    {
        return Err(NeoFsError::InvalidDirectory);
    }
    out.push(validate_fs_block(inode_block, record.fs_block_count)?);
    for block in record
        .direct_blocks
        .iter()
        .copied()
        .take(record.block_count as usize)
    {
        out.push(validate_fs_block(block, record.fs_block_count)?);
    }
    Ok(())
}

/// Mark `blocks` free in the map the next publish writes, deferred until
/// `generation` is durable on every copy (#180 rule 2). Returns the blocks
/// it changed, for [`unfree`]. A volume without a free map frees nothing.
fn free_superseded(
    metadata: &mut NeoFsMetadata,
    blocks: &[u64],
    generation: u64,
) -> Result<Vec<u64>, NeoFsError> {
    let total_blocks = metadata.superblock.total_blocks;
    let Some(free_map) = metadata.free_map.as_mut() else {
        return Ok(Vec::new());
    };
    // The whole list first (C1): a block out of range, already free or
    // named twice refuses the commit with no bit changed.
    alloc_logic::check_frees(&free_map.bytes, total_blocks, blocks)
        .map_err(|_| NeoFsError::InvalidInode)?;
    let mut freed = Vec::with_capacity(blocks.len());
    for block in blocks.iter().copied() {
        free_map_set(&mut free_map.bytes, block, true)?;
        freed.push(block);
    }
    if !freed.is_empty() {
        free_map.dirty = true;
    }
    for block in freed.iter().copied() {
        metadata.deferred.defer(block, generation);
    }
    Ok(freed)
}

/// Undo [`free_superseded`] when its publish failed: the blocks are still
/// named by the live tree.
fn unfree(metadata: &mut NeoFsMetadata, freed: &[u64]) -> Result<(), NeoFsError> {
    let Some(free_map) = metadata.free_map.as_mut() else {
        return Ok(());
    };
    for block in freed.iter().copied() {
        free_map_set(&mut free_map.bytes, block, false)?;
        metadata.deferred.cancel(block);
    }
    Ok(())
}

fn collect_regular_tree_blocks_cached(
    cache: &Arc<NeoFsBlockCache>,
    inode_block: u64,
    inode: &InodeRecord,
) -> Result<Vec<u64>, NeoFsError> {
    validate_readable_inode(inode)?;
    let mut blocks = Vec::new();
    blocks.push(validate_fs_block(inode_block, inode.fs_block_count)?);

    let mut remaining = inode.block_count;
    let direct_used = cmp::min(remaining, DIRECT_BLOCKS as u64);
    for block in inode
        .direct_blocks
        .iter()
        .copied()
        .take(direct_used as usize)
    {
        blocks.push(validate_fs_block(block, inode.fs_block_count)?);
    }
    remaining -= direct_used;

    let single_used = cmp::min(remaining, SINGLE_INDIRECT_BLOCKS as u64);
    if single_used != 0 {
        collect_indirect_tree_blocks_cached(
            cache,
            inode.single_indirect_block,
            1,
            single_used,
            inode.fs_block_count,
            &mut blocks,
        )?;
        remaining -= single_used;
    }

    let double_used = cmp::min(remaining, DOUBLE_INDIRECT_BLOCKS as u64);
    if double_used != 0 {
        collect_indirect_tree_blocks_cached(
            cache,
            inode.double_indirect_block,
            2,
            double_used,
            inode.fs_block_count,
            &mut blocks,
        )?;
        remaining -= double_used;
    }

    if remaining != 0 {
        collect_indirect_tree_blocks_cached(
            cache,
            inode.triple_indirect_block,
            3,
            remaining,
            inode.fs_block_count,
            &mut blocks,
        )?;
    }
    Ok(blocks)
}

fn collect_indirect_tree_blocks_cached(
    cache: &Arc<NeoFsBlockCache>,
    block: u64,
    depth: u8,
    covered_data_blocks: u64,
    fs_block_count: u64,
    blocks: &mut Vec<u64>,
) -> Result<(), NeoFsError> {
    if covered_data_blocks == 0 {
        return Ok(());
    }
    let block = validate_fs_block(block, fs_block_count)?;
    blocks.push(block);
    let mut bytes = vec![0u8; FS_BLOCK_SIZE];
    cache.read_block_prefix(block, &mut bytes)?;
    if depth == 1 {
        for idx in 0..covered_data_blocks {
            blocks.push(pointer_from_indirect_bytes(&bytes, idx, fs_block_count)?);
        }
        return Ok(());
    }

    let child_span = indirect_span(depth - 1)?;
    let mut remaining = covered_data_blocks;
    let mut slot = 0u64;
    while remaining != 0 {
        if slot >= INDIRECT_POINTERS_PER_BLOCK as u64 {
            return Err(NeoFsError::InvalidInode);
        }
        let child = pointer_from_indirect_bytes(&bytes, slot, fs_block_count)?;
        let child_blocks = cmp::min(remaining, child_span);
        collect_indirect_tree_blocks_cached(
            cache,
            child,
            depth - 1,
            child_blocks,
            fs_block_count,
            blocks,
        )?;
        remaining -= child_blocks;
        slot += 1;
    }
    Ok(())
}

fn write_content_blocks(
    device: &Arc<dyn BlockDevice>,
    blocks: &[u64],
    content: &[u8],
) -> Result<(), NeoFsError> {
    for (idx, block) in blocks.iter().copied().enumerate() {
        let start = idx * FS_BLOCK_SIZE;
        let end = cmp::min(start + FS_BLOCK_SIZE, content.len());
        let mut block_bytes = vec![0u8; FS_BLOCK_SIZE];
        if start < end {
            block_bytes[..end - start].copy_from_slice(&content[start..end]);
        }
        write_fs_block(device, block, &block_bytes)?;
    }
    Ok(())
}

fn write_content_blocks_cached(
    cache: &Arc<NeoFsBlockCache>,
    blocks: &[u64],
    content: &[u8],
) -> Result<(), NeoFsError> {
    for (idx, block) in blocks.iter().copied().enumerate() {
        let start = idx * FS_BLOCK_SIZE;
        let end = cmp::min(start + FS_BLOCK_SIZE, content.len());
        let mut block_bytes = vec![0u8; FS_BLOCK_SIZE];
        if start < end {
            block_bytes[..end - start].copy_from_slice(&content[start..end]);
        }
        cache.write_block(block, &block_bytes, CachedBlockRole::Data)?;
    }
    Ok(())
}

fn write_indirect_blocks_cached(
    cache: &Arc<NeoFsBlockCache>,
    blocks: &[RuntimeIndirectBlock],
) -> Result<(), NeoFsError> {
    for (block, bytes) in blocks {
        if bytes.len() != FS_BLOCK_SIZE {
            return Err(NeoFsError::InvalidInode);
        }
        cache.write_block(*block, bytes, CachedBlockRole::Data)?;
    }
    Ok(())
}

fn put_indirect_pointer(block: &mut [u8], index: u64, value: u64) -> Result<(), NeoFsError> {
    if block.len() != FS_BLOCK_SIZE || index >= INDIRECT_POINTERS_PER_BLOCK as u64 {
        return Err(NeoFsError::InvalidInode);
    }
    let offset = index as usize * core::mem::size_of::<u64>();
    let end = offset
        .checked_add(core::mem::size_of::<u64>())
        .ok_or(NeoFsError::InvalidInode)?;
    let slot = block.get_mut(offset..end).ok_or(NeoFsError::InvalidInode)?;
    slot.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

impl alloc_logic::Space for NeoFsMetadata {
    fn next_free_block(&self) -> u64 {
        self.superblock.next_free_block
    }

    fn set_next_free_block(&mut self, block: u64) {
        self.superblock.next_free_block = block;
    }

    fn total_blocks(&self) -> u64 {
        self.superblock.total_blocks
    }

    fn next_generation(&self) -> u32 {
        self.superblock.next_generation
    }

    fn set_next_generation(&mut self, generation: u32) {
        self.superblock.next_generation = generation;
    }

    fn free_map(&mut self) -> Option<(&mut [u8], &mut bool, &dyn alloc_logic::Deferred)> {
        let deferred = &self.deferred;
        self.free_map
            .as_mut()
            .map(|m| (m.bytes.as_mut_slice(), &mut m.dirty, deferred as _))
    }

    /// #180: blocks freed by a commit become reusable once that commit's
    /// generation is durable on every copy.
    fn release_deferred(&mut self) {
        let durable = self.durable_generation.load(Ordering::Acquire);
        self.deferred.release(durable);
    }
}

impl alloc_logic::Deferred for DeferredFrees {
    fn is_deferred(&self, block: u64) -> bool {
        DeferredFrees::is_deferred(self, block)
    }
}

impl From<alloc_logic::AllocError> for NeoFsError {
    fn from(e: alloc_logic::AllocError) -> Self {
        match e {
            alloc_logic::AllocError::NoSpace => NeoFsError::NoSpace,
            alloc_logic::AllocError::Invalid => NeoFsError::InvalidSuperblock,
        }
    }
}

fn allocate_one_block(metadata: &mut NeoFsMetadata) -> Result<u64, NeoFsError> {
    Ok(alloc_logic::allocate_one_block(metadata)?)
}

fn allocate_contiguous_blocks(metadata: &mut NeoFsMetadata, count: u64) -> Result<u64, NeoFsError> {
    Ok(alloc_logic::allocate_contiguous_blocks(metadata, count)?)
}

fn free_map_get(bytes: &[u8], block: u64) -> Result<bool, NeoFsError> {
    Ok(alloc_logic::free_map_get(bytes, block)?)
}

fn free_map_set(bytes: &mut [u8], block: u64, free: bool) -> Result<(), NeoFsError> {
    Ok(alloc_logic::free_map_set(bytes, block, free)?)
}

pub(super) fn read_free_map_bytes(
    device: &Arc<dyn BlockDevice>,
    superblock: &Superblock,
) -> Result<Option<Vec<u8>>, NeoFsError> {
    let Some(free_map) = superblock.free_map else {
        return Ok(None);
    };
    let len = free_map_byte_len(free_map.block_count)?;
    let mut bytes = vec![0u8; len];
    for idx in 0..free_map.block_count {
        let start = usize::try_from(idx)
            .ok()
            .and_then(|idx| idx.checked_mul(FS_BLOCK_SIZE))
            .ok_or(NeoFsError::InvalidSuperblock)?;
        read_block_prefix(
            device,
            free_map.start_block + idx,
            &mut bytes[start..start + FS_BLOCK_SIZE],
        )?;
    }
    if bytes.len() != len || hash(&bytes) != free_map.hash {
        return Err(NeoFsError::InvalidSuperblock);
    }
    Ok(Some(bytes))
}

pub(super) fn validate_free_map_against_live_set(
    device: &Arc<dyn BlockDevice>,
    superblock: &Superblock,
    bytes: Option<&[u8]>,
) -> Result<(), NeoFsError> {
    let Some(bytes) = bytes else {
        return Ok(());
    };
    let expected_blocks = free_map_block_count_for_total(superblock.total_blocks)?;
    let free_map = superblock.free_map.ok_or(NeoFsError::InvalidSuperblock)?;
    if free_map.block_count != expected_blocks || bytes.len() != free_map_byte_len(expected_blocks)?
    {
        return Err(NeoFsError::InvalidSuperblock);
    }

    let (live, _live_count) = collect_live_blocks(device, superblock)?;
    for block in 0..superblock.total_blocks {
        let is_free = free_map_get(bytes, block)?;
        if live[block as usize] != 0 && is_free {
            return Err(NeoFsError::InvalidSuperblock);
        }
        if block >= superblock.next_free_block && is_free {
            return Err(NeoFsError::InvalidSuperblock);
        }
    }
    Ok(())
}

fn build_swept_free_map(superblock: &Superblock, live: &[u8]) -> Result<Vec<u8>, NeoFsError> {
    let block_count = superblock
        .free_map
        .ok_or(NeoFsError::UnsupportedType)?
        .block_count;
    if block_count != free_map_block_count_for_total(superblock.total_blocks)? {
        return Err(NeoFsError::InvalidSuperblock);
    }
    let mut bytes = vec![0u8; free_map_byte_len(block_count)?];
    let limit = cmp::min(superblock.next_free_block, superblock.total_blocks);
    for block in 1..limit {
        if live[block as usize] == 0 {
            free_map_set(&mut bytes, block, true)?;
        }
    }
    Ok(bytes)
}

fn collect_live_blocks(
    device: &Arc<dyn BlockDevice>,
    superblock: &Superblock,
) -> Result<(Vec<u8>, u64), NeoFsError> {
    let live_len =
        usize::try_from(superblock.total_blocks).map_err(|_| NeoFsError::InvalidSuperblock)?;
    let mut live = vec![0u8; live_len];
    for offset in SUPERBLOCK_COPY_OFFSETS {
        let block = offset / FS_BLOCK_SIZE as u64;
        if block < superblock.total_blocks {
            mark_live(&mut live, block)?;
        }
    }
    if let Some(free_map) = superblock.free_map {
        for block in free_map.start_block..free_map.start_block + free_map.block_count {
            mark_live(&mut live, block)?;
        }
    }
    mark_inode_tree(
        device,
        superblock.total_blocks,
        superblock.root_inode_block,
        &mut live,
        0,
    )?;
    let count = live.iter().filter(|byte| **byte != 0).count() as u64;
    Ok((live, count))
}

fn mark_inode_tree(
    device: &Arc<dyn BlockDevice>,
    total_blocks: u64,
    inode_block: u64,
    live: &mut [u8],
    depth: usize,
) -> Result<(), NeoFsError> {
    if depth > 16 {
        return Err(NeoFsError::UnsupportedType);
    }
    // An inode reached twice (two entries naming it, or a cycle) is refused
    // by `mark_live`, as is any block two owners name (#206 AAG, C1).
    mark_live(live, inode_block)?;
    let (inode, _bytes) = read_inode_from_block(device, inode_block, total_blocks)?;
    mark_inode_file_blocks(device, &inode, live)?;
    if inode.kind == super::format::NeoFsKind::Directory {
        let content = read_verified_logical_content(device, &inode)?;
        let entries = parse_directory_entries(&inode, &content)?;
        for entry in entries {
            mark_inode_tree(device, total_blocks, entry.child_block, live, depth + 1)?;
        }
    }
    Ok(())
}

fn mark_inode_file_blocks(
    device: &Arc<dyn BlockDevice>,
    inode: &InodeRecord,
    live: &mut [u8],
) -> Result<(), NeoFsError> {
    validate_readable_inode(inode)?;
    let mut remaining = inode.block_count;
    let direct_used = cmp::min(remaining, DIRECT_BLOCKS as u64);
    for block in inode
        .direct_blocks
        .iter()
        .copied()
        .take(direct_used as usize)
    {
        mark_live(live, validate_fs_block(block, inode.fs_block_count)?)?;
    }
    remaining -= direct_used;

    let single_used = cmp::min(remaining, SINGLE_INDIRECT_BLOCKS as u64);
    if single_used != 0 {
        mark_indirect_file_blocks(
            device,
            inode.single_indirect_block,
            1,
            single_used,
            inode.fs_block_count,
            live,
        )?;
        remaining -= single_used;
    }

    let double_used = cmp::min(remaining, DOUBLE_INDIRECT_BLOCKS as u64);
    if double_used != 0 {
        mark_indirect_file_blocks(
            device,
            inode.double_indirect_block,
            2,
            double_used,
            inode.fs_block_count,
            live,
        )?;
        remaining -= double_used;
    }

    if remaining != 0 {
        mark_indirect_file_blocks(
            device,
            inode.triple_indirect_block,
            3,
            remaining,
            inode.fs_block_count,
            live,
        )?;
    }
    Ok(())
}

fn mark_indirect_file_blocks(
    device: &Arc<dyn BlockDevice>,
    block: u64,
    depth: u8,
    covered_data_blocks: u64,
    fs_block_count: u64,
    live: &mut [u8],
) -> Result<(), NeoFsError> {
    if covered_data_blocks == 0 {
        return Ok(());
    }
    let block = validate_fs_block(block, fs_block_count)?;
    mark_live(live, block)?;
    let bytes = read_fs_block_vec(device, block)?;
    if depth == 1 {
        for idx in 0..covered_data_blocks {
            let data = pointer_from_indirect_bytes(&bytes, idx, fs_block_count)?;
            mark_live(live, data)?;
        }
        return Ok(());
    }

    let child_span = indirect_span(depth - 1)?;
    let mut remaining = covered_data_blocks;
    let mut slot = 0u64;
    while remaining != 0 {
        if slot >= INDIRECT_POINTERS_PER_BLOCK as u64 {
            return Err(NeoFsError::InvalidInode);
        }
        let child = pointer_from_indirect_bytes(&bytes, slot, fs_block_count)?;
        let child_blocks = cmp::min(remaining, child_span);
        mark_indirect_file_blocks(device, child, depth - 1, child_blocks, fs_block_count, live)?;
        remaining -= child_blocks;
        slot += 1;
    }
    Ok(())
}

fn indirect_span(depth: u8) -> Result<u64, NeoFsError> {
    match depth {
        1 => Ok(INDIRECT_POINTERS_PER_BLOCK as u64),
        2 => Ok(DOUBLE_INDIRECT_BLOCKS as u64),
        _ => Err(NeoFsError::InvalidInode),
    }
}

fn mark_live(live: &mut [u8], block: u64) -> Result<(), NeoFsError> {
    alloc_logic::mark_live_once(live, block).map_err(|_| NeoFsError::InvalidInode)
}

fn count_free_blocks(bytes: &[u8], total_blocks: u64) -> Result<u64, NeoFsError> {
    let mut count = 0u64;
    for block in 0..total_blocks {
        if free_map_get(bytes, block)? {
            count += 1;
        }
    }
    Ok(count)
}

fn write_free_map_blocks(
    device: &Arc<dyn BlockDevice>,
    start_block: u64,
    block_count: u64,
    bytes: &[u8],
) -> Result<(), NeoFsError> {
    if bytes.len() != free_map_byte_len(block_count)? {
        return Err(NeoFsError::InvalidSuperblock);
    }
    for idx in 0..block_count {
        let start = usize::try_from(idx)
            .ok()
            .and_then(|idx| idx.checked_mul(FS_BLOCK_SIZE))
            .ok_or(NeoFsError::InvalidSuperblock)?;
        let mut block_bytes = vec![0u8; FS_BLOCK_SIZE];
        block_bytes.copy_from_slice(&bytes[start..start + FS_BLOCK_SIZE]);
        write_fs_block(device, start_block + idx, &block_bytes)?;
    }
    Ok(())
}

fn write_free_map_blocks_cached(
    cache: &Arc<NeoFsBlockCache>,
    start_block: u64,
    block_count: u64,
    bytes: &[u8],
) -> Result<(), NeoFsError> {
    if bytes.len() != free_map_byte_len(block_count)? {
        return Err(NeoFsError::InvalidSuperblock);
    }
    for idx in 0..block_count {
        let start = usize::try_from(idx)
            .ok()
            .and_then(|idx| idx.checked_mul(FS_BLOCK_SIZE))
            .ok_or(NeoFsError::InvalidSuperblock)?;
        let mut block_bytes = vec![0u8; FS_BLOCK_SIZE];
        block_bytes.copy_from_slice(&bytes[start..start + FS_BLOCK_SIZE]);
        cache.write_block(start_block + idx, &block_bytes, CachedBlockRole::FreeMap)?;
    }
    Ok(())
}

fn bump_superblock_generation(superblock: &mut Superblock) -> Result<(), NeoFsError> {
    superblock.generation = superblock
        .generation
        .checked_add(1)
        .ok_or(NeoFsError::InvalidSuperblock)?;
    Ok(())
}

fn write_superblock_copies(
    device: &Arc<dyn BlockDevice>,
    superblock: &Superblock,
) -> Result<(), NeoFsError> {
    let geometry = device.geometry();
    let device_bytes = geometry
        .block_count
        .checked_mul(geometry.logical_block_size as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    let bytes = write_superblock(superblock);
    let mut wrote = false;
    for offset in SUPERBLOCK_COPY_OFFSETS {
        if offset + SUPERBLOCK_SIZE as u64 > device_bytes {
            continue;
        }
        write_bytes_at(device, offset, &bytes)?;
        wrote = true;
    }
    if !wrote {
        return Err(NeoFsError::InvalidSuperblock);
    }
    Ok(())
}

fn write_superblock_copies_cached(
    cache: &Arc<NeoFsBlockCache>,
    device: &Arc<dyn BlockDevice>,
    superblock: &Superblock,
) -> Result<(), NeoFsError> {
    let geometry = device.geometry();
    let device_bytes = geometry
        .block_count
        .checked_mul(geometry.logical_block_size as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;
    let bytes = write_superblock(superblock);
    let mut wrote = false;
    for offset in SUPERBLOCK_COPY_OFFSETS {
        if offset + SUPERBLOCK_SIZE as u64 > device_bytes {
            continue;
        }
        cache.write_bytes_at(offset, &bytes, CachedBlockRole::Superblock)?;
        wrote = true;
    }
    if !wrote {
        return Err(NeoFsError::InvalidSuperblock);
    }
    Ok(())
}
