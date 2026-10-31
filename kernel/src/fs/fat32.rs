// SPDX-License-Identifier: MPL-2.0
//! Read-only FAT32 foreign filesystem support for Phase 3 BUILD-4C.
//!
//! The driver presents FAT32 through the existing `MountedFs`/`Inode`
//! boundary. It intentionally imports only `Directory` and `Regular` kinds;
//! foreign filesystems do not have NeoFS typed-inode metadata.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp;

use crate::drivers::block::{BlockDevice, DmaBuffer};
use crate::fs::{FileRights, Inode, InodeKind, MountedFs, VfsDirEntry, VfsError};

const SECTOR_SIZE: usize = 512;
const FAT_ENTRY_MASK: u32 = 0x0fff_ffff;
const FAT_EOC_MIN: u32 = 0x0fff_fff8;
const FAT_BAD_CLUSTER: u32 = 0x0fff_fff7;

const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_LFN: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

const NT_BASE_LOWERCASE: u8 = 0x08;
const NT_EXT_LOWERCASE: u8 = 0x10;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Fat32Error {
    BlockDevice,
    Invalid,
}

pub struct Fat32 {
    state: Arc<Fat32State>,
}

struct Fat32State {
    device: Arc<dyn BlockDevice>,
    sectors_per_cluster: u8,
    first_data_sector: u32,
    total_sectors: u32,
    root_cluster: u32,
    data_clusters: u32,
    fat: Vec<u32>,
}

#[derive(Clone)]
struct Fat32DirEntry {
    name: String,
    kind: InodeKind,
    first_cluster: u32,
    size: u64,
}

struct Fat32Inode {
    state: Arc<Fat32State>,
    kind: InodeKind,
    first_cluster: u32,
    size: u64,
}

#[derive(Clone)]
struct LfnSlot {
    order: u8,
    is_last: bool,
    checksum: u8,
    chars: [u16; 13],
}

impl Fat32 {
    pub fn mount_readonly(device: Arc<dyn BlockDevice>) -> Result<Self, Fat32Error> {
        let geometry = device.geometry();
        if geometry.logical_block_size as usize != SECTOR_SIZE {
            return Err(Fat32Error::Invalid);
        }

        let boot = read_sector_from(&device, 0)?;
        if boot[510] != 0x55 || boot[511] != 0xaa {
            return Err(Fat32Error::Invalid);
        }

        let bytes_per_sector = read_u16_le(&boot[11..13]);
        if bytes_per_sector as usize != SECTOR_SIZE {
            return Err(Fat32Error::Invalid);
        }

        let sectors_per_cluster = boot[13];
        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err(Fat32Error::Invalid);
        }

        let reserved_sectors = read_u16_le(&boot[14..16]) as u32;
        let fat_count = boot[16];
        let root_entry_count = read_u16_le(&boot[17..19]);
        let total_sectors_16 = read_u16_le(&boot[19..21]) as u32;
        let sectors_per_fat_16 = read_u16_le(&boot[22..24]) as u32;
        let total_sectors_32 = read_u32_le(&boot[32..36]);
        let sectors_per_fat = read_u32_le(&boot[36..40]);
        let root_cluster = read_u32_le(&boot[44..48]);

        if reserved_sectors == 0
            || fat_count == 0
            || root_entry_count != 0
            || total_sectors_16 != 0
            || sectors_per_fat_16 != 0
            || total_sectors_32 == 0
            || sectors_per_fat == 0
            || root_cluster < 2
        {
            return Err(Fat32Error::Invalid);
        }

        if u64::from(total_sectors_32) > geometry.block_count {
            return Err(Fat32Error::Invalid);
        }

        let fat_sectors_total = u32::from(fat_count)
            .checked_mul(sectors_per_fat)
            .ok_or(Fat32Error::Invalid)?;
        let first_data_sector = reserved_sectors
            .checked_add(fat_sectors_total)
            .ok_or(Fat32Error::Invalid)?;
        if first_data_sector >= total_sectors_32 {
            return Err(Fat32Error::Invalid);
        }
        let data_sectors = total_sectors_32 - first_data_sector;
        let data_clusters = data_sectors / u32::from(sectors_per_cluster);
        if data_clusters == 0 {
            return Err(Fat32Error::Invalid);
        }

        let fat_entries = sectors_per_fat
            .checked_mul((SECTOR_SIZE / 4) as u32)
            .ok_or(Fat32Error::Invalid)?;
        if root_cluster >= fat_entries {
            return Err(Fat32Error::Invalid);
        }

        let mut fat = Vec::with_capacity(fat_entries as usize);
        for sector_offset in 0..sectors_per_fat {
            let sector = read_sector_from(&device, u64::from(reserved_sectors + sector_offset))?;
            for entry in sector.chunks_exact(4) {
                fat.push(read_u32_le(entry) & FAT_ENTRY_MASK);
            }
        }

        Ok(Self {
            state: Arc::new(Fat32State {
                device,
                sectors_per_cluster,
                first_data_sector,
                total_sectors: total_sectors_32,
                root_cluster,
                data_clusters,
                fat,
            }),
        })
    }
}

impl MountedFs for Fat32 {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        Ok(Arc::new(Fat32Inode::directory(
            self.state.clone(),
            String::new(),
            self.state.root_cluster,
        )))
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        if relative.is_empty() {
            return self.root();
        }

        let mut current_cluster = self.state.root_cluster;
        let mut components = relative.split('/').peekable();
        while let Some(component) = components.next() {
            if component.is_empty() || component == "." || component == ".." {
                return Err(VfsError::Invalid);
            }
            let entry = self
                .state
                .find_entry(current_cluster, component)?
                .ok_or(VfsError::NotFound)?;
            if components.peek().is_none() {
                return Ok(Arc::new(Fat32Inode {
                    state: self.state.clone(),
                    kind: entry.kind,
                    first_cluster: entry.first_cluster,
                    size: entry.size,
                }));
            }
            if entry.kind != InodeKind::Directory {
                return Err(VfsError::NotDirectory);
            }
            current_cluster = entry.first_cluster;
        }

        self.root()
    }
}

impl Fat32Inode {
    fn directory(state: Arc<Fat32State>, _name: String, first_cluster: u32) -> Self {
        Self {
            state,
            kind: InodeKind::Directory,
            first_cluster,
            size: 0,
        }
    }
}

impl Inode for Fat32Inode {
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
        let entries = self.state.read_directory(self.first_cluster)?;
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
        let data = self
            .state
            .read_file(self.first_cluster, self.size)
            .map_err(|_| VfsError::Invalid)?;
        if offset >= data.len() {
            return Ok(0);
        }
        let n = cmp::min(out.len(), data.len() - offset);
        out[..n].copy_from_slice(&data[offset..offset + n]);
        Ok(n)
    }

    fn write_at(&self, _offset: usize, _input: &[u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        Err(VfsError::InsufficientRights)
    }
}

impl Fat32State {
    fn read_sector(&self, sector: u64) -> Result<[u8; SECTOR_SIZE], Fat32Error> {
        if sector >= u64::from(self.total_sectors) {
            return Err(Fat32Error::Invalid);
        }
        read_sector_from(&self.device, sector)
    }

    fn first_sector_for_cluster(&self, cluster: u32) -> Result<u32, Fat32Error> {
        if cluster < 2 || cluster >= self.max_cluster_number() {
            return Err(Fat32Error::Invalid);
        }
        let data_index = cluster - 2;
        let sector_delta = data_index
            .checked_mul(u32::from(self.sectors_per_cluster))
            .ok_or(Fat32Error::Invalid)?;
        self.first_data_sector
            .checked_add(sector_delta)
            .ok_or(Fat32Error::Invalid)
    }

    fn max_cluster_number(&self) -> u32 {
        self.data_clusters.saturating_add(2)
    }

    fn next_cluster(&self, cluster: u32) -> Result<Option<u32>, Fat32Error> {
        let idx = usize::try_from(cluster).map_err(|_| Fat32Error::Invalid)?;
        let next = *self.fat.get(idx).ok_or(Fat32Error::Invalid)? & FAT_ENTRY_MASK;
        if next >= FAT_EOC_MIN {
            return Ok(None);
        }
        if next == FAT_BAD_CLUSTER || next < 2 || next >= self.max_cluster_number() {
            return Err(Fat32Error::Invalid);
        }
        Ok(Some(next))
    }

    fn read_cluster_chain(&self, first_cluster: u32) -> Result<Vec<u8>, Fat32Error> {
        if first_cluster < 2 {
            return Err(Fat32Error::Invalid);
        }
        let mut cluster = first_cluster;
        let mut clusters_seen = 0u32;
        let mut out = Vec::new();

        loop {
            clusters_seen = clusters_seen.checked_add(1).ok_or(Fat32Error::Invalid)?;
            if clusters_seen > self.data_clusters {
                return Err(Fat32Error::Invalid);
            }

            let first_sector = self.first_sector_for_cluster(cluster)?;
            for sector_offset in 0..u32::from(self.sectors_per_cluster) {
                let sector = self.read_sector(u64::from(first_sector + sector_offset))?;
                out.extend_from_slice(&sector);
            }

            match self.next_cluster(cluster)? {
                Some(next) => cluster = next,
                None => return Ok(out),
            }
        }
    }

    fn read_file(&self, first_cluster: u32, size: u64) -> Result<Vec<u8>, Fat32Error> {
        if size == 0 {
            return Ok(Vec::new());
        }
        let wanted = usize::try_from(size).map_err(|_| Fat32Error::Invalid)?;
        let mut data = self.read_cluster_chain(first_cluster)?;
        if data.len() < wanted {
            return Err(Fat32Error::Invalid);
        }
        data.truncate(wanted);
        Ok(data)
    }

    fn read_directory(&self, first_cluster: u32) -> Result<Vec<Fat32DirEntry>, VfsError> {
        let bytes = self
            .read_cluster_chain(first_cluster)
            .map_err(|_| VfsError::Invalid)?;
        let mut entries = Vec::new();
        let mut lfn_slots: Vec<LfnSlot> = Vec::new();

        for raw in bytes.chunks_exact(32) {
            match raw[0] {
                0x00 => break,
                0xe5 => {
                    lfn_slots.clear();
                    continue;
                }
                _ => {}
            }

            let attr = raw[11];
            if attr == ATTR_LFN {
                if let Some(slot) = parse_lfn_slot(raw) {
                    lfn_slots.push(slot);
                } else {
                    lfn_slots.clear();
                }
                continue;
            }

            if attr & ATTR_VOLUME_ID != 0 {
                lfn_slots.clear();
                continue;
            }

            let checksum = lfn_checksum(&raw[0..11]);
            let short_name = parse_short_name(&raw[0..11], raw[12]);
            let name = lfn_name(&lfn_slots, checksum).unwrap_or(short_name);
            lfn_slots.clear();

            if name == "." || name == ".." || name.is_empty() {
                continue;
            }

            let high = u32::from(read_u16_le(&raw[20..22]));
            let low = u32::from(read_u16_le(&raw[26..28]));
            let first_cluster = (high << 16) | low;
            let kind = if attr & ATTR_DIRECTORY != 0 {
                InodeKind::Directory
            } else {
                InodeKind::Regular
            };
            let size = u64::from(read_u32_le(&raw[28..32]));

            if kind == InodeKind::Directory && first_cluster < 2 {
                continue;
            }
            entries.push(Fat32DirEntry {
                name,
                kind,
                first_cluster,
                size,
            });
        }

        Ok(entries)
    }

    fn find_entry(
        &self,
        directory_cluster: u32,
        wanted: &str,
    ) -> Result<Option<Fat32DirEntry>, VfsError> {
        for entry in self.read_directory(directory_cluster)? {
            if entry.name.eq_ignore_ascii_case(wanted) {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }
}

fn read_sector_from(
    device: &Arc<dyn BlockDevice>,
    sector: u64,
) -> Result<[u8; SECTOR_SIZE], Fat32Error> {
    let dma = DmaBuffer::alloc_zeroed(SECTOR_SIZE).map_err(|_| Fat32Error::BlockDevice)?;
    let buffers = [dma.clone()];
    device
        .read_block(sector, &buffers)
        .map_err(|_| Fat32Error::BlockDevice)?;
    let mut out = [0u8; SECTOR_SIZE];
    // SAFETY: the synchronous block read has completed and this DMA buffer is
    // not shared with an active device request when copied.
    unsafe {
        out.copy_from_slice(&dma.as_slice()[..SECTOR_SIZE]);
    }
    Ok(out)
}

fn read_u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn parse_lfn_slot(raw: &[u8]) -> Option<LfnSlot> {
    let order = raw[0] & 0x1f;
    if order == 0 || raw[11] != ATTR_LFN || raw[12] != 0 || read_u16_le(&raw[26..28]) != 0 {
        return None;
    }

    let mut chars = [0u16; 13];
    let mut idx = 0usize;
    for off in [1usize, 3, 5, 7, 9] {
        chars[idx] = read_u16_le(&raw[off..off + 2]);
        idx += 1;
    }
    for off in [14usize, 16, 18, 20, 22, 24] {
        chars[idx] = read_u16_le(&raw[off..off + 2]);
        idx += 1;
    }
    for off in [28usize, 30] {
        chars[idx] = read_u16_le(&raw[off..off + 2]);
        idx += 1;
    }

    Some(LfnSlot {
        order,
        is_last: raw[0] & 0x40 != 0,
        checksum: raw[13],
        chars,
    })
}

fn lfn_name(slots: &[LfnSlot], checksum: u8) -> Option<String> {
    if slots.is_empty() {
        return None;
    }
    let count = slots.len();
    let mut ordered: Vec<Option<&LfnSlot>> = vec![None; count + 1];
    for slot in slots {
        if slot.checksum != checksum {
            return None;
        }
        let order = usize::from(slot.order);
        if order == 0 || order > count || ordered[order].is_some() {
            return None;
        }
        ordered[order] = Some(slot);
    }

    let last = ordered[count]?;
    if !last.is_last {
        return None;
    }
    for slot in ordered.iter().take(count).skip(1).flatten() {
        if slot.is_last {
            return None;
        }
    }

    let mut units = Vec::new();
    'slots: for slot in ordered.iter().take(count + 1).skip(1) {
        let slot = (*slot)?;
        for unit in slot.chars {
            match unit {
                0x0000 => break 'slots,
                0xffff => {}
                _ => units.push(unit),
            }
        }
    }
    decode_utf16_units(&units)
}

fn decode_utf16_units(units: &[u16]) -> Option<String> {
    let mut out = String::new();
    let mut idx = 0usize;
    while idx < units.len() {
        let unit = units[idx];
        let scalar = if (0xd800..=0xdbff).contains(&unit) {
            idx += 1;
            let low = *units.get(idx)?;
            if !(0xdc00..=0xdfff).contains(&low) {
                return None;
            }
            0x10000 + (((u32::from(unit) - 0xd800) << 10) | (u32::from(low) - 0xdc00))
        } else if (0xdc00..=0xdfff).contains(&unit) {
            return None;
        } else {
            u32::from(unit)
        };
        out.push(char::from_u32(scalar)?);
        idx += 1;
    }
    if out.is_empty() { None } else { Some(out) }
}

fn lfn_checksum(raw_short_name: &[u8]) -> u8 {
    raw_short_name.iter().fold(0u8, |sum, byte| {
        ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(*byte)
    })
}

fn parse_short_name(raw: &[u8], nt_case: u8) -> String {
    let mut base = String::new();
    push_short_component(&mut base, &raw[0..8], nt_case & NT_BASE_LOWERCASE != 0);

    let mut ext = String::new();
    push_short_component(&mut ext, &raw[8..11], nt_case & NT_EXT_LOWERCASE != 0);

    if ext.is_empty() {
        base
    } else {
        base.push('.');
        base.push_str(&ext);
        base
    }
}

fn push_short_component(out: &mut String, raw: &[u8], lowercase: bool) {
    let end = raw
        .iter()
        .rposition(|b| *b != b' ')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    for (idx, byte) in raw[..end].iter().copied().enumerate() {
        let byte = if idx == 0 && byte == 0x05 { 0xe5 } else { byte };
        if byte < 0x80 {
            let ascii = if lowercase {
                byte.to_ascii_lowercase()
            } else {
                byte
            };
            out.push(char::from(ascii));
        } else {
            out.push('_');
        }
    }
}
