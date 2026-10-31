// SPDX-License-Identifier: MPL-2.0
//! OT-88 S1 (the part that needs no app shell): the `NEOSET1` settings
//! record, with no I/O. `docs/THINK-2026-10-29-ot88-app-model.md` §5.2-5.3
//! is the spec; this crate's docs are its byte layout.
//!
//! - [`encode`] / [`decode`]: one record, every key with its type and range
//!   checked ([`Key`]);
//! - [`select`]: two copies (`/neofs/.system/settings.a` and `.b`), the
//!   valid one with the higher generation wins, and a write goes only to the
//!   file not holding it ([`plan`]), so a cut can damage only the file being
//!   written. The same model as `NEOCRED1` (OT-66 THINK §4.3);
//! - [`Settings::with`]: `OP_SETTING {key, value}`'s check (a bad value is
//!   refused and changes nothing);
//! - [`migrate`]: the first record from today's presence files.
//!
//! The WM is the store's only writer (§5.3): it reads and writes the files.
//!
//! ## Layout (little-endian)
//!
//! | Offset | Length | Field |
//! |---|---|---|
//! | 0 | 8 | Magic `NEOSET1\0` |
//! | 8 | 2 | Version, 1 |
//! | 10 | 2 | Entry count `n`, at most [`MAX_ENTRIES`] |
//! | 12 | 8 | Generation, from 1 |
//! | 20 | 8·`n` | Entries: key id `u16`, type `u8`, reserved `u8` (0), value `u32` |
//! | 20 + 8·`n` | 16 | The first 16 bytes of BLAKE3 over bytes 0..20 + 8·`n` |
//!
//! Entries are in strictly ascending key order. A key this version doesn't
//! know, a type that isn't the key's, a value outside its range, a non-zero
//! reserved byte or a duplicate refuses the whole record, which then reads
//! as absent (its defaults: the idle lock on). A key the record lacks reads
//! as its default. The entry is 8 bytes rather than 7 so every value is a
//! 4-byte field at a 4-byte offset; the reserved byte is the pad.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub const MAGIC: [u8; 8] = *b"NEOSET1\0";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 20;
pub const ENTRY_LEN: usize = 8;
pub const CHECKSUM_LEN: usize = 16;
/// The most entries a record may hold (room for later keys).
pub const MAX_ENTRIES: usize = 16;

pub const SYSTEM_DIR: &str = "/neofs/.system";
pub const SETTINGS_A: &str = "/neofs/.system/settings.a";
pub const SETTINGS_B: &str = "/neofs/.system/settings.b";
/// Today's presence files (DS-1): read once by [`migrate`], left on disk.
pub const RT_PRESENCE: &str = "/neofs/atrium-reduce-transparency";
pub const RM_PRESENCE: &str = "/neofs/atrium-reduce-motion";

/// A value's type (the entry's byte 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Type {
    /// 0 or 1.
    Bool = 1,
    /// One of a fixed list of choices, numbered from 0.
    Choice = 2,
    /// Minutes, one of a fixed list.
    Minutes = 3,
}

/// The keys (§5.1). Ids are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Key {
    /// `Bool`; default off.
    ReduceTransparency = 1,
    /// `Choice`: 0 Full, 1 Reduced, 2 Off ([`Motion`]); default Full.
    ReduceMotion = 2,
    /// `Minutes`: 0 (Off), 5, 10 or 30; default 10 (J.c., OT-66 THINK).
    IdleLockMinutes = 3,
}

pub const KEYS: [Key; 3] = [
    Key::ReduceTransparency,
    Key::ReduceMotion,
    Key::IdleLockMinutes,
];

/// The idle-lock choices, in minutes (0: off; Super+L still locks).
pub const IDLE_CHOICES: [u32; 4] = [0, 5, 10, 30];

impl Key {
    pub const fn from_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::ReduceTransparency),
            2 => Some(Self::ReduceMotion),
            3 => Some(Self::IdleLockMinutes),
            _ => None,
        }
    }

    pub const fn id(self) -> u16 {
        self as u16
    }

    pub const fn kind(self) -> Type {
        match self {
            Self::ReduceTransparency => Type::Bool,
            Self::ReduceMotion => Type::Choice,
            Self::IdleLockMinutes => Type::Minutes,
        }
    }

    /// Whether `value` is in this key's range.
    pub const fn valid(self, value: u32) -> bool {
        match self {
            Self::ReduceTransparency => value <= 1,
            Self::ReduceMotion => value <= 2,
            Self::IdleLockMinutes => matches!(value, 0 | 5 | 10 | 30),
        }
    }

    /// Whether a client (the settings app) may set this key with
    /// `OP_SETTING`. The idle lock is the WM's alone (the PM's #222 bounce): a
    /// client that could set it to 0 would turn the auto-lock off, for good.
    /// A key added later is WM-only unless it is listed here.
    pub const fn client_writable(self) -> bool {
        match self {
            Self::ReduceTransparency | Self::ReduceMotion => true,
            Self::IdleLockMinutes => false,
        }
    }

    pub const fn default_value(self) -> u32 {
        match self {
            Self::ReduceTransparency | Self::ReduceMotion => 0,
            Self::IdleLockMinutes => 10,
        }
    }
}

/// Reduce Motion's choices (neo-atrium's `motion::Profile`, in order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Full,
    Reduced,
    Off,
}

impl Motion {
    const fn from_value(value: u32) -> Self {
        match value {
            1 => Self::Reduced,
            2 => Self::Off,
            _ => Self::Full,
        }
    }

    const fn value(self) -> u32 {
        match self {
            Self::Full => 0,
            Self::Reduced => 1,
            Self::Off => 2,
        }
    }
}

/// Every setting's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub reduce_transparency: bool,
    pub reduce_motion: Motion,
    /// One of [`IDLE_CHOICES`].
    pub idle_lock_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            reduce_transparency: false,
            reduce_motion: Motion::Full,
            idle_lock_minutes: 10,
        }
    }
}

/// Why `OP_SETTING` was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetError {
    /// No such key.
    Key,
    /// Outside the key's range.
    Value,
}

impl Settings {
    pub const fn get(&self, key: Key) -> u32 {
        match key {
            Key::ReduceTransparency => self.reduce_transparency as u32,
            Key::ReduceMotion => self.reduce_motion.value(),
            Key::IdleLockMinutes => self.idle_lock_minutes,
        }
    }

    /// `key` set to `value`, both checked first; `self` is unchanged.
    pub fn with(&self, key: u16, value: u32) -> Result<Self, SetError> {
        let key = Key::from_id(key).ok_or(SetError::Key)?;
        if !key.valid(value) {
            return Err(SetError::Value);
        }
        let mut next = *self;
        match key {
            Key::ReduceTransparency => next.reduce_transparency = value == 1,
            Key::ReduceMotion => next.reduce_motion = Motion::from_value(value),
            Key::IdleLockMinutes => next.idle_lock_minutes = value,
        }
        Ok(next)
    }

    /// The idle time in nanoseconds (0: the idle lock is off).
    pub const fn idle_ns(&self) -> u64 {
        self.idle_lock_minutes as u64 * 60_000_000_000
    }
}

/// A decoded record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub generation: u64,
    pub settings: Settings,
}

/// Why bytes are not a record. Every one is refused alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// Shorter than a header, or not exactly the length the count gives.
    Length,
    Magic,
    Version,
    /// More than [`MAX_ENTRIES`].
    Count,
    /// The checksum doesn't match (a torn write).
    Checksum,
    /// Generation 0.
    Generation,
    /// An unknown key, or keys out of order (or repeated).
    Key,
    /// A type that isn't the key's, or a non-zero reserved byte.
    Type,
    /// A value outside its key's range.
    Value,
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn checksum(prefix: &[u8]) -> [u8; CHECKSUM_LEN] {
    let mut out = [0u8; CHECKSUM_LEN];
    out.copy_from_slice(&blake3::hash(prefix).as_bytes()[..CHECKSUM_LEN]);
    out
}

/// The record's bytes: every key, in key order.
pub fn encode(settings: &Settings, generation: u64) -> Vec<u8> {
    let n = KEYS.len();
    let mut b = Vec::with_capacity(HEADER_LEN + ENTRY_LEN * n + CHECKSUM_LEN);
    b.extend_from_slice(&MAGIC);
    b.extend_from_slice(&VERSION.to_le_bytes());
    b.extend_from_slice(&(n as u16).to_le_bytes());
    b.extend_from_slice(&generation.to_le_bytes());
    for key in KEYS {
        b.extend_from_slice(&key.id().to_le_bytes());
        b.push(key.kind() as u8);
        b.push(0);
        b.extend_from_slice(&settings.get(key).to_le_bytes());
    }
    let sum = checksum(&b);
    b.extend_from_slice(&sum);
    b
}

/// Read one file's bytes as a record. The checksum is checked before any
/// entry is trusted.
pub fn decode(bytes: &[u8]) -> Result<Record, RecordError> {
    if bytes.len() < HEADER_LEN + CHECKSUM_LEN {
        return Err(RecordError::Length);
    }
    if bytes[0..8] != MAGIC {
        return Err(RecordError::Magic);
    }
    if le16(bytes, 8) != VERSION {
        return Err(RecordError::Version);
    }
    let n = usize::from(le16(bytes, 10));
    if n > MAX_ENTRIES {
        return Err(RecordError::Count);
    }
    let body = HEADER_LEN + ENTRY_LEN * n;
    if bytes.len() != body + CHECKSUM_LEN {
        return Err(RecordError::Length);
    }
    if bytes[body..] != checksum(&bytes[..body]) {
        return Err(RecordError::Checksum);
    }
    let generation = u64::from_le_bytes(bytes[12..20].try_into().expect("8 bytes"));
    if generation == 0 {
        return Err(RecordError::Generation);
    }
    let mut settings = Settings::default();
    let mut last = 0u16;
    for i in 0..n {
        let at = HEADER_LEN + ENTRY_LEN * i;
        let id = le16(bytes, at);
        let key = Key::from_id(id).ok_or(RecordError::Key)?;
        if id <= last {
            return Err(RecordError::Key);
        }
        last = id;
        if bytes[at + 2] != key.kind() as u8 || bytes[at + 3] != 0 {
            return Err(RecordError::Type);
        }
        settings = settings
            .with(id, le32(bytes, at + 4))
            .map_err(|_| RecordError::Value)?;
    }
    Ok(Record {
        generation,
        settings,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    A,
    B,
}

impl File {
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    pub const fn path(self) -> &'static str {
        match self {
            Self::A => SETTINGS_A,
            Self::B => SETTINGS_B,
        }
    }
}

/// The record in force, and the file holding it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Current {
    pub file: File,
    pub record: Record,
}

/// Read both files (`None`: absent or unreadable) and pick the record: the
/// valid one with the higher generation; on equal generations `a`.
pub fn select(a: Option<&[u8]>, b: Option<&[u8]>) -> Option<Current> {
    let a = a.and_then(|bytes| decode(bytes).ok());
    let b = b.and_then(|bytes| decode(bytes).ok());
    match (a, b) {
        (Some(a), Some(b)) if b.generation > a.generation => Some(Current {
            file: File::B,
            record: b,
        }),
        (Some(a), _) => Some(Current {
            file: File::A,
            record: a,
        }),
        (None, Some(b)) => Some(Current {
            file: File::B,
            record: b,
        }),
        (None, None) => None,
    }
}

/// Where and what to write for `settings`: the file not holding `current`,
/// at the next generation (the first record: `a`, generation 1). `None`
/// when the generation can't advance.
pub fn plan(current: Option<&Current>, settings: &Settings) -> Option<(File, Vec<u8>)> {
    let (file, generation) = match current {
        None => (File::A, 1),
        Some(c) => (c.file.other(), c.record.generation.checked_add(1)?),
    };
    Some((file, encode(settings, generation)))
}

/// The first record's settings, from today's presence files: each one
/// present means its reduction is on (Reduce Motion: Reduced). The rest
/// are defaults.
pub fn migrate(reduce_transparency: bool, reduce_motion: bool) -> Settings {
    Settings {
        reduce_transparency,
        reduce_motion: if reduce_motion {
            Motion::Reduced
        } else {
            Motion::Full
        },
        ..Settings::default()
    }
}

#[cfg(test)]
mod tests;
