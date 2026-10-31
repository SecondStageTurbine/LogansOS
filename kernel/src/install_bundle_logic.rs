// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.2: the install bundle, `NEOINST1`
//! (`docs/THINK-2026-10-27-ot87-installer.md` §4.3). Pure: the host builder
//! in xtask writes it, R4.3's installer reads it, and
//! `tools/install-bundle-logic-tests` holds both to a hand-laid byte oracle.
//!
//! One Limine module carries every byte the installer writes that isn't the
//! running system itself: the two ESP templates, their slot maps, and the
//! NeoFS seed. All integers are little-endian.
//!
//! ```text
//! header (32 bytes)
//!   0   8  magic "NEOINST1"
//!   8   4  version = 1
//!  12   4  part count N
//!  16   8  bundle length (bytes, the whole module)
//!  24   8  reserved = 0
//! part table (N × 96 bytes, from offset 32): the manifest
//!   0  32  name, ASCII, NUL-padded
//!  32   4  kind: 1 sparse image, 2 slot map
//!  36   4  reserved = 0
//!  40   8  offset of the part's stored bytes, from the bundle's start
//!  48   8  stored length
//!  56   8  expanded length (a sparse image's size; a slot map's = stored)
//!  64  32  BLAKE3 of the stored bytes
//! parts, in table order, contiguous, after the table
//! ```
//!
//! A **sparse image** is `run count (8)` then, per run, `offset (8)`,
//! `length (8)` and `length` bytes. Runs are non-empty, in ascending order,
//! don't overlap, and end within the expanded length. Every byte outside the
//! runs is zero. The builder emits 4096-aligned runs of the image's
//! non-zero 4 KiB blocks.
//!
//! A **slot map** names the preallocated files of one ESP template (§7.1),
//! each one contiguous:
//!
//! ```text
//!   0   4  sector size S (512 or 4096), the template's bytes per sector
//!   4   4  entry count M
//!   8   8  reserved = 0
//!  16  M × 64:
//!        0  32  path, ASCII, NUL-padded ("slot/A/kernel")
//!       32   8  first sector of the extent, partition-relative, in S
//!       40   8  extent length, in S
//!       48   8  the sector holding the file's 32-byte short directory entry
//!       56   4  that entry's byte offset in the sector
//!       60   4  reserved = 0
//! ```
//!
//! The whole bundle is pinned by its BLAKE3 (§4.3 item 2); each part is then
//! checked against the manifest before it is used.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

pub const MAGIC: &[u8; 8] = b"NEOINST1";
pub const VERSION: u32 = 1;
pub const HEADER_BYTES: usize = 32;
pub const PART_ENTRY_BYTES: usize = 96;
pub const NAME_BYTES: usize = 32;
pub const SLOT_MAP_HEADER_BYTES: usize = 16;
pub const SLOT_ENTRY_BYTES: usize = 64;

pub const KIND_SPARSE: u32 = 1;
pub const KIND_SLOT_MAP: u32 = 2;

/// The parts every bundle carries, in this order (§4.3 item 1).
pub const ESP_512: &str = "esp-512";
pub const ESP_4096: &str = "esp-4096";
pub const SLOTS_512: &str = "slots-512";
pub const SLOTS_4096: &str = "slots-4096";
pub const NEOFS_SEED: &str = "neofs-seed";
pub const PART_NAMES: [&str; 5] = [ESP_512, ESP_4096, SLOTS_512, SLOTS_4096, NEOFS_SEED];

/// Why a bundle, a part or a slot map was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BundleError {
    Truncated,
    BadMagic,
    BadVersion,
    LengthMismatch,
    BadName,
    BadKind,
    ReservedNotZero,
    /// A part's bytes lie outside the bundle, overlap another's, or leave a gap.
    BadPartRange,
    /// A part's stored bytes don't hash to the manifest's BLAKE3.
    PartHashMismatch,
    /// The parts aren't exactly [`PART_NAMES`], in order.
    WrongParts,
    BadRun,
    BadSectorSize,
    BadSlotEntry,
}

/// One manifest row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Part {
    pub name: String,
    pub kind: u32,
    pub offset: u64,
    pub length: u64,
    pub expanded: u64,
    pub blake3: [u8; 32],
}

/// One run of a sparse image.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Run<'a> {
    pub offset: u64,
    pub bytes: &'a [u8],
}

/// One slot-map row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotEntry {
    pub path: String,
    pub first_sector: u64,
    pub sectors: u64,
    pub dirent_sector: u64,
    pub dirent_offset: u32,
}

/// A parsed slot map.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotMap {
    pub sector_size: u32,
    pub entries: Vec<SlotEntry>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// A NUL-padded ASCII name: printable bytes, then only NULs.
fn name_at(b: &[u8]) -> Option<String> {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    if end == 0 || b[end..].iter().any(|&c| c != 0) {
        return None;
    }
    let name = &b[..end];
    if name.iter().any(|&c| !(0x21..=0x7e).contains(&c)) {
        return None;
    }
    Some(name.iter().map(|&c| c as char).collect())
}

fn put_name(out: &mut [u8], name: &str) {
    out[..name.len()].copy_from_slice(name.as_bytes());
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= NAME_BYTES && name.bytes().all(|c| (0x21..=0x7e).contains(&c))
}

/// The manifest of `bundle`: header and part table checked, and the parts
/// laid out contiguously from the end of the table to the end of the bundle.
/// Hashes are not checked here; see [`check_part`].
pub fn manifest(bundle: &[u8]) -> Result<Vec<Part>, BundleError> {
    if bundle.len() < HEADER_BYTES {
        return Err(BundleError::Truncated);
    }
    if &bundle[..8] != MAGIC {
        return Err(BundleError::BadMagic);
    }
    if u32_at(bundle, 8) != Some(VERSION) {
        return Err(BundleError::BadVersion);
    }
    let count = u32_at(bundle, 12).ok_or(BundleError::Truncated)? as usize;
    if u64_at(bundle, 16) != Some(bundle.len() as u64) {
        return Err(BundleError::LengthMismatch);
    }
    if u64_at(bundle, 24) != Some(0) {
        return Err(BundleError::ReservedNotZero);
    }
    let table_end = count
        .checked_mul(PART_ENTRY_BYTES)
        .and_then(|t| t.checked_add(HEADER_BYTES))
        .filter(|&end| end <= bundle.len())
        .ok_or(BundleError::Truncated)?;
    let mut parts = Vec::with_capacity(count);
    let mut next = table_end as u64;
    for i in 0..count {
        let e = &bundle[HEADER_BYTES + i * PART_ENTRY_BYTES..][..PART_ENTRY_BYTES];
        let name = name_at(&e[..NAME_BYTES]).ok_or(BundleError::BadName)?;
        let kind = u32_at(e, 32).ok_or(BundleError::Truncated)?;
        if kind != KIND_SPARSE && kind != KIND_SLOT_MAP {
            return Err(BundleError::BadKind);
        }
        if u32_at(e, 36) != Some(0) {
            return Err(BundleError::ReservedNotZero);
        }
        let offset = u64_at(e, 40).ok_or(BundleError::Truncated)?;
        let length = u64_at(e, 48).ok_or(BundleError::Truncated)?;
        let expanded = u64_at(e, 56).ok_or(BundleError::Truncated)?;
        if offset != next {
            return Err(BundleError::BadPartRange);
        }
        next = offset
            .checked_add(length)
            .filter(|&end| end <= bundle.len() as u64)
            .ok_or(BundleError::BadPartRange)?;
        if kind == KIND_SLOT_MAP && expanded != length {
            return Err(BundleError::BadPartRange);
        }
        let mut blake3 = [0u8; 32];
        blake3.copy_from_slice(&e[64..96]);
        parts.push(Part {
            name,
            kind,
            offset,
            length,
            expanded,
            blake3,
        });
    }
    if next != bundle.len() as u64 {
        return Err(BundleError::BadPartRange);
    }
    Ok(parts)
}

/// The manifest of a bundle that carries exactly [`PART_NAMES`], in order,
/// with the kinds they need.
pub fn installer_manifest(bundle: &[u8]) -> Result<Vec<Part>, BundleError> {
    let parts = manifest(bundle)?;
    let kinds = [
        KIND_SPARSE,
        KIND_SPARSE,
        KIND_SLOT_MAP,
        KIND_SLOT_MAP,
        KIND_SPARSE,
    ];
    let ok = parts.len() == PART_NAMES.len()
        && parts
            .iter()
            .zip(PART_NAMES.iter().zip(kinds))
            .all(|(p, (name, kind))| p.name == *name && p.kind == kind);
    if !ok {
        return Err(BundleError::WrongParts);
    }
    Ok(parts)
}

/// `part`'s stored bytes, once they hash to the manifest's BLAKE3.
pub fn check_part<'a>(bundle: &'a [u8], part: &Part) -> Result<&'a [u8], BundleError> {
    let end = part
        .offset
        .checked_add(part.length)
        .ok_or(BundleError::BadPartRange)?;
    let bytes = bundle
        .get(part.offset as usize..end as usize)
        .ok_or(BundleError::BadPartRange)?;
    if blake3::hash(bytes).as_bytes() != &part.blake3 {
        return Err(BundleError::PartHashMismatch);
    }
    Ok(bytes)
}

/// The runs of a sparse image whose expanded size is `expanded`.
pub fn sparse_runs(stored: &[u8], expanded: u64) -> Result<Vec<Run<'_>>, BundleError> {
    let count = u64_at(stored, 0).ok_or(BundleError::Truncated)?;
    let mut at = 8usize;
    let mut runs = Vec::new();
    let mut floor = 0u64;
    for _ in 0..count {
        let offset = u64_at(stored, at).ok_or(BundleError::Truncated)?;
        let length = u64_at(stored, at + 8).ok_or(BundleError::Truncated)?;
        at += 16;
        let end = offset.checked_add(length).ok_or(BundleError::BadRun)?;
        if length == 0 || offset < floor || end > expanded {
            return Err(BundleError::BadRun);
        }
        let bytes = stored
            .get(at..)
            .and_then(|rest| rest.get(..usize::try_from(length).ok()?))
            .ok_or(BundleError::Truncated)?;
        at += bytes.len();
        runs.push(Run { offset, bytes });
        floor = end;
    }
    if at != stored.len() {
        return Err(BundleError::BadRun);
    }
    Ok(runs)
}

/// Copy the bytes of the expanded image at `[offset, offset + out.len())`
/// into `out`: zeros except where a run covers them.
pub fn expand_into(runs: &[Run<'_>], offset: u64, out: &mut [u8]) {
    out.fill(0);
    let end = offset + out.len() as u64;
    for run in runs {
        let run_end = run.offset + run.bytes.len() as u64;
        if run_end <= offset || run.offset >= end {
            continue;
        }
        let from = run.offset.max(offset);
        let to = run_end.min(end);
        out[(from - offset) as usize..(to - offset) as usize]
            .copy_from_slice(&run.bytes[(from - run.offset) as usize..(to - run.offset) as usize]);
    }
}

/// A slot map.
pub fn slot_map(stored: &[u8]) -> Result<SlotMap, BundleError> {
    let sector_size = u32_at(stored, 0).ok_or(BundleError::Truncated)?;
    if sector_size != 512 && sector_size != 4096 {
        return Err(BundleError::BadSectorSize);
    }
    let count = u32_at(stored, 4).ok_or(BundleError::Truncated)? as usize;
    if u64_at(stored, 8) != Some(0) {
        return Err(BundleError::ReservedNotZero);
    }
    let want = count
        .checked_mul(SLOT_ENTRY_BYTES)
        .and_then(|t| t.checked_add(SLOT_MAP_HEADER_BYTES));
    if want != Some(stored.len()) {
        return Err(BundleError::Truncated);
    }
    let mut entries: Vec<SlotEntry> = Vec::with_capacity(count);
    for i in 0..count {
        let e = &stored[SLOT_MAP_HEADER_BYTES + i * SLOT_ENTRY_BYTES..][..SLOT_ENTRY_BYTES];
        let path = name_at(&e[..NAME_BYTES]).ok_or(BundleError::BadName)?;
        let first_sector = u64_at(e, 32).ok_or(BundleError::Truncated)?;
        let sectors = u64_at(e, 40).ok_or(BundleError::Truncated)?;
        let dirent_sector = u64_at(e, 48).ok_or(BundleError::Truncated)?;
        let dirent_offset = u32_at(e, 56).ok_or(BundleError::Truncated)?;
        if u32_at(e, 60) != Some(0) {
            return Err(BundleError::ReservedNotZero);
        }
        // Checked: a crafted offset or extent must be refused, not wrap
        // (release builds don't check overflow; Chad's R4.2 review, item 1).
        let Some(end) = first_sector.checked_add(sectors) else {
            return Err(BundleError::BadSlotEntry);
        };
        let entry_fits = dirent_offset % 32 == 0
            && dirent_offset
                .checked_add(32)
                .is_some_and(|e| e <= sector_size);
        // Extents don't overlap one another (each earlier one was checked
        // the same way, so its end doesn't overflow), and a directory entry
        // sits outside every extent.
        let overlaps = entries
            .iter()
            .any(|o| first_sector < o.first_sector + o.sectors && o.first_sector < end);
        if sectors == 0 || !entry_fits || overlaps {
            return Err(BundleError::BadSlotEntry);
        }
        entries.push(SlotEntry {
            path,
            first_sector,
            sectors,
            dirent_sector,
            dirent_offset,
        });
    }
    if entries.iter().any(|e| {
        entries
            .iter()
            .any(|o| (o.first_sector..o.first_sector + o.sectors).contains(&e.dirent_sector))
    }) {
        return Err(BundleError::BadSlotEntry);
    }
    Ok(SlotMap {
        sector_size,
        entries,
    })
}

/// The stored bytes of a sparse image: the non-zero `block`-byte blocks of
/// `image`, adjacent ones merged into one run.
pub fn encode_sparse(image: &[u8], block: usize) -> Vec<u8> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, chunk) in image.chunks(block).enumerate() {
        if chunk.iter().all(|&b| b == 0) {
            continue;
        }
        let start = i * block;
        let end = start + chunk.len();
        match runs.last_mut() {
            Some(last) if last.1 == start => last.1 = end,
            _ => runs.push((start, end)),
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&(runs.len() as u64).to_le_bytes());
    for (start, end) in runs {
        out.extend_from_slice(&(start as u64).to_le_bytes());
        out.extend_from_slice(&((end - start) as u64).to_le_bytes());
        out.extend_from_slice(&image[start..end]);
    }
    out
}

/// The stored bytes of a slot map.
pub fn encode_slot_map(map: &SlotMap) -> Option<Vec<u8>> {
    let mut out = alloc::vec![0u8; SLOT_MAP_HEADER_BYTES + map.entries.len() * SLOT_ENTRY_BYTES];
    out[0..4].copy_from_slice(&map.sector_size.to_le_bytes());
    out[4..8].copy_from_slice(&(map.entries.len() as u32).to_le_bytes());
    for (i, e) in map.entries.iter().enumerate() {
        if !valid_name(&e.path) {
            return None;
        }
        let row = &mut out[SLOT_MAP_HEADER_BYTES + i * SLOT_ENTRY_BYTES..][..SLOT_ENTRY_BYTES];
        put_name(&mut row[..NAME_BYTES], &e.path);
        row[32..40].copy_from_slice(&e.first_sector.to_le_bytes());
        row[40..48].copy_from_slice(&e.sectors.to_le_bytes());
        row[48..56].copy_from_slice(&e.dirent_sector.to_le_bytes());
        row[56..60].copy_from_slice(&e.dirent_offset.to_le_bytes());
    }
    Some(out)
}

/// A bundle of `parts`: `(name, kind, expanded length, stored bytes)`.
pub fn encode_bundle(parts: &[(&str, u32, u64, &[u8])]) -> Option<Vec<u8>> {
    let table_end = HEADER_BYTES + parts.len() * PART_ENTRY_BYTES;
    let total = table_end + parts.iter().map(|p| p.3.len()).sum::<usize>();
    let mut out = alloc::vec![0u8; table_end];
    out[..8].copy_from_slice(MAGIC);
    out[8..12].copy_from_slice(&VERSION.to_le_bytes());
    out[12..16].copy_from_slice(&(parts.len() as u32).to_le_bytes());
    out[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    let mut offset = table_end as u64;
    for (i, (name, kind, expanded, stored)) in parts.iter().enumerate() {
        if !valid_name(name) {
            return None;
        }
        let row = &mut out[HEADER_BYTES + i * PART_ENTRY_BYTES..][..PART_ENTRY_BYTES];
        put_name(&mut row[..NAME_BYTES], name);
        row[32..36].copy_from_slice(&kind.to_le_bytes());
        row[40..48].copy_from_slice(&offset.to_le_bytes());
        row[48..56].copy_from_slice(&(stored.len() as u64).to_le_bytes());
        row[56..64].copy_from_slice(&expanded.to_le_bytes());
        row[64..96].copy_from_slice(blake3::hash(stored).as_bytes());
        offset += stored.len() as u64;
    }
    for (_, _, _, stored) in parts {
        out.extend_from_slice(stored);
    }
    Some(out)
}
