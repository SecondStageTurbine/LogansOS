// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: the installer's userland ABI, pure: the mirror of the
//! kernel's `install_ui_logic` (the record layout is documented there), and
//! the text both front ends show. The host oracle `tools/install-ui-abi-tests`
//! holds both sides to the same hand-laid bytes.

extern crate alloc;

use alloc::vec::Vec;

/// OT-87 R4.4 (kernel `installer`; ENOSYS elsewhere, EPERM without the
/// `Installer` cap): the installer's disk list.
pub const SYS_INSTALL_LIST: u64 = 0x1C0;
/// OT-87 R4.4: install to a listed disk.
pub const SYS_INSTALL_RUN: u64 = 0x1C1;

pub const ROW_BYTES: usize = 128;
pub const ROW_VERSION: u8 = 1;
pub const MODEL_BYTES: usize = 48;
pub const SERIAL_BYTES: usize = 32;
pub const FLAG_IDENTITY: u8 = 1 << 0;

pub mod class {
    pub const BLANK: u8 = 0;
    pub const INCOMPLETE: u8 = 1;
    pub const INSTALLED: u8 = 2;
    pub const FOREIGN: u8 = 3;
    pub const UNREADABLE: u8 = 4;
}

pub mod action {
    pub const NONE: u8 = 0;
    pub const INSTALL: u8 = 1;
    pub const FINISH: u8 = 2;
}

pub mod refusal {
    pub const NONE: u8 = 0;
    pub const BOOT_DISK: u8 = 1;
    pub const PROTECTED: u8 = 2;
    pub const TOO_SMALL: u8 = 3;
    pub const SECTOR_SIZE: u8 = 4;
    pub const NO_FLUSH: u8 = 5;
    pub const INSTALLED: u8 = 6;
    pub const FOREIGN: u8 = 7;
    pub const DEVICE_DEAD: u8 = 8;
    pub const IN_USE: u8 = 9;
    pub const CHANGED: u8 = 10;
    pub const MEDIUM: u8 = 11;
    pub const NO_IDENTITY: u8 = 12;
    pub const OTHER: u8 = 13;
}

pub const STATUS_INSTALLED: i64 = 0;
pub const STATUS_STOPPED: i64 = 0x100;

/// One disk, as the kernel listed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub slot: u32,
    pub generation: u64,
    pub capacity: u64,
    pub sector: u32,
    pub class: u8,
    pub action: u8,
    pub refusal: u8,
    pub identity: bool,
    pub model: [u8; MODEL_BYTES],
    pub model_len: usize,
    pub serial: [u8; SERIAL_BYTES],
    pub serial_len: usize,
}

impl Row {
    /// Decode one record. `None` for another version, a length past its
    /// field, a nonzero reserved byte, a class, action or refusal this
    /// version doesn't name, a flag bit it doesn't name (#148 F2), or a row
    /// that is neither offered nor refused (the layout: the action is
    /// `NONE` exactly when the refusal isn't).
    pub fn decode(rec: &[u8]) -> Option<Self> {
        if rec.len() != ROW_BYTES || rec[0] != ROW_VERSION || rec[7] != 0 {
            return None;
        }
        if rec[112..].iter().any(|&b| b != 0) {
            return None;
        }
        let (class, action, refusal, flags) = (rec[1], rec[2], rec[3], rec[4]);
        if class > class::UNREADABLE
            || action > action::FINISH
            || refusal > refusal::OTHER
            || flags & !FLAG_IDENTITY != 0
            || (action == action::NONE) == (refusal == refusal::NONE)
        {
            return None;
        }
        let model_len = rec[5] as usize;
        let serial_len = rec[6] as usize;
        if model_len > MODEL_BYTES || serial_len > SERIAL_BYTES {
            return None;
        }
        let u32_at = |at: usize| u32::from_le_bytes(rec[at..at + 4].try_into().unwrap());
        let u64_at = |at: usize| u64::from_le_bytes(rec[at..at + 8].try_into().unwrap());
        let mut model = [0u8; MODEL_BYTES];
        model.copy_from_slice(&rec[32..80]);
        let mut serial = [0u8; SERIAL_BYTES];
        serial.copy_from_slice(&rec[80..112]);
        Some(Self {
            slot: u32_at(8),
            generation: u64_at(16),
            capacity: u64_at(24),
            sector: u32_at(12),
            class,
            action,
            refusal,
            identity: flags & FLAG_IDENTITY != 0,
            model,
            model_len,
            serial,
            serial_len,
        })
    }

    pub fn model(&self) -> &[u8] {
        &self.model[..self.model_len]
    }

    pub fn serial(&self) -> &[u8] {
        &self.serial[..self.serial_len]
    }

    /// Whether the row may be chosen.
    pub fn offered(&self) -> bool {
        self.action != action::NONE && self.refusal == refusal::NONE
    }
}

// The text both installer front ends show (THINK §4.5), pure.

/// A size as the list shows it: GiB with one decimal from 1 GiB, else MiB.
pub fn size_text(bytes: u64) -> alloc::string::String {
    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;
    if bytes >= GIB {
        let tenths = bytes / (GIB / 10);
        alloc::format!("{}.{} GiB", tenths / 10, tenths % 10)
    } else {
        alloc::format!("{} MiB", bytes / MIB)
    }
}

pub fn class_text(class: u8) -> &'static str {
    match class {
        class::BLANK => "blank",
        class::INCOMPLETE => "LogansOS (installation incomplete)",
        class::INSTALLED => "LogansOS (installed)",
        class::FOREIGN => "another system or an unknown partition table",
        class::UNREADABLE => "can't be read",
        _ => "unknown",
    }
}

pub fn action_text(action: u8) -> &'static str {
    match action {
        action::INSTALL => "Install",
        action::FINISH => "Finish installing",
        _ => "",
    }
}

pub fn refusal_text(refusal: u8) -> &'static str {
    match refusal {
        refusal::BOOT_DISK => "this is the disk LogansOS is running from",
        refusal::PROTECTED => "protected",
        refusal::TOO_SMALL => "too small: needs 2 GiB",
        refusal::SECTOR_SIZE => "its sector size isn't 512 or 4096 bytes",
        refusal::NO_FLUSH => "it can't confirm writes reach it",
        refusal::INSTALLED => "LogansOS is installed here; reinstalling isn't in this version",
        refusal::FOREIGN => "erasing another system isn't in this version",
        refusal::DEVICE_DEAD => "the disk stopped responding",
        refusal::IN_USE => "another install is using it",
        refusal::CHANGED => "it changed since it was listed; list the disks again",
        refusal::MEDIUM => "this LogansOS medium can't install (its files don't match their pins)",
        refusal::NO_IDENTITY => "can't be identified, so LogansOS won't overwrite it",
        _ => "refused",
    }
}

/// The disk's name: model and serial, or "identity unavailable".
pub fn name_text(row: &Row) -> alloc::string::String {
    let model = core::str::from_utf8(row.model()).unwrap_or("?");
    let serial = core::str::from_utf8(row.serial()).unwrap_or("?");
    match (row.identity, model.is_empty()) {
        (true, true) => alloc::string::String::from(serial),
        (true, false) => alloc::format!("{model} {serial}"),
        (false, true) => alloc::string::String::from("identity unavailable"),
        (false, false) => alloc::format!("{model} (identity unavailable)"),
    }
}

/// What the row says beside the disk: its size, sector size and class.
pub fn detail_text(row: &Row) -> alloc::string::String {
    alloc::format!(
        "{}, {}-byte sectors, {}",
        size_text(row.capacity),
        row.sector,
        class_text(row.class)
    )
}

/// The action, or the refusal reason.
pub fn verdict_text(row: &Row) -> alloc::string::String {
    if row.offered() {
        alloc::string::String::from(action_text(row.action))
    } else {
        alloc::format!("not available: {}", refusal_text(row.refusal))
    }
}

/// The number each row gets in the console list: offered rows are numbered
/// from 1 in list order; refused rows get none and can't be chosen.
pub fn numbers(rows: &[Row]) -> Vec<Option<usize>> {
    let mut n = 0;
    rows.iter()
        .map(|r| {
            r.offered().then(|| {
                n += 1;
                n
            })
        })
        .collect()
}

/// What the user typed at the console list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// An empty line: go back.
    Back,
    /// The row at this index of the list.
    Row(usize),
    /// Not the number of an offered row.
    Invalid,
}

pub fn choose(rows: &[Row], input: &str) -> Choice {
    let input = input.trim();
    if input.is_empty() {
        return Choice::Back;
    }
    let Ok(want) = input.parse::<usize>() else {
        return Choice::Invalid;
    };
    numbers(rows)
        .iter()
        .position(|n| *n == Some(want))
        .map_or(Choice::Invalid, Choice::Row)
}

/// What `SYS_INSTALL_RUN`'s status means, for the user.
pub fn status_text(status: i64) -> alloc::string::String {
    match status {
        STATUS_INSTALLED => alloc::string::String::from(
            "LogansOS is installed. Remove the USB and restart to boot from the disk.",
        ),
        s if s > STATUS_STOPPED && s <= STATUS_STOPPED + 6 => alloc::format!(
            "The install stopped at step W{}. Nothing is lost: choose the disk again to finish installing.",
            s - STATUS_STOPPED
        ),
        s if s > 0 && s < 256 => {
            alloc::format!("The install was refused: {}.", refusal_text(s as u8))
        }
        s => alloc::format!("The install couldn't start (error {s})."),
    }
}
