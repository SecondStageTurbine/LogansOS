// SPDX-License-Identifier: MPL-2.0
//! The `NEOSET1` byte oracle with `setup-keys` (F11 S2, setup-menu THINK
//! §2.2): keys 6 (Language) and 7 (KeyboardLayout). As in `tests.rs`, every
//! record is hand-laid by a separate script from the layout table, and its
//! checksum is BLAKE3 from outside this crate (Python's `blake3` package);
//! the same script reproduces `tests.rs`'s `DEFAULTS_GEN1` byte for byte.
//!
//! | Record | Generation | Entries (key, type, reserved, value) |
//! |---|---|---|
//! | `DEFAULTS7_GEN1` | 1 | today's five defaults, (6, 2, 0, 0) (7, 2, 0, 0) |
//! | `CHANGED7_GEN2` | 2 | `tests.rs`'s `CHANGED` five, (6, 2, 0, 0) (7, 2, 0, 0) |
//! | `LANGUAGE_1` | 3 | (6, 2, 0, 1): one past the only language |
//! | `LANGUAGE_2` | 3 | (6, 2, 0, 2): one past Spanish, with `i18n-catalog` |
//! | `LAYOUT_1` | 3 | (7, 2, 0, 1): one past the only layout (Spanish with `keyboard-layouts`) |
//! | `LAYOUT_2` | 3 | (7, 2, 0, 2): one past Spanish, with `keyboard-layouts` |
//! | `LAYOUT_BOOL` | 3 | (7, 1, 0, 0): the wrong type |
//! | `DEFAULTS5_GEN1` | 1 | today's five-key record |

use super::*;

// DEFAULTS7_GEN1: 92 bytes, checksum 1ca373c0c135b0d880ac427a1540a232
#[rustfmt::skip]
const DEFAULTS7_GEN1: [u8; 92] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x07, 0x00, 0x01, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, // 16
    0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x04, 0x00, 0x02, 0x00, // 32
    0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x02, 0x00, // 48
    0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0xa3, 0x73, 0xc0, // 64
    0xc1, 0x35, 0xb0, 0xd8, 0x80, 0xac, 0x42, 0x7a, 0x15, 0x40, 0xa2, 0x32, // 80
];
// CHANGED7_GEN2: 92 bytes, checksum 6944e4d622ee2246b453b5a40df49ddf
#[rustfmt::skip]
const CHANGED7_GEN2: [u8; 92] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x07, 0x00, 0x02, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, // 16
    0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x1e, 0x00, 0x00, 0x00, 0x04, 0x00, 0x02, 0x00, // 32
    0x69, 0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x02, 0x00, // 48
    0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x69, 0x44, 0xe4, 0xd6, // 64
    0x22, 0xee, 0x22, 0x46, 0xb4, 0x53, 0xb5, 0xa4, 0x0d, 0xf4, 0x9d, 0xdf, // 80
];
// LANGUAGE_1: 44 bytes, checksum ed4227f72f04dc6ad42ea3269ac7aaf7
#[rustfmt::skip]
const LANGUAGE_1: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0xed, 0x42, 0x27, 0xf7, // 16
    0x2f, 0x04, 0xdc, 0x6a, 0xd4, 0x2e, 0xa3, 0x26, 0x9a, 0xc7, 0xaa, 0xf7, // 32
];
// LANGUAGE_2: 44 bytes, checksum 09e33ae04ff8efbff8653795aadfe5fc
#[cfg(feature = "i18n-catalog")]
#[rustfmt::skip]
const LANGUAGE_2: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0x09, 0xe3, 0x3a, 0xe0, // 16
    0x4f, 0xf8, 0xef, 0xbf, 0xf8, 0x65, 0x37, 0x95, 0xaa, 0xdf, 0xe5, 0xfc, // 32
];
// LAYOUT_1: 44 bytes, checksum c3055360dc2fff237ec5348fc4dac8a1
#[rustfmt::skip]
const LAYOUT_1: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0xc3, 0x05, 0x53, 0x60, // 16
    0xdc, 0x2f, 0xff, 0x23, 0x7e, 0xc5, 0x34, 0x8f, 0xc4, 0xda, 0xc8, 0xa1, // 32
];
// LAYOUT_2: 44 bytes, checksum aea5536e19169977a9b4308da6d16006
#[cfg(feature = "keyboard-layouts")]
#[rustfmt::skip]
const LAYOUT_2: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0xae, 0xa5, 0x53, 0x6e, // 16
    0x19, 0x16, 0x99, 0x77, 0xa9, 0xb4, 0x30, 0x8d, 0xa6, 0xd1, 0x60, 0x06, // 32
];
// LAYOUT_BOOL: 44 bytes, checksum 0bb1ef6c55545dbc307fca29d9eef0b2
#[rustfmt::skip]
const LAYOUT_BOOL: [u8; 44] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xb1, 0xef, 0x6c, // 16
    0x55, 0x54, 0x5d, 0xbc, 0x30, 0x7f, 0xca, 0x29, 0xd9, 0xee, 0xf0, 0xb2, // 32
];
// DEFAULTS5_GEN1: 76 bytes, checksum d52a55082e67d1b1c93fb73b207eb9bf
#[rustfmt::skip]
const DEFAULTS5_GEN1: [u8; 76] = [
    0x4e, 0x45, 0x4f, 0x53, 0x45, 0x54, 0x31, 0x00, 0x01, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, // 0
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, // 16
    0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x04, 0x00, 0x02, 0x00, // 32
    0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0xd5, 0x2a, 0x55, 0x08, // 48
    0x2e, 0x67, 0xd1, 0xb1, 0xc9, 0x3f, 0xb7, 0x3b, 0x20, 0x7e, 0xb9, 0xbf, // 64
];

const CHANGED: Settings = Settings {
    reduce_transparency: true,
    reduce_motion: Motion::Reduced,
    idle_lock_minutes: 30,
    time_zone: 105,
    rtc_local: false,
    language: 0,
    keyboard_layout: 0,
};

#[test]
fn encode_is_the_oracle_with_seven_keys() {
    assert_eq!(encode(&Settings::default(), 1), DEFAULTS7_GEN1);
    assert_eq!(encode(&CHANGED, 2), CHANGED7_GEN2);
    assert_eq!(
        decode(&CHANGED7_GEN2),
        Ok(Record {
            generation: 2,
            settings: CHANGED
        })
    );
}

/// Today's five-key record reads with the new keys at their defaults: an
/// installed system's store survives the upgrade.
#[test]
fn a_five_key_record_reads_the_new_keys_as_defaults() {
    assert_eq!(
        decode(&DEFAULTS5_GEN1),
        Ok(Record {
            generation: 1,
            settings: Settings::default()
        })
    );
}

#[test]
fn the_new_keys_ranges_types_and_ids() {
    assert_eq!(KEYS.map(Key::id), [1, 2, 3, 4, 5, 6, 7]);
    #[cfg(not(any(feature = "keyboard-layouts", feature = "i18n-catalog")))]
    assert_eq!((LANGUAGE_COUNT, LAYOUT_COUNT), (1, 1));
    #[cfg(not(feature = "i18n-catalog"))]
    assert_eq!(decode(&LANGUAGE_1), Err(RecordError::Value));
    #[cfg(not(feature = "keyboard-layouts"))]
    assert_eq!(decode(&LAYOUT_1), Err(RecordError::Value));
    assert_eq!(decode(&LAYOUT_BOOL), Err(RecordError::Type));
    assert_eq!(Key::from_id(8), None);
    let s = Settings::default();
    #[cfg(not(feature = "i18n-catalog"))]
    assert_eq!(s.with(6, 1), Err(SetError::Value));
    #[cfg(not(feature = "keyboard-layouts"))]
    assert_eq!(s.with(7, 1), Err(SetError::Value));
    assert_eq!(s.with(7, 0), Ok(s));
    assert_eq!(
        (Key::Language.kind(), Key::KeyboardLayout.kind()),
        (Type::Choice, Type::Choice)
    );
    assert_eq!(
        (
            Key::Language.default_value(),
            Key::KeyboardLayout.default_value()
        ),
        (0, 0)
    );
}

/// The setup menu's keys are the WM's alone (setup-menu THINK §4.4).
#[test]
fn the_new_keys_are_wm_only() {
    assert!(!Key::Language.client_writable());
    assert!(!Key::KeyboardLayout.client_writable());
    assert!(Key::ReduceTransparency.client_writable() && Key::ReduceMotion.client_writable());
}

/// F11 S5 (`keyboard-layouts`): two layouts, so Spanish (1) reads and sets, and
/// one past it (2) is refused, in a record and in `with`.
#[cfg(feature = "keyboard-layouts")]
#[test]
fn two_layouts_accept_uk_and_refuse_the_next_id() {
    assert_eq!(LAYOUT_COUNT, 2);
    assert_eq!(
        decode(&LAYOUT_1),
        Ok(Record {
            generation: 3,
            settings: Settings {
                keyboard_layout: 1,
                ..Settings::default()
            }
        })
    );
    assert_eq!(decode(&LAYOUT_2), Err(RecordError::Value));
    let s = Settings::default();
    assert_eq!(
        s.with(7, 1),
        Ok(Settings {
            keyboard_layout: 1,
            ..s
        })
    );
    assert_eq!(s.with(7, 2), Err(SetError::Value));
}

/// F11 S6b (`i18n-catalog`): two languages, so Spanish (1) reads and sets,
/// and one past it (2) is refused, in a record and in `with`.
#[cfg(feature = "i18n-catalog")]
#[test]
fn two_languages_accept_spanish_and_refuse_the_next_id() {
    assert_eq!(LANGUAGE_COUNT, 2);
    assert_eq!(
        decode(&LANGUAGE_1),
        Ok(Record {
            generation: 3,
            settings: Settings {
                language: 1,
                ..Settings::default()
            }
        })
    );
    assert_eq!(decode(&LANGUAGE_2), Err(RecordError::Value));
    let s = Settings::default();
    assert_eq!(s.with(6, 1), Ok(Settings { language: 1, ..s }));
    assert_eq!(s.with(6, 2), Err(SetError::Value));
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
