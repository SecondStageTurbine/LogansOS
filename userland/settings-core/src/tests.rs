// SPDX-License-Identifier: MPL-2.0
//! The `NEOSET1` byte oracle and the store's KATs (OT-88 S1).
//!
//! The records below are hand-laid, independent of [`encode`] and
//! [`decode`]: a separate script writes each field from the layout table in
//! the crate docs. The checksums come from outside this crate: BLAKE3 (the
//! workspace's pinned `blake3` 1.8.5, in a standalone program) over each
//! record's bytes before the checksum.
//!
//! | Record | Generation | Entries (key, type, reserved, value) |
//! |---|---|---|
//! | `DEFAULTS_GEN1` | 1 | (1, 1, 0, 0) (2, 2, 0, 0) (3, 3, 0, 10) (4, 2, 0, 0) (5, 1, 0, 1) |
//! | `CHANGED_GEN2` | 2 | (1, 1, 0, 1) (2, 2, 0, 1) (3, 3, 0, 30) (4, 2, 0, 105) (5, 1, 0, 0) |
//! | `IDLE_OFF_ONLY_GEN5` | 5 | (3, 3, 0, 0) |
//! | `UNKNOWN_KEY` | 3 | (1, 1, 0, 0) (99, 1, 0, 0) |
//! | `ZONE_344` | 3 | (4, 2, 0, 344): one past the table's last zone id |
//! | `RTC_2` | 3 | (5, 1, 0, 2) |
//! | `WRONG_TYPE` | 3 | (1, 2, 0, 0) |
//! | `RESERVED_SET` | 3 | (1, 1, 1, 0) |
//! | `IDLE_7` | 3 | (3, 3, 0, 7) |
//! | `MOTION_3` | 3 | (2, 2, 0, 3) |
//! | `OUT_OF_ORDER` | 3 | (2, 2, 0, 0) (1, 1, 0, 0) |
//! | `DUPLICATE` | 3 | (1, 1, 0, 0) (1, 1, 0, 1) |
//! | `GEN0` | 0 | (1, 1, 0, 0) |
//!
//! The refused records carry valid checksums, so each check past the
//! checksum is reached.

use super::*;

// DEFAULTS_GEN1: 76 bytes, checksum d52a55082e67d1b1c93fb73b207eb9bf
#[rustfmt::skip]
const DEFAULTS_GEN1: [u8; 76] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, // 16
    0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x04, 0x00, 0x02, 0x00, // 32
    0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0xd5, 0x2a, 0x55, 0x08, // 48
    0x2e, 0x67, 0xd1, 0xb1, 0xc9, 0x3f, 0xb7, 0x3b, 0x20, 0x7e, 0xb9, 0xbf, // 64
];
// CHANGED_GEN2: 76 bytes, checksum 684f9cfc78be42bd7c0f48ef33eeddba
#[rustfmt::skip]
const CHANGED_GEN2: [u8; 76] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x05, 0x00, 0x02, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, // 16
    0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x1e, 0x00, 0x00, 0x00, 0x04, 0x00, 0x02, 0x00, // 32
    0x69, 0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x68, 0x4f, 0x9c, 0xfc, // 48
    0x78, 0xbe, 0x42, 0xbd, 0x7c, 0x0f, 0x48, 0xef, 0x33, 0xee, 0xdd, 0xba, // 64
];
// IDLE_OFF_ONLY_GEN5: 44 bytes, checksum 2af894f40af9c1d0418bb1d11a0713d8
#[rustfmt::skip]
const IDLE_OFF_ONLY_GEN5: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x05, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2a, 0xf8, 0x94, 0xf4, // 16
    0x0a, 0xf9, 0xc1, 0xd0, 0x41, 0x8b, 0xb1, 0xd1, 0x1a, 0x07, 0x13, 0xd8, // 32
];
// UNKNOWN_KEY: 52 bytes, checksum b73979a8df47625b9bda363e7242179e
#[rustfmt::skip]
const UNKNOWN_KEY: [u8; 52] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x63, 0x00, 0x01, 0x00, // 16
    0x00, 0x00, 0x00, 0x00, 0xb7, 0x39, 0x79, 0xa8, 0xdf, 0x47, 0x62, 0x5b, 0x9b, 0xda, 0x36, 0x3e, // 32
    0x72, 0x42, 0x17, 0x9e, // 48
];
// WRONG_TYPE: 44 bytes, checksum eb79dace93f7c0b5c4d14edf1591fa95
#[rustfmt::skip]
const WRONG_TYPE: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xeb, 0x79, 0xda, 0xce, // 16
    0x93, 0xf7, 0xc0, 0xb5, 0xc4, 0xd1, 0x4e, 0xdf, 0x15, 0x91, 0xfa, 0x95, // 32
];
// RESERVED_SET: 44 bytes, checksum 1b7c3cedc86d5ab9a4018ff227428285
#[rustfmt::skip]
const RESERVED_SET: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x1b, 0x7c, 0x3c, 0xed, // 16
    0xc8, 0x6d, 0x5a, 0xb9, 0xa4, 0x01, 0x8f, 0xf2, 0x27, 0x42, 0x82, 0x85, // 32
];
// IDLE_7: 44 bytes, checksum afe6749f805aae2c380bcfb1093b3f8e
#[rustfmt::skip]
const IDLE_7: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x07, 0x00, 0x00, 0x00, 0xaf, 0xe6, 0x74, 0x9f, // 16
    0x80, 0x5a, 0xae, 0x2c, 0x38, 0x0b, 0xcf, 0xb1, 0x09, 0x3b, 0x3f, 0x8e, // 32
];
// MOTION_3: 44 bytes, checksum f4ec893903e2505abd34167ca9816c68
#[rustfmt::skip]
const MOTION_3: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, 0xf4, 0xec, 0x89, 0x39, // 16
    0x03, 0xe2, 0x50, 0x5a, 0xbd, 0x34, 0x16, 0x7c, 0xa9, 0x81, 0x6c, 0x68, // 32
];
// OUT_OF_ORDER: 52 bytes, checksum 141e156eda933f7ccbcbbd56c123d53e
#[rustfmt::skip]
const OUT_OF_ORDER: [u8; 52] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, // 16
    0x00, 0x00, 0x00, 0x00, 0x14, 0x1e, 0x15, 0x6e, 0xda, 0x93, 0x3f, 0x7c, 0xcb, 0xcb, 0xbd, 0x56, // 32
    0xc1, 0x23, 0xd5, 0x3e, // 48
];
// DUPLICATE: 52 bytes, checksum 79a27e84cd102fce3aa7ceb8905df100
#[rustfmt::skip]
const DUPLICATE: [u8; 52] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, // 16
    0x01, 0x00, 0x00, 0x00, 0x79, 0xa2, 0x7e, 0x84, 0xcd, 0x10, 0x2f, 0xce, 0x3a, 0xa7, 0xce, 0xb8, // 32
    0x90, 0x5d, 0xf1, 0x00, // 48
];
// GEN0: 44 bytes, checksum cc1baa2adef5c1e1f5c54d607728ab57
#[rustfmt::skip]
const GEN0: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xcc, 0x1b, 0xaa, 0x2a, // 16
    0xde, 0xf5, 0xc1, 0xe1, 0xf5, 0xc5, 0x4d, 0x60, 0x77, 0x28, 0xab, 0x57, // 32
];

// ZONE_344: 44 bytes, checksum 852b73987eff864e7c11cd5334711774
#[rustfmt::skip]
const ZONE_344: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x02, 0x00, 0x58, 0x01, 0x00, 0x00, 0x85, 0x2b, 0x73, 0x98, // 16
    0x7e, 0xff, 0x86, 0x4e, 0x7c, 0x11, 0xcd, 0x53, 0x34, 0x71, 0x17, 0x74, // 32
];
// RTC_2: 44 bytes, checksum 44406347c785127cd7e047258e4812c8
#[rustfmt::skip]
const RTC_2: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x44, 0x40, 0x63, 0x47, // 16
    0xc7, 0x85, 0x12, 0x7c, 0xd7, 0xe0, 0x47, 0x25, 0x8e, 0x48, 0x12, 0xc8, // 32
];

const CHANGED: Settings = Settings {
    reduce_transparency: true,
    reduce_motion: Motion::Reduced,
    idle_lock_minutes: 30,
    // America/New_York (libneo's tz table id 105) on an RTC that holds UTC.
    time_zone: 105,
    rtc_local: false,
};

#[test]
fn encode_is_the_oracle() {
    assert_eq!(encode(&Settings::default(), 1), DEFAULTS_GEN1);
    assert_eq!(encode(&CHANGED, 2), CHANGED_GEN2);
}

#[test]
fn decode_reads_the_oracle() {
    assert_eq!(
        decode(&DEFAULTS_GEN1),
        Ok(Record {
            generation: 1,
            settings: Settings::default(),
        })
    );
    assert_eq!(
        decode(&CHANGED_GEN2),
        Ok(Record {
            generation: 2,
            settings: CHANGED,
        })
    );
}

/// A key the record lacks reads as its default.
#[test]
fn a_missing_key_is_its_default() {
    let r = decode(&IDLE_OFF_ONLY_GEN5).unwrap();
    assert_eq!(r.generation, 5);
    assert_eq!(
        r.settings,
        Settings {
            idle_lock_minutes: 0,
            ..Settings::default()
        }
    );
    assert_eq!(r.settings.idle_ns(), 0);
}

#[test]
fn every_bad_field_refuses_the_record() {
    assert_eq!(decode(&UNKNOWN_KEY), Err(RecordError::Key));
    assert_eq!(decode(&OUT_OF_ORDER), Err(RecordError::Key));
    assert_eq!(decode(&DUPLICATE), Err(RecordError::Key));
    assert_eq!(decode(&WRONG_TYPE), Err(RecordError::Type));
    assert_eq!(decode(&RESERVED_SET), Err(RecordError::Type));
    assert_eq!(decode(&IDLE_7), Err(RecordError::Value));
    assert_eq!(decode(&MOTION_3), Err(RecordError::Value));
    assert_eq!(decode(&GEN0), Err(RecordError::Generation));
    assert_eq!(decode(&ZONE_344), Err(RecordError::Value));
    assert_eq!(decode(&RTC_2), Err(RecordError::Value));
}

#[test]
fn the_header_and_checksum_are_checked() {
    assert_eq!(decode(&[]), Err(RecordError::Length));
    assert_eq!(decode(&DEFAULTS_GEN1[..75]), Err(RecordError::Length));
    let mut longer = DEFAULTS_GEN1.to_vec();
    longer.push(0);
    assert_eq!(decode(&longer), Err(RecordError::Length));
    let mut magic = DEFAULTS_GEN1;
    magic[0] ^= 1;
    assert_eq!(decode(&magic), Err(RecordError::Magic));
    let mut version = DEFAULTS_GEN1;
    version[8] = 2;
    assert_eq!(decode(&version), Err(RecordError::Version));
    let mut count = DEFAULTS_GEN1;
    count[10] = 17;
    assert_eq!(decode(&count), Err(RecordError::Count));
    // A count that doesn't match the length.
    let mut short_count = DEFAULTS_GEN1;
    short_count[10] = 2;
    assert_eq!(decode(&short_count), Err(RecordError::Length));
    // Every single-bit flip after the header's length fields is a checksum
    // failure (or, in the magic, version or count, refused earlier).
    for byte in 0..DEFAULTS_GEN1.len() {
        for bit in 0..8 {
            let mut torn = DEFAULTS_GEN1;
            torn[byte] ^= 1 << bit;
            assert!(decode(&torn).is_err(), "byte {byte} bit {bit}");
        }
    }
}

#[test]
fn the_key_table() {
    assert_eq!(KEYS.map(Key::id), [1, 2, 3, 4, 5]);
    assert_eq!(
        KEYS.map(Key::kind),
        [
            Type::Bool,
            Type::Choice,
            Type::Minutes,
            Type::Choice,
            Type::Bool
        ]
    );
    assert_eq!(KEYS.map(Key::default_value), [0, 0, 10, 0, 1]);
    assert_eq!(Key::from_id(0), None);
    assert_eq!(Key::from_id(6), None);
    // The zone is an id of libneo's generated table (2026e: 344 zones).
    assert_eq!(TZ_ZONE_COUNT, 344);
    assert!(Key::TimeZone.valid(0) && Key::TimeZone.valid(343) && !Key::TimeZone.valid(344));
    assert!(Key::RtcLocal.valid(1) && !Key::RtcLocal.valid(2));
    assert_eq!(IDLE_CHOICES, [0, 5, 10, 30]);
    for minutes in 0..=60 {
        assert_eq!(
            Key::IdleLockMinutes.valid(minutes),
            IDLE_CHOICES.contains(&minutes),
            "{minutes}"
        );
    }
    assert!(!Key::IdleLockMinutes.valid(u32::MAX));
    assert!(Key::ReduceTransparency.valid(1) && !Key::ReduceTransparency.valid(2));
    assert!(Key::ReduceMotion.valid(2) && !Key::ReduceMotion.valid(3));
}

/// `OP_SETTING`'s check: a bad key or value is refused and nothing changes.
#[test]
fn with_refuses_and_changes_nothing() {
    let s = Settings::default();
    assert_eq!(s.with(9, 0), Err(SetError::Key));
    assert_eq!(s.with(Key::IdleLockMinutes.id(), 7), Err(SetError::Value));
    assert_eq!(
        s.with(Key::ReduceTransparency.id(), 2),
        Err(SetError::Value)
    );
    assert_eq!(s, Settings::default());
    let off = s.with(Key::IdleLockMinutes.id(), 0).unwrap();
    assert_eq!(off.idle_ns(), 0);
    let five = s.with(Key::IdleLockMinutes.id(), 5).unwrap();
    assert_eq!(five.idle_ns(), 300_000_000_000);
    assert_eq!(Settings::default().idle_ns(), 600_000_000_000);
    let changed = s
        .with(1, 1)
        .and_then(|s| s.with(2, 1))
        .and_then(|s| s.with(3, 30))
        .and_then(|s| s.with(4, 105))
        .and_then(|s| s.with(5, 0))
        .unwrap();
    assert_eq!(changed, CHANGED);
    assert_eq!(s.with(4, 344), Err(SetError::Value));
    assert_eq!(s.with(2, 2).unwrap().reduce_motion, Motion::Off);
}

#[test]
fn migrate_reads_the_presence_files() {
    assert_eq!(migrate(false, false), Settings::default());
    let both = migrate(true, true);
    assert!(both.reduce_transparency);
    assert_eq!(both.reduce_motion, Motion::Reduced);
    assert_eq!(both.idle_lock_minutes, 10);
}

#[test]
fn select_takes_the_newer_valid_copy() {
    let a = Some(&DEFAULTS_GEN1[..]);
    let b = Some(&CHANGED_GEN2[..]);
    assert_eq!(select(a, b).unwrap().file, File::B);
    assert_eq!(select(b, a).unwrap().file, File::A);
    assert_eq!(select(a, a).unwrap().file, File::A, "equal: a");
    assert_eq!(select(a, Some(&IDLE_7[..])).unwrap().file, File::A);
    assert_eq!(select(None, b).unwrap().file, File::B);
    assert_eq!(select(None, None), None);
    assert_eq!(select(Some(&GEN0[..]), Some(&[][..])), None);
}

#[test]
fn plan_writes_the_other_file_at_the_next_generation() {
    let (file, bytes) = plan(None, &Settings::default()).unwrap();
    assert_eq!((file, bytes.as_slice()), (File::A, &DEFAULTS_GEN1[..]));
    let current = select(Some(&DEFAULTS_GEN1), None).unwrap();
    let (file, bytes) = plan(Some(&current), &CHANGED).unwrap();
    assert_eq!((file, bytes.as_slice()), (File::B, &CHANGED_GEN2[..]));
    let stuck = Current {
        file: File::A,
        record: Record {
            generation: u64::MAX,
            settings: Settings::default(),
        },
    };
    assert_eq!(plan(Some(&stuck), &CHANGED), None);
}

/// The torn-write model (OT-66 THINK §4.3's, for this record): a write goes
/// to the file not holding the record. Whatever a cut leaves in that file
/// (a prefix of the new bytes, or a prefix over the file's older bytes, or
/// any one bit wrong), the record read back is the old one, or, only if the
/// file holds the new bytes whole, the new one. Never neither, never a mix.
#[test]
fn every_cut_leaves_the_old_record_or_the_new() {
    // A holds generation 2; B holds generation 1; the change writes B.
    let current = select(Some(&CHANGED_GEN2), Some(&DEFAULTS_GEN1)).unwrap();
    assert_eq!(current.file, File::A);
    let next = Settings {
        idle_lock_minutes: 5,
        ..CHANGED
    };
    let (file, new) = plan(Some(&current), &next).unwrap();
    assert_eq!(file, File::B);
    let old = current.record;
    let check = |b: &[u8]| {
        let got = select(Some(&CHANGED_GEN2), Some(b)).unwrap();
        if b == new.as_slice() {
            assert_eq!(got.record.settings, next);
            assert_eq!(got.record.generation, 3);
        } else {
            assert_eq!(got.record, old, "{b:02x?}");
        }
    };
    let mut cuts = 0;
    for k in 0..=new.len() {
        // The file truncated to the first k new bytes.
        check(&new[..k]);
        // The first k new bytes over the file's older record.
        let mut over = DEFAULTS_GEN1.to_vec();
        over[..k].copy_from_slice(&new[..k]);
        check(&over);
        cuts += 2;
    }
    for byte in 0..new.len() {
        for bit in 0..8 {
            let mut torn = new.clone();
            torn[byte] ^= 1 << bit;
            check(&torn);
            cuts += 1;
        }
    }
    assert_eq!(cuts, 2 * 77 + 76 * 8);
    // The first record: a cut leaves no record, so the next boot reads the
    // defaults (and migrates again).
    let (_, first) = plan(None, &Settings::default()).unwrap();
    for k in 0..first.len() {
        assert_eq!(select(Some(&first[..k]), None), None);
    }
}

/// The PM's #222 bounce: only Reduce Transparency and Reduce Motion are a
/// client's to set; the idle lock is the WM's.
#[test]
fn only_the_display_keys_are_client_writable() {
    let writable: Vec<u16> = KEYS
        .iter()
        .filter(|k| k.client_writable())
        .map(|k| k.id())
        .collect();
    assert_eq!(writable, [1, 2]);
    assert!(!Key::IdleLockMinutes.client_writable());
    // The zone and the RTC mode are the WM's (its `timezone` and `rtc`
    // verbs) until the settings app's zone picker exists.
    assert!(!Key::TimeZone.client_writable() && !Key::RtcLocal.client_writable());
}

/// OT-100 R3 (THINK §6): until the persistence volume is encrypted, no
/// network secret (a Wi-Fi PSK, a passphrase) may be written to it. The
/// settings store, the only record the setup walk writes, can't hold one:
/// every key is a small number (a flag, a choice from a fixed list, or a
/// minute count), so no free text reaches the volume. A key that could
/// carry text, or a network key, must come back through this rule.
#[test]
fn no_setting_can_hold_a_network_secret() {
    for key in KEYS {
        assert!(
            matches!(key.kind(), Type::Bool | Type::Choice | Type::Minutes),
            "{key:?}"
        );
        // The largest value any key accepts fits in a u16: a list index or
        // a minute count, never an encoded string.
        let mut largest = 0u32;
        for v in 0..=u16::MAX as u32 + 1 {
            if key.valid(v) {
                largest = v;
            }
        }
        assert!(largest <= u16::MAX as u32, "{key:?} accepts {largest}");
    }
}
