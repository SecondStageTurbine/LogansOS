// SPDX-License-Identifier: MPL-2.0
//! L3a-2: the pure part of the FAT mount (`fs::fat`), host-tested through
//! `#[path]` (`tools/fat-logic-tests`): the boot sector's geometry checks
//! (trap T13) and the two dirty indicators, read apart (trap T4).
//!
//! No `alloc`, no `crate::`: only byte slices in, verdicts out.

/// Why a boot sector was refused before the library saw it (T13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BpbError {
    /// Shorter than one 512-byte sector, or no `55 AA` at 510.
    NotABootSector,
    /// exFAT (`EXFAT   ` at 3): not FAT (T12).
    ExFat,
    /// Bytes per sector not a power of two in 512..=4096.
    BytesPerSector,
    /// Sectors per cluster 0 or not a power of two.
    SectorsPerCluster,
    /// No reserved sectors (the boot sector itself is one).
    ReservedSectors,
    /// FAT count not 1 or 2.
    FatCount,
    /// Root entries: 0 on FAT12/16, non-zero on FAT32, or not a whole number
    /// of sectors.
    RootEntries,
    /// Total sectors: both the 16- and 32-bit fields 0, or both set and
    /// different.
    TotalSectors,
    /// Sectors per FAT 0 (both the 16- and 32-bit fields).
    SectorsPerFat,
    /// The reserved area, the FATs and the root directory reach the end of
    /// the volume: no data region.
    NoDataRegion,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// What [`check_bpb`] read from a sane boot sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bpb {
    pub bytes_per_sector: u32,
    pub reserved_sectors: u32,
    /// The FAT32 layout (16-bit sectors-per-FAT field 0, no fixed root).
    pub fat32: bool,
}

/// T13: refuse a boot sector whose layout arithmetic is meaningless, before
/// the library divides by it (MS FAT spec, "Boot Sector and BPB"; the checks
/// Linux's `fat_fill_super` makes, read for principle). The FAT type itself
/// (by cluster count, T1) is the library's to decide; this only guarantees
/// the numbers it decides from are sane.
pub fn check_bpb(sector: &[u8]) -> Result<Bpb, BpbError> {
    if sector.len() < 512 || sector[510] != 0x55 || sector[511] != 0xaa {
        return Err(BpbError::NotABootSector);
    }
    if &sector[3..11] == b"EXFAT   " {
        return Err(BpbError::ExFat);
    }
    let bytes_per_sector = u32::from(u16_at(sector, 11));
    if !bytes_per_sector.is_power_of_two() || !(512..=4096).contains(&bytes_per_sector) {
        return Err(BpbError::BytesPerSector);
    }
    let sectors_per_cluster = u32::from(sector[13]);
    if !sectors_per_cluster.is_power_of_two() {
        return Err(BpbError::SectorsPerCluster);
    }
    let reserved_sectors = u32::from(u16_at(sector, 14));
    if reserved_sectors == 0 {
        return Err(BpbError::ReservedSectors);
    }
    let fats = u32::from(sector[16]);
    if !(1..=2).contains(&fats) {
        return Err(BpbError::FatCount);
    }
    let root_entries = u32::from(u16_at(sector, 17));
    let total_16 = u32::from(u16_at(sector, 19));
    let sectors_per_fat_16 = u32::from(u16_at(sector, 22));
    let total_32 = u32_at(sector, 32);
    let fat32 = sectors_per_fat_16 == 0;
    let sectors_per_fat = if fat32 {
        u32_at(sector, 36)
    } else {
        sectors_per_fat_16
    };
    if sectors_per_fat == 0 {
        return Err(BpbError::SectorsPerFat);
    }
    if fat32 != (root_entries == 0) || (root_entries * 32) % bytes_per_sector != 0 {
        return Err(BpbError::RootEntries);
    }
    let total = match (total_16, total_32) {
        (0, 0) => return Err(BpbError::TotalSectors),
        (0, t) | (t, 0) => t,
        (a, b) if a == b => a,
        _ => return Err(BpbError::TotalSectors),
    };
    let root_sectors = root_entries * 32 / bytes_per_sector;
    let meta = u64::from(reserved_sectors)
        + u64::from(fats) * u64::from(sectors_per_fat)
        + u64::from(root_sectors);
    if meta >= u64::from(total) {
        return Err(BpbError::NoDataRegion);
    }
    Ok(Bpb {
        bytes_per_sector,
        reserved_sectors,
        fat32,
    })
}

/// T4: the two dirty indicators, read apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Dirty {
    /// The BPB's status byte (`0x25` on FAT12/16, `0x41` on FAT32), bit 0.
    pub bpb: bool,
    /// FAT[1]'s clean-shutdown bit clear, or its hard-error bit clear (FAT16
    /// bits 15/14, FAT32 bits 27/26). FAT12 has none (always false).
    pub fat1: bool,
}

/// T4: read both dirty indicators from the boot sector and the first sector
/// of the first FAT. `fat16` says which FAT[1] layout applies when the
/// boot sector is the FAT12/16 shape (the library decides FAT12 vs FAT16 by
/// cluster count; FAT12 has no flags, so pass `false` for it).
pub fn dirty_flags(boot: &Bpb, sector0: &[u8], fat_sector: &[u8], fat16: bool) -> Dirty {
    let bpb = if boot.fat32 {
        sector0[0x41] & 1 != 0
    } else {
        sector0[0x25] & 1 != 0
    };
    let fat1 = if boot.fat32 {
        let entry = u32_at(fat_sector, 4);
        entry & (1 << 27) == 0 || entry & (1 << 26) == 0
    } else if fat16 {
        let entry = u16_at(fat_sector, 2);
        entry & (1 << 15) == 0 || entry & (1 << 14) == 0
    } else {
        false
    };
    Dirty { bpb, fat1 }
}
