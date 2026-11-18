// SPDX-License-Identifier: MPL-2.0
//! OT-100 R3: the persistence area's pure decisions
//! (`docs/THINK-2026-11-10-ot100-usb-persistence.md` §2.3, §4). Kernel-free:
//! the kernel's `persist` module and the host KATs in
//! `tools/persist-logic-tests` mount this file by `#[path]`, so it stays free
//! of `crate::` paths, feature cfgs and allocation.
//!
//! - [`parse_cmdline`]: "do we write this boot?" from the Limine command line.
//!   Off is the default; only the exact token `persist=on` turns it on.

/// Whether this boot may write the persistence area (THINK §2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Persist {
    On,
    Off,
}

/// The command-line key, with its `=`.
pub const PERSIST_KEY: &[u8] = b"persist=";

/// The longest command line read; a longer one (or one with no NUL in reach)
/// is not read at all, which is `Off`.
pub const CMDLINE_MAX: usize = 4096;

/// The persistent image's two Limine entries pass `persist=on` and
/// `persist=off`. `On` only for exactly one `persist=` token whose value is
/// exactly `on`. Off for: no command line, no `persist=` token, any other
/// value (`ON`, `1`, `on,` ...), or two `persist=` tokens, even if both say
/// `on`. Tokens are separated by ASCII whitespace; the bytes need not be
/// UTF-8.
pub fn parse_cmdline(cmdline: Option<&[u8]>) -> Persist {
    let Some(line) = cmdline else {
        return Persist::Off;
    };
    let mut found: Option<&[u8]> = None;
    for token in line
        .split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
    {
        if let Some(value) = token.strip_prefix(PERSIST_KEY) {
            if found.is_some() {
                return Persist::Off;
            }
            found = Some(value);
        }
    }
    if found == Some(b"on".as_slice()) {
        Persist::On
    } else {
        Persist::Off
    }
}

/// The NUL-terminated string at the start of `bytes`, or `None` when no NUL
/// lies within the first [`CMDLINE_MAX`] bytes (the kernel reads Limine's
/// pointer through this, so a missing terminator is `Off`, never an overrun).
pub fn c_string(bytes: &[u8]) -> Option<&[u8]> {
    let reach = &bytes[..bytes.len().min(CMDLINE_MAX)];
    reach.iter().position(|&b| b == 0).map(|n| &reach[..n])
}

/// A GPT disk GUID as GPT stores it.
pub type DiskGuid = [u8; 16];

/// Which registered device is the boot medium (THINK §4.1 item 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootDevice {
    /// Exactly one device carries the boot medium's disk GUID.
    Identified(usize),
    /// Not identified: no GUID (or an all-zero one: a medium with no GPT), no
    /// device carries it, or two or more do (two sticks written from one
    /// image share it). Nothing on USB is written.
    NotIdentified { guid_known: bool, matches: usize },
}

/// Identify the boot device without guessing: `boot` is Limine's disk GUID
/// for the medium the kernel came from; `devices` is each registered
/// device's index and the disk GUID of its validated GPT, if it has one.
pub fn identify(boot: Option<DiskGuid>, devices: &[(usize, Option<DiskGuid>)]) -> BootDevice {
    let Some(guid) = boot.filter(|g| g.iter().any(|&b| b != 0)) else {
        return BootDevice::NotIdentified {
            guid_known: false,
            matches: 0,
        };
    };
    let mut found = None;
    let mut matches = 0;
    for &(index, disk) in devices {
        if disk == Some(guid) {
            matches += 1;
            found = Some(index);
        }
    }
    match (matches, found) {
        (1, Some(index)) => BootDevice::Identified(index),
        _ => BootDevice::NotIdentified {
            guid_known: true,
            matches,
        },
    }
}

/// What the read-write rule looks at for one LogansOS volume (THINK §4.1
/// item 2). Gathered by the kernel; judged here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RwFacts {
    /// The volume is on a `usb-storage` device.
    pub usb: bool,
    pub boot: BootDevice,
    /// The registered index of the device carrying the volume.
    pub device: usize,
    pub persist: Persist,
    /// The guard's arming of that device is exactly one NeoFS claim, equal to
    /// the partition's span.
    pub armed_exactly_the_partition: bool,
    /// The write probe's answers (`None`: no probe came back).
    pub write_protected: Option<bool>,
    pub sync_ok: bool,
}

/// Why a volume mounts read-only: the first condition of the rule that
/// failed, in the order the rule lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RwRefusal {
    /// The boot medium could not be identified.
    BootNotIdentified,
    /// The volume is on a device (of any bus) that isn't the boot medium.
    NotTheBootDevice,
    /// The boot entry did not say `persist=on` (the "try" entry, or none).
    PersistOff,
    /// The guard arms something other than exactly this partition.
    ArmingMismatch,
    /// The stick reports write-protect.
    WriteProtected,
    /// No probe came back, or SYNCHRONIZE CACHE doesn't work.
    NoDurableFlush,
}

impl RwRefusal {
    pub const fn reason(self) -> &'static str {
        match self {
            RwRefusal::BootNotIdentified => "boot device not identified",
            RwRefusal::NotTheBootDevice => "not the boot device",
            RwRefusal::PersistOff => "persist=off",
            RwRefusal::ArmingMismatch => "guard arming is not exactly the partition",
            RwRefusal::WriteProtected => "write-protected",
            RwRefusal::NoDurableFlush => "no durable flush",
        }
    }
}

/// The read-write rule (THINK §4.1 items 2 and 3, with the PM's #110 F4
/// ruling). On any bus, a volume is read-write only on the identified boot
/// device. There, a volume that isn't on USB keeps today's selection (`Ok`:
/// an installed system booted from its own disk), and a USB volume is
/// read-write only when every stick condition holds too.
pub fn rw_decision(f: &RwFacts) -> Result<(), RwRefusal> {
    // #110 review F4 (the PM's ruling): on any bus, only the identified boot
    // device mounts read-write. An installed LogansOS boots from its internal
    // disk, so that disk is the boot device; a stick boot never writes an
    // internal disk.
    match f.boot {
        BootDevice::NotIdentified { .. } => return Err(RwRefusal::BootNotIdentified),
        BootDevice::Identified(index) if index != f.device => {
            return Err(RwRefusal::NotTheBootDevice);
        }
        BootDevice::Identified(_) => {}
    }
    // Off the stick, the boot device keeps today's selection.
    if !f.usb {
        return Ok(());
    }
    if f.persist != Persist::On {
        return Err(RwRefusal::PersistOff);
    }
    if !f.armed_exactly_the_partition {
        return Err(RwRefusal::ArmingMismatch);
    }
    if f.write_protected == Some(true) {
        return Err(RwRefusal::WriteProtected);
    }
    if !f.sync_ok {
        return Err(RwRefusal::NoDurableFlush);
    }
    Ok(())
}

/// A block-layer failure, as the NeoFS cache records it (a copy of the
/// kernel's `BlockError` variants this rule names). The codes are what a
/// device stores (0 is "none").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockFailure {
    /// The guard or the device refused the write (DATA PROTECT).
    WriteRefused,
    /// The device is gone (pulled).
    NoDevice,
    /// An I/O error after the driver's own retries and deadline.
    Io,
    /// The device's flush failed after its retries.
    Flush,
    /// Anything else.
    Other,
}

impl BlockFailure {
    pub const fn code(self) -> u8 {
        match self {
            BlockFailure::WriteRefused => 1,
            BlockFailure::NoDevice => 2,
            BlockFailure::Io => 3,
            BlockFailure::Flush => 4,
            BlockFailure::Other => 5,
        }
    }

    pub const fn from_code(code: u8) -> Option<BlockFailure> {
        match code {
            1 => Some(BlockFailure::WriteRefused),
            2 => Some(BlockFailure::NoDevice),
            3 => Some(BlockFailure::Io),
            4 => Some(BlockFailure::Flush),
            5 => Some(BlockFailure::Other),
            _ => None,
        }
    }
}

/// Why a mounted volume went read-only (THINK §4.2). The code is what the
/// mount stores (0 is "not lost").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Lost {
    WriteProtected = 1,
    Removed = 2,
    NotResponding = 3,
    FlushFailed = 4,
    /// The user switched to read-only ("Try").
    User = 5,
    Other = 6,
}

impl Lost {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn from_code(code: u8) -> Option<Lost> {
        match code {
            1 => Some(Lost::WriteProtected),
            2 => Some(Lost::Removed),
            3 => Some(Lost::NotResponding),
            4 => Some(Lost::FlushFailed),
            5 => Some(Lost::User),
            6 => Some(Lost::Other),
            _ => None,
        }
    }

    /// The reason in the `persist: volume lost (<reason>)` line.
    pub const fn reason(self) -> &'static str {
        match self {
            Lost::WriteProtected => "write-protected",
            Lost::Removed => "removed",
            Lost::NotResponding => "not responding",
            Lost::FlushFailed => "flush failed",
            Lost::User => "switched to read-only by the user",
            Lost::Other => "write failed",
        }
    }

    /// The desktop notice's cause (THINK §4.2).
    pub const fn notice(self) -> &'static str {
        match self {
            Lost::WriteProtected => "the stick is write-protected",
            Lost::Removed => "the stick was removed",
            Lost::NotResponding => "the stick stopped responding",
            Lost::FlushFailed => "the stick stopped responding",
            Lost::User => "you chose to try without saving",
            Lost::Other => "the stick stopped responding",
        }
    }
}

/// The drain worker's classifier (THINK §4.2): every block failure that
/// reaches NeoFS is fatal in v1 (transient ones, UNIT ATTENTION and NOT
/// READY, are retried inside the driver and never get here).
pub const fn classify(failure: BlockFailure) -> Lost {
    match failure {
        BlockFailure::WriteRefused => Lost::WriteProtected,
        BlockFailure::NoDevice => Lost::Removed,
        BlockFailure::Io => Lost::NotResponding,
        BlockFailure::Flush => Lost::FlushFailed,
        BlockFailure::Other => Lost::Other,
    }
}
