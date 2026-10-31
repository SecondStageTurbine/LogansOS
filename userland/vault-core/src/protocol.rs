// SPDX-License-Identifier: MPL-2.0
//! Vault protocol v1 wire envelope. Channel possession is the authority.

pub const MAGIC: [u8; 4] = *b"NVP1";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 24;
pub const KIND_REQUEST: u8 = 0;
pub const KIND_RESPONSE: u8 = 1;
pub const OP_UNLOCK: u16 = 1;
pub const OP_LOCK: u16 = 2;
pub const OP_STORE: u16 = 3;
pub const OP_RETRIEVE: u16 = 4;

/// Scenario-only test verbs (Phase-B QEMU proof). Not part of the caller-facing
/// operation surface: they let the harness drive the vault's own store into the
/// tamper/weak-parameter states so the header-AAD binding (§F4) and KDF floor
/// (§F5) are exercised on the **live** open path. A production vault does not
/// honor these opcodes.
pub const OP_TEST_CORRUPT_HEADER: u16 = 0xF001;
pub const OP_TEST_INJECT_WEAK: u16 = 0xF002;
pub const OP_TEST_SHUTDOWN: u16 = 0xF003;

pub const STATUS_OK: i32 = 0;
pub const STATUS_BAD_REQUEST: i32 = 1;
pub const STATUS_NOT_AUTHORIZED: i32 = 2;
pub const STATUS_LOCKED: i32 = 3;
pub const STATUS_AUTH_FAILED: i32 = 4;
pub const STATUS_OUTPUT_TOO_SMALL: i32 = 5;
pub const STATUS_BUSY: i32 = 6;
pub const STATUS_INTERNAL: i32 = 7;
pub const STATUS_NOT_FOUND: i32 = 8;
pub const STATUS_WEAK_PARAMS: i32 = 9;

pub const UNLOCK_PAYLOAD_LEN: u32 = 24;
pub const LOCK_PAYLOAD_LEN: u32 = 8;
pub const STORE_PAYLOAD_LEN: u32 = 40;
pub const RETRIEVE_PAYLOAD_LEN: u32 = 40;

/// Map a [`crate::session::VaultError`] to its coarse public status. The
/// mapping is deliberately lossy where the threat model requires it:
/// wrong-password, wrong-key, tampered-header, and corrupt-record all collapse
/// to [`STATUS_AUTH_FAILED`], leaking no oracle (ADR-075 §5.2).
pub const fn status_for(err: crate::session::VaultError) -> i32 {
    use crate::session::VaultError::*;
    match err {
        Locked => STATUS_LOCKED,
        AuthFailed => STATUS_AUTH_FAILED,
        BadRequest => STATUS_BAD_REQUEST,
        NotFound => STATUS_NOT_FOUND,
        OutputTooSmall => STATUS_OUTPUT_TOO_SMALL,
        Full => STATUS_BUSY,
        Rng => STATUS_INTERNAL,
        WeakParams => STATUS_WEAK_PARAMS,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub kind: u8,
    pub opcode: u16,
    pub status: i32,
    pub request_id: u64,
    pub payload_len: u32,
}

impl Header {
    pub fn encode(self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[..4].copy_from_slice(&MAGIC);
        out[4] = VERSION;
        out[5] = self.kind;
        out[6..8].copy_from_slice(&self.opcode.to_le_bytes());
        out[8..12].copy_from_slice(&self.status.to_le_bytes());
        out[12..20].copy_from_slice(&self.request_id.to_le_bytes());
        out[20..24].copy_from_slice(&self.payload_len.to_le_bytes());
        out
    }

    /// Parse a wire header, validating magic and version. Returns `None` on a
    /// short buffer, wrong magic, or an unsupported version — the vault rejects
    /// such a frame rather than acting on attacker-shaped bytes.
    pub fn decode(bytes: &[u8]) -> Option<Header> {
        if bytes.len() < HEADER_LEN || bytes[..4] != MAGIC || bytes[4] != VERSION {
            return None;
        }
        Some(Header {
            kind: bytes[5],
            opcode: u16::from_le_bytes([bytes[6], bytes[7]]),
            status: i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            request_id: u64::from_le_bytes([
                bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18],
                bytes[19],
            ]),
            payload_len: u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
        })
    }

    /// A request header for `opcode` carrying `payload_len` payload bytes.
    pub fn request(opcode: u16, request_id: u64, payload_len: u32) -> Header {
        Header {
            kind: KIND_REQUEST,
            opcode,
            status: 0,
            request_id,
            payload_len,
        }
    }

    /// A response header echoing `opcode`/`request_id` with a coarse `status`.
    pub fn response(opcode: u16, request_id: u64, status: i32, payload_len: u32) -> Header {
        Header {
            kind: KIND_RESPONSE,
            opcode,
            status,
            request_id,
            payload_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verb_envelope_byte_oracles() {
        for (opcode, payload_len, opcode_bytes, payload_len_bytes) in [
            (
                OP_UNLOCK,
                UNLOCK_PAYLOAD_LEN,
                [0x01, 0x00],
                [0x18, 0x00, 0x00, 0x00],
            ),
            (
                OP_LOCK,
                LOCK_PAYLOAD_LEN,
                [0x02, 0x00],
                [0x08, 0x00, 0x00, 0x00],
            ),
            (
                OP_STORE,
                STORE_PAYLOAD_LEN,
                [0x03, 0x00],
                [0x28, 0x00, 0x00, 0x00],
            ),
            (
                OP_RETRIEVE,
                RETRIEVE_PAYLOAD_LEN,
                [0x04, 0x00],
                [0x28, 0x00, 0x00, 0x00],
            ),
        ] {
            let actual = Header {
                kind: KIND_REQUEST,
                opcode,
                status: 0,
                request_id: 0x0102_0304_0506_0708,
                payload_len,
            }
            .encode();
            let mut expected = [0u8; HEADER_LEN];
            expected[..6].copy_from_slice(&[0x4e, 0x56, 0x50, 0x31, 0x01, 0x00]);
            // These fields are deliberately literal, not encoded with the
            // production helpers: an endian/opcode/length bug must fail.
            expected[6..8].copy_from_slice(&opcode_bytes);
            expected[12..20].copy_from_slice(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
            expected[20..24].copy_from_slice(&payload_len_bytes);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn decode_is_the_inverse_of_encode() {
        let h = Header::response(OP_RETRIEVE, 0x1122_3344_5566_7788, STATUS_OK, 12);
        let bytes = h.encode();
        assert_eq!(Header::decode(&bytes), Some(h));
    }

    #[test]
    fn decode_rejects_bad_magic_version_and_short_frames() {
        let mut bytes = Header::request(OP_UNLOCK, 1, 0).encode();
        assert!(Header::decode(&bytes[..HEADER_LEN - 1]).is_none(), "short");
        let mut bad_magic = bytes;
        bad_magic[0] ^= 0xff;
        assert!(Header::decode(&bad_magic).is_none(), "bad magic");
        bytes[4] = VERSION + 1;
        assert!(Header::decode(&bytes).is_none(), "bad version");
    }

    #[test]
    fn hand_laid_response_header_oracle() {
        // A LOCKED response to a RETRIEVE (request_id 0x0102030405060708),
        // no payload — asserted against literal bytes independent of encode().
        let actual =
            Header::response(OP_RETRIEVE, 0x0102_0304_0506_0708, STATUS_LOCKED, 0).encode();
        let expected: [u8; HEADER_LEN] = [
            0x4e, 0x56, 0x50, 0x31, // "NVP1"
            0x01, // version
            0x01, // kind = RESPONSE
            0x04, 0x00, // opcode = OP_RETRIEVE (4)
            0x03, 0x00, 0x00, 0x00, // status = STATUS_LOCKED (3)
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // request_id LE
            0x00, 0x00, 0x00, 0x00, // payload_len = 0
        ];
        assert_eq!(actual, expected);
    }
}
