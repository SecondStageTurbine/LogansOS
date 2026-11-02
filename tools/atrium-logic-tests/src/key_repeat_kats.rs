// SPDX-License-Identifier: MPL-2.0
//! Host KATs for software key auto-repeat (`key_repeat_logic.rs`) and the
//! input queue's repeat rule. Every expectation is written out by hand (times
//! in ns, usages from the HID Keyboard/Keypad page, the wire bytes), not
//! derived from the code under test.

use super::input_logic::{InputEventV1, decode_event, encode_event};
use super::input_queue_logic::{Enqueued, InputQueue};
use super::key_repeat_logic::{KEY_REPEAT, KeyRepeat, is_repeat, repeats};

const MS: u64 = 1_000_000;
const T0: u64 = 1_000 * MS;
const EPOCH: u64 = 7;
const BACKSPACE: u8 = 0x2A;
const A: u8 = 0x04;
const B: u8 = 0x05;
const ESCAPE: u8 = 0x29;
const LSHIFT_BIT: u8 = 0x02;

fn held(usage: u8, modifiers: u8) -> KeyRepeat {
    let mut r = KeyRepeat::new();
    r.on_key(usage, modifiers, true, T0, EPOCH);
    r
}

/// Every poll every 10 ms (the tick) from T0 up to, not including, `end_ns`.
fn repeats_until(r: &mut KeyRepeat, end_ns: u64) -> Vec<u64> {
    (0..)
        .map(|i| T0 + i * 10 * MS)
        .take_while(|&t| t < end_ns)
        .filter(|&t| r.poll(t, EPOCH).is_some())
        .collect()
}

#[test]
fn wire_oracle_backspace_repeat_with_left_shift() {
    let mut r = held(BACKSPACE, LSHIFT_BIT);
    let event = r.poll(T0 + 500 * MS, EPOCH).expect("due at 500 ms");
    // kind 1 (key), flags 0x03 (pressed | repeat), code 0x002A LE,
    // x = modifiers 0x0002 LE, y = 0.
    let bytes = [0x01, 0x03, 0x2A, 0x00, 0x02, 0x00, 0x00, 0x00];
    assert_eq!(encode_event(event), bytes);
    assert_eq!(decode_event(bytes), event);
    assert_eq!(KEY_REPEAT, 0x02);
    // Clients that test only bit 0 see a press.
    assert_eq!(bytes[1] & 0x01, 0x01);
}

/// A repeat carries its press's lock state (`y`), so a letter held under
/// Caps Lock keeps typing capitals.
#[test]
fn wire_oracle_repeat_carries_the_press_lock_state() {
    let mut r = held(0x04, 0);
    r.on_press_locks(1);
    let event = r.poll(T0 + 500 * MS, EPOCH).expect("due at 500 ms");
    // kind 1, flags 0x03, code 0x0004 LE, x = 0, y = 0x0001 LE (Caps Lock).
    assert_eq!(
        encode_event(event),
        [0x01, 0x03, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00]
    );
}

#[test]
fn first_repeat_at_500_ms_then_every_33_ms() {
    let mut r = held(BACKSPACE, 0);
    assert_eq!(r.poll(T0, EPOCH), None);
    assert_eq!(r.poll(T0 + 499_999_999, EPOCH), None);
    assert!(r.poll(T0 + 500 * MS, EPOCH).is_some());
    assert_eq!(r.poll(T0 + 500 * MS, EPOCH), None, "one per due time");
    // 500 ms + 1 s / 30 = 533 333 333 ns.
    assert_eq!(r.poll(T0 + 533_333_332, EPOCH), None);
    assert!(r.poll(T0 + 533_333_333, EPOCH).is_some());
    assert_eq!(r.poll(T0 + 566_666_665, EPOCH), None);
    assert!(r.poll(T0 + 566_666_666, EPOCH).is_some());
}

#[test]
fn thirty_repeats_in_the_second_after_the_delay() {
    // Held 1.5 s, polled every tick: due at 500 + 33.33k ms for k = 0..=29
    // (k = 30 is 1500 ms, not before the release).
    let mut r = held(BACKSPACE, 0);
    let times = repeats_until(&mut r, T0 + 1_500 * MS);
    assert_eq!(times.len(), 30);
    assert_eq!(times[0], T0 + 500 * MS);
    // Each lands on the first tick at or after its due time.
    assert_eq!(times[1], T0 + 540 * MS);
    assert_eq!(times[29], T0 + 1_470 * MS);
}

#[test]
fn a_late_pump_gets_one_repeat_not_a_burst() {
    let mut r = held(BACKSPACE, 0);
    let late = T0 + 5_000 * MS;
    assert!(r.poll(late, EPOCH).is_some());
    assert_eq!(r.poll(late, EPOCH), None);
    assert_eq!(r.poll(late + 33 * MS, EPOCH), None);
    assert!(r.poll(late + 34 * MS, EPOCH).is_some());
}

#[test]
fn release_stops_the_repeat_and_other_releases_do_not() {
    let mut r = held(A, 0);
    assert!(r.poll(T0 + 600 * MS, EPOCH).is_some());
    r.on_key(B, 0, false, T0 + 610 * MS, EPOCH);
    assert!(r.poll(T0 + 640 * MS, EPOCH).is_some(), "B was not held");
    r.on_key(A, 0, false, T0 + 650 * MS, EPOCH);
    assert_eq!(r.key(), None);
    assert_eq!(repeats_until(&mut r, T0 + 3_000 * MS), Vec::<u64>::new());
}

#[test]
fn the_last_key_pressed_repeats_and_a_non_repeating_press_ends_it() {
    let mut r = held(A, 0);
    r.on_key(B, 0, true, T0 + 100 * MS, EPOCH);
    assert_eq!(r.key(), Some(B));
    // B's own delay: nothing before 600 ms, then B.
    assert_eq!(r.poll(T0 + 599 * MS, EPOCH), None);
    let event = r.poll(T0 + 600 * MS, EPOCH).expect("B repeats");
    assert_eq!(event.code, B as u16);
    // Releasing A (no longer the repeat key) changes nothing.
    r.on_key(A, 0, false, T0 + 610 * MS, EPOCH);
    assert_eq!(r.key(), Some(B));
    r.on_key(ESCAPE, 0, true, T0 + 620 * MS, EPOCH);
    assert_eq!(r.key(), None);
}

#[test]
fn which_keys_repeat() {
    for usage in 0xE0..=0xE7 {
        assert!(!repeats(usage), "modifier {usage:#04x}");
    }
    // Caps, Scroll, Num Lock, Pause, Print Screen, Escape, Enter, KP Enter.
    for usage in [0x39, 0x47, 0x53, 0x48, 0x46, 0x29, 0x28, 0x58, 0x00] {
        assert!(!repeats(usage), "{usage:#04x}");
    }
    // Letters, digits, Backspace, Tab, Space, Delete, the arrows, KP 4.
    for usage in [
        0x04, 0x1D, 0x1E, 0x27, 0x2A, 0x2B, 0x2C, 0x4C, 0x4F, 0x50, 0x51, 0x52, 0x5C,
    ] {
        assert!(repeats(usage), "{usage:#04x}");
    }
}

#[test]
fn a_modifier_press_never_arms_and_a_modifier_change_ends_the_repeat() {
    let r = held(0xE1, 0); // left Shift as a key event
    assert_eq!(r.key(), None);

    let mut r = held(A, LSHIFT_BIT);
    r.on_modifiers(LSHIFT_BIT);
    assert_eq!(r.key(), Some(A), "same modifiers");
    let event = r.poll(T0 + 500 * MS, EPOCH).expect("due");
    assert_eq!(event.x, LSHIFT_BIT as i16, "carries the press's modifiers");
    // Super (left GUI, 0x08) goes down: the repeat ends, so it can never
    // become a Super chord.
    r.on_modifiers(LSHIFT_BIT | 0x08);
    assert_eq!(r.key(), None);
    assert_eq!(r.poll(T0 + 600 * MS, EPOCH), None);
}

#[test]
fn a_focus_move_ends_the_repeat_for_good() {
    let mut r = held(A, 0);
    assert_eq!(r.poll(T0 + 500 * MS, EPOCH + 1), None);
    assert_eq!(r.key(), None);
    assert_eq!(
        r.poll(T0 + 600 * MS, EPOCH),
        None,
        "the epoch coming back does not revive it"
    );
}

#[test]
fn twenty_seconds_without_a_release_ends_the_repeat() {
    let mut r = held(A, 0);
    assert!(r.poll(T0 + 19_999 * MS, EPOCH).is_some());
    assert_eq!(r.poll(T0 + 20_000 * MS, EPOCH), None);
    assert_eq!(r.key(), None);
}

#[test]
fn only_a_repeat_is_a_repeat() {
    let mut r = held(A, 0);
    let repeat = r.poll(T0 + 500 * MS, EPOCH).expect("due");
    assert!(is_repeat(&repeat));
    assert!(!is_repeat(&InputEventV1::key(A, 0, true)));
    assert!(!is_repeat(&InputEventV1::key(A, 0, false)));
    // A pointer event's flags are buttons: bit 1 is the right button.
    assert!(!is_repeat(&InputEventV1::pointer(1, 1, 0x02)));
}

#[test]
fn the_queue_holds_at_most_one_unread_repeat() {
    let repeat = {
        let mut r = held(BACKSPACE, 0);
        r.poll(T0 + 500 * MS, EPOCH).expect("due")
    };
    let mut q = InputQueue::with_capacity(8);
    assert_eq!(
        q.enqueue(InputEventV1::key(BACKSPACE, 0, true)),
        Enqueued::Appended
    );
    assert_eq!(q.enqueue(repeat), Enqueued::Appended);
    assert_eq!(q.enqueue(repeat), Enqueued::Coalesced);
    assert_eq!(q.enqueue(InputEventV1::key(A, 0, true)), Enqueued::Appended);
    assert_eq!(
        q.enqueue(repeat),
        Enqueued::Coalesced,
        "behind a key, still unread"
    );
    assert_eq!(q.len(), 3);
    // The client takes the press and the repeat: the next repeat queues.
    assert_eq!(q.pop_front(), Some(InputEventV1::key(BACKSPACE, 0, true)));
    assert_eq!(q.pop_front(), Some(repeat));
    assert_eq!(q.enqueue(repeat), Enqueued::Appended);
    assert_eq!(q.len(), 2);
}

#[test]
fn a_repeat_never_evicts_a_key_from_a_full_queue() {
    let mut r = held(BACKSPACE, 0);
    let repeat = r.poll(T0 + 500 * MS, EPOCH).expect("due");
    let mut q = InputQueue::with_capacity(4);
    let keys = [0x04, 0x05, 0x06, 0x07].map(|u| InputEventV1::key(u, 0, true));
    for k in keys {
        assert_eq!(q.enqueue(k), Enqueued::Appended);
    }
    assert_eq!(q.enqueue(repeat), Enqueued::Coalesced);
    let drained: Vec<_> = core::iter::from_fn(|| q.pop_front()).collect();
    assert_eq!(drained, keys);
}

/// Chad's first input (#106 review, item 11): a repeat 490 ms after the
/// press. The machine gives none until 500 ms, polled every 1 ms.
#[test]
fn no_repeat_at_490_ms_or_anywhere_before_500() {
    let mut r = held(BACKSPACE, 0);
    for ms in 0..500 {
        assert_eq!(r.poll(T0 + ms * MS, EPOCH), None, "{ms} ms");
    }
    assert!(r.poll(T0 + 500 * MS, EPOCH).is_some());
}

/// Chad's second input: repeats 30 ms apart (500, 530, 560 ms). The second
/// is not due until 533⅓ ms, so a poll at 530 gives nothing.
#[test]
fn no_repeat_30_ms_after_the_previous_when_polled_on_time() {
    let mut r = held(BACKSPACE, 0);
    assert!(r.poll(T0 + 500 * MS, EPOCH).is_some());
    assert_eq!(r.poll(T0 + 530 * MS, EPOCH), None);
    assert!(r.poll(T0 + 533_333_333, EPOCH).is_some());
    assert_eq!(r.poll(T0 + 563_333_333, EPOCH), None);
}

/// The bound the QEMU scenario asserts, here for any poll pattern: repeat k
/// (from 0) is never earlier than press + 500 ms + k × (1 s / 30). Polled
/// every 1 ms for 3 s, and every tick (10 ms), where adjacent repeats land
/// 30 or 40 ms apart on tick edges and still meet the bound.
#[test]
fn repeat_k_never_before_500_ms_plus_k_thirtieths() {
    for step_ms in [1, 10] {
        let mut r = held(BACKSPACE, 0);
        let times: Vec<u64> = (0..3_000 / step_ms)
            .map(|i| T0 + i * step_ms * MS)
            .filter(|&t| r.poll(t, EPOCH).is_some())
            .collect();
        assert!(times.len() >= 75, "step {step_ms} ms: {}", times.len());
        for (k, t) in times.iter().enumerate() {
            let earliest = T0 + 500 * MS + k as u64 * 1_000_000_000 / 30;
            assert!(*t >= earliest, "step {step_ms} ms: repeat {k} at {t}");
        }
        if step_ms == 10 {
            let gaps: Vec<u64> = times.windows(2).map(|w| (w[1] - w[0]) / MS).collect();
            assert!(gaps.iter().all(|g| *g == 30 || *g == 40), "{gaps:?}");
        }
    }
}
