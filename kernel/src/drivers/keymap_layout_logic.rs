// SPDX-License-Identifier: MPL-2.0
// F11 S5 (`keyboard-layouts`, setup-menu THINK §3.2, §5 S5): the keyboard
// layouts, selected by an id. US is `keymap_us_logic.rs`, unchanged; UK is
// the keys whose legends differ from US on a UK ISO keyboard, and every
// other key types what US types.
//
// No inner attributes or doc comments at the top, as in `keymap_us_logic.rs`:
// a decoder takes both files with `include!` into one module, and this one
// calls that file's `printable` by name.
//
// The output is printable ASCII (0x20..=0x7e), as for US: every decoder's
// text is ASCII (the WM's line editor, Arthur's grid). A legend outside ASCII
// (UK `£` on Shift+3 and `¬` on Shift+`) types nothing until text is wider
// than ASCII. AltGr is not a layout level here: as in US, the Alt keys do not
// change what a key types.
//
// Pure and dependency-free; host-tested by `tools/xtask/src/keymap_tests.rs`
// (every key x Shift x Caps Lock x Num Lock for both layouts, written out by
// hand from the keycap legends, and US by id equal to `printable` for every
// usage, modifier byte and lock state).

/// Layout id 0: US (the default, and the only layout without
/// `keyboard-layouts`). The id is the settings store's `KeyboardLayout`.
pub const LAYOUT_US: u8 = 0;
/// Layout id 1: UK (ISO).
pub const LAYOUT_UK: u8 = 1;
/// How many layouts exist: an id is below it.
pub const LAYOUT_COUNT: u8 = 2;

/// The names the Keyboard page shows, by id.
pub const LAYOUT_NAMES: [&str; LAYOUT_COUNT as usize] = ["English (US)", "English (UK)"];

/// The UK keys whose legends differ from US: usage, plain, shifted (0: the
/// legend is not ASCII, so the key types nothing). Usage 0x31 (US `\`) and
/// 0x32 (Non-US `#`) are the same key on a UK keyboard, the one left of
/// Enter (a keyboard sends one or the other; Linux maps both to one key).
/// Usage 0x64 (Non-US `\`) is the key left of Z, which a US keyboard lacks.
const UK: [(u8, u8, u8); 7] = [
    (0x1f, b'2', b'"'),
    (0x20, b'3', 0), // Shift: `£`
    (0x31, b'#', b'~'),
    (0x32, b'#', b'~'),
    (0x34, b'\'', b'@'),
    (0x35, b'`', 0), // Shift: `¬`
    (0x64, b'\\', b'|'),
];

/// Whether `layout` is a layout id ([`LAYOUT_COUNT`]).
pub const fn layout_valid(layout: u8) -> bool {
    layout < LAYOUT_COUNT
}

/// The printable character a key press types in `layout`, as `printable`
/// does for US. An id that is not a layout types US: a bad id must not leave
/// the keyboard dead.
pub const fn printable_in(layout: u8, usage: u8, modifiers: u8, locks: u8) -> Option<u8> {
    if layout == LAYOUT_UK {
        let shift = modifiers & MOD_SHIFT != 0;
        let mut i = 0;
        while i < UK.len() {
            let (u, plain, shifted) = UK[i];
            if u == usage {
                let c = if shift { shifted } else { plain };
                return if c == 0 { None } else { Some(c) };
            }
            i += 1;
        }
    }
    printable(usage, modifiers, locks)
}
