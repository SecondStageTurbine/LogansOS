// SPDX-License-Identifier: MPL-2.0
//! L3a (G7) KATs for `kernel/src/drivers/block/mbr_logic.rs`. The byte
//! oracle is the hand-laid sectors below: every non-zero byte is written out
//! here from the MBR layout (boot code 0..446, four 16-byte entries at 446,
//! `55 AA` at 510), not produced by any writer in this tree.

use super::mbr_logic::{
    ChainError, Entry, MAX_LOGICAL, Rejected, Sector0, classify, extended_chain,
    is_fat_boot_sector, parse_source, partition,
};

/// A 16 GB stick's worth of 512-byte blocks.
const STICK_BLOCKS: u64 = 31_266_816;

/// The common stick layout: boot code zeroed, one FAT32-LBA (`0x0C`) entry
/// at LBA 2048 with 31_264_768 sectors (`0x01DD_1000`), not bootable.
fn hand_laid_stick_mbr() -> [u8; 512] {
    let mut s = [0u8; 512];
    // Entry 1 at 446 (0x1BE): status, CHS first (3), type, CHS last (3),
    // LBA first (LE u32), sector count (LE u32).
    let e1: [u8; 16] = [
        0x00, // not bootable
        0x20, 0x21, 0x00, // CHS first (ignored)
        0x0c, // FAT32 LBA
        0xfe, 0xff, 0xff, // CHS last (ignored)
        0x00, 0x08, 0x00, 0x00, // first LBA = 0x0000_0800 = 2048
        0x00, 0x10, 0xdd, 0x01, // count = 0x01DD_1000 = 31_264_768
    ];
    s[446..462].copy_from_slice(&e1);
    s[510] = 0x55;
    s[511] = 0xaa;
    s
}

/// A FAT32 boot sector at LBA 0 (a superfloppy), the fields `mformat -F`
/// writes for a 64 MiB volume: `EB 58 90`, "MTOO4043", 512 bytes/sector,
/// 1 sector/cluster, 32 reserved, 2 FATs, total32 = 131072.
fn hand_laid_superfloppy() -> [u8; 512] {
    let mut s = [0u8; 512];
    s[0..3].copy_from_slice(&[0xeb, 0x58, 0x90]);
    s[3..11].copy_from_slice(b"MTOO4043");
    s[11..13].copy_from_slice(&[0x00, 0x02]); // 512
    s[13] = 0x01; // sectors per cluster
    s[14..16].copy_from_slice(&[0x20, 0x00]); // 32 reserved
    s[16] = 0x02; // FATs
    s[21] = 0xf8; // media
    s[32..36].copy_from_slice(&[0x00, 0x00, 0x02, 0x00]); // 131072
    // A boot sector's tail is code, which can look like a table entry with
    // a non-zero type: the BPB check must win before the table is read.
    s[446 + 4] = 0x83;
    s[510] = 0x55;
    s[511] = 0xaa;
    s
}

fn put_entry(s: &mut [u8; 512], slot: usize, boot: u8, ty: u8, first: u32, count: u32) {
    let at = 446 + slot * 16;
    s[at] = boot;
    s[at + 4] = ty;
    s[at + 8..at + 12].copy_from_slice(&first.to_le_bytes());
    s[at + 12..at + 16].copy_from_slice(&count.to_le_bytes());
}

#[test]
fn hand_laid_stick_mbr_decodes_to_its_bytes() {
    let s = hand_laid_stick_mbr();
    let want = Entry {
        index: 1,
        boot_flag: 0,
        part_type: 0x0c,
        first_lba: 2048,
        lba_count: 31_264_768,
    };
    match classify(&s, STICK_BLOCKS) {
        Sector0::Table(slots) => {
            assert_eq!(slots[0], Some(Ok(want)));
            assert_eq!(slots[1..], [None, None, None]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(partition(&s, STICK_BLOCKS, 1, &[]), Some(want));
    assert_eq!(partition(&s, STICK_BLOCKS, 2, &[]), None);
    assert_eq!(partition(&s, STICK_BLOCKS, 0, &[]), None);
    assert_eq!(partition(&s, STICK_BLOCKS, 5, &[]), None);
}

#[test]
fn partition_ending_exactly_at_the_last_block_is_believed() {
    let s = hand_laid_stick_mbr();
    // 2048 + 31_264_768 = 31_266_816: ends on the device's end, inclusive.
    assert!(partition(&s, 2048 + 31_264_768, 1, &[]).is_some());
    // One block short of a device: past the end.
    match classify(&s, 2048 + 31_264_768 - 1) {
        Sector0::Table(slots) => assert_eq!(slots[0].unwrap().unwrap_err().1, Rejected::PastEnd),
        other => panic!("{other:?}"),
    }
}

#[test]
fn superfloppy_is_not_read_as_a_table() {
    let s = hand_laid_superfloppy();
    assert!(is_fat_boot_sector(&s));
    assert_eq!(classify(&s, 131_072), Sector0::Superfloppy);
    assert_eq!(partition(&s, 131_072, 1, &[]), None);
}

#[test]
fn mbr_is_not_read_as_a_boot_sector() {
    assert!(!is_fat_boot_sector(&hand_laid_stick_mbr()));
    // Windows-style boot code (`33 C0 8E D0`, xor ax,ax; mov ss,ax) is no jump.
    let mut s = hand_laid_stick_mbr();
    s[0..4].copy_from_slice(&[0x33, 0xc0, 0x8e, 0xd0]);
    assert!(matches!(classify(&s, STICK_BLOCKS), Sector0::Table(_)));
}

#[test]
fn bpb_sanity_fields_each_matter() {
    let base = hand_laid_superfloppy();
    let cases: [(usize, &[u8]); 6] = [
        (0, &[0x00]),        // no jump
        (11, &[0x00, 0x03]), // 768 bytes/sector: not a power of two
        (11, &[0x00, 0x20]), // 8192: too large
        (13, &[0x03]),       // 3 sectors/cluster
        (14, &[0x00, 0x00]), // no reserved sectors
        (16, &[0x03]),       // 3 FATs
    ];
    for (at, bytes) in cases {
        let mut s = base;
        s[at..at + bytes.len()].copy_from_slice(bytes);
        assert!(!is_fat_boot_sector(&s), "field at {at}");
    }
    let mut s = base;
    s[32..36].copy_from_slice(&[0; 4]); // no total sectors
    assert!(!is_fat_boot_sector(&s));
}

#[test]
fn no_signature_is_blank() {
    let mut s = hand_laid_stick_mbr();
    s[511] = 0x00;
    assert_eq!(classify(&s, STICK_BLOCKS), Sector0::Blank);
    assert_eq!(classify(&[0u8; 512], STICK_BLOCKS), Sector0::Blank);
    assert_eq!(classify(&[0u8; 100], STICK_BLOCKS), Sector0::Blank);
}

#[test]
fn protective_mbr_defers_to_gpt() {
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x00, 0xee, 1, 0xffff_ffff);
    s[510] = 0x55;
    s[511] = 0xaa;
    assert_eq!(classify(&s, STICK_BLOCKS), Sector0::Protective);
    assert_eq!(partition(&s, STICK_BLOCKS, 1, &[]), None);
}

#[test]
fn bad_entries_are_rejected_and_never_mounted() {
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x80, 0x0c, 2048, 4096); // FAT, overlaps entry 3
    put_entry(&mut s, 1, 0x00, 0x83, 8192, 4096); // Linux: not FAT
    put_entry(&mut s, 2, 0x00, 0x0e, 4096, 4096); // overlaps entry 1
    put_entry(&mut s, 3, 0x12, 0x06, 16384, 16); // bad boot flag
    s[510] = 0x55;
    s[511] = 0xaa;
    let Sector0::Table(slots) = classify(&s, 65536) else {
        panic!()
    };
    // Overlap is symmetric: neither of two overlapping entries is believed.
    assert_eq!(slots[0].unwrap().unwrap_err().1, Rejected::Overlaps);
    assert_eq!(slots[1].unwrap().unwrap_err().1, Rejected::NotFat);
    assert_eq!(slots[2].unwrap().unwrap_err().1, Rejected::Overlaps);
    assert_eq!(slots[3].unwrap().unwrap_err().1, Rejected::BadBootFlag);
    for m in 1..=4 {
        assert_eq!(partition(&s, 65536, m, &[]), None, "p{m}");
    }
}

#[test]
fn degenerate_entries_are_rejected() {
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x00, 0x0c, 0, 4096); // over the table
    put_entry(&mut s, 1, 0x00, 0x0c, 2048, 0); // empty
    s[510] = 0x55;
    s[511] = 0xaa;
    let Sector0::Table(slots) = classify(&s, 65536) else {
        panic!()
    };
    assert_eq!(slots[0].unwrap().unwrap_err().1, Rejected::Degenerate);
    assert_eq!(slots[1].unwrap().unwrap_err().1, Rejected::Degenerate);
}

/// Chad's #17 review: every entry counts against every other, whatever its
/// type or verdict. His input: slot 1 FAT32 LBA at 2048+4096 and slot 2 Linux
/// at 4096+4096 on a 65536-block device; slot 1 used to mount.
#[test]
fn a_fat_entry_overlapping_any_entry_is_refused() {
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x00, 0x0c, 2048, 4096);
    put_entry(&mut s, 1, 0x00, 0x83, 4096, 4096);
    s[510] = 0x55;
    s[511] = 0xaa;
    // The byte oracle: the two entries as laid out.
    assert_eq!(
        s[446..478],
        [
            0x00, 0, 0, 0, 0x0c, 0, 0, 0, 0x00, 0x08, 0, 0, 0x00, 0x10, 0, 0, //
            0x00, 0, 0, 0, 0x83, 0, 0, 0, 0x00, 0x10, 0, 0, 0x00, 0x10, 0, 0,
        ]
    );
    let Sector0::Table(slots) = classify(&s, 65536) else {
        panic!()
    };
    assert_eq!(slots[0].unwrap().unwrap_err().1, Rejected::Overlaps);
    assert_eq!(partition(&s, 65536, 1, &[]), None);
    // A past-end entry still claims its space: a FAT entry under it is
    // refused too (nothing says which one is right).
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x00, 0x0c, 2048, 1_000_000);
    put_entry(&mut s, 1, 0x00, 0x0c, 2048, 4096);
    s[510] = 0x55;
    s[511] = 0xaa;
    assert_eq!(partition(&s, 65536, 1, &[]), None);
    assert_eq!(partition(&s, 65536, 2, &[]), None);
    // An extended container overlapping a FAT primary refuses it; the same
    // FAT primary clear of it is believed.
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x00, 0x0c, 2048, 4096);
    put_entry(&mut s, 1, 0x00, 0x0f, 6000, 8192);
    s[510] = 0x55;
    s[511] = 0xaa;
    assert_eq!(partition(&s, 65536, 1, &[]), None);
    put_entry(&mut s, 1, 0x00, 0x0f, 6144, 8192);
    assert!(partition(&s, 65536, 1, &[]).is_some());
    // A logical partition (from the chain) under a FAT primary refuses it.
    assert_eq!(partition(&s, 65536, 1, &[(5000, 5100)]), None);
}

/// An EBR: entry 0 the logical partition (relative to this EBR), entry 1
/// the next EBR (relative to the container start), `0x55AA`.
fn ebr(logical: (u32, u32), next: Option<u32>) -> [u8; 512] {
    let mut s = [0u8; 512];
    put_entry(&mut s, 0, 0x00, 0x83, logical.0, logical.1);
    if let Some(next) = next {
        put_entry(&mut s, 1, 0x00, 0x05, next, 1);
    }
    s[510] = 0x55;
    s[511] = 0xaa;
    s
}

/// Chad's #17 review: the extended chain is walked, bounded. A container
/// at 10000+20000 with EBRs at 10000 and 12000.
#[test]
fn the_extended_chain_walk_is_bounded() {
    let container = (10_000u64, 20_000u64);
    let mut out = [(0u64, 0u64); MAX_LOGICAL];
    // A proper two-link chain: logical partitions at 10063+100 and 12063+200.
    let mut read = |lba: u64| match lba {
        10_000 => Some(ebr((63, 100), Some(2000))),
        12_000 => Some(ebr((63, 200), None)),
        _ => None,
    };
    assert_eq!(extended_chain(container, &mut read, &mut out), (2, None));
    assert_eq!(&out[..2], &[(10_063, 10_163), (12_063, 12_263)]);
    // A loop (the second EBR points back at the first) stops at once.
    let mut read = |lba: u64| match lba {
        10_000 => Some(ebr((63, 100), Some(2000))),
        12_000 => Some(ebr((63, 200), Some(0))),
        _ => None,
    };
    assert_eq!(
        extended_chain(container, &mut read, &mut out),
        (2, Some(ChainError::Loop))
    );
    // A self-link stops too.
    let mut read = |_| Some(ebr((63, 100), Some(0)));
    assert_eq!(
        extended_chain(container, &mut read, &mut out),
        (1, Some(ChainError::Loop))
    );
    // A link past the container.
    let mut read = |lba: u64| (lba == 10_000).then(|| ebr((63, 100), Some(25_000)));
    assert_eq!(
        extended_chain(container, &mut read, &mut out),
        (1, Some(ChainError::OutsideContainer))
    );
    // An endless forward chain (each EBR one block on) stops at MAX_LOGICAL.
    let mut reads = 0;
    let mut read = |_| {
        reads += 1;
        Some(ebr((1, 1), Some(reads as u32)))
    };
    assert_eq!(
        extended_chain(container, &mut read, &mut out),
        (MAX_LOGICAL, Some(ChainError::TooLong))
    );
    // An unreadable or unsigned EBR.
    let mut read = |_| None;
    assert_eq!(
        extended_chain(container, &mut read, &mut out),
        (0, Some(ChainError::BadEbr))
    );
    let mut read = |_| Some([0u8; 512]);
    assert_eq!(
        extended_chain(container, &mut read, &mut out),
        (0, Some(ChainError::BadEbr))
    );
}

#[test]
fn sources_parse() {
    assert_eq!(parse_source("device1"), Some((1, None)));
    assert_eq!(parse_source("device12:p1"), Some((12, Some(1))));
    assert_eq!(parse_source("device0:p4"), Some((0, Some(4))));
    for bad in [
        "",
        "device",
        "device:p1",
        "device1:p",
        "device1:p0",
        "device1:p5",
        "device1:1",
        "device1:p1x",
        "dev1",
        "device-1",
        "device1:p01x",
        "device99999999999999999999999",
    ] {
        assert_eq!(parse_source(bad), None, "{bad:?}");
    }
}
