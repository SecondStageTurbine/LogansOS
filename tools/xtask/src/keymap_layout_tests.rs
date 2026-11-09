// SPDX-License-Identifier: MPL-2.0
//! F11 S5 keyboard layouts host KAT (`kernel/src/drivers/keymap_layout_logic.rs`).
//!
//! The UK table below is written out by hand from the keycap legends of a UK
//! ISO keyboard, not derived from the code under test. Three raw sources agree
//! on every legend used here (read as files, not through a summarizing fetch):
//! xkeyboard-config `symbols/gb` ("basic"), kbd `data/keymaps/i386/qwerty/uk.map`
//! and Unicode CLDR `keyboards/windows/en-GB-t-k0-windows.xml` (release-43).
//! The HID usages are the USB HID Usage Tables' Keyboard/Keypad page (0x07):
//! 0x32 is "Non-US # and ~" (the key left of Enter on ISO) and 0x64 is "Non-US
//! \ and |" (the key left of Z).
//!
//! `None` marks a legend outside ASCII: UK Shift+3 is `£` and Shift+` is `¬`,
//! and every decoder's text is ASCII today, so those keys type nothing.

#[allow(dead_code)]
mod layout {
    include!("../../../kernel/src/drivers/keymap_us_logic.rs");
    include!("../../../kernel/src/drivers/keymap_layout_logic.rs");
}

use layout::{LAYOUT_COUNT, LAYOUT_NAMES, LAYOUT_UK, LAYOUT_US, printable, printable_in};

const LSHIFT: u8 = 0x02;
const RSHIFT: u8 = 0x20;
const CAPS: u8 = 1;
const NUM_OFF: u8 = 2;

type Row = (u8, Option<u8>, Option<u8>, Option<u8>, Option<u8>);

const fn s(c: u8) -> Option<u8> {
    Some(c)
}

/// UK ISO, by hand from the keycap legends: (usage, plain, Shift, Caps Lock,
/// Caps Lock + Shift). Caps Lock affects letters only.
#[rustfmt::skip]
const UK: &[Row] = &[
    (0x04, s(b'a'), s(b'A'), s(b'A'), s(b'a')),
    (0x05, s(b'b'), s(b'B'), s(b'B'), s(b'b')),
    (0x06, s(b'c'), s(b'C'), s(b'C'), s(b'c')),
    (0x07, s(b'd'), s(b'D'), s(b'D'), s(b'd')),
    (0x08, s(b'e'), s(b'E'), s(b'E'), s(b'e')),
    (0x09, s(b'f'), s(b'F'), s(b'F'), s(b'f')),
    (0x0a, s(b'g'), s(b'G'), s(b'G'), s(b'g')),
    (0x0b, s(b'h'), s(b'H'), s(b'H'), s(b'h')),
    (0x0c, s(b'i'), s(b'I'), s(b'I'), s(b'i')),
    (0x0d, s(b'j'), s(b'J'), s(b'J'), s(b'j')),
    (0x0e, s(b'k'), s(b'K'), s(b'K'), s(b'k')),
    (0x0f, s(b'l'), s(b'L'), s(b'L'), s(b'l')),
    (0x10, s(b'm'), s(b'M'), s(b'M'), s(b'm')),
    (0x11, s(b'n'), s(b'N'), s(b'N'), s(b'n')),
    (0x12, s(b'o'), s(b'O'), s(b'O'), s(b'o')),
    (0x13, s(b'p'), s(b'P'), s(b'P'), s(b'p')),
    (0x14, s(b'q'), s(b'Q'), s(b'Q'), s(b'q')),
    (0x15, s(b'r'), s(b'R'), s(b'R'), s(b'r')),
    (0x16, s(b's'), s(b'S'), s(b'S'), s(b's')),
    (0x17, s(b't'), s(b'T'), s(b'T'), s(b't')),
    (0x18, s(b'u'), s(b'U'), s(b'U'), s(b'u')),
    (0x19, s(b'v'), s(b'V'), s(b'V'), s(b'v')),
    (0x1a, s(b'w'), s(b'W'), s(b'W'), s(b'w')),
    (0x1b, s(b'x'), s(b'X'), s(b'X'), s(b'x')),
    (0x1c, s(b'y'), s(b'Y'), s(b'Y'), s(b'y')),
    (0x1d, s(b'z'), s(b'Z'), s(b'Z'), s(b'z')),
    (0x1e, s(b'1'), s(b'!'), s(b'1'), s(b'!')),
    (0x1f, s(b'2'), s(b'"'), s(b'2'), s(b'"')),
    (0x20, s(b'3'), None,    s(b'3'), None),    // Shift: £
    (0x21, s(b'4'), s(b'$'), s(b'4'), s(b'$')),
    (0x22, s(b'5'), s(b'%'), s(b'5'), s(b'%')),
    (0x23, s(b'6'), s(b'^'), s(b'6'), s(b'^')),
    (0x24, s(b'7'), s(b'&'), s(b'7'), s(b'&')),
    (0x25, s(b'8'), s(b'*'), s(b'8'), s(b'*')),
    (0x26, s(b'9'), s(b'('), s(b'9'), s(b'(')),
    (0x27, s(b'0'), s(b')'), s(b'0'), s(b')')),
    (0x2c, s(b' '), s(b' '), s(b' '), s(b' ')),
    (0x2d, s(b'-'), s(b'_'), s(b'-'), s(b'_')),
    (0x2e, s(b'='), s(b'+'), s(b'='), s(b'+')),
    (0x2f, s(b'['), s(b'{'), s(b'['), s(b'{')),
    (0x30, s(b']'), s(b'}'), s(b']'), s(b'}')),
    (0x31, s(b'#'), s(b'~'), s(b'#'), s(b'~')), // the key left of Enter
    (0x32, s(b'#'), s(b'~'), s(b'#'), s(b'~')), // the same key, Non-US #
    (0x33, s(b';'), s(b':'), s(b';'), s(b':')),
    (0x34, s(b'\''), s(b'@'), s(b'\''), s(b'@')),
    (0x35, s(b'`'), None,    s(b'`'), None),    // Shift: ¬
    (0x36, s(b','), s(b'<'), s(b','), s(b'<')),
    (0x37, s(b'.'), s(b'>'), s(b'.'), s(b'>')),
    (0x38, s(b'/'), s(b'?'), s(b'/'), s(b'?')),
    (0x64, s(b'\\'), s(b'|'), s(b'\\'), s(b'|')), // the key left of Z
];

/// The same keys on a US keyboard, by hand from the US legends (as
/// `keymap_tests`'s table, plus the two ISO keys a US keyboard lacks, which
/// type nothing in US).
#[rustfmt::skip]
const US: &[Row] = &[
    (0x1f, s(b'2'), s(b'@'), s(b'2'), s(b'@')),
    (0x20, s(b'3'), s(b'#'), s(b'3'), s(b'#')),
    (0x31, s(b'\\'), s(b'|'), s(b'\\'), s(b'|')),
    (0x32, None,    None,    None,    None),
    (0x34, s(b'\''), s(b'"'), s(b'\''), s(b'"')),
    (0x35, s(b'`'), s(b'~'), s(b'`'), s(b'~')),
    (0x64, None,    None,    None,    None),
];

fn check(layout: u8, rows: &[Row]) {
    for &(usage, plain, shift, caps, caps_shift) in rows {
        // Num Lock on (0) and off: the typing keys ignore it.
        for num in [0, NUM_OFF] {
            let at = |mods, locks| printable_in(layout, usage, mods, locks | num);
            let what =
                |state: &str| format!("layout {layout} usage {usage:#04x} {state} num={num}");
            assert_eq!(at(0, 0), plain, "{}", what("plain"));
            assert_eq!(at(LSHIFT, 0), shift, "{}", what("left Shift"));
            assert_eq!(at(RSHIFT, 0), shift, "{}", what("right Shift"));
            assert_eq!(at(0, CAPS), caps, "{}", what("Caps Lock"));
            assert_eq!(at(LSHIFT, CAPS), caps_shift, "{}", what("Caps + Shift"));
        }
    }
}

#[test]
fn every_uk_key_under_every_shift_caps_and_num_state() {
    assert_eq!(UK.len(), 50);
    check(LAYOUT_UK, UK);
}

#[test]
fn us_rows_for_the_keys_uk_changes() {
    check(LAYOUT_US, US);
}

/// The two hand-written tables agree everywhere except where the legends
/// differ, and those places are exactly these (usage, column) pairs.
#[test]
fn uk_and_us_differ_only_where_the_legends_do() {
    // Columns: 0 plain, 1 Shift, 2 Caps, 3 Caps + Shift.
    let expected: &[(u8, usize)] = &[
        (0x1f, 1),
        (0x1f, 3), // " vs @
        (0x20, 1),
        (0x20, 3), // £ vs #
        (0x31, 0),
        (0x31, 1),
        (0x31, 2),
        (0x31, 3), // #~ vs \|
        (0x32, 0),
        (0x32, 1),
        (0x32, 2),
        (0x32, 3), // #~ vs nothing
        (0x34, 1),
        (0x34, 3), // @ vs "
        (0x35, 1),
        (0x35, 3), // ¬ vs ~
        (0x64, 0),
        (0x64, 1),
        (0x64, 2),
        (0x64, 3), // \| vs nothing
    ];
    let mut differ = Vec::new();
    for usage in 0..=u8::MAX {
        for (col, (mods, locks)) in [(0, 0), (LSHIFT, 0), (0, CAPS), (LSHIFT, CAPS)]
            .into_iter()
            .enumerate()
        {
            for num in [0, NUM_OFF] {
                let uk = printable_in(LAYOUT_UK, usage, mods, locks | num);
                let us = printable_in(LAYOUT_US, usage, mods, locks | num);
                if uk != us && num == 0 {
                    differ.push((usage, col));
                }
                if uk != us && num == NUM_OFF {
                    assert!(
                        differ.contains(&(usage, col)),
                        "{usage:#04x} differs only with Num off"
                    );
                }
            }
        }
    }
    assert_eq!(differ, expected);
    // And the hand tables say the same: the US rows differ from the UK rows
    // at exactly those places.
    for &(usage, col) in expected {
        let uk = UK.iter().find(|r| r.0 == usage).unwrap();
        let us = US.iter().find(|r| r.0 == usage).unwrap();
        let pick = |r: &Row| [r.1, r.2, r.3, r.4][col];
        assert_ne!(pick(uk), pick(us), "{usage:#04x} col {col}");
    }
}

/// The kill-switch's proof: US by id is `printable`, for every usage, every
/// modifier byte and every lock state, so a decoder that selects US is the
/// decoder without layouts. A bad id types US too.
#[test]
fn us_by_id_is_the_us_table_for_every_key_modifier_and_lock() {
    for usage in 0..=u8::MAX {
        for mods in 0..=u8::MAX {
            for locks in 0..4u8 {
                let us = printable(usage, mods, locks);
                assert_eq!(printable_in(LAYOUT_US, usage, mods, locks), us);
                for bad in [LAYOUT_COUNT, 0xff] {
                    assert_eq!(printable_in(bad, usage, mods, locks), us);
                }
            }
        }
    }
}

/// Only Shift picks the upper legend in UK too (Control, Alt and AltGr do
/// not), and the keypad is the same in both layouts.
#[test]
fn uk_ignores_non_shift_modifiers_and_keeps_the_keypad() {
    for mods in [0x01, 0x04, 0x08, 0x10, 0x40, 0x80] {
        assert_eq!(printable_in(LAYOUT_UK, 0x1f, mods, 0), Some(b'2'));
        assert_eq!(printable_in(LAYOUT_UK, 0x64, mods, 0), Some(b'\\'));
    }
    for usage in 0x53..=0x63 {
        for mods in [0, LSHIFT] {
            for locks in 0..4 {
                assert_eq!(
                    printable_in(LAYOUT_UK, usage, mods, locks),
                    printable(usage, mods, locks),
                    "{usage:#04x}"
                );
            }
        }
    }
}

#[test]
fn layout_ids_and_names() {
    assert_eq!((LAYOUT_US, LAYOUT_UK, LAYOUT_COUNT), (0, 1, 2));
    assert_eq!(LAYOUT_NAMES, ["English (US)", "English (UK)"]);
    assert!(layout::layout_valid(1) && !layout::layout_valid(2));
}
