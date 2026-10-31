// SPDX-License-Identifier: MPL-2.0
//! Host KATs for L7.1's pure logic, `kernel/src/boot_time_logic.rs`, mounted
//! through `#[path]` so these run the exact code the kernel and libneo run.
//!
//! The oracles are independent of the code under test:
//! - an FPDT laid out by hand from ACPI 6.5 §5.2.24 in the shape a laptop's
//!   firmware ships (header, the FBPT pointer record, the S3 pointer record;
//!   0x44 bytes): its FBPT pointer `00 80 92 6b 00 00 00 00` at offset 0x2c
//!   is 0x6b928000. Real-machine tables stay out of this public repo;
//! - an FBPT and a `SYS_BOOT_TIMES` record laid out by hand from ACPI 6.5
//!   §5.2.24 and the record table in the module.

#![allow(dead_code)]

#[path = "../../../kernel/src/boot_time_logic.rs"]
mod boot_time_logic;

#[cfg(test)]
mod tests {
    use super::boot_time_logic::*;

    /// The FPDT oracle: a 36-byte ACPI header (OEM fields generic), the FBPT
    /// pointer record at 0x24 (type 0, len 16, rev 1, reserved, address),
    /// and the S3 pointer record at 0x34 (type 1, len 16, rev 1). The
    /// checksum byte is left 0: `fbpt_address` does not judge it (the kernel
    /// reads the table through its RSDT entry, which the ACPI walk checked).
    #[rustfmt::skip]
    const FPDT_ORACLE: &[u8] = &[
        // signature, length 0x44, revision 1, checksum
        b'F', b'P', b'D', b'T', 0x44, 0x00, 0x00, 0x00, 0x01, 0x00,
        // OEM ID (6), OEM table ID (8)
        b'O', b'E', b'M', b'I', b'D', b' ',
        b'T', b'A', b'B', b'L', b'E', b'I', b'D', b' ',
        // OEM revision, creator ID, creator revision
        0x01, 0x00, 0x00, 0x00, b'C', b'R', b'T', b'R', 0x01, 0x00, 0x00, 0x00,
        // 0x24: FBPT pointer record: type 0, len 16, rev 1, reserved u32
        0x00, 0x00, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00,
        // 0x2c: the FBPT's address, 0x6b928000
        0x00, 0x80, 0x92, 0x6b, 0x00, 0x00, 0x00, 0x00,
        // 0x34: S3 performance table pointer record: type 1, len 16, rev 1
        0x01, 0x00, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x90, 0x92, 0x6b, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn the_fpdt_oracle_points_at_its_fbpt() {
        assert_eq!(FPDT_ORACLE.len(), 0x44);
        assert_eq!(fbpt_address(FPDT_ORACLE, 46), Ok(0x6b92_8000));
        // An address with bits above the CPU's width is refused (firmware
        // has shipped them): 0x6b928000 needs 31 bits.
        assert_eq!(
            fbpt_address(FPDT_ORACLE, 30),
            Err(FpdtError::AddressOutOfRange)
        );
    }

    #[test]
    fn a_damaged_fpdt_is_refused() {
        let mut bad = FPDT_ORACLE.to_vec();
        bad[0] = b'X';
        assert_eq!(fbpt_address(&bad, 46), Err(FpdtError::BadSignature));
        // The header's length past the bytes.
        let mut long = FPDT_ORACLE.to_vec();
        long[4] = 0x45;
        assert_eq!(fbpt_address(&long, 46), Err(FpdtError::BadLength));
        // A record of length 0 (byte 0x26) must fail, not loop.
        let mut zero = FPDT_ORACLE.to_vec();
        zero[0x26] = 0;
        assert_eq!(fbpt_address(&zero, 46), Err(FpdtError::BadRecord));
        // A record running past the table.
        let mut past = FPDT_ORACLE.to_vec();
        past[0x26] = 0x30;
        assert_eq!(fbpt_address(&past, 46), Err(FpdtError::BadRecord));
        // Only the S3 pointer left (the FBPT pointer's type made 1).
        let mut s3_only = FPDT_ORACLE.to_vec();
        s3_only[0x24] = 1;
        assert_eq!(fbpt_address(&s3_only, 46), Err(FpdtError::NoFbptPointer));
    }

    /// An FBPT: header (len 0x50), an unknown record type 0x1000 of 16 bytes
    /// that must be skipped, then the boot record (type 2, len 48, rev 2)
    /// with reset end 1.5 ms, load image 2 s, start image 2.1 s, EBS entry
    /// 4.1 s, EBS exit 4.12 s, then a second boot record that must be ignored.
    #[rustfmt::skip]
    const FBPT: [u8; 0x50] = [
        b'F', b'B', b'P', b'T', 0x50, 0x00, 0x00, 0x00,
        // unknown record: type 0x1000, len 16, rev 1, 12 bytes of payload
        0x00, 0x10, 0x10, 0x01, 0xaa, 0xaa, 0xaa, 0xaa,
        0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        // boot record: type 2, len 48, rev 2, reserved u32
        0x02, 0x00, 0x30, 0x02, 0x00, 0x00, 0x00, 0x00,
        // reset end 1_500_000 ns = 0x16e360
        0x60, 0xe3, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00,
        // load image 2_000_000_000 ns = 0x77359400
        0x00, 0x94, 0x35, 0x77, 0x00, 0x00, 0x00, 0x00,
        // start image 2_100_000_000 ns = 0x7d2b7500
        0x00, 0x75, 0x2b, 0x7d, 0x00, 0x00, 0x00, 0x00,
        // EBS entry 4_100_000_000 ns = 0xf4610900
        0x00, 0x09, 0x61, 0xf4, 0x00, 0x00, 0x00, 0x00,
        // EBS exit 4_120_000_000 ns = 0xf5923600
        0x00, 0x36, 0x92, 0xf5, 0x00, 0x00, 0x00, 0x00,
        // a duplicate boot record (first wins): header only + 4 bytes
        0x02, 0x00, 0x08, 0x02, 0xff, 0xff, 0xff, 0xff,
    ];

    #[test]
    fn the_fbpt_boot_record_is_read() {
        assert_eq!(fbpt_len(&FBPT[..8]), Ok(0x50));
        assert_eq!(
            parse_fbpt(&FBPT),
            Ok(FirmwareBoot {
                reset_end_ns: 1_500_000,
                load_image_ns: 2_000_000_000,
                start_image_ns: 2_100_000_000,
                ebs_entry_ns: 4_100_000_000,
                ebs_exit_ns: 4_120_000_000,
            })
        );
    }

    #[test]
    fn a_damaged_fbpt_is_refused() {
        let mut sig = FBPT;
        sig[1] = b'X';
        assert_eq!(parse_fbpt(&sig), Err(FpdtError::BadSignature));
        // Shorter than its header, and longer than the kernel will read.
        let mut short = FBPT;
        short[4] = 4;
        assert_eq!(parse_fbpt(&short), Err(FpdtError::BadLength));
        let mut huge = FBPT;
        huge[5] = 0x20;
        assert_eq!(fbpt_len(&huge[..8]), Err(FpdtError::BadLength));
        // A zero-length record must fail, not loop.
        let mut zero = FBPT;
        zero[10] = 0;
        assert_eq!(parse_fbpt(&zero), Err(FpdtError::BadRecord));
        // A boot record too short to hold its five times is skipped.
        let mut no_boot = FBPT;
        no_boot[24] = 0x01; // the boot record's type made 1
        assert_eq!(parse_fbpt(&no_boot), Err(FpdtError::NoBootRecord));
    }

    /// The record for: TSC 2.8 GHz (0xa6e49c00), entry TSC 0x1_0000_0000,
    /// desktop TSC 0x2_0000_0000, the FBPT's firmware times, and model-ready
    /// TSC 0x1_8000_0000.
    #[rustfmt::skip]
    const RECORD: [u8; BOOT_TIMES_V1_LEN] = [
        0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // version 1, flags: firmware
        0x00, 0x9c, 0xe4, 0xa6, 0x00, 0x00, 0x00, 0x00, // 2_800_000_000 Hz
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // kernel entry TSC
        0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, // desktop start TSC
        0x60, 0xe3, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, // reset end
        0x00, 0x94, 0x35, 0x77, 0x00, 0x00, 0x00, 0x00, // load image
        0x00, 0x75, 0x2b, 0x7d, 0x00, 0x00, 0x00, 0x00, // start image
        0x00, 0x09, 0x61, 0xf4, 0x00, 0x00, 0x00, 0x00, // EBS entry
        0x00, 0x36, 0x92, 0xf5, 0x00, 0x00, 0x00, 0x00, // EBS exit
        0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, // model ready TSC
    ];

    fn times() -> BootTimes {
        BootTimes {
            tsc_hz: 2_800_000_000,
            tsc_kernel_entry: 0x1_0000_0000,
            tsc_desktop_start: 0x2_0000_0000,
            tsc_model_ready: 0x1_8000_0000,
            firmware: parse_fbpt(&FBPT).ok(),
            rate_estimated: false,
        }
    }

    /// The same firmware times, with TSC stamps after its handoff: entry
    /// 0x4_0000_0000 (6.135 s at 2.8 GHz), model 0x5_0000_0000 (7.669 s),
    /// desktop 0x6_0000_0000 (9.203 s).
    fn after_handoff() -> BootTimes {
        BootTimes {
            tsc_kernel_entry: 0x4_0000_0000,
            tsc_model_ready: 0x5_0000_0000,
            tsc_desktop_start: 0x6_0000_0000,
            ..times()
        }
    }

    #[test]
    fn the_record_matches_its_hand_laid_bytes() {
        assert_eq!(encode(&times()), RECORD);
        assert_eq!(decode(&RECORD), Some(times()));
        // Without the firmware: flag clear, its five fields zero.
        let mut no_fw = times();
        no_fw.firmware = None;
        let bytes = encode(&no_fw);
        assert_eq!(bytes[4], 0);
        assert!(bytes[32..72].iter().all(|&b| b == 0));
        assert_eq!(decode(&bytes), Some(no_fw));
        // An estimated rate: flag bit 1, nothing else changes.
        let mut estimated_bytes = RECORD;
        estimated_bytes[4] = 0x03;
        let estimated = BootTimes {
            rate_estimated: true,
            ..times()
        };
        assert_eq!(encode(&estimated), estimated_bytes);
        assert_eq!(decode(&estimated_bytes), Some(estimated));
    }

    #[test]
    fn a_record_of_another_shape_is_refused() {
        assert_eq!(decode(&RECORD[..79]), None);
        let mut version = RECORD;
        version[0] = 2;
        assert_eq!(decode(&version), None);
        let mut flags = RECORD;
        flags[4] = 4;
        assert_eq!(decode(&flags), None);
    }

    /// Jarvis #254 item 1: `times()`'s kernel entry (1.53 s) is before the
    /// firmware's handoff (4.12 s), so its TSC can't have counted from
    /// power-on. Its lines say "since TSC reset" and leave the firmware out.
    #[test]
    fn a_tsc_before_the_handoff_is_not_from_power_on() {
        assert!(!power_on_origin(&times()));
        let mut line = String::new();
        write_kernel_line(&mut line, &times(), None).unwrap();
        assert_eq!(
            line,
            "firmware handoff 4.12 s, kernel entry 1.53 s, model ready 2.30 s, \
             desktop start 3.06 s (since TSC reset; TSC 2800000000 Hz)"
        );
        // The summary: 56e9 / 2.8e9 = 20 s.
        let mut line = String::new();
        write_summary(&mut line, "Home", &times(), 56_000_000_000).unwrap();
        assert_eq!(
            line,
            "Since TSC reset: model ready 2.30 s, desktop 3.06 s, Home 20.00 s."
        );
        let mut line = String::new();
        write_milestone(&mut line, "Sign-in", &times(), 28_000_000_000).unwrap();
        assert_eq!(line, "Sign-in 10.00 s after TSC reset.");
        // No firmware record at all: no origin either.
        let mut bare = after_handoff();
        bare.firmware = None;
        bare.tsc_desktop_start = 0;
        bare.tsc_model_ready = 0;
        assert!(!power_on_origin(&bare));
        let mut line = String::new();
        write_summary(&mut line, "Arthur ready", &bare, 56_000_000_000).unwrap();
        assert_eq!(line, "Since TSC reset: Arthur ready 20.00 s.");
        let mut line = String::new();
        write_kernel_line(&mut line, &bare, Some(FpdtError::NoFpdt)).unwrap();
        assert!(line.starts_with("firmware handoff unknown (no-fpdt), kernel entry 6.13 s"));
        assert!(
            line.ends_with("(since TSC reset; TSC 2800000000 Hz)"),
            "{line}"
        );
    }

    #[test]
    fn a_tsc_after_the_handoff_is_from_power_on() {
        let on = after_handoff();
        assert!(power_on_origin(&on));
        let mut line = String::new();
        write_kernel_line(&mut line, &on, None).unwrap();
        assert_eq!(
            line,
            "firmware handoff 4.12 s, kernel entry 6.13 s, model ready 7.66 s, \
             desktop start 9.20 s (from power-on; TSC 2800000000 Hz)"
        );
        let mut line = String::new();
        write_summary(&mut line, "Home", &on, 56_000_000_000).unwrap();
        assert_eq!(
            line,
            "From power-on: firmware 4.12 s, model ready 7.66 s, desktop 9.20 s, Home 20.00 s."
        );
        let mut line = String::new();
        write_milestone(&mut line, "Sign-in", &on, 28_000_000_000).unwrap();
        assert_eq!(line, "Sign-in 10.00 s after power-on (firmware 4.12 s).");
        // No rate: the milestone line's fallback, the firmware time alone.
        let no_rate = BootTimes { tsc_hz: 0, ..on };
        let mut line = String::new();
        write_summary(&mut line, "Arthur ready", &no_rate, 5).unwrap();
        assert_eq!(
            line,
            "Arthur ready at TSC 5 (no TSC rate) (firmware 4.12 s)."
        );
    }

    /// Jarvis #254 item 4: a handoff of 0 is not recorded, never `0.00 s`,
    /// and with it the origin is unknown.
    #[test]
    fn an_unrecorded_handoff_is_left_out() {
        let mut zero = after_handoff();
        if let Some(fw) = zero.firmware.as_mut() {
            fw.ebs_exit_ns = 0;
        }
        assert!(!power_on_origin(&zero));
        let mut line = String::new();
        write_kernel_line(&mut line, &zero, None).unwrap();
        assert!(
            line.starts_with("firmware handoff not recorded, kernel entry 6.13 s"),
            "{line}"
        );
        let mut line = String::new();
        write_summary(&mut line, "Home", &zero, 56_000_000_000).unwrap();
        assert_eq!(
            line,
            "Since TSC reset: model ready 7.66 s, desktop 9.20 s, Home 20.00 s."
        );
        assert!(!line.contains("firmware 0.00 s"));
    }

    /// Jarvis #254 item 1: a CPUID-0x16 rate is marked as an estimate.
    #[test]
    fn an_estimated_rate_is_marked() {
        let est = BootTimes {
            rate_estimated: true,
            ..after_handoff()
        };
        let mut line = String::new();
        write_summary(&mut line, "Home", &est, 56_000_000_000).unwrap();
        assert_eq!(
            line,
            "From power-on (estimated clock): firmware 4.12 s, model ready 7.66 s, \
             desktop 9.20 s, Home 20.00 s."
        );
        let mut line = String::new();
        write_kernel_line(&mut line, &est, None).unwrap();
        assert!(
            line.ends_with("(from power-on; TSC 2800000000 Hz, estimated)"),
            "{line}"
        );
        let mut line = String::new();
        write_milestone(&mut line, "Sign-in", &est, 28_000_000_000).unwrap();
        assert_eq!(
            line,
            "Sign-in 10.00 s after power-on (estimated clock) (firmware 4.12 s)."
        );
    }
}
