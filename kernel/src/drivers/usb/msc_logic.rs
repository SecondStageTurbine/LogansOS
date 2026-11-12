// SPDX-License-Identifier: MPL-2.0
//! L3u-1: USB mass storage, Bulk-Only Transport + the SCSI subset, as pure
//! logic (`docs/THINK-2026-10-31-l3u-usb-mass-storage.md` §3, §4).
//!
//! The kernel's binding (L3u-2) and the host KATs in
//! `tools/usb-msc-logic-tests` mount this file by `#[path]`, so keep it free
//! of `crate::` paths, feature cfgs and allocation.
//!
//! - [`encode_cbw`] / [`check_csw`]: the 31-byte Command Block Wrapper and the
//!   13-byte Command Status Wrapper (BOT 1.0 §5.1, §5.2; traps U2, U3, U6).
//! - The five CDBs L3u sends ([`cdb_inquiry`], [`cdb_test_unit_ready`],
//!   [`cdb_request_sense`], [`cdb_read_capacity_10`] / [`cdb_read_capacity_16`],
//!   [`cdb_read_10`]) and the replies it decodes (U8–U11).
//! - [`command`]: one BOT command over a [`Pipe`], with the host's error
//!   handling (U3–U5): a stalled data stage still reads the CSW, a stalled
//!   CSW is retried once, and anything else invalid ends in
//!   [`reset_recovery`] (U4).

/// `USBC`, little-endian: the CBW's first four bytes.
pub const CBW_SIGNATURE: u32 = 0x4342_5355;
/// `USBS`, little-endian: the CSW's first four bytes.
pub const CSW_SIGNATURE: u32 = 0x5342_5355;
pub const CBW_LEN: usize = 31;
pub const CSW_LEN: usize = 13;
/// `bmCBWFlags` bit 7: the data stage is device-to-host.
pub const CBW_FLAG_IN: u8 = 0x80;

/// The interface L3u binds: class mass storage, subclass SCSI transparent,
/// protocol Bulk-Only (U1: protocol `0x62` is UAS and is not bound).
pub const CLASS_MASS_STORAGE: u8 = 0x08;
pub const SUBCLASS_SCSI: u8 = 0x06;
pub const PROTOCOL_BULK_ONLY: u8 = 0x50;
pub const PROTOCOL_UAS: u8 = 0x62;

/// Class requests on the interface (BOT §3.1, §3.2).
pub const REQ_MASS_STORAGE_RESET: u8 = 0xff;
pub const REQ_GET_MAX_LUN: u8 = 0xfe;

/// INQUIRY's allocation length: 36 bytes, the standard data's size; some
/// devices misbehave on any other length (U8).
pub const INQUIRY_LEN: u8 = 36;
/// REQUEST SENSE's allocation length: fixed-format sense data.
pub const SENSE_LEN: u8 = 18;
/// READ(10)'s per-command cap in blocks (U14): 128 × 512 = 64 KiB.
pub const MAX_READ_BLOCKS: u16 = 128;

/// True for the interface triple L3u binds (U1).
pub fn is_bulk_only_scsi(class: u8, subclass: u8, protocol: u8) -> bool {
    class == CLASS_MASS_STORAGE && subclass == SUBCLASS_SCSI && protocol == PROTOCOL_BULK_ONLY
}

/// The CBW for one command: `data_len` bytes in the data stage, `data_in`
/// for device-to-host, `cdb` 1..=16 bytes (zero-padded to 16). `None` for a
/// CDB of another length or a LUN past 15.
pub fn encode_cbw(
    tag: u32,
    data_len: u32,
    data_in: bool,
    lun: u8,
    cdb: &[u8],
) -> Option<[u8; CBW_LEN]> {
    if cdb.is_empty() || cdb.len() > 16 || lun > 15 {
        return None;
    }
    let mut out = [0u8; CBW_LEN];
    out[0..4].copy_from_slice(&CBW_SIGNATURE.to_le_bytes());
    out[4..8].copy_from_slice(&tag.to_le_bytes());
    out[8..12].copy_from_slice(&data_len.to_le_bytes());
    out[12] = if data_in && data_len != 0 {
        CBW_FLAG_IN
    } else {
        0
    };
    out[13] = lun;
    out[14] = cdb.len() as u8;
    out[15..15 + cdb.len()].copy_from_slice(cdb);
    Some(out)
}

/// What a CSW says about its command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CswVerdict {
    /// Status 0. `residue`: requested bytes not transferred.
    Passed { residue: u32 },
    /// Status 1: the command failed; REQUEST SENSE says why.
    Failed { residue: u32 },
    /// Not a valid, meaningful CSW, or status 2: reset recovery follows.
    PhaseError(PhaseReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseReason {
    /// Not 13 bytes.
    Length,
    /// Not `USBS`.
    Signature,
    /// The tag is not the CBW's (U3).
    Tag,
    /// The residue is more than was asked for (U6).
    Residue,
    /// `bCSWStatus` 2, or a reserved value.
    Status,
}

/// Judge `csw` (as received) against the command's `tag` and `data_len`.
pub fn check_csw(csw: &[u8], tag: u32, data_len: u32) -> CswVerdict {
    if csw.len() != CSW_LEN {
        return CswVerdict::PhaseError(PhaseReason::Length);
    }
    let word = |at: usize| u32::from_le_bytes([csw[at], csw[at + 1], csw[at + 2], csw[at + 3]]);
    if word(0) != CSW_SIGNATURE {
        return CswVerdict::PhaseError(PhaseReason::Signature);
    }
    if word(4) != tag {
        return CswVerdict::PhaseError(PhaseReason::Tag);
    }
    let residue = word(8);
    if residue > data_len {
        return CswVerdict::PhaseError(PhaseReason::Residue);
    }
    match csw[12] {
        0 => CswVerdict::Passed { residue },
        1 => CswVerdict::Failed { residue },
        _ => CswVerdict::PhaseError(PhaseReason::Status),
    }
}

/// The only SCSI opcodes this driver sends (U17, Chad's #20 review): INQUIRY,
/// TEST UNIT READY, REQUEST SENSE, READ CAPACITY(10), SERVICE ACTION IN(16)
/// (READ CAPACITY(16)) and READ(10). MODE SENSE, SYNCHRONIZE CACHE, PREVENT
/// ALLOW MEDIUM REMOVAL and START STOP UNIT hang or stall some devices
/// (Linux's `US_FL_NO_*` quirks), and every write command is out of scope:
/// [`command`] refuses any other opcode before a byte reaches the device.
pub const ALLOWED_OPCODES: [u8; 6] = [0x12, 0x00, 0x03, 0x25, 0x9e, 0x28];

/// Whether `cdb` starts with an [`ALLOWED_OPCODES`] opcode.
pub fn cdb_allowed(cdb: &[u8]) -> bool {
    cdb.first().is_some_and(|op| ALLOWED_OPCODES.contains(op))
}

/// INQUIRY (`0x12`), 36 bytes of standard data.
pub fn cdb_inquiry() -> [u8; 6] {
    [0x12, 0, 0, 0, INQUIRY_LEN, 0]
}

/// TEST UNIT READY (`0x00`): no data stage.
pub fn cdb_test_unit_ready() -> [u8; 6] {
    [0; 6]
}

/// REQUEST SENSE (`0x03`), fixed-format, 18 bytes.
pub fn cdb_request_sense() -> [u8; 6] {
    [0x03, 0, 0, 0, SENSE_LEN, 0]
}

/// READ CAPACITY(10) (`0x25`): 8 bytes back.
pub fn cdb_read_capacity_10() -> [u8; 10] {
    [0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// READ CAPACITY(16): SERVICE ACTION IN(16) (`0x9E`), service action `0x10`,
/// allocation length 32 big-endian at bytes 10..14.
pub fn cdb_read_capacity_16() -> [u8; 16] {
    let mut c = [0u8; 16];
    c[0] = 0x9e;
    c[1] = 0x10;
    c[10..14].copy_from_slice(&32u32.to_be_bytes());
    c
}

/// READ(10) (`0x28`): `blocks` blocks from `lba`, both big-endian.
pub fn cdb_read_10(lba: u32, blocks: u16) -> [u8; 10] {
    let mut c = [0u8; 10];
    c[0] = 0x28;
    c[2..6].copy_from_slice(&lba.to_be_bytes());
    c[7..9].copy_from_slice(&blocks.to_be_bytes());
    c
}

/// A device's size: block count and block size in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity {
    pub blocks: u64,
    pub block_len: u32,
}

/// What READ CAPACITY(10) says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capacity10 {
    Known(Capacity),
    /// The last LBA does not fit 32 bits: ask READ CAPACITY(16) (U11).
    Needs16,
    /// A zero block length.
    Invalid,
}

/// READ CAPACITY(10)'s 8 bytes: the **last** LBA, then the block length,
/// both big-endian. Blocks = last + 1 (U10).
pub fn decode_capacity_10(reply: &[u8; 8]) -> Capacity10 {
    let last = u32::from_be_bytes([reply[0], reply[1], reply[2], reply[3]]);
    let block_len = u32::from_be_bytes([reply[4], reply[5], reply[6], reply[7]]);
    if last == u32::MAX {
        return Capacity10::Needs16;
    }
    if block_len == 0 {
        return Capacity10::Invalid;
    }
    Capacity10::Known(Capacity {
        blocks: u64::from(last) + 1,
        block_len,
    })
}

/// READ CAPACITY(16)'s first 12 bytes: the last LBA (8, big-endian), then
/// the block length (4). `None` for a zero length or a last LBA of
/// `u64::MAX`.
pub fn decode_capacity_16(reply: &[u8]) -> Option<Capacity> {
    if reply.len() < 12 {
        return None;
    }
    let mut last = [0u8; 8];
    last.copy_from_slice(&reply[0..8]);
    let last = u64::from_be_bytes(last);
    let block_len = u32::from_be_bytes([reply[8], reply[9], reply[10], reply[11]]);
    if block_len == 0 {
        return None;
    }
    Some(Capacity {
        blocks: last.checked_add(1)?,
        block_len,
    })
}

/// Sense data's key / additional code / qualifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sense {
    pub key: u8,
    pub asc: u8,
    pub ascq: u8,
}

pub const SENSE_KEY_NOT_READY: u8 = 0x2;
pub const SENSE_KEY_UNIT_ATTENTION: u8 = 0x6;
/// ASC `3A`: medium not present (U9).
pub const ASC_MEDIUM_NOT_PRESENT: u8 = 0x3a;

/// Decode sense data: fixed format (`0x70`/`0x71`: key in byte 2, ASC/ASCQ in
/// 12/13) or descriptor format (`0x72`/`0x73`: key in byte 1, ASC/ASCQ in
/// 2/3). `None` for another response code or too few bytes.
pub fn decode_sense(data: &[u8]) -> Option<Sense> {
    match data.first()? & 0x7f {
        0x70 | 0x71 if data.len() >= 14 => Some(Sense {
            key: data[2] & 0x0f,
            asc: data[12],
            ascq: data[13],
        }),
        0x72 | 0x73 if data.len() >= 4 => Some(Sense {
            key: data[1] & 0x0f,
            asc: data[2],
            ascq: data[3],
        }),
        _ => None,
    }
}

/// What to do after TEST UNIT READY failed with `sense` (U9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadyAction {
    /// UNIT ATTENTION (power-on, medium changed): ask again at once.
    Retry,
    /// NOT READY, becoming ready (ASC 04/01: spinning up): ask again after a
    /// pause, within the bind's spin-up budget (Chad's #20 review; Linux's
    /// `sd_spinup_disk` waits the same way).
    Wait,
    /// NOT READY, an initializing command required (ASC 04/02): the device
    /// wants START STOP UNIT, which this read-only driver doesn't send (it is
    /// not in [`ALLOWED_OPCODES`]). Bind no block device, and say why.
    NeedsStart,
    /// NOT READY, medium not present (ASC 3A): bind no block device. Final,
    /// not retried (a card reader with no card; Linux stops here too).
    NoMedium,
    /// Anything else: give up on the device.
    Fail,
}

pub fn ready_action(sense: Sense) -> ReadyAction {
    match (sense.key, sense.asc) {
        (SENSE_KEY_UNIT_ATTENTION, _) => ReadyAction::Retry,
        (SENSE_KEY_NOT_READY, ASC_MEDIUM_NOT_PRESENT) => ReadyAction::NoMedium,
        // NOT READY, becoming ready (ASC 04, ASCQ 01).
        (SENSE_KEY_NOT_READY, 0x04) if sense.ascq == 0x01 => ReadyAction::Wait,
        // NOT READY, initializing command required (ASC 04, ASCQ 02).
        (SENSE_KEY_NOT_READY, 0x04) if sense.ascq == 0x02 => ReadyAction::NeedsStart,
        _ => ReadyAction::Fail,
    }
}

/// The next READ(10) of a split read: `(lba, blocks)` for the part of
/// `count` blocks from `lba` starting `done` blocks in, at most
/// [`MAX_READ_BLOCKS`] (U14). `None` when done, or when an LBA would not
/// fit READ(10)'s 32 bits.
pub fn next_read(lba: u64, count: u64, done: u64) -> Option<(u32, u16)> {
    if done >= count {
        return None;
    }
    let at = u32::try_from(lba.checked_add(done)?).ok()?;
    let blocks = (count - done).min(u64::from(MAX_READ_BLOCKS)) as u16;
    // The last block of this part must be addressable too.
    at.checked_add(u32::from(blocks) - 1)?;
    Some((at, blocks))
}

/// One bulk transfer's outcome, as the transport saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Xfer {
    /// Completed, with this many bytes (a short packet is still `Done`).
    Done(usize),
    /// The endpoint stalled (halted).
    Stall,
    /// Timeout, disconnect or a transport error.
    Error,
}

/// The device's bulk pipes and its interface's class requests.
pub trait Pipe {
    fn bulk_out(&mut self, data: &[u8]) -> Xfer;
    fn bulk_in(&mut self, buf: &mut [u8]) -> Xfer;
    /// ClearFeature(ENDPOINT_HALT) on the bulk-in endpoint (and the host's
    /// endpoint reset). False on failure.
    fn clear_halt_in(&mut self) -> bool;
    fn clear_halt_out(&mut self) -> bool;
    /// The Bulk-Only Mass Storage Reset class request. False on failure.
    fn mass_storage_reset(&mut self) -> bool;
}

/// Reset recovery, in the order BOT §5.3.4 requires (U4): the class reset,
/// then clear halt on bulk-in, then on bulk-out. Every step runs even if an
/// earlier one failed; true only if all three succeeded.
pub fn reset_recovery(pipe: &mut impl Pipe) -> bool {
    let reset = pipe.mass_storage_reset();
    let halt_in = pipe.clear_halt_in();
    let halt_out = pipe.clear_halt_out();
    reset && halt_in && halt_out
}

/// One command's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Status passed; `len` bytes arrived in the data stage.
    Passed { len: usize },
    /// Status failed (REQUEST SENSE next); `len` bytes arrived.
    Failed { len: usize },
    /// The transport broke; reset recovery ran (`recovered` says whether it
    /// succeeded). The command did not complete.
    Reset {
        reason: ResetReason,
        recovered: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetReason {
    /// The CBW was not accepted.
    Cbw,
    /// The data stage failed other than by a stall.
    Data,
    /// No CSW after one stall retry.
    Csw,
    /// A CSW arrived but meant a phase error.
    Phase(PhaseReason),
    /// The CSW's residue disagrees with the bytes that arrived.
    ResidueMismatch,
    /// The CDB's opcode is not in [`ALLOWED_OPCODES`]: nothing was sent.
    NotAllowed,
}

/// Run one command with an optional data-in stage into `data` (U3–U6; BOT
/// §6.7's host-side cases for a device-to-host or no-data command).
pub fn command(pipe: &mut impl Pipe, tag: u32, lun: u8, cdb: &[u8], data: &mut [u8]) -> Outcome {
    let data_len = data.len() as u32;
    let reset = |pipe: &mut _, reason| Outcome::Reset {
        reason,
        recovered: reset_recovery(pipe),
    };
    if !cdb_allowed(cdb) {
        return Outcome::Reset {
            reason: ResetReason::NotAllowed,
            recovered: true,
        };
    }
    let Some(cbw) = encode_cbw(tag, data_len, true, lun, cdb) else {
        return Outcome::Reset {
            reason: ResetReason::Cbw,
            recovered: true,
        };
    };
    match pipe.bulk_out(&cbw) {
        Xfer::Done(n) if n == CBW_LEN => {}
        _ => return reset(pipe, ResetReason::Cbw),
    }
    let mut got = 0usize;
    if !data.is_empty() {
        match pipe.bulk_in(data) {
            Xfer::Done(n) => got = n.min(data.len()),
            // A stalled data stage: clear it, and the CSW still follows (U5).
            Xfer::Stall => {
                if !pipe.clear_halt_in() {
                    return reset(pipe, ResetReason::Data);
                }
            }
            Xfer::Error => return reset(pipe, ResetReason::Data),
        }
    }
    let mut csw = [0u8; CSW_LEN];
    let mut csw_len = None;
    for _ in 0..2 {
        match pipe.bulk_in(&mut csw) {
            Xfer::Done(n) => {
                csw_len = Some(n);
                break;
            }
            // A stalled CSW: clear and read it once more (U5).
            Xfer::Stall => {
                if !pipe.clear_halt_in() {
                    break;
                }
            }
            Xfer::Error => break,
        }
    }
    let Some(n) = csw_len else {
        return reset(pipe, ResetReason::Csw);
    };
    match check_csw(&csw[..n.min(CSW_LEN)], tag, data_len) {
        CswVerdict::PhaseError(why) => reset(pipe, ResetReason::Phase(why)),
        CswVerdict::Passed { residue } | CswVerdict::Failed { residue }
            if data_len - residue < got as u32 =>
        {
            // More arrived than the device says it sent.
            reset(pipe, ResetReason::ResidueMismatch)
        }
        CswVerdict::Passed { .. } => Outcome::Passed { len: got },
        CswVerdict::Failed { .. } => Outcome::Failed { len: got },
    }
}

// ---------------------------------------------------------------------------
// OT-100 R2: the write path (`docs/THINK-2026-11-10-ot100-usb-persistence.md`
// §3). Only `usb-storage-write` kernels call anything below; [`command`] and
// [`ALLOWED_OPCODES`] above are the read-only driver's, unchanged.
// ---------------------------------------------------------------------------

/// A command's data stage, by direction (BOT §5.1: `bmCBWFlags` bit 7).
pub enum Data<'a> {
    /// No data stage.
    None,
    /// Device-to-host into the slice.
    In(&'a mut [u8]),
    /// Host-to-device from the slice.
    Out(&'a [u8]),
}

/// A data stage's direction, as the write allowlist names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    None,
    In,
    Out,
}

impl Data<'_> {
    pub fn dir(&self) -> Dir {
        match self {
            Data::None => Dir::None,
            Data::In(_) => Dir::In,
            Data::Out(_) => Dir::Out,
        }
    }

    fn len(&self) -> usize {
        match self {
            Data::None => 0,
            Data::In(b) => b.len(),
            Data::Out(b) => b.len(),
        }
    }
}

/// The write path's allowlist (THINK §3.1): every opcode with the one
/// direction it may run in. Today's six keep theirs; WRITE(10) runs only
/// host-to-device, SYNCHRONIZE CACHE(10) with no data, MODE SENSE(6) only
/// device-to-host. A known opcode in the wrong direction is refused like an
/// unknown one, before a byte reaches the device.
pub const WRITE_PATH_COMMANDS: [(u8, Dir); 9] = [
    (0x12, Dir::In),   // INQUIRY
    (0x00, Dir::None), // TEST UNIT READY
    (0x03, Dir::In),   // REQUEST SENSE
    (0x25, Dir::In),   // READ CAPACITY(10)
    (0x9e, Dir::In),   // SERVICE ACTION IN(16): READ CAPACITY(16)
    (0x28, Dir::In),   // READ(10)
    (0x2a, Dir::Out),  // WRITE(10)
    (0x35, Dir::None), // SYNCHRONIZE CACHE(10)
    (0x1a, Dir::In),   // MODE SENSE(6)
];

pub const OP_WRITE_10: u8 = 0x2a;
pub const OP_SYNCHRONIZE_CACHE_10: u8 = 0x35;
pub const OP_MODE_SENSE_6: u8 = 0x1a;

/// Whether `cdb`'s opcode may run with a data stage in `dir`.
pub fn cdb_allowed_dir(cdb: &[u8], dir: Dir) -> bool {
    cdb.first().is_some_and(|op| {
        WRITE_PATH_COMMANDS
            .iter()
            .any(|&(o, d)| o == *op && d == dir)
    })
}

/// WRITE(10) (`0x2a`): `blocks` blocks at `lba`, both big-endian. FUA (byte
/// 1 bit 3) and every other flag stay 0 (Linux `BROKEN_FUA`).
pub fn cdb_write_10(lba: u32, blocks: u16) -> [u8; 10] {
    let mut c = [0u8; 10];
    c[0] = OP_WRITE_10;
    c[2..6].copy_from_slice(&lba.to_be_bytes());
    c[7..9].copy_from_slice(&blocks.to_be_bytes());
    c
}

/// SYNCHRONIZE CACHE(10) (`0x35`) of the whole medium: LBA 0, 0 blocks (SBC:
/// "all logical blocks from the LBA to the end").
pub fn cdb_synchronize_cache_10() -> [u8; 10] {
    [OP_SYNCHRONIZE_CACHE_10, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// MODE SENSE(6)'s allocation length: the 4-byte header only (THINK §3.1;
/// Linux `sd` asks USB devices for little, as large page-0x3f reads hang
/// some of them).
pub const MODE_SENSE_HEADER_LEN: u8 = 4;

/// MODE SENSE(6) (`0x1a`): DBD set (no block descriptors), page 0x3f (all),
/// the header only.
pub fn cdb_mode_sense_6_header() -> [u8; 6] {
    [OP_MODE_SENSE_6, 0x08, 0x3f, 0, MODE_SENSE_HEADER_LEN, 0]
}

/// The write-protect bit of a MODE SENSE(6) reply: byte 2 (device-specific
/// parameter) bit 7. `None` if fewer than 3 bytes arrived.
pub fn mode_sense_write_protected(reply: &[u8]) -> Option<bool> {
    reply.get(2).map(|b| b & 0x80 != 0)
}

/// WRITE(10)'s per-command cap in blocks: the bounce buffer, as for reads.
pub const MAX_WRITE_BLOCKS: u16 = MAX_READ_BLOCKS;

/// An LBA a 32-bit command cannot address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfRange;

/// The next WRITE(10) of a split write: `(lba, blocks)` for the part of
/// `count` blocks from `lba` starting `done` blocks in, at most `max`
/// blocks (clamped to 1..=[`MAX_WRITE_BLOCKS`]). `Ok(None)` when done;
/// `Err(OutOfRange)` when the part's first or last block does not fit 32
/// bits, so nothing is sent for it (THINK §3.2).
pub fn next_write(
    lba: u64,
    count: u64,
    done: u64,
    max: u16,
) -> Result<Option<(u32, u16)>, OutOfRange> {
    if done >= count {
        return Ok(None);
    }
    let max = max.clamp(1, MAX_WRITE_BLOCKS);
    let at = lba.checked_add(done).ok_or(OutOfRange)?;
    let at = u32::try_from(at).map_err(|_| OutOfRange)?;
    let blocks = (count - done).min(u64::from(max)) as u16;
    at.checked_add(u32::from(blocks) - 1).ok_or(OutOfRange)?;
    Ok(Some((at, blocks)))
}

/// Whether a split read of `count` blocks from `lba` reaches a block past
/// READ(10)'s 32 bits: such a read is refused before any command (THINK
/// §3.2: [`next_read`] stops there, which would leave a short read).
pub fn read_past_32_bits(lba: u64, count: u64) -> bool {
    count != 0
        && lba
            .checked_add(count - 1)
            .is_none_or(|last| last > u64::from(u32::MAX))
}

pub const SENSE_KEY_DATA_PROTECT: u8 = 0x7;
pub const SENSE_KEY_ILLEGAL_REQUEST: u8 = 0x5;
pub const SENSE_KEY_MEDIUM_ERROR: u8 = 0x3;
pub const SENSE_KEY_HARDWARE_ERROR: u8 = 0x4;

/// What to do after a WRITE(10) or SYNCHRONIZE CACHE failed with `sense`
/// (THINK §3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteAction {
    /// UNIT ATTENTION: retry once.
    Retry,
    /// NOT READY: retry after a pause, a few times.
    Wait,
    /// DATA PROTECT: the medium is write-protected; latch it.
    WriteProtected,
    /// ILLEGAL REQUEST: the device does not do this command (for SYNCHRONIZE
    /// CACHE: latch `sync_unsupported`, Haiku's `sync_support = 0`).
    Unsupported,
    /// MEDIUM ERROR, HARDWARE ERROR, anything else: an I/O error.
    Fail,
}

pub fn write_action(sense: Sense) -> WriteAction {
    match sense.key {
        SENSE_KEY_UNIT_ATTENTION => WriteAction::Retry,
        SENSE_KEY_NOT_READY => WriteAction::Wait,
        SENSE_KEY_DATA_PROTECT => WriteAction::WriteProtected,
        SENSE_KEY_ILLEGAL_REQUEST => WriteAction::Unsupported,
        _ => WriteAction::Fail,
    }
}

/// Run one command on the write path (THINK §3.2; BOT §6.7's host cases):
/// the data stage's direction is `data`'s, the CBW's flag follows it, and
/// the pair must be in [`WRITE_PATH_COMMANDS`].
///
/// - `In`: as [`command`].
/// - `Out`: a bulk-OUT of the slice. A stall clears the bulk-out halt and
///   still reads the CSW (BOT §6.7.3); a transport error is reset recovery.
///   The CSW's residue must not claim more was taken than was sent; a
///   passed status with a non-zero residue is a short write, reported as
///   `Failed` with the bytes the device took.
pub fn command_dir(pipe: &mut impl Pipe, tag: u32, lun: u8, cdb: &[u8], data: Data<'_>) -> Outcome {
    let dir = data.dir();
    let data_len = data.len() as u32;
    let reset = |pipe: &mut _, reason| Outcome::Reset {
        reason,
        recovered: reset_recovery(pipe),
    };
    if !cdb_allowed_dir(cdb, dir) || (dir != Dir::None && data_len == 0) {
        return Outcome::Reset {
            reason: ResetReason::NotAllowed,
            recovered: true,
        };
    }
    let Some(cbw) = encode_cbw(tag, data_len, dir == Dir::In, lun, cdb) else {
        return Outcome::Reset {
            reason: ResetReason::Cbw,
            recovered: true,
        };
    };
    match pipe.bulk_out(&cbw) {
        Xfer::Done(n) if n == CBW_LEN => {}
        _ => return reset(pipe, ResetReason::Cbw),
    }
    // Bytes moved in the data stage, if the transport knows (a stall does
    // not say how many went).
    let mut moved: Option<usize> = Some(0);
    match data {
        Data::None => {}
        Data::In(buf) => match pipe.bulk_in(buf) {
            Xfer::Done(n) => moved = Some(n.min(buf.len())),
            Xfer::Stall => {
                if !pipe.clear_halt_in() {
                    return reset(pipe, ResetReason::Data);
                }
            }
            Xfer::Error => return reset(pipe, ResetReason::Data),
        },
        Data::Out(buf) => match pipe.bulk_out(buf) {
            Xfer::Done(n) => moved = Some(n.min(buf.len())),
            Xfer::Stall => {
                if !pipe.clear_halt_out() {
                    return reset(pipe, ResetReason::Data);
                }
                moved = None;
            }
            Xfer::Error => return reset(pipe, ResetReason::Data),
        },
    }
    let mut csw = [0u8; CSW_LEN];
    let mut csw_len = None;
    for _ in 0..2 {
        match pipe.bulk_in(&mut csw) {
            Xfer::Done(n) => {
                csw_len = Some(n);
                break;
            }
            Xfer::Stall => {
                if !pipe.clear_halt_in() {
                    break;
                }
            }
            Xfer::Error => break,
        }
    }
    // A 0-length CSW (a stray zero-length packet after the data): read once
    // more (Linux `transport.c`).
    if csw_len == Some(0) {
        csw_len = match pipe.bulk_in(&mut csw) {
            Xfer::Done(n) => Some(n),
            _ => None,
        };
    }
    let Some(n) = csw_len else {
        return reset(pipe, ResetReason::Csw);
    };
    let (passed, residue) = match check_csw(&csw[..n.min(CSW_LEN)], tag, data_len) {
        CswVerdict::PhaseError(why) => return reset(pipe, ResetReason::Phase(why)),
        CswVerdict::Passed { residue } => (true, residue),
        CswVerdict::Failed { residue } => (false, residue),
    };
    let taken = (data_len - residue) as usize;
    match (dir, moved) {
        // More arrived than the device says it sent.
        (Dir::In, Some(got)) if taken < got => reset(pipe, ResetReason::ResidueMismatch),
        // The device says it took more than was sent.
        (Dir::Out, Some(sent)) if taken > sent => reset(pipe, ResetReason::ResidueMismatch),
        (Dir::In, got) => {
            let len = got.unwrap_or(0);
            if passed {
                Outcome::Passed { len }
            } else {
                Outcome::Failed { len }
            }
        }
        (Dir::Out, _) if passed && residue == 0 => Outcome::Passed { len: taken },
        (Dir::Out, _) => Outcome::Failed { len: taken },
        (Dir::None, _) if passed => Outcome::Passed { len: 0 },
        (Dir::None, _) => Outcome::Failed { len: 0 },
    }
}

// ---------------------------------------------------------------------------
// OT-100 R2 (relay 393 F1–F3): the write path's time bounds, retry policy,
// latches and diagnostics, as pure logic over a [`WritePort`], so a host test
// drives them with a controlled clock. The kernel's port is `msc.rs`'s.
// ---------------------------------------------------------------------------

/// The timer tick's period in ms (the kernel's 100 Hz `TICKS`).
pub const MS_PER_TICK: u64 = 10;
/// One WRITE(10) attempt's bound: its CBW, data and CSW, its REQUEST SENSE
/// and any reset recovery, all together.
pub const WRITE_ATTEMPT_MS: u64 = 10_000;
/// One SYNCHRONIZE CACHE(10) attempt's bound, the same way.
pub const SYNC_ATTEMPT_MS: u64 = 30_000;
/// The write probe's MODE SENSE(6) and TEST UNIT READY, each.
pub const PROBE_ATTEMPT_MS: u64 = 10_000;
/// NOT READY on a WRITE: retried this many times, after a pause each.
pub const NOT_READY_TRIES: u32 = 4;
pub const NOT_READY_PAUSE_MS: u64 = 250;
/// A WRITE chunk's attempts, at most: the first, one UNIT ATTENTION retry,
/// the NOT READY retries, one retry after a recovered reset. A passed
/// deadline ends the chunk at once.
pub const WRITE_MAX_ATTEMPTS: u32 = 1 + 1 + NOT_READY_TRIES + 1;
/// `flush`'s SYNCHRONIZE CACHE attempts, at most.
pub const SYNC_MAX_ATTEMPTS: u32 = 3;

/// A WRITE chunk's worst case in ms: every attempt to its deadline, plus the
/// NOT READY pauses.
pub const fn write_chunk_bound_ms() -> u64 {
    WRITE_MAX_ATTEMPTS as u64 * WRITE_ATTEMPT_MS + NOT_READY_TRIES as u64 * NOT_READY_PAUSE_MS
}

/// An absolute deadline on the tick clock. One is made per attempt, and every
/// wait in that attempt (its transfers, its sense, its recovery) stops at the
/// same tick: a slow stage leaves less time for the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deadline {
    /// The first tick at which the attempt has run out of time.
    pub end: u64,
}

impl Deadline {
    /// `ms` from `now`, rounded up to whole ticks, plus one tick (the current
    /// tick may be almost over).
    pub fn after(now: u64, ms: u64) -> Self {
        Deadline {
            end: now.saturating_add(ms.div_ceil(MS_PER_TICK) + 1),
        }
    }

    pub fn passed(&self, now: u64) -> bool {
        now >= self.end
    }
}

/// One look at a transfer: done with `(completion code, residue)`, still
/// pending, or the device is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    Done(u32, u32),
    Pending,
    Gone,
}

/// How a [`wait_by`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waited {
    Done(u32, u32),
    /// The deadline passed.
    TimedOut,
    /// The device is gone.
    Gone,
    /// The poll backstop ran out with the clock not reaching the deadline
    /// (a tick that does not advance).
    Backstop,
}

/// Wait for one transfer until `deadline`: `poll` looks, `idle` pauses
/// between looks, `now` reads the tick. At most `backstop` looks, for a tick
/// that does not advance (interrupts off).
pub fn wait_by(
    deadline: Deadline,
    backstop: u64,
    mut now: impl FnMut() -> u64,
    mut poll: impl FnMut() -> Poll,
    mut idle: impl FnMut(),
) -> Waited {
    for _ in 0..backstop {
        match poll() {
            Poll::Done(code, residue) => return Waited::Done(code, residue),
            Poll::Gone => return Waited::Gone,
            Poll::Pending => {}
        }
        if deadline.passed(now()) {
            return Waited::TimedOut;
        }
        idle();
    }
    Waited::Backstop
}

/// Which write-path command an event is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Write,
    Sync,
}

/// What the write path reports (the kernel prints each as one serial line;
/// the host tests collect them). These are the PR's trap-bank discriminators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteEvent {
    /// The device went away during `op`.
    Gone { op: Op, lba: u32 },
    /// UNIT ATTENTION; the command is retried once (`INITIAL_READ10`).
    UnitAttention {
        op: Op,
        lba: u32,
        blocks: u16,
        sense: Sense,
    },
    /// NOT READY; retry `attempt` after a pause.
    NotReady {
        op: Op,
        lba: u32,
        sense: Sense,
        attempt: u32,
    },
    /// DATA PROTECT: write-protect is now latched.
    DataProtect { op: Op, lba: u32 },
    /// SYNCHRONIZE CACHE refused with ILLEGAL REQUEST: flushing is now off.
    SyncUnsupported,
    /// A failed status that ends the command: the device took `took` bytes;
    /// the sense, if one came back.
    Failed {
        op: Op,
        lba: u32,
        blocks: u16,
        took: usize,
        sense: Option<Sense>,
    },
    /// Reset recovery ran (the `n`th in this chunk).
    Reset {
        op: Op,
        lba: u32,
        blocks: u16,
        n: u32,
        reason: ResetReason,
        recovered: bool,
    },
    /// The attempt's deadline passed (`MAX_SECTORS_*`): `stage` is where the
    /// command stood (`None`: in its REQUEST SENSE), after `elapsed` ticks.
    Deadline {
        op: Op,
        lba: u32,
        blocks: u16,
        ms: u64,
        stage: Option<ResetReason>,
        elapsed: u64,
    },
}

/// The device side of the write path.
pub trait WritePort {
    /// The tick clock.
    fn now(&mut self) -> u64;
    /// The device was unplugged.
    fn gone(&self) -> bool;
    /// One write-path command ([`command_dir`]); every wait in it, recovery
    /// included, stops at `by`.
    fn command(&mut self, cdb: &[u8], data: Data<'_>, by: Deadline) -> Outcome;
    /// REQUEST SENSE through the write path, stopping at `by` (the failed
    /// command's own deadline). `None` if no sense came back.
    fn sense(&mut self, by: Deadline) -> Option<Sense>;
    /// Wait about `ms` (NOT READY's back-off).
    fn pause(&mut self, ms: u64);
    fn event(&mut self, event: WriteEvent);
}

/// What the probe and the writes have learned, shared by every path that
/// reads it (`write_protected()`, `supports_flush()`, the write gate).
pub struct WriteLatches {
    /// 0 unknown, 1 not write-protected, 2 write-protected.
    protect: core::sync::atomic::AtomicU8,
    sync_ok: core::sync::atomic::AtomicBool,
}

const PROTECT_UNKNOWN: u8 = 0;
const PROTECT_NO: u8 = 1;
const PROTECT_YES: u8 = 2;

impl Default for WriteLatches {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteLatches {
    pub const fn new() -> Self {
        WriteLatches {
            protect: core::sync::atomic::AtomicU8::new(PROTECT_UNKNOWN),
            sync_ok: core::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn write_protected(&self) -> Option<bool> {
        use core::sync::atomic::Ordering;
        match self.protect.load(Ordering::Acquire) {
            PROTECT_NO => Some(false),
            PROTECT_YES => Some(true),
            _ => None,
        }
    }

    /// MODE SENSE's answer. Never clears a latched write-protect.
    pub fn note_mode_sense(&self, write_protected: bool) {
        use core::sync::atomic::Ordering;
        let new = if write_protected {
            PROTECT_YES
        } else {
            PROTECT_NO
        };
        let _ = self.protect.fetch_max(new, Ordering::AcqRel);
    }

    /// DATA PROTECT from any write-path command: latched for good.
    pub fn latch_write_protected(&self) {
        use core::sync::atomic::Ordering;
        self.protect.store(PROTECT_YES, Ordering::Release);
    }

    pub fn sync_ok(&self) -> bool {
        self.sync_ok.load(core::sync::atomic::Ordering::Acquire)
    }

    pub fn set_sync_ok(&self, ok: bool) {
        self.sync_ok
            .store(ok, core::sync::atomic::Ordering::Release);
    }
}

/// How a WRITE chunk ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkResult {
    /// The device took every byte.
    Written,
    NoDevice,
    /// DATA PROTECT (now latched); refuse.
    WriteProtected,
    Io,
}

/// One WRITE(10) chunk of `blocks` blocks at `lba` (THINK §3.4): each attempt
/// has one deadline for its command, its sense and its recovery; at most
/// [`WRITE_MAX_ATTEMPTS`] attempts; a passed deadline is final.
pub fn write_chunk(
    port: &mut impl WritePort,
    latches: &WriteLatches,
    lba: u32,
    blocks: u16,
    bytes: &[u8],
) -> ChunkResult {
    let op = Op::Write;
    let mut attentions = 0;
    let mut not_ready = 0;
    let mut resets = 0;
    for _ in 0..WRITE_MAX_ATTEMPTS {
        if port.gone() {
            port.event(WriteEvent::Gone { op, lba });
            return ChunkResult::NoDevice;
        }
        let start = port.now();
        let by = Deadline::after(start, WRITE_ATTEMPT_MS);
        let deadline = |port: &mut _, stage| {
            let elapsed = WritePort::now(port).saturating_sub(start);
            WritePort::event(
                port,
                WriteEvent::Deadline {
                    op,
                    lba,
                    blocks,
                    ms: WRITE_ATTEMPT_MS,
                    stage,
                    elapsed,
                },
            );
            ChunkResult::Io
        };
        match port.command(&cdb_write_10(lba, blocks), Data::Out(bytes), by) {
            Outcome::Passed { len } if len == bytes.len() => return ChunkResult::Written,
            Outcome::Passed { len } | Outcome::Failed { len } => {
                let sense = port.sense(by);
                if sense.is_none() && port.gone() {
                    port.event(WriteEvent::Gone { op, lba });
                    return ChunkResult::NoDevice;
                }
                if sense.is_none() && by.passed(port.now()) {
                    return deadline(port, None);
                }
                match sense.map(write_action) {
                    Some(WriteAction::Retry) if attentions == 0 => {
                        attentions += 1;
                        port.event(WriteEvent::UnitAttention {
                            op,
                            lba,
                            blocks,
                            sense: sense.unwrap_or(NO_SENSE),
                        });
                    }
                    Some(WriteAction::Wait) if not_ready < NOT_READY_TRIES => {
                        not_ready += 1;
                        port.event(WriteEvent::NotReady {
                            op,
                            lba,
                            sense: sense.unwrap_or(NO_SENSE),
                            attempt: not_ready,
                        });
                        port.pause(NOT_READY_PAUSE_MS);
                    }
                    Some(WriteAction::WriteProtected) => {
                        latches.latch_write_protected();
                        port.event(WriteEvent::DataProtect { op, lba });
                        return ChunkResult::WriteProtected;
                    }
                    _ => {
                        port.event(WriteEvent::Failed {
                            op,
                            lba,
                            blocks,
                            took: len,
                            sense,
                        });
                        return ChunkResult::Io;
                    }
                }
            }
            Outcome::Reset { reason, recovered } => {
                if port.gone() {
                    port.event(WriteEvent::Gone { op, lba });
                    return ChunkResult::NoDevice;
                }
                if by.passed(port.now()) {
                    return deadline(port, Some(reason));
                }
                resets += 1;
                port.event(WriteEvent::Reset {
                    op,
                    lba,
                    blocks,
                    n: resets,
                    reason,
                    recovered,
                });
                if reason == ResetReason::NotAllowed || resets > 1 || !recovered {
                    return ChunkResult::Io;
                }
            }
        }
    }
    port.event(WriteEvent::Failed {
        op,
        lba,
        blocks,
        took: 0,
        sense: None,
    });
    ChunkResult::Io
}

/// Sense bytes for an event when none came back.
const NO_SENSE: Sense = Sense {
    key: 0xff,
    asc: 0xff,
    ascq: 0xff,
};

/// How one SYNCHRONIZE CACHE(10) ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncResult {
    Synced,
    /// ILLEGAL REQUEST: the device doesn't do it.
    Unsupported,
    /// DATA PROTECT (now latched).
    WriteProtected,
    NoDevice,
    Failed,
}

/// One SYNCHRONIZE CACHE(10), with one deadline for the command, its sense
/// and its recovery. DATA PROTECT latches write-protect here as on a WRITE.
pub fn sync_once(port: &mut impl WritePort, latches: &WriteLatches) -> SyncResult {
    let op = Op::Sync;
    if port.gone() {
        port.event(WriteEvent::Gone { op, lba: 0 });
        return SyncResult::NoDevice;
    }
    let start = port.now();
    let by = Deadline::after(start, SYNC_ATTEMPT_MS);
    let deadline = |port: &mut _, stage| {
        let elapsed = WritePort::now(port).saturating_sub(start);
        WritePort::event(
            port,
            WriteEvent::Deadline {
                op,
                lba: 0,
                blocks: 0,
                ms: SYNC_ATTEMPT_MS,
                stage,
                elapsed,
            },
        );
        SyncResult::Failed
    };
    match port.command(&cdb_synchronize_cache_10(), Data::None, by) {
        Outcome::Passed { .. } => SyncResult::Synced,
        Outcome::Failed { .. } => {
            let sense = port.sense(by);
            if sense.is_none() && port.gone() {
                port.event(WriteEvent::Gone { op, lba: 0 });
                return SyncResult::NoDevice;
            }
            if sense.is_none() && by.passed(port.now()) {
                return deadline(port, None);
            }
            match sense.map(write_action) {
                Some(WriteAction::Unsupported) => SyncResult::Unsupported,
                Some(WriteAction::WriteProtected) => {
                    latches.latch_write_protected();
                    port.event(WriteEvent::DataProtect { op, lba: 0 });
                    SyncResult::WriteProtected
                }
                _ => {
                    port.event(WriteEvent::Failed {
                        op,
                        lba: 0,
                        blocks: 0,
                        took: 0,
                        sense,
                    });
                    SyncResult::Failed
                }
            }
        }
        Outcome::Reset { reason, recovered } => {
            if port.gone() {
                port.event(WriteEvent::Gone { op, lba: 0 });
                return SyncResult::NoDevice;
            }
            if by.passed(port.now()) {
                return deadline(port, Some(reason));
            }
            port.event(WriteEvent::Reset {
                op,
                lba: 0,
                blocks: 0,
                n: 1,
                reason,
                recovered,
            });
            SyncResult::Failed
        }
    }
}

/// How a flush ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlushResult {
    Flushed,
    /// Flushing is off (never probed ok, or ILLEGAL REQUEST since).
    Unsupported,
    WriteProtected,
    NoDevice,
    Io,
}

/// `flush`: SYNCHRONIZE CACHE only once the probe showed it works, at most
/// [`SYNC_MAX_ATTEMPTS`] times. ILLEGAL REQUEST latches flushing off; DATA
/// PROTECT latches write-protect and ends it.
pub fn flush(port: &mut impl WritePort, latches: &WriteLatches) -> FlushResult {
    if !latches.sync_ok() {
        return FlushResult::Unsupported;
    }
    for _ in 0..SYNC_MAX_ATTEMPTS {
        match sync_once(port, latches) {
            SyncResult::Synced => return FlushResult::Flushed,
            SyncResult::Unsupported => {
                latches.set_sync_ok(false);
                port.event(WriteEvent::SyncUnsupported);
                return FlushResult::Unsupported;
            }
            SyncResult::WriteProtected => return FlushResult::WriteProtected,
            SyncResult::NoDevice => return FlushResult::NoDevice,
            SyncResult::Failed => {}
        }
    }
    FlushResult::Io
}

/// The write probe's answer (THINK §3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeResult {
    pub write_protected: Option<bool>,
    pub sync: SyncResult,
}

/// The write probe: MODE SENSE(6)'s header for write-protect, TEST UNIT READY
/// (some devices report UNIT ATTENTION after a refused MODE SENSE), one
/// SYNCHRONIZE CACHE(10). Each has its own deadline, and so does each sense.
pub fn write_probe(port: &mut impl WritePort, latches: &WriteLatches) -> ProbeResult {
    let mut header = [0u8; MODE_SENSE_HEADER_LEN as usize];
    let by = Deadline::after(port.now(), PROBE_ATTEMPT_MS);
    match port.command(&cdb_mode_sense_6_header(), Data::In(&mut header), by) {
        Outcome::Passed { len } => {
            if let Some(wp) = mode_sense_write_protected(&header[..len]) {
                latches.note_mode_sense(wp);
            }
        }
        // Clear the device's pending sense; unknown is the answer.
        Outcome::Failed { .. } => {
            let _ = port.sense(by);
        }
        Outcome::Reset { .. } => {}
    }
    let by = Deadline::after(port.now(), PROBE_ATTEMPT_MS);
    if let Outcome::Failed { .. } = port.command(&cdb_test_unit_ready(), Data::None, by) {
        let _ = port.sense(by);
    }
    let sync = sync_once(port, latches);
    latches.set_sync_ok(sync == SyncResult::Synced);
    ProbeResult {
        write_protected: latches.write_protected(),
        sync,
    }
}
