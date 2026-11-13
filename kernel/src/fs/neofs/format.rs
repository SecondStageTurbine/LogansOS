// SPDX-License-Identifier: MPL-2.0
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::fs::FileRights;

use super::NeoFsError;

pub const FS_BLOCK_SIZE: usize = 4096;
pub const FS_BLOCK_SIZE_LOG2: u16 = 12;
pub const SUPERBLOCK_SIZE: usize = 512;
pub const INODE_SIZE: usize = 256;
pub const DIRENT_SIZE: usize = 64;
pub const DIRECT_BLOCKS: usize = 8;
pub const INDIRECT_POINTERS_PER_BLOCK: usize = FS_BLOCK_SIZE / core::mem::size_of::<u64>();
pub const SINGLE_INDIRECT_BLOCKS: usize = INDIRECT_POINTERS_PER_BLOCK;
pub const DOUBLE_INDIRECT_BLOCKS: usize = INDIRECT_POINTERS_PER_BLOCK * INDIRECT_POINTERS_PER_BLOCK;
pub const TRIPLE_INDIRECT_BLOCKS: usize =
    INDIRECT_POINTERS_PER_BLOCK * INDIRECT_POINTERS_PER_BLOCK * INDIRECT_POINTERS_PER_BLOCK;
pub const MAX_FILE_SIZE: usize = 2 * 1024 * 1024 * 1024;
pub const MAX_FILE_BLOCKS: u64 = (MAX_FILE_SIZE / FS_BLOCK_SIZE) as u64;
pub const FORMAT_VERSION: u16 = 1;
pub const TYPE_VERSION: u8 = 1;
pub const SUPERBLOCK_COPY_OFFSETS: [u64; 3] = [0, 1024 * 1024, 64 * 1024 * 1024];
/// Whether the superblock copy at `offset` is on a device of
/// `device_bytes`: the single rule every reader and writer of the copies,
/// and the F0 pin, use (`commit_logic::superblock_copy_on_device`).
pub fn superblock_copy_on_device(offset: u64, device_bytes: u64) -> bool {
    super::commit_logic::superblock_copy_on_device(offset, SUPERBLOCK_SIZE as u64, device_bytes)
}

/// #106 relay 405: every copy on the device has its whole block there
/// (`commit_logic::superblock_copies_pinnable`).
#[cfg(feature = "neofs-dir-ops")]
pub fn superblock_copies_pinnable(device_bytes: u64) -> bool {
    super::commit_logic::superblock_copies_pinnable(
        &SUPERBLOCK_COPY_OFFSETS,
        SUPERBLOCK_SIZE as u64,
        FS_BLOCK_SIZE as u64,
        device_bytes,
    )
}

/// The superblock's `generation` (u64 LE), `docs/specs/neofs-format.md`.
pub const SUPERBLOCK_GENERATION_OFFSET: usize = 32;
pub const FREE_MAP_BITS_PER_BLOCK: u64 = FS_BLOCK_SIZE as u64 * 8;

/// Single source with the block write guard, which arms writes on a device
/// only after reading this magic at one of `SUPERBLOCK_COPY_OFFSETS`.
const SUPERBLOCK_MAGIC: &[u8; 8] = crate::drivers::block::write_guard_logic::NEOFS_MAGIC;
const _: () = {
    let guard = crate::drivers::block::write_guard_logic::NEOFS_SUPERBLOCK_OFFSETS;
    assert!(
        guard[0] == SUPERBLOCK_COPY_OFFSETS[0]
            && guard[1] == SUPERBLOCK_COPY_OFFSETS[1]
            && guard[2] == SUPERBLOCK_COPY_OFFSETS[2]
    );
};
const INODE_MAGIC: &[u8; 8] = b"NEOFSINO";

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum NeoFsKind {
    Regular = 0,
    Directory = 1,
    Symlink = 2,
    Model = 3,
}

impl NeoFsKind {
    pub fn from_disk(value: u8) -> Result<Self, NeoFsError> {
        match value {
            0 => Ok(Self::Regular),
            1 => Ok(Self::Directory),
            2 => Err(NeoFsError::UnsupportedType),
            3 => Ok(Self::Model),
            _ => Err(NeoFsError::UnsupportedType),
        }
    }

    pub fn as_vfs_kind(self) -> crate::fs::InodeKind {
        match self {
            Self::Regular => crate::fs::InodeKind::Regular,
            Self::Directory => crate::fs::InodeKind::Directory,
            Self::Symlink => crate::fs::InodeKind::Symlink,
            Self::Model => crate::fs::InodeKind::Model,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Superblock {
    pub slot: usize,
    pub total_blocks: u64,
    pub next_free_block: u64,
    pub generation: u64,
    pub root_inode_block: u64,
    pub root_inode_hash: [u8; 32],
    pub next_generation: u32,
    pub free_map: Option<FreeMapRef>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct FreeMapRef {
    pub start_block: u64,
    pub block_count: u64,
    pub hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct InodeRecord {
    pub kind: NeoFsKind,
    pub generation: u32,
    pub file_size: u64,
    pub block_count: u64,
    pub direct_blocks: [u64; DIRECT_BLOCKS],
    pub single_indirect_block: u64,
    pub double_indirect_block: u64,
    pub triple_indirect_block: u64,
    pub content_hash: [u8; 32],
    pub available_rights: FileRights,
    pub directory_entry_count: u32,
    pub fs_block_count: u64,
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub child_block: u64,
    pub kind: NeoFsKind,
}

pub fn parse_superblock(
    bytes: &[u8; SUPERBLOCK_SIZE],
    slot: usize,
) -> Result<Superblock, NeoFsError> {
    if &bytes[0..8] != SUPERBLOCK_MAGIC {
        return Err(NeoFsError::InvalidSuperblock);
    }
    if hash(&bytes[..480]) != bytes[480..512] {
        return Err(NeoFsError::InvalidSuperblock);
    }
    if get_u16(bytes, 8) != FORMAT_VERSION
        || get_u16(bytes, 10) != FS_BLOCK_SIZE_LOG2
        || bytes[12] != TYPE_VERSION
        || get_u32(bytes, 80) != INODE_SIZE as u32
    {
        return Err(NeoFsError::InvalidSuperblock);
    }
    if !all_zero(&bytes[13..16]) {
        return Err(NeoFsError::InvalidSuperblock);
    }

    let total_blocks = get_u64(bytes, 16);
    let next_free_block = get_u64(bytes, 24);
    let generation = get_u64(bytes, SUPERBLOCK_GENERATION_OFFSET);
    let root_inode_block = get_u64(bytes, 40);
    if generation == 0
        || total_blocks == 0
        || root_inode_block == 0
        || root_inode_block >= total_blocks
        || next_free_block > total_blocks
    {
        return Err(NeoFsError::InvalidSuperblock);
    }
    let free_map = parse_free_map_ref(bytes, total_blocks)?;

    let mut root_inode_hash = [0u8; 32];
    root_inode_hash.copy_from_slice(&bytes[48..80]);
    Ok(Superblock {
        slot,
        total_blocks,
        next_free_block,
        generation,
        root_inode_block,
        root_inode_hash,
        next_generation: get_u32(bytes, 84),
        free_map,
    })
}

pub fn parse_inode(bytes: &[u8; INODE_SIZE], total_blocks: u64) -> Result<InodeRecord, NeoFsError> {
    if &bytes[0..8] != INODE_MAGIC {
        return Err(NeoFsError::InvalidInode);
    }
    if hash(&bytes[..224]) != bytes[224..256] {
        return Err(NeoFsError::InvalidInode);
    }
    if bytes[9] != TYPE_VERSION || get_u16(bytes, 10) != 0 {
        return Err(NeoFsError::InvalidInode);
    }
    if !all_zero(&bytes[160..224]) {
        return Err(NeoFsError::InvalidInode);
    }

    let kind = NeoFsKind::from_disk(bytes[8])?;
    let generation = get_u32(bytes, 12);
    let file_size = get_u64(bytes, 16);
    let block_count = get_u64(bytes, 24);
    let expected_blocks = file_data_blocks(file_size)?;
    if generation == 0
        || block_count != expected_blocks
        || block_count > MAX_FILE_BLOCKS
        || file_size > MAX_FILE_SIZE as u64
    {
        return Err(NeoFsError::InvalidInode);
    }

    let mut direct_blocks = [0u64; DIRECT_BLOCKS];
    for (idx, slot) in direct_blocks.iter_mut().enumerate() {
        *slot = get_u64(bytes, 32 + idx * 8);
    }
    let direct_used = core::cmp::min(block_count as usize, DIRECT_BLOCKS);
    for block in direct_blocks.iter().copied().take(direct_used) {
        if block == 0 || block >= total_blocks {
            return Err(NeoFsError::InvalidInode);
        }
    }
    if direct_blocks[direct_used..].iter().any(|block| *block != 0) {
        return Err(NeoFsError::InvalidInode);
    }

    let single_indirect_block = get_u64(bytes, 96);
    let double_indirect_block = get_u64(bytes, 136);
    let triple_indirect_block = get_u64(bytes, 144);
    validate_indirect_root_blocks(
        block_count,
        total_blocks,
        single_indirect_block,
        double_indirect_block,
        triple_indirect_block,
    )?;

    let mut content_hash = [0u8; 32];
    content_hash.copy_from_slice(&bytes[104..136]);
    Ok(InodeRecord {
        kind,
        generation,
        file_size,
        block_count,
        direct_blocks,
        single_indirect_block,
        double_indirect_block,
        triple_indirect_block,
        content_hash,
        available_rights: FileRights::from_bits(get_u32(bytes, 152)),
        directory_entry_count: get_u32(bytes, 156),
        fs_block_count: total_blocks,
    })
}

pub fn parse_directory_entries(
    inode: &InodeRecord,
    content: &[u8],
) -> Result<Vec<DirEntry>, NeoFsError> {
    if inode.kind != NeoFsKind::Directory {
        return Err(NeoFsError::InvalidDirectory);
    }
    let expected_len = inode
        .directory_entry_count
        .checked_mul(DIRENT_SIZE as u32)
        .ok_or(NeoFsError::InvalidDirectory)? as usize;
    if expected_len != inode.file_size as usize || content.len() != expected_len {
        return Err(NeoFsError::InvalidDirectory);
    }

    let mut entries = Vec::new();
    for idx in 0..inode.directory_entry_count as usize {
        let entry = &content[idx * DIRENT_SIZE..(idx + 1) * DIRENT_SIZE];
        let name_end = entry[..48]
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(NeoFsError::InvalidDirectory)?;
        if !all_zero(&entry[name_end + 1..48]) || !all_zero(&entry[57..64]) {
            return Err(NeoFsError::InvalidDirectory);
        }
        let name = core::str::from_utf8(&entry[..name_end])
            .map_err(|_| NeoFsError::InvalidDirectory)?
            .into();
        let child_block = get_u64(entry, 48);
        if child_block == 0 {
            continue;
        }
        let kind = NeoFsKind::from_disk(entry[56])?;
        entries.push(DirEntry {
            name,
            child_block,
            kind,
        });
    }
    Ok(entries)
}

pub fn hash(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

pub fn write_superblock(superblock: &Superblock) -> [u8; SUPERBLOCK_SIZE] {
    let mut out = [0u8; SUPERBLOCK_SIZE];
    out[0..8].copy_from_slice(SUPERBLOCK_MAGIC);
    put_u16(&mut out, 8, FORMAT_VERSION);
    put_u16(&mut out, 10, FS_BLOCK_SIZE_LOG2);
    out[12] = TYPE_VERSION;
    put_u64(&mut out, 16, superblock.total_blocks);
    put_u64(&mut out, 24, superblock.next_free_block);
    put_u64(&mut out, 32, superblock.generation);
    put_u64(&mut out, 40, superblock.root_inode_block);
    out[48..80].copy_from_slice(&superblock.root_inode_hash);
    put_u32(&mut out, 80, INODE_SIZE as u32);
    put_u32(&mut out, 84, superblock.next_generation);
    if let Some(free_map) = superblock.free_map {
        put_u64(&mut out, 88, free_map.start_block);
        put_u64(&mut out, 96, free_map.block_count);
        out[104..136].copy_from_slice(&free_map.hash);
    }
    let checksum = hash(&out[..480]);
    out[480..512].copy_from_slice(&checksum);
    out
}

pub fn write_inode(inode: &InodeRecord) -> [u8; INODE_SIZE] {
    let mut out = [0u8; INODE_SIZE];
    out[0..8].copy_from_slice(INODE_MAGIC);
    out[8] = inode.kind as u8;
    out[9] = TYPE_VERSION;
    put_u32(&mut out, 12, inode.generation);
    put_u64(&mut out, 16, inode.file_size);
    put_u64(&mut out, 24, inode.block_count);
    for (idx, block) in inode.direct_blocks.iter().copied().enumerate() {
        put_u64(&mut out, 32 + idx * 8, block);
    }
    put_u64(&mut out, 96, inode.single_indirect_block);
    out[104..136].copy_from_slice(&inode.content_hash);
    put_u64(&mut out, 136, inode.double_indirect_block);
    put_u64(&mut out, 144, inode.triple_indirect_block);
    put_u32(&mut out, 152, inode.available_rights.bits());
    put_u32(&mut out, 156, inode.directory_entry_count);
    let checksum = hash(&out[..224]);
    out[224..256].copy_from_slice(&checksum);
    out
}

pub fn write_dir_entry(entry: &DirEntry) -> Result<[u8; DIRENT_SIZE], NeoFsError> {
    let mut out = [0u8; DIRENT_SIZE];
    let name = entry.name.as_bytes();
    if name.len() > 47 || name.contains(&0) || entry.child_block == 0 {
        return Err(NeoFsError::InvalidDirectory);
    }
    out[..name.len()].copy_from_slice(name);
    put_u64(&mut out, 48, entry.child_block);
    out[56] = entry.kind as u8;
    Ok(out)
}

pub fn block_offset(block: u64) -> Result<u64, NeoFsError> {
    block
        .checked_mul(FS_BLOCK_SIZE as u64)
        .ok_or(NeoFsError::InvalidInode)
}

pub fn free_map_byte_len(block_count: u64) -> Result<usize, NeoFsError> {
    block_count
        .checked_mul(FS_BLOCK_SIZE as u64)
        .and_then(|len| usize::try_from(len).ok())
        .ok_or(NeoFsError::InvalidSuperblock)
}

pub fn free_map_block_count_for_total(total_blocks: u64) -> Result<u64, NeoFsError> {
    if total_blocks == 0 {
        return Err(NeoFsError::InvalidSuperblock);
    }
    total_blocks
        .checked_add(FREE_MAP_BITS_PER_BLOCK - 1)
        .map(|blocks| blocks / FREE_MAP_BITS_PER_BLOCK)
        .filter(|blocks| *blocks != 0)
        .ok_or(NeoFsError::InvalidSuperblock)
}

fn parse_free_map_ref(
    bytes: &[u8; SUPERBLOCK_SIZE],
    total_blocks: u64,
) -> Result<Option<FreeMapRef>, NeoFsError> {
    if all_zero(&bytes[88..480]) {
        return Ok(None);
    }
    if !all_zero(&bytes[136..480]) {
        return Err(NeoFsError::InvalidSuperblock);
    }

    let start_block = get_u64(bytes, 88);
    let block_count = get_u64(bytes, 96);
    let end_block = start_block
        .checked_add(block_count)
        .ok_or(NeoFsError::InvalidSuperblock)?;
    let capacity = block_count
        .checked_mul(FREE_MAP_BITS_PER_BLOCK)
        .ok_or(NeoFsError::InvalidSuperblock)?;
    if start_block == 0 || block_count == 0 || end_block > total_blocks || capacity < total_blocks {
        return Err(NeoFsError::InvalidSuperblock);
    }

    let mut free_map_hash = [0u8; 32];
    free_map_hash.copy_from_slice(&bytes[104..136]);
    Ok(Some(FreeMapRef {
        start_block,
        block_count,
        hash: free_map_hash,
    }))
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

pub fn file_data_blocks(file_size: u64) -> Result<u64, NeoFsError> {
    if file_size == 0 {
        return Ok(0);
    }
    let blocks = file_size
        .checked_add(FS_BLOCK_SIZE as u64 - 1)
        .map(|value| value / FS_BLOCK_SIZE as u64)
        .ok_or(NeoFsError::InvalidInode)?;
    if blocks > MAX_FILE_BLOCKS {
        return Err(NeoFsError::InvalidInode);
    }
    Ok(blocks)
}

fn validate_indirect_root_blocks(
    block_count: u64,
    total_blocks: u64,
    single: u64,
    double: u64,
    triple: u64,
) -> Result<(), NeoFsError> {
    let direct = DIRECT_BLOCKS as u64;
    let single_end = direct + SINGLE_INDIRECT_BLOCKS as u64;
    let double_end = single_end + DOUBLE_INDIRECT_BLOCKS as u64;

    validate_indirect_root(single, block_count > direct, total_blocks)?;
    validate_indirect_root(double, block_count > single_end, total_blocks)?;
    validate_indirect_root(triple, block_count > double_end, total_blocks)?;
    Ok(())
}

fn validate_indirect_root(block: u64, required: bool, total_blocks: u64) -> Result<(), NeoFsError> {
    if required {
        if block == 0 || block >= total_blocks {
            return Err(NeoFsError::InvalidInode);
        }
    } else if block != 0 {
        return Err(NeoFsError::InvalidInode);
    }
    Ok(())
}

fn get_u16(buf: &[u8], offset: usize) -> u16 {
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&buf[offset..offset + 2]);
    u16::from_le_bytes(bytes)
}

fn get_u32(buf: &[u8], offset: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[offset..offset + 4]);
    u32::from_le_bytes(bytes)
}

fn get_u64(buf: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn put_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
