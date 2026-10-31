// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.3: the install state machine (`docs/THINK-2026-10-27-ot87-installer.md`
//! §4.6, §4.7). Pure: the kernel's installer drives it over the install claim,
//! and `tools/install-state-logic-tests` drives it over a write-back-cache
//! disk model that cuts power at every submission and every flush.
//!
//! - [`check_target`]: the refusals decided before any write (§4.6, §4.7
//!   rule 1).
//! - [`classify`]: what a disk is, from its bytes: blank, LogansOS
//!   incomplete, LogansOS installed, or foreign.
//! - [`install`]: W1–W6 for a blank disk, or W3–W6 to finish an incomplete
//!   one. Each step is write → flush → read back and verify → flush →
//!   step-boundary check (rule 2). Every GPT copy goes down array first: the
//!   header that makes an array recognisable is submitted only after that
//!   array's flush returned success (rule 3, the relay #88 header-only trace).
//!   A failed flush stops everything: nothing more is submitted (rule 4).
//!
//! W3–W5 write the payload (the ESP template and slot A, the NeoFS seed and
//! its grow, the model and the deployment copy). Their bytes come from the
//! caller through [`Payload`]; this module owns only their order and
//! barriers. Mounted beside `gpt_logic` (kernel `drivers::block`, the host
//! KAT crate's root), so keep it free of `crate::` paths and feature cfgs.

extern crate alloc;

use alloc::vec;

use super::gpt_logic::{self, Disk, Gpt, Layout};

/// The smallest target (§4.6): GPT, alignment, the 512 MiB ESP and a NeoFS of
/// at least 1 GiB, rounded up.
pub const MIN_TARGET_BYTES: u64 = 2 << 30;
/// The first MiB must be zero for a disk to count as blank (§4.5).
pub const BLANK_PREFIX_BYTES: u64 = 1 << 20;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Step {
    W1,
    W2,
    W3,
    W4,
    W5,
    W6,
}

impl Step {
    pub const ALL: [Step; 6] = [Step::W1, Step::W2, Step::W3, Step::W4, Step::W5, Step::W6];

    pub fn name(self) -> &'static str {
        match self {
            Step::W1 => "W1",
            Step::W2 => "W2",
            Step::W3 => "W3",
            Step::W4 => "W4",
            Step::W5 => "W5",
            Step::W6 => "W6",
        }
    }
}

/// Why a target was refused before any write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// Not 512 or 4096 bytes per logical sector (§4.4).
    SectorSize,
    /// Under [`MIN_TARGET_BYTES`] (§4.6).
    TooSmall,
    /// The device can't flush, so it can't confirm a write reached it
    /// (§4.7 rule 1).
    NoFlush,
    /// The layout doesn't fit (it always does at the minimum; kept as a
    /// refusal, not a panic).
    NoLayout,
    /// The disk isn't the class this kind of install needs: a fresh install
    /// needs a blank disk, Finish installing an incomplete LogansOS one.
    WrongClass(Class),
}

/// What a disk holds, from its bytes (§4.5, the outcomes of §4.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// No GPT signature in either header sector, and the first MiB is zero.
    Blank,
    /// Our two-entry table with entry 2 INCOMPLETE: never mounted, so it
    /// never held user data.
    Incomplete(Guids),
    /// Our two-entry table, complete: may hold user data.
    Installed(Guids),
    /// Anything else, including a GPT signature whose table doesn't validate.
    Foreign,
}

/// The GUIDs of a table, as GPT stores them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Guids {
    pub disk: [u8; 16],
    pub esp: [u8; 16],
    pub neofs: [u8; 16],
}

/// The two tables an install writes: entry 2 INCOMPLETE (W1, W2) and
/// complete (W6). Same geometry and GUIDs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tables {
    pub incomplete: Layout,
    pub complete: Layout,
}

impl Tables {
    pub fn new(sector_size: u64, last_lba: u64, guids: Guids) -> Option<Self> {
        let layout = |incomplete| {
            gpt_logic::gpt_layout(
                sector_size,
                last_lba,
                guids.disk,
                guids.esp,
                guids.neofs,
                incomplete,
            )
        };
        Some(Self {
            incomplete: layout(true)?,
            complete: layout(false)?,
        })
    }
}

/// The refusals decided before any write: sector size, size, flush.
pub fn check_target(capacity: u64, sector_size: u64, supports_flush: bool) -> Result<(), Refusal> {
    if sector_size != 512 && sector_size != 4096 {
        return Err(Refusal::SectorSize);
    }
    if capacity < MIN_TARGET_BYTES {
        return Err(Refusal::TooSmall);
    }
    if !supports_flush {
        return Err(Refusal::NoFlush);
    }
    Ok(())
}

/// What `disk` is. A table is ours only when it is exactly the two entries
/// [`gpt_logic::gpt_layout`] writes for this geometry, at its LBAs.
pub fn classify(disk: &dyn Disk) -> Class {
    let ss = disk.sector_size();
    let cap = disk.capacity();
    if ss == 0 || cap < 2 * ss {
        return Class::Foreign;
    }
    let last_lba = cap / ss - 1;
    if let Some(gpt) = gpt_logic::read(disk) {
        return classify_table(&gpt, ss, last_lba);
    }
    let signed = |lba: u64| {
        let mut head = [0u8; 8];
        !disk.read(lba * ss, &mut head) || &head == gpt_logic::SIGNATURE
    };
    if signed(1) || signed(last_lba) {
        // A GPT we cannot believe: opaque, so foreign.
        return Class::Foreign;
    }
    let mut chunk = vec![0u8; 64 * 1024];
    let mut at = 0u64;
    while at < BLANK_PREFIX_BYTES.min(cap) {
        let take = (BLANK_PREFIX_BYTES.min(cap) - at).min(chunk.len() as u64) as usize;
        if !disk.read(at, &mut chunk[..take]) || chunk[..take].iter().any(|&b| b != 0) {
            return Class::Foreign;
        }
        at += take as u64;
    }
    Class::Blank
}

/// Whether a disk of `class` may take an install that starts at `start`
/// (R4's `fresh` kinds; erase and reinstall are R4b's). Finish installing
/// keeps the disk's own GUIDs.
pub fn admit(class: Class, start: Start) -> Result<Option<Guids>, Refusal> {
    match (class, start) {
        (Class::Blank, Start::Fresh) => Ok(None),
        (Class::Incomplete(guids), Start::Finish) => Ok(Some(guids)),
        (other, _) => Err(Refusal::WrongClass(other)),
    }
}

fn classify_table(gpt: &Gpt, ss: u64, last_lba: u64) -> Class {
    let [esp, neofs] = gpt.entries.as_slice() else {
        return Class::Foreign;
    };
    let guids = Guids {
        disk: gpt.disk_guid,
        esp: esp.unique_guid,
        neofs: neofs.unique_guid,
    };
    let Some(tables) = Tables::new(ss, last_lba, guids) else {
        return Class::Foreign;
    };
    let layout = &tables.complete;
    let ours = esp.index == 1
        && neofs.index == 2
        && esp.type_guid == gpt_logic::ESP_TYPE_GUID
        && neofs.type_guid == gpt_logic::LOGANSOS_NEOFS_TYPE_GUID
        && (esp.first_lba, esp.last_lba) == layout.esp
        && (neofs.first_lba, neofs.last_lba) == layout.neofs
        && esp.attributes == 0
        && neofs.attributes & !gpt_logic::ATTR_INCOMPLETE == 0;
    match (ours, neofs.attributes & gpt_logic::ATTR_INCOMPLETE != 0) {
        (false, _) => Class::Foreign,
        (true, true) => Class::Incomplete(guids),
        (true, false) => Class::Installed(guids),
    }
}

/// The install target, at logical-sector granularity. In the kernel it is the
/// install claim (`install_claim::ClaimedWriter`), whose own checks run on
/// every call; in the KAT, the write-back-cache model.
pub trait Target {
    fn sector_size(&self) -> u64;
    /// Submit `bytes` (whole sectors) at `lba`. False: the write failed or
    /// the claim stopped; nothing more is submitted.
    fn write(&mut self, lba: u64, bytes: &[u8]) -> bool;
    /// Make everything written so far durable. False: the outcome is
    /// uncertain (§4.7 rule 4).
    fn flush(&mut self) -> bool;
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool;
    /// The step-boundary identity and fingerprint check (§2a item 5).
    fn boundary(&mut self) -> bool;
}

/// W3, W4 and W5's bytes. `write` submits the step's writes; `verify` reads
/// them back after the step's flush. Both may also flush inside the step
/// (W4's grow, W5's NeoFS sync); the step's own flush and boundary check
/// still follow.
pub trait Payload<T: Target> {
    fn write(&mut self, step: Step, target: &mut T) -> Result<(), Failure>;
    fn verify(&mut self, step: Step, target: &mut T) -> Result<(), Failure>;
}

/// How an install stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Failure {
    /// A write wasn't accepted (an I/O error, or the claim stopped).
    Write { step: Step, lba: u64 },
    /// A flush failed: what it covered may or may not have persisted.
    Flush { step: Step },
    /// A read-back didn't match, or couldn't be read.
    Verify { step: Step, lba: u64 },
    /// The step-boundary identity or fingerprint check failed.
    Boundary { step: Step },
}

impl Failure {
    pub fn step(self) -> Step {
        match self {
            Failure::Write { step, .. }
            | Failure::Flush { step }
            | Failure::Verify { step, .. }
            | Failure::Boundary { step } => step,
        }
    }
}

/// Where an install starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Start {
    /// A blank disk: W1–W6.
    Fresh,
    /// An incomplete LogansOS disk: W3–W6 with every barrier (§4.7).
    Finish,
}

/// How the GPT copies are ordered. Only [`Order::ArrayFirst`] is the
/// installer's; [`Order::HeaderWithArray`] exists so the KAT can show the
/// relay #88 header-only trace bites without the barrier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    ArrayFirst,
    HeaderWithArray,
}

/// How far an install got.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// W6's last flush returned success: report "installed".
    Installed,
    /// Stopped on request after this step completed (`install-stop-after`).
    StoppedAfter(Step),
}

/// Run the install from `start`, stopping after `stop_after` if given.
pub fn install<T: Target, P: Payload<T>>(
    target: &mut T,
    tables: &Tables,
    payload: &mut P,
    start: Start,
    stop_after: Option<Step>,
) -> Result<Outcome, Failure> {
    install_ordered(
        target,
        tables,
        payload,
        start,
        stop_after,
        Order::ArrayFirst,
    )
}

/// [`install`] with a chosen GPT [`Order`]. The kernel never calls this.
pub fn install_ordered<T: Target, P: Payload<T>>(
    target: &mut T,
    tables: &Tables,
    payload: &mut P,
    start: Start,
    stop_after: Option<Step>,
    order: Order,
) -> Result<Outcome, Failure> {
    let first = match start {
        Start::Fresh => Step::W1,
        Start::Finish => Step::W3,
    };
    for step in Step::ALL.into_iter().filter(|s| *s >= first) {
        match step {
            Step::W1 => {
                let t = &tables.incomplete;
                gpt_copy(
                    target,
                    step,
                    order,
                    &[(t.backup_entries_lba, &t.entries)],
                    (t.last_lba, &t.backup_header),
                )?;
            }
            Step::W2 => {
                let t = &tables.incomplete;
                gpt_copy(
                    target,
                    step,
                    order,
                    &[(0, &t.mbr), (2, &t.entries)],
                    (1, &t.primary_header),
                )?;
            }
            Step::W3 | Step::W4 | Step::W5 => {
                payload.write(step, target)?;
                flush(target, step)?;
                payload.verify(step, target)?;
                flush(target, step)?;
            }
            Step::W6 => {
                // Backup first, then primary: either copy may still say
                // INCOMPLETE until the end; the parser prefers a valid
                // primary, else the valid backup.
                let t = &tables.complete;
                gpt_copy(
                    target,
                    step,
                    order,
                    &[(t.backup_entries_lba, &t.entries)],
                    (t.last_lba, &t.backup_header),
                )?;
                gpt_copy(
                    target,
                    step,
                    order,
                    &[(0, &t.mbr), (2, &t.entries)],
                    (1, &t.primary_header),
                )?;
            }
        }
        if !target.boundary() {
            return Err(Failure::Boundary { step });
        }
        if step == Step::W6 {
            return Ok(Outcome::Installed);
        }
        if stop_after == Some(step) {
            return Ok(Outcome::StoppedAfter(step));
        }
    }
    unreachable!("W6 returns")
}

fn flush<T: Target>(target: &mut T, step: Step) -> Result<(), Failure> {
    if target.flush() {
        Ok(())
    } else {
        Err(Failure::Flush { step })
    }
}

fn write<T: Target>(target: &mut T, step: Step, lba: u64, bytes: &[u8]) -> Result<(), Failure> {
    if target.write(lba, bytes) {
        Ok(())
    } else {
        Err(Failure::Write { step, lba })
    }
}

/// Read `expected.len()` bytes at `lba` and compare.
pub fn verify<T: Target>(
    target: &mut T,
    step: Step,
    lba: u64,
    expected: &[u8],
) -> Result<(), Failure> {
    let mut back = vec![0u8; expected.len()];
    if !target.read(lba, &mut back) || back != expected {
        return Err(Failure::Verify { step, lba });
    }
    Ok(())
}

/// One GPT copy: `arrays` (the entry array, and for the primary the
/// protective MBR) written, flushed and verified, then `header` written,
/// flushed and verified. Under [`Order::HeaderWithArray`] the header goes in
/// the same flush window as the arrays (the KAT's counter-model).
fn gpt_copy<T: Target>(
    target: &mut T,
    step: Step,
    order: Order,
    arrays: &[(u64, &[u8])],
    header: (u64, &[u8]),
) -> Result<(), Failure> {
    for (lba, bytes) in arrays {
        write(target, step, *lba, bytes)?;
    }
    if order == Order::ArrayFirst {
        flush(target, step)?;
        for (lba, bytes) in arrays {
            verify(target, step, *lba, bytes)?;
        }
    }
    write(target, step, header.0, header.1)?;
    flush(target, step)?;
    for (lba, bytes) in arrays.iter().chain([&header]) {
        verify(target, step, *lba, bytes)?;
    }
    flush(target, step)
}
