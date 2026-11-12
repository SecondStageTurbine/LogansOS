// SPDX-License-Identifier: MPL-2.0
// F11 S5 (`keyboard-layouts`, setup-menu THINK §3.2, §5 S5): the keyboard
// layouts, selected by an id. US is `keymap_us_logic.rs`, unchanged; Spanish
// (Spain, ISO) is the keys whose legends differ from US, an AltGr level and
// four dead keys (grave, acute, circumflex, diaeresis); every other key types
// what US types.
//
// No inner attributes or doc comments at the top, as in `keymap_us_logic.rs`:
// a decoder takes both files with `include!` into one module, and this one
// calls that file's `printable` by name.
//
// Two outputs:
// - [`typed_in`]: what a key types in a layout, a char or a dead key, and
//   [`Composer`] turns a run of them into text (the setup menu's test field).
// - [`ascii_in`]: the printable ASCII a key types, for the decoders whose text
//   is still ASCII (the WM's Home field, Arthur): a non-ASCII legend types
//   nothing there, and a dead key types its accent only when that is ASCII
//   (` and ^), without composing.
//
// Pure and dependency-free; host-tested by `tools/xtask/src/keymap_layout_tests.rs`
// (every key x Shift x AltGr x Caps Lock for Spanish, written out by hand from
// the keycap legends, the dead-key compositions, and US by id equal to
// `printable` for every usage, modifier byte and lock state).

/// Layout id 0: US (the default, and the only layout without
/// `keyboard-layouts`). The id is the settings store's `KeyboardLayout`.
pub const LAYOUT_US: u8 = 0;
/// Layout id 1: Spanish (Spain, ISO).
pub const LAYOUT_ES: u8 = 1;
/// How many layouts exist: an id is below it.
pub const LAYOUT_COUNT: u8 = 2;

/// The names the Keyboard page shows, by id.
pub const LAYOUT_NAMES: [&str; LAYOUT_COUNT as usize] = ["English (US)", "Spanish (Spain)"];

/// HID modifier byte: right Alt, which is AltGr on an ISO layout.
pub const MOD_ALTGR: u8 = 0x40;

/// A dead key: it types nothing, and the next key composes with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dead {
    Grave,
    Acute,
    Circumflex,
    Diaeresis,
}

/// What a key types in a layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typed {
    /// Nothing (a modifier, a navigation key, Enter, ...).
    None,
    Char(char),
    Dead(Dead),
}

/// One level of a key: nothing, a character or a dead key.
#[derive(Clone, Copy)]
enum Lv {
    N,
    C(char),
    D(Dead),
}

/// The Spanish keys whose plain and Shift legends differ from US: usage,
/// plain, Shift, and whether Caps Lock acts on it (a letter). Usage 0x31 (US
/// `\`) and 0x32 (Non-US `#`) are the same key on an ISO keyboard, the one
/// left of Enter; 0x64 (Non-US `\`) is the key left of Z.
const ES: [(u8, Lv, Lv, bool); 20] = [
    (0x35, Lv::C('º'), Lv::C('ª'), false),
    (0x1f, Lv::C('2'), Lv::C('"'), false),
    (0x20, Lv::C('3'), Lv::C('·'), false),
    (0x23, Lv::C('6'), Lv::C('&'), false),
    (0x24, Lv::C('7'), Lv::C('/'), false),
    (0x25, Lv::C('8'), Lv::C('('), false),
    (0x26, Lv::C('9'), Lv::C(')'), false),
    (0x27, Lv::C('0'), Lv::C('='), false),
    (0x2d, Lv::C('\''), Lv::C('?'), false),
    (0x2e, Lv::C('¡'), Lv::C('¿'), false),
    (0x2f, Lv::D(Dead::Grave), Lv::D(Dead::Circumflex), false),
    (0x30, Lv::C('+'), Lv::C('*'), false),
    (0x33, Lv::C('ñ'), Lv::C('Ñ'), true),
    (0x34, Lv::D(Dead::Acute), Lv::D(Dead::Diaeresis), false),
    (0x31, Lv::C('ç'), Lv::C('Ç'), true),
    (0x32, Lv::C('ç'), Lv::C('Ç'), true),
    (0x64, Lv::C('<'), Lv::C('>'), false),
    (0x36, Lv::C(','), Lv::C(';'), false),
    (0x37, Lv::C('.'), Lv::C(':'), false),
    (0x38, Lv::C('-'), Lv::C('_'), false),
];

/// The Spanish AltGr level (with or without Caps Lock; AltGr with Shift
/// types nothing). Any other key types nothing with AltGr, except the
/// keypad, which ignores it.
const ES_ALTGR: [(u8, char); 13] = [
    (0x35, '\\'),
    (0x1e, '|'),
    (0x1f, '@'),
    (0x20, '#'),
    (0x21, '~'),
    (0x22, '€'),
    (0x23, '¬'),
    (0x08, '€'),
    (0x2f, '['),
    (0x30, ']'),
    (0x34, '{'),
    (0x31, '}'),
    (0x32, '}'),
];

/// The dead keys' compositions: dead key, base, result (CLDR's transforms).
const COMPOSE: [(Dead, char, char); 42] = [
    (Dead::Grave, 'a', 'à'),
    (Dead::Grave, 'A', 'À'),
    (Dead::Grave, 'e', 'è'),
    (Dead::Grave, 'E', 'È'),
    (Dead::Grave, 'i', 'ì'),
    (Dead::Grave, 'I', 'Ì'),
    (Dead::Grave, 'o', 'ò'),
    (Dead::Grave, 'O', 'Ò'),
    (Dead::Grave, 'u', 'ù'),
    (Dead::Grave, 'U', 'Ù'),
    (Dead::Acute, 'a', 'á'),
    (Dead::Acute, 'A', 'Á'),
    (Dead::Acute, 'e', 'é'),
    (Dead::Acute, 'E', 'É'),
    (Dead::Acute, 'i', 'í'),
    (Dead::Acute, 'I', 'Í'),
    (Dead::Acute, 'o', 'ó'),
    (Dead::Acute, 'O', 'Ó'),
    (Dead::Acute, 'u', 'ú'),
    (Dead::Acute, 'U', 'Ú'),
    (Dead::Acute, 'y', 'ý'),
    (Dead::Acute, 'Y', 'Ý'),
    (Dead::Circumflex, 'a', 'â'),
    (Dead::Circumflex, 'A', 'Â'),
    (Dead::Circumflex, 'e', 'ê'),
    (Dead::Circumflex, 'E', 'Ê'),
    (Dead::Circumflex, 'i', 'î'),
    (Dead::Circumflex, 'I', 'Î'),
    (Dead::Circumflex, 'o', 'ô'),
    (Dead::Circumflex, 'O', 'Ô'),
    (Dead::Circumflex, 'u', 'û'),
    (Dead::Circumflex, 'U', 'Û'),
    (Dead::Diaeresis, 'a', 'ä'),
    (Dead::Diaeresis, 'A', 'Ä'),
    (Dead::Diaeresis, 'e', 'ë'),
    (Dead::Diaeresis, 'E', 'Ë'),
    (Dead::Diaeresis, 'i', 'ï'),
    (Dead::Diaeresis, 'I', 'Ï'),
    (Dead::Diaeresis, 'o', 'ö'),
    (Dead::Diaeresis, 'O', 'Ö'),
    (Dead::Diaeresis, 'u', 'ü'),
    (Dead::Diaeresis, 'U', 'Ü'),
];

/// Whether `layout` is a layout id ([`LAYOUT_COUNT`]).
pub const fn layout_valid(layout: u8) -> bool {
    layout < LAYOUT_COUNT
}

/// The upper case of a Spanish letter key's lower legend.
const fn upper(c: char) -> char {
    match c {
        'ñ' => 'Ñ',
        'ç' => 'Ç',
        _ => c,
    }
}

const fn level(lv: Lv) -> Typed {
    match lv {
        Lv::N => Typed::None,
        Lv::C(c) => Typed::Char(c),
        Lv::D(d) => Typed::Dead(d),
    }
}

/// What a key press types in `layout`. US is `printable` (it never has a
/// dead key, and Alt does not change a key). An id that is not a layout
/// types US: a bad id must not leave the keyboard dead.
pub const fn typed_in(layout: u8, usage: u8, modifiers: u8, locks: u8) -> Typed {
    if layout == LAYOUT_ES {
        let keypad = usage >= 0x53 && usage <= 0x63;
        if modifiers & MOD_ALTGR != 0 && !keypad {
            if modifiers & MOD_SHIFT != 0 {
                return Typed::None;
            }
            let mut i = 0;
            while i < ES_ALTGR.len() {
                let (u, c) = ES_ALTGR[i];
                if u == usage {
                    return Typed::Char(c);
                }
                i += 1;
            }
            return Typed::None;
        }
        let shift = modifiers & MOD_SHIFT != 0;
        let mut i = 0;
        while i < ES.len() {
            let (u, plain, shifted, letter) = ES[i];
            if u == usage {
                if letter {
                    let caps = locks & LOCK_CAPS != 0;
                    return match plain {
                        Lv::C(c) if shift != caps => Typed::Char(upper(c)),
                        _ => level(plain),
                    };
                }
                return level(if shift { shifted } else { plain });
            }
            i += 1;
        }
    }
    match printable(usage, modifiers, locks) {
        Some(b) => Typed::Char(b as char),
        None => Typed::None,
    }
}

/// The accent a dead key types on its own (followed by Space, or by a key
/// it does not compose with).
pub const fn spacing(dead: Dead) -> char {
    match dead {
        Dead::Grave => '`',
        Dead::Acute => '´',
        Dead::Circumflex => '^',
        Dead::Diaeresis => '¨',
    }
}

/// The character a dead key and a base compose to, if any.
pub const fn compose(dead: Dead, base: char) -> Option<char> {
    let mut i = 0;
    while i < COMPOSE.len() {
        let (d, b, r) = COMPOSE[i];
        if d as u8 == dead as u8 && b == base {
            return Some(r);
        }
        i += 1;
    }
    None
}

/// The printable ASCII a key press types in `layout`: [`typed_in`]'s
/// character when it is ASCII, a dead key's accent when that is ASCII (no
/// composing), else nothing. In US this is `printable`, for every input.
pub const fn ascii_in(layout: u8, usage: u8, modifiers: u8, locks: u8) -> Option<u8> {
    let c = match typed_in(layout, usage, modifiers, locks) {
        Typed::None => return None,
        Typed::Char(c) => c,
        Typed::Dead(d) => spacing(d),
    };
    if c as u32 >= 0x20 && (c as u32) < 0x7f {
        Some(c as u8)
    } else {
        None
    }
}

/// Up to two characters a key press adds to the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Out {
    pub chars: [char; 2],
    pub len: u8,
}

impl Out {
    pub const NONE: Out = Out {
        chars: ['\0'; 2],
        len: 0,
    };

    const fn one(c: char) -> Out {
        Out {
            chars: [c, '\0'],
            len: 1,
        }
    }

    const fn two(a: char, b: char) -> Out {
        Out {
            chars: [a, b],
            len: 2,
        }
    }

    pub fn as_slice(&self) -> &[char] {
        &self.chars[..self.len as usize]
    }
}

/// Dead-key composition over a run of key presses. A dead key waits for the
/// next printable key: with a base it composes with, the composed character;
/// with Space, its accent; with any other character or dead key, its accent
/// then that key's output (a second dead key is typed as its accent too). A
/// key that types nothing (Enter, an arrow) drops a waiting dead key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Composer {
    pending: Option<Dead>,
}

impl Composer {
    /// Whether a dead key is waiting.
    pub const fn pending(&self) -> Option<Dead> {
        self.pending
    }

    /// Feed one key press's [`Typed`]; what it adds to the text.
    pub fn feed(&mut self, typed: Typed) -> Out {
        let waiting = self.pending.take();
        match (waiting, typed) {
            (_, Typed::None) => Out::NONE,
            (None, Typed::Dead(d)) => {
                self.pending = Some(d);
                Out::NONE
            }
            (None, Typed::Char(c)) => Out::one(c),
            (Some(w), Typed::Dead(d)) => Out::two(spacing(w), spacing(d)),
            (Some(w), Typed::Char(' ')) => Out::one(spacing(w)),
            (Some(w), Typed::Char(c)) => match compose(w, c) {
                Some(r) => Out::one(r),
                None => Out::two(spacing(w), c),
            },
        }
    }
}
