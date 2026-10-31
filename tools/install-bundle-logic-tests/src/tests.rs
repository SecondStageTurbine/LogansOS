// SPDX-License-Identifier: MPL-2.0
//! The `NEOINST1` byte oracle (CLAUDE.md, "Format/ABI specs require an
//! independent byte oracle"): a two-part bundle and one slot-map entry, laid
//! out field by field from the format table in `install_bundle_logic.rs`,
//! independently of the encoder. Only the BLAKE3 fields come from the
//! `blake3` crate.

use super::install_bundle_logic::*;

fn le32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

fn le64(v: u64) -> [u8; 8] {
    v.to_le_bytes()
}

fn padded(name: &str, len: usize) -> Vec<u8> {
    let mut v = name.as_bytes().to_vec();
    v.resize(len, 0);
    v
}

/// Part 0, a sparse image of 16 bytes: one run, `AB CD` at offset 4.
fn stored_sparse() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0x01, 0, 0, 0, 0, 0, 0, 0]); // run count 1
    v.extend_from_slice(&[0x04, 0, 0, 0, 0, 0, 0, 0]); // offset 4
    v.extend_from_slice(&[0x02, 0, 0, 0, 0, 0, 0, 0]); // length 2
    v.extend_from_slice(&[0xab, 0xcd]);
    assert_eq!(v.len(), 26);
    v
}

/// Part 1, a slot map with one entry: `slot/A/kernel` at sectors
/// 0x100..0x120 of a 512-byte-sector template, its directory entry the
/// fourth 32-byte entry (offset 0x60) of sector 0x40.
fn stored_slot_map() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0x00, 0x02, 0, 0]); // S = 512
    v.extend_from_slice(&[0x01, 0, 0, 0]); // one entry
    v.extend_from_slice(&[0; 8]); // reserved
    let entry_start = v.len();
    v.extend_from_slice(b"slot/A/kernel");
    v.extend_from_slice(&[0; 19]); // the path, NUL-padded to 32
    v.extend_from_slice(&[0x00, 0x01, 0, 0, 0, 0, 0, 0]); // first sector 0x100
    v.extend_from_slice(&[0x20, 0, 0, 0, 0, 0, 0, 0]); // 0x20 sectors
    v.extend_from_slice(&[0x40, 0, 0, 0, 0, 0, 0, 0]); // dirent sector 0x40
    v.extend_from_slice(&[0x60, 0, 0, 0]); // dirent offset 0x60
    v.extend_from_slice(&[0; 4]); // reserved
    assert_eq!(v.len() - entry_start, SLOT_ENTRY_BYTES);
    assert_eq!(v.len(), 80);
    v
}

/// The whole bundle, by hand: header, two part rows, then the parts.
/// Table end = 32 + 2 × 96 = 224 (0xe0); part 1 starts at 224 + 26 = 250
/// (0xfa); the bundle is 250 + 80 = 330 (0x14a) bytes.
fn oracle() -> Vec<u8> {
    let (a, m) = (stored_sparse(), stored_slot_map());
    let mut v = Vec::new();
    v.extend_from_slice(b"NEOINST1");
    v.extend_from_slice(&[0x01, 0, 0, 0]); // version 1
    v.extend_from_slice(&[0x02, 0, 0, 0]); // two parts
    v.extend_from_slice(&[0x4a, 0x01, 0, 0, 0, 0, 0, 0]); // 330 bytes
    v.extend_from_slice(&[0; 8]); // reserved
    // Row 0: "a", sparse, at 0xe0, 26 stored, 16 expanded.
    v.extend_from_slice(&padded("a", 32));
    v.extend_from_slice(&[0x01, 0, 0, 0, 0, 0, 0, 0]); // kind 1, reserved
    v.extend_from_slice(&[0xe0, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0x1a, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0x10, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(blake3::hash(&a).as_bytes());
    // Row 1: "m", slot map, at 0xfa, 80 stored, 80 expanded.
    v.extend_from_slice(&padded("m", 32));
    v.extend_from_slice(&[0x02, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0xfa, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0x50, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0x50, 0, 0, 0, 0, 0, 0, 0]);
    v.extend_from_slice(blake3::hash(&m).as_bytes());
    assert_eq!(v.len(), 224);
    v.extend_from_slice(&a);
    v.extend_from_slice(&m);
    assert_eq!(v.len(), 330);
    v
}

#[test]
fn encoder_matches_the_hand_laid_bundle() {
    let entry = SlotEntry {
        path: "slot/A/kernel".into(),
        first_sector: 0x100,
        sectors: 0x20,
        dirent_sector: 0x40,
        dirent_offset: 0x60,
    };
    let map = SlotMap {
        sector_size: 512,
        entries: vec![entry],
    };
    let m = encode_slot_map(&map).unwrap();
    assert_eq!(m, stored_slot_map());
    let mut image = [0u8; 16];
    image[4] = 0xab;
    image[5] = 0xcd;
    // A 2-byte block, so the run is exactly the two non-zero bytes.
    let a = encode_sparse(&image, 2);
    assert_eq!(a, stored_sparse());
    let bundle =
        encode_bundle(&[("a", KIND_SPARSE, 16, &a), ("m", KIND_SLOT_MAP, 80, &m)]).unwrap();
    assert_eq!(bundle, oracle());
}

#[test]
fn reader_parses_the_hand_laid_bundle() {
    let bundle = oracle();
    let parts = manifest(&bundle).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(
        (
            parts[0].name.as_str(),
            parts[0].kind,
            parts[0].offset,
            parts[0].length,
            parts[0].expanded
        ),
        ("a", KIND_SPARSE, 224, 26, 16)
    );
    assert_eq!(
        (
            parts[1].name.as_str(),
            parts[1].kind,
            parts[1].offset,
            parts[1].length,
            parts[1].expanded
        ),
        ("m", KIND_SLOT_MAP, 250, 80, 80)
    );
    let a = check_part(&bundle, &parts[0]).unwrap();
    let runs = sparse_runs(a, parts[0].expanded).unwrap();
    assert_eq!(
        runs,
        vec![Run {
            offset: 4,
            bytes: &[0xab, 0xcd]
        }]
    );
    let mut out = [0xffu8; 16];
    expand_into(&runs, 0, &mut out);
    assert_eq!(out, [0, 0, 0, 0, 0xab, 0xcd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    // A window that cuts the run in half.
    let mut half = [0xffu8; 3];
    expand_into(&runs, 5, &mut half);
    assert_eq!(half, [0xcd, 0, 0]);
    let m = check_part(&bundle, &parts[1]).unwrap();
    assert_eq!(
        slot_map(m).unwrap(),
        SlotMap {
            sector_size: 512,
            entries: vec![SlotEntry {
                path: "slot/A/kernel".into(),
                first_sector: 0x100,
                sectors: 0x20,
                dirent_sector: 0x40,
                dirent_offset: 0x60,
            }],
        }
    );
}

#[test]
fn header_and_table_damage_is_refused() {
    let good = oracle();
    let with = |at: usize, byte: u8| {
        let mut b = good.clone();
        b[at] = byte;
        b
    };
    assert_eq!(manifest(&good[..31]), Err(BundleError::Truncated));
    assert_eq!(manifest(&with(7, b'2')), Err(BundleError::BadMagic));
    assert_eq!(manifest(&with(8, 2)), Err(BundleError::BadVersion));
    assert_eq!(manifest(&with(16, 0x4b)), Err(BundleError::LengthMismatch));
    assert_eq!(manifest(&with(24, 1)), Err(BundleError::ReservedNotZero));
    // Part count 3: the table runs into the first part's bytes and its
    // third row isn't a valid name.
    assert!(manifest(&with(12, 3)).is_err());
    assert_eq!(manifest(&with(32, 0)), Err(BundleError::BadName));
    assert_eq!(manifest(&with(33, 0x01)), Err(BundleError::BadName)); // a byte after the NUL
    assert_eq!(manifest(&with(64, 3)), Err(BundleError::BadKind));
    assert_eq!(manifest(&with(68, 1)), Err(BundleError::ReservedNotZero));
    // Part 0 one byte later: a gap after the table.
    assert_eq!(manifest(&with(72, 0xe1)), Err(BundleError::BadPartRange));
    // Part 1 one byte shorter: a gap before the end.
    assert_eq!(
        manifest(&with(32 + 96 + 48, 0x4f)),
        Err(BundleError::BadPartRange)
    );
    // A slot map whose expanded length isn't its stored length.
    assert_eq!(
        manifest(&with(32 + 96 + 56, 0x51)),
        Err(BundleError::BadPartRange)
    );
    // Trailing bytes after the last part (length field updated to match).
    let mut long = good.clone();
    long.push(0);
    long[16] = 0x4b;
    assert_eq!(manifest(&long), Err(BundleError::BadPartRange));
}

#[test]
fn a_flipped_part_byte_fails_its_hash() {
    let mut bundle = oracle();
    bundle[224 + 24] ^= 1; // the first byte of the run's data
    let parts = manifest(&bundle).unwrap();
    assert_eq!(
        check_part(&bundle, &parts[0]),
        Err(BundleError::PartHashMismatch)
    );
    assert!(check_part(&bundle, &parts[1]).is_ok());
}

#[test]
fn bad_runs_are_refused() {
    let run = |offset: u64, len: u64| {
        let mut v = Vec::new();
        v.extend_from_slice(&le64(offset));
        v.extend_from_slice(&le64(len));
        v.extend(std::iter::repeat_n(0x11u8, len as usize));
        v
    };
    let image = |runs: &[Vec<u8>]| {
        let mut v = le64(runs.len() as u64).to_vec();
        for r in runs {
            v.extend_from_slice(r);
        }
        v
    };
    assert!(sparse_runs(&image(&[run(0, 4), run(4, 4)]), 8).is_ok()); // touching is fine
    assert_eq!(
        sparse_runs(&image(&[run(0, 4), run(3, 4)]), 8),
        Err(BundleError::BadRun)
    ); // overlap
    assert_eq!(
        sparse_runs(&image(&[run(4, 4), run(0, 4)]), 8),
        Err(BundleError::BadRun)
    ); // descending
    assert_eq!(
        sparse_runs(&image(&[run(2, 0)]), 8),
        Err(BundleError::BadRun)
    ); // empty
    assert_eq!(
        sparse_runs(&image(&[run(6, 4)]), 8),
        Err(BundleError::BadRun)
    ); // past the end
    assert_eq!(
        sparse_runs(&image(&[run(u64::MAX, 1)]), 8),
        Err(BundleError::BadRun)
    ); // wraps
    let mut short = image(&[run(0, 4)]);
    short.pop();
    assert_eq!(sparse_runs(&short, 8), Err(BundleError::Truncated));
    let mut extra = image(&[run(0, 4)]);
    extra.push(0);
    assert_eq!(sparse_runs(&extra, 8), Err(BundleError::BadRun));
}

#[test]
fn bad_slot_maps_are_refused() {
    let map = |s: u32, rows: &[(u64, u64, u64, u32)]| {
        let mut v = Vec::new();
        v.extend_from_slice(&le32(s));
        v.extend_from_slice(&le32(rows.len() as u32));
        v.extend_from_slice(&[0; 8]);
        for (i, (first, len, dirent, off)) in rows.iter().enumerate() {
            v.extend_from_slice(&padded(&format!("slot/A/f{i}"), 32));
            v.extend_from_slice(&le64(*first));
            v.extend_from_slice(&le64(*len));
            v.extend_from_slice(&le64(*dirent));
            v.extend_from_slice(&le32(*off));
            v.extend_from_slice(&[0; 4]);
        }
        v
    };
    assert!(slot_map(&map(4096, &[(100, 10, 5, 0), (110, 10, 5, 32)])).is_ok());
    assert_eq!(
        slot_map(&map(1024, &[(100, 10, 5, 0)])),
        Err(BundleError::BadSectorSize)
    );
    assert_eq!(
        slot_map(&map(512, &[(100, 0, 5, 0)])),
        Err(BundleError::BadSlotEntry)
    ); // empty extent
    assert_eq!(
        slot_map(&map(512, &[(100, 10, 5, 16)])),
        Err(BundleError::BadSlotEntry)
    ); // not 32-aligned
    assert_eq!(
        slot_map(&map(512, &[(100, 10, 5, 512)])),
        Err(BundleError::BadSlotEntry)
    ); // past the sector
    assert_eq!(
        slot_map(&map(512, &[(100, 10, 5, 0), (109, 10, 5, 32)])),
        Err(BundleError::BadSlotEntry) // overlapping extents
    );
    assert_eq!(
        slot_map(&map(512, &[(100, 10, 105, 0)])),
        Err(BundleError::BadSlotEntry) // the directory entry inside an extent
    );
    assert_eq!(
        slot_map(&map(512, &[(u64::MAX, 2, 5, 0)])),
        Err(BundleError::BadSlotEntry)
    );
    let mut short = map(512, &[(100, 10, 5, 0)]);
    short.pop();
    assert_eq!(slot_map(&short), Err(BundleError::Truncated));
}

#[test]
fn the_installer_manifest_needs_its_five_parts_in_order() {
    let sparse = encode_sparse(&[0u8; 8], 4);
    let map = encode_slot_map(&SlotMap {
        sector_size: 512,
        entries: Vec::new(),
    })
    .unwrap();
    let parts = |names: [&'static str; 5]| {
        let kinds = [
            KIND_SPARSE,
            KIND_SPARSE,
            KIND_SLOT_MAP,
            KIND_SLOT_MAP,
            KIND_SPARSE,
        ];
        let rows: Vec<(&str, u32, u64, &[u8])> = names
            .iter()
            .zip(kinds)
            .map(|(n, k)| {
                if k == KIND_SPARSE {
                    (*n, k, 8, sparse.as_slice())
                } else {
                    (*n, k, map.len() as u64, map.as_slice())
                }
            })
            .collect();
        encode_bundle(&rows).unwrap()
    };
    assert!(installer_manifest(&parts(PART_NAMES)).is_ok());
    let mut swapped = PART_NAMES;
    swapped.swap(0, 1);
    assert_eq!(
        installer_manifest(&parts(swapped)),
        Err(BundleError::WrongParts)
    );
    // Four parts.
    let four = encode_bundle(&[
        (ESP_512, KIND_SPARSE, 8, &sparse),
        (ESP_4096, KIND_SPARSE, 8, &sparse),
        (SLOTS_512, KIND_SLOT_MAP, map.len() as u64, &map),
        (SLOTS_4096, KIND_SLOT_MAP, map.len() as u64, &map),
    ])
    .unwrap();
    assert_eq!(installer_manifest(&four), Err(BundleError::WrongParts));
}

#[test]
fn sparse_encoding_round_trips_and_merges_adjacent_blocks() {
    let mut image = vec![0u8; 64 * 1024];
    image[0] = 1; // block 0
    image[4096 + 7] = 2; // block 1: merges with block 0
    image[5 * 4096 + 4095] = 3; // block 5: its own run
    image[64 * 1024 - 1] = 4; // the last block
    let stored = encode_sparse(&image, 4096);
    let runs = sparse_runs(&stored, image.len() as u64).unwrap();
    assert_eq!(
        runs.iter()
            .map(|r| (r.offset, r.bytes.len()))
            .collect::<Vec<_>>(),
        vec![(0, 8192), (5 * 4096, 4096), (15 * 4096, 4096)]
    );
    let mut out = vec![0xeeu8; image.len()];
    expand_into(&runs, 0, &mut out);
    assert_eq!(out, image);
    // A sector-sized window anywhere expands to the image's bytes.
    for at in (0..image.len()).step_by(512) {
        let mut sector = [0xeeu8; 512];
        expand_into(&runs, at as u64, &mut sector);
        assert_eq!(&sector[..], &image[at..at + 512]);
    }
    // An all-zero image has no runs.
    assert_eq!(encode_sparse(&[0u8; 8192], 4096), le64(0).to_vec());
}

#[test]
fn names_must_be_short_printable_ascii() {
    assert!(encode_bundle(&[("", KIND_SPARSE, 0, &[])]).is_none());
    assert!(encode_bundle(&[("has space", KIND_SPARSE, 0, &[])]).is_none());
    let long = "x".repeat(33);
    assert!(encode_bundle(&[(long.as_str(), KIND_SPARSE, 0, &[])]).is_none());
    let exact = "x".repeat(32);
    let b = encode_bundle(&[(exact.as_str(), KIND_SLOT_MAP, 0, &[])]).unwrap();
    assert_eq!(manifest(&b).unwrap()[0].name, exact);
}

/// Chad's R4.2 review, item 1, exactly: sector size 512, one entry
/// `slot/A/kernel` at (first 100, 10 sectors), directory sector 5, and
/// `dirent_offset = 0xffffffe0`: 32-aligned, and `+ 32` overflows `u32`.
/// Refused, in debug and release alike. Also an extent whose end overflows
/// after a valid entry (the overlap test used to add it unchecked), and a
/// part range that overflows.
#[test]
fn overflowing_slot_map_fields_are_refused() {
    let mut v = Vec::new();
    v.extend_from_slice(&le32(512));
    v.extend_from_slice(&le32(1));
    v.extend_from_slice(&[0; 8]);
    v.extend_from_slice(&padded("slot/A/kernel", 32));
    v.extend_from_slice(&le64(100));
    v.extend_from_slice(&le64(10));
    v.extend_from_slice(&le64(5));
    v.extend_from_slice(&le32(0xffff_ffe0));
    v.extend_from_slice(&[0; 4]);
    assert_eq!(v.len(), 16 + 64);
    assert_eq!(slot_map(&v), Err(BundleError::BadSlotEntry));

    let mut two = Vec::new();
    two.extend_from_slice(&le32(512));
    two.extend_from_slice(&le32(2));
    two.extend_from_slice(&[0; 8]);
    for (i, (first, len, off)) in [(100u64, 10u64, 0u32), (u64::MAX, 2, 32)]
        .iter()
        .enumerate()
    {
        two.extend_from_slice(&padded(&format!("slot/A/f{i}"), 32));
        two.extend_from_slice(&le64(*first));
        two.extend_from_slice(&le64(*len));
        two.extend_from_slice(&le64(5));
        two.extend_from_slice(&le32(*off));
        two.extend_from_slice(&[0; 4]);
    }
    assert_eq!(slot_map(&two), Err(BundleError::BadSlotEntry));

    let part = Part {
        name: "esp-512".into(),
        kind: KIND_SPARSE,
        offset: u64::MAX,
        length: 2,
        expanded: 2,
        blake3: [0; 32],
    };
    assert_eq!(
        check_part(&[0u8; 64], &part),
        Err(BundleError::BadPartRange)
    );
}
