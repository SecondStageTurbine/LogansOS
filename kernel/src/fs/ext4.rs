// SPDX-License-Identifier: MPL-2.0
//! Read-only ext4 foreign filesystem support for Phase 3 BUILD-4D.
//!
//! This driver intentionally stays behind the existing `MountedFs`/`Inode`
//! boundary. It supports extent-backed directories and regular files only;
//! symlinks and special files are skipped at directory import time.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp;

use crate::drivers::block::{BlockDevice, DmaBuffer};
use crate::fs::{FileRights, Inode, InodeKind, MountedFs, VfsDirEntry, VfsError};

const SECTOR_SIZE: usize = 512;
const EXT4_SUPERBLOCK_OFFSET: u64 = 1024;
const EXT4_SUPERBLOCK_SIZE: usize = 1024;
const EXT4_SUPER_MAGIC: u16 = 0xef53;
const EXT4_ROOT_INO: u32 = 2;

const EXT4_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
const EXT4_FEATURE_INCOMPAT_EXTENTS: u32 = 0x0040;
const EXT4_FEATURE_INCOMPAT_64BIT: u32 = 0x0080;
const EXT4_FEATURE_INCOMPAT_ENCRYPT: u32 = 0x0001_0000;

const EXT4_EXTENTS_FL: u32 = 0x0008_0000;
const EXT4_INLINE_DATA_FL: u32 = 0x1000_0000;
const EXT4_EXTENT_MAGIC: u16 = 0xf30a;

const S_IFMT: u16 = 0xf000;
const S_IFREG: u16 = 0x8000;
const S_IFDIR: u16 = 0x4000;
const S_IFLNK: u16 = 0xa000;

const EXT4_FT_UNKNOWN: u8 = 0;
const EXT4_FT_REG_FILE: u8 = 1;
const EXT4_FT_DIR: u8 = 2;
const EXT4_FT_SYMLINK: u8 = 7;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Ext4Error {
    BlockDevice,
    Invalid,
    NotFound,
    Unsupported,
}

pub struct Ext4 {
    state: Arc<Ext4State>,
}

struct Ext4State {
    device: Arc<dyn BlockDevice>,
    block_size: usize,
    block_count: u64,
    inode_count: u32,
    first_data_block: u32,
    inodes_per_group: u32,
    inode_size: usize,
    group_count: u32,
    desc_size: usize,
    has_dir_file_type: bool,
}

#[derive(Clone)]
struct Ext4RawInode {
    mode: u16,
    flags: u32,
    size: u64,
    file_acl: u64,
    block: [u8; 60],
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct Ext4DirEntry {
    name: String,
    kind: InodeKind,
    raw: Ext4RawInode,
}

#[derive(Clone)]
struct Ext4Extent {
    logical_block: u32,
    physical_block: u64,
    block_count: u32,
}

struct Ext4Inode {
    state: Arc<Ext4State>,
    kind: InodeKind,
    raw: Ext4RawInode,
}

impl Ext4 {
    pub fn mount_readonly(device: Arc<dyn BlockDevice>) -> Result<Self, Ext4Error> {
        let geometry = device.geometry();
        if geometry.logical_block_size as usize != SECTOR_SIZE {
            return Err(Ext4Error::Invalid);
        }

        let superblock =
            read_device_bytes_from(&device, EXT4_SUPERBLOCK_OFFSET, EXT4_SUPERBLOCK_SIZE)?;
        if read_u16_le(&superblock[56..58]) != EXT4_SUPER_MAGIC {
            return Err(Ext4Error::Invalid);
        }

        let inode_count = read_u32_le(&superblock[0..4]);
        let block_count_lo = u64::from(read_u32_le(&superblock[4..8]));
        let first_data_block = read_u32_le(&superblock[20..24]);
        let log_block_size = read_u32_le(&superblock[24..28]);
        if log_block_size > 6 {
            return Err(Ext4Error::Unsupported);
        }
        let block_size = 1024usize
            .checked_shl(log_block_size)
            .ok_or(Ext4Error::Invalid)?;
        if block_size < 1024 || block_size % SECTOR_SIZE != 0 {
            return Err(Ext4Error::Invalid);
        }

        let blocks_per_group = read_u32_le(&superblock[32..36]);
        let inodes_per_group = read_u32_le(&superblock[40..44]);
        let inode_size = usize::from(read_u16_le(&superblock[88..90]));
        let feature_incompat = read_u32_le(&superblock[96..100]);
        if feature_incompat & EXT4_FEATURE_INCOMPAT_ENCRYPT != 0 {
            return Err(Ext4Error::Unsupported);
        }
        if feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS == 0 {
            return Err(Ext4Error::Unsupported);
        }
        if blocks_per_group == 0
            || inodes_per_group == 0
            || inode_size < 128
            || inode_size > block_size
        {
            return Err(Ext4Error::Invalid);
        }

        let block_count_hi = if feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0 {
            u64::from(read_u32_le(&superblock[336..340]))
        } else {
            0
        };
        let block_count = block_count_lo | (block_count_hi << 32);
        if block_count == 0 {
            return Err(Ext4Error::Invalid);
        }
        let image_bytes = geometry
            .block_count
            .checked_mul(SECTOR_SIZE as u64)
            .ok_or(Ext4Error::Invalid)?;
        if block_count
            .checked_mul(block_size as u64)
            .ok_or(Ext4Error::Invalid)?
            > image_bytes
        {
            return Err(Ext4Error::Invalid);
        }

        let desc_size = if feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0 {
            let size = usize::from(read_u16_le(&superblock[254..256]));
            if size == 0 { 64 } else { size }
        } else {
            32
        };
        if desc_size < 32 || desc_size > block_size {
            return Err(Ext4Error::Invalid);
        }

        let group_count_u64 = div_round_up(
            block_count.saturating_sub(u64::from(first_data_block)),
            u64::from(blocks_per_group),
        );
        let group_count = u32::try_from(group_count_u64).map_err(|_| Ext4Error::Unsupported)?;

        let state = Arc::new(Ext4State {
            device,
            block_size,
            block_count,
            inode_count,
            first_data_block,
            inodes_per_group,
            inode_size,
            group_count,
            desc_size,
            has_dir_file_type: feature_incompat & EXT4_FEATURE_INCOMPAT_FILETYPE != 0,
        });

        let root = state.read_inode(EXT4_ROOT_INO)?;
        if inode_kind(root.mode) != Some(InodeKind::Directory) {
            return Err(Ext4Error::Invalid);
        }

        Ok(Self { state })
    }
}

impl MountedFs for Ext4 {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        let raw = self.state.read_inode(EXT4_ROOT_INO).map_err(ext4_to_vfs)?;
        Ok(Arc::new(Ext4Inode {
            state: self.state.clone(),
            kind: InodeKind::Directory,
            raw,
        }))
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        if relative.is_empty() {
            return self.root();
        }
        let (kind, raw) = self.state.lookup_raw(relative).map_err(ext4_to_vfs)?;
        Ok(Arc::new(Ext4Inode {
            state: self.state.clone(),
            kind,
            raw,
        }))
    }
}

impl Inode for Ext4Inode {
    fn kind(&self) -> InodeKind {
        self.kind
    }

    fn available_rights(&self) -> FileRights {
        match self.kind {
            InodeKind::Directory => {
                FileRights::from_bits(FileRights::LOOKUP.bits() | FileRights::READDIR.bits())
            }
            InodeKind::Regular => {
                FileRights::from_bits(FileRights::READ.bits() | FileRights::EXECUTE.bits())
            }
            InodeKind::Symlink | InodeKind::Model => FileRights::empty(),
        }
    }

    fn readdir_at(&self, cursor: u64) -> Result<Option<VfsDirEntry>, VfsError> {
        if self.kind != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        let idx = usize::try_from(cursor).map_err(|_| VfsError::Invalid)?;
        let entries = self.state.read_directory(&self.raw).map_err(ext4_to_vfs)?;
        Ok(entries.get(idx).map(|entry| VfsDirEntry {
            name: entry.name.clone(),
            kind: entry.kind,
            next_cursor: (idx + 1) as u64,
        }))
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        self.state
            .read_file_range(&self.raw, offset, out)
            .map_err(ext4_to_vfs)
    }

    fn write_at(&self, _offset: usize, _input: &[u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        Err(VfsError::InsufficientRights)
    }
}

impl Ext4State {
    fn lookup_raw(&self, relative: &str) -> Result<(InodeKind, Ext4RawInode), Ext4Error> {
        let mut current = self.read_inode(EXT4_ROOT_INO)?;
        let mut components = relative.split('/').peekable();
        while let Some(component) = components.next() {
            if component.is_empty() || component == "." || component == ".." {
                return Err(Ext4Error::Invalid);
            }
            if inode_kind(current.mode) != Some(InodeKind::Directory) {
                return Err(Ext4Error::Invalid);
            }
            let entry = self
                .read_directory(&current)?
                .into_iter()
                .find(|entry| entry.name == component)
                .ok_or(Ext4Error::NotFound)?;
            current = entry.raw.clone();
            if components.peek().is_none() {
                return Ok((entry.kind, entry.raw));
            }
        }
        Err(Ext4Error::Invalid)
    }

    fn read_device_bytes(&self, offset: u64, len: usize) -> Result<Vec<u8>, Ext4Error> {
        read_device_bytes_from(&self.device, offset, len)
    }

    fn read_fs_block(&self, block: u64) -> Result<Vec<u8>, Ext4Error> {
        if block >= self.block_count {
            return Err(Ext4Error::Invalid);
        }
        self.read_device_bytes(
            block
                .checked_mul(self.block_size as u64)
                .ok_or(Ext4Error::Invalid)?,
            self.block_size,
        )
    }

    fn group_descriptor(&self, group: u32) -> Result<u64, Ext4Error> {
        if group >= self.group_count {
            return Err(Ext4Error::Invalid);
        }
        let table_offset = u64::from(self.first_data_block + 1)
            .checked_mul(self.block_size as u64)
            .ok_or(Ext4Error::Invalid)?;
        let offset = table_offset
            .checked_add(u64::from(group) * self.desc_size as u64)
            .ok_or(Ext4Error::Invalid)?;
        let desc = self.read_device_bytes(offset, self.desc_size)?;
        let lo = u64::from(read_u32_le(&desc[8..12]));
        let hi = if self.desc_size >= 64 {
            u64::from(read_u32_le(&desc[40..44]))
        } else {
            0
        };
        let inode_table = lo | (hi << 32);
        if inode_table == 0 || inode_table >= self.block_count {
            return Err(Ext4Error::Invalid);
        }
        Ok(inode_table)
    }

    fn read_inode(&self, inode_number: u32) -> Result<Ext4RawInode, Ext4Error> {
        if inode_number == 0 || inode_number > self.inode_count {
            return Err(Ext4Error::Invalid);
        }
        let zero_based = inode_number - 1;
        let group = zero_based / self.inodes_per_group;
        let index = zero_based % self.inodes_per_group;
        let inode_table = self.group_descriptor(group)?;
        let inode_offset = inode_table
            .checked_mul(self.block_size as u64)
            .and_then(|base| base.checked_add(u64::from(index) * self.inode_size as u64))
            .ok_or(Ext4Error::Invalid)?;
        let bytes = self.read_device_bytes(inode_offset, self.inode_size)?;
        if bytes.len() < 128 {
            return Err(Ext4Error::Invalid);
        }

        let mode = read_u16_le(&bytes[0..2]);
        let size_lo = u64::from(read_u32_le(&bytes[4..8]));
        let flags = read_u32_le(&bytes[32..36]);
        let file_acl = u64::from(read_u32_le(&bytes[104..108]));
        let size_hi = if inode_kind(mode) == Some(InodeKind::Regular) {
            u64::from(read_u32_le(&bytes[108..112]))
        } else {
            0
        };
        let mut block = [0u8; 60];
        block.copy_from_slice(&bytes[40..100]);

        Ok(Ext4RawInode {
            mode,
            flags,
            size: size_lo | (size_hi << 32),
            file_acl,
            block,
            bytes,
        })
    }

    fn read_directory(&self, raw: &Ext4RawInode) -> Result<Vec<Ext4DirEntry>, Ext4Error> {
        if inode_kind(raw.mode) != Some(InodeKind::Directory) {
            return Err(Ext4Error::Invalid);
        }
        let bytes = self.read_file(raw)?;
        let mut entries = Vec::new();

        for block in bytes.chunks(self.block_size) {
            let mut offset = 0usize;
            while offset + 8 <= block.len() {
                let inode_number = read_u32_le(&block[offset..offset + 4]);
                let rec_len = usize::from(read_u16_le(&block[offset + 4..offset + 6]));
                if rec_len == 0 {
                    return Err(Ext4Error::Invalid);
                }
                if rec_len < 8 || offset + rec_len > block.len() {
                    return Err(Ext4Error::Invalid);
                }

                let name_len = if self.has_dir_file_type {
                    usize::from(block[offset + 6])
                } else {
                    usize::from(read_u16_le(&block[offset + 6..offset + 8]))
                };
                if name_len > rec_len.saturating_sub(8) {
                    return Err(Ext4Error::Invalid);
                }
                let file_type = if self.has_dir_file_type {
                    block[offset + 7]
                } else {
                    EXT4_FT_UNKNOWN
                };

                if inode_number != 0
                    && let Some(name) = parse_dir_name(&block[offset + 8..offset + 8 + name_len])
                    && name != "."
                    && name != ".."
                {
                    let entry_raw = self.read_inode(inode_number)?;
                    self.read_xattrs_ignored(&entry_raw)?;
                    if let Some(kind) = self.import_kind(file_type, entry_raw.mode) {
                        entries.push(Ext4DirEntry {
                            name,
                            kind,
                            raw: entry_raw,
                        });
                    }
                }

                offset += rec_len;
            }
        }

        Ok(entries)
    }

    fn import_kind(&self, file_type: u8, mode: u16) -> Option<InodeKind> {
        match file_type {
            EXT4_FT_REG_FILE => Some(InodeKind::Regular),
            EXT4_FT_DIR => Some(InodeKind::Directory),
            EXT4_FT_SYMLINK => None,
            EXT4_FT_UNKNOWN => inode_kind(mode),
            _ => None,
        }
    }

    fn read_file(&self, raw: &Ext4RawInode) -> Result<Vec<u8>, Ext4Error> {
        if raw.size == 0 {
            return Ok(Vec::new());
        }
        let wanted = usize::try_from(raw.size).map_err(|_| Ext4Error::Unsupported)?;
        let mut out = vec![0u8; wanted];
        let n = self.read_file_range(raw, 0, &mut out)?;
        if n != wanted {
            return Err(Ext4Error::Invalid);
        }
        Ok(out)
    }

    fn read_file_range(
        &self,
        raw: &Ext4RawInode,
        offset: usize,
        out: &mut [u8],
    ) -> Result<usize, Ext4Error> {
        if raw.size == 0 || out.is_empty() {
            return Ok(0);
        }
        if raw.flags & EXT4_INLINE_DATA_FL != 0 {
            return Err(Ext4Error::Unsupported);
        }
        let file_size = usize::try_from(raw.size).map_err(|_| Ext4Error::Unsupported)?;
        if offset >= file_size {
            return Ok(0);
        }
        let wanted = cmp::min(out.len(), file_size - offset);
        out[..wanted].fill(0);

        let extents = self.collect_extents(raw)?;
        if extents.is_empty() {
            return Err(Ext4Error::Invalid);
        }
        let request_start = offset;
        let request_end = request_start
            .checked_add(wanted)
            .ok_or(Ext4Error::Invalid)?;

        for extent in extents {
            let extent_start = usize::try_from(extent.logical_block)
                .map_err(|_| Ext4Error::Unsupported)?
                .checked_mul(self.block_size)
                .ok_or(Ext4Error::Invalid)?;
            let extent_bytes = usize::try_from(extent.block_count)
                .map_err(|_| Ext4Error::Unsupported)?
                .checked_mul(self.block_size)
                .ok_or(Ext4Error::Invalid)?;
            let extent_end = extent_start
                .checked_add(extent_bytes)
                .ok_or(Ext4Error::Invalid)?;
            let overlap_start = cmp::max(request_start, extent_start);
            let overlap_end = cmp::min(request_end, extent_end);
            if overlap_start >= overlap_end {
                continue;
            }

            let mut pos = overlap_start;
            while pos < overlap_end {
                let extent_rel = pos - extent_start;
                let block_offset = extent_rel / self.block_size;
                let in_block = extent_rel % self.block_size;
                let block = self.read_fs_block(
                    extent
                        .physical_block
                        .checked_add(block_offset as u64)
                        .ok_or(Ext4Error::Invalid)?,
                )?;
                let n = cmp::min(self.block_size - in_block, overlap_end - pos);
                let out_pos = pos - request_start;
                out[out_pos..out_pos + n].copy_from_slice(&block[in_block..in_block + n]);
                pos += n;
            }
        }

        Ok(wanted)
    }

    fn collect_extents(&self, raw: &Ext4RawInode) -> Result<Vec<Ext4Extent>, Ext4Error> {
        if raw.flags & EXT4_EXTENTS_FL == 0 {
            return Err(Ext4Error::Unsupported);
        }
        let mut extents = Vec::new();
        self.read_extent_node(&raw.block, 16, &mut extents)?;
        Ok(extents)
    }

    fn read_extent_node(
        &self,
        node: &[u8],
        remaining_depth_limit: u16,
        extents: &mut Vec<Ext4Extent>,
    ) -> Result<(), Ext4Error> {
        if remaining_depth_limit == 0 || node.len() < 12 {
            return Err(Ext4Error::Unsupported);
        }
        if read_u16_le(&node[0..2]) != EXT4_EXTENT_MAGIC {
            return Err(Ext4Error::Invalid);
        }
        let entries = usize::from(read_u16_le(&node[2..4]));
        let max_entries = usize::from(read_u16_le(&node[4..6]));
        let depth = read_u16_le(&node[6..8]);
        if entries > max_entries || 12 + entries * 12 > node.len() {
            return Err(Ext4Error::Invalid);
        }

        if depth == 0 {
            for idx in 0..entries {
                let entry = 12 + idx * 12;
                let logical_block = read_u32_le(&node[entry..entry + 4]);
                let len_raw = read_u16_le(&node[entry + 4..entry + 6]);
                if len_raw & 0x8000 != 0 {
                    return Err(Ext4Error::Unsupported);
                }
                let block_count = u32::from(len_raw);
                let physical_hi = u64::from(read_u16_le(&node[entry + 6..entry + 8]));
                let physical_lo = u64::from(read_u32_le(&node[entry + 8..entry + 12]));
                let physical_block = physical_lo | (physical_hi << 32);
                if block_count == 0 || physical_block >= self.block_count {
                    return Err(Ext4Error::Invalid);
                }
                extents.push(Ext4Extent {
                    logical_block,
                    physical_block,
                    block_count,
                });
            }
            return Ok(());
        }

        for idx in 0..entries {
            let entry = 12 + idx * 12;
            let leaf_lo = u64::from(read_u32_le(&node[entry + 4..entry + 8]));
            let leaf_hi = u64::from(read_u16_le(&node[entry + 8..entry + 10]));
            let leaf_block = leaf_lo | (leaf_hi << 32);
            let child = self.read_fs_block(leaf_block)?;
            self.read_extent_node(&child, remaining_depth_limit - 1, extents)?;
        }

        Ok(())
    }

    fn read_xattrs_ignored(&self, raw: &Ext4RawInode) -> Result<(), Ext4Error> {
        if raw.file_acl != 0 {
            let _ = self.read_fs_block(raw.file_acl)?;
        }
        if raw.bytes.len() >= 132 {
            let extra_isize = usize::from(read_u16_le(&raw.bytes[128..130]));
            let inline_offset = 128usize.saturating_add(extra_isize);
            if inline_offset + 4 <= raw.bytes.len() {
                let _ = read_u32_le(&raw.bytes[inline_offset..inline_offset + 4]);
            }
        }
        Ok(())
    }
}

#[cfg(feature = "neo-test-phase3-ext4")]
pub fn debug_extent_count_for_device(
    device: Arc<dyn BlockDevice>,
    relative: &str,
) -> Result<usize, VfsError> {
    let fs = Ext4::mount_readonly(device).map_err(ext4_to_vfs)?;
    let (kind, raw) = fs.state.lookup_raw(relative).map_err(ext4_to_vfs)?;
    if kind != InodeKind::Regular {
        return Err(VfsError::TypeForbidden);
    }
    fs.state
        .collect_extents(&raw)
        .map(|extents| extents.len())
        .map_err(ext4_to_vfs)
}

fn read_device_bytes_from(
    device: &Arc<dyn BlockDevice>,
    offset: u64,
    len: usize,
) -> Result<Vec<u8>, Ext4Error> {
    let mut out = vec![0u8; len];
    let mut copied = 0usize;
    while copied < len {
        let absolute = offset
            .checked_add(copied as u64)
            .ok_or(Ext4Error::Invalid)?;
        let sector = absolute / SECTOR_SIZE as u64;
        let sector_offset =
            usize::try_from(absolute % SECTOR_SIZE as u64).map_err(|_| Ext4Error::Invalid)?;
        let sector_bytes = read_sector_from(device, sector)?;
        let n = cmp::min(SECTOR_SIZE - sector_offset, len - copied);
        out[copied..copied + n].copy_from_slice(&sector_bytes[sector_offset..sector_offset + n]);
        copied += n;
    }
    Ok(out)
}

fn read_sector_from(
    device: &Arc<dyn BlockDevice>,
    sector: u64,
) -> Result<[u8; SECTOR_SIZE], Ext4Error> {
    let dma = DmaBuffer::alloc_zeroed(SECTOR_SIZE).map_err(|_| Ext4Error::BlockDevice)?;
    let buffers = [dma.clone()];
    device
        .read_block(sector, &buffers)
        .map_err(|_| Ext4Error::BlockDevice)?;
    let mut out = [0u8; SECTOR_SIZE];
    // SAFETY: the synchronous block read has completed and this DMA buffer is
    // not shared with an active device request when copied.
    unsafe {
        out.copy_from_slice(&dma.as_slice()[..SECTOR_SIZE]);
    }
    Ok(out)
}

fn parse_dir_name(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    core::str::from_utf8(bytes)
        .ok()
        .map(|name| name.to_string())
}

fn inode_kind(mode: u16) -> Option<InodeKind> {
    match mode & S_IFMT {
        S_IFREG => Some(InodeKind::Regular),
        S_IFDIR => Some(InodeKind::Directory),
        S_IFLNK => None,
        _ => None,
    }
}

fn ext4_to_vfs(err: Ext4Error) -> VfsError {
    match err {
        Ext4Error::NotFound => VfsError::NotFound,
        Ext4Error::Invalid | Ext4Error::Unsupported => VfsError::Invalid,
        Ext4Error::BlockDevice => VfsError::NotFound,
    }
}

fn div_round_up(value: u64, divisor: u64) -> u64 {
    if value == 0 {
        0
    } else {
        1 + (value - 1) / divisor
    }
}

fn read_u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
