// SPDX-License-Identifier: MPL-2.0
//! F11 S5 keyboard layouts host KAT (`kernel/src/drivers/keymap_layout_logic.rs`).
//!
//! The Spanish (Spain, ISO) table below is written out by hand from the keycap
//! legends, not derived from the code under test. The legends come from two
//! raw sources read as files (never a summarizing fetch), which agree on every
//! level used here: Unicode CLDR `keyboards/windows/es-t-k0-windows.xml`
//! (release-43: the base, Shift, Caps and AltGr maps and the dead-key
//! transforms) and xkeyboard-config `symbols/es` "basic" with `latin(type4)`
//! (Ubuntu xkb-data 2.41). Where they differ CLDR is followed, being the
//! printed keycaps: AltGr+5 is `€` (xkb has none), and AltGr+4 is a plain `~`
//! as in xkb (CLDR makes it a dead tilde; S5 keeps only the four dead keys on
//! the legends). The HID usages are the USB HID Usage Tables' page 0x07: 0x31
//! and 0x32 are the ISO key left of Enter, 0x64 the key left of Z.

#[allow(dead_code)]
mod layout {
    include!("../../../kernel/src/drivers/keymap_us_logic.rs");
    include!("../../../kernel/src/drivers/keymap_layout_logic.rs");
}

use layout::{
    Composer, Dead, LAYOUT_COUNT, LAYOUT_ES, LAYOUT_NAMES, LAYOUT_US, Typed, ascii_in, compose,
    printable, spacing, typed_in,
};

const LSHIFT: u8 = 0x02;
const RSHIFT: u8 = 0x20;
const ALTGR: u8 = 0x40;
const LALT: u8 = 0x04;
const CAPS: u8 = 1;
const NUM_OFF: u8 = 2;

const N: Typed = Typed::None;
const fn c(ch: char) -> Typed {
    Typed::Char(ch)
}
const fn d(dead: Dead) -> Typed {
    Typed::Dead(dead)
}

/// (usage, plain, Shift, Caps Lock, Caps Lock + Shift, AltGr). AltGr types
/// the same with Caps Lock, and nothing with Shift.
#[rustfmt::skip]
const ES: &[(u8, Typed, Typed, Typed, Typed, Typed)] = &[
    (0x04, c('a'), c('A'), c('A'), c('a'), N),
    (0x05, c('b'), c('B'), c('B'), c('b'), N),
    (0x06, c('c'), c('C'), c('C'), c('c'), N),
    (0x07, c('d'), c('D'), c('D'), c('d'), N),
    (0x08, c('e'), c('E'), c('E'), c('e'), c('€')),
    (0x09, c('f'), c('F'), c('F'), c('f'), N),
    (0x0a, c('g'), c('G'), c('G'), c('g'), N),
    (0x0b, c('h'), c('H'), c('H'), c('h'), N),
    (0x0c, c('i'), c('I'), c('I'), c('i'), N),
    (0x0d, c('j'), c('J'), c('J'), c('j'), N),
    (0x0e, c('k'), c('K'), c('K'), c('k'), N),
    (0x0f, c('l'), c('L'), c('L'), c('l'), N),
    (0x10, c('m'), c('M'), c('M'), c('m'), N),
    (0x11, c('n'), c('N'), c('N'), c('n'), N),
    (0x12, c('o'), c('O'), c('O'), c('o'), N),
    (0x13, c('p'), c('P'), c('P'), c('p'), N),
    (0x14, c('q'), c('Q'), c('Q'), c('q'), N),
    (0x15, c('r'), c('R'), c('R'), c('r'), N),
    (0x16, c('s'), c('S'), c('S'), c('s'), N),
    (0x17, c('t'), c('T'), c('T'), c('t'), N),
    (0x18, c('u'), c('U'), c('U'), c('u'), N),
    (0x19, c('v'), c('V'), c('V'), c('v'), N),
    (0x1a, c('w'), c('W'), c('W'), c('w'), N),
    (0x1b, c('x'), c('X'), c('X'), c('x'), N),
    (0x1c, c('y'), c('Y'), c('Y'), c('y'), N),
    (0x1d, c('z'), c('Z'), c('Z'), c('z'), N),
    (0x1e, c('1'), c('!'), c('1'), c('!'), c('|')),
    (0x1f, c('2'), c('"'), c('2'), c('"'), c('@')),
    (0x20, c('3'), c('·'), c('3'), c('·'), c('#')),
    (0x21, c('4'), c('$'), c('4'), c('$'), c('~')),
    (0x22, c('5'), c('%'), c('5'), c('%'), c('€')),
    (0x23, c('6'), c('&'), c('6'), c('&'), c('¬')),
    (0x24, c('7'), c('/'), c('7'), c('/'), N),
    (0x25, c('8'), c('('), c('8'), c('('), N),
    (0x26, c('9'), c(')'), c('9'), c(')'), N),
    (0x27, c('0'), c('='), c('0'), c('='), N),
    (0x2c, c(' '), c(' '), c(' '), c(' '), N),
    (0x2d, c('\''), c('?'), c('\''), c('?'), N),
    (0x2e, c('¡'), c('¿'), c('¡'), c('¿'), N),
    (0x2f, d(Dead::Grave), d(Dead::Circumflex), d(Dead::Grave), d(Dead::Circumflex), c('[')),
    (0x30, c('+'), c('*'), c('+'), c('*'), c(']')),
    (0x31, c('ç'), c('Ç'), c('Ç'), c('ç'), c('}')),
    (0x32, c('ç'), c('Ç'), c('Ç'), c('ç'), c('}')),
    (0x33, c('ñ'), c('Ñ'), c('Ñ'), c('ñ'), N),
    (0x34, d(Dead::Acute), d(Dead::Diaeresis), d(Dead::Acute), d(Dead::Diaeresis), c('{')),
    (0x35, c('º'), c('ª'), c('º'), c('ª'), c('\\')),
    (0x36, c(','), c(';'), c(','), c(';'), N),
    (0x37, c('.'), c(':'), c('.'), c(':'), N),
    (0x38, c('-'), c('_'), c('-'), c('_'), N),
    (0x64, c('<'), c('>'), c('<'), c('>'), N),
];

#[test]
fn every_spanish_key_under_shift_altgr_caps_and_num() {
    assert_eq!(ES.len(), 50);
    for &(usage, plain, shift, caps, caps_shift, altgr) in ES {
        for num in [0, NUM_OFF] {
            let at = |mods, locks| typed_in(LAYOUT_ES, usage, mods, locks | num);
            let what = |s: &str| format!("usage {usage:#04x} {s} num={num}");
            assert_eq!(at(0, 0), plain, "{}", what("plain"));
            assert_eq!(at(LSHIFT, 0), shift, "{}", what("left Shift"));
            assert_eq!(at(RSHIFT, 0), shift, "{}", what("right Shift"));
            assert_eq!(at(0, CAPS), caps, "{}", what("Caps Lock"));
            assert_eq!(at(LSHIFT, CAPS), caps_shift, "{}", what("Caps + Shift"));
            assert_eq!(at(ALTGR, 0), altgr, "{}", what("AltGr"));
            assert_eq!(at(ALTGR, CAPS), altgr, "{}", what("AltGr + Caps"));
            assert_eq!(at(ALTGR | LSHIFT, 0), N, "{}", what("AltGr + Shift"));
            // Left Alt is not AltGr: the plain level.
            assert_eq!(at(LALT, 0), plain, "{}", what("left Alt"));
        }
    }
}

/// Every other usage types in Spanish exactly what it types in US (the
/// keypad, Enter, the navigation and function keys), AltGr included for the
/// keypad, and nothing with AltGr otherwise.
#[test]
fn keys_outside_the_table_are_the_us_keys() {
    for usage in 0..=u8::MAX {
        if ES.iter().any(|r| r.0 == usage) {
            continue;
        }
        let keypad = (0x53..=0x63).contains(&usage);
        for (mods, locks) in [
            (0, 0),
            (LSHIFT, 0),
            (0, CAPS),
            (LSHIFT, NUM_OFF),
            (0, NUM_OFF),
        ] {
            let us = match printable(usage, mods, locks) {
                Some(b) => Typed::Char(b as char),
                None => N,
            };
            assert_eq!(typed_in(LAYOUT_ES, usage, mods, locks), us, "{usage:#04x}");
            let altgr = typed_in(LAYOUT_ES, usage, mods | ALTGR, locks);
            if keypad {
                let us_alt = match printable(usage, mods | ALTGR, locks) {
                    Some(b) => Typed::Char(b as char),
                    None => N,
                };
                assert_eq!(altgr, us_alt, "keypad {usage:#04x}");
            } else if mods & LSHIFT == 0 {
                assert_eq!(altgr, N, "AltGr {usage:#04x}");
            }
        }
    }
}

/// The dead keys' compositions, by hand from CLDR's transforms: each vowel
/// (and y for acute) in both cases, Space for the bare accent.
#[test]
fn dead_key_compositions() {
    let table: &[(Dead, &str, &str)] = &[
        (Dead::Grave, "aAeEiIoOuU", "àÀèÈìÌòÒùÙ"),
        (Dead::Acute, "aAeEiIoOuUyY", "áÁéÉíÍóÓúÚýÝ"),
        (Dead::Circumflex, "aAeEiIoOuU", "âÂêÊîÎôÔûÛ"),
        (Dead::Diaeresis, "aAeEiIoOuU", "äÄëËïÏöÖüÜ"),
    ];
    let mut count = 0;
    for &(dead, bases, results) in table {
        for (b, r) in bases.chars().zip(results.chars()) {
            assert_eq!(compose(dead, b), Some(r), "{dead:?} {b}");
            count += 1;
        }
        assert_eq!(compose(dead, 'n'), None);
        assert_eq!(compose(dead, 'ñ'), None);
    }
    assert_eq!(count, 42);
    assert_eq!(
        [Dead::Grave, Dead::Acute, Dead::Circumflex, Dead::Diaeresis].map(spacing),
        ['`', '´', '^', '¨']
    );
}

/// The composer over key presses: the gate's ´ then a, a dead key then a
/// key it does not compose with (both), then Space (the accent), two dead
/// keys (both accents), and a key that types nothing (drops it).
#[test]
fn the_composer() {
    fn run(k: &mut Composer, typed: &[Typed]) -> String {
        let mut s = String::new();
        for &t in typed {
            s.extend(k.feed(t).as_slice());
        }
        s
    }
    let mut k = Composer::default();
    let mut run = |typed: &[Typed]| run(&mut k, typed);
    let acute = d(Dead::Acute);
    let dia = d(Dead::Diaeresis);
    assert_eq!(
        run(&[c('ñ'), c('¿'), c('¡'), c('ç'), acute, c('a')]),
        "ñ¿¡çá"
    );
    assert_eq!(run(&[dia, c('u'), acute, c('E')]), "üÉ");
    assert_eq!(run(&[acute, c('n')]), "´n");
    assert_eq!(run(&[acute, c(' ')]), "´");
    assert_eq!(run(&[d(Dead::Grave), d(Dead::Circumflex)]), "`^");
    assert_eq!(run(&[acute, N, c('a')]), "a");
    assert_eq!(run(&[acute]), "");
    assert_eq!(run(&[c('o')]), "ó");
    let mut k = Composer::default();
    k.feed(acute);
    assert_eq!(k.pending(), Some(Dead::Acute));
}

/// The ASCII decoders' view of Spanish (Home's field, Arthur): ASCII legends,
/// a dead key's ASCII accent (` and ^) without composing, nothing otherwise.
#[test]
fn ascii_in_spanish() {
    let at = |usage, mods| ascii_in(LAYOUT_ES, usage, mods, 0);
    assert_eq!(at(0x1f, LSHIFT), Some(b'"'));
    assert_eq!(at(0x1f, ALTGR), Some(b'@'));
    assert_eq!(at(0x27, LSHIFT), Some(b'='));
    assert_eq!(at(0x2f, 0), Some(b'`'));
    assert_eq!(at(0x2f, LSHIFT), Some(b'^'));
    for (usage, mods) in [
        (0x33, 0),
        (0x2e, 0),
        (0x2e, LSHIFT),
        (0x34, 0),
        (0x34, LSHIFT),
        (0x22, ALTGR),
    ] {
        assert_eq!(at(usage, mods), None, "{usage:#04x} {mods:#04x}");
    }
}

/// The kill-switch's proof: US by id is `printable`, for every usage, every
/// modifier byte and every lock state, as a [`Typed`] and as ASCII; a bad id
/// types US too.
#[test]
fn us_by_id_is_the_us_table_for_every_key_modifier_and_lock() {
    for usage in 0..=u8::MAX {
        for mods in 0..=u8::MAX {
            for locks in 0..4u8 {
                let us = printable(usage, mods, locks);
                for layout in [LAYOUT_US, LAYOUT_COUNT, 0xff] {
                    assert_eq!(ascii_in(layout, usage, mods, locks), us);
                    let typed = match us {
                        Some(b) => Typed::Char(b as char),
                        None => N,
                    };
                    assert_eq!(typed_in(layout, usage, mods, locks), typed);
                }
            }
        }
    }
}

#[test]
fn layout_ids_and_names() {
    assert_eq!((LAYOUT_US, LAYOUT_ES, LAYOUT_COUNT), (0, 1, 2));
    assert_eq!(LAYOUT_NAMES, ["English (US)", "Spanish (Spain)"]);
    assert!(layout::layout_valid(1) && !layout::layout_valid(2));
}
