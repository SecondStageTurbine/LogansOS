// SPDX-License-Identifier: MPL-2.0
//! The GUID partition table: one validated reader and the installer's writer
//! (OT-87 R3, `docs/THINK-2026-10-27-ot87-installer.md` §4.2, §4.4).
//!
//! Pure and allocation-light. The write guard (`write_guard_logic`), the
//! kernel's partition view, xtask's one-disk VM image and the host KATs in
//! `tools/block-guard-logic-tests` all mount this file by `#[path]`, so keep it
//! free of `crate::` paths and feature cfgs.
//!
//! - [`read_header`] / [`validate`]: a header copy is believed only when it
//!   validates completely (the guard's rule since Janus F1): signature, header
//!   size and CRC32, its own LBA, the usable range inside the device and not
//!   over the header, the entry array in bounds and outside the usable range,
//!   the entry-array CRC32, each copy's alternate LBA naming the other, the
//!   usable range clear of both copies' headers and arrays, every partition
//!   inside the usable range, and no two partitions overlapping.
//! - [`gpt_layout`]: the protective MBR, both headers and the entry array the
//!   installer writes for a sector size `S` and last LBA `L` (§4.4).

extern crate alloc;

use alloc::vec::Vec;

/// Read access to the device being judged. The kernel's implementation goes
/// through the driver; the KATs use an in-memory image.
pub trait Disk {
    /// Device size in bytes.
    fn capacity(&self) -> u64;
    /// Logical sector size in bytes (512 or 4096).
    fn sector_size(&self) -> u64;
    /// Fill `out` from byte `offset`. False on an I/O error or past the end.
    fn read(&self, offset: u64, out: &mut [u8]) -> bool;
}

pub const SIGNATURE: &[u8; 8] = b"EFI PART";
/// The most entry-array bytes a header may describe (the UEFI minimum is
/// 16 KiB; 512 KiB bounds the read).
pub const MAX_ENTRY_BYTES: u64 = 512 * 1024;
/// The UEFI header size; a copy claiming less is not believed.
pub const HEADER_MIN: usize = 92;

/// The EFI system partition type, C12A7328-F81F-11D2-BA4B-00A0C93EC93B
/// (UEFI 2.10 §5.3.3), as GPT stores it: the first three fields
/// little-endian, the last two as written.
pub const ESP_TYPE_GUID: [u8; 16] = [
    0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
];

/// The LogansOS NeoFS partition type, EAA7A9D0-75C1-4385-9664-B2A74FC2622D,
/// minted once for OT-87 R3 (a random v4 UUID), as GPT stores it.
pub const LOGANSOS_NEOFS_TYPE_GUID: [u8; 16] = [
    0xd0, 0xa9, 0xa7, 0xea, 0xc1, 0x75, 0x85, 0x43, 0x96, 0x64, 0xb2, 0xa7, 0x4f, 0xc2, 0x62, 0x2d,
];

/// Partition attribute bit 48 on a LogansOS NeoFS entry: the install has not
/// finished (§4.7). `boot-neofs` never mounts such a partition and the
/// write guard's partition rule never arms it.
pub const ATTR_INCOMPLETE: u64 = 1 << 48;

/// CRC-32/ISO-HDLC (polynomial 0x04C11DB7, implemented reflected as
/// 0xEDB88320; init and xorout 0xFFFFFFFF): the GPT's header and entry-array
/// checksum.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_u64(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

/// One used (non-zero type GUID) entry of a validated table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    /// 1-based position in the entry array.
    pub index: u32,
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub first_lba: u64,
    /// Inclusive.
    pub last_lba: u64,
    pub attributes: u64,
}

/// A header copy that validated, and its used entries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Gpt {
    /// The LBA of the copy that validated (1, or the device's last LBA).
    pub header_lba: u64,
    pub alternate_lba: u64,
    pub first_usable: u64,
    pub last_usable: u64,
    pub disk_guid: [u8; 16],
    pub entries_lba: u64,
    /// Entry-array bytes: entry count × entry size.
    pub array_bytes: u64,
    pub entries: Vec<Entry>,
}

/// The `S`-byte sector at `lba` (up to 512 bytes of it), if it starts with
/// the GPT signature, or if it cannot be read: an unreadable header sector is
/// not "no GPT".
pub fn read_header(disk: &dyn Disk, lba: u64, ss: u64) -> Option<[u8; 512]> {
    let mut hdr = [0u8; 512];
    if !disk.read(lba.saturating_mul(ss), &mut hdr) {
        // Zeroed: fails validation, but still counts as a GPT seen.
        return Some([0u8; 512]);
    }
    (&hdr[..8] == SIGNATURE).then_some(hdr)
}

/// The table a header copy at `lba` describes, or `None` unless the copy
/// validates completely (see the module doc).
pub fn validate(disk: &dyn Disk, hdr: &[u8; 512], lba: u64, ss: u64, cap: u64) -> Option<Gpt> {
    let last_lba = cap / ss - 1;
    if &hdr[..8] != SIGNATURE {
        return None;
    }
    let header_size = le_u32(hdr, 12) as usize;
    if !(HEADER_MIN..=hdr.len()).contains(&header_size) || header_size as u64 > ss {
        return None;
    }
    let mut copy = [0u8; 512];
    copy[..header_size].copy_from_slice(&hdr[..header_size]);
    copy[16..20].fill(0);
    if crc32(&copy[..header_size]) != le_u32(hdr, 16) {
        return None;
    }
    let my_lba = le_u64(hdr, 24);
    let alternate = le_u64(hdr, 32);
    let first_usable = le_u64(hdr, 40);
    let last_usable = le_u64(hdr, 48);
    let entries_lba = le_u64(hdr, 72);
    let count = le_u32(hdr, 80) as u64;
    let entry_size = le_u32(hdr, 84) as u64;
    let array = count.saturating_mul(entry_size);
    let usable = first_usable..=last_usable;
    if my_lba != lba
        || first_usable < 2
        || first_usable > last_usable
        || last_usable > last_lba
        || usable.contains(&lba)
        || !(128..=4096).contains(&entry_size)
        || !entry_size.is_multiple_of(8)
        || array == 0
        || array > MAX_ENTRY_BYTES
    {
        return None;
    }
    // The usable range stays clear of both copies' metadata, so no
    // partition can span it (Janus's R3 review, item 7; I-17). A is the
    // array's sectors.
    // - The primary's array, at LBA 2 by convention: `first_usable` is past
    //   it (a backup can't see where the primary put its array).
    // - The backup this copy names: a primary's alternate is the backup
    //   header, inside the device, with the backup array in the A sectors
    //   before it, all above `last_usable`. That is the device's last LBA, or
    //   the end of a smaller image written to a larger device (our own ISO on
    //   a USB stick). A backup's alternate is the primary, at LBA 1.
    let a = array.div_ceil(ss);
    let alternate_ok = if lba == 1 {
        alternate <= last_lba && last_usable.saturating_add(a) < alternate
    } else {
        alternate == 1
    };
    if !alternate_ok || first_usable < 2 + a {
        return None;
    }
    // The entry array: on the device, clear of LBA 0, its header and the
    // usable range.
    let array_end = entries_lba.checked_add(array.div_ceil(ss))?; // exclusive
    if entries_lba < 1
        || array_end > last_lba + 1
        || (entries_lba..array_end).contains(&lba)
        || (entries_lba <= last_usable && array_end > first_usable)
    {
        return None;
    }
    let mut array_bytes = alloc::vec![0u8; array as usize];
    if !disk.read(entries_lba.saturating_mul(ss), &mut array_bytes)
        || crc32(&array_bytes) != le_u32(hdr, 88)
    {
        return None;
    }
    let mut entries = Vec::new();
    for (i, entry) in array_bytes.chunks_exact(entry_size as usize).enumerate() {
        if entry[..16].iter().all(|&b| b == 0) {
            continue;
        }
        let first = le_u64(entry, 32);
        let last = le_u64(entry, 40);
        if last < first || !usable.contains(&first) || !usable.contains(&last) {
            return None;
        }
        let mut type_guid = [0u8; 16];
        type_guid.copy_from_slice(&entry[..16]);
        let mut unique_guid = [0u8; 16];
        unique_guid.copy_from_slice(&entry[16..32]);
        entries.push(Entry {
            index: i as u32 + 1,
            type_guid,
            unique_guid,
            first_lba: first,
            last_lba: last,
            attributes: le_u64(entry, 48),
        });
    }
    if any_overlap(&entries) {
        return None;
    }
    let mut disk_guid = [0u8; 16];
    disk_guid.copy_from_slice(&hdr[56..72]);
    Some(Gpt {
        header_lba: lba,
        alternate_lba: alternate,
        first_usable,
        last_usable,
        disk_guid,
        entries_lba,
        array_bytes: array,
        entries,
    })
}

/// The LBA ranges (inclusive) GPT metadata may occupy, from the copy that
/// validated, on a device whose last LBA is `last_lba`: the protective MBR,
/// the primary header and its array at LBA 2 (the convention) and wherever
/// this copy puts its own array, and the backup array and header before the
/// alternate this copy names and before the device's last LBA.
pub fn metadata_lbas(gpt: &Gpt, ss: u64, last_lba: u64) -> [(u64, u64); 5] {
    let a = gpt.array_bytes.div_ceil(ss).max(1);
    let backup_header = if gpt.header_lba == 1 {
        gpt.alternate_lba
    } else {
        gpt.header_lba
    };
    [
        (0, 1),
        (2, 1 + a),
        (gpt.entries_lba, gpt.entries_lba.saturating_add(a - 1)),
        (backup_header.saturating_sub(a), backup_header),
        (last_lba.saturating_sub(a), last_lba),
    ]
}

/// Whether any two used entries share an LBA. A copy that has them doesn't
/// validate (OT-87 R3, Muse item 5): two partitions over one span would let a
/// claim on one cover the other. Touching entries (`a.last + 1 == b.first`)
/// don't overlap.
fn any_overlap(entries: &[Entry]) -> bool {
    let mut spans: Vec<(u64, u64)> = entries.iter().map(|e| (e.first_lba, e.last_lba)).collect();
    spans.sort_unstable();
    spans.windows(2).any(|w| w[1].0 <= w[0].1)
}

/// The device's table: the primary at LBA 1 if it validates, else the backup
/// in the last LBA, checked on its own (nothing from the primary is trusted).
/// `None` when neither copy validates. This is the reader for the partition
/// view and the boot probe; the write guard keeps its own order of the same
/// two steps, because it must also tell "no GPT" from "a GPT we cannot
/// believe".
pub fn read(disk: &dyn Disk) -> Option<Gpt> {
    let ss = disk.sector_size();
    let cap = disk.capacity();
    if ss < 512 || cap < ss * 2 {
        return None;
    }
    let last_lba = cap / ss - 1;
    if let Some(gpt) = read_header(disk, 1, ss).and_then(|hdr| validate(disk, &hdr, 1, ss, cap)) {
        return Some(gpt);
    }
    read_header(disk, last_lba, ss).and_then(|hdr| validate(disk, &hdr, last_lba, ss, cap))
}

/// The device LBA a partition-relative request `[lba, lba + blocks)` maps to,
/// for a partition of `lba_count` blocks starting at `first_lba`. `None` when
/// the request is empty, crosses the partition's end, or overflows: a
/// crossing request is refused, never truncated.
pub fn partition_lba(first_lba: u64, lba_count: u64, lba: u64, blocks: u64) -> Option<u64> {
    let end = lba.checked_add(blocks)?;
    if blocks == 0 || end > lba_count {
        return None;
    }
    first_lba.checked_add(lba)
}

// === The writer (§4.4) ===

/// Entries in the installer's array, and their size: 16 KiB, the UEFI
/// minimum.
pub const LAYOUT_ENTRIES: u32 = 128;
pub const LAYOUT_ENTRY_SIZE: u32 = 128;
pub const LAYOUT_ARRAY_BYTES: usize = (LAYOUT_ENTRIES * LAYOUT_ENTRY_SIZE) as usize;
/// The ESP's size in MiB (§4.1, the measured budget of §7.2).
pub const LAYOUT_ESP_MIB: u64 = 512;

/// What [`gpt_layout`] writes, sector by sector, and where the two
/// partitions are.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Layout {
    pub sector_size: u64,
    pub last_lba: u64,
    /// LBA 0: the protective MBR, `S` bytes.
    pub mbr: Vec<u8>,
    /// LBA 1, `S` bytes.
    pub primary_header: Vec<u8>,
    /// LBA `L`, `S` bytes.
    pub backup_header: Vec<u8>,
    /// The 16 KiB array, written at LBA 2 and at LBA `L − A`.
    pub entries: Vec<u8>,
    pub backup_entries_lba: u64,
    /// Entry 1, the ESP: inclusive LBAs.
    pub esp: (u64, u64),
    /// Entry 2, LogansOS NeoFS: inclusive LBAs.
    pub neofs: (u64, u64),
    /// The NeoFS volume that fills entry 2, in 4 KiB blocks.
    pub neofs_total_blocks: u64,
}

fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

fn put_name(entry: &mut [u8], name: &str) {
    for (i, unit) in name.encode_utf16().take(36).enumerate() {
        entry[56 + 2 * i..58 + 2 * i].copy_from_slice(&unit.to_le_bytes());
    }
}

/// The table the installer writes on a disk with sector size `s` and last LBA
/// `l` (§4.4): entry 1 the 512 MiB ESP at 1 MiB, entry 2 LogansOS NeoFS from
/// there to the last 1 MiB boundary before the backup array, with
/// [`ATTR_INCOMPLETE`] set when `incomplete`. GUIDs are given as GPT stores
/// them. `None` for a sector size other than 512 or 4096, or a disk with no
/// room for a NeoFS partition after the ESP.
pub fn gpt_layout(
    s: u64,
    l: u64,
    disk_guid: [u8; 16],
    esp_guid: [u8; 16],
    neofs_guid: [u8; 16],
    incomplete: bool,
) -> Option<Layout> {
    if s != 512 && s != 4096 {
        return None;
    }
    let a = LAYOUT_ARRAY_BYTES as u64 / s;
    let m = (1024 * 1024) / s;
    let first_usable = 2 + a;
    let last_usable = l.checked_sub(1 + a)?;
    let esp = (m, m + LAYOUT_ESP_MIB * m - 1);
    let neofs_first = (LAYOUT_ESP_MIB + 1) * m;
    let neofs_last = ((l - a) / m).checked_mul(m)?.checked_sub(1)?;
    if neofs_last < neofs_first || neofs_last > last_usable {
        return None;
    }
    let neofs_total_blocks = (neofs_last + 1 - neofs_first) * s / 4096;

    let mut entries = alloc::vec![0u8; LAYOUT_ARRAY_BYTES];
    {
        let e1 = &mut entries[..128];
        e1[..16].copy_from_slice(&ESP_TYPE_GUID);
        e1[16..32].copy_from_slice(&esp_guid);
        put_u64(e1, 32, esp.0);
        put_u64(e1, 40, esp.1);
        put_name(e1, "EFI System");
    }
    {
        let e2 = &mut entries[128..256];
        e2[..16].copy_from_slice(&LOGANSOS_NEOFS_TYPE_GUID);
        e2[16..32].copy_from_slice(&neofs_guid);
        put_u64(e2, 32, neofs_first);
        put_u64(e2, 40, neofs_last);
        put_u64(e2, 48, if incomplete { ATTR_INCOMPLETE } else { 0 });
        put_name(e2, "LogansOS");
    }
    let crc_e = crc32(&entries);
    let backup_entries_lba = l - a;
    let header = |my: u64, alternate: u64, entries_lba: u64| {
        let mut h = alloc::vec![0u8; s as usize];
        h[..8].copy_from_slice(SIGNATURE);
        put_u32(&mut h, 8, 0x0001_0000);
        put_u32(&mut h, 12, HEADER_MIN as u32);
        put_u64(&mut h, 24, my);
        put_u64(&mut h, 32, alternate);
        put_u64(&mut h, 40, first_usable);
        put_u64(&mut h, 48, last_usable);
        h[56..72].copy_from_slice(&disk_guid);
        put_u64(&mut h, 72, entries_lba);
        put_u32(&mut h, 80, LAYOUT_ENTRIES);
        put_u32(&mut h, 84, LAYOUT_ENTRY_SIZE);
        put_u32(&mut h, 88, crc_e);
        let crc_h = crc32(&h[..HEADER_MIN]);
        put_u32(&mut h, 16, crc_h);
        h
    };
    let primary_header = header(1, l, 2);
    let backup_header = header(l, 1, backup_entries_lba);

    let mut mbr = alloc::vec![0u8; s as usize];
    let rec = &mut mbr[446..462];
    rec[0] = 0x00;
    rec[1..4].copy_from_slice(&[0x00, 0x02, 0x00]);
    rec[4] = 0xee;
    rec[5..8].copy_from_slice(&[0xff, 0xff, 0xff]);
    put_u32(rec, 8, 1);
    put_u32(rec, 12, l.min(0xffff_ffff) as u32);
    mbr[510] = 0x55;
    mbr[511] = 0xaa;

    Some(Layout {
        sector_size: s,
        last_lba: l,
        mbr,
        primary_header,
        backup_header,
        entries,
        backup_entries_lba,
        esp,
        neofs: (neofs_first, neofs_last),
        neofs_total_blocks,
    })
}
