// SPDX-License-Identifier: MPL-2.0
//! The keyboard lock state (Caps Lock, Num Lock), one for the machine: every keyboard,
//! PS/2 or USB, toggles and reads the same state, as one person types on
//! them. Key events carry it in `y` (`keymap_us_logic::LOCK_*`), so a
//! client turns usages into text without tracking lock keys itself.
//!
//! The keyboards' LEDs are not driven: neither driver sends the PS/2 `0xED`
//! command or the HID SET_REPORT today, so the lights stay as firmware left
//! them.

use core::sync::atomic::{AtomicU8, Ordering};

use super::keymap_us_logic;

static LOCKS: AtomicU8 = AtomicU8::new(0);

/// Apply one key transition (non-modifier keys, as decoded) and return the
/// lock state to stamp on its event: the state after a lock key's press,
/// so the Caps Lock press itself already carries the new state.
pub fn on_key(usage: u8, pressed: bool) -> u8 {
    let mut current = LOCKS.load(Ordering::Relaxed);
    loop {
        let next = keymap_us_logic::next_locks(current, usage, pressed);
        if next == current {
            return current;
        }
        match LOCKS.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(seen) => current = seen,
        }
    }
}

/// The lock state now.
#[allow(dead_code)] // test hooks
pub fn current() -> u8 {
    LOCKS.load(Ordering::Relaxed)
}
