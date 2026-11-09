// SPDX-License-Identifier: MPL-2.0
//! L2.1: Arthur's action log, `NAL1` (THINK-2026-10-30-l2 §2). The WM is its
//! only writer (`/neofs/.system/actions.log`): every request, answer, lend,
//! refusal and done that passes its permission prompt, with the intent that
//! caused it. Records follow one another; each names the first 16 bytes of
//! the previous record's BLAKE3 (`prev`), so an edit or a removal in the
//! middle shows at the next record ([`verify`]).
//!
//! Record (little-endian): magic `NAL1`, version `u16` = 1, kind `u8`,
//! action `u8`, sequence `u64`, wall-clock seconds `u64`, detail `u32`, path
//! length `u16`, intent length `u16`, prev `[u8; 16]`, flags `u8`, 15
//! reserved zero bytes, then the path and the intent.
//!
//! Flags (byte 48, formerly reserved and always zero, so every older record
//! reads as flags 0): [`TIME_UTC`]. A record written before the session
//! confirmed the clock (`SYS_CLOCK_REALTIME_STATUS`, THINK-2026-11-02 §2.7)
//! has it clear, and its time is the computer's clock as read (possibly
//! local wall time), not UTC.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub const MAGIC: [u8; 4] = *b"NAL1";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 64;
pub const PREV_LEN: usize = 16;
/// The longest path and intent a record holds.
pub const TEXT_MAX: usize = 255;
/// The log file (the WM-only store).
pub const PATH: &str = "/neofs/.system/actions.log";

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Asked = 1,
    AllowedOnce = 2,
    AllowedAlways = 3,
    Denied = 4,
    LentFromRecord = 5,
    Refused = 6,
    Done = 7,
    GrantRevoked = 8,
    Undone = 9,
    /// F11 S3 (setup-menu THINK §4.4): a setup-menu choice. `path` is the
    /// page left (`welcome`, `timezone`, `comfort`, `done`), `intent` the
    /// choice (`begin`, a zone name, `rm=1 rt=0`, `try`); `action` and
    /// `detail` are 0. Appended at the wizard's end, each with its own time.
    Setup = 10,
}

impl Kind {
    pub const fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::Asked,
            2 => Self::AllowedOnce,
            3 => Self::AllowedAlways,
            4 => Self::Denied,
            5 => Self::LentFromRecord,
            6 => Self::Refused,
            7 => Self::Done,
            8 => Self::GrantRevoked,
            9 => Self::Undone,
            10 => Self::Setup,
            _ => return None,
        })
    }
}

/// A refusal's reason (`detail` of a `Refused` record).
pub const REFUSED_PATH: u32 = 1;
pub const REFUSED_READ_ONLY: u32 = 2;
pub const REFUSED_DENIED_THIS_SESSION: u32 = 3;
pub const REFUSED_LOCKED: u32 = 4;
/// The WM couldn't open the path to lend it (the grant, if any, isn't saved).
pub const REFUSED_OPEN: u32 = 5;
/// The log's chain was broken at the record whose index this record's
/// sequence names; the chain carries on from the last good one (the bytes
/// cut off are kept beside the log, [`BROKEN_PATH`]).
pub const REFUSED_CHAIN_BROKEN: u32 = 0xFFFF_FFFF;
/// The log was rotated: the oldest records were dropped. This record, the
/// first in the file, stands outside the chain as its header: its `seq` and
/// `prev` are those the next record carries (the first record kept, whose
/// bytes and link are unchanged).
pub const REFUSED_ROTATED: u32 = 0xFFFF_FFFE;
/// The log was shorter than the WM last left it (records dropped off its
/// end) and carries on from what is there.
pub const REFUSED_TRUNCATED: u32 = 0xFFFF_FFFD;
/// A `Done` whose cap the WM revoked without the client reporting it (its
/// next request, or the session's end).
pub const DONE_UNREPORTED: u32 = 0xFFFF_FFFF;
/// Where a repair keeps the bytes it cut off the log.
pub const BROKEN_PATH: &str = "/neofs/.system/actions.broken";

/// `flags` bit 0: `time_s` is UTC (the kernel had confirmed realtime as UTC
/// when the record was written). Clear: the computer's clock as read.
pub const TIME_UTC: u8 = 1;
/// Every flag bit this version defines; a record with any other bit set is
/// not well formed.
pub const FLAGS_KNOWN: u8 = TIME_UTC;

/// Whether `record` is a rotation marker (the only record that may open a
/// log at a sequence other than 0, with a link other than zero).
pub fn is_rotation(record: &Record<'_>) -> bool {
    record.kind == Kind::Refused && record.detail == REFUSED_ROTATED
}

/// One record's fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record<'a> {
    pub kind: Kind,
    /// 0 none, 1 read, 2 list, 3 write (the atrium `PERMISSION_*` ids).
    pub action: u8,
    pub seq: u64,
    pub time_s: u64,
    pub detail: u32,
    pub path: &'a str,
    pub intent: &'a str,
    pub prev: [u8; PREV_LEN],
    /// [`TIME_UTC`] or 0.
    pub flags: u8,
}

/// The bytes of `record`; `None`: a text is longer than [`TEXT_MAX`], or a
/// flag this version doesn't define is set.
pub fn encode(record: &Record<'_>) -> Option<Vec<u8>> {
    if record.path.len() > TEXT_MAX
        || record.intent.len() > TEXT_MAX
        || record.flags & !FLAGS_KNOWN != 0
    {
        return None;
    }
    let mut out = Vec::with_capacity(HEADER_LEN + record.path.len() + record.intent.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(record.kind as u8);
    out.push(record.action);
    out.extend_from_slice(&record.seq.to_le_bytes());
    out.extend_from_slice(&record.time_s.to_le_bytes());
    out.extend_from_slice(&record.detail.to_le_bytes());
    out.extend_from_slice(&(record.path.len() as u16).to_le_bytes());
    out.extend_from_slice(&(record.intent.len() as u16).to_le_bytes());
    out.extend_from_slice(&record.prev);
    out.push(record.flags);
    out.extend_from_slice(&[0u8; 15]);
    out.extend_from_slice(record.path.as_bytes());
    out.extend_from_slice(record.intent.as_bytes());
    Some(out)
}

/// The first 16 bytes of `bytes`' BLAKE3: the next record's `prev`.
pub fn link(bytes: &[u8]) -> [u8; PREV_LEN] {
    let mut out = [0u8; PREV_LEN];
    out.copy_from_slice(&blake3::hash(bytes).as_bytes()[..PREV_LEN]);
    out
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

/// The record at the start of `bytes` and its length, or `None` if it isn't
/// a well-formed record (magic, version, kind, known flags, reserved zero,
/// lengths, UTF-8).
pub fn decode(bytes: &[u8]) -> Option<(Record<'_>, usize)> {
    if bytes.len() < HEADER_LEN
        || bytes[..4] != MAGIC
        || u16_at(bytes, 4) != VERSION
        || bytes[48] & !FLAGS_KNOWN != 0
        || bytes[49..64].iter().any(|&b| b != 0)
    {
        return None;
    }
    let kind = Kind::from_u8(bytes[6])?;
    let (path_len, intent_len) = (
        usize::from(u16_at(bytes, 28)),
        usize::from(u16_at(bytes, 30)),
    );
    if path_len > TEXT_MAX || intent_len > TEXT_MAX {
        return None;
    }
    let len = HEADER_LEN + path_len + intent_len;
    if bytes.len() < len {
        return None;
    }
    let mut prev = [0u8; PREV_LEN];
    prev.copy_from_slice(&bytes[32..48]);
    let path = core::str::from_utf8(&bytes[HEADER_LEN..HEADER_LEN + path_len]).ok()?;
    let intent = core::str::from_utf8(&bytes[HEADER_LEN + path_len..len]).ok()?;
    Some((
        Record {
            kind,
            action: bytes[7],
            seq: u64_at(bytes, 8),
            time_s: u64_at(bytes, 16),
            detail: u32_at(bytes, 24),
            path,
            intent,
            prev,
            flags: bytes[48],
        },
        len,
    ))
}

/// A log's state after [`verify`]: how many records are good, the byte
/// length they span, and the `prev` the next record must carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub records: u64,
    pub len: usize,
    pub link: [u8; PREV_LEN],
    /// `Some(i)`: record `i` (counting from 0) is the first bad one; the head
    /// is the last good record's.
    pub broken_at: Option<u64>,
}

/// Walk `log` from the start: each record well-formed, its sequence the
/// next number, and its `prev` the link of the one before (zero for the
/// first). A bad or truncated record stops the walk. A rotated log opens
/// with a rotation marker ([`is_rotation`]), outside the chain, whose `seq`
/// and `prev` are where the walk starts; `records` is then the next sequence
/// number, not a count of what the file holds.
pub fn verify(log: &[u8]) -> Head {
    let mut head = Head {
        records: 0,
        len: 0,
        link: [0u8; PREV_LEN],
        broken_at: None,
    };
    while head.len < log.len() {
        let rest = &log[head.len..];
        if head.len == 0
            && let Some((record, len)) = decode(rest)
            && is_rotation(&record)
        {
            head.records = record.seq;
            head.link = record.prev;
            head.len = len;
            continue;
        }
        let ok = match decode(rest) {
            Some((record, len)) if record.seq == head.records && record.prev == head.link => {
                Some(len)
            }
            _ => None,
        };
        let Some(len) = ok else {
            head.broken_at = Some(head.records);
            break;
        };
        head.link = link(&rest[..len]);
        head.records += 1;
        head.len += len;
    }
    head
}

#[cfg(test)]
mod tests;
