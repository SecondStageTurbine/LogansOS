// SPDX-License-Identifier: MPL-2.0
//! US keymap host KAT (`kernel/src/drivers/keymap_us_logic.rs`). The table
//! below is written out by hand from the US keycap legends and the USB HID
//! Usage Tables (Keyboard/Keypad page 0x07), not derived from the code under
//! test: every printable key under plain, Shift, Caps Lock and Caps+Shift.
//! Then the i8042 set-1 decoder is chained in front of it, so a scancode
//! sequence typed on a PS/2 keyboard is checked all the way to the text.

#[allow(dead_code)]
mod keymap {
    include!("../../../kernel/src/drivers/keymap_us_logic.rs");
}

// The i8042 decoder, as `i8042_tests` loads it (a file loads as one module).
use super::i8042_tests::i8042_logic;

const LSHIFT: u8 = 0x02;
const RSHIFT: u8 = 0x20;
const CAPS: u8 = 1;

/// (usage, plain, Shift, Caps Lock, Caps Lock + Shift).
const US: &[(u8, u8, u8, u8, u8)] = &[
    (0x04, b'a', b'A', b'A', b'a'),
    (0x05, b'b', b'B', b'B', b'b'),
    (0x06, b'c', b'C', b'C', b'c'),
    (0x07, b'd', b'D', b'D', b'd'),
    (0x08, b'e', b'E', b'E', b'e'),
    (0x09, b'f', b'F', b'F', b'f'),
    (0x0a, b'g', b'G', b'G', b'g'),
    (0x0b, b'h', b'H', b'H', b'h'),
    (0x0c, b'i', b'I', b'I', b'i'),
    (0x0d, b'j', b'J', b'J', b'j'),
    (0x0e, b'k', b'K', b'K', b'k'),
    (0x0f, b'l', b'L', b'L', b'l'),
    (0x10, b'm', b'M', b'M', b'm'),
    (0x11, b'n', b'N', b'N', b'n'),
    (0x12, b'o', b'O', b'O', b'o'),
    (0x13, b'p', b'P', b'P', b'p'),
    (0x14, b'q', b'Q', b'Q', b'q'),
    (0x15, b'r', b'R', b'R', b'r'),
    (0x16, b's', b'S', b'S', b's'),
    (0x17, b't', b'T', b'T', b't'),
    (0x18, b'u', b'U', b'U', b'u'),
    (0x19, b'v', b'V', b'V', b'v'),
    (0x1a, b'w', b'W', b'W', b'w'),
    (0x1b, b'x', b'X', b'X', b'x'),
    (0x1c, b'y', b'Y', b'Y', b'y'),
    (0x1d, b'z', b'Z', b'Z', b'z'),
    // Caps Lock is not Shift Lock: digits and symbols ignore it.
    (0x1e, b'1', b'!', b'1', b'!'),
    (0x1f, b'2', b'@', b'2', b'@'),
    (0x20, b'3', b'#', b'3', b'#'),
    (0x21, b'4', b'$', b'4', b'$'),
    (0x22, b'5', b'%', b'5', b'%'),
    (0x23, b'6', b'^', b'6', b'^'),
    (0x24, b'7', b'&', b'7', b'&'),
    (0x25, b'8', b'*', b'8', b'*'),
    (0x26, b'9', b'(', b'9', b'('),
    (0x27, b'0', b')', b'0', b')'),
    (0x2c, b' ', b' ', b' ', b' '),
    (0x2d, b'-', b'_', b'-', b'_'),
    (0x2e, b'=', b'+', b'=', b'+'),
    (0x2f, b'[', b'{', b'[', b'{'),
    (0x30, b']', b'}', b']', b'}'),
    (0x31, b'\\', b'|', b'\\', b'|'),
    (0x33, b';', b':', b';', b':'),
    (0x34, b'\'', b'"', b'\'', b'"'),
    (0x35, b'`', b'~', b'`', b'~'),
    (0x36, b',', b'<', b',', b'<'),
    (0x37, b'.', b'>', b'.', b'>'),
    (0x38, b'/', b'?', b'/', b'?'),
];

#[test]
fn every_us_key_under_every_shift_and_caps_state() {
    for &(usage, plain, shift, caps, caps_shift) in US {
        let at = |mods, locks| keymap::printable(usage, mods, locks);
        assert_eq!(at(0, 0), Some(plain), "usage {usage:#04x} plain");
        assert_eq!(at(LSHIFT, 0), Some(shift), "usage {usage:#04x} left Shift");
        assert_eq!(at(RSHIFT, 0), Some(shift), "usage {usage:#04x} right Shift");
        assert_eq!(at(0, CAPS), Some(caps), "usage {usage:#04x} Caps Lock");
        assert_eq!(
            at(LSHIFT, CAPS),
            Some(caps_shift),
            "usage {usage:#04x} Caps Lock + Shift"
        );
    }
}

/// The table covers all 47 printable keys of a US keyboard (26 letters, 10
/// digits, 11 symbol keys) plus Space, and every printable ASCII byte is
/// typed by exactly one (key, Shift) pair, so nothing is missing or doubled.
#[test]
fn us_table_covers_printable_ascii_once() {
    assert_eq!(US.len(), 48);
    let mut seen = [0u8; 128];
    for &(usage, plain, shift, _, _) in US {
        seen[plain as usize] += 1;
        if usage != 0x2c {
            seen[shift as usize] += 1;
        }
    }
    for (byte, &count) in seen.iter().enumerate().take(0x7f).skip(0x20) {
        assert_eq!(count, 1, "ASCII {byte:#04x} ({:?})", byte as u8 as char);
    }
}

/// Every usage not in the table types nothing, under any Shift/Caps state:
/// modifiers, function and navigation keys, Enter/Esc/Backspace/Tab (callers
/// handle those by usage), Non-US `#` (0x32) and Non-US `\` (0x64).
#[test]
fn keys_outside_the_table_type_nothing() {
    for usage in 0..=u8::MAX {
        if US.iter().any(|row| row.0 == usage) {
            continue;
        }
        for (mods, locks) in [(0, 0), (LSHIFT, 0), (0, CAPS), (RSHIFT, CAPS)] {
            assert_eq!(
                keymap::printable(usage, mods, locks),
                None,
                "usage {usage:#04x} mods {mods:#04x} locks {locks}"
            );
        }
    }
}

/// Control and Alt do not change the character (only Shift does).
#[test]
fn only_shift_selects_the_upper_legend() {
    for mods in [0x01, 0x04, 0x08, 0x10, 0x40, 0x80] {
        assert_eq!(keymap::printable(0x33, mods, 0), Some(b';'));
        assert_eq!(keymap::printable(0x04, mods, 0), Some(b'a'));
    }
    assert_eq!(keymap::printable(0x33, LSHIFT | RSHIFT, 0), Some(b':'));
}

#[test]
fn caps_lock_toggles_on_each_press_only() {
    let mut locks = 0;
    for (usage, pressed, want) in [
        (0x39, true, CAPS),  // press: on
        (0x39, false, CAPS), // release: unchanged
        (0x04, true, CAPS),  // another key: unchanged
        (0x39, true, 0),     // press again: off
        (0x39, false, 0),
    ] {
        locks = keymap::next_locks(locks, usage, pressed);
        assert_eq!(locks, want, "usage {usage:#04x} pressed={pressed}");
    }
}

/// PS/2 end to end on the host: hand-laid set-1 bytes (what the controller
/// hands the driver for Shift+`;`, Shift+`'` and Caps Lock `a`) through the
/// i8042 decoder, the driver's modifier and lock tracking, then the keymap.
#[test]
fn ps2_set1_bytes_type_colon_quote_and_caps() {
    // LShift make, `;` make/break, `'` make/break, LShift break; Caps make/
    // break, `a` make/break; RShift make, `/` make/break, RShift break.
    let bytes = [
        0x2a, 0x27, 0xa7, 0x28, 0xa8, 0xaa, 0x3a, 0xba, 0x1e, 0x9e, 0x36, 0x35, 0xb5, 0xb6,
    ];
    let mut decoder = i8042_logic::Decoder::new();
    let (mut mods, mut locks) = (0u8, 0u8);
    let mut text = Vec::new();
    for byte in bytes {
        let Some(event) = decoder.feed(byte) else {
            continue;
        };
        if (0xe0..=0xe7).contains(&event.usage) {
            let bit = 1 << (event.usage - 0xe0);
            if event.pressed {
                mods |= bit
            } else {
                mods &= !bit
            }
            continue;
        }
        locks = keymap::next_locks(locks, event.usage, event.pressed);
        if event.pressed
            && let Some(c) = keymap::printable(event.usage, mods, locks)
        {
            text.push(c);
        }
    }
    assert_eq!(text, b":\"A?");
}
