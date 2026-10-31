// SPDX-License-Identifier: MPL-2.0
//! The `NAL1` byte oracle (L2.1). The two records below are hand-laid,
//! independent of [`encode`]: each field written from THINK-2026-10-30-l2
//! §2's table by a separate script (Python `struct`, field by field), and the
//! links (`prev`) are BLAKE3 from the Python `blake3` package over those
//! bytes, not this crate's [`link`].

use super::*;

/// Record 0: asked, read, seq 0, 1761830400 s, detail 0, the path, the intent.
const R0: [u8; 101] = [
    0x4e, 0x41, 0x4c, 0x31, 0x01, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x66, 0x03, 0x69, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x15, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x2f, 0x6e, 0x65, 0x6f, 0x66, 0x73, 0x2f, 0x6e, 0x6f, 0x74, 0x65, 0x73, 0x2e, 0x74, 0x78, 0x74,
    0x72, 0x65, 0x61, 0x64, 0x20, 0x2f, 0x6e, 0x65, 0x6f, 0x66, 0x73, 0x2f, 0x6e, 0x6f, 0x74, 0x65,
    0x73, 0x2e, 0x74, 0x78, 0x74,
];
/// BLAKE3(R0)[..16].
const L0: [u8; 16] = [
    0x8f, 0x70, 0xab, 0xda, 0x49, 0xcb, 0xed, 0xa3, 0xb3, 0x41, 0xaf, 0x50, 0x46, 0xd4, 0xad, 0xa6,
];
/// Record 1: done, read, seq 1, 1761830405 s, detail 13 (EACCES), the path,
/// no intent, prev = L0.
const R1: [u8; 80] = [
    0x4e, 0x41, 0x4c, 0x31, 0x01, 0x00, 0x07, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x05, 0x66, 0x03, 0x69, 0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
    0x8f, 0x70, 0xab, 0xda, 0x49, 0xcb, 0xed, 0xa3, 0xb3, 0x41, 0xaf, 0x50, 0x46, 0xd4, 0xad, 0xa6,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x2f, 0x6e, 0x65, 0x6f, 0x66, 0x73, 0x2f, 0x6e, 0x6f, 0x74, 0x65, 0x73, 0x2e, 0x74, 0x78, 0x74,
];
/// BLAKE3(R1)[..16].
const L1: [u8; 16] = [
    0x6d, 0xb5, 0x1f, 0xe5, 0xd2, 0x96, 0x86, 0xe0, 0x58, 0xdd, 0x78, 0x79, 0x77, 0xe0, 0x12, 0xca,
];

fn r0() -> Record<'static> {
    Record {
        kind: Kind::Asked,
        action: 1,
        seq: 0,
        time_s: 1_761_830_400,
        detail: 0,
        path: "/neofs/notes.txt",
        intent: "read /neofs/notes.txt",
        prev: [0; 16],
    }
}

fn r1() -> Record<'static> {
    Record {
        kind: Kind::Done,
        action: 1,
        seq: 1,
        time_s: 1_761_830_405,
        detail: 13,
        path: "/neofs/notes.txt",
        intent: "",
        prev: L0,
    }
}

fn log() -> Vec<u8> {
    let mut log = R0.to_vec();
    log.extend_from_slice(&R1);
    log
}

#[test]
fn encode_is_the_oracle() {
    assert_eq!(encode(&r0()).unwrap(), R0);
    assert_eq!(encode(&r1()).unwrap(), R1);
}

#[test]
fn decode_reads_the_oracle() {
    assert_eq!(decode(&R0), Some((r0(), R0.len())));
    assert_eq!(decode(&R1), Some((r1(), R1.len())));
}

#[test]
fn link_is_the_oracle() {
    assert_eq!(link(&R0), L0);
    assert_eq!(link(&R1), L1);
}

#[test]
fn verify_walks_the_chain() {
    let head = verify(&log());
    assert_eq!(
        head,
        Head {
            records: 2,
            len: R0.len() + R1.len(),
            link: L1,
            broken_at: None,
        }
    );
    assert_eq!(
        verify(&[]),
        Head {
            records: 0,
            len: 0,
            link: [0; 16],
            broken_at: None
        }
    );
}

/// Any one byte changed in record 0 breaks the chain at record 0 or 1: a
/// field check refuses record 0 itself, or its link no longer matches
/// record 1's `prev`. Nothing in it changes silently.
#[test]
fn any_flipped_byte_is_caught() {
    for at in 0..R0.len() {
        for bit in 0..8 {
            let mut bad = log();
            bad[at] ^= 1 << bit;
            let head = verify(&bad);
            assert!(head.broken_at.is_some(), "byte {at} bit {bit}");
            assert!(head.records < 2, "byte {at} bit {bit}");
        }
    }
}

#[test]
fn a_removed_or_reordered_record_is_caught() {
    // Record 1 alone: its sequence is 1 and its prev isn't zero.
    assert_eq!(verify(&R1).broken_at, Some(0));
    // Swapped.
    let mut swapped = R1.to_vec();
    swapped.extend_from_slice(&R0);
    assert_eq!(verify(&swapped).broken_at, Some(0));
    // A truncated tail: the good record stands, the stub is reported.
    let mut cut = log();
    cut.truncate(R0.len() + 10);
    let head = verify(&cut);
    assert_eq!((head.records, head.link, head.broken_at), (1, L0, Some(1)));
}

#[test]
fn texts_over_255_are_refused() {
    let long = "x".repeat(256);
    let mut r = r0();
    r.intent = &long;
    assert_eq!(encode(&r), None);
}

/// A rotated log: its first record is a rotation marker, outside the
/// chain, naming the sequence and link the first kept record carries; that
/// record's bytes (and so every later link) are unchanged. A rotation
/// marker anywhere but first is an ordinary record and must chain.
#[test]
fn a_rotated_log_verifies_from_its_marker() {
    let marker = Record {
        kind: Kind::Refused,
        action: 0,
        seq: 1,
        time_s: 0,
        detail: REFUSED_ROTATED,
        path: "",
        intent: "",
        prev: L0,
    };
    let m = encode(&marker).unwrap();
    // Record 0 dropped; record 1 kept as it was.
    let mut log = m.clone();
    log.extend_from_slice(&R1);
    let head = verify(&log);
    assert_eq!((head.records, head.link, head.broken_at), (2, L1, None));
    // A marker that names the wrong link: the first kept record breaks.
    let wrong = encode(&Record { prev: L1, ..marker }).unwrap();
    let mut bad = wrong.clone();
    bad.extend_from_slice(&R1);
    assert_eq!(verify(&bad).broken_at, Some(1));
    // Not first: it has to chain like any record (the right one does; one
    // naming another link doesn't).
    let mut late = R0.to_vec();
    late.extend_from_slice(&m);
    assert_eq!(verify(&late).broken_at, None);
    let mut late = R0.to_vec();
    late.extend_from_slice(&wrong);
    assert_eq!(verify(&late).broken_at, Some(1));
    // Any other first record must still start at 0 with a zero link.
    assert_eq!(verify(&R1).broken_at, Some(0));
}
