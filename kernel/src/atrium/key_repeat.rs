// SPDX-License-Identifier: MPL-2.0
//! Kernel side of key auto-repeat (`key-autorepeat`): the clock and focus
//! epoch [`KeyRepeat`] needs, and the counters the screen shows. Called only
//! from the keyboard pumps (syscall context, under each driver's own lock),
//! never from an ISR: this takes no lock and touches only atomics.

use core::sync::atomic::{AtomicU64, Ordering};

use super::input_logic::{INPUT_EVENT_KEY, InputEventV1, KEY_PRESSED};
use super::key_repeat_logic::KeyRepeat;

/// Repeats generated, every keyboard.
static REPEATS: AtomicU64 = AtomicU64::new(0);
/// Typematic makes the i8042 decoder dropped (the keyboard's own repeat).
static TYPEMATIC: AtomicU64 = AtomicU64::new(0);

/// The timer tick's period (100 Hz nominal), as `futex` and `wait_any` use it.
pub const NS_PER_TICK: u64 = 10_000_000;

/// The monotonic clock, from the timer tick. A pump reads it once and passes
/// the same value to [`on_event`] and [`poll`].
pub fn now_ns() -> u64 {
    crate::arch::x86_64::timer::TICKS
        .load(Ordering::Relaxed)
        .saturating_mul(NS_PER_TICK)
}

/// A key event the focus is being given (the SAK filter passed it), at
/// `now_ns` ([`now_ns`]).
pub fn on_event(repeat: &mut KeyRepeat, event: InputEventV1, now_ns: u64) {
    if event.kind != INPUT_EVENT_KEY || event.code > 0xff {
        return;
    }
    // The scenario's control: nothing ever arms, so nothing repeats.
    if cfg!(feature = "neo-test-ps2-key-repeat-control") {
        return;
    }
    repeat.on_key(
        event.code as u8,
        event.x as u8,
        event.flags & KEY_PRESSED != 0,
        now_ns,
        super::focus::activation_epoch(),
    );
    if event.flags & KEY_PRESSED != 0 {
        repeat.on_press_locks(event.y);
    }
}

/// The repeat due at `now_ns`, if any.
pub fn poll(repeat: &mut KeyRepeat, now_ns: u64) -> Option<InputEventV1> {
    let event = repeat.poll(now_ns, super::focus::activation_epoch())?;
    REPEATS.fetch_add(1, Ordering::Relaxed);
    Some(event)
}

pub fn note_typematic(n: u64) {
    if n != 0 {
        TYPEMATIC.fetch_add(n, Ordering::Relaxed);
    }
}

/// `(repeats generated, typematic makes dropped)`.
pub fn counters() -> (u64, u64) {
    (
        REPEATS.load(Ordering::Relaxed),
        TYPEMATIC.load(Ordering::Relaxed),
    )
}
