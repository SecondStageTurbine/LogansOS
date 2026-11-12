// SPDX-License-Identifier: MPL-2.0
extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::block::BlockDevice;
use crate::fs::{Inode, MountedFs, VfsError};

use super::NeoFsError;
use super::format::{
    DirEntry, FS_BLOCK_SIZE, InodeRecord, NeoFsKind, SUPERBLOCK_COPY_OFFSETS, SUPERBLOCK_SIZE,
    Superblock, hash, parse_directory_entries, parse_superblock,
};
use super::inode::{
    NeoFsState, NeoInode, read_bytes_at, read_free_map_bytes, read_inode_from_block,
    read_verified_logical_content, validate_free_map_against_live_set, validate_readable_inode,
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct MountReport {
    pub selected_superblock_slot: usize,
    pub generation: u64,
    pub valid_superblocks: usize,
    pub rejected_superblocks: usize,
    pub rejected_superblock_mask: u8,
    pub root_kind: NeoFsKind,
    pub model_kind: NeoFsKind,
}

pub struct NeoFs {
    state: Arc<NeoFsState>,
    report: MountReport,
}

struct MountSelection {
    superblock: Superblock,
    root: InodeRecord,
    entries: Vec<DirEntry>,
    free_map_bytes: Option<Vec<u8>>,
    model_kind: NeoFsKind,
    valid_superblocks: usize,
    rejected_superblocks: usize,
    rejected_superblock_mask: u8,
}

impl NeoFs {
    pub fn mount_readonly(device: Arc<dyn BlockDevice>) -> Result<Self, NeoFsError> {
        Self::mount(device, false)
    }

    pub fn mount_readwrite(device: Arc<dyn BlockDevice>) -> Result<Self, NeoFsError> {
        Self::mount(device, true)
    }

    pub fn probe(device: &Arc<dyn BlockDevice>) -> Result<MountReport, NeoFsError> {
        let selected = select_superblock(device)?;
        Ok(report_from_selection(&selected))
    }

    fn mount(device: Arc<dyn BlockDevice>, writable: bool) -> Result<Self, NeoFsError> {
        // The commit order rests on flushes (`commit_logic`): without one,
        // nothing orders the superblocks after the blocks they name, so a cut
        // could leave no mountable generation. Refuse rather than promise.
        if writable && !device.supports_flush() {
            crate::serial_println!(
                "[ kernel ] neofs: refusing a writable mount: the device cannot flush its write cache"
            );
            return Err(NeoFsError::NoDurableFlush);
        }
        let selected = select_superblock(&device)?;
        let state = Arc::new(NeoFsState::new(
            device.clone(),
            selected.superblock,
            writable,
            selected.superblock.root_inode_block,
            selected.root.clone(),
            selected.entries.clone(),
            selected.free_map_bytes.clone(),
        ));

        let report = report_from_selection(&selected);
        Ok(Self { state, report })
    }

    pub fn report(&self) -> MountReport {
        self.report
    }

    pub fn lookup(&self, path: &str) -> Result<Arc<dyn Inode>, VfsError> {
        let normalized = normalize_relative_path(path)?;
        if normalized.is_empty() {
            let (root_block, root_record) = self.state.root_snapshot();
            let inode: Arc<dyn Inode> = Arc::new(NeoInode::new(
                self.state.clone(),
                None,
                root_block,
                root_record,
            ));
            return Ok(inode);
        }
        let (inode_block, record) = self.state.resolve_path(&normalized).map_err(neofs_to_vfs)?;
        let state = self.state.clone();
        let inode_value = NeoInode::new(state, Some(normalized), inode_block, record);
        let typed_inode = Arc::new(inode_value);
        let inode: Arc<dyn Inode> = typed_inode;
        Ok(inode)
    }

    pub fn create_regular(&self, path: &str) -> Result<Arc<dyn Inode>, VfsError> {
        if !self.state.is_writable() {
            return Err(VfsError::InsufficientRights);
        }
        let name = validate_create_path(path)?;
        let (inode_block, record) = self
            .state
            .create_regular(name.clone())
            .map_err(neofs_to_vfs)?;
        let inode = Arc::new(NeoInode::new(
            self.state.clone(),
            Some(name),
            inode_block,
            record,
        ));
        let created: Arc<dyn Inode> = inode;
        Ok(created)
    }
}

/// NFS-W1: a file of any size, written in one piece.
#[cfg(feature = "neofs-large-write")]
impl NeoFs {
    /// Create the regular file `path` holding `bytes` (up to 2 GiB), its
    /// parent existing and the name new. On success the file and its
    /// directory entry are committed; on any error nothing is.
    pub fn create_file_from_bytes(
        &self,
        path: &str,
        bytes: &[u8],
    ) -> Result<Arc<dyn Inode>, VfsError> {
        if !self.state.is_writable() {
            return Err(VfsError::InsufficientRights);
        }
        let name = validate_create_path(path)?;
        let (inode_block, record) = self
            .state
            .create_file_from_bytes(name.clone(), bytes)
            .map_err(neofs_to_vfs)?;
        let inode: Arc<dyn Inode> = Arc::new(NeoInode::new(
            self.state.clone(),
            Some(name),
            inode_block,
            record,
        ));
        Ok(inode)
    }
}

/// OT-87 R4.3: what the installer needs beyond the VFS surface.
#[cfg(feature = "installer")]
impl NeoFs {
    /// An empty directory at `path` (its parent must exist).
    pub fn create_directory(&self, path: &str) -> Result<(), VfsError> {
        if !self.state.is_writable() {
            return Err(VfsError::InsufficientRights);
        }
        let name = validate_create_path(path)?;
        self.state
            .create_directory(name)
            .map(|_| ())
            .map_err(neofs_to_vfs)
    }

    /// Grow the volume to `total_blocks` 4 KiB blocks.
    pub fn grow(&self, total_blocks: u64) -> Result<(), VfsError> {
        self.state.grow(total_blocks).map_err(neofs_to_vfs)
    }

    /// The volume's size in 4 KiB blocks.
    pub fn total_blocks(&self) -> u64 {
        self.state.total_blocks()
    }

    /// Write every dirty block and flush the device (the mount-wide drain
    /// `fsync` uses).
    pub fn sync(&self) -> Result<(), VfsError> {
        self.state.fsync_regular().map_err(neofs_to_vfs)
    }
}

impl MountedFs for NeoFs {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        NeoFs::lookup(self, "")
    }

    /// OT-90 P3a (K2): freeze, bounded drain, stop the writeback worker.
    #[cfg(feature = "power-clean")]
    fn shutdown(&self, stall_ticks: u64) -> Option<super::ShutdownReport> {
        Some(self.state.shutdown(stall_ticks))
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        NeoFs::lookup(self, relative)
    }

    fn create_regular(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        NeoFs::create_regular(self, relative)
    }
}

impl NeoFs {
    #[cfg(feature = "neo-test")]
    pub fn debug_generation(&self) -> u64 {
        self.state.generation()
    }

    #[cfg(feature = "neo-test")]
    pub fn debug_regular_blocks(&self, path: &str) -> Result<([u64; 8], u64), VfsError> {
        let normalized = normalize_relative_path(path)?;
        self.state
            .debug_regular_blocks(&normalized)
            .map_err(neofs_to_vfs)
    }

    /// The inode block and generation a path resolves to now.
    #[cfg(feature = "neo-test")]
    pub fn debug_inode_version(&self, path: &str) -> Result<(u64, u32), VfsError> {
        let normalized = normalize_relative_path(path)?;
        let (block, record) = self.state.resolve_path(&normalized).map_err(neofs_to_vfs)?;
        Ok((block, record.generation))
    }

    #[cfg(feature = "neo-test")]
    pub fn debug_cache_stats(&self) -> super::CacheStats {
        self.state.debug_cache_stats()
    }

    #[cfg(all(feature = "neo-test", feature = "neofs-large-write"))]
    pub fn debug_forget_range(&self, first: u64, count: u64) {
        self.state.debug_forget_range(first, count);
    }

    #[cfg(feature = "neo-test")]
    pub fn debug_set_writeback_hold(&self, hold: bool) {
        self.state.debug_set_writeback_hold(hold);
    }

    #[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
    pub fn debug_hold_entries_yielding(
        &self,
        ticks: u64,
        gate: &core::sync::atomic::AtomicU8,
    ) -> (u64, bool) {
        self.state.debug_hold_entries_yielding(ticks, gate)
    }

    #[cfg(feature = "neo-test")]
    pub fn debug_wait_clean_cache(&self, max_yields: usize) -> bool {
        self.state.debug_wait_clean_cache(max_yields)
    }

    #[cfg(feature = "neo-test")]
    pub fn debug_stop_writeback_worker(&self) -> super::WriteBackWorkerStopReport {
        self.state.debug_stop_writeback_worker()
    }

    #[cfg(feature = "neo-test-phase3-neofs-crash-mid-create-recovery")]
    pub fn debug_inject_create_before_superblock(&self, path: &str) -> Result<(), VfsError> {
        let name = validate_create_path(path)?;
        self.state
            .debug_inject_create_before_superblock(name)
            .map_err(neofs_to_vfs)
    }

    #[cfg(feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery")]
    pub fn debug_inject_overwrite_before_superblock(
        &self,
        path: &str,
        input: &[u8],
    ) -> Result<(), VfsError> {
        let name = normalize_relative_path(path)?;
        self.state
            .debug_inject_overwrite_before_superblock(&name, 0, input)
            .map_err(neofs_to_vfs)
    }

    #[cfg(feature = "neo-test-neofs-file-replace")]
    pub fn debug_replace_before_publish(&self, path: &str, bytes: &[u8]) -> Result<(), VfsError> {
        self.state
            .debug_replace_before_publish(path, bytes)
            .map_err(neofs_to_vfs)
    }

    /// The inode block `path` resolves to, and its bytes read from the device.
    #[cfg(any(
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub fn debug_raw_inode(&self, path: &str) -> Result<(u64, alloc::vec::Vec<u8>), VfsError> {
        let (block, _record) = self.state.resolve_path(path).map_err(neofs_to_vfs)?;
        let bytes = self.debug_raw_block(block)?;
        Ok((block, bytes[..super::format::INODE_SIZE].to_vec()))
    }

    /// `block`'s bytes on the device, after draining the cache.
    /// F0's F2 leg (`debug_neofs_cold_cache`).
    #[cfg(feature = "neo-test-neofs-dir-ops")]
    pub fn debug_cold_cache(&self) -> Result<(), VfsError> {
        self.state.debug_cold_cache().map_err(neofs_to_vfs)
    }

    #[cfg(any(
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub fn debug_raw_block(&self, block: u64) -> Result<alloc::vec::Vec<u8>, VfsError> {
        self.state.debug_drain_all().map_err(neofs_to_vfs)?;
        let bytes = super::inode::read_fs_block(&self.state.device, block).map_err(neofs_to_vfs)?;
        Ok(bytes.to_vec())
    }

    #[cfg(any(
        feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery",
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub fn debug_drain_to_barrier(&self) -> Result<u64, VfsError> {
        self.state.debug_drain_to_barrier().map_err(neofs_to_vfs)
    }

    #[cfg(feature = "neo-test-phase3-neofs-superblock-rotation-recovery")]
    pub fn debug_write_invalid_superblock_copy(&self, slot: usize) -> Result<(), VfsError> {
        self.state
            .debug_write_invalid_superblock_copy(slot)
            .map_err(neofs_to_vfs)
    }

    #[cfg(any(
        feature = "neo-test-phase3-neofs-3c",
        feature = "neo-test-neofs-large-write",
        feature = "neo-test-neofs-overwrite-frees",
        feature = "neo-test-neofs-grow-write",
        feature = "neo-test-neofs-file-replace",
        feature = "neo-test-neofs-dir-ops"
    ))]
    pub fn debug_free_map_stats(&self) -> Result<super::FreeMapStats, VfsError> {
        self.state.debug_free_map_stats().map_err(neofs_to_vfs)
    }

    #[cfg(feature = "neo-test-phase3-neofs-3c")]
    pub fn debug_run_quiescent_gc(&self) -> Result<super::GcStats, VfsError> {
        self.state.debug_run_quiescent_gc().map_err(neofs_to_vfs)
    }

    #[cfg(feature = "neo-test-phase3-neofs-3c")]
    pub fn debug_inject_gc_before_superblock(&self) -> Result<(), VfsError> {
        self.state
            .debug_inject_gc_before_superblock()
            .map_err(neofs_to_vfs)
    }
}

fn report_from_selection(selected: &MountSelection) -> MountReport {
    MountReport {
        selected_superblock_slot: selected.superblock.slot,
        generation: selected.superblock.generation,
        valid_superblocks: selected.valid_superblocks,
        rejected_superblocks: selected.rejected_superblocks,
        rejected_superblock_mask: selected.rejected_superblock_mask,
        root_kind: selected.root.kind,
        model_kind: selected.model_kind,
    }
}

fn select_superblock(device: &Arc<dyn BlockDevice>) -> Result<MountSelection, NeoFsError> {
    let geometry = device.geometry();
    let device_bytes = geometry
        .block_count
        .checked_mul(geometry.logical_block_size as u64)
        .ok_or(NeoFsError::InvalidDeviceGeometry)?;

    let mut selected: Option<MountSelection> = None;
    let mut valid = 0usize;
    let mut rejected = 0usize;
    let mut rejected_mask = 0u8;
    for (slot, offset) in SUPERBLOCK_COPY_OFFSETS.iter().copied().enumerate() {
        if offset + SUPERBLOCK_SIZE as u64 > device_bytes {
            continue;
        }
        let mut bytes = [0u8; SUPERBLOCK_SIZE];
        let parsed = read_bytes_at(device, offset, &mut bytes)
            .and_then(|_| parse_superblock(&bytes, slot))
            .and_then(|sb| {
                let fs_bytes = sb
                    .total_blocks
                    .checked_mul(FS_BLOCK_SIZE as u64)
                    .ok_or(NeoFsError::InvalidSuperblock)?;
                if fs_bytes > device_bytes {
                    Err(NeoFsError::InvalidSuperblock)
                } else {
                    Ok(sb)
                }
            })
            .and_then(|sb| validate_mount_candidate(device, sb));
        match parsed {
            Ok(candidate) => {
                valid += 1;
                if selected
                    .as_ref()
                    .map(|current| candidate.superblock.generation > current.superblock.generation)
                    .unwrap_or(true)
                {
                    selected = Some(candidate);
                }
            }
            Err(_) => {
                rejected += 1;
                rejected_mask |= 1 << slot;
            }
        }
    }

    let mut selected = selected.ok_or(NeoFsError::NoValidSuperblock)?;
    selected.valid_superblocks = valid;
    selected.rejected_superblocks = rejected;
    selected.rejected_superblock_mask = rejected_mask;
    Ok(selected)
}

fn validate_mount_candidate(
    device: &Arc<dyn BlockDevice>,
    superblock: Superblock,
) -> Result<MountSelection, NeoFsError> {
    let (root, root_bytes) =
        read_inode_from_block(device, superblock.root_inode_block, superblock.total_blocks)?;
    if hash(&root_bytes) != superblock.root_inode_hash {
        return Err(NeoFsError::RootHashMismatch);
    }
    if root.kind != NeoFsKind::Directory {
        return Err(NeoFsError::RootNotDirectory);
    }
    let root_content = read_verified_logical_content(device, &root)?;
    let entries = parse_directory_entries(&root, &root_content)?;
    let model_kind = validate_children(device, superblock.total_blocks, &entries)?;
    let free_map_bytes = read_free_map_bytes(device, &superblock)?;
    validate_free_map_against_live_set(device, &superblock, free_map_bytes.as_deref())?;
    Ok(MountSelection {
        superblock,
        root,
        entries,
        free_map_bytes,
        model_kind,
        valid_superblocks: 0,
        rejected_superblocks: 0,
        rejected_superblock_mask: 0,
    })
}

fn validate_children(
    device: &Arc<dyn BlockDevice>,
    total_blocks: u64,
    dir_entries: &[DirEntry],
) -> Result<NeoFsKind, NeoFsError> {
    let mut saw_hello = false;
    let mut model_kind = None;

    for entry in dir_entries {
        let (inode, _bytes) = read_inode_from_block(device, entry.child_block, total_blocks)?;
        if inode.kind != entry.kind {
            return Err(NeoFsError::DirectoryTypeMismatch);
        }
        if should_verify_child_content_at_mount(inode.kind) {
            read_verified_logical_content(device, &inode)?;
        } else {
            validate_readable_inode(&inode)?;
        }
        if entry.name.as_str() == "hello.txt" && inode.kind == NeoFsKind::Regular {
            saw_hello = true;
        }
        if entry.name.as_str() == "model.neom" && inode.kind == NeoFsKind::Model {
            model_kind = Some(inode.kind);
        }
    }

    if !saw_hello {
        return Err(NeoFsError::NotFound);
    }
    model_kind.ok_or(NeoFsError::UnsupportedType)
}

fn should_verify_child_content_at_mount(kind: NeoFsKind) -> bool {
    kind != NeoFsKind::Model
}

fn normalize_relative_path(path: &str) -> Result<String, VfsError> {
    if path.starts_with('/') {
        return Err(VfsError::Invalid);
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            segment => {
                validate_path_component(segment)?;
                parts.push(segment);
            }
        }
    }

    let mut normalized = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            normalized.push('/');
        }
        normalized.push_str(part);
    }
    Ok(normalized)
}

fn validate_path_component(segment: &str) -> Result<(), VfsError> {
    let bytes = segment.as_bytes();
    if bytes.is_empty() || bytes.len() > 47 || bytes.contains(&0) {
        return Err(VfsError::Invalid);
    }
    Ok(())
}

fn validate_create_path(path: &str) -> Result<String, VfsError> {
    let normalized = normalize_relative_path(path)?;
    if normalized.is_empty() {
        return Err(VfsError::Invalid);
    }
    Ok(normalized)
}

fn neofs_to_vfs(error: NeoFsError) -> VfsError {
    match error {
        NeoFsError::NotFound => VfsError::NotFound,
        NeoFsError::ReadOnly => VfsError::InsufficientRights,
        NeoFsError::NoDurableFlush => VfsError::Unsupported,
        _ => VfsError::Invalid,
    }
}
