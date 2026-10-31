// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the SAK chord policy (`sak_logic.rs`), driven through the
//! REAL boot-keyboard decoder (`hid_logic::BootKeyEventDecoder`) from raw
//! 8-byte boot reports, so the modifier-first rule is tested against the
//! decoder's actual "events on non-modifier changes only" behaviour (Jarvis
//! F5): key-first, modifier-first, Super-first release, key-first release,
//! and rollover.

use super::hid_logic::{BootKeyEventDecoder, KeyEvent};
use super::input_logic::InputEventV1;
use super::sak_logic::*;

const LGUI: u8 = 0x08;
const RGUI: u8 = 0x80;
const LSHIFT: u8 = 0x02;
const KEY_A: u8 = 0x04;

/// A boot report: modifiers, reserved, up to six keys.
fn report(modifiers: u8, keys: &[u8]) -> [u8; 8] {
    let mut r = [0u8; 8];
    r[0] = modifiers;
    r[2..2 + keys.len()].copy_from_slice(keys);
    r
}

/// What a client would receive (code, pressed) and the chords latched, for a
/// sequence of raw reports.
fn run(reports: &[[u8; 8]]) -> (Vec<(u8, bool)>, Vec<u8>) {
    let mut decoder = BootKeyEventDecoder::new();
    let mut sak = SakFilter::new();
    let (mut delivered, mut chords) = (Vec::new(), Vec::new());
    for r in reports {
        let mut events = [KeyEvent {
            keycode: 0,
            modifiers: 0,
            pressed: false,
        }; 12];
        let n = decoder.decode_report(r, &mut events);
        for e in &events[..n] {
            let event = InputEventV1::key(e.keycode, e.modifiers, e.pressed);
            match sak.filter(event) {
                SakVerdict::Pass => delivered.push((e.keycode, e.pressed)),
                SakVerdict::Chord(key) => chords.push(key),
                SakVerdict::Swallow => {}
            }
        }
    }
    (delivered, chords)
}

#[test]
fn modifier_first_chord_is_latched_and_reaches_no_client() {
    // Super down, Escape down, Escape up, Super up (QEMU's `meta_l-esc`).
    let (delivered, chords) = run(&[
        report(LGUI, &[]),
        report(LGUI, &[KEY_ESCAPE]),
        report(LGUI, &[]),
        report(0, &[]),
    ]);
    assert_eq!(chords, vec![KEY_ESCAPE]);
    assert!(delivered.is_empty(), "{delivered:?}");
}

#[test]
fn super_released_first_still_swallows_the_release() {
    let (delivered, chords) = run(&[
        report(RGUI, &[]),
        report(RGUI, &[KEY_TAB]),
        report(0, &[KEY_TAB]), // Super up first: no event (modifier-only)
        report(0, &[]),        // Tab up, now without Super
    ]);
    assert_eq!(chords, vec![KEY_TAB]);
    assert!(delivered.is_empty(), "{delivered:?}");
}

#[test]
fn key_first_is_not_a_chord_and_the_client_sees_a_whole_keystroke() {
    // Tab down first: delivered. Super added later: the decoder emits
    // nothing, so no chord. Tab's release must also reach the client.
    let (delivered, chords) = run(&[
        report(0, &[KEY_TAB]),
        report(LGUI, &[KEY_TAB]),
        report(LGUI, &[]),
        report(0, &[]),
    ]);
    assert!(chords.is_empty());
    assert_eq!(delivered, vec![(KEY_TAB, true), (KEY_TAB, false)]);
}

#[test]
fn key_first_released_after_super_released_is_whole() {
    let (delivered, chords) = run(&[
        report(0, &[KEY_SPACE]),
        report(LGUI, &[KEY_SPACE]),
        report(0, &[KEY_SPACE]),
        report(0, &[]),
    ]);
    assert!(chords.is_empty());
    assert_eq!(delivered, vec![(KEY_SPACE, true), (KEY_SPACE, false)]);
}

#[test]
fn rollover_only_the_chord_key_is_taken() {
    // Super held, 'a' and Escape in one report, then both released. 'a' is a
    // client key (Super+a is not a chord); Escape is the chord.
    let (delivered, chords) = run(&[
        report(LGUI, &[]),
        report(LGUI, &[KEY_A, KEY_ESCAPE]),
        report(LGUI | LSHIFT, &[KEY_A]),
        report(0, &[]),
    ]);
    assert_eq!(chords, vec![KEY_ESCAPE]);
    assert_eq!(delivered, vec![(KEY_A, true), (KEY_A, false)]);
}

#[test]
fn plain_keys_and_shifted_chord_keys_pass() {
    let (delivered, chords) = run(&[
        report(0, &[KEY_ESCAPE]),
        report(0, &[]),
        report(LSHIFT, &[KEY_TAB]),
        report(0, &[]),
    ]);
    assert!(chords.is_empty());
    assert_eq!(
        delivered,
        vec![
            (KEY_ESCAPE, true),
            (KEY_ESCAPE, false),
            (KEY_TAB, true),
            (KEY_TAB, false)
        ]
    );
}

#[test]
fn every_chord_key_has_a_distinct_latch_bit() {
    let bits: Vec<u8> = SAK_KEYS.iter().map(|&k| sak_bit(k)).collect();
    #[cfg(not(feature = "sak-lock"))]
    assert_eq!(bits, vec![1, 2, 4]);
    #[cfg(feature = "sak-lock")]
    assert_eq!(bits, vec![1, 2, 4, 8]);
    assert_eq!(sak_bit(KEY_A), 0);
    assert_eq!(MOD_SUPER, LGUI | RGUI);
}

/// Without the feature the drivers never call the filter, so keys are
/// bit-identical to ADR-074; with it, a key-only stream that contains no
/// Super chord is delivered exactly as the decoder produced it.
#[test]
fn non_chord_streams_are_untouched() {
    let reports: Vec<[u8; 8]> = (0..40u8)
        .map(|i| {
            if i % 2 == 0 {
                report((i / 2) % 3, &[0x04 + i / 2])
            } else {
                report(0, &[])
            }
        })
        .collect();
    let (delivered, chords) = run(&reports);
    assert!(chords.is_empty());
    let mut decoder = BootKeyEventDecoder::new();
    let mut raw = Vec::new();
    for r in &reports {
        let mut events = [KeyEvent {
            keycode: 0,
            modifiers: 0,
            pressed: false,
        }; 12];
        let n = decoder.decode_report(r, &mut events);
        raw.extend(events[..n].iter().map(|e| (e.keycode, e.pressed)));
    }
    assert_eq!(delivered, raw);
}

/// OT-66 L2: Super+L is a chord (the lock), latched as bit 8; plain L and
/// Shift+L reach the client; L pressed before Super stays the client's.
#[cfg(feature = "sak-lock")]
#[test]
fn super_l_is_the_lock_chord() {
    assert_eq!(KEY_L, 0x0f);
    assert_eq!(sak_bit(KEY_L), 8);
    let (delivered, chords) = run(&[
        report(LGUI, &[]),
        report(LGUI, &[KEY_L]),
        report(LGUI, &[]),
        report(0, &[]),
    ]);
    assert_eq!(chords, vec![KEY_L]);
    assert!(delivered.is_empty(), "{delivered:?}");
    // The activity bit shares the latch but is no chord's.
    assert_eq!(ACTIVITY_BIT, 0x10);
    assert!(SAK_KEYS.iter().all(|&k| sak_bit(k) & ACTIVITY_BIT == 0));
    let (delivered, chords) = run(&[
        report(0, &[KEY_L]),
        report(0, &[]),
        report(LSHIFT, &[KEY_L]),
        report(0, &[]),
        report(0, &[KEY_L]),
        report(RGUI, &[KEY_L]),
        report(0, &[]),
    ]);
    assert!(chords.is_empty());
    assert_eq!(
        delivered,
        vec![
            (KEY_L, true),
            (KEY_L, false),
            (KEY_L, true),
            (KEY_L, false),
            (KEY_L, true),
            (KEY_L, false)
        ]
    );
}

/// OT-66 L2b: the lock route word, against hand-laid words (bits 63..1 the
/// lock focus's id, bit 0 locked), not against its own encoder.
#[cfg(feature = "sak-lock-route")]
#[test]
fn lock_word_transitions_match_the_hand_laid_words() {
    use super::sak_logic::lock_word::*;
    assert_eq!(ENGAGED, 1);
    // Nothing registered: no lock, the route is the active focus.
    assert_eq!(engage(0), None);
    assert_eq!(route(0, 7), 7);
    assert!(admits(0, 7));
    // Focus 5 registered: 5 << 1 = 0b1010.
    let w = register(0, 5);
    assert_eq!(w, 0b1010);
    assert_eq!(id(w), 5);
    assert_eq!(route(w, 7), 7);
    // Locked: 0b1011. The route is 5 whatever the active focus is.
    let w = engage(w).unwrap();
    assert_eq!(w, 0b1011);
    assert_eq!(route(w, 7), 5);
    assert_eq!(route(w, 0), 5);
    assert_eq!(engage(w), None, "a second lock changes nothing");
    assert!(admits(w, 5));
    assert!(!admits(w, 7));
    // A WM restart registers focus 9 while locked: 9 << 1 | 1 = 0b10011.
    let w = register(w, 9);
    assert_eq!(w, 0b1_0011);
    assert_eq!(route(w, 7), 9);
    // Unlocked: 0b10010; the route is the active focus again.
    let w = release(w);
    assert_eq!(w, 0b1_0010);
    assert_eq!(route(w, 7), 7);
    assert!(admits(w, 7));
    // The lock focus dies: nothing registered; another focus dying is no
    // change.
    assert_eq!(dropped(0b1_0011, 9), Some(0));
    assert_eq!(dropped(0b1_0011, 5), None);
    assert_eq!(engage(dropped(0b1_0010, 9).unwrap()), None);
}
