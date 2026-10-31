// SPDX-License-Identifier: MPL-2.0
//! The fixed secure-attention (SAK) chord recognizer (DI-1, Jarvis F1/F5).
//!
//! Pure and syscall-free (mounted by the host KATs together with the real
//! boot-keyboard decoder). It runs on the decoded key events of
//! `BootKeyEventDecoder`, which emits an event only when a NON-modifier
//! keycode changes. That fixes the chord policy:
//!
//! - **Modifier-first only.** A chord is Super held, then one of
//!   [`SAK_KEYS`] pressed. If the key goes down first, its press has already
//!   been delivered to the client and pressing Super later cannot (and does
//!   not) turn it into a chord; the key's release then passes too, so the
//!   client never sees a press without its release.
//! - **Swallow-release.** A chord press is never delivered; neither is the
//!   release of that key, in whatever order the keys come up (Super first
//!   included). One bit per keycode.
//! - **Fixed.** Widening the table is a kernel change through the AAG; no
//!   principal can configure it.
//!
//! A chord is delivered through the kernel's coalescing command latch
//! (`focus::latch_sak`), never through the best-effort input queues.

use super::input_logic::{INPUT_EVENT_KEY, InputEventV1, KEY_PRESSED};

/// HID boot modifier bits for left/right GUI ("Super").
pub const MOD_SUPER: u8 = 0x08 | 0x80;
pub const KEY_ESCAPE: u8 = 0x29;
pub const KEY_TAB: u8 = 0x2b;
pub const KEY_SPACE: u8 = 0x2c;
/// OT-66 L2: Super+L locks the session (`sak-lock`).
#[cfg(feature = "sak-lock")]
pub const KEY_L: u8 = 0x0f;
/// Super + one of these: next window, summon the Spine, escape.
#[cfg(not(feature = "sak-lock"))]
pub const SAK_KEYS: [u8; 3] = [KEY_TAB, KEY_SPACE, KEY_ESCAPE];
/// Super + one of these: next window, summon the Spine, escape, lock.
#[cfg(feature = "sak-lock")]
pub const SAK_KEYS: [u8; 4] = [KEY_TAB, KEY_SPACE, KEY_ESCAPE, KEY_L];

/// OT-66 L2 (`sak-lock`): the latch bit for "a key or pointer event was
/// produced since the last take" (`focus::note_activity`). Not a chord: the
/// WM's idle timer restarts on it.
#[cfg(feature = "sak-lock")]
pub const ACTIVITY_BIT: u8 = 0x10;

/// The latch bit for a chord key (`0` for anything else).
pub const fn sak_bit(key: u8) -> u8 {
    match key {
        KEY_TAB => 1,
        KEY_SPACE => 2,
        KEY_ESCAPE => 4,
        #[cfg(feature = "sak-lock")]
        KEY_L => 8,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SakVerdict {
    /// Not a chord: route to the keyboard focus as before.
    Pass,
    /// A chord press, carrying its key: latch it, deliver nothing.
    Chord(u8),
    /// The release of a swallowed press: deliver nothing.
    Swallow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SakFilter {
    swallowed: [u64; 4],
}

impl SakFilter {
    pub const fn new() -> Self {
        Self { swallowed: [0; 4] }
    }

    pub fn filter(&mut self, event: InputEventV1) -> SakVerdict {
        if event.kind != INPUT_EVENT_KEY || event.code > 0xff {
            return SakVerdict::Pass;
        }
        let key = event.code as u8;
        let (word, bit) = ((key / 64) as usize, 1u64 << (key % 64));
        let pressed = event.flags & KEY_PRESSED != 0;
        let modifiers = event.x as u8;
        if pressed && modifiers & MOD_SUPER != 0 && sak_bit(key) != 0 {
            self.swallowed[word] |= bit;
            return SakVerdict::Chord(key);
        }
        if !pressed && self.swallowed[word] & bit != 0 {
            self.swallowed[word] &= !bit;
            return SakVerdict::Swallow;
        }
        SakVerdict::Pass
    }
}

/// OT-66 L2b (`sak-lock-route`): the transitions of the kernel's lock route
/// word (`focus::LOCK_ROUTE`). Bits 63..1 hold the lock focus's id (0: none;
/// focus ids start at 1 and never reach 2^63), bit 0 says locked.
#[cfg(feature = "sak-lock-route")]
pub mod lock_word {
    pub const ENGAGED: u64 = 1;

    /// The registered lock focus (0: none).
    pub const fn id(word: u64) -> u64 {
        word >> 1
    }

    /// The route input goes to: the lock focus while locked, else `active`.
    pub const fn route(word: u64, active: u64) -> u64 {
        if word & ENGAGED != 0 {
            id(word)
        } else {
            active
        }
    }

    /// Lock: only with a lock focus registered, and only once.
    pub const fn engage(word: u64) -> Option<u64> {
        if id(word) != 0 && word & ENGAGED == 0 {
            Some(word | ENGAGED)
        } else {
            None
        }
    }

    /// Register `focus` (a WM restart replaces it), keeping the locked bit.
    pub const fn register(word: u64, focus: u64) -> u64 {
        focus << 1 | word & ENGAGED
    }

    /// Unlock (the caller has made the target the active route first).
    pub const fn release(word: u64) -> u64 {
        word & !ENGAGED
    }

    /// May a pointer record go to focus `target`? Unlocked, any; locked,
    /// only the lock focus.
    pub const fn admits(word: u64, target: u64) -> bool {
        word & ENGAGED == 0 || id(word) == target
    }

    /// Focus `dying` drops: if it is the lock focus, nothing stays
    /// registered (`Some(0)`); the caller fails a locked session closed.
    pub const fn dropped(word: u64, dying: u64) -> Option<u64> {
        if id(word) == dying { Some(0) } else { None }
    }
}
