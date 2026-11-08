// SPDX-License-Identifier: MPL-2.0
//! The client side of the Atrium surface protocol's NMC1 records
//! (`docs/ADR-073-atrium-surface-protocol-v1.md`), for a client built on
//! real std (the browser, OT-61 L5).
//!
//! A std bundle may not link libneo: libneo makes raw syscalls, and A4 allows
//! syscalls only from std's own trampoline. So the four requests a client
//! sends, and the reply it reads, are encoded here again, with no
//! dependencies and no I/O. The host tests hold both this file and libneo's
//! `atrium_protocol` to hand-laid bytes from ADR-073's tables, so a drift on
//! either side fails.
//!
//! Every field is little-endian. A request is a 24-byte header (magic
//! "NMC1", version 1, opcode, payload length, 4 zero bytes, request id), then
//! the identity (namespace, surface id, surface-cap provenance: 24 bytes),
//! then the verb's own fields. A reply is 56 bytes: the header with the
//! opcode's top bit set, then status, 4 zero bytes and the identity.

#![no_std]

pub const MAGIC: u32 = 0x3143_4d4e; // "NMC1" in little-endian bytes
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 24;
pub const MAX_MESSAGE_LEN: usize = 72;
pub const REPLY_LEN: usize = 56;
pub const REPLY_PAYLOAD_LEN: u32 = 32;
pub const REPLY_OPCODE_BIT: u16 = 0x8000;

pub const OP_CREATE_SURFACE: u16 = 1;
pub const OP_IMPORT_ATTACH_BUFFER: u16 = 2;
pub const OP_FOCUS_REQUEST: u16 = 4;
pub const OP_COMMIT_DAMAGE: u16 = 9;

pub const ROLE_TEXT: u32 = 2;
pub const FOCUS_REASON_USER_ACTIVATION: u32 = 1;

/// Who a request is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub namespace_id: u64,
    pub surface_id: u64,
    /// Provenance of the channel endpoint carrying this verb.
    pub surface_cap_provenance: u64,
}

/// One encoded request: the bytes and how many of them to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub bytes: [u8; MAX_MESSAGE_LEN],
    pub len: usize,
}

impl Message {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn put_u16(out: &mut [u8], at: usize, v: u16) {
    out[at..at + 2].copy_from_slice(&v.to_le_bytes());
}
fn put_u32(out: &mut [u8], at: usize, v: u32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn put_u64(out: &mut [u8], at: usize, v: u64) {
    out[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn get_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn get_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn get_u64(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

/// The header and identity every request starts with; the caller fills the
/// verb's fields at 48.. and the payload is `payload_len` bytes from 24.
fn request(opcode: u16, payload_len: usize, request_id: u64, identity: Identity) -> Message {
    let mut bytes = [0u8; MAX_MESSAGE_LEN];
    put_u32(&mut bytes, 0, MAGIC);
    put_u16(&mut bytes, 4, VERSION);
    put_u16(&mut bytes, 6, opcode);
    put_u32(&mut bytes, 8, payload_len as u32);
    put_u32(&mut bytes, 12, 0);
    put_u64(&mut bytes, 16, request_id);
    put_u64(&mut bytes, 24, identity.namespace_id);
    put_u64(&mut bytes, 32, identity.surface_id);
    put_u64(&mut bytes, 40, identity.surface_cap_provenance);
    Message {
        bytes,
        len: HEADER_LEN + payload_len,
    }
}

/// `CREATE_SURFACE`: role, width, height, 4 zero bytes, the client's tag.
pub fn create_surface(
    request_id: u64,
    identity: Identity,
    role: u32,
    width: u32,
    height: u32,
    client_tag: u64,
) -> Message {
    let mut m = request(OP_CREATE_SURFACE, 48, request_id, identity);
    put_u32(&mut m.bytes, 48, role);
    put_u32(&mut m.bytes, 52, width);
    put_u32(&mut m.bytes, 56, height);
    put_u64(&mut m.bytes, 64, client_tag);
    m
}

/// `IMPORT_ATTACH_BUFFER`: the transfer id and the provenance of the buffer
/// handle the client grants on the surface channel after the WM's `R`.
pub fn import_attach_buffer(
    request_id: u64,
    identity: Identity,
    transfer_id: u64,
    buffer_cap_provenance: u64,
) -> Message {
    let mut m = request(OP_IMPORT_ATTACH_BUFFER, 40, request_id, identity);
    put_u64(&mut m.bytes, 48, transfer_id);
    put_u64(&mut m.bytes, 56, buffer_cap_provenance);
    m
}

/// `FOCUS_REQUEST`: reason, 4 zero bytes, a request serial.
pub fn focus_request(
    request_id: u64,
    identity: Identity,
    reason: u32,
    request_serial: u64,
) -> Message {
    let mut m = request(OP_FOCUS_REQUEST, 40, request_id, identity);
    put_u32(&mut m.bytes, 48, reason);
    put_u64(&mut m.bytes, 56, request_serial);
    m
}

/// `COMMIT_DAMAGE`: the client serial (strictly increasing), then the
/// damaged rect as four u16s (x, y, width, height); that ends the record.
pub fn commit_damage(
    request_id: u64,
    identity: Identity,
    client_serial: u64,
    rect: (u16, u16, u16, u16),
) -> Message {
    let mut m = request(OP_COMMIT_DAMAGE, 40, request_id, identity);
    put_u64(&mut m.bytes, 48, client_serial);
    put_u16(&mut m.bytes, 56, rect.0);
    put_u16(&mut m.bytes, 58, rect.1);
    put_u16(&mut m.bytes, 60, rect.2);
    put_u16(&mut m.bytes, 62, rect.3);
    m
}

/// A reply the WM sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply {
    /// The request's opcode (the reply bit cleared).
    pub opcode: u16,
    pub request_id: u64,
    /// 0 is success; anything else is the protocol's error code.
    pub status: u32,
    pub identity: Identity,
}

/// Why a reply was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyError {
    /// Not 56 bytes.
    Length,
    /// Not NMC1 version 1, or the reply bit is clear.
    Header,
    /// A length or reserved field is wrong.
    Field,
}

pub fn decode_reply(b: &[u8]) -> Result<Reply, ReplyError> {
    if b.len() != REPLY_LEN {
        return Err(ReplyError::Length);
    }
    let opcode = get_u16(b, 6);
    if get_u32(b, 0) != MAGIC || get_u16(b, 4) != VERSION || opcode & REPLY_OPCODE_BIT == 0 {
        return Err(ReplyError::Header);
    }
    if get_u32(b, 8) != REPLY_PAYLOAD_LEN || get_u32(b, 12) != 0 || get_u32(b, 28) != 0 {
        return Err(ReplyError::Field);
    }
    Ok(Reply {
        opcode: opcode & !REPLY_OPCODE_BIT,
        request_id: get_u64(b, 16),
        status: get_u32(b, 24),
        identity: Identity {
            namespace_id: get_u64(b, 32),
            surface_id: get_u64(b, 40),
            surface_cap_provenance: get_u64(b, 48),
        },
    })
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../libneo/src/atrium_protocol.rs"]
mod wm_protocol;

#[cfg(test)]
mod tests;
