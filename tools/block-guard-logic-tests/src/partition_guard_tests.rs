// SPDX-License-Identifier: MPL-2.0
//! OT-87 R3: the guard's partition rule (`write_guard_logic::partition_claims`,
//! THINK §4.2), relax-and-re-prove. On the §4.4 oracle disk, exactly
//! partition 2 arms, and only when all five conditions hold. A table whose
//! entries overlap doesn't validate, so it arms nothing (Muse, item 5).

use super::gpt_logic::{self, LOGANSOS_NEOFS_TYPE_GUID, Layout, gpt_layout};
use super::gpt_oracle::{DISK_GUID, ESP_GUID, NEOFS_GUID};
use super::gpt_tests::Sparse;
use super::write_guard_logic::{Extent, WriteMarker, judge, judge_with_partitions, seal};

const GIB4: u64 = 4 * 1024 * 1024 * 1024;
const BLOCKS: u64 = 916_992;

/// A NEOFSv1 superblock laid out by hand (as `lib.rs`'s): magic, version 1,
/// log2 block size 12, type version 1, total_blocks at 16, BLAKE3 seal.
fn superblock(total_blocks: u64) -> [u8; 512] {
    let mut sb = [0u8; 512];
    sb[..8].copy_from_slice(b"NEOFSv1\0");
    sb[8..10].copy_from_slice(&1u16.to_le_bytes());
    sb[10..12].copy_from_slice(&12u16.to_le_bytes());
    sb[12] = 1;
    sb[16..24].copy_from_slice(&total_blocks.to_le_bytes());
    let h = seal(&sb[..480]);
    sb[480..].copy_from_slice(&h);
    sb
}

fn layout(s: u64) -> Layout {
    gpt_layout(s, GIB4 / s - 1, DISK_GUID, ESP_GUID, NEOFS_GUID, false).unwrap()
}

/// Recompute CRC-E and both header CRCs after editing `entries`.
fn reseal(l: &mut Layout) {
    let crc_e = gpt_logic::crc32(&l.entries);
    for h in [&mut l.primary_header, &mut l.backup_header] {
        h[88..92].copy_from_slice(&crc_e.to_le_bytes());
        h[16..20].fill(0);
        let c = gpt_logic::crc32(&h[..92]);
        h[16..20].copy_from_slice(&c.to_le_bytes());
    }
}

fn disk(l: &Layout, sb_blocks: Option<u64>) -> Sparse {
    let mut img = Sparse::with_layout(GIB4, l);
    if let Some(blocks) = sb_blocks {
        img.put(l.neofs.0 * l.sector_size, &superblock(blocks));
    }
    img
}

fn partition_extent(l: &Layout) -> Extent {
    Extent {
        start: l.neofs.0 * l.sector_size,
        end: (l.neofs.1 + 1) * l.sector_size,
    }
}

/// The oracle disk with a valid superblock: exactly partition 2 arms, at both
/// sector sizes. Without the rule (`judge`), nothing arms, as today.
#[test]
fn oracle_disk_arms_partition_2_only() {
    for s in [512, 4096] {
        let l = layout(s);
        assert_eq!(l.neofs_total_blocks, BLOCKS);
        let img = disk(&l, Some(BLOCKS));
        let today = judge(&img);
        assert!(today.claims.is_empty(), "S={s}: {today:?}");
        assert_eq!(today.table, "gpt");
        let arming = judge_with_partitions(&img);
        let p = partition_extent(&l);
        assert_eq!(arming.claims.len(), 1, "S={s}: {arming:?}");
        assert_eq!(arming.claims[0].marker, WriteMarker::NeoFs);
        assert_eq!(arming.claims[0].extent, p);
        // Inside, and exactly at both ends.
        assert!(arming.allows(p.start, p.start + s));
        assert!(arming.allows(p.end - s, p.end));
        assert!(arming.allows(p.start, p.end));
        // One sector either side, the ESP, LBA 0, both headers, both arrays.
        assert!(!arming.allows(p.start - s, p.start));
        assert!(!arming.allows(p.end, p.end + s));
        assert!(!arming.allows(p.end - s, p.end + 1));
        assert!(!arming.allows(l.esp.0 * s, l.esp.0 * s + s));
        for lba in [0, 1, 2, l.backup_entries_lba, l.last_lba] {
            assert!(!arming.allows(lba * s, lba * s + s), "S={s} lba {lba}");
        }
    }
}

/// Only the type GUID right (no superblock), or a superblock with a broken
/// seal: nothing arms.
#[test]
fn type_guid_alone_arms_nothing() {
    for s in [512, 4096] {
        let l = layout(s);
        assert!(judge_with_partitions(&disk(&l, None)).claims.is_empty());
        let mut img = disk(&l, Some(BLOCKS));
        let mut sb = superblock(BLOCKS);
        sb[200] ^= 1; // after the seal was taken
        img.put(l.neofs.0 * s, &sb);
        assert!(judge_with_partitions(&img).claims.is_empty(), "S={s}");
    }
}

/// Only the superblock right: the entry is typed as something else (Linux
/// filesystem data, 0FC63DAF-8483-4772-8E79-3D69D8477DE4), re-sealed.
#[test]
fn superblock_alone_arms_nothing() {
    const LINUX_FS: [u8; 16] = [
        0xaf, 0x3d, 0xc6, 0x0f, 0x83, 0x84, 0x72, 0x47, 0x8e, 0x79, 0x3d, 0x69, 0xd8, 0x47, 0x7d,
        0xe4,
    ];
    for s in [512, 4096] {
        let mut l = layout(s);
        l.entries[128..144].copy_from_slice(&LINUX_FS);
        reseal(&mut l);
        let img = disk(&l, Some(BLOCKS));
        assert!(gpt_logic::read(&img).is_some(), "still a valid GPT");
        let arming = judge_with_partitions(&img);
        assert!(arming.claims.is_empty(), "S={s}: {arming:?}");
        // And the ESP entry, our superblock written at its start, arms nothing.
        let mut img = disk(&layout(s), None);
        img.put(l.esp.0 * s, &superblock(1000));
        assert!(judge_with_partitions(&img).claims.is_empty());
    }
}

/// A superblock claiming past its partition arms nothing; exactly the
/// partition, or less, arms the whole partition.
#[test]
fn superblock_past_its_partition_arms_nothing() {
    for s in [512, 4096] {
        let l = layout(s);
        let p = partition_extent(&l);
        assert!(
            judge_with_partitions(&disk(&l, Some(BLOCKS + 1)))
                .claims
                .is_empty()
        );
        assert!(
            judge_with_partitions(&disk(&l, Some(u64::MAX / 4096)))
                .claims
                .is_empty()
        );
        for blocks in [BLOCKS, 1000] {
            let arming = judge_with_partitions(&disk(&l, Some(blocks)));
            assert_eq!(arming.claims.len(), 1);
            assert_eq!(arming.claims[0].extent, p, "the partition, not the volume");
        }
    }
}

/// A partition overlapping the table's own sectors or the backup array makes
/// the GPT invalid, so nothing arms and the disk is never written.
#[test]
fn partition_over_the_tables_arms_nothing() {
    for s in [512, 4096] {
        let base = layout(s);
        let a = 16384 / s;
        for (first, last) in [
            (base.neofs.0, base.backup_entries_lba), // into the backup array
            (base.neofs.0, base.last_lba),           // to the backup header
            (1, base.neofs.1),                       // over the primary header
            (2, base.neofs.1),                       // over the primary array
            (1 + a, base.neofs.1),                   // the last array sector
        ] {
            let mut l = base.clone();
            l.entries[128 + 32..128 + 40].copy_from_slice(&first.to_le_bytes());
            l.entries[128 + 40..128 + 48].copy_from_slice(&last.to_le_bytes());
            reseal(&mut l);
            let mut img = Sparse::with_layout(GIB4, &l);
            img.put(first * s, &superblock(1000));
            assert!(gpt_logic::read(&img).is_none(), "S={s} {first}..{last}");
            let arming = judge_with_partitions(&img);
            assert!(
                arming.claims.is_empty(),
                "S={s} {first}..{last}: {arming:?}"
            );
            assert_eq!(arming.table, "opaque-gpt-invalid");
        }
    }
}

/// A stale CRC (the entry moved without re-sealing) is not believed: the
/// whole disk counts as allocated, as today.
#[test]
fn unsealed_edit_arms_nothing() {
    let mut l = layout(512);
    l.entries[128 + 32] ^= 1;
    let img = disk(&l, Some(BLOCKS));
    let arming = judge_with_partitions(&img);
    assert!(arming.claims.is_empty());
    assert_eq!(arming.table, "opaque-gpt-invalid");
}

/// The backup alone (primary wiped) still validates, and arms partition 2.
#[test]
fn backup_alone_arms_partition_2() {
    let l = layout(512);
    let mut img = disk(&l, Some(BLOCKS));
    img.put(512, &[0u8; 512]);
    let arming = judge_with_partitions(&img);
    assert_eq!(arming.table, "gpt-backup");
    assert_eq!(arming.claims.len(), 1);
    assert_eq!(arming.claims[0].extent, partition_extent(&l));
}

/// Two LogansOS partitions, each with its own superblock: each arms its own
/// span, and nothing between them.
#[test]
fn each_partition_arms_its_own_span() {
    let mut l = layout(512);
    let (first, last) = l.neofs;
    let mid = first + (last + 1 - first) / 2;
    let e3 = l.entries[128..256].to_vec();
    l.entries[256..384].copy_from_slice(&e3);
    l.entries[256 + 16] ^= 0x55; // its own unique GUID
    l.entries[128 + 40..128 + 48].copy_from_slice(&(mid - 2049).to_le_bytes());
    l.entries[256 + 32..256 + 40].copy_from_slice(&mid.to_le_bytes());
    reseal(&mut l);
    let mut img = Sparse::with_layout(GIB4, &l);
    img.put(first * 512, &superblock(1000));
    img.put(mid * 512, &superblock(1000));
    let arming = judge_with_partitions(&img);
    assert_eq!(arming.claims.len(), 2, "{arming:?}");
    assert!(!arming.allows((mid - 2048) * 512, mid * 512), "the gap");
    assert!(
        !arming.allows(first * 512, (last + 1) * 512),
        "no single claim spans both"
    );
}

/// An INCOMPLETE partition (attribute bit 48, an interrupted install) arms
/// nothing, even with a valid superblock; the same disk with the bit clear
/// arms partition 2 (the fifth condition, PM, 2026-10-27).
#[test]
fn incomplete_partition_arms_nothing() {
    for s in [512, 4096] {
        let busy = gpt_layout(s, GIB4 / s - 1, DISK_GUID, ESP_GUID, NEOFS_GUID, true).unwrap();
        let img = disk(&busy, Some(BLOCKS));
        assert_eq!(
            gpt_logic::read(&img).unwrap().entries[1].attributes,
            gpt_logic::ATTR_INCOMPLETE
        );
        let arming = judge_with_partitions(&img);
        assert!(arming.claims.is_empty(), "S={s}: {arming:?}");
        assert_eq!(arming.table, "gpt");
        let done = layout(s);
        let arming = judge_with_partitions(&disk(&done, Some(BLOCKS)));
        assert_eq!(arming.claims.len(), 1, "S={s}");
        assert_eq!(arming.claims[0].extent, partition_extent(&done));
    }
}

/// The type GUID is the one minted for R3, as GPT stores it.
#[test]
fn logansos_type_guid_bytes() {
    assert_eq!(
        LOGANSOS_NEOFS_TYPE_GUID,
        uuid::uuid!("eaa7a9d0-75c1-4385-9664-b2a74fc2622d").to_bytes_le()
    );
}

/// The partition view's bounds: exact at the first and last LBA, a crossing
/// or empty request refused, never truncated.
#[test]
fn partition_view_bounds() {
    use gpt_logic::partition_lba;
    let (first, count) = (1_050_624u64, 7_335_936u64);
    assert_eq!(partition_lba(first, count, 0, 1), Some(first));
    assert_eq!(
        partition_lba(first, count, count - 1, 1),
        Some(first + count - 1)
    );
    assert_eq!(partition_lba(first, count, 0, count), Some(first));
    assert_eq!(
        partition_lba(first, count, count - 8, 8),
        Some(first + count - 8)
    );
    assert_eq!(
        partition_lba(first, count, count - 7, 8),
        None,
        "crosses the end"
    );
    assert_eq!(partition_lba(first, count, count, 1), None);
    assert_eq!(partition_lba(first, count, 0, count + 1), None);
    assert_eq!(partition_lba(first, count, 5, 0), None, "empty");
    assert_eq!(partition_lba(first, count, u64::MAX, 2), None, "overflow");
    assert_eq!(partition_lba(u64::MAX, 4, 1, 1), None, "overflow");
}

/// Set entry `i` (0-based) to `[first, last]`.
fn set_span(l: &mut Layout, i: usize, first: u64, last: u64) {
    let e = &mut l.entries[i * 128..(i + 1) * 128];
    e[32..40].copy_from_slice(&first.to_le_bytes());
    e[40..48].copy_from_slice(&last.to_le_bytes());
}

/// Muse's layout (R3 review, item 5), exactly: ESP and LogansOS entries both
/// `[2048, 1050623]`, both copies re-sealed, a sealed NEOFSv1 superblock at
/// 2048 that fits the span. The table doesn't validate, so nothing arms on
/// either path, and a write to the ESP's first sector is refused. The same
/// disk without the ESP entry arms that span: the superblock alone would.
#[test]
fn muse_overlap_layout_arms_nothing() {
    let mut l = layout(512);
    assert_eq!(l.esp, (2048, 1_050_623));
    set_span(&mut l, 1, 2048, 1_050_623);
    reseal(&mut l);
    let mut img = Sparse::with_layout(GIB4, &l);
    // 1,048,576 sectors of 512 bytes: 131,072 blocks fill the span exactly.
    img.put(2048 * 512, &superblock(131_072));
    for arming in [judge_with_partitions(&img), judge(&img)] {
        assert!(arming.claims.is_empty(), "{arming:?}");
        assert_eq!(arming.table, "opaque-gpt-invalid");
        assert!(!arming.allows(2048 * 512, 2049 * 512));
    }
    assert!(gpt_logic::read(&img).is_none());

    let mut alone = l.clone();
    alone.entries[..128].fill(0);
    reseal(&mut alone);
    let mut img = Sparse::with_layout(GIB4, &alone);
    img.put(2048 * 512, &superblock(131_072));
    let arming = judge_with_partitions(&img);
    assert_eq!(arming.claims.len(), 1, "{arming:?}");
    assert_eq!(
        arming.claims[0].extent,
        Extent {
            start: 2048 * 512,
            end: 1_050_624 * 512
        }
    );
}

/// One shared sector at either end is an overlap: nothing arms. Touching
/// (the oracle's own `esp.last + 1 == neofs.first`) still arms.
#[test]
fn overlap_by_one_sector_arms_nothing_touching_arms() {
    for s in [512, 4096] {
        let base = layout(s);
        let (esp, neofs) = (base.esp, base.neofs);
        assert_eq!(esp.1 + 1, neofs.0, "the oracle's entries touch");
        assert_eq!(
            judge_with_partitions(&disk(&base, Some(BLOCKS)))
                .claims
                .len(),
            1
        );
        // Entry 2 starts on the ESP's last sector; or the ESP ends on entry
        // 2's first.
        for (i, first, last) in [(1, esp.1, neofs.1), (0, esp.0, neofs.0)] {
            let mut l = base.clone();
            set_span(&mut l, i, first, last);
            reseal(&mut l);
            let mut img = Sparse::with_layout(GIB4, &l);
            img.put(neofs.0 * s, &superblock(BLOCKS - 1));
            img.put(esp.1 * s, &superblock(BLOCKS - 1));
            let arming = judge_with_partitions(&img);
            assert!(arming.claims.is_empty(), "S={s} entry {i}: {arming:?}");
            assert_eq!(arming.table, "opaque-gpt-invalid");
        }
    }
}

/// An overlapping primary (re-sealed on its own) with a clean backup: the
/// primary doesn't validate, so the backup's table is the one judged, and
/// partition 2 arms from it.
#[test]
fn overlapping_primary_clean_backup_takes_the_backup() {
    let l = layout(512);
    let mut img = disk(&l, Some(BLOCKS));
    let mut bad = l.clone();
    set_span(&mut bad, 1, l.esp.1, l.neofs.1);
    reseal(&mut bad);
    img.put(512, &bad.primary_header);
    img.put(2 * 512, &bad.entries);
    let arming = judge_with_partitions(&img);
    assert_eq!(arming.table, "gpt-backup");
    assert_eq!(arming.claims.len(), 1, "{arming:?}");
    assert_eq!(arming.claims[0].extent, partition_extent(&l));
    assert_eq!(gpt_logic::read(&img).unwrap().header_lba, l.last_lba);
}

/// Set header field `at` (a little-endian u64) in `h` and re-seal its CRC.
fn set_header_u64(h: &mut [u8], at: usize, v: u64) {
    h[at..at + 8].copy_from_slice(&v.to_le_bytes());
    h[16..20].fill(0);
    let c = gpt_logic::crc32(&h[..92]);
    h[16..20].copy_from_slice(&c.to_le_bytes());
}

/// Janus's R3 review, item 7, exactly: a 4 GiB disk, S=512, a CRC-valid
/// primary with `alternate = last_usable = L`, its array at LBA 2, one
/// LogansOS entry over 2048..L, a sealed superblock at 2048 that fits. The
/// partition would cover the backup array and header. With the backup
/// wiped, and with the standard backup left in place (whose usable range
/// the entry exceeds), nothing arms and a write to L is refused.
#[test]
fn janus_layout_arms_nothing() {
    let base = layout(512);
    let last = base.last_lba;
    assert_eq!(last, 8_388_607);
    let mut l = base.clone();
    l.entries[..128].fill(0);
    set_span(&mut l, 1, 2048, last);
    reseal(&mut l);
    set_header_u64(&mut l.primary_header, 48, last);
    let blocks = (last + 1 - 2048) * 512 / 4096;
    for wipe_backup in [true, false] {
        let mut img = Sparse::with_layout(GIB4, &l);
        img.put(2048 * 512, &superblock(blocks));
        if wipe_backup {
            img.put(last * 512, &[0u8; 512]);
        }
        let arming = judge_with_partitions(&img);
        assert!(arming.claims.is_empty(), "wipe={wipe_backup}: {arming:?}");
        assert_eq!(arming.table, "opaque-gpt-invalid");
        assert!(!arming.allows(last * 512, (last + 1) * 512));
        assert!(!arming.allows((last - 32) * 512, (last - 31) * 512));
    }
}

/// Each copy must name the other, past its usable range: a primary whose
/// alternate is low, or sits on its own last usable LBA or the backup array,
/// and a backup whose alternate isn't LBA 1, don't validate. With the other
/// copy wiped, nothing arms.
#[test]
fn a_wrong_alternate_lba_refuses_the_copy() {
    let l = layout(512);
    let last = l.last_lba;
    for alternate in [5, l.neofs.1, last - 1] {
        let mut bad = l.clone();
        set_header_u64(&mut bad.primary_header, 32, alternate);
        let mut img = disk(&bad, Some(BLOCKS));
        img.put(last * 512, &[0u8; 512]);
        let arming = judge_with_partitions(&img);
        assert!(arming.claims.is_empty(), "alternate {alternate}");
        assert_eq!(arming.table, "opaque-gpt-invalid", "alternate {alternate}");
    }
    let mut bad = l.clone();
    set_header_u64(&mut bad.backup_header, 32, 2);
    let mut img = disk(&bad, Some(BLOCKS));
    img.put(512, &[0u8; 512]);
    assert!(judge_with_partitions(&img).claims.is_empty());
}

/// `last_usable` one sector into the backup array (it starts at L - 32 at
/// S=512): the copy doesn't validate. One sector below it does.
#[test]
fn last_usable_into_the_backup_array_refuses_the_copy() {
    let l = layout(512);
    let last = l.last_lba;
    let backup_array = last - 32;
    assert_eq!(l.backup_entries_lba, backup_array);
    let mut bad = l.clone();
    set_header_u64(&mut bad.primary_header, 48, backup_array);
    let mut img = disk(&bad, Some(BLOCKS));
    img.put(last * 512, &[0u8; 512]);
    assert!(gpt_logic::read(&img).is_none());
    assert!(judge_with_partitions(&img).claims.is_empty());
    let mut ok = l.clone();
    set_header_u64(&mut ok.primary_header, 48, backup_array - 1);
    let mut img = disk(&ok, Some(BLOCKS));
    img.put(last * 512, &[0u8; 512]);
    assert!(gpt_logic::read(&img).is_some());
    assert_eq!(judge_with_partitions(&img).claims.len(), 1);
}

/// The defence-in-depth spans for the oracle disk: the MBR and primary
/// header, the primary array, the backup array and header.
#[test]
fn metadata_spans_of_the_oracle_disk() {
    for s in [512, 4096] {
        let l = layout(s);
        let img = disk(&l, Some(BLOCKS));
        let gpt = gpt_logic::read(&img).unwrap();
        let a = (16 * 1024) / s;
        let spans = gpt_logic::metadata_lbas(&gpt, s, l.last_lba);
        assert_eq!(spans[0], (0, 1));
        assert_eq!(spans[1], (2, 1 + a));
        assert_eq!(spans[2], (2, 1 + a));
        assert_eq!(spans[3], (l.last_lba - a, l.last_lba));
        assert_eq!(spans[4], (l.last_lba - a, l.last_lba));
        // The NeoFS partition touches none of them.
        for (lo, hi) in spans {
            assert!(l.neofs.1 < lo || hi < l.neofs.0, "S={s} {lo}..{hi}");
        }
    }
}
