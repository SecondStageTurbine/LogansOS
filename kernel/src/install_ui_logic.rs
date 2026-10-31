// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: the installer's userland ABI (`docs/THINK-2026-10-27-ot87-installer.md`
//! §4.5), pure so the host oracle (`tools/install-ui-abi-tests`) checks it
//! against hand-laid bytes and against libneo's decoder.
//!
//! - `SYS_INSTALL_LIST(buf, len) -> count`: one [`ROW_BYTES`] record per
//!   enumerated disk, as many as fit in `len`; the return is the number of
//!   disks, so a short buffer can be sized and retried.
//! - `SYS_INSTALL_RUN(slot, generation, start) -> status`: installs to a disk
//!   the last list named. `start` is [`action::INSTALL`] or
//!   [`action::FINISH`]. The status is [`STATUS_INSTALLED`], a refusal code
//!   (made before any write), [`STATUS_STOPPED`] plus the step that stopped,
//!   or a negative errno.
//!
//! The record, little-endian:
//!
//! | Offset | Size | Field |
//! |---|---|---|
//! | 0 | 1 | version, [`ROW_VERSION`] |
//! | 1 | 1 | class ([`class`]) |
//! | 2 | 1 | action ([`action`]); `NONE` when refused |
//! | 3 | 1 | refusal ([`refusal`]); `NONE` when offered |
//! | 4 | 1 | flags: bit 0, the disk identifies itself |
//! | 5 | 1 | model length, at most [`MODEL_BYTES`] |
//! | 6 | 1 | serial length, at most [`SERIAL_BYTES`] |
//! | 7 | 1 | zero |
//! | 8 | 4 | registry slot |
//! | 12 | 4 | logical sector size |
//! | 16 | 8 | registry generation |
//! | 24 | 8 | capacity in bytes |
//! | 32 | 48 | model, ASCII, zero-padded |
//! | 80 | 32 | serial, ASCII, zero-padded |
//! | 112 | 16 | zero |

pub const ROW_BYTES: usize = 128;
pub const ROW_VERSION: u8 = 1;
pub const MODEL_BYTES: usize = 48;
pub const SERIAL_BYTES: usize = 32;
pub const FLAG_IDENTITY: u8 = 1 << 0;

/// What a disk holds (§4.5's classes; foreign kinds are R4b's).
pub mod class {
    pub const BLANK: u8 = 0;
    pub const INCOMPLETE: u8 = 1;
    pub const INSTALLED: u8 = 2;
    pub const FOREIGN: u8 = 3;
    /// The disk couldn't be read.
    pub const UNREADABLE: u8 = 4;
}

/// What the user may do with a disk; also `SYS_INSTALL_RUN`'s `start`.
pub mod action {
    pub const NONE: u8 = 0;
    /// "Install": a blank disk, W1–W6.
    pub const INSTALL: u8 = 1;
    /// "Finish installing": an incomplete LogansOS disk, W3–W6.
    pub const FINISH: u8 = 2;
}

/// Why a disk is refused (§4.5's reasons, then the preflight's). Also
/// `SYS_INSTALL_RUN`'s status for a refusal.
pub mod refusal {
    pub const NONE: u8 = 0;
    /// "This is the disk LogansOS is running from."
    pub const BOOT_DISK: u8 = 1;
    /// "Protected."
    pub const PROTECTED: u8 = 2;
    /// "Too small: needs 2 GiB."
    pub const TOO_SMALL: u8 = 3;
    /// Not 512 or 4096 bytes per sector.
    pub const SECTOR_SIZE: u8 = 4;
    /// The disk can't flush, so it can't confirm a write reached it.
    pub const NO_FLUSH: u8 = 5;
    /// LogansOS is installed on it: reinstalling erases it (R4b).
    pub const INSTALLED: u8 = 6;
    /// It holds something else: erasing it is R4b.
    pub const FOREIGN: u8 = 7;
    /// The driver declared the device dead.
    pub const DEVICE_DEAD: u8 = 8;
    /// Another install holds it.
    pub const IN_USE: u8 = 9;
    /// It changed since it was listed: list again.
    pub const CHANGED: u8 = 10;
    /// This medium can't install: its bundle, deployment or model is missing
    /// or doesn't match its pin.
    pub const MEDIUM: u8 = 11;
    /// "Can't be identified, so LogansOS won't overwrite it."
    pub const NO_IDENTITY: u8 = 12;
    /// Anything else (an I/O error, the layout).
    pub const OTHER: u8 = 13;
}

/// `SYS_INSTALL_RUN`: W6's last flush returned success.
pub const STATUS_INSTALLED: i64 = 0;
/// `SYS_INSTALL_RUN`: the install stopped during step `Wn`; the status is
/// `STATUS_STOPPED + n` (1–6).
pub const STATUS_STOPPED: i64 = 0x100;

/// One row, before encoding.
pub struct Row<'a> {
    pub slot: u32,
    pub generation: u64,
    pub capacity: u64,
    pub sector: u32,
    pub class: u8,
    pub action: u8,
    pub refusal: u8,
    pub identity: bool,
    pub model: &'a [u8],
    pub serial: &'a [u8],
}

/// Encode `row` as one record. Model and serial longer than their fields are
/// cut to fit.
pub fn encode(row: &Row<'_>) -> [u8; ROW_BYTES] {
    let mut out = [0u8; ROW_BYTES];
    let model = &row.model[..row.model.len().min(MODEL_BYTES)];
    let serial = &row.serial[..row.serial.len().min(SERIAL_BYTES)];
    out[0] = ROW_VERSION;
    out[1] = row.class;
    out[2] = row.action;
    out[3] = row.refusal;
    out[4] = if row.identity { FLAG_IDENTITY } else { 0 };
    out[5] = model.len() as u8;
    out[6] = serial.len() as u8;
    out[8..12].copy_from_slice(&row.slot.to_le_bytes());
    out[12..16].copy_from_slice(&row.sector.to_le_bytes());
    out[16..24].copy_from_slice(&row.generation.to_le_bytes());
    out[24..32].copy_from_slice(&row.capacity.to_le_bytes());
    out[32..32 + model.len()].copy_from_slice(model);
    out[80..80 + serial.len()].copy_from_slice(serial);
    out
}
