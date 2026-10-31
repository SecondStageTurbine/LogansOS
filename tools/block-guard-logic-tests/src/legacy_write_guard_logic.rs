// SPDX-License-Identifier: MPL-2.0
//! OT-87 R3, the differential oracle: `write_guard_logic.rs` exactly as it was
//! on `main` at `e0a59501`, before its GPT parser moved into `gpt_logic.rs`.
//! Test-only and frozen: `differential_tests` holds the new guard to this
//! one's judgement over the whole guard corpus and a mutation sweep. Never
//! edit it, except to drop it once R3 has merged and been reviewed.
//!
//! (The original's doc comment and code follow, unchanged.)

//! Pure rules for the block write guard (Zenbook lane: "we never write to a
//! disk that is not ours"), shared by `drivers::block`, xtask (which builds
//! the sealed markers) and the host KATs in `tools/block-guard-logic-tests`
//! (all mount this file by `#[path]`).
//!
//! A device is written only inside an **extent** that a marker read from it
//! claims, and only if that marker is sealed and structurally valid:
//! - a NEOFSv1 superblock (any of its three copies) whose BLAKE3 checksum and
//!   format fields validate claims its own volume, `[0, total_blocks * 4 KiB)`;
//! - a sealed log-sink header (v3: `NEO_LOG_v1`, region size, BLAKE3 seal) at
//!   256 MiB claims only its own region, `[256 MiB, 256 MiB + region_size)`;
//! - a sealed scratch sector (xtask's throwaway QEMU disks) in the last 512
//!   bytes, naming the device's exact capacity, claims the whole device.
//!
//! On a disk with a partition table (MBR or GPT), a claimed extent must lie
//! entirely in space the table leaves unallocated: not in a partition, not in
//! the table's own sectors. A Windows GPT disk is allocated from its first
//! usable LBA to its last, so no claim on it can ever arm, even if a file on
//! it carries a valid-looking marker at one of the probe offsets. A GPT is
//! believed only when a header copy validates: header CRC32, its own LBA,
//! usable range and entry array in bounds, entry-array CRC32, and every
//! partition inside the usable range. The primary is tried, then the backup
//! in the device's last LBA (Janus F1: a pointer edit with stale CRCs made a
//! Windows disk read as an empty GPT). A table we cannot believe (a
//! protective MBR or a GPT signature with no valid copy, an unreadable LBA 0)
//! counts as allocating the whole disk.
//!
//! Anything else, a laptop's own Windows NVMe above all, is never written.

extern crate alloc;

use alloc::vec::Vec;

/// Compiled into a kernel built with the guard OFF (the `block_write_guard`
/// control), and nowhere else. xtask refuses to export a bare-metal bundle
/// whose image contains it (Janus F2: whatever the feature was spelled).
pub const GUARD_OFF_MARKER: &[u8; 35] = b"NEOPLUTARCH-BLOCK-WRITE-GUARD-OFF/1";

/// NEOFSv1 superblock magic (`fs::neofs::format`).
pub const NEOFS_MAGIC: &[u8; 8] = b"NEOFSv1\0";
/// NEOFSv1 superblock copy offsets (`fs::neofs::format::SUPERBLOCK_COPY_OFFSETS`).
pub const NEOFS_SUPERBLOCK_OFFSETS: [u64; 3] = [0, 1024 * 1024, 64 * 1024 * 1024];
/// NEOFSv1 superblock size and the start of its BLAKE3 checksum.
pub const NEOFS_SUPERBLOCK_SIZE: usize = 512;
const NEOFS_CHECKSUM_AT: usize = 480;
/// NEOFSv1 format fields the guard checks (`fs::neofs::format`).
const NEOFS_FORMAT_VERSION: u16 = 1;
const NEOFS_BLOCK_SIZE_LOG2: u16 = 12;
const NEOFS_TYPE_VERSION: u8 = 1;

/// Log-sink header magic (`log::block_sink::LOG_MAGIC`).
pub const LOG_MAGIC: &[u8; 11] = b"NEO_LOG_v1\0";
/// Log-sink region offset (`log::block_sink::DEFAULT_REGION_OFFSET`).
pub const LOG_REGION_OFFSET: u64 = 0x1000_0000;
/// Log-sink header layout (`log::block_sink`): size, the fields the guard
/// reads, and the v3 seal (BLAKE3 of every byte before it).
pub const LOG_HEADER_SIZE: usize = 4096;
pub const LOG_OFFSET_REGION_SIZE: usize = 0x010;
pub const LOG_OFFSET_HEADER_VERSION: usize = 0x088;
pub const LOG_HEADER_VERSION: u32 = 3;
pub const LOG_OFFSET_SEAL: usize = LOG_HEADER_SIZE - 32;
/// The largest region a sealed header may claim.
const LOG_REGION_MAX: u64 = 1 << 30;

/// Scratch-disk sector (xtask's QEMU disks), the device's last 512 bytes:
/// magic, the device capacity, BLAKE3 seal of the 480 bytes before it.
pub const SCRATCH_MAGIC: &[u8; 32] = b"NEOPLUTARCH_SCRATCH_DISK_v2\0\0\0\0\0";
pub const SCRATCH_SECTOR: usize = 512;
const SCRATCH_OFFSET_CAPACITY: usize = 32;
const SCRATCH_SEAL_AT: usize = 480;

/// Which marker claimed an extent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WriteMarker {
    NeoFs,
    LogSink,
    Scratch,
}

impl WriteMarker {
    pub const fn name(self) -> &'static str {
        match self {
            WriteMarker::NeoFs => "neofs",
            WriteMarker::LogSink => "log-sink",
            WriteMarker::Scratch => "scratch",
        }
    }
}

/// A byte range `[start, end)` on the device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Extent {
    pub start: u64,
    pub end: u64,
}

impl Extent {
    /// Whether `[start, end)` lies inside this extent.
    pub const fn contains(&self, start: u64, end: u64) -> bool {
        start <= end && start >= self.start && end <= self.end
    }

    pub const fn overlaps(&self, other: &Extent) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// A marker's claim that passed every check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Claim {
    pub marker: WriteMarker,
    pub extent: Extent,
}

/// What a device may have written to it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Arming {
    /// Writable extents (empty: the device is never written).
    pub claims: Vec<Claim>,
    /// Valid claims dropped because a partition table allocates their space.
    pub blocked_by_table: usize,
    /// The partition table found: "none", "mbr", "gpt" or "opaque".
    pub table: &'static str,
}

impl Arming {
    /// Whether a write of `[start, end)` lies inside one armed extent.
    pub fn allows(&self, start: u64, end: u64) -> bool {
        self.claims.iter().any(|c| c.extent.contains(start, end))
    }
}

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

/// BLAKE3 of `bytes`, the seal of every marker.
pub fn seal(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_u64(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

/// The volume size a NEOFSv1 superblock claims, if it validates: magic,
/// BLAKE3 checksum, format version, 4 KiB blocks, type version, a non-zero
/// block count.
pub fn neofs_volume_bytes(sb: &[u8; NEOFS_SUPERBLOCK_SIZE]) -> Option<u64> {
    if &sb[..8] != NEOFS_MAGIC
        || seal(&sb[..NEOFS_CHECKSUM_AT]) != sb[NEOFS_CHECKSUM_AT..]
        || le_u16(sb, 8) != NEOFS_FORMAT_VERSION
        || le_u16(sb, 10) != NEOFS_BLOCK_SIZE_LOG2
        || sb[12] != NEOFS_TYPE_VERSION
    {
        return None;
    }
    match le_u64(sb, 16) {
        0 => None,
        blocks => blocks.checked_mul(1 << NEOFS_BLOCK_SIZE_LOG2),
    }
}

/// Seal a log-sink header in place (v3): the BLAKE3 of every byte before
/// `LOG_OFFSET_SEAL` goes into the last 32 bytes.
pub fn seal_log_header(header: &mut [u8; LOG_HEADER_SIZE]) {
    let s = seal(&header[..LOG_OFFSET_SEAL]);
    header[LOG_OFFSET_SEAL..].copy_from_slice(&s);
}

/// The region size a sealed log-sink header claims, if it validates: magic,
/// version 3, seal, and whole 4 KiB pages above the header, at most 1 GiB.
pub fn log_region_bytes(header: &[u8; LOG_HEADER_SIZE]) -> Option<u64> {
    if &header[..LOG_MAGIC.len()] != LOG_MAGIC
        || le_u32(header, LOG_OFFSET_HEADER_VERSION) != LOG_HEADER_VERSION
        || seal(&header[..LOG_OFFSET_SEAL]) != header[LOG_OFFSET_SEAL..]
    {
        return None;
    }
    let size = le_u64(header, LOG_OFFSET_REGION_SIZE);
    if size <= LOG_HEADER_SIZE as u64 || size > LOG_REGION_MAX || !size.is_multiple_of(4096) {
        return None;
    }
    Some(size)
}

/// The header an operator provisions before the first bare-metal run
/// (`cargo xtask log-sink-header`): magic, region size, version 3, all else
/// zero, sealed. The kernel rewrites and reseals it on install.
pub fn provisioning_log_header(region_size: u64) -> [u8; LOG_HEADER_SIZE] {
    let mut h = [0u8; LOG_HEADER_SIZE];
    h[..LOG_MAGIC.len()].copy_from_slice(LOG_MAGIC);
    h[LOG_OFFSET_REGION_SIZE..LOG_OFFSET_REGION_SIZE + 8]
        .copy_from_slice(&region_size.to_le_bytes());
    h[LOG_OFFSET_HEADER_VERSION..LOG_OFFSET_HEADER_VERSION + 4]
        .copy_from_slice(&LOG_HEADER_VERSION.to_le_bytes());
    seal_log_header(&mut h);
    h
}

/// The scratch sector xtask writes at the end of a `capacity`-byte QEMU disk.
pub fn scratch_sector(capacity: u64) -> [u8; SCRATCH_SECTOR] {
    let mut s = [0u8; SCRATCH_SECTOR];
    s[..SCRATCH_MAGIC.len()].copy_from_slice(SCRATCH_MAGIC);
    s[SCRATCH_OFFSET_CAPACITY..SCRATCH_OFFSET_CAPACITY + 8]
        .copy_from_slice(&capacity.to_le_bytes());
    let sealed = seal(&s[..SCRATCH_SEAL_AT]);
    s[SCRATCH_SEAL_AT..].copy_from_slice(&sealed);
    s
}

/// Whether `sector` is a sealed scratch sector for a disk of `capacity`.
pub fn scratch_valid(sector: &[u8; SCRATCH_SECTOR], capacity: u64) -> bool {
    &sector[..SCRATCH_MAGIC.len()] == SCRATCH_MAGIC
        && le_u64(sector, SCRATCH_OFFSET_CAPACITY) == capacity
        && seal(&sector[..SCRATCH_SEAL_AT]) == sector[SCRATCH_SEAL_AT..]
}

/// A partition table's allocated space.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Table {
    /// No MBR signature with a used entry, no GPT header.
    None,
    /// The table's own sectors and every partition.
    Allocated(&'static str, Vec<Extent>),
    /// A table we cannot believe: the whole disk counts as allocated. The
    /// name (`opaque-gpt-invalid`, `opaque-unreadable`) is the refusal
    /// line's `table=`.
    Opaque(&'static str),
}

const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
/// GPT entry arrays above this are refused as implausible (128 x 128 B is the
/// norm).
const GPT_MAX_ENTRY_BYTES: u64 = 512 * 1024;
/// The GPT header's size field: 92 bytes in UEFI 2.x, at most a sector.
const GPT_HEADER_MIN: usize = 92;

/// The CRC32 GPT uses (UEFI 2.10 §5.3.2: IEEE 802.3, reflected polynomial
/// 0xEDB88320, initial and final XOR 0xFFFFFFFF).
pub fn crc32(bytes: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut t = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            t[i] = c;
            i += 1;
        }
        t
    };
    let mut c = !0u32;
    for &b in bytes {
        c = TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

/// Read the partition table, if any (MBR at LBA 0, GPT headers at LBA 1 and
/// in the last LBA). Fails closed (R1-1): a read error, a GPT or protective
/// MBR with no valid copy, a malformed MBR, a boot sector or filesystem with
/// no partition table (a whole-disk "superfloppy" volume), or a boot
/// signature over an empty table is `Opaque`. Only a disk with no table and
/// no recognisable contents is `None`. On a valid GPT, any MBR partitions
/// beside it (a hybrid MBR) are allocated too (R1-2).
pub fn read_table(disk: &dyn Disk) -> Table {
    let ss = disk.sector_size();
    let cap = disk.capacity();
    if ss < 512 || cap < ss * 2 {
        return Table::None;
    }
    let mut lba0 = [0u8; 512];
    if !disk.read(0, &mut lba0) {
        return Table::Opaque("opaque-unreadable");
    }
    let total = cap / ss;
    let boot_signature = lba0[510] == 0x55 && lba0[511] == 0xaa;
    let mut mbr_parts: Vec<Extent> = Vec::new();
    let mut protective = false;
    let mut mbr_valid = true;
    if boot_signature {
        for entry in lba0[446..510].chunks_exact(16) {
            let kind = entry[4];
            if kind == 0 {
                continue;
            }
            if kind == 0xee {
                protective = true;
                continue;
            }
            let start = le_u32(entry, 8) as u64;
            let len = le_u32(entry, 12) as u64;
            // Boot flag 0x00 or 0x80, a non-empty extent on the device.
            if !matches!(entry[0], 0x00 | 0x80) || len == 0 || start + len > total {
                mbr_valid = false;
            }
            mbr_parts.push(Extent {
                start: start.saturating_mul(ss),
                end: (start + len).saturating_mul(ss),
            });
        }
    }

    let last_lba = total - 1;
    let primary = read_gpt_header(disk, 1, ss);
    let mut gpt = primary
        .as_ref()
        .and_then(|hdr| validated_gpt(disk, hdr, 1, ss, cap))
        .map(|out| ("gpt", out));
    let backup = if gpt.is_none() {
        // The primary is absent or invalid: the backup in the last LBA,
        // checked the same way and on its own (nothing from the primary is
        // trusted).
        read_gpt_header(disk, last_lba, ss)
    } else {
        None
    };
    if gpt.is_none()
        && let Some(hdr) = &backup
    {
        gpt = validated_gpt(disk, hdr, last_lba, ss, cap).map(|out| ("gpt-backup", out));
    }
    if let Some((name, mut out)) = gpt {
        // A hybrid MBR: what either table allocates is allocated.
        out.extend(mbr_parts);
        return Table::Allocated(name, out);
    }
    if primary.is_some() || backup.is_some() || protective {
        // Someone's GPT, and no copy of it we can believe: never write.
        return Table::Opaque("opaque-gpt-invalid");
    }
    if boot_signature && !mbr_valid {
        return Table::Opaque("opaque-mbr-invalid");
    }
    if !mbr_parts.is_empty() {
        // LBA 0 itself, and what a boot loader keeps beside it.
        mbr_parts.push(Extent { start: 0, end: ss });
        return Table::Allocated("mbr", mbr_parts);
    }
    // No partition table: someone's whole-disk volume, or a boot sector over
    // an empty table, is still someone's.
    match whole_disk_contents(disk, &lba0, boot_signature) {
        Some(reason) => Table::Opaque(reason),
        None => Table::None,
    }
}

/// A recognisable volume or boot record on a disk with no partition table:
/// FAT / NTFS / exFAT / BitLocker boot sectors, LUKS, LVM, ext2/3/4, an
/// ISO 9660 volume, or any boot signature. `None` when there is nothing.
fn whole_disk_contents(
    disk: &dyn Disk,
    lba0: &[u8; 512],
    boot_signature: bool,
) -> Option<&'static str> {
    let oem = &lba0[3..11];
    let jump = (lba0[0] == 0xeb && lba0[2] == 0x90) || lba0[0] == 0xe9;
    if jump
        || oem == b"NTFS    "
        || oem == b"EXFAT   "
        || oem == b"-FVE-FS-"
        || &lba0[54..59] == b"FAT12"
        || &lba0[54..59] == b"FAT16"
        || &lba0[82..87] == b"FAT32"
        || lba0[..6] == *b"LUKS\xba\xbe"
    {
        return Some("opaque-filesystem");
    }
    if boot_signature {
        return Some("opaque-boot-record");
    }
    // LVM's label in sector 1, the ext superblock magic (0xEF53 at byte
    // 1080), the ISO 9660 primary volume descriptor ("CD001" at 32769).
    let probes: [(u64, &[u8]); 15] = [
        (512, b"LABELONE"),                    // LVM
        (1080, &[0x53, 0xef]),                 // ext2/3/4
        (32769, b"CD001"),                     // ISO 9660
        (0, b"XFSB"),                          // XFS
        (32, b"NXSB"),                         // APFS container
        (3, b"ReFS"),                          // ReFS
        (1024, b"H+"),                         // HFS+
        (1024, b"HX"),                         // HFSX
        (1024, &[0x10, 0x20, 0xf5, 0xf2]),     // f2fs
        (0x1_0040, b"_BHRfS_M"),               // btrfs
        (0, &[0xfc, 0x4e, 0x2b, 0xa9]),        // md v1.1
        (4096, &[0xfc, 0x4e, 0x2b, 0xa9]),     // md v1.2
        (0x2_0000, &[0x0c, 0xb1, 0xba, 0x00]), // ZFS uberblock (label 0)
        (4086, b"SWAPSPACE2"),                 // Linux swap (4 KiB pages)
        (4086, b"SWAP-SPACE"),                 // Linux swap (old)
    ];
    for (at, magic) in probes {
        let mut buf = [0u8; 16];
        let buf = &mut buf[..magic.len()];
        if at + magic.len() as u64 > disk.capacity() {
            continue;
        }
        if !disk.read(at, buf) {
            return Some("opaque-unreadable");
        }
        if buf == magic {
            return Some("opaque-filesystem");
        }
    }
    None
}

/// The 512 bytes at `lba`, if they start with the GPT signature (or cannot
/// be read: an unreadable header sector is not "no GPT").
fn read_gpt_header(disk: &dyn Disk, lba: u64, ss: u64) -> Option<[u8; 512]> {
    let mut hdr = [0u8; 512];
    if !disk.read(lba.saturating_mul(ss), &mut hdr) {
        // Zeroed: fails validation, but still counts as a GPT seen.
        return Some([0u8; 512]);
    }
    (&hdr[..8] == GPT_SIGNATURE).then_some(hdr)
}

/// The space a GPT header copy at `lba` allocates, or `None` unless the copy
/// validates completely (Janus F1): signature, header size and CRC32, its
/// own LBA, the usable range inside the device and not over the header, the
/// entry array in bounds and outside the usable range, the entry-array
/// CRC32, and every partition inside the usable range.
fn validated_gpt(
    disk: &dyn Disk,
    hdr: &[u8; 512],
    lba: u64,
    ss: u64,
    cap: u64,
) -> Option<Vec<Extent>> {
    let last_lba = cap / ss - 1;
    if &hdr[..8] != GPT_SIGNATURE {
        return None;
    }
    let header_size = le_u32(hdr, 12) as usize;
    if !(GPT_HEADER_MIN..=hdr.len()).contains(&header_size) || header_size as u64 > ss {
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
        || array > GPT_MAX_ENTRY_BYTES
    {
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
    let mut entries = alloc::vec![0u8; array as usize];
    if !disk.read(entries_lba.saturating_mul(ss), &mut entries)
        || crc32(&entries) != le_u32(hdr, 88)
    {
        return None;
    }
    // The protective MBR, the primary header and its entries: everything
    // below the first usable LBA.
    let mut out = alloc::vec![Extent {
        start: 0,
        end: first_usable.saturating_mul(ss),
    }];
    if lba == 1 && alternate <= last_lba {
        // The backup header and the backup entry array just before it.
        out.push(Extent {
            start: alternate
                .saturating_sub(array.div_ceil(ss))
                .saturating_mul(ss),
            end: alternate.saturating_add(1).saturating_mul(ss),
        });
    } else {
        // The backup copy is in use, or the primary names a backup past the
        // end of the device: keep everything past the last usable LBA.
        out.push(Extent {
            start: last_usable.saturating_add(1).saturating_mul(ss),
            end: u64::MAX,
        });
    }
    for entry in entries.chunks_exact(entry_size as usize) {
        if entry[..16].iter().all(|&b| b == 0) {
            continue;
        }
        let first = le_u64(entry, 32);
        let last = le_u64(entry, 40);
        if last < first || !usable.contains(&first) || !usable.contains(&last) {
            return None;
        }
        out.push(Extent {
            start: first.saturating_mul(ss),
            end: last.saturating_add(1).saturating_mul(ss),
        });
    }
    Some(out)
}

/// How much of a table-less disk's start must be blank (all zero) before a
/// log sink may arm on it: 1 MiB, past every whole-disk volume's first
/// superblock or label we know of.
pub const BLANK_START_BYTES: u64 = 1024 * 1024;

/// Whether the first `BLANK_START_BYTES` (or the whole device, if smaller)
/// read back all zero. A read error is not blank.
fn start_is_blank(disk: &dyn Disk) -> bool {
    let end = BLANK_START_BYTES.min(disk.capacity());
    let mut buf = [0u8; 4096];
    let mut at = 0;
    while at < end {
        let take = (end - at).min(buf.len() as u64) as usize;
        if !disk.read(at, &mut buf[..take]) || buf[..take].iter().any(|&b| b != 0) {
            return false;
        }
        at += take as u64;
    }
    true
}

/// Judge a device: read every marker, keep the sealed, structurally valid
/// claims that fit the device, then drop any claim the partition table
/// allocates. Reads only.
pub fn judge(disk: &dyn Disk) -> Arming {
    let cap = disk.capacity();
    let mut claims: Vec<Claim> = Vec::new();
    // NEOFS: the valid primary's volume alone; with no valid primary, the
    // copies that validate must agree, or none is believed (R1-6: a stale
    // secondary saying a larger volume widened the extent).
    let mut copies: [Option<u64>; 3] = [None; 3];
    for (slot, offset) in NEOFS_SUPERBLOCK_OFFSETS.iter().enumerate() {
        let mut sb = [0u8; NEOFS_SUPERBLOCK_SIZE];
        if offset.saturating_add(NEOFS_SUPERBLOCK_SIZE as u64) > cap || !disk.read(*offset, &mut sb)
        {
            continue;
        }
        copies[slot] = neofs_volume_bytes(&sb).filter(|&bytes| bytes <= cap);
    }
    let neofs_primary = copies[0].is_some();
    let neofs_bytes = match copies[0] {
        Some(bytes) => Some(bytes),
        None => {
            let mut valid = copies.iter().flatten();
            let first = valid.next().copied();
            first.filter(|f| valid.all(|b| b == f))
        }
    };
    if let Some(bytes) = neofs_bytes {
        claims.push(Claim {
            marker: WriteMarker::NeoFs,
            extent: Extent {
                start: 0,
                end: bytes,
            },
        });
    }

    let mut header = [0u8; LOG_HEADER_SIZE];
    if LOG_REGION_OFFSET.saturating_add(LOG_HEADER_SIZE as u64) <= cap
        && disk.read(LOG_REGION_OFFSET, &mut header)
        && let Some(size) = log_region_bytes(&header)
        && LOG_REGION_OFFSET + size <= cap
    {
        claims.push(Claim {
            marker: WriteMarker::LogSink,
            extent: Extent {
                start: LOG_REGION_OFFSET,
                end: LOG_REGION_OFFSET + size,
            },
        });
    }

    let mut sector = [0u8; SCRATCH_SECTOR];
    if cap >= SCRATCH_SECTOR as u64
        && disk.read(cap - SCRATCH_SECTOR as u64, &mut sector)
        && scratch_valid(&sector, cap)
    {
        claims.push(Claim {
            marker: WriteMarker::Scratch,
            extent: Extent { start: 0, end: cap },
        });
    }

    // A valid primary NEOFS superblock owns LBA 0, so no MBR lives there (its
    // checksum bytes could otherwise mimic a 0x55AA signature).
    let table = if neofs_primary {
        Table::None
    } else {
        read_table(disk)
    };
    let before = claims.len();
    let name = match &table {
        // R1-7: no table and nothing recognised is not proof the disk is
        // free. A sink, the one claim a copied file could carry onto
        // someone's disk, arms on a table-less disk only if the disk's
        // start is blank (an allow-list; the signatures are a second layer).
        // A disk whose LBA 0 is our own NEOFS primary is exempt.
        Table::None
            if !neofs_primary && claims.iter().any(|c| c.marker == WriteMarker::LogSink) =>
        {
            if start_is_blank(disk) {
                "none"
            } else {
                claims.retain(|c| c.marker != WriteMarker::LogSink);
                "none-start-in-use"
            }
        }
        Table::None => "none",
        Table::Allocated(kind, allocated) => {
            claims.retain(|c| !allocated.iter().any(|a| a.overlaps(&c.extent)));
            kind
        }
        Table::Opaque(reason) => {
            claims.clear();
            reason
        }
    };
    Arming {
        blocked_by_table: before - claims.len(),
        claims,
        table: name,
    }
}
