// SPDX-License-Identifier: MPL-2.0
//! Software key auto-repeat (`key-autorepeat`): the state machine both
//! keyboard pumps (i8042 and the USB HID boot keyboard) run.
//!
//! **Why software.** A USB HID keyboard never repeats: it reports a held key
//! once, and the host makes the repeats. A PS/2 keyboard sends typematic
//! makes, but the i8042 decoder reports a held key once, like the HID boot
//! report, and drops those makes (it only counts them, as `typematic=`). So
//! this module is the one source of repeats for every keyboard, and nothing
//! repeats twice.
//!
//! **Policy.**
//! - The most recently pressed key that [`repeats`] is the repeat key. After
//!   [`DELAY_NS`] it repeats every [`INTERVAL_NS`].
//! - The repeat stops when that key is released, when any other key is
//!   pressed, when the modifiers change, when the keyboard focus moves (the
//!   activation epoch changes), and after [`MAX_HOLD_NS`], in case a break
//!   code was lost.
//! - Modifiers, the lock keys, Pause, Print Screen, Escape and Enter (both)
//!   never repeat. Enter is excluded because it submits an Ask, so holding it
//!   would queue several.
//! - A repeat is the key's press again with [`KEY_REPEAT`] set. Clients that
//!   test only `KEY_PRESSED` see an ordinary press.
//! - A modifier change cancels the repeat, so a repeat always carries the
//!   modifiers of the press the SAK filter passed. It can therefore never
//!   become a SAK chord, and every repeat run ends before the key's release.
//!
//! **Time.** The pump passes the monotonic tick clock in ns, so repeats land
//! on tick edges (10 ms) and the rate is correct on average. [`KeyRepeat::poll`]
//! emits at most one repeat per call and never catches up after a stall, so a
//! late pump does not deliver a burst.
//!
//! Pure and syscall-free: host-tested through `tools/atrium-logic-tests`.

use super::input_logic::{INPUT_EVENT_KEY, InputEventV1, KEY_PRESSED};

/// `flags` bit 1 of a key event: this press is an auto-repeat of a key that
/// is still held (always with `KEY_PRESSED`).
pub const KEY_REPEAT: u8 = 2;

/// Hold time before the first repeat.
pub const DELAY_NS: u64 = 500_000_000;
/// Time between repeats (30 per second).
pub const INTERVAL_NS: u64 = 1_000_000_000 / 30;
/// A repeat run ends after this long even if no release arrives.
pub const MAX_HOLD_NS: u64 = 20_000_000_000;

/// Keys that never repeat: Caps Lock, Scroll Lock, Num Lock, Pause, Print
/// Screen, Escape, Enter and keypad Enter (the modifiers 0xE0..=0xE7 never
/// repeat either).
const NO_REPEAT: [u8; 8] = [0x39, 0x47, 0x53, 0x48, 0x46, 0x29, 0x28, 0x58];

/// Whether a key (a USB HID Keyboard/Keypad usage) auto-repeats.
pub const fn repeats(usage: u8) -> bool {
    if usage == 0 || matches!(usage, 0xE0..=0xE7) {
        return false;
    }
    let mut i = 0;
    while i < NO_REPEAT.len() {
        if NO_REPEAT[i] == usage {
            return false;
        }
        i += 1;
    }
    true
}

/// Whether an input event is an auto-repeat.
pub const fn is_repeat(event: &InputEventV1) -> bool {
    event.kind == INPUT_EVENT_KEY && event.flags & KEY_REPEAT != 0
}

/// One keyboard's repeat state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyRepeat {
    /// The repeating key, 0 = none.
    key: u8,
    modifiers: u8,
    /// The press's lock state (`y`): a repeat types what the press typed.
    /// A lock key's press ends the run first, so it cannot change mid-run.
    locks: i16,
    /// The focus activation epoch the key was pressed under.
    epoch: u64,
    pressed_ns: u64,
    next_ns: u64,
}

impl KeyRepeat {
    pub const fn new() -> Self {
        Self {
            key: 0,
            modifiers: 0,
            locks: 0,
            epoch: 0,
            pressed_ns: 0,
            next_ns: 0,
        }
    }

    /// The key that will repeat, if any.
    pub const fn key(&self) -> Option<u8> {
        if self.key == 0 { None } else { Some(self.key) }
    }

    /// A non-modifier key event the focus was given. Call it only for events
    /// the SAK filter passed.
    pub fn on_key(&mut self, usage: u8, modifiers: u8, pressed: bool, now_ns: u64, epoch: u64) {
        if pressed {
            self.key = if repeats(usage) { usage } else { 0 };
            self.modifiers = modifiers;
            self.epoch = epoch;
            self.pressed_ns = now_ns;
            self.next_ns = now_ns.saturating_add(DELAY_NS);
        } else if usage == self.key {
            self.key = 0;
        }
    }

    /// The lock state the last press carried (call it after [`Self::on_key`]).
    pub fn on_press_locks(&mut self, locks: i16) {
        self.locks = locks;
    }

    /// The keyboard's current modifier byte. A change ends the repeat.
    pub fn on_modifiers(&mut self, modifiers: u8) {
        if modifiers != self.modifiers {
            self.key = 0;
        }
    }

    /// The repeat due at `now_ns`, if any (at most one per call).
    pub fn poll(&mut self, now_ns: u64, epoch: u64) -> Option<InputEventV1> {
        if self.key == 0 {
            return None;
        }
        if epoch != self.epoch || now_ns.saturating_sub(self.pressed_ns) >= MAX_HOLD_NS {
            self.key = 0;
            return None;
        }
        if now_ns < self.next_ns {
            return None;
        }
        self.next_ns = self.next_ns.saturating_add(INTERVAL_NS);
        if self.next_ns <= now_ns {
            // The pump was late: carry on from now, without a burst.
            self.next_ns = now_ns.saturating_add(INTERVAL_NS);
        }
        Some(InputEventV1 {
            kind: INPUT_EVENT_KEY,
            flags: KEY_PRESSED | KEY_REPEAT,
            code: self.key as u16,
            x: self.modifiers as i16,
            y: self.locks,
        })
    }
}
