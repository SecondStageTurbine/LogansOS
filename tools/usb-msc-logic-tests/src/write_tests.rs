// SPDX-License-Identifier: MPL-2.0
//! OT-100 R2: host KATs for the write path in `msc_logic.rs`
//! (`docs/THINK-2026-11-10-ot100-usb-persistence.md` §3).
//!
//! The byte oracle is independent of the builders: every CBW, CDB and CSW is
//! written out as literal bytes from BOT 1.0 §5.1/§5.2 and SBC-3 (WRITE(10)
//! 5.35, SYNCHRONIZE CACHE(10) 5.22) / SPC-4 (MODE SENSE(6) 6.11, its
//! header 7.5.5: device-specific parameter bit 7 = WP for direct-access
//! devices). Nothing below is produced by the code under test.

use super::msc_logic::*;
use std::collections::VecDeque;

/// What the recording pipe saw, with the bytes of every bulk-OUT.
#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Out(Vec<u8>),
    In(usize),
    ClearIn,
    ClearOut,
    Reset,
}

enum Reply {
    Out(Xfer),
    In(Xfer, Vec<u8>),
}

struct Rec {
    seen: Vec<Seen>,
    replies: VecDeque<Reply>,
}

impl Rec {
    fn new(replies: Vec<Reply>) -> Self {
        Rec {
            seen: Vec::new(),
            replies: replies.into(),
        }
    }
}

impl Pipe for Rec {
    fn bulk_out(&mut self, data: &[u8]) -> Xfer {
        self.seen.push(Seen::Out(data.to_vec()));
        match self.replies.pop_front() {
            Some(Reply::Out(x)) => x,
            _ => panic!("unexpected bulk_out"),
        }
    }
    fn bulk_in(&mut self, buf: &mut [u8]) -> Xfer {
        self.seen.push(Seen::In(buf.len()));
        match self.replies.pop_front() {
            Some(Reply::In(x, bytes)) => {
                buf[..bytes.len()].copy_from_slice(&bytes);
                x
            }
            _ => panic!("unexpected bulk_in"),
        }
    }
    fn clear_halt_in(&mut self) -> bool {
        self.seen.push(Seen::ClearIn);
        true
    }
    fn clear_halt_out(&mut self) -> bool {
        self.seen.push(Seen::ClearOut);
        true
    }
    fn mass_storage_reset(&mut self) -> bool {
        self.seen.push(Seen::Reset);
        true
    }
}

const TAG: u32 = 0xa1b2_c3d4;

/// A CSW for `TAG`, laid out by hand: "USBS", tag LE, residue LE, status.
fn csw(residue: u32, status: u8) -> Vec<u8> {
    let r = residue.to_le_bytes();
    vec![
        0x55, 0x53, 0x42, 0x53, // "USBS"
        0xd4, 0xc3, 0xb2, 0xa1, // tag 0xa1b2c3d4
        r[0], r[1], r[2], r[3], // residue
        status,
    ]
}

/// 1024 bytes (two 512-byte blocks) of a recognisable pattern.
fn pattern() -> Vec<u8> {
    (0..1024u32).map(|i| (i * 7 + 3) as u8).collect()
}

/// WRITE(10) of LBA 0x01020304, 2 blocks (1024 bytes out), tag 0xa1b2c3d4,
/// LUN 0: BOT §5.1 + SBC-3 5.35.
#[rustfmt::skip]
const CBW_WRITE10: [u8; 31] = [
    0x55, 0x53, 0x42, 0x43, // "USBC"
    0xd4, 0xc3, 0xb2, 0xa1, // tag
    0x00, 0x04, 0x00, 0x00, // dCBWDataTransferLength = 1024
    0x00,                   // bmCBWFlags: host-to-device
    0x00,                   // LUN 0
    0x0a,                   // CDB length 10
    0x2a, 0x00,             // WRITE(10), no flags (no FUA)
    0x01, 0x02, 0x03, 0x04, // LBA, big-endian
    0x00,                   // group
    0x00, 0x02,             // 2 blocks, big-endian
    0x00,                   // control
    0, 0, 0, 0, 0, 0,       // CDB padding to 16
];

/// SYNCHRONIZE CACHE(10) of the whole medium, no data stage.
#[rustfmt::skip]
const CBW_SYNC10: [u8; 31] = [
    0x55, 0x53, 0x42, 0x43,
    0xd4, 0xc3, 0xb2, 0xa1,
    0x00, 0x00, 0x00, 0x00, // no data
    0x00,                   // no direction
    0x00,
    0x0a,
    0x35, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0, 0, 0, 0, 0, 0,
];

/// MODE SENSE(6), DBD, page 0x3f, 4 bytes in.
#[rustfmt::skip]
const CBW_MODE_SENSE6: [u8; 31] = [
    0x55, 0x53, 0x42, 0x43,
    0xd4, 0xc3, 0xb2, 0xa1,
    0x04, 0x00, 0x00, 0x00, // 4 bytes
    0x80,                   // device-to-host
    0x00,
    0x06,
    0x1a, 0x08, 0x3f, 0x00, 0x04, 0x00,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// READ(10) of LBA 2048, 8 blocks, tag 0xa1b2c3d4: today's read CBW.
#[rustfmt::skip]
const CBW_READ10: [u8; 31] = [
    0x55, 0x53, 0x42, 0x43,
    0xd4, 0xc3, 0xb2, 0xa1,
    0x00, 0x10, 0x00, 0x00, // 4096 bytes
    0x80,                   // device-to-host
    0x00,
    0x0a,
    0x28, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08, 0x00,
    0, 0, 0, 0, 0, 0,
];

/// Dexter's relay-385 test (THINK §3.2): a WRITE(10) goes out with the CBW
/// flag 0x00, its data stage is one bulk-OUT of exactly the pattern, and a
/// CSW with residue 0 passes.
#[test]
fn usb_msc_write_cbw_direction() {
    let data = pattern();
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::Out(Xfer::Done(1024)),
        Reply::In(Xfer::Done(13), csw(0, 0)),
    ]);
    let got = command_dir(
        &mut pipe,
        TAG,
        0,
        &cdb_write_10(0x0102_0304, 2),
        Data::Out(&data),
    );
    assert_eq!(got, Outcome::Passed { len: 1024 });
    assert_eq!(
        pipe.seen,
        vec![
            Seen::Out(CBW_WRITE10.to_vec()),
            Seen::Out(data.clone()),
            Seen::In(13)
        ]
    );
    // The flag byte on its own, as the review names it.
    let Seen::Out(cbw) = &pipe.seen[0] else {
        unreachable!()
    };
    assert_eq!(cbw[12], 0x00);
    assert_ne!(cbw[12], 0x80);
}

#[test]
fn new_cdbs_match_the_hand_laid_bytes() {
    assert_eq!(
        cdb_write_10(0x0102_0304, 2),
        [0x2a, 0x00, 0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x02, 0x00]
    );
    assert_eq!(
        cdb_write_10(u32::MAX, 0x80),
        [0x2a, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x80, 0x00]
    );
    assert_eq!(
        cdb_synchronize_cache_10(),
        [0x35, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        cdb_mode_sense_6_header(),
        [0x1a, 0x08, 0x3f, 0x00, 0x04, 0x00]
    );
}

#[test]
fn synchronize_cache_has_no_data_stage() {
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::In(Xfer::Done(13), csw(0, 0)),
    ]);
    let got = command_dir(&mut pipe, TAG, 0, &cdb_synchronize_cache_10(), Data::None);
    assert_eq!(got, Outcome::Passed { len: 0 });
    assert_eq!(
        pipe.seen,
        vec![Seen::Out(CBW_SYNC10.to_vec()), Seen::In(13)]
    );
}

#[test]
fn mode_sense_reads_the_write_protect_bit() {
    let mut reply = [0u8; 4];
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        // Mode data length 3, medium type 0, device-specific 0x80 (WP), no
        // block descriptors.
        Reply::In(Xfer::Done(4), vec![0x03, 0x00, 0x80, 0x00]),
        Reply::In(Xfer::Done(13), csw(0, 0)),
    ]);
    let got = command_dir(
        &mut pipe,
        TAG,
        0,
        &cdb_mode_sense_6_header(),
        Data::In(&mut reply),
    );
    assert_eq!(got, Outcome::Passed { len: 4 });
    assert_eq!(pipe.seen[0], Seen::Out(CBW_MODE_SENSE6.to_vec()));
    assert_eq!(mode_sense_write_protected(&reply), Some(true));
    assert_eq!(
        mode_sense_write_protected(&[0x03, 0x00, 0x00, 0x00]),
        Some(false)
    );
    assert_eq!(mode_sense_write_protected(&[0x03, 0x00, 0x7f]), Some(false));
    assert_eq!(mode_sense_write_protected(&[0x03, 0x00]), None);
}

/// The read path through the write-path entry point gives today's READ(10)
/// CBW byte for byte, and so does today's `command` (the kill-switch's side:
/// the read CBW never changed).
#[test]
fn read10_cbw_is_unchanged_on_both_paths() {
    for via_new in [false, true] {
        let mut buf = vec![0u8; 4096];
        let mut pipe = Rec::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(4096), vec![0x5a; 4096]),
            Reply::In(Xfer::Done(13), csw(0, 0)),
        ]);
        let cdb = cdb_read_10(2048, 8);
        let got = if via_new {
            command_dir(&mut pipe, TAG, 0, &cdb, Data::In(&mut buf))
        } else {
            command(&mut pipe, TAG, 0, &cdb, &mut buf)
        };
        assert_eq!(got, Outcome::Passed { len: 4096 });
        assert_eq!(pipe.seen[0], Seen::Out(CBW_READ10.to_vec()));
        assert!(buf.iter().all(|&b| b == 0x5a));
    }
}

/// The kill-switch's table: with `usb-storage-write` off the kernel only
/// calls `command`, whose allowlist is today's six opcodes and refuses every
/// write-path command before the pipe.
#[test]
fn read_only_allowlist_is_todays() {
    assert_eq!(ALLOWED_OPCODES, [0x12, 0x00, 0x03, 0x25, 0x9e, 0x28]);
    for cdb in [
        cdb_write_10(0, 1).to_vec(),
        cdb_synchronize_cache_10().to_vec(),
        cdb_mode_sense_6_header().to_vec(),
    ] {
        assert!(!cdb_allowed(&cdb), "{cdb:02x?}");
        let mut pipe = Rec::new(vec![]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb, &mut []),
            Outcome::Reset {
                reason: ResetReason::NotAllowed,
                recovered: true
            }
        );
        assert!(pipe.seen.is_empty(), "{cdb:02x?} reached the device");
    }
}

/// A known opcode in the wrong direction, an empty data stage that claims a
/// direction, and an unknown opcode all go nowhere.
#[test]
fn wrong_direction_is_refused_before_the_pipe() {
    let refused = Outcome::Reset {
        reason: ResetReason::NotAllowed,
        recovered: true,
    };
    let block = [0u8; 512];
    let mut sink = [0u8; 512];
    let run = |cdb: &[u8], data: Data<'_>| {
        let mut pipe = Rec::new(vec![]);
        let got = command_dir(&mut pipe, TAG, 0, cdb, data);
        assert!(pipe.seen.is_empty(), "{cdb:02x?} reached the device");
        got
    };
    // WRITE(10) as a read, or with no data.
    assert_eq!(run(&cdb_write_10(0, 1), Data::In(&mut sink)), refused);
    assert_eq!(run(&cdb_write_10(0, 1), Data::None), refused);
    // READ(10) as a write.
    assert_eq!(run(&cdb_read_10(0, 1), Data::Out(&block)), refused);
    // SYNCHRONIZE CACHE with data either way.
    assert_eq!(run(&cdb_synchronize_cache_10(), Data::Out(&block)), refused);
    // MODE SENSE as a write.
    assert_eq!(run(&cdb_mode_sense_6_header(), Data::Out(&block)), refused);
    // An empty Out slice claims a direction with no data.
    assert_eq!(run(&cdb_write_10(0, 1), Data::Out(&[])), refused);
    // Opcodes on neither list.
    assert_eq!(run(&[0x1b, 0, 0, 0, 1, 0], Data::None), refused); // START STOP UNIT
    assert_eq!(run(&[0x1e, 0, 0, 0, 1, 0], Data::None), refused); // PREVENT ALLOW
    assert_eq!(
        run(
            &[0x8a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
            Data::Out(&block)
        ),
        refused
    ); // WRITE(16)
    assert!(cdb_allowed_dir(&cdb_write_10(0, 1), Dir::Out));
    assert!(!cdb_allowed_dir(&[], Dir::None));
}

/// BOT §6.7.3: the device stalls bulk-out mid-data. The host clears the
/// bulk-out halt and still reads the CSW, which says the write failed.
#[test]
fn a_stalled_data_out_clears_out_then_reads_the_csw() {
    let data = pattern();
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::Out(Xfer::Stall),
        Reply::In(Xfer::Done(13), csw(1024, 1)),
    ]);
    let got = command_dir(
        &mut pipe,
        TAG,
        0,
        &cdb_write_10(0x0102_0304, 2),
        Data::Out(&data),
    );
    assert_eq!(got, Outcome::Failed { len: 0 });
    assert_eq!(
        pipe.seen,
        vec![
            Seen::Out(CBW_WRITE10.to_vec()),
            Seen::Out(data),
            Seen::ClearOut,
            Seen::In(13)
        ]
    );
}

/// A passed CSW with a residue on a write is a short write: `Failed`, with
/// the bytes the device took.
#[test]
fn a_write_residue_is_a_failure() {
    let data = pattern();
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::Out(Xfer::Done(1024)),
        Reply::In(Xfer::Done(13), csw(512, 0)),
    ]);
    let got = command_dir(&mut pipe, TAG, 0, &cdb_write_10(1, 2), Data::Out(&data));
    assert_eq!(got, Outcome::Failed { len: 512 });
}

/// The device claims it took more than was sent: the transport is out of
/// step, so reset recovery runs.
#[test]
fn a_device_claiming_more_than_sent_is_reset() {
    let data = pattern();
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::Out(Xfer::Done(512)),
        Reply::In(Xfer::Done(13), csw(0, 0)),
    ]);
    let got = command_dir(&mut pipe, TAG, 0, &cdb_write_10(1, 2), Data::Out(&data));
    assert_eq!(
        got,
        Outcome::Reset {
            reason: ResetReason::ResidueMismatch,
            recovered: true
        }
    );
    assert_eq!(
        &pipe.seen[3..],
        &[Seen::Reset, Seen::ClearIn, Seen::ClearOut]
    );
}

/// A data-out transport error is reset recovery, and no CSW is read.
#[test]
fn a_data_out_error_is_reset() {
    let data = pattern();
    let mut pipe = Rec::new(vec![Reply::Out(Xfer::Done(31)), Reply::Out(Xfer::Error)]);
    let got = command_dir(&mut pipe, TAG, 0, &cdb_write_10(1, 2), Data::Out(&data));
    assert_eq!(
        got,
        Outcome::Reset {
            reason: ResetReason::Data,
            recovered: true
        }
    );
}

/// A zero-length packet where the CSW should be is read past once.
#[test]
fn a_zero_length_csw_is_read_again() {
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::In(Xfer::Done(0), vec![]),
        Reply::In(Xfer::Done(13), csw(0, 0)),
    ]);
    let got = command_dir(&mut pipe, TAG, 0, &cdb_synchronize_cache_10(), Data::None);
    assert_eq!(got, Outcome::Passed { len: 0 });
    assert_eq!(pipe.seen.len(), 3);
}

/// A bad CSW signature after a write is a phase error: reset recovery.
#[test]
fn a_bad_csw_after_a_write_is_reset() {
    let data = pattern();
    let mut bad = csw(0, 0);
    bad[3] = 0x00;
    let mut pipe = Rec::new(vec![
        Reply::Out(Xfer::Done(31)),
        Reply::Out(Xfer::Done(1024)),
        Reply::In(Xfer::Done(13), bad),
    ]);
    let got = command_dir(&mut pipe, TAG, 0, &cdb_write_10(1, 2), Data::Out(&data));
    assert_eq!(
        got,
        Outcome::Reset {
            reason: ResetReason::Phase(PhaseReason::Signature),
            recovered: true
        }
    );
}

#[test]
fn next_write_splits_at_the_cap_and_refuses_past_32_bits() {
    // 300 blocks from LBA 63, 128 at a time: 128, 128, 44.
    let mut done = 0;
    let mut parts = vec![];
    while let Some((at, n)) = next_write(63, 300, done, 128).unwrap() {
        parts.push((at, n));
        done += u64::from(n);
    }
    assert_eq!(parts, vec![(63, 128), (191, 128), (319, 44)]);
    // A smaller cap (the MAX_SECTORS_64 fallback).
    assert_eq!(next_write(0, 100, 0, 64), Ok(Some((0, 64))));
    assert_eq!(next_write(0, 100, 64, 64), Ok(Some((64, 36))));
    // The cap is clamped to 1..=128.
    assert_eq!(next_write(0, 500, 0, 0), Ok(Some((0, 1))));
    assert_eq!(next_write(0, 500, 0, 4096), Ok(Some((0, 128))));
    // The last addressable block, then one past.
    assert_eq!(
        next_write(u64::from(u32::MAX), 1, 0, 128),
        Ok(Some((u32::MAX, 1)))
    );
    assert_eq!(
        next_write(u64::from(u32::MAX) - 1, 2, 0, 128),
        Ok(Some((u32::MAX - 1, 2)))
    );
    assert_eq!(next_write(u64::from(u32::MAX), 2, 0, 128), Err(OutOfRange));
    assert_eq!(next_write(1 << 32, 1, 0, 128), Err(OutOfRange));
    assert_eq!(next_write(u64::MAX, 2, 1, 128), Err(OutOfRange));
    assert_eq!(next_write(5, 3, 3, 128), Ok(None));
}

#[test]
fn reads_past_32_bits_are_caught_before_any_command() {
    assert!(!read_past_32_bits(0, 0));
    assert!(!read_past_32_bits(0, 1 << 32));
    assert!(!read_past_32_bits(u64::from(u32::MAX), 1));
    assert!(read_past_32_bits(u64::from(u32::MAX), 2));
    assert!(read_past_32_bits(1 << 32, 1));
    assert!(read_past_32_bits(u64::MAX, 2));
}

#[test]
fn write_sense_mapping() {
    let s = |key, asc, ascq| Sense { key, asc, ascq };
    assert_eq!(write_action(s(0x6, 0x28, 0x00)), WriteAction::Retry);
    assert_eq!(write_action(s(0x2, 0x04, 0x01)), WriteAction::Wait);
    assert_eq!(
        write_action(s(0x7, 0x27, 0x00)),
        WriteAction::WriteProtected
    );
    assert_eq!(write_action(s(0x5, 0x20, 0x00)), WriteAction::Unsupported);
    assert_eq!(write_action(s(0x3, 0x0c, 0x00)), WriteAction::Fail);
    assert_eq!(write_action(s(0x4, 0x44, 0x00)), WriteAction::Fail);
    assert_eq!(write_action(s(0xb, 0x00, 0x00)), WriteAction::Fail);
    // The sense bytes the driver decodes (fixed format), laid out by hand:
    // DATA PROTECT, ASC 0x27 (write protected).
    let fixed = [
        0x70, 0x00, 0x07, 0, 0, 0, 0, 0x0a, 0, 0, 0, 0, 0x27, 0x00, 0, 0, 0, 0,
    ];
    assert_eq!(
        decode_sense(&fixed).map(write_action),
        Some(WriteAction::WriteProtected)
    );
}
