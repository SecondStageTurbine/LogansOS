// SPDX-License-Identifier: MPL-2.0
//! The key encoder (THINK L1a §5.3): a key press (USB HID usage on the
//! Keyboard/Keypad page plus the HID modifier byte, as the kernel's
//! `InputEventV1` carries them) → the bytes xterm sends, from xterm's
//! `ctlseqs.txt` (fetched raw; the THINK records the snapshot and lines).
//!
//! - Cursor keys and Home/End: `CSI x`, or `SS3 x` under DECCKM; with a
//!   modifier, `CSI 1 ; m x` in both modes.
//! - Insert, Delete, PageUp, PageDown, F5-F12: `CSI n ~`; with a modifier,
//!   `CSI n ; m ~`.
//! - F1-F4: `SS3 P..S`; with a modifier, `CSI 1 ; m P..S`.
//! - The modifier parameter is 1 + Shift (1) + Alt (2) + Ctrl (4).
//! - Ctrl+letter is the C0 byte (`letter AND 1f`); Ctrl+/ is `1f`.
//! - Backspace is DEL (`7f`), Enter CR, Tab HT, Esc ESC.
//! - Printable keys follow the US layout the WM's own decoder uses.
//!
//! Ctrl+Shift+C is the terminal's End process (§4), never encoded.

/// HID modifier byte bits.
pub const MOD_LCTRL: u8 = 0x01;
pub const MOD_LSHIFT: u8 = 0x02;
pub const MOD_LALT: u8 = 0x04;
pub const MOD_RCTRL: u8 = 0x10;
pub const MOD_RSHIFT: u8 = 0x20;
pub const MOD_RALT: u8 = 0x40;

/// HID usages the encoder knows by name.
pub mod usage {
    pub const A: u8 = 0x04;
    pub const C: u8 = 0x06;
    pub const Z: u8 = 0x1d;
    pub const ENTER: u8 = 0x28;
    pub const ESCAPE: u8 = 0x29;
    pub const BACKSPACE: u8 = 0x2a;
    pub const TAB: u8 = 0x2b;
    pub const SLASH: u8 = 0x38;
    pub const F1: u8 = 0x3a;
    pub const F12: u8 = 0x45;
    pub const INSERT: u8 = 0x49;
    pub const HOME: u8 = 0x4a;
    pub const PAGE_UP: u8 = 0x4b;
    pub const DELETE: u8 = 0x4c;
    pub const END: u8 = 0x4d;
    pub const PAGE_DOWN: u8 = 0x4e;
    pub const RIGHT: u8 = 0x4f;
    pub const LEFT: u8 = 0x50;
    pub const DOWN: u8 = 0x51;
    pub const UP: u8 = 0x52;
}

/// At most `CSI 2 4 ; 8 ~` (7 bytes).
pub const MAX_ENCODED: usize = 8;

/// What a key press becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoded {
    /// Bytes for the child (through the line discipline).
    Bytes { buf: [u8; MAX_ENCODED], len: u8 },
    /// The terminal's End process (Ctrl+Shift+C).
    EndProcess,
    /// A key the terminal doesn't send.
    Nothing,
}

impl Encoded {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Encoded::Bytes { buf, len } => &buf[..*len as usize],
            _ => &[],
        }
    }

    fn from(bytes: &[u8]) -> Self {
        let mut buf = [0u8; MAX_ENCODED];
        buf[..bytes.len()].copy_from_slice(bytes);
        Encoded::Bytes {
            buf,
            len: bytes.len() as u8,
        }
    }
}

const ESC: u8 = 0x1b;

fn shift(mods: u8) -> bool {
    mods & (MOD_LSHIFT | MOD_RSHIFT) != 0
}
fn alt(mods: u8) -> bool {
    mods & (MOD_LALT | MOD_RALT) != 0
}
fn ctrl(mods: u8) -> bool {
    mods & (MOD_LCTRL | MOD_RCTRL) != 0
}

/// xterm's modifier parameter, or `None` with no Shift, Alt or Ctrl.
fn modifier_param(mods: u8) -> Option<u8> {
    let m = shift(mods) as u8 | (alt(mods) as u8) << 1 | (ctrl(mods) as u8) << 2;
    (m != 0).then_some(1 + m)
}

/// Encode one key press. `decckm` is the child's application-cursor mode.
pub fn encode(usage: u8, mods: u8, decckm: bool) -> Encoded {
    use usage::*;
    let m = modifier_param(mods);
    // Cursor keys and Home/End.
    let cursor = match usage {
        UP => Some(b'A'),
        DOWN => Some(b'B'),
        RIGHT => Some(b'C'),
        LEFT => Some(b'D'),
        HOME => Some(b'H'),
        END => Some(b'F'),
        _ => None,
    };
    if let Some(fin) = cursor {
        return match m {
            None if decckm => Encoded::from(&[ESC, b'O', fin]),
            None => Encoded::from(&[ESC, b'[', fin]),
            Some(m) => Encoded::from(&[ESC, b'[', b'1', b';', b'0' + m, fin]),
        };
    }
    // The editing keypad and F5-F12: CSI n ~.
    let tilde: Option<&[u8]> = match usage {
        INSERT => Some(b"2"),
        DELETE => Some(b"3"),
        PAGE_UP => Some(b"5"),
        PAGE_DOWN => Some(b"6"),
        0x3e => Some(b"15"), // F5
        0x3f => Some(b"17"), // F6
        0x40 => Some(b"18"), // F7
        0x41 => Some(b"19"), // F8
        0x42 => Some(b"20"), // F9
        0x43 => Some(b"21"), // F10
        0x44 => Some(b"23"), // F11
        0x45 => Some(b"24"), // F12
        _ => None,
    };
    if let Some(n) = tilde {
        let mut out = [0u8; MAX_ENCODED];
        let mut len = 0;
        for &b in [ESC, b'['].iter().chain(n) {
            out[len] = b;
            len += 1;
        }
        if let Some(m) = m {
            out[len] = b';';
            out[len + 1] = b'0' + m;
            len += 2;
        }
        out[len] = b'~';
        return Encoded::from(&out[..len + 1]);
    }
    // F1-F4: SS3 P..S, or CSI 1 ; m P..S with a modifier.
    if (F1..F1 + 4).contains(&usage) {
        let fin = b'P' + (usage - F1);
        return match m {
            None => Encoded::from(&[ESC, b'O', fin]),
            Some(m) => Encoded::from(&[ESC, b'[', b'1', b';', b'0' + m, fin]),
        };
    }
    match usage {
        ENTER => return Encoded::from(b"\r"),
        TAB => return Encoded::from(b"\t"),
        ESCAPE => return Encoded::from(&[ESC]),
        BACKSPACE => return Encoded::from(&[0x7f]),
        _ => {}
    }
    if ctrl(mods) {
        if usage == C && shift(mods) {
            return Encoded::EndProcess;
        }
        if (A..=Z).contains(&usage) {
            return Encoded::from(&[(b'a' + (usage - A)) & 0x1f]);
        }
        if usage == SLASH {
            return Encoded::from(&[0x1f]);
        }
        return Encoded::Nothing;
    }
    match printable(usage, shift(mods)) {
        Some(c) => Encoded::from(&[c]),
        None => Encoded::Nothing,
    }
}

/// The US layout, as the WM's Home decoder has it (`neo-atrium` `keys.rs`).
fn printable(usage: u8, shifted: bool) -> Option<u8> {
    const DIGITS_SHIFTED: &[u8; 10] = b"!@#$%^&*()";
    Some(match usage {
        0x04..=0x1d => {
            let c = b'a' + (usage - 0x04);
            if shifted { c - 32 } else { c }
        }
        0x1e..=0x27 => {
            let i = (usage - 0x1e) as usize;
            if shifted {
                DIGITS_SHIFTED[i]
            } else {
                b"1234567890"[i]
            }
        }
        0x2c => b' ',
        0x2d..=0x38 if usage != 0x32 => {
            const PLAIN: &[u8; 12] = b"-=[]\\?;'`,./";
            const SHIFT: &[u8; 12] = b"_+{}|?:\"~<>?";
            let i = (usage - 0x2d) as usize;
            if shifted { SHIFT[i] } else { PLAIN[i] }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::usage::*;
    use super::*;

    fn bytes(usage: u8, mods: u8, decckm: bool) -> Vec<u8> {
        encode(usage, mods, decckm).as_bytes().to_vec()
    }

    const CTRL: u8 = MOD_LCTRL;
    const SHIFT: u8 = MOD_LSHIFT;

    /// THINK §5.3, the key-encoder table, row by row.
    #[test]
    fn the_think_table() {
        let rows: &[(u8, u8, &[u8], &[u8])] = &[
            // key, mods, normal, DECCKM set
            (UP, 0, b"\x1b[A", b"\x1bOA"),
            (DOWN, 0, b"\x1b[B", b"\x1bOB"),
            (RIGHT, 0, b"\x1b[C", b"\x1bOC"),
            (LEFT, 0, b"\x1b[D", b"\x1bOD"),
            (HOME, 0, b"\x1b[H", b"\x1bOH"),
            (END, 0, b"\x1b[F", b"\x1bOF"),
            (INSERT, 0, b"\x1b[2~", b"\x1b[2~"),
            (DELETE, 0, b"\x1b[3~", b"\x1b[3~"),
            (PAGE_UP, 0, b"\x1b[5~", b"\x1b[5~"),
            (PAGE_DOWN, 0, b"\x1b[6~", b"\x1b[6~"),
            (F1, 0, b"\x1bOP", b"\x1bOP"),
            (F1 + 1, 0, b"\x1bOQ", b"\x1bOQ"),
            (F1 + 2, 0, b"\x1bOR", b"\x1bOR"),
            (F1 + 3, 0, b"\x1bOS", b"\x1bOS"),
            (F1 + 4, 0, b"\x1b[15~", b"\x1b[15~"),
            (F12, 0, b"\x1b[24~", b"\x1b[24~"),
            (F1 + 4, SHIFT, b"\x1b[15;2~", b"\x1b[15;2~"),
            (RIGHT, CTRL, b"\x1b[1;5C", b"\x1b[1;5C"),
            (UP, CTRL, b"\x1b[1;5A", b"\x1b[1;5A"),
            (DOWN, CTRL, b"\x1b[1;5B", b"\x1b[1;5B"),
            (LEFT, CTRL, b"\x1b[1;5D", b"\x1b[1;5D"),
            (BACKSPACE, 0, b"\x7f", b"\x7f"),
            (ENTER, 0, b"\r", b"\r"),
            (TAB, 0, b"\t", b"\t"),
            (ESCAPE, 0, b"\x1b", b"\x1b"),
            (SLASH, CTRL, b"\x1f", b"\x1f"),
        ];
        for &(key, mods, normal, app) in rows {
            assert_eq!(
                bytes(key, mods, false),
                normal,
                "usage {key:#x} mods {mods:#x}"
            );
            assert_eq!(
                bytes(key, mods, true),
                app,
                "usage {key:#x} mods {mods:#x} DECCKM"
            );
        }
    }

    /// Every Ctrl+letter is its C0 byte, `01` … `1a`.
    #[test]
    fn every_ctrl_letter() {
        for (i, key) in (A..=Z).enumerate() {
            assert_eq!(bytes(key, CTRL, false), [i as u8 + 1]);
        }
        // The right Ctrl too.
        assert_eq!(bytes(A, MOD_RCTRL, false), [0x01]);
    }

    /// kibi's bindings (L1b survey §3.4): Ctrl+Q/S/F/G/O/X/C/V/D/E/R/L/H.
    #[test]
    fn kibis_ctrl_keys() {
        let letters = b"qsfgoxcvderlh";
        let want = [
            0x11, 0x13, 0x06, 0x07, 0x0f, 0x18, 0x03, 0x16, 0x04, 0x05, 0x12, 0x0c, 0x08,
        ];
        for (&l, &w) in letters.iter().zip(want.iter()) {
            assert_eq!(bytes(A + (l - b'a'), CTRL, false), [w]);
        }
    }

    #[test]
    fn modifier_parameters() {
        assert_eq!(bytes(UP, SHIFT, false), b"\x1b[1;2A");
        assert_eq!(bytes(UP, MOD_LALT, false), b"\x1b[1;3A");
        assert_eq!(bytes(UP, SHIFT | MOD_LALT, false), b"\x1b[1;4A");
        assert_eq!(bytes(UP, CTRL | SHIFT | MOD_LALT, false), b"\x1b[1;8A");
        // A modified F1 switches SS3 to CSI.
        assert_eq!(bytes(F1, CTRL, false), b"\x1b[1;5P");
        assert_eq!(bytes(DELETE, MOD_RSHIFT, false), b"\x1b[3;2~");
        assert_eq!(bytes(F12, CTRL | SHIFT | MOD_LALT, false), b"\x1b[24;8~");
    }

    #[test]
    fn printable_us_layout() {
        assert_eq!(bytes(A, 0, false), b"a");
        assert_eq!(bytes(A, SHIFT, false), b"A");
        assert_eq!(bytes(0x1e, 0, false), b"1");
        assert_eq!(bytes(0x1e, SHIFT, false), b"!");
        assert_eq!(bytes(0x2c, 0, false), b" ");
        assert_eq!(bytes(0x31, 0, false), b"\\");
        assert_eq!(bytes(0x31, SHIFT, false), b"|");
        assert_eq!(bytes(SLASH, 0, false), b"/");
        assert_eq!(bytes(SLASH, SHIFT, false), b"?");
        assert_eq!(encode(0x32, 0, false), Encoded::Nothing);
    }

    #[test]
    fn ctrl_shift_c_is_end_process_and_never_bytes() {
        assert_eq!(encode(C, CTRL | SHIFT, false), Encoded::EndProcess);
        assert_eq!(encode(C, MOD_RCTRL | MOD_RSHIFT, true), Encoded::EndProcess);
        assert_eq!(bytes(C, CTRL, false), [0x03]);
    }

    #[test]
    fn unknown_keys_send_nothing() {
        assert_eq!(encode(0x00, 0, false), Encoded::Nothing);
        assert_eq!(encode(0xe0, 0, false), Encoded::Nothing); // a modifier key itself
        assert_eq!(encode(0x1e, CTRL, false), Encoded::Nothing); // Ctrl+1
    }

    #[test]
    fn nothing_encodes_longer_than_max() {
        for usage in 0..=0xff {
            for mods in [0, SHIFT, CTRL, MOD_LALT, CTRL | SHIFT | MOD_LALT] {
                for decckm in [false, true] {
                    assert!(encode(usage, mods, decckm).as_bytes().len() <= MAX_ENCODED);
                }
            }
        }
    }
}
