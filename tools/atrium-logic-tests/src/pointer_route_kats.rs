// SPDX-License-Identifier: MPL-2.0
//! Host KATs for DI-1a B-ref records (`pointer_route_logic.rs`): consume
//! exactly once, replay and overwrite fail closed, wrap is exhaustion (not
//! reuse), RESYNC after a lost transition, phase derivation, i16 rejection,
//! and a hand-laid kind-4 byte oracle.

use super::input_logic::{INPUT_EVENT_KEY, InputEventV1, decode_event, encode_event};
use super::input_queue_logic::{Enqueued, InputQueue};
use super::pointer_route_logic::*;

#[test]
fn kind4_wire_byte_oracle() {
    // Record seq 0x0203 (tag 0x0203), buttons = left (1), dx = -2, dy = 300.
    let bytes = [0x04, 0x01, 0x03, 0x02, 0xfe, 0xff, 0x2c, 0x01];
    let mut ring = RecordRing::starting_at(0x0203);
    let event = ring.mint(-2, 300, 1).unwrap();
    assert_eq!(encode_event(event), bytes);
    assert_eq!(
        decode_event(bytes),
        InputEventV1 {
            kind: INPUT_EVENT_POINTER_REC,
            flags: 1,
            code: 0x0203,
            x: -2,
            y: 300,
        }
    );
    assert_eq!(RESYNC, 0x80);
}

/// Hand-laid kind-3 (forwarded to a client): DOWN, button 1, x=300 (2c 01),
/// y=-15 (f1 ff).
#[test]
fn kind3_wire_byte_oracle() {
    let bytes = [0x03, 0x01, 0x01, 0x00, 0x2c, 0x01, 0xf1, 0xff];
    let event = InputEventV1 {
        kind: INPUT_EVENT_POINTER_ABS,
        flags: 1,
        code: PHASE_DOWN,
        x: 300,
        y: -15,
    };
    assert_eq!(encode_event(event), bytes);
    assert_eq!(decode_event(bytes), event);
}

/// A9-C2, hand-laid: a kind-2 pointer report with a wheel carries it in
/// `code` as an i16 (wheel -1 = ff ff; right button, dx 5, dy -3); with no
/// wheel it is byte-for-byte today's report (`code` 00 00).
#[test]
fn kind2_wheel_wire_byte_oracle() {
    let bytes = [0x02, 0x02, 0xff, 0xff, 0x05, 0x00, 0xfd, 0xff];
    let event = InputEventV1::pointer_wheel(5, -3, 2, -1);
    assert_eq!(encode_event(event), bytes);
    assert_eq!(decode_event(bytes).wheel(), -1);
    assert_eq!(
        encode_event(InputEventV1::pointer_wheel(5, -3, 2, 0)),
        encode_event(InputEventV1::pointer(5, -3, 2))
    );
    assert_eq!(
        encode_event(InputEventV1::pointer(5, -3, 2)),
        [0x02, 0x02, 0x00, 0x00, 0x05, 0x00, 0xfd, 0xff]
    );
    assert_eq!(InputEventV1::pointer_wheel(0, 0, 0, 3).wheel(), 3);
}

/// A9-C2, hand-laid: a kind-4 WHEEL record (tag 0x0104; left button held,
/// so flags = 0x20 | 0x01; x = the wheel, +2; y = 0) and the kind-5 event a
/// forward of it delivers (buttons 1, wheel +2 in `code`, local 40,-7).
#[test]
fn wheel_records_wire_byte_oracle() {
    let mut ring = RecordRing::starting_at(0x0103);
    ring.mint(0, 0, 1).unwrap();
    let record = ring.mint_wheel(2).unwrap();
    assert_eq!(
        encode_event(record),
        [0x04, 0x21, 0x04, 0x01, 0x02, 0x00, 0x00, 0x00]
    );
    assert_eq!((WHEEL, INPUT_EVENT_POINTER_WHEEL), (0x20, 5));
    let forwarded = InputEventV1 {
        kind: INPUT_EVENT_POINTER_WHEEL,
        flags: 1,
        code: 2,
        x: 40,
        y: -7,
    };
    assert_eq!(
        encode_event(forwarded),
        [0x05, 0x01, 0x02, 0x00, 0x28, 0x00, 0xf9, 0xff]
    );
    assert_eq!(
        decode_event([0x05, 0x01, 0xfe, 0xff, 0, 0, 0, 0]).wheel(),
        -2
    );
}

/// A9-C2: a wheel record keeps the buttons it was minted under, is consumed
/// once like any record and yields its wheel; it is never a button
/// transition, so expiring one unconsumed forces no RESYNC (CP-52), while a
/// lost button transition still does.
#[test]
fn wheel_records_consume_once_and_never_resync() {
    let mut ring = RecordRing::new();
    assert_eq!(ring.mint_wheel(0), None, "no wheel, no record");
    let down = ring.mint(0, 0, 1).unwrap().code;
    let wheel = ring.mint_wheel(-3).unwrap();
    assert_eq!(wheel.flags, WHEEL | 1);
    assert_eq!((wheel.x, wheel.y), (-3, 0));
    assert_eq!(
        ring.consume(wheel.code),
        Ok(Consumed {
            buttons: 1,
            phase: PHASE_MOVE,
            wheel: -3,
        })
    );
    assert_eq!(ring.consume(wheel.code), Err(ConsumeError::Already));
    assert_eq!(ring.consume(down).map(|c| c.wheel), Ok(0));
    // A full ring of unconsumed wheel records: they expire, and none of them
    // was a transition, so the next record carries no RESYNC.
    for _ in 0..RECORD_RING {
        ring.mint_wheel(1).unwrap();
    }
    let after = ring.mint_wheel(1).unwrap();
    assert!(ring.expired() > 0);
    assert_eq!(after.flags & RESYNC, 0);
    // A lost button transition still forces one.
    let mut lossy = RecordRing::new();
    lossy.mint(0, 0, 1).unwrap(); // a press, never consumed
    for _ in 0..RECORD_RING - 1 {
        lossy.mint_wheel(1).unwrap();
    }
    // A wheel record minted over the lost press never carries RESYNC (the
    // WM reads it for its wheel alone); the next motion record does, once.
    let over = lossy.mint_wheel(1).unwrap();
    assert_eq!(over.flags, WHEEL | 1);
    assert_eq!(lossy.mint_wheel(1).unwrap().flags, WHEEL | 1);
    assert_eq!(lossy.mint(0, 0, 1).unwrap().flags, RESYNC | 1);
    assert_eq!(lossy.mint(0, 0, 1).unwrap().flags, 1);
}

#[test]
fn consume_once_then_already_and_unknown_is_stale() {
    let mut ring = RecordRing::new();
    let down = ring.mint(0, 0, 1).unwrap().code;
    let up = ring.mint(0, 0, 0).unwrap().code;
    assert_eq!(
        ring.consume(down),
        Ok(Consumed {
            buttons: 1,
            phase: PHASE_DOWN,
            wheel: 0,
        })
    );
    assert_eq!(ring.consume(down), Err(ConsumeError::Already));
    assert_eq!(ring.peek(down), Err(ConsumeError::Already));
    assert_eq!(ring.peek(up), Ok(()));
    assert_eq!(
        ring.consume(up),
        Ok(Consumed {
            buttons: 0,
            phase: PHASE_UP,
            wheel: 0,
        })
    );
    // Never minted.
    assert_eq!(ring.consume(up.wrapping_add(1)), Err(ConsumeError::Stale));
}

#[test]
fn overwritten_records_are_stale_and_counted() {
    let mut ring = RecordRing::new();
    let first = ring.mint(1, 0, 0).unwrap().code;
    for _ in 0..RECORD_RING {
        ring.mint(1, 0, 0).unwrap();
    }
    // Slot reused: the tag no longer matches.
    assert_eq!(ring.consume(first), Err(ConsumeError::Stale));
    assert_eq!(ring.expired(), 1);
}

#[test]
fn a_lost_transition_makes_the_next_record_resync() {
    let mut ring = RecordRing::new();
    ring.mint(0, 0, 1).unwrap(); // a press, never consumed
    let mut resynced = None;
    for i in 0..RECORD_RING {
        let e = ring.mint(1, 0, 1).unwrap();
        if e.flags & RESYNC != 0 {
            resynced = Some((i, e.flags & !RESYNC));
        }
    }
    // The 64th mint expires the press (a transition) and carries RESYNC with
    // the true bitmap; expiring plain motion does not.
    assert_eq!(resynced, Some((RECORD_RING - 1, 1)));
    let next = ring.mint(1, 0, 1).unwrap();
    assert_eq!(next.flags & RESYNC, 0);
    ring.note_lost();
    assert_ne!(ring.mint(0, 0, 0).unwrap().flags & RESYNC, 0);
}

#[test]
fn sequence_exhaustion_fails_closed_never_reuses() {
    let mut ring = RecordRing::starting_at(MAX_SEQ - 1);
    assert!(ring.mint(0, 0, 0).is_some());
    assert!(ring.mint(0, 0, 0).is_some());
    assert_eq!(ring.mint(0, 0, 0), None);
    assert_eq!(ring.mint(0, 0, 0), None);
}

/// Jarvis (#46 RETURN F1): a retained tag must never consume a later record.
/// Mint seq 1, keep its tag, mint the whole rest of the sequence space: the
/// route is spent before any tag recurs, so `consume(1)` can only be stale
/// (seq 1's slot was overwritten), never a hit on some newer record. The old
/// 16-bit-wrapping ring consumed seq 65 537 here.
#[test]
fn a_retained_tag_never_aliases_a_newer_record() {
    let mut ring = RecordRing::new();
    let first = ring.mint(1, 0, 0).unwrap();
    assert_eq!(first.code, 1);
    let mut minted = 1u32;
    while ring.mint(1, 0, 0).is_some() {
        minted += 1;
    }
    assert_eq!(minted, MAX_SEQ);
    assert_eq!(ring.consume(first.code), Err(ConsumeError::Stale));
    // Every live tag is the record it was minted as: the 64 newest.
    for seq in MAX_SEQ - 63..=MAX_SEQ {
        assert!(ring.consume(seq as u16).is_ok(), "seq {seq}");
    }
}

/// A spent route sends exactly one ROUTE_EXPIRED record (tag 0, never
/// minted, so unconsumable), carrying the held buttons, and mints nothing.
#[test]
fn a_spent_route_sends_one_expiry_record() {
    let mut ring = RecordRing::starting_at(MAX_SEQ);
    assert_eq!(ring.expiry(), None);
    assert!(ring.mint(0, 0, 1).is_some());
    assert_eq!(ring.mint(0, 0, 1), None);
    let expiry = ring.expiry().unwrap();
    assert_eq!(expiry.kind, INPUT_EVENT_POINTER_REC);
    assert_eq!(expiry.flags, ROUTE_EXPIRED | 1);
    assert_eq!(expiry.code, 0);
    assert_eq!(ring.consume(0), Err(ConsumeError::Stale));
    assert_eq!(ring.expiry(), None);
    assert_eq!(ROUTE_EXPIRED, 0x40);
}

#[test]
fn phase_truth_table() {
    assert_eq!(phase(0, 1), PHASE_DOWN);
    assert_eq!(phase(1, 3), PHASE_DOWN);
    assert_eq!(phase(3, 1), PHASE_UP);
    assert_eq!(phase(1, 0), PHASE_UP);
    assert_eq!(phase(1, 1), PHASE_MOVE);
    assert_eq!(phase(0, 0), PHASE_MOVE);
    // A simultaneous press and release counts as a press (new button wins).
    assert_eq!(phase(1, 2), PHASE_DOWN);
}

#[test]
fn forward_coordinates_are_rejected_not_clamped() {
    assert_eq!(forward_coords(-5, 7), Some((-5, 7)));
    assert_eq!(forward_coords(32_767, -32_768), Some((32_767, -32_768)));
    assert_eq!(forward_coords(32_768, 0), None);
    assert_eq!(forward_coords(0, -32_769), None);
    assert_eq!(forward_coords(i64::MAX, 0), None);
}

/// Random interleavings: every live record is consumable exactly once, and
/// any record older than the ring is refused.
#[test]
fn property_consume_at_most_once() {
    let mut ring = RecordRing::new();
    let mut rng = 0x9e37_79b9_7f4a_7c15u64;
    let mut minted: Vec<(u16, u64)> = Vec::new();
    let mut consumed = std::collections::HashSet::new();
    for step in 0..20_000u64 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        if !rng.is_multiple_of(3) {
            let e = ring.mint(1, 1, (rng >> 8) as u8 & 3).unwrap();
            minted.push((e.code, step));
        } else if let Some(&(tag, at)) = minted.get((rng >> 16) as usize % minted.len().max(1)) {
            let age = minted.iter().filter(|(_, s)| *s > at).count();
            match ring.consume(tag) {
                Ok(_) => {
                    assert!(age < RECORD_RING, "consumed an overwritten record");
                    assert!(consumed.insert((tag, at)), "consumed twice");
                }
                Err(ConsumeError::Already) => assert!(consumed.contains(&(tag, at))),
                Err(ConsumeError::Stale) => assert!(age >= RECORD_RING),
            }
        }
    }
}

fn abs(flags: u8, code: u16, x: i16) -> InputEventV1 {
    InputEventV1 {
        kind: INPUT_EVENT_POINTER_ABS,
        flags,
        code,
        x,
        y: 0,
    }
}

/// AAG F1: forwarded (kind-3) drags take the pointer policy in the client
/// queue, so a drag that overflows it never evicts a queued keystroke.
#[test]
fn a_forwarded_drag_never_evicts_queued_keys() {
    let mut q = InputQueue::with_capacity(64);
    for k in 0..3u8 {
        q.enqueue(InputEventV1::key(4 + k, 0, true));
    }
    q.enqueue(abs(1, PHASE_DOWN, 0));
    for i in 0..200 {
        q.enqueue(abs(1, PHASE_MOVE, i));
        // A second button pressed and released breaks coalescing, so the
        // queue really overflows.
        q.enqueue(abs(3, PHASE_DOWN, i));
        q.enqueue(abs(1, PHASE_UP, i));
    }
    q.enqueue(abs(0, PHASE_UP, 7));
    let got: Vec<_> = core::iter::from_fn(|| q.pop_front()).collect();
    let keys: Vec<u16> = got
        .iter()
        .filter(|e| e.kind == INPUT_EVENT_KEY)
        .map(|e| e.code)
        .collect();
    assert_eq!(keys, vec![4, 5, 6]);
    assert_eq!(got.last(), Some(&abs(0, PHASE_UP, 7)));
}

#[test]
fn forwarded_moves_coalesce_by_replacement_not_presses() {
    let mut q = InputQueue::with_capacity(8);
    assert_eq!(q.enqueue(abs(1, PHASE_DOWN, 1)), Enqueued::Appended);
    assert_eq!(q.enqueue(abs(1, PHASE_MOVE, 2)), Enqueued::Appended);
    assert_eq!(q.enqueue(abs(1, PHASE_MOVE, 9)), Enqueued::Coalesced);
    // A relative event never merges into an absolute tail.
    assert_eq!(
        q.enqueue(InputEventV1::pointer(1, 1, 1)),
        Enqueued::Appended
    );
    let got: Vec<_> = core::iter::from_fn(|| q.pop_front()).collect();
    assert_eq!(got[1], abs(1, PHASE_MOVE, 9));
    assert_eq!(got.len(), 3);
}

/// Chad, #104 RETURN F2: a press that arrives with a wheel, on a WM queue
/// one short of full whose oldest record is an unread release (the WM still
/// holds the old grab), must carry RESYNC, so the WM cancels the old grab
/// instead of handing the new press to it.
///
/// Chad's scenario and assertions, with one change: his test minted the two
/// records itself, in the order `focus.rs` used (both minted, motion first,
/// before either was queued), which is the defect. Here the report goes
/// through `RecordRing::mint_report`, the function `mint_records` now calls,
/// so the KAT tests the kernel's order rather than a copy of it. The wheel
/// record now comes first, so the press is tag 66, not 65. On the old order
/// this test fails exactly as Chad's did (flags = 1, no RESYNC).
#[test]
fn press_with_wheel_after_lost_release_carries_resync() {
    let mut ring = RecordRing::new();
    let mut q = InputQueue::with_capacity(64);
    let down = ring.mint(0, 0, 1).unwrap();
    ring.consume(down.code).unwrap();
    q.enqueue(ring.mint(0, 0, 0).unwrap()); // release tag 2
    for _ in 0..62 {
        q.enqueue(ring.mint_wheel(1).unwrap());
    }
    // A left press, the pointer moving up, with one wheel notch.
    let report = InputEventV1::pointer_wheel(0, -127, 1, 1);
    ring.mint_report(&report, |r| {
        matches!(q.enqueue(r), Enqueued::Appended | Enqueued::Coalesced)
    });
    let press = std::iter::from_fn(|| q.pop_front())
        .find(|r| r.flags & WHEEL == 0)
        .unwrap();
    assert_ne!(press.flags & RESYNC, 0, "the press after the lost release");
    assert_eq!(press.code, 66, "the wheel record (65) went first");
    // The WM's own decision on it (`neo_atrium::pointer_grab`, as
    // `desktop_session.rs` runs it): it still holds buttons 1 and the old
    // grab, having never seen the release. RESYNC with a button held cancels
    // that grab, and is not taken as a fresh press on the old target.
    let decision = neo_atrium::pointer_grab::decide(press.flags, 1, true);
    assert!(decision.cancel_grab && !decision.pressed && !decision.release);
}
