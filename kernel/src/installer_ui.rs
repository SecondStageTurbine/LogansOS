// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: the installer's userland surface (`SYS_INSTALL_LIST`,
//! `SYS_INSTALL_RUN`; the ABI is `install_ui_logic`).
//!
//! The list and the run use the same check. Each listed row's verdict is
//! [`installer::preflight`] itself, run and dropped: the claim it takes is
//! released without a write, so the list never offers a disk the run would
//! refuse, and listing writes nothing. The class shown comes from a read of
//! the disk through the guard's reader, so a refused row (the boot disk, say)
//! still says what is on it.
//!
//! The run takes the enumeration the last list saw for that token, so the
//! claim checks the disk against what the user was shown: a disk that
//! changed since is refused (`ChangedSinceEnumeration`), as is a token the
//! registry has moved past (`StaleToken`).
//!
//! Process context, interrupts on: both calls do block I/O and may park.

extern crate alloc;

use alloc::vec::Vec;

use crate::drivers::block::claim_logic::{self, DriverIdentity, Enumerated};
use crate::drivers::block::install_claim;
use crate::drivers::block::install_logic::{self as logic, Class, Outcome, Start};
use crate::errno::EINVAL;
use crate::install_ui_logic::{self as ui, ROW_BYTES, action, class, refusal};
use crate::installer::{self, Refused};
use crate::serial_println;
use crate::sync::SwitchMutex;

/// What the last list enumerated. A run names one of these by token.
static LISTED: SwitchMutex<Vec<Enumerated>> = SwitchMutex::new(Vec::new());

fn class_code(class: Option<Class>) -> u8 {
    match class {
        Some(Class::Blank) => class::BLANK,
        Some(Class::Incomplete(_)) => class::INCOMPLETE,
        Some(Class::Installed(_)) => class::INSTALLED,
        Some(Class::Foreign) => class::FOREIGN,
        None => class::UNREADABLE,
    }
}

fn refusal_code(refused: &Refused) -> u8 {
    match refused {
        Refused::NoBundle
        | Refused::NoPin
        | Refused::PinMismatch
        | Refused::Bundle(_)
        | Refused::NoKernelFile
        | Refused::MissingModule(_)
        | Refused::NoModel
        | Refused::SlotTooSmall { .. } => refusal::MEDIUM,
        Refused::Claim(why) => match why {
            claim_logic::Refusal::BootDisk => refusal::BOOT_DISK,
            claim_logic::Refusal::Protected => refusal::PROTECTED,
            claim_logic::Refusal::IdentityUnavailable => refusal::NO_IDENTITY,
            claim_logic::Refusal::DeviceDead => refusal::DEVICE_DEAD,
            claim_logic::Refusal::AlreadyClaimed => refusal::IN_USE,
            claim_logic::Refusal::StaleToken | claim_logic::Refusal::ChangedSinceEnumeration => {
                refusal::CHANGED
            }
            _ => refusal::OTHER,
        },
        Refused::Target(why) => match why {
            logic::Refusal::SectorSize => refusal::SECTOR_SIZE,
            logic::Refusal::TooSmall => refusal::TOO_SMALL,
            logic::Refusal::NoFlush => refusal::NO_FLUSH,
            logic::Refusal::NoLayout => refusal::OTHER,
            logic::Refusal::WrongClass(Class::Installed(_)) => refusal::INSTALLED,
            logic::Refusal::WrongClass(Class::Foreign) => refusal::FOREIGN,
            // A start that doesn't match the class: the disk changed since
            // it was listed.
            logic::Refusal::WrongClass(Class::Blank | Class::Incomplete(_)) => refusal::CHANGED,
        },
    }
}

/// The start a disk of `class` is offered: Finish installing for an
/// incomplete LogansOS disk, Install for anything else (the preflight then
/// refuses whatever isn't blank).
fn start_for(class: Option<Class>) -> Start {
    match class {
        Some(Class::Incomplete(_)) => Start::Finish,
        _ => Start::Fresh,
    }
}

fn action_code(start: Start) -> u8 {
    match start {
        Start::Fresh => action::INSTALL,
        Start::Finish => action::FINISH,
    }
}

/// `SYS_INSTALL_LIST`: one record per enumerated disk, in registry order.
pub fn list() -> Vec<[u8; ROW_BYTES]> {
    let rows = install_claim::enumerate();
    serial_println!(
        "[ kernel ] install: list boot_disk_guid={:02x?}",
        installer::boot_disk_guid()
    );
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let slot = row.token.slot;
        let class = install_claim::class_of(slot);
        let start = start_for(class);
        let (action, refusal) = match installer::preflight(row, start) {
            // Dropped here: the claim is released, nothing was written.
            Ok(_) => (action_code(start), refusal::NONE),
            Err(why) => (action::NONE, refusal_code(&why)),
        };
        let identity = row.identity.as_ref();
        serial_println!(
            "[ kernel ] install: list slot={} generation={} disk_guid={:02x?} class={} action={} refusal={}",
            slot,
            row.token.generation,
            row.disk_guid,
            class_code(class),
            action,
            refusal
        );
        let (capacity, sector) = install_claim::geometry(slot).unwrap_or((0, 0));
        out.push(ui::encode(&ui::Row {
            slot: slot as u32,
            generation: row.token.generation,
            capacity,
            sector,
            class: class_code(class),
            action,
            refusal,
            identity: identity.is_some_and(DriverIdentity::available),
            model: identity.map(|i| i.model.as_bytes()).unwrap_or(&[]),
            serial: identity.map(|i| i.serial.as_bytes()).unwrap_or(&[]),
        }));
    }
    *LISTED.lock() = rows;
    // Every claim the preflights took was dropped with them, refused or not.
    serial_println!(
        "[ kernel ] install: list rows={} claimed_after={}",
        out.len(),
        install_claim::claimed_count()
    );
    out
}

/// `SYS_INSTALL_RUN`: install to the listed disk `(slot, generation)` from
/// `start` (`action::INSTALL` or `action::FINISH`). Returns the
/// `install_ui_logic` status.
pub fn run(slot: u64, generation: u64, start: u64) -> i64 {
    let start = match u8::try_from(start) {
        Ok(action::INSTALL) => Start::Fresh,
        Ok(action::FINISH) => Start::Finish,
        _ => return EINVAL,
    };
    let row = LISTED
        .lock()
        .iter()
        .find(|r| r.token.slot as u64 == slot && r.token.generation == generation)
        .cloned();
    let Some(row) = row else {
        serial_println!(
            "[ kernel ] install: run slot={} generation={} not listed",
            slot,
            generation
        );
        return i64::from(refusal::CHANGED);
    };
    let prepared = match installer::preflight(&row, start) {
        Ok(prepared) => prepared,
        Err(why) => {
            let code = refusal_code(&why);
            serial_println!(
                "[ kernel ] install: run slot={} refused={:?} code={}",
                slot,
                why,
                code
            );
            return i64::from(code);
        }
    };
    match installer::install(prepared, None) {
        Ok(Outcome::Installed) => ui::STATUS_INSTALLED,
        // No stop was asked for.
        Ok(Outcome::StoppedAfter(step)) => ui::STATUS_STOPPED + step_number(step),
        Err(failure) => ui::STATUS_STOPPED + step_number(failure.step()),
    }
}

fn step_number(step: logic::Step) -> i64 {
    logic::Step::ALL
        .iter()
        .position(|s| *s == step)
        .map_or(0, |i| i as i64 + 1)
}
