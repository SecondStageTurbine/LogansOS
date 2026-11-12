// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/drivers/usb/msc_logic.rs` (L3u-1,
//! `docs/THINK-2026-10-31-l3u-usb-mass-storage.md`). The module is mounted by
//! `#[path]`, so these run the exact code the kernel will.
//!
//! The byte oracle is independent of the encoder: every wrapper, CDB and
//! reply below is written out as literal bytes from the layouts in the USB
//! Mass Storage Bulk-Only Transport 1.0 spec (§5.1 CBW, §5.2 CSW) and SBC/SPC
//! (READ(10), READ CAPACITY(10)/(16), INQUIRY, REQUEST SENSE), cross-read
//! against the raw source of `usbd-storage` 3.0.0 (MIT; signature constants
//! `0x43425355`/`0x53425355`, lengths 31/13). Nothing here is produced by the
//! code under test.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/usb/msc_logic.rs"]
mod msc_logic;

// OT-100 R2: the write path's KATs.
#[cfg(test)]
mod write_tests;

// OT-100 R2, relay 393 F1–F3: deadlines, retries, latches, diagnostics.
#[cfg(test)]
mod policy_tests;

#[cfg(test)]
mod tests {
    use super::msc_logic::*;

    /// READ(10) of LBA 2048, 8 blocks (4096 bytes in), tag 0x12345678, LUN 0.
    #[rustfmt::skip]
    const CBW_READ10: [u8; 31] = [
        0x55, 0x53, 0x42, 0x43, // "USBC"
        0x78, 0x56, 0x34, 0x12, // tag
        0x00, 0x10, 0x00, 0x00, // 4096 bytes
        0x80,                   // data in
        0x00,                   // LUN 0
        0x0a,                   // 10-byte CDB
        // READ(10): opcode, flags, LBA 0x00000800 BE, group, 8 blocks BE, control
        0x28, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // CDB padding to 16
    ];

    /// TEST UNIT READY, tag 1, LUN 2: no data, so the direction bit is 0.
    #[rustfmt::skip]
    const CBW_TUR: [u8; 31] = [
        0x55, 0x53, 0x42, 0x43, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x06,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// "USBS", tag 0x12345678, residue 0, status passed.
    #[rustfmt::skip]
    const CSW_OK: [u8; 13] = [
        0x55, 0x53, 0x42, 0x53, 0x78, 0x56, 0x34, 0x12,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn cbw_matches_its_hand_laid_bytes() {
        assert_eq!(
            encode_cbw(0x1234_5678, 4096, true, 0, &cdb_read_10(2048, 8)),
            Some(CBW_READ10)
        );
        assert_eq!(
            encode_cbw(1, 0, true, 2, &cdb_test_unit_ready()),
            Some(CBW_TUR)
        );
        assert_eq!(CBW_READ10.len(), CBW_LEN);
    }

    #[test]
    fn cbw_refuses_bad_shapes() {
        assert_eq!(encode_cbw(1, 0, false, 0, &[]), None);
        assert_eq!(encode_cbw(1, 0, false, 0, &[0u8; 17]), None);
        assert_eq!(encode_cbw(1, 0, false, 16, &[0u8; 6]), None);
        // A 16-byte CDB fills the field exactly.
        let cbw = encode_cbw(1, 32, true, 0, &cdb_read_capacity_16()).unwrap();
        assert_eq!(cbw[14], 16);
        assert_eq!(&cbw[15..31], &cdb_read_capacity_16());
    }

    #[test]
    fn cdbs_match_their_hand_laid_bytes() {
        assert_eq!(cdb_inquiry(), [0x12, 0x00, 0x00, 0x00, 0x24, 0x00]);
        assert_eq!(cdb_test_unit_ready(), [0x00; 6]);
        assert_eq!(cdb_request_sense(), [0x03, 0x00, 0x00, 0x00, 0x12, 0x00]);
        assert_eq!(cdb_read_capacity_10(), [0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            cdb_read_capacity_16(),
            [
                0x9e, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x00, 0x00, 0x20, 0, 0
            ]
        );
        // LBA 0x01020304, 0x0080 blocks.
        assert_eq!(
            cdb_read_10(0x0102_0304, 128),
            [0x28, 0x00, 0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x80, 0x00]
        );
    }

    #[test]
    fn csw_verdicts() {
        assert_eq!(
            check_csw(&CSW_OK, 0x1234_5678, 4096),
            CswVerdict::Passed { residue: 0 }
        );
        let mut failed = CSW_OK;
        failed[12] = 1;
        failed[8] = 0x00;
        failed[9] = 0x02; // residue 512
        assert_eq!(
            check_csw(&failed, 0x1234_5678, 4096),
            CswVerdict::Failed { residue: 512 }
        );
        assert_eq!(
            check_csw(&CSW_OK, 0x1234_5679, 4096),
            CswVerdict::PhaseError(PhaseReason::Tag)
        );
        let mut sig = CSW_OK;
        sig[3] = 0x43; // "USBC" is a CBW, not a CSW
        assert_eq!(
            check_csw(&sig, 0x1234_5678, 4096),
            CswVerdict::PhaseError(PhaseReason::Signature)
        );
        let mut phase = CSW_OK;
        phase[12] = 2;
        assert_eq!(
            check_csw(&phase, 0x1234_5678, 4096),
            CswVerdict::PhaseError(PhaseReason::Status)
        );
        phase[12] = 3;
        assert_eq!(
            check_csw(&phase, 0x1234_5678, 4096),
            CswVerdict::PhaseError(PhaseReason::Status)
        );
        let mut over = CSW_OK;
        over[9] = 0x10;
        over[8] = 0x01; // residue 4097 > 4096
        assert_eq!(
            check_csw(&over, 0x1234_5678, 4096),
            CswVerdict::PhaseError(PhaseReason::Residue)
        );
        assert_eq!(
            check_csw(&CSW_OK[..12], 0x1234_5678, 4096),
            CswVerdict::PhaseError(PhaseReason::Length)
        );
        let mut long = [0u8; 14];
        long[..13].copy_from_slice(&CSW_OK);
        assert_eq!(
            check_csw(&long, 0x1234_5678, 4096),
            CswVerdict::PhaseError(PhaseReason::Length)
        );
    }

    #[test]
    fn capacity_10_is_last_lba_plus_one() {
        // A 16 GB stick: last LBA 0x01DD_0FFF, 512-byte blocks.
        let reply = [0x01, 0xdd, 0x0f, 0xff, 0x00, 0x00, 0x02, 0x00];
        assert_eq!(
            decode_capacity_10(&reply),
            Capacity10::Known(Capacity {
                blocks: 0x01dd_1000,
                block_len: 512
            })
        );
        assert_eq!(
            decode_capacity_10(&[0xff, 0xff, 0xff, 0xff, 0, 0, 2, 0]),
            Capacity10::Needs16
        );
        assert_eq!(
            decode_capacity_10(&[0, 0, 0, 7, 0, 0, 0, 0]),
            Capacity10::Invalid
        );
        // A 4K-sector drive (U13): reported as is.
        assert_eq!(
            decode_capacity_10(&[0x00, 0x00, 0x0f, 0xff, 0x00, 0x00, 0x10, 0x00]),
            Capacity10::Known(Capacity {
                blocks: 0x1000,
                block_len: 4096
            })
        );
    }

    #[test]
    fn capacity_16() {
        // 4 TB: last LBA 0x0000_0001_D1C0_BEAF, 512-byte blocks, then 20 more bytes.
        let mut reply = [0u8; 32];
        reply[..12].copy_from_slice(&[0, 0, 0, 0x01, 0xd1, 0xc0, 0xbe, 0xaf, 0, 0, 0x02, 0]);
        assert_eq!(
            decode_capacity_16(&reply),
            Some(Capacity {
                blocks: 0x1_d1c0_beb0,
                block_len: 512
            })
        );
        assert_eq!(decode_capacity_16(&reply[..11]), None);
        reply[10] = 0;
        assert_eq!(decode_capacity_16(&reply), None);
        let max = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 2, 0];
        assert_eq!(decode_capacity_16(&max), None);
    }

    #[test]
    fn sense_formats_and_actions() {
        // Fixed format: UNIT ATTENTION, ASC 28 (medium may have changed).
        let mut fixed = [0u8; 18];
        fixed[0] = 0x70;
        fixed[2] = 0x06;
        fixed[7] = 10;
        fixed[12] = 0x28;
        let s = decode_sense(&fixed).unwrap();
        assert_eq!(
            s,
            Sense {
                key: 6,
                asc: 0x28,
                ascq: 0
            }
        );
        assert_eq!(ready_action(s), ReadyAction::Retry);
        // Valid bit set on the response code still decodes.
        fixed[0] = 0xf0;
        assert_eq!(decode_sense(&fixed), Some(s));
        // Descriptor format: NOT READY, medium not present.
        let desc = [0x72, 0x02, 0x3a, 0x00, 0, 0, 0, 0];
        let s = decode_sense(&desc).unwrap();
        assert_eq!(ready_action(s), ReadyAction::NoMedium);
        // NOT READY, becoming ready.
        assert_eq!(
            ready_action(Sense {
                key: 2,
                asc: 4,
                ascq: 1
            }),
            ReadyAction::Wait
        );
        // NOT READY, initializing command required: START UNIT is not sent.
        assert_eq!(
            ready_action(Sense {
                key: 2,
                asc: 4,
                ascq: 2
            }),
            ReadyAction::NeedsStart
        );
        // NOT READY, other ASC 04 qualifiers (manual intervention, format in
        // progress): give up.
        assert_eq!(
            ready_action(Sense {
                key: 2,
                asc: 4,
                ascq: 3
            }),
            ReadyAction::Fail
        );
        assert_eq!(
            ready_action(Sense {
                key: 3,
                asc: 0x11,
                ascq: 0
            }),
            ReadyAction::Fail
        );
        assert_eq!(decode_sense(&[0x70, 0, 6]), None);
        assert_eq!(decode_sense(&[0x00; 18]), None);
        assert_eq!(decode_sense(&[]), None);
    }

    #[test]
    fn reads_split_at_64_kib() {
        assert_eq!(next_read(2048, 300, 0), Some((2048, 128)));
        assert_eq!(next_read(2048, 300, 128), Some((2176, 128)));
        assert_eq!(next_read(2048, 300, 256), Some((2304, 44)));
        assert_eq!(next_read(2048, 300, 300), None);
        // The last addressable READ(10) block is 0xFFFF_FFFF.
        assert_eq!(next_read(0xffff_ffff, 1, 0), Some((0xffff_ffff, 1)));
        assert_eq!(next_read(0xffff_ffff, 2, 0), None);
        assert_eq!(next_read(0x1_0000_0000, 1, 0), None);
    }

    #[test]
    fn binds_bulk_only_scsi_only() {
        assert!(is_bulk_only_scsi(0x08, 0x06, 0x50));
        assert!(!is_bulk_only_scsi(0x08, 0x06, PROTOCOL_UAS));
        assert!(!is_bulk_only_scsi(0x08, 0x02, 0x50));
        assert!(!is_bulk_only_scsi(0x03, 0x06, 0x50));
    }

    // === The BOT state machine against a scripted pipe ===================

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum Call {
        Out,
        In,
        ClearIn,
        ClearOut,
        Reset,
    }

    /// Replies in order; each `bulk_in` takes the next `In` reply.
    enum Reply {
        Out(Xfer),
        In(Xfer, Vec<u8>),
    }

    struct Fake {
        replies: std::collections::VecDeque<Reply>,
        calls: Vec<Call>,
        clear_ok: bool,
    }

    impl Fake {
        fn new(replies: Vec<Reply>) -> Self {
            Self {
                replies: replies.into(),
                calls: Vec::new(),
                clear_ok: true,
            }
        }
    }

    impl Pipe for Fake {
        fn bulk_out(&mut self, _data: &[u8]) -> Xfer {
            self.calls.push(Call::Out);
            match self.replies.pop_front() {
                Some(Reply::Out(x)) => x,
                _ => panic!("unexpected bulk_out"),
            }
        }
        fn bulk_in(&mut self, buf: &mut [u8]) -> Xfer {
            self.calls.push(Call::In);
            match self.replies.pop_front() {
                Some(Reply::In(x, bytes)) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    x
                }
                _ => panic!("unexpected bulk_in"),
            }
        }
        fn clear_halt_in(&mut self) -> bool {
            self.calls.push(Call::ClearIn);
            self.clear_ok
        }
        fn clear_halt_out(&mut self) -> bool {
            self.calls.push(Call::ClearOut);
            self.clear_ok
        }
        fn mass_storage_reset(&mut self) -> bool {
            self.calls.push(Call::Reset);
            true
        }
    }

    const TAG: u32 = 0x1234_5678;

    fn csw(status: u8, residue: u32) -> Vec<u8> {
        let mut c = CSW_OK.to_vec();
        c[8..12].copy_from_slice(&residue.to_le_bytes());
        c[12] = status;
        c
    }

    use Call::*;

    #[test]
    fn a_read_passes() {
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(4), vec![1, 2, 3, 4]),
            Reply::In(Xfer::Done(13), csw(0, 0)),
        ]);
        let mut data = [0u8; 4];
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_read_10(0, 1), &mut data),
            Outcome::Passed { len: 4 }
        );
        assert_eq!(data, [1, 2, 3, 4]);
        assert_eq!(pipe.calls, [Out, In, In]);
    }

    #[test]
    fn a_no_data_command_reads_only_the_csw() {
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(13), csw(1, 0)),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Failed { len: 0 }
        );
        assert_eq!(pipe.calls, [Out, In]);
    }

    #[test]
    fn a_stalled_data_stage_still_reads_the_csw() {
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Stall, vec![]),
            Reply::In(Xfer::Done(13), csw(1, 36)),
        ]);
        let mut data = [0u8; 36];
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_inquiry(), &mut data),
            Outcome::Failed { len: 0 }
        );
        assert_eq!(pipe.calls, [Out, In, ClearIn, In]);
    }

    #[test]
    fn a_stalled_csw_is_read_once_more() {
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Stall, vec![]),
            Reply::In(Xfer::Done(13), csw(0, 0)),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Passed { len: 0 }
        );
        assert_eq!(pipe.calls, [Out, In, ClearIn, In]);
    }

    #[test]
    fn two_stalled_csws_end_in_reset_recovery_in_order() {
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Stall, vec![]),
            Reply::In(Xfer::Stall, vec![]),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Csw,
                recovered: true
            }
        );
        // The class reset, then bulk-in, then bulk-out (U4).
        assert_eq!(
            pipe.calls,
            [Out, In, ClearIn, In, ClearIn, Reset, ClearIn, ClearOut]
        );
    }

    #[test]
    fn a_wrong_tag_is_a_phase_error() {
        let mut bad = csw(0, 0);
        bad[4] ^= 1;
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(13), bad),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Phase(PhaseReason::Tag),
                recovered: true
            }
        );
        assert_eq!(pipe.calls, [Out, In, Reset, ClearIn, ClearOut]);
    }

    #[test]
    fn a_phase_error_status_and_a_short_csw_reset() {
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(13), csw(2, 0)),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Phase(PhaseReason::Status),
                recovered: true
            }
        );
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(12), csw(0, 0)),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Phase(PhaseReason::Length),
                recovered: true
            }
        );
    }

    #[test]
    fn a_refused_cbw_or_broken_data_stage_resets() {
        let mut pipe = Fake::new(vec![Reply::Out(Xfer::Stall)]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Cbw,
                recovered: true
            }
        );
        assert_eq!(pipe.calls, [Out, Reset, ClearIn, ClearOut]);
        let mut pipe = Fake::new(vec![Reply::Out(Xfer::Done(30))]);
        assert!(matches!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Cbw,
                ..
            }
        ));
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Error, vec![]),
        ]);
        let mut data = [0u8; 8];
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_read_capacity_10(), &mut data),
            Outcome::Reset {
                reason: ResetReason::Data,
                recovered: true
            }
        );
    }

    #[test]
    fn a_short_read_is_reported_and_an_impossible_residue_resets() {
        // 36 asked, 20 arrived, residue 16: consistent.
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(20), vec![0; 20]),
            Reply::In(Xfer::Done(13), csw(0, 16)),
        ]);
        let mut data = [0u8; 36];
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_inquiry(), &mut data),
            Outcome::Passed { len: 20 }
        );
        // 36 arrived but the device says only 20 were sent.
        let mut pipe = Fake::new(vec![
            Reply::Out(Xfer::Done(31)),
            Reply::In(Xfer::Done(36), vec![0; 36]),
            Reply::In(Xfer::Done(13), csw(0, 16)),
        ]);
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_inquiry(), &mut data),
            Outcome::Reset {
                reason: ResetReason::ResidueMismatch,
                recovered: true
            }
        );
    }

    #[test]
    fn a_failed_halt_clear_is_reported_by_recovery() {
        let mut pipe = Fake::new(vec![Reply::Out(Xfer::Stall)]);
        pipe.clear_ok = false;
        assert_eq!(
            command(&mut pipe, TAG, 0, &cdb_test_unit_ready(), &mut []),
            Outcome::Reset {
                reason: ResetReason::Cbw,
                recovered: false
            }
        );
        // Every step still ran.
        assert_eq!(pipe.calls, [Out, Reset, ClearIn, ClearOut]);
    }

    /// Chad's #20 review (U17): a device that stalls every CBW whose opcode
    /// it doesn't support. Every command the driver can build passes on it;
    /// MODE SENSE, SYNCHRONIZE CACHE, PREVENT ALLOW MEDIUM REMOVAL, START
    /// STOP UNIT and WRITE(10) are refused before the pipe is touched.
    struct Picky {
        calls: usize,
        pending: Option<Vec<u8>>,
        tag: [u8; 4],
    }

    impl Pipe for Picky {
        fn bulk_out(&mut self, cbw: &[u8]) -> Xfer {
            self.calls += 1;
            let opcode = cbw[15];
            if !matches!(opcode, 0x12 | 0x00 | 0x03 | 0x25 | 0x9e | 0x28) {
                return Xfer::Stall;
            }
            self.tag.copy_from_slice(&cbw[4..8]);
            let len = u32::from_le_bytes([cbw[8], cbw[9], cbw[10], cbw[11]]) as usize;
            self.pending = (len > 0).then(|| vec![0u8; len]);
            Xfer::Done(cbw.len())
        }
        fn bulk_in(&mut self, buf: &mut [u8]) -> Xfer {
            self.calls += 1;
            if let Some(data) = self.pending.take() {
                buf[..data.len()].copy_from_slice(&data);
                return Xfer::Done(data.len());
            }
            let mut c = CSW_OK.to_vec();
            c[4..8].copy_from_slice(&self.tag);
            buf[..13].copy_from_slice(&c);
            Xfer::Done(13)
        }
        fn clear_halt_in(&mut self) -> bool {
            true
        }
        fn clear_halt_out(&mut self) -> bool {
            true
        }
        fn mass_storage_reset(&mut self) -> bool {
            true
        }
    }

    #[test]
    fn only_allowlisted_commands_reach_the_device() {
        // Every builder's opcode is on the list.
        for cdb in [
            cdb_inquiry().to_vec(),
            cdb_test_unit_ready().to_vec(),
            cdb_request_sense().to_vec(),
            cdb_read_capacity_10().to_vec(),
            cdb_read_capacity_16().to_vec(),
            cdb_read_10(0, 1).to_vec(),
        ] {
            assert!(cdb_allowed(&cdb), "{cdb:02x?}");
            let mut pipe = Picky {
                calls: 0,
                pending: None,
                tag: [0; 4],
            };
            let mut data = [0u8; 36];
            let len = if cdb[0] == 0x00 { 0 } else { 36 };
            let got = command(&mut pipe, TAG, 0, &cdb, &mut data[..len]);
            assert!(matches!(got, Outcome::Passed { .. }), "{cdb:02x?}: {got:?}");
        }
        // The commands that hang or stall devices, and a write, never go out.
        for cdb in [
            vec![0x1a, 0, 0x3f, 0, 192, 0],        // MODE SENSE(6)
            vec![0x35, 0, 0, 0, 0, 0, 0, 0, 0, 0], // SYNCHRONIZE CACHE(10)
            vec![0x1e, 0, 0, 0, 1, 0],             // PREVENT ALLOW MEDIUM REMOVAL
            vec![0x1b, 0, 0, 0, 1, 0],             // START STOP UNIT
            vec![0x2a, 0, 0, 0, 0, 0, 0, 0, 1, 0], // WRITE(10)
        ] {
            assert!(!cdb_allowed(&cdb));
            let mut pipe = Picky {
                calls: 0,
                pending: None,
                tag: [0; 4],
            };
            let got = command(&mut pipe, TAG, 0, &cdb, &mut []);
            assert_eq!(
                got,
                Outcome::Reset {
                    reason: ResetReason::NotAllowed,
                    recovered: true
                }
            );
            assert_eq!(pipe.calls, 0, "{cdb:02x?} reached the device");
        }
        assert!(!cdb_allowed(&[]));
    }
}
