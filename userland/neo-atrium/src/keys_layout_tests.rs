// SPDX-License-Identifier: MPL-2.0
//! F11 S5 (`keyboard-layouts`): the WM's decoder by layout id. The tables
//! themselves are KAT'd in `tools/xtask/src/keymap_layout_tests.rs`; these
//! check the decoder around them.

use crate::keys::{self, Key};

fn key(usage: u8, modifiers: u8, locks: u8) -> [u8; 8] {
    [1, 1, usage, 0, modifiers, 0, locks, 0]
}

/// The kill-switch: in US, the decoder with layouts is the decoder without
/// them, for every usage, modifier byte, lock state and press/release.
#[test]
fn us_decoder_is_the_decoder_without_layouts() {
    for usage in 0..=u8::MAX {
        for modifiers in 0..=u8::MAX {
            for locks in 0..4u8 {
                for flags in [0u8, 1] {
                    let mut e = key(usage, modifiers, locks);
                    e[1] = flags;
                    assert_eq!(
                        keys::decode_in(keys::LAYOUT_US, e),
                        keys::decode_us(e),
                        "usage {usage:#04x} mods {modifiers:#04x} locks {locks} flags {flags}"
                    );
                }
            }
        }
    }
    // A pointer event and a high usage (not a byte) decode to nothing.
    assert_eq!(
        keys::decode_in(keys::LAYOUT_UK, [2, 1, 4, 0, 0, 0, 0, 0]),
        None
    );
    assert_eq!(
        keys::decode_in(keys::LAYOUT_UK, [1, 1, 0x04, 0x01, 0, 0, 0, 0]),
        None
    );
}

/// The gate's four keys: Shift+2, Shift+', the key left of Enter and the key
/// left of Z type `"` `@` `#` `\` in UK and `@` `"` `\` (nothing) in US.
#[test]
fn the_gate_keys_in_each_layout() {
    let keys_ = [
        key(0x1f, 0x02, 0),
        key(0x34, 0x20, 0),
        key(0x31, 0, 0),
        key(0x64, 0, 0),
    ];
    let typed = |layout| -> Vec<Option<Key>> {
        keys_.iter().map(|&e| keys::decode_in(layout, e)).collect()
    };
    assert_eq!(
        typed(keys::LAYOUT_UK),
        [
            Some(Key::Char(b'"')),
            Some(Key::Char(b'@')),
            Some(Key::Char(b'#')),
            Some(Key::Char(b'\\'))
        ]
    );
    assert_eq!(
        typed(keys::LAYOUT_US),
        [
            Some(Key::Char(b'@')),
            Some(Key::Char(b'"')),
            Some(Key::Char(b'\\')),
            None
        ]
    );
}

/// The session's layout: `decode` follows it, a bad id is refused and the
/// layout stays. One test owns the global, so the order is fixed.
#[test]
fn decode_follows_the_session_layout() {
    let shift2 = key(0x1f, 0x02, 0);
    assert_eq!(keys::layout(), keys::LAYOUT_US);
    assert_eq!(keys::decode(shift2), Some(Key::Char(b'@')));
    assert!(keys::set_layout(keys::LAYOUT_UK));
    assert_eq!(keys::decode(shift2), Some(Key::Char(b'"')));
    assert!(!keys::set_layout(keys::LAYOUT_COUNT));
    assert_eq!(keys::layout(), keys::LAYOUT_UK);
    // Navigation keys are the layout's business only through the keypad,
    // which is the same in both.
    assert_eq!(keys::decode(key(0x28, 0, 0)), Some(Key::Enter));
    assert!(keys::set_layout(keys::LAYOUT_US));
    assert_eq!(keys::decode(shift2), Some(Key::Char(b'@')));
}
