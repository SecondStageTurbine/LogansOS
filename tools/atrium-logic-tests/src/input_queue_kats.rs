// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the per-client input queue policy (`input_queue_logic.rs`).
//!
//! The defect: under ADR-074's original drop-oldest rule a moving mouse evicts
//! keystrokes from a slow-draining client's queue. These KATs pin the fix
//! (keystrokes are never evicted by pointer traffic, order is preserved) and the
//! kill-switch ISC (key-only streams are bit-identical to the legacy rule).

use super::input_logic::{InputEventV1, decode_event, encode_event};
use super::input_queue_logic::{Enqueued, InputQueue};

const CAP: usize = 64;

fn key(code: u8) -> InputEventV1 {
    InputEventV1::key(code, 0, true)
}

fn motion(dx: i8, dy: i8) -> InputEventV1 {
    InputEventV1::pointer(dx, dy, 0)
}

fn drain(q: &mut InputQueue) -> Vec<InputEventV1> {
    core::iter::from_fn(|| q.pop_front()).collect()
}

/// The reference for the pre-split rule, written independently of the module:
/// a plain bounded FIFO that drops its oldest element when full.
fn reference_drop_oldest(events: &[InputEventV1]) -> Vec<InputEventV1> {
    let mut out: std::collections::VecDeque<InputEventV1> = Default::default();
    for &e in events {
        if out.len() == CAP {
            out.pop_front();
        }
        out.push_back(e);
    }
    out.into_iter().collect()
}

/// Deterministic xorshift so the property loops are reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn the_defect_reproduces_under_the_legacy_rule() {
    // A client that typed 10 keys and then saw 100 motion reports before
    // draining: drop-oldest loses every keystroke.
    let mut stream: Vec<_> = (0..10).map(|i| key(4 + i)).collect();
    stream.extend((0..100).map(|i| InputEventV1::pointer(1, 0, (i % 2) as u8)));
    let mut legacy = InputQueue::with_capacity(CAP);
    for &e in &stream {
        legacy.enqueue_legacy(e);
    }
    let kept = drain(&mut legacy);
    assert_eq!(kept, reference_drop_oldest(&stream));
    assert!(
        kept.iter().all(|e| e.kind == 2),
        "legacy rule evicted every key"
    );
}

#[test]
fn keystrokes_survive_any_motion_flood_in_order() {
    let mut rng = Rng(0x5eed_1234_abcd_ef01);
    for round in 0..200 {
        let mut q = InputQueue::with_capacity(CAP);
        let mut keys = Vec::new();
        for _ in 0..2000 {
            let r = rng.next();
            // ~2% keys, the rest motion with occasional button changes; the
            // queue is never drained, so it overflows hard.
            let e = if r.is_multiple_of(50) && keys.len() < CAP {
                let k = key(4 + (keys.len() % 26) as u8);
                keys.push(k);
                k
            } else {
                InputEventV1::pointer(
                    (r >> 8) as i8,
                    (r >> 16) as i8,
                    (r >> 24).is_multiple_of(3) as u8,
                )
            };
            q.enqueue(e);
        }
        let kept = drain(&mut q);
        let kept_keys: Vec<_> = kept.iter().copied().filter(|e| e.kind == 1).collect();
        assert_eq!(kept_keys, keys, "round {round}: every key, in order");
        assert!(kept.len() <= CAP);
    }
}

#[test]
fn key_only_streams_are_bit_identical_to_the_legacy_rule() {
    // Kill-switch ISC: with no pointer events, the new policy IS the old one,
    // byte for byte, at every length including far past capacity.
    let mut rng = Rng(0xdead_beef_0bad_f00d);
    for len in [0usize, 1, 63, 64, 65, 128, 1000] {
        let stream: Vec<_> = (0..len)
            .map(|_| {
                let r = rng.next();
                InputEventV1::key((r % 200) as u8, (r >> 8) as u8, r & 1 == 1)
            })
            .collect();
        let mut new = InputQueue::with_capacity(CAP);
        let mut legacy = InputQueue::with_capacity(CAP);
        for &e in &stream {
            let a = new.enqueue(e);
            let b = legacy.enqueue_legacy(e);
            assert_eq!(a, b, "same outcome per event");
        }
        let a: Vec<_> = drain(&mut new).into_iter().map(encode_event).collect();
        let b: Vec<_> = drain(&mut legacy).into_iter().map(encode_event).collect();
        let r: Vec<_> = reference_drop_oldest(&stream)
            .into_iter()
            .map(encode_event)
            .collect();
        assert_eq!(a, b, "len {len}: new == legacy, byte for byte");
        assert_eq!(
            a, r,
            "len {len}: legacy == independent drop-oldest reference"
        );
    }
}

#[test]
fn motion_coalesces_into_the_tail_only() {
    let mut q = InputQueue::with_capacity(CAP);
    assert_eq!(q.enqueue(motion(3, -2)), Enqueued::Appended);
    assert_eq!(q.enqueue(motion(4, 5)), Enqueued::Coalesced);
    assert_eq!(q.enqueue(key(4)), Enqueued::Appended);
    // A key sits at the tail now: motion appends rather than merging across it.
    assert_eq!(q.enqueue(motion(1, 1)), Enqueued::Appended);
    assert_eq!(
        drain(&mut q),
        vec![motion(7, 3), key(4), motion(1, 1)],
        "merged deltas, order preserved"
    );
}

#[test]
fn coalescing_saturates_and_never_merges_button_transitions() {
    let mut q = InputQueue::with_capacity(CAP);
    for _ in 0..400 {
        q.enqueue(motion(127, -128));
    }
    let only = q.pop_front().unwrap();
    assert_eq!((only.x, only.y), (i16::MAX, i16::MIN), "saturating sum");
    assert!(q.is_empty());

    // press, drag, drag, release: the press and release are transitions and
    // stay separate; the two drags merge with each other only.
    let mut q = InputQueue::with_capacity(CAP);
    q.enqueue(InputEventV1::pointer(0, 0, 1));
    q.enqueue(InputEventV1::pointer(2, 0, 1));
    q.enqueue(InputEventV1::pointer(3, 0, 1));
    q.enqueue(InputEventV1::pointer(0, 0, 0));
    assert_eq!(
        drain(&mut q),
        vec![
            InputEventV1::pointer(0, 0, 1),
            InputEventV1::pointer(5, 0, 1),
            InputEventV1::pointer(0, 0, 0),
        ]
    );
}

#[test]
fn eviction_order_is_motion_then_superseded_transitions_then_keys() {
    let mut q = InputQueue::with_capacity(5);
    q.enqueue(InputEventV1::pointer(0, 0, 1)); // press (transition)
    q.enqueue(key(4));
    q.enqueue(InputEventV1::pointer(1, 0, 1)); // motion-only (button held)
    q.enqueue(InputEventV1::pointer(0, 0, 0)); // release (transition, newest pointer)
    q.enqueue(key(5));
    // Full. Motion goes first.
    assert_eq!(q.enqueue(key(6)), Enqueued::EvictedMotion);
    // Then the press: the later release supersedes it.
    assert_eq!(q.enqueue(key(7)), Enqueued::EvictedPointer);
    // The release is the newest pointer event: it is never evicted, so the
    // oldest key goes (the queue is keys plus one pointer event).
    assert_eq!(q.enqueue(key(8)), Enqueued::EvictedKey);
    assert_eq!(
        drain(&mut q),
        vec![
            InputEventV1::pointer(0, 0, 0),
            key(5),
            key(6),
            key(7),
            key(8)
        ]
    );
}

#[test]
fn incoming_pointer_traffic_never_evicts_a_key() {
    // 63 keys + one button change (the newest pointer event) fill the queue.
    let mut q = InputQueue::with_capacity(CAP);
    for i in 0..63 {
        q.enqueue(key(4 + (i % 26) as u8));
    }
    q.enqueue(InputEventV1::pointer(0, 0, 1));
    // Incoming motion: nothing cheaper queued, so the motion itself is dropped.
    assert_eq!(
        q.enqueue(InputEventV1::pointer(5, 5, 1)),
        Enqueued::DroppedIncomingMotion
    );
    // Incoming release supersedes the queued press: the press goes, not a key.
    assert_eq!(
        q.enqueue(InputEventV1::pointer(0, 0, 0)),
        Enqueued::EvictedPointer
    );
    let kept = drain(&mut q);
    assert_eq!(kept.iter().filter(|e| e.kind == 1).count(), 63);
    assert_eq!(kept.last(), Some(&InputEventV1::pointer(0, 0, 0)));
}

#[test]
fn the_final_button_state_always_reaches_the_client() {
    // AAG concurrency-lens repro: press delivered, then 63 keys, a release and
    // one more key. Evicting the release would leave the client holding the
    // button forever; it must survive.
    let mut q = InputQueue::with_capacity(CAP);
    q.enqueue(InputEventV1::pointer(0, 0, 1));
    assert_eq!(q.pop_front(), Some(InputEventV1::pointer(0, 0, 1)));
    for i in 0..63 {
        q.enqueue(key(4 + (i % 26) as u8));
    }
    q.enqueue(InputEventV1::pointer(0, 0, 0));
    q.enqueue(key(30));
    let kept = drain(&mut q);
    assert_eq!(
        kept.iter().rev().find(|e| e.kind == 2).map(|e| e.flags),
        Some(0)
    );

    // Property: over random streams with random partial drains, the last button
    // state the client observes equals the last button state the mouse sent.
    let mut rng = Rng(0x0b5e_55ed_c0ff_ee00);
    for _ in 0..5000 {
        let mut q = InputQueue::with_capacity(16);
        let (mut sent, mut seen) = (None, None);
        for _ in 0..200 {
            let r = rng.next();
            match r % 10 {
                0..=3 => {
                    q.enqueue(key((r >> 8) as u8 % 100));
                }
                4..=7 => {
                    let buttons = ((r >> 8) % 4) as u8;
                    sent = Some(buttons);
                    q.enqueue(InputEventV1::pointer((r >> 16) as i8, 0, buttons));
                }
                _ => {
                    if let Some(e) = q.pop_front()
                        && e.kind == 2
                    {
                        seen = Some(e.flags);
                    }
                }
            }
        }
        for e in drain(&mut q) {
            if e.kind == 2 {
                seen = Some(e.flags);
            }
        }
        assert_eq!(seen, sent, "final button state");
    }
}

#[test]
fn wire_bytes_of_a_coalesced_event_match_a_hand_laid_oracle() {
    // dx = 100 + 100 + 100 = 300 = 0x012c, dy = -5 * 3 = -15 = 0xfff1, buttons 0.
    let mut q = InputQueue::with_capacity(CAP);
    for _ in 0..3 {
        q.enqueue(motion(100, -5));
    }
    let bytes = encode_event(q.pop_front().unwrap());
    assert_eq!(bytes, [0x02, 0x00, 0x00, 0x00, 0x2c, 0x01, 0xf1, 0xff]);
    assert_eq!(
        decode_event(bytes),
        InputEventV1 {
            kind: 2,
            flags: 0,
            code: 0,
            x: 300,
            y: -15
        }
    );
}

/// Janus (Codex, #38 RETURN): a key overflow must not evict the only
/// unsuperseded button transition even when it sits at the queue head.
/// Capacity 3: down is delivered (the client holds the button), then the
/// release and two keys are queued; a third key overflows. The old fallback
/// popped the head — the release — leaving the client stuck "down".
#[test]
fn a_key_overflow_never_evicts_the_sole_queued_release_at_the_head() {
    let mut q = InputQueue::with_capacity(3);
    assert_eq!(
        q.enqueue(InputEventV1::pointer(0, 0, 1)),
        Enqueued::Appended
    );
    assert_eq!(q.pop_front(), Some(InputEventV1::pointer(0, 0, 1)));
    let release = InputEventV1::pointer(0, 0, 0);
    assert_eq!(q.enqueue(release), Enqueued::Appended);
    assert_eq!(q.enqueue(key(4)), Enqueued::Appended);
    assert_eq!(q.enqueue(key(5)), Enqueued::Appended);
    assert_eq!(q.enqueue(key(6)), Enqueued::EvictedKey);
    let kept = drain(&mut q);
    assert_eq!(
        kept,
        vec![release, key(5), key(6)],
        "the only queued release was evicted"
    );
}

fn wheel(step: i8) -> InputEventV1 {
    InputEventV1::pointer_wheel(0, 0, 0, step)
}

/// A9-C2: a report carrying a wheel is never merged into motion (its wheel
/// would be lost), and motion is never merged into it.
#[test]
fn wheel_reports_are_never_coalesced() {
    let mut q = InputQueue::with_capacity(8);
    q.enqueue(motion(1, 1));
    assert_eq!(q.enqueue(wheel(1)), Enqueued::Appended);
    assert_eq!(q.enqueue(motion(2, 2)), Enqueued::Appended);
    assert_eq!(q.enqueue(wheel(-1)), Enqueued::Appended);
    let wheels: Vec<i16> = drain(&mut q).iter().map(|e| e.wheel()).collect();
    assert_eq!(wheels, [0, 1, 0, -1]);
}

/// A9-C2: a full queue drops an incoming wheel report rather than evict a
/// key or a button transition; a queued wheel report is evicted neither as
/// motion nor as a superseded transition.
#[test]
fn wheel_reports_never_evict_keys_and_are_not_evicted() {
    let mut q = InputQueue::with_capacity(3);
    for c in [4, 5, 6] {
        q.enqueue(key(c));
    }
    assert_eq!(q.enqueue(wheel(1)), Enqueued::DroppedIncomingMotion);
    assert!(drain(&mut q).iter().all(|e| e.kind == 1), "the keys stay");
    // Queued wheel reports survive motion and later pointer reports.
    let mut q = InputQueue::with_capacity(3);
    q.enqueue(wheel(1));
    q.enqueue(wheel(2));
    q.enqueue(motion(1, 0));
    // A press evicts the motion; a release then evicts the press it
    // supersedes. Neither touches the wheel reports.
    assert_eq!(
        q.enqueue(InputEventV1::pointer(0, 0, 1)),
        Enqueued::EvictedMotion
    );
    assert_eq!(
        q.enqueue(InputEventV1::pointer(0, 0, 0)),
        Enqueued::EvictedPointer
    );
    let left: Vec<(i16, u8)> = drain(&mut q).iter().map(|e| (e.wheel(), e.flags)).collect();
    assert_eq!(left, [(1, 0), (2, 0), (0, 0)]);
}

/// Chad, #104 RETURN F1: a WM-forwarded wheel (kind 5) arriving at a client
/// queue full of keys is dropped, the same as a raw wheel; it never evicts a
/// keystroke. The test is Chad's, as given.
#[test]
fn forwarded_wheel_never_evicts_a_key() {
    let mut q = InputQueue::with_capacity(64);
    for c in 4..68 {
        q.enqueue(InputEventV1::key(c, 0, true));
    }
    let w = InputEventV1 {
        kind: 5,
        flags: 0,
        code: 1,
        x: 0,
        y: 0,
    };
    // FAIL: actual is EvictedKey, and the first remaining code is 5.
    assert_eq!(q.enqueue(w), Enqueued::DroppedIncomingMotion);
    assert_eq!(q.pop_front().unwrap().code, 4);
}
