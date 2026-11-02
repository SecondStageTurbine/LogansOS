// SPDX-License-Identifier: MPL-2.0
//! DS-1 Home: the WM's own keyboard decoding. The WM reads keys only from its
//! OWN input focus (while Home holds the keyboard), never a client's, so the
//! WM-blind property of ADR-074 is kept. Events are `INPUT_EVENT_V1`
//! (8 bytes): kind 1 = key, flags bit 0 = pressed, a USB HID usage code
//! (Keyboard/Keypad page), the modifier bits (bit 1/5 = left/right shift) and
//! the lock state (`y`: Caps Lock bit 0, Num Lock OFF bit 1).

/// A key the Home view acts on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    /// A printable ASCII character (0x20..=0x7e).
    Char(u8),
    Enter,
    Backspace,
    Escape,
    Tab,
    Up,
    Down,
}

/// Decode one input event; `None` for releases, non-key events and keys the
/// Home view does not use.
pub fn decode(event: [u8; 8]) -> Option<Key> {
    if event[0] != 1 || event[1] & 1 == 0 {
        return None;
    }
    let code = u16::from_le_bytes([event[2], event[3]]);
    let modifiers = u16::from_le_bytes([event[4], event[5]]) as u8;
    let locks = u16::from_le_bytes([event[6], event[7]]) as u8;
    // A keypad key in navigation mode (Num Lock off) acts as its arrow.
    let usage = keymap::effective_usage(u8::try_from(code).ok()?, modifiers, locks);
    Some(match usage {
        0x28 | keymap::USAGE_KP_ENTER => Key::Enter,
        0x29 => Key::Escape,
        0x2a => Key::Backspace,
        0x2b => Key::Tab,
        0x52 => Key::Up,
        0x51 => Key::Down,
        _ => Key::Char(keymap::printable(usage, modifiers, locks)?),
    })
}

/// The shared US layout (`kernel/src/drivers/keymap_us_logic.rs`): the same
/// table the kernel and Arthur read.
#[allow(dead_code)]
mod keymap {
    include!("../../../kernel/src/drivers/keymap_us_logic.rs");
}

/// The intent field's line editor: printable ASCII only, bounded.
#[derive(Clone, Debug)]
pub struct Line<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Default for Line<N> {
    fn default() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }
}

impl<const N: usize> Line<N> {
    /// Apply an editing key; `true` when the text changed.
    pub fn edit(&mut self, key: Key) -> bool {
        match key {
            Key::Char(c) if self.len < N => {
                self.bytes[self.len] = c;
                self.len += 1;
                true
            }
            Key::Backspace if self.len > 0 => {
                self.len -= 1;
                true
            }
            _ => false,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// The text (always ASCII, so always valid UTF-8).
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Clear and zero every byte (OT-66 L1: the field held a password).
    pub fn wipe(&mut self) {
        for byte in &mut self.bytes {
            // SAFETY: a valid, aligned `&mut u8`; volatile so the zeroing
            // is not elided as a dead store.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.len = 0;
    }
}
