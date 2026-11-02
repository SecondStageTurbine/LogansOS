// SPDX-License-Identifier: MPL-2.0
// The US keyboard layout: one table from a USB HID Keyboard/Keypad usage
// (page 0x07) to the ASCII character it types. Every path that turns a key
// into text reads it: the kernel's legacy byte path (`usb::hid_logic`), the
// WM's Home decoder (`neo_atrium::keys`) and Arthur's (`showcase_logic`).
// The i8042 driver produces the same HID usages (`ps2::i8042_logic`), so
// PS/2 and USB keyboards share this table.
//
// No inner attributes or doc comments at the top: the decoders outside the
// kernel's module tree take this file with `include!`, which resolves the
// same from any file (a nested `#[path]` does not).
//
// Pure and dependency-free; host-tested by the KAT table in
// `tools/xtask/src/keymap_tests.rs` (every key x plain / Shift / Caps Lock /
// Caps+Shift, written out by hand from the US keycap legends).

/// HID modifier byte: left and right Shift.
pub const MOD_SHIFT: u8 = 0x02 | 0x20;

/// Lock-state bits, carried in a key event's `y` field (`InputEventV1`).
/// Zero is the default state: Caps Lock off, Num Lock on.
pub const LOCK_CAPS: u8 = 1 << 0;
/// Num Lock is OFF (inverted, so the boot default, Num Lock on, is 0 and a
/// client that ignores `y` sees the default).
pub const LOCK_NUM_OFF: u8 = 1 << 1;

/// HID usage of Caps Lock.
pub const USAGE_CAPS_LOCK: u8 = 0x39;
/// HID usage of Num Lock (keypad Num Lock and Clear).
pub const USAGE_NUM_LOCK: u8 = 0x53;
/// HID usage of keypad Enter (callers treat it as Enter).
pub const USAGE_KP_ENTER: u8 = 0x58;

/// The keypad's digit and period keys: usage, the character with Num Lock
/// on, and the navigation key (its HID usage) with Num Lock off. Keypad 5
/// has no navigation key (it is "Begin" on the keycap and types nothing).
const KEYPAD: [(u8, u8, u8); 11] = [
    (0x59, b'1', 0x4d), // End
    (0x5a, b'2', 0x51), // Down
    (0x5b, b'3', 0x4e), // Page Down
    (0x5c, b'4', 0x50), // Left
    (0x5d, b'5', 0x00), // (none)
    (0x5e, b'6', 0x4f), // Right
    (0x5f, b'7', 0x4a), // Home
    (0x60, b'8', 0x52), // Up
    (0x61, b'9', 0x4b), // Page Up
    (0x62, b'0', 0x49), // Insert
    (0x63, b'.', 0x4c), // Delete
];

/// The keypad operators: the same character whatever Shift and Num Lock.
const KEYPAD_OPERATORS: [(u8, u8); 4] = [(0x54, b'/'), (0x55, b'*'), (0x56, b'-'), (0x57, b'+')];

/// Whether a keypad digit/period key acts as its digit: Num Lock on, and
/// Shift inverts it (the PC convention: Shift+keypad 8 is Up with Num Lock
/// on, and 8 with Num Lock off).
const fn keypad_digits(modifiers: u8, locks: u8) -> bool {
    let num = locks & LOCK_NUM_OFF == 0;
    let shift = modifiers & MOD_SHIFT != 0;
    num != shift
}

/// The usage a key acts as: a keypad digit or period key in navigation mode
/// becomes its navigation key (keypad 5 becomes 0, no key); every other key
/// is itself. Callers match navigation keys (Up, Page Down, ...) on this.
pub const fn effective_usage(usage: u8, modifiers: u8, locks: u8) -> u8 {
    let mut i = 0;
    while i < KEYPAD.len() {
        let (u, _, nav) = KEYPAD[i];
        if u == usage {
            return if keypad_digits(modifiers, locks) {
                usage
            } else {
                nav
            };
        }
        i += 1;
    }
    usage
}

/// The keys whose two legends are not a letter's case: usage, plain, shifted.
/// Usage 0x32 (Non-US `#`) is not on a US keyboard and types nothing.
const SYMBOLS: [(u8, u8, u8); 21] = [
    (0x1e, b'1', b'!'),
    (0x1f, b'2', b'@'),
    (0x20, b'3', b'#'),
    (0x21, b'4', b'$'),
    (0x22, b'5', b'%'),
    (0x23, b'6', b'^'),
    (0x24, b'7', b'&'),
    (0x25, b'8', b'*'),
    (0x26, b'9', b'('),
    (0x27, b'0', b')'),
    (0x2d, b'-', b'_'),
    (0x2e, b'=', b'+'),
    (0x2f, b'[', b'{'),
    (0x30, b']', b'}'),
    (0x31, b'\\', b'|'),
    (0x33, b';', b':'),
    (0x34, b'\'', b'"'),
    (0x35, b'`', b'~'),
    (0x36, b',', b'<'),
    (0x37, b'.', b'>'),
    (0x38, b'/', b'?'),
];

/// The printable character (0x20..=0x7e) a key press types, or `None` for a
/// key that types none (modifiers, function and navigation keys, Enter,
/// keypad Enter, Backspace, Tab and Escape: callers handle those by usage,
/// after [`effective_usage`]).
///
/// Shift picks the upper legend. Caps Lock affects letters only (it is not
/// Shift Lock), and Shift with Caps Lock types a letter in lower case.
pub const fn printable(usage: u8, modifiers: u8, locks: u8) -> Option<u8> {
    let shift = modifiers & MOD_SHIFT != 0;
    if usage >= 0x04 && usage <= 0x1d {
        let lower = b'a' + (usage - 0x04);
        let caps = locks & LOCK_CAPS != 0;
        return Some(if shift != caps { lower - 32 } else { lower });
    }
    if usage == 0x2c {
        return Some(b' ');
    }
    let mut i = 0;
    while i < KEYPAD.len() {
        let (u, digit, _) = KEYPAD[i];
        if u == usage {
            return if keypad_digits(modifiers, locks) {
                Some(digit)
            } else {
                None
            };
        }
        i += 1;
    }
    let mut i = 0;
    while i < KEYPAD_OPERATORS.len() {
        let (u, c) = KEYPAD_OPERATORS[i];
        if u == usage {
            return Some(c);
        }
        i += 1;
    }
    let mut i = 0;
    while i < SYMBOLS.len() {
        let (u, plain, shifted) = SYMBOLS[i];
        if u == usage {
            return Some(if shift { shifted } else { plain });
        }
        i += 1;
    }
    None
}

/// The lock state after a key transition: each press of Caps Lock or Num
/// Lock toggles its bit.
pub const fn next_locks(locks: u8, usage: u8, pressed: bool) -> u8 {
    if !pressed {
        locks
    } else if usage == USAGE_CAPS_LOCK {
        locks ^ LOCK_CAPS
    } else if usage == USAGE_NUM_LOCK {
        locks ^ LOCK_NUM_OFF
    } else {
        locks
    }
}
