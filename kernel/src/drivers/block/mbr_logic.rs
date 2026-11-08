// SPDX-License-Identifier: MPL-2.0
//! L3a (G7): the MBR partition table, read-only
//! (`docs/THINK-2026-10-31-l3a-fat-readonly.md` §6a).
//!
//! A USB stick or an SD card almost always carries an MBR with one FAT
//! partition (at LBA 2048, or 63 on old tools), or no table at all (a
//! "superfloppy": the FAT boot sector is LBA 0). Both end sector 0 in
//! `0x55AA`, so [`classify`] tells them apart the way Linux's `msdos.c` does
//! in principle: a sector whose BPB is sane is a boot sector, not a table.
//!
//! Pure and allocation-free. The kernel's source parser and the host KATs in
//! `tools/block-guard-logic-tests` mount this file by `#[path]`, so keep it
//! free of `crate::` paths and feature cfgs.
//!
//! Only primary FAT entries are mounted. An extended container's chain of
//! EBRs is walked ([`extended_chain`], bounded in length and refusing any
//! link that doesn't move forward inside the container), but only so its
//! logical partitions count as claimed space: a FAT primary is believed only
//! when it starts past LBA 0, lies inside the device and overlaps no other
//! entry of any type, primary or logical (Chad's #17 review). Anything else
//! is reported, never mounted.

/// The MBR's boot signature at bytes 510..512.
pub const SIGNATURE: [u8; 2] = [0x55, 0xaa];
/// Offset of the first of the four 16-byte partition entries.
pub const TABLE_OFFSET: usize = 446;
pub const ENTRY_SIZE: usize = 16;
pub const PRIMARY_ENTRIES: usize = 4;
/// A GPT disk's protective MBR entry type.
pub const TYPE_GPT_PROTECTIVE: u8 = 0xee;

/// The partition types L3a mounts as FAT: FAT12 (`0x01`), FAT16 < 32 MiB
/// (`0x04`), FAT16 (`0x06`), FAT32 CHS (`0x0B`), FAT32 LBA (`0x0C`) and
/// FAT16 LBA (`0x0E`).
pub const FAT_TYPES: [u8; 6] = [0x01, 0x04, 0x06, 0x0b, 0x0c, 0x0e];

/// One primary entry, decoded. `index` is 1-based (the `pM` of a source
/// string).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub index: u32,
    pub boot_flag: u8,
    pub part_type: u8,
    pub first_lba: u64,
    pub lba_count: u64,
}

/// Why a non-empty entry was not believed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejected {
    /// Not one of [`FAT_TYPES`] (an extended container, Linux, NTFS, ...).
    NotFat,
    /// The boot flag is neither `0x00` nor `0x80`.
    BadBootFlag,
    /// Starts at LBA 0 (over the table itself) or has no sectors.
    Degenerate,
    /// Ends past the device's last block.
    PastEnd,
    /// Overlaps another entry, of any type (another primary, an extended
    /// container, or a logical partition from its chain): both are refused,
    /// since nothing says which one is right.
    Overlaps,
}

/// What sector 0 is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sector0 {
    /// No `0x55AA`: neither a table nor a boot sector.
    Blank,
    /// A FAT boot sector at LBA 0: the whole device is the volume.
    Superfloppy,
    /// A GPT's protective MBR: the GPT path owns this disk.
    Protective,
    /// An MBR. Each slot is the entry's verdict; `None` for an empty slot.
    Table([Option<Result<Entry, (Entry, Rejected)>>; PRIMARY_ENTRIES]),
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// True when `sector` starts with a believable FAT BPB (the T11 rule): a
/// jump (`EB ?? 90` or `E9`), bytes per sector a power of two in 512..=4096,
/// sectors per cluster a power of two, reserved sectors non-zero, one or two
/// FATs, and a non-zero total sector count (16- or 32-bit field). A real MBR
/// fails this almost always: its first bytes are boot code, and its bytes
/// 11..=21 rarely look like a BPB.
pub fn is_fat_boot_sector(sector: &[u8]) -> bool {
    if sector.len() < 512 {
        return false;
    }
    let jump = (sector[0] == 0xeb && sector[2] == 0x90) || sector[0] == 0xe9;
    let bytes_per_sector = le_u16(sector, 11);
    let sectors_per_cluster = sector[13];
    let reserved = le_u16(sector, 14);
    let fats = sector[16];
    let total16 = le_u16(sector, 19);
    let total32 = le_u32(sector, 32);
    jump && bytes_per_sector.is_power_of_two()
        && (512..=4096).contains(&bytes_per_sector)
        && sectors_per_cluster.is_power_of_two()
        && reserved != 0
        && (fats == 1 || fats == 2)
        && (total16 != 0 || total32 != 0)
}

/// Decode primary slot `slot` (0..4) of `sector`. `None` for an empty slot
/// (type `0x00`).
pub fn entry(sector: &[u8], slot: usize) -> Option<Entry> {
    let at = TABLE_OFFSET + slot * ENTRY_SIZE;
    let part_type = sector[at + 4];
    if part_type == 0 {
        return None;
    }
    Some(Entry {
        index: slot as u32 + 1,
        boot_flag: sector[at],
        part_type,
        first_lba: le_u32(sector, at + 8) as u64,
        lba_count: le_u32(sector, at + 12) as u64,
    })
}

/// Extended-container partition types: CHS (`0x05`), LBA (`0x0F`), Linux
/// (`0x85`).
pub const EXTENDED_TYPES: [u8; 3] = [0x05, 0x0f, 0x85];
/// The most logical partitions one chain may hold before it is refused.
pub const MAX_LOGICAL: usize = 32;

/// Why an extended chain was refused (the walk stops there; what it found
/// before still counts as claimed space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainError {
    /// More than [`MAX_LOGICAL`] links.
    TooLong,
    /// A link that doesn't move strictly forward (a loop, or a step back).
    Loop,
    /// A link outside its extended container.
    OutsideContainer,
    /// An EBR that couldn't be read, or without `0x55AA`.
    BadEbr,
}

/// Walk the EBR chain of the extended container `(first_lba, lba_count)`:
/// each EBR's first entry is a logical partition (relative to that EBR), its
/// second the next EBR (relative to the container's start). `read(lba)`
/// returns one 512-byte sector. Every logical partition found is written to
/// `out` as `(first, end)`; returns how many, and the error that stopped the
/// walk if one did. Bounded: at most [`MAX_LOGICAL`] reads, every link
/// strictly after the one before and inside the container.
pub fn extended_chain(
    container: (u64, u64),
    read: &mut dyn FnMut(u64) -> Option<[u8; 512]>,
    out: &mut [(u64, u64); MAX_LOGICAL],
) -> (usize, Option<ChainError>) {
    let (start, count) = container;
    let end = start + count;
    let mut ebr = start;
    let mut n = 0;
    loop {
        if ebr < start || ebr >= end {
            return (n, Some(ChainError::OutsideContainer));
        }
        if n == MAX_LOGICAL {
            return (n, Some(ChainError::TooLong));
        }
        let Some(sector) = read(ebr) else {
            return (n, Some(ChainError::BadEbr));
        };
        if sector[510..512] != SIGNATURE {
            return (n, Some(ChainError::BadEbr));
        }
        if let Some(logical) = entry(&sector, 0)
            && logical.lba_count != 0
        {
            let first = ebr + logical.first_lba;
            out[n] = (first, first + logical.lba_count);
            n += 1;
        }
        let Some(next) = entry(&sector, 1) else {
            return (n, None);
        };
        let next = start + next.first_lba;
        if next <= ebr {
            return (n, Some(ChainError::Loop));
        }
        ebr = next;
    }
}

/// Classify sector 0 (at least 512 bytes) of a device of `block_count`
/// blocks, with no extended chain read (`classify_with` with nothing more
/// claimed).
pub fn classify(sector: &[u8], block_count: u64) -> Sector0 {
    classify_with(sector, block_count, &[])
}

/// [`classify`], with `logical` (the `(first, end)` ranges an extended
/// chain claims, from [`extended_chain`]) counted as taken space too.
pub fn classify_with(sector: &[u8], block_count: u64, logical: &[(u64, u64)]) -> Sector0 {
    if sector.len() < 512 || sector[510..512] != SIGNATURE {
        return Sector0::Blank;
    }
    if is_fat_boot_sector(sector) {
        return Sector0::Superfloppy;
    }
    if (0..PRIMARY_ENTRIES)
        .any(|s| entry(sector, s).is_some_and(|e| e.part_type == TYPE_GPT_PROTECTIVE))
    {
        return Sector0::Protective;
    }
    // Every non-empty primary with a real extent claims its space, whatever
    // its type and whether or not it is believed itself.
    let mut claimed: [Option<(u64, u64)>; PRIMARY_ENTRIES] = [None; PRIMARY_ENTRIES];
    for (slot, c) in claimed.iter_mut().enumerate() {
        if let Some(e) = entry(sector, slot)
            && e.lba_count != 0
        {
            *c = Some((e.first_lba, e.first_lba + e.lba_count));
        }
    }
    let mut out = [None; PRIMARY_ENTRIES];
    for (slot, verdict) in out.iter_mut().enumerate() {
        let Some(e) = entry(sector, slot) else {
            continue;
        };
        let others = claimed
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != slot)
            .filter_map(|(_, c)| *c)
            .chain(logical.iter().copied());
        *verdict = Some(judge(&e, block_count, others).map_err(|r| (e, r)));
    }
    Sector0::Table(out)
}

fn judge(
    e: &Entry,
    block_count: u64,
    mut others: impl Iterator<Item = (u64, u64)>,
) -> Result<Entry, Rejected> {
    if e.boot_flag != 0x00 && e.boot_flag != 0x80 {
        return Err(Rejected::BadBootFlag);
    }
    if !FAT_TYPES.contains(&e.part_type) {
        return Err(Rejected::NotFat);
    }
    if e.first_lba == 0 || e.lba_count == 0 {
        return Err(Rejected::Degenerate);
    }
    // u32 + u32 never overflows u64.
    let end = e.first_lba + e.lba_count;
    if end > block_count {
        return Err(Rejected::PastEnd);
    }
    if others.any(|(s, x)| e.first_lba < x && s < end) {
        return Err(Rejected::Overlaps);
    }
    Ok(*e)
}

/// The believed entry `index` (1..=4) of `sector`, or `None` (no table, a
/// superfloppy, a protective MBR, an empty slot, or a rejected entry).
/// `logical` as for [`classify_with`].
pub fn partition(
    sector: &[u8],
    block_count: u64,
    index: u32,
    logical: &[(u64, u64)],
) -> Option<Entry> {
    if !(1..=PRIMARY_ENTRIES as u32).contains(&index) {
        return None;
    }
    match classify_with(sector, block_count, logical) {
        Sector0::Table(slots) => slots[index as usize - 1]?.ok(),
        _ => None,
    }
}

/// Parse a block-device source: `"deviceN"` (the whole device, partition
/// `None`) or `"deviceN:pM"` (MBR partition M, 1..=4). Anything else is
/// `None`.
pub fn parse_source(source: &str) -> Option<(usize, Option<u32>)> {
    let rest = source.strip_prefix("device")?;
    let (dev, part) = match rest.split_once(":p") {
        Some((d, p)) => (d, Some(p)),
        None => (rest, None),
    };
    let device = parse_decimal(dev)?;
    let part = match part {
        None => None,
        Some(p) => {
            let m = parse_decimal(p)?;
            if !(1..=PRIMARY_ENTRIES).contains(&m) {
                return None;
            }
            Some(m as u32)
        }
    };
    Some((device, part))
}

fn parse_decimal(s: &str) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    let mut v = 0usize;
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((b - b'0') as usize)?;
    }
    Some(v)
}
