// SPDX-License-Identifier: MPL-2.0
//! OT-100 R3: the persistent image's oracle
//! (`docs/THINK-2026-11-10-ot100-usb-persistence.md` §2.2). It reads the
//! built ISO with its own GPT reader, written from the UEFI 2.10 §5.3 byte
//! layout, independent of xorriso and of the kernel's `gpt_logic`, and checks
//! that the persistence area is exactly as designed. Then the guard KAT runs
//! the kernel's real `write_guard_logic::judge_with_partitions` over the image
//! as built and over a "repaired" copy (the backup GPT moved to a larger
//! disk's end, as firmware or Windows do), and both must arm exactly the
//! area's span.

use crate::{gpt_logic, write_guard_logic};

const SECTOR: u64 = 512;
/// The LogansOS NeoFS partition type `EAA7A9D0-75C1-4385-9664-B2A74FC2622D`,
/// as GPT stores a GUID: the first three fields little-endian, the last two
/// as bytes.
pub const PERSIST_TYPE: [u8; 16] = [
    0xd0, 0xa9, 0xa7, 0xea, 0xc1, 0x75, 0x85, 0x43, 0x96, 0x64, 0xb2, 0xa7, 0x4f, 0xc2, 0x62, 0x2d,
];
/// 1 MiB in sectors: the area's start alignment.
const ALIGN_SECTORS: u64 = (1 << 20) / SECTOR;
/// NEOFSv1's superblock (`fs::neofs::format`, ADR-030): magic, format
/// version 1, block size log2 12, type version 1, total blocks at 16, the
/// BLAKE3 of bytes 0..480 at 480.
const NEOFS_MAGIC: &[u8; 8] = b"NEOFSv1\0";
const NEOFS_SEAL_AT: usize = 480;

/// Where the area was found.
#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    /// The GPT entry's 1-based number.
    pub entry: usize,
    pub first_lba: u64,
    pub last_lba: u64,
}

/// CRC-32 (IEEE 802.3, reflected, as GPT uses), bit by bit: no table to get
/// wrong.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// One GPT header's fields (UEFI §5.3.2).
#[derive(Debug, Clone, Copy)]
struct Header {
    my_lba: u64,
    alternate_lba: u64,
    first_usable: u64,
    last_usable: u64,
    disk_guid: [u8; 16],
    entries_lba: u64,
    count: u32,
    entry_size: u32,
    array_crc: u32,
}

fn sector(image: &[u8], lba: u64) -> Result<&[u8], String> {
    let at = (lba * SECTOR) as usize;
    image
        .get(at..at + SECTOR as usize)
        .ok_or_else(|| format!("LBA {lba} is past the image"))
}

fn header(image: &[u8], lba: u64, which: &str) -> Result<Header, String> {
    let s = sector(image, lba)?;
    if &s[..8] != b"EFI PART" {
        return Err(format!("{which} GPT header at LBA {lba}: no signature"));
    }
    if u32_at(s, 8) != 0x0001_0000 {
        return Err(format!("{which} GPT header: revision {:#x}", u32_at(s, 8)));
    }
    let size = u32_at(s, 12) as usize;
    if size != 92 {
        return Err(format!("{which} GPT header: size {size}"));
    }
    let mut h = s[..size].to_vec();
    h[16..20].fill(0);
    if crc32(&h) != u32_at(s, 16) {
        return Err(format!("{which} GPT header: CRC mismatch"));
    }
    Ok(Header {
        my_lba: u64_at(s, 24),
        alternate_lba: u64_at(s, 32),
        first_usable: u64_at(s, 40),
        last_usable: u64_at(s, 48),
        disk_guid: s[56..72].try_into().unwrap(),
        entries_lba: u64_at(s, 72),
        count: u32_at(s, 80),
        entry_size: u32_at(s, 84),
        array_crc: u32_at(s, 88),
    })
}

fn array<'a>(image: &'a [u8], h: &Header, which: &str) -> Result<&'a [u8], String> {
    if h.entry_size != 128 || h.count == 0 || h.count > 1024 {
        return Err(format!(
            "{which} GPT: {} entries of {} bytes",
            h.count, h.entry_size
        ));
    }
    let at = (h.entries_lba * SECTOR) as usize;
    let len = h.count as usize * h.entry_size as usize;
    let bytes = image
        .get(at..at + len)
        .ok_or_else(|| format!("{which} GPT entry array is past the image"))?;
    if crc32(bytes) != h.array_crc {
        return Err(format!("{which} GPT entry array: CRC mismatch"));
    }
    Ok(bytes)
}

/// Check the persistent image (THINK §2.2's oracle, items 1-5, and the APM):
/// `area` is the formatted volume, which must sit in the area byte for byte.
pub fn check(image: &[u8], area: &[u8]) -> Result<Found, String> {
    let area_len = area.len() as u64;
    if !(image.len() as u64).is_multiple_of(SECTOR) || (image.len() as u64) < 64 * SECTOR {
        return Err(format!("image length {} is not whole sectors", image.len()));
    }
    let last = image.len() as u64 / SECTOR - 1;
    // 1. Both headers, their CRCs, their LBA pairs and one disk GUID.
    let primary = header(image, 1, "primary")?;
    if primary.my_lba != 1 || primary.alternate_lba != last {
        return Err(format!(
            "primary GPT header: my_lba {} alternate {} (want 1, {last})",
            primary.my_lba, primary.alternate_lba
        ));
    }
    let backup = header(image, last, "backup")?;
    if backup.my_lba != last || backup.alternate_lba != 1 {
        return Err(format!(
            "backup GPT header: my_lba {} alternate {} (want {last}, 1)",
            backup.my_lba, backup.alternate_lba
        ));
    }
    if primary.disk_guid != backup.disk_guid {
        return Err("the two GPT headers name different disk GUIDs".to_string());
    }
    if (primary.first_usable, primary.last_usable) != (backup.first_usable, backup.last_usable) {
        return Err("the two GPT headers disagree on the usable range".to_string());
    }
    // 2. Both entry arrays, byte-identical.
    let entries = array(image, &primary, "primary")?;
    if array(image, &backup, "backup")? != entries {
        return Err("the two GPT entry arrays differ".to_string());
    }
    // 3. Exactly one LogansOS entry: attributes 0, 1 MiB-aligned, usable.
    let mut used = Vec::new();
    let mut area_entry = None;
    for (i, e) in entries.chunks_exact(128).enumerate() {
        if e[..16].iter().all(|&b| b == 0) {
            continue;
        }
        let (first, last_lba, attrs) = (u64_at(e, 32), u64_at(e, 40), u64_at(e, 48));
        if first > last_lba {
            return Err(format!(
                "GPT entry {}: first {first} > last {last_lba}",
                i + 1
            ));
        }
        if e[..16] == PERSIST_TYPE {
            if area_entry.is_some() {
                return Err("two LogansOS entries".to_string());
            }
            if attrs != 0 {
                return Err(format!("the area's attributes are {attrs:#x}, not 0"));
            }
            area_entry = Some((i + 1, first, last_lba));
        }
        used.push((i + 1, first, last_lba));
    }
    let (entry, first_lba, last_lba) = area_entry.ok_or("no LogansOS entry in the GPT")?;
    if first_lba % ALIGN_SECTORS != 0 {
        return Err(format!(
            "the area starts at LBA {first_lba}, not 1 MiB-aligned"
        ));
    }
    if first_lba < primary.first_usable || last_lba > primary.last_usable {
        return Err(format!(
            "the area {first_lba}..={last_lba} is outside the usable {}..={}",
            primary.first_usable, primary.last_usable
        ));
    }
    if (last_lba - first_lba + 1) * SECTOR != area_len {
        return Err(format!(
            "the area is {} bytes, the formatted volume {area_len}",
            (last_lba - first_lba + 1) * SECTOR
        ));
    }
    // 4. No other entry overlaps it.
    for &(n, a, b) in &used {
        if n != entry && a <= last_lba && first_lba <= b {
            return Err(format!("GPT entry {n} ({a}..={b}) overlaps the area"));
        }
    }
    // 5. A sealed NEOFSv1 superblock whose volume fits, at the area's start.
    let start = (first_lba * SECTOR) as usize;
    let sb = &image[start..start + 512];
    if &sb[..8] != NEOFS_MAGIC {
        return Err("the area does not start with a NEOFSv1 superblock".to_string());
    }
    if sb[8..10] != [1, 0] || sb[10..12] != [12, 0] || sb[12] != 1 || sb[13..16] != [0, 0, 0] {
        return Err(format!(
            "superblock fields: version {:?}, block log2 {:?}, type {}",
            &sb[8..10],
            &sb[10..12],
            sb[12]
        ));
    }
    if blake3::hash(&sb[..NEOFS_SEAL_AT]).as_bytes()[..] != sb[NEOFS_SEAL_AT..] {
        return Err("the area's superblock seal does not match".to_string());
    }
    let blocks = u64_at(sb, 16);
    if blocks == 0 || blocks.saturating_mul(4096) > area_len {
        return Err(format!(
            "the superblock's {blocks} blocks don't fit the {area_len}-byte area"
        ));
    }
    if image.get(start..start + area.len()) != Some(area) {
        return Err("the area's bytes differ from the formatted volume".to_string());
    }
    // The APM (from `-hfsplus`) must not map the area.
    let mut k = 1u64;
    while let Some(e) = image.get((k * 2048) as usize..(k * 2048 + 512) as usize) {
        if &e[..2] != b"PM" {
            break;
        }
        let a = u64::from(u32::from_be_bytes(e[8..12].try_into().unwrap())) * 4;
        let n = u64::from(u32::from_be_bytes(e[12..16].try_into().unwrap())) * 4;
        if n > 0 && a <= last_lba && first_lba < a + n {
            return Err(format!(
                "APM entry {k} maps LBAs {a}..{} over the area",
                a + n
            ));
        }
        k += 1;
    }
    Ok(Found {
        entry,
        first_lba,
        last_lba,
    })
}

/// A disk over a byte buffer, for the guard.
struct Bytes<'a>(&'a [u8]);

impl gpt_logic::Disk for Bytes<'_> {
    fn capacity(&self) -> u64 {
        self.0.len() as u64
    }
    fn sector_size(&self) -> u64 {
        SECTOR
    }
    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        let at = offset as usize;
        match self.0.get(at..at + out.len()) {
            Some(src) => {
                out.copy_from_slice(src);
                true
            }
            None => false,
        }
    }
}

/// The image on a larger disk after a firmware or Windows GPT repair: `extra`
/// zero bytes appended, the backup entry array and header moved to the new
/// end, both headers' alternate / last-usable fields and CRCs redone.
pub fn repaired(image: &[u8], extra: u64) -> Result<Vec<u8>, String> {
    let last = image.len() as u64 / SECTOR - 1;
    let primary = header(image, 1, "primary")?;
    let backup = header(image, last, "backup")?;
    let array_sectors = (u64::from(primary.count) * 128).div_ceil(SECTOR);
    let mut out = image.to_vec();
    out.resize(image.len() + extra as usize, 0);
    let new_last = out.len() as u64 / SECTOR - 1;
    let new_entries = new_last - array_sectors;
    let array_bytes = array(image, &backup, "backup")?.to_vec();
    // Clear the old backup copy, as a repair overwrites it with data later.
    let old = (backup.entries_lba * SECTOR) as usize;
    out[old..(last + 1) as usize * SECTOR as usize].fill(0);
    let at = (new_entries * SECTOR) as usize;
    out[at..at + array_bytes.len()].copy_from_slice(&array_bytes);
    let new_last_usable = new_entries - 1;
    let fix = |out: &mut Vec<u8>, lba: u64, my: u64, alt: u64, entries: u64| {
        let o = (lba * SECTOR) as usize;
        out[o + 24..o + 32].copy_from_slice(&my.to_le_bytes());
        out[o + 32..o + 40].copy_from_slice(&alt.to_le_bytes());
        out[o + 48..o + 56].copy_from_slice(&new_last_usable.to_le_bytes());
        out[o + 72..o + 80].copy_from_slice(&entries.to_le_bytes());
        out[o + 16..o + 20].fill(0);
        let crc = crc32(&out[o..o + 92]);
        out[o + 16..o + 20].copy_from_slice(&crc.to_le_bytes());
    };
    // The backup header: copy the primary's sector, then make it the backup.
    let p = SECTOR as usize;
    let primary_sector = out[p..2 * p].to_vec();
    let b = (new_last * SECTOR) as usize;
    out[b..b + p].copy_from_slice(&primary_sector);
    fix(&mut out, new_last, new_last, 1, new_entries);
    fix(&mut out, 1, 1, new_last, primary.entries_lba);
    Ok(out)
}

/// The guard KAT: the kernel's own judgement arms exactly the area's span,
/// on the image as built and on its repaired copy.
pub fn guard_arms_exactly(image: &[u8], found: &Found) -> Result<(), String> {
    let span = write_guard_logic::Extent {
        start: found.first_lba * SECTOR,
        end: (found.last_lba + 1) * SECTOR,
    };
    for (what, disk) in [
        ("as built", image.to_vec()),
        ("repaired to a larger disk", repaired(image, 64 << 20)?),
    ] {
        let arming = write_guard_logic::judge_with_partitions(&Bytes(&disk));
        let extents: Vec<_> = arming.claims.iter().map(|c| c.extent).collect();
        if extents != [span] {
            return Err(format!(
                "guard KAT ({what}): armed {extents:?}, want exactly {span:?} (table {})",
                arming.table
            ));
        }
    }
    Ok(())
}

/// The LogansOS entry's LBA span in `image`'s primary GPT, if it has one
/// (no checks: the scenarios use it to find the area they corrupt).
pub fn area_span(image: &[u8]) -> Option<(u64, u64)> {
    let h = header(image, 1, "primary").ok()?;
    let at = (h.entries_lba * SECTOR) as usize;
    let entries = image.get(at..at + h.count as usize * h.entry_size as usize)?;
    entries
        .chunks_exact(h.entry_size as usize)
        .find(|e| e[..16] == PERSIST_TYPE)
        .map(|e| (u64_at(e, 32), u64_at(e, 40)))
}

/// `image` with a different GPT disk GUID in both headers (CRCs redone): a
/// second stick written from another build.
pub fn set_disk_guid(image: &mut [u8], guid: [u8; 16]) -> Result<(), String> {
    let last = image.len() as u64 / SECTOR - 1;
    header(image, 1, "primary")?;
    header(image, last, "backup")?;
    for lba in [1, last] {
        let o = (lba * SECTOR) as usize;
        image[o + 56..o + 72].copy_from_slice(&guid);
        image[o + 16..o + 20].fill(0);
        let crc = crc32(&image[o..o + 92]);
        image[o + 16..o + 20].copy_from_slice(&crc.to_le_bytes());
    }
    Ok(())
}

/// A GUID string's bytes as GPT stores them.
fn guid(s: &str) -> [u8; 16] {
    let hex: Vec<u8> = s
        .bytes()
        .filter(|b| *b != b'-')
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    let mut g = [0u8; 16];
    g[..4].copy_from_slice(&[hex[3], hex[2], hex[1], hex[0]]);
    g[4..6].copy_from_slice(&[hex[5], hex[4]]);
    g[6..8].copy_from_slice(&[hex[7], hex[6]]);
    g[8..].copy_from_slice(&hex[8..16]);
    g
}

/// THINK §5's internal disk: a Windows-shaped GPT disk of `size` bytes (ESP,
/// MSR, basic data, recovery), with `planted` (a sealed NEOFSv1 superblock)
/// at the data partition's start and at 64 MiB, which lies inside it: the
/// whole-disk NeoFS copy's place. A guard that believed either would arm
/// Windows' data.
pub fn windows_disk(size: u64, planted: &[u8; 512]) -> Vec<u8> {
    const MIB: u64 = (1 << 20) / SECTOR;
    let mut d = vec![0u8; size as usize];
    let last = size / SECTOR - 1;
    let parts = [
        (
            "C12A7328-F81F-11D2-BA4B-00A0C93EC93B",
            MIB,
            17 * MIB - 1,
            "EFI system partition",
        ),
        (
            "E3C9E316-0B5C-4DB8-817D-F92DF00215AE",
            17 * MIB,
            33 * MIB - 1,
            "Microsoft reserved partition",
        ),
        (
            "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7",
            33 * MIB,
            last - 33 - 16 * MIB,
            "Basic data partition",
        ),
        (
            "DE94BBA4-06D1-4D40-A16A-BFD50179D6AC",
            last - 33 - 16 * MIB + 1,
            last - 34,
            "Recovery",
        ),
    ];
    let mut entries = vec![0u8; 128 * 128];
    for (i, (ty, first, end, name)) in parts.iter().enumerate() {
        let e = &mut entries[i * 128..(i + 1) * 128];
        e[..16].copy_from_slice(&guid(ty));
        e[16..32].copy_from_slice(&[(i as u8) + 1; 16]);
        e[32..40].copy_from_slice(&first.to_le_bytes());
        e[40..48].copy_from_slice(&end.to_le_bytes());
        for (k, c) in name.encode_utf16().enumerate() {
            e[56 + 2 * k..58 + 2 * k].copy_from_slice(&c.to_le_bytes());
        }
    }
    // The protective MBR.
    d[446 + 4] = 0xee;
    d[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    d[446 + 12..446 + 16].copy_from_slice(&(last.min(u64::from(u32::MAX)) as u32).to_le_bytes());
    d[510] = 0x55;
    d[511] = 0xaa;
    let array_crc = crc32(&entries);
    let mut put_header = |lba: u64, alt: u64, entries_lba: u64| {
        let o = (lba * SECTOR) as usize;
        let h = &mut d[o..o + 92];
        h[..8].copy_from_slice(b"EFI PART");
        h[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        h[12..16].copy_from_slice(&92u32.to_le_bytes());
        h[24..32].copy_from_slice(&lba.to_le_bytes());
        h[32..40].copy_from_slice(&alt.to_le_bytes());
        h[40..48].copy_from_slice(&34u64.to_le_bytes());
        h[48..56].copy_from_slice(&(last - 33).to_le_bytes());
        h[56..72].copy_from_slice(&[0x5d; 16]);
        h[72..80].copy_from_slice(&entries_lba.to_le_bytes());
        h[80..84].copy_from_slice(&128u32.to_le_bytes());
        h[84..88].copy_from_slice(&128u32.to_le_bytes());
        h[88..92].copy_from_slice(&array_crc.to_le_bytes());
        let crc = crc32(h);
        h[16..20].copy_from_slice(&crc.to_le_bytes());
    };
    put_header(1, last, 2);
    put_header(last, 1, last - 32);
    d[2 * SECTOR as usize..2 * SECTOR as usize + entries.len()].copy_from_slice(&entries);
    let b = ((last - 32) * SECTOR) as usize;
    d[b..b + entries.len()].copy_from_slice(&entries);
    let data = (33 * MIB * SECTOR) as usize;
    d[data..data + 512].copy_from_slice(planted);
    let at = 64usize << 20;
    d[at..at + 512].copy_from_slice(planted);
    d
}

/// The guard's judgement of a disk image (the scenario holds the internal
/// disk to "nothing armed").
pub fn guard_claims(disk: &[u8]) -> usize {
    write_guard_logic::judge_with_partitions(&Bytes(disk))
        .claims
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CRC-32 check value from the standard's catalogue: "123456789".
    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    /// The persistent menu, from the real `boot/limine.conf`: `timeout: 3`,
    /// the first entry with `cmdline: persist=on` after its `protocol:` line,
    /// and a second entry, the same modules, with `persist=off`.
    #[test]
    fn the_persistent_menu() {
        let src =
            std::fs::read_to_string(crate::workspace_root().join("boot/limine.conf")).unwrap();
        let conf = crate::persist_limine_conf(&src).unwrap();
        assert!(conf.lines().any(|l| l == "timeout: 3"));
        assert!(!conf.lines().any(|l| l == "timeout: 0"));
        let entries: Vec<&str> = conf.lines().filter(|l| l.starts_with('/')).collect();
        assert_eq!(
            entries,
            ["/NeoPlutarch", "/NeoPlutarch - try (nothing is saved)"]
        );
        let cmdlines: Vec<&str> = conf
            .lines()
            .filter(|l| l.trim_start().starts_with("cmdline:"))
            .map(str::trim)
            .collect();
        assert_eq!(cmdlines, ["cmdline: persist=on", "cmdline: persist=off"]);
        let modules = |from: &str| {
            let at = conf.find(from).unwrap();
            let rest = &conf[at + from.len()..];
            let end = rest.find("\n/").unwrap_or(rest.len());
            rest[..end]
                .lines()
                .filter(|l| l.trim_start().starts_with("module_path:"))
                .count()
        };
        assert_eq!(modules("/NeoPlutarch\n"), modules("/NeoPlutarch - try"));
        assert!(modules("/NeoPlutarch\n") >= 4);
        // Built once only: a second pass refuses (two entries, a command line).
        assert!(crate::persist_limine_conf(&conf).is_err());
    }

    /// The kill-switch: without the area, the arguments are the pre-R3 list
    /// plus only the pinned file dates; with it, the five extra options.
    #[test]
    fn the_xorriso_arguments() {
        let s = |v: Vec<std::ffi::OsString>| -> Vec<String> {
            v.into_iter().map(|a| a.into_string().unwrap()).collect()
        };
        let base = s(crate::xorriso_iso_args("1700000000", None));
        assert_eq!(
            base,
            [
                "-as",
                "mkisofs",
                "-R",
                "-r",
                "-J",
                "-b",
                "boot/limine-bios-cd.bin",
                "-no-emul-boot",
                "-boot-load-size",
                "4",
                "-boot-info-table",
                "-hfsplus",
                "-apm-block-size",
                "2048",
                "--efi-boot",
                "boot/limine-uefi-cd.bin",
                "-efi-boot-part",
                "--efi-boot-image",
                "--protective-msdos-label",
                "--set_all_file_dates",
                "=1700000000",
            ]
        );
        let with = s(crate::xorriso_iso_args(
            "1700000000",
            Some(std::path::Path::new("/a/persist.neofs")),
        ));
        assert_eq!(&with[..base.len()], &base[..]);
        assert_eq!(
            &with[base.len()..],
            [
                "-partition_cyl_align",
                "all",
                "-partition_hd_cyl",
                "64",
                "-partition_sec_hd",
                "32",
                "-append_partition",
                "3",
                "EAA7A9D0-75C1-4385-9664-B2A74FC2622D",
                "/a/persist.neofs",
                "-appended_part_as_gpt",
            ]
        );
    }

    #[test]
    fn the_persist_flag() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            crate::split_persist_mib(&a(&["--features", "x", "--persist-mib", "512"])).unwrap(),
            (Some(512), a(&["--features", "x"]))
        );
        assert_eq!(
            crate::split_persist_mib(&a(&["--persist-mib=65"])).unwrap(),
            (Some(65), a(&[]))
        );
        assert_eq!(
            crate::split_persist_mib(&a(&["x"])).unwrap(),
            (None, a(&["x"]))
        );
        for bad in [
            &["--persist-mib", "64"][..],
            &["--persist-mib", "16385"],
            &["--persist-mib", "x"],
            &["--persist-mib"],
            &["--persist-mib=100", "--persist-mib=100"],
        ] {
            assert!(crate::split_persist_mib(&a(bad)).is_err(), "{bad:?}");
        }
        assert!(crate::refuse_persist_for_vm(&a(&["--persist-mib", "512"])).is_err());
        assert!(crate::refuse_persist_for_vm(&a(&["--persist-mib=512"])).is_err());
        assert!(crate::refuse_persist_for_vm(&a(&["--features", "x"])).is_ok());
    }

    /// The Windows-shaped disk is a valid GPT the guard never arms, even
    /// with sealed superblocks planted inside its data partition (THINK §5).
    #[test]
    fn the_windows_disk_is_never_armed() {
        let mut sb = [0u8; 512];
        sb[..8].copy_from_slice(b"NEOFSv1\0");
        sb[8] = 1;
        sb[10] = 12;
        sb[12] = 1;
        sb[16..24].copy_from_slice(&256u64.to_le_bytes());
        let seal = *blake3::hash(&sb[..480]).as_bytes();
        sb[480..].copy_from_slice(&seal);
        let disk = windows_disk(128 << 20, &sb);
        let arming = write_guard_logic::judge_with_partitions(&Bytes(&disk));
        assert_eq!(arming.table, "gpt");
        assert!(arming.claims.is_empty(), "{:?}", arming.claims);
        assert!(
            arming.blocked_by_table >= 1,
            "the planted copy is seen, then blocked"
        );
        assert_eq!(guid("EAA7A9D0-75C1-4385-9664-B2A74FC2622D"), PERSIST_TYPE);
    }

    /// The type GUID's bytes, laid out by hand from the string form.
    #[test]
    fn the_type_guid_bytes() {
        // EAA7A9D0 -> d0 a9 a7 ea; 75C1 -> c1 75; 4385 -> 85 43; then as is.
        assert_eq!(&PERSIST_TYPE[..4], &[0xd0, 0xa9, 0xa7, 0xea]);
        assert_eq!(&PERSIST_TYPE[4..8], &[0xc1, 0x75, 0x85, 0x43]);
        assert_eq!(
            &PERSIST_TYPE[8..],
            &[0x96, 0x64, 0xb2, 0xa7, 0x4f, 0xc2, 0x62, 0x2d]
        );
    }
}
