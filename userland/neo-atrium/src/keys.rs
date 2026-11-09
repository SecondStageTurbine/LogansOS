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
/// Home view does not use. With `keyboard-layouts`, in the session's layout
/// ([`set_layout`]).
pub fn decode(event: [u8; 8]) -> Option<Key> {
    #[cfg(feature = "keyboard-layouts")]
    return decode_in(layout(), event);
    #[cfg(not(feature = "keyboard-layouts"))]
    decode_us(event)
}

/// F11 S5: the session's keyboard layout, the settings store's
/// `KeyboardLayout` (0: US). The WM sets it when the setting applies and
/// hands it to its clients at session setup; every WM decoder reads it.
#[cfg(feature = "keyboard-layouts")]
static LAYOUT: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

/// Set the session's layout. An id that is not a layout is refused
/// (`false`) and the layout stays.
#[cfg(feature = "keyboard-layouts")]
pub fn set_layout(layout: u8) -> bool {
    if !keymap::layout_valid(layout) {
        return false;
    }
    LAYOUT.store(layout, core::sync::atomic::Ordering::Relaxed);
    true
}

/// The session's layout id.
#[cfg(feature = "keyboard-layouts")]
pub fn layout() -> u8 {
    LAYOUT.load(core::sync::atomic::Ordering::Relaxed)
}

#[cfg(feature = "keyboard-layouts")]
pub use keymap::{Composer, Dead, LAYOUT_COUNT, LAYOUT_ES, LAYOUT_NAMES, LAYOUT_US, Out, Typed};

/// [`decode`] in `layout`, whatever the session's. The text is ASCII: a key
/// whose legend is not ASCII types nothing, and a dead key types its accent
/// only when that is ASCII, without composing (`keymap_layout_logic.rs`'s
/// `ascii_in`; the Home field and Arthur's line are ASCII).
#[cfg(feature = "keyboard-layouts")]
pub fn decode_in(layout: u8, event: [u8; 8]) -> Option<Key> {
    decode_with(event, |usage, modifiers, locks| {
        keymap::ascii_in(layout, usage, modifiers, locks)
    })
}

/// What a pressed key types in `layout`, with dead keys and every legend
/// (`None` for a release or a non-key event): the Keyboard page's test
/// field, which composes them ([`Composer`]).
#[cfg(feature = "keyboard-layouts")]
pub fn typed_in(layout: u8, event: [u8; 8]) -> Option<Typed> {
    if event[0] != 1 || event[1] & 1 == 0 {
        return None;
    }
    let usage = u8::try_from(u16::from_le_bytes([event[2], event[3]])).ok()?;
    let modifiers = u16::from_le_bytes([event[4], event[5]]) as u8;
    let locks = u16::from_le_bytes([event[6], event[7]]) as u8;
    Some(keymap::typed_in(
        layout,
        keymap::effective_usage(usage, modifiers, locks),
        modifiers,
        locks,
    ))
}

/// The decoder without layouts: the US table. With `keyboard-layouts` it is
/// the kill-switch test's reference (`decode_in(LAYOUT_US, e)` equals it for
/// every event).
#[cfg_attr(feature = "keyboard-layouts", allow(dead_code))]
pub(crate) fn decode_us(event: [u8; 8]) -> Option<Key> {
    decode_with(event, keymap::printable)
}

fn decode_with(event: [u8; 8], printable: impl Fn(u8, u8, u8) -> Option<u8>) -> Option<Key> {
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
        _ => Key::Char(printable(usage, modifiers, locks)?),
    })
}

/// The shared US layout (`kernel/src/drivers/keymap_us_logic.rs`): the same
/// table the kernel and Arthur read. With `keyboard-layouts`, the layouts by
/// id too (`keymap_layout_logic.rs`), as Arthur reads them.
#[allow(dead_code)]
mod keymap {
    include!("../../../kernel/src/drivers/keymap_us_logic.rs");
    #[cfg(feature = "keyboard-layouts")]
    include!("../../../kernel/src/drivers/keymap_layout_logic.rs");
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
