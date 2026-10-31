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
    /// UNIT ATTENTION (power-on, medium changed): ask again.
    Retry,
    /// NOT READY, medium not present: bind no block device.
    NoMedium,
    /// Anything else: give up on the device.
    Fail,
}

pub fn ready_action(sense: Sense) -> ReadyAction {
    match (sense.key, sense.asc) {
        (SENSE_KEY_UNIT_ATTENTION, _) => ReadyAction::Retry,
        (SENSE_KEY_NOT_READY, ASC_MEDIUM_NOT_PRESENT) => ReadyAction::NoMedium,
        // NOT READY, becoming ready (ASC 04, ASCQ 01).
        (SENSE_KEY_NOT_READY, 0x04) if sense.ascq == 0x01 => ReadyAction::Retry,
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
}

/// Run one command with an optional data-in stage into `data` (U3–U6; BOT
/// §6.7's host-side cases for a device-to-host or no-data command).
pub fn command(pipe: &mut impl Pipe, tag: u32, lun: u8, cdb: &[u8], data: &mut [u8]) -> Outcome {
    let data_len = data.len() as u32;
    let reset = |pipe: &mut _, reason| Outcome::Reset {
        reason,
        recovered: reset_recovery(pipe),
    };
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
