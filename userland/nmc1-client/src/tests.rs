// SPDX-License-Identifier: MPL-2.0
//! The byte oracle. Each record here is laid out by hand from ADR-073's
//! tables (the offsets in the module doc), byte by byte, not produced by any
//! encoder. Both this crate and libneo's `atrium_protocol` (what the WM
//! decodes with) must produce exactly these bytes.

extern crate std;

use super::*;
use crate::wm_protocol as wm;

/// The probe app's namespace, so the bytes are a realistic case:
/// 0x5052_4F42_4150_0001 is `01 00 50 41 42 4f 52 50` in little-endian bytes.
const ID: Identity = Identity {
    namespace_id: 0x5052_4F42_4150_0001,
    surface_id: 1,
    surface_cap_provenance: 0xAB,
};

fn wm_id() -> wm::Identity {
    wm::Identity {
        namespace_id: ID.namespace_id,
        surface_id: ID.surface_id,
        surface_cap_provenance: ID.surface_cap_provenance,
    }
}

/// The 24 header bytes for `opcode`, a `payload` byte payload, request id 7.
fn header(opcode: u8, payload: u8) -> [u8; 24] {
    [
        0x4e, 0x4d, 0x43, 0x31, // "NMC1"
        0x01, 0x00, // version 1
        opcode, 0x00, // opcode
        payload, 0x00, 0x00, 0x00, // payload length
        0x00, 0x00, 0x00, 0x00, // flags: zero
        0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // request id 7
    ]
}

/// The identity's 24 bytes.
const IDENTITY: [u8; 24] = [
    0x01, 0x00, 0x50, 0x41, 0x42, 0x4f, 0x52, 0x50, // namespace
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // surface id 1
    0xab, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // surface-cap provenance
];

fn record(opcode: u8, payload: u8, verb: &[u8]) -> std::vec::Vec<u8> {
    let mut v = std::vec::Vec::new();
    v.extend_from_slice(&header(opcode, payload));
    v.extend_from_slice(&IDENTITY);
    v.extend_from_slice(verb);
    assert_eq!(
        v.len(),
        24 + payload as usize,
        "the hand-laid record's own length"
    );
    v
}

fn wm_bytes(req: wm::Request) -> std::vec::Vec<u8> {
    let (b, n) = wm::encode_request(7, req);
    b[..n].to_vec()
}

#[test]
fn create_surface_bytes() {
    let want = record(
        1,
        48,
        &[
            0x02, 0x00, 0x00, 0x00, // role: TEXT
            0x80, 0x02, 0x00, 0x00, // width 640
            0x90, 0x01, 0x00, 0x00, // height 400
            0x00, 0x00, 0x00, 0x00, // reserved
            0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // client tag 0x42
        ],
    );
    let ours = create_surface(7, ID, ROLE_TEXT, 640, 400, 0x42);
    assert_eq!(ours.as_bytes(), &want[..]);
    let theirs = wm_bytes(wm::Request::Create(wm::CreateSurface {
        identity: wm_id(),
        role: wm::ROLE_TEXT,
        width: 640,
        height: 400,
        client_tag: 0x42,
    }));
    assert_eq!(theirs, want, "libneo encodes CREATE_SURFACE the same way");
}

#[test]
fn import_attach_bytes() {
    let want = record(
        2,
        40,
        &[
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // transfer id 5
            0x01, 0x00, 0x55, 0x42, 0x42, 0x4f, 0x52, 0x50, // buffer provenance
        ],
    );
    let ours = import_attach_buffer(7, ID, 5, 0x5052_4F42_4255_0001);
    assert_eq!(ours.as_bytes(), &want[..]);
    let theirs = wm_bytes(wm::Request::ImportAttach(wm::ImportAttachBuffer {
        identity: wm_id(),
        transfer_id: 5,
        buffer_cap_provenance: 0x5052_4F42_4255_0001,
    }));
    assert_eq!(
        theirs, want,
        "libneo encodes IMPORT_ATTACH_BUFFER the same way"
    );
}

#[test]
fn focus_request_bytes() {
    let want = record(
        4,
        40,
        &[
            0x01, 0x00, 0x00, 0x00, // reason: user activation
            0x00, 0x00, 0x00, 0x00, // reserved
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // request serial 3
        ],
    );
    let ours = focus_request(7, ID, FOCUS_REASON_USER_ACTIVATION, 3);
    assert_eq!(ours.as_bytes(), &want[..]);
    let theirs = wm_bytes(wm::Request::Focus(wm::FocusRequest {
        identity: wm_id(),
        reason: wm::FOCUS_REASON_USER_ACTIVATION,
        request_serial: 3,
    }));
    assert_eq!(theirs, want, "libneo encodes FOCUS_REQUEST the same way");
}

#[test]
fn commit_damage_bytes() {
    let want = record(
        9,
        40,
        &[
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // client serial 2
            0x00, 0x00, // x 0
            0x00, 0x00, // y 0
            0x80, 0x02, // width 640
            0x90, 0x01, // height 400
        ],
    );
    let ours = commit_damage(7, ID, 2, (0, 0, 640, 400));
    assert_eq!(ours.as_bytes(), &want[..]);
    let theirs = wm_bytes(wm::Request::CommitDamage(wm::CommitDamage {
        identity: wm_id(),
        client_serial: 2,
        x: 0,
        y: 0,
        width: 640,
        height: 400,
    }));
    assert_eq!(theirs, want, "libneo encodes COMMIT_DAMAGE the same way");
}

/// A success reply to request 7 (CREATE_SURFACE), laid out by hand.
fn reply_bytes(status: u8) -> [u8; 56] {
    let mut r = [0u8; 56];
    r[..24].copy_from_slice(&[
        0x4e, 0x4d, 0x43, 0x31, 0x01, 0x00, // "NMC1", version 1
        0x01, 0x80, // opcode 1 with the reply bit
        0x20, 0x00, 0x00, 0x00, // payload length 32
        0x00, 0x00, 0x00, 0x00, // flags
        0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // request id 7
    ]);
    r[24] = status; // status, then 4 reserved zero bytes at 28
    r[32..56].copy_from_slice(&IDENTITY);
    r
}

#[test]
fn reply_bytes_decode_and_match_libneo() {
    let want = Reply {
        opcode: OP_CREATE_SURFACE,
        request_id: 7,
        status: 0,
        identity: ID,
    };
    assert_eq!(decode_reply(&reply_bytes(0)), Ok(want));
    let theirs = wm::encode_reply(wm::Reply {
        opcode: wm::OP_CREATE_SURFACE,
        request_id: 7,
        status: 0,
        identity: wm_id(),
    });
    assert_eq!(
        theirs,
        reply_bytes(0),
        "libneo encodes a reply the same way"
    );
    assert_eq!(decode_reply(&reply_bytes(5)).map(|r| r.status), Ok(5));
}

#[test]
fn bad_replies_are_refused() {
    assert_eq!(decode_reply(&reply_bytes(0)[..55]), Err(ReplyError::Length));
    let mut b = reply_bytes(0);
    b[7] = 0x00; // reply bit clear: a request, not a reply
    assert_eq!(decode_reply(&b), Err(ReplyError::Header));
    let mut b = reply_bytes(0);
    b[0] = 0x4f; // wrong magic
    assert_eq!(decode_reply(&b), Err(ReplyError::Header));
    let mut b = reply_bytes(0);
    b[28] = 1; // reserved field set
    assert_eq!(decode_reply(&b), Err(ReplyError::Field));
    let mut b = reply_bytes(0);
    b[8] = 0x21; // payload length 33
    assert_eq!(decode_reply(&b), Err(ReplyError::Field));
}
