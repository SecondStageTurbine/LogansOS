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

    /// Review #16 item 1: the whole FBPT extent is inside the CPU's physical
    /// width, not just its first byte. Janus's input: an FBPT pointer of
    /// 0xf_ffff_fffc with a 36-bit width (its 8-byte header would end past
    /// 0x10_0000_0000) is refused; so is one whose largest table would.
    #[test]
    fn the_whole_fbpt_extent_is_inside_the_physical_width() {
        let with = |addr: u64| {
            let mut t = FPDT_ORACLE.to_vec();
            t[0x2c..0x34].copy_from_slice(&addr.to_le_bytes());
            t
        };
        assert_eq!(
            fbpt_address(&with(0x0000_000f_ffff_fffc), 36),
            Err(FpdtError::AddressOutOfRange)
        );
        let last_ok = (1u64 << 36) - FBPT_MAX_LEN as u64;
        assert_eq!(fbpt_address(&with(last_ok), 36), Ok(last_ok));
        assert_eq!(
            fbpt_address(&with(last_ok + 1), 36),
            Err(FpdtError::AddressOutOfRange)
        );
        // An address that overflows when the extent is added.
        assert_eq!(
            fbpt_address(&with(u64::MAX - 2), 64),
            Err(FpdtError::AddressOutOfRange)
        );
    }

    /// Review #16 item 1: a firmware pointer is read only inside one
    /// firmware-owned memory-map region, wholly.
    #[test]
    fn a_firmware_range_must_lie_in_one_firmware_region() {
        let regions = [(0x6b92_0000u64, 0x1_0000u64), (0xe_0000, 0x2_0000)];
        assert!(in_firmware_memory(0x6b92_8000, 8, &regions));
        assert!(in_firmware_memory(0x6b92_f000, 0x1000, &regions)); // ends at the top
        assert!(!in_firmware_memory(0x6b92_f001, 0x1000, &regions)); // one past
        assert!(!in_firmware_memory(0x6b91_fffc, 8, &regions)); // starts below
        assert!(!in_firmware_memory(0x1000_0000, 8, &regions)); // a hole
        assert!(!in_firmware_memory(u64::MAX - 3, 8, &regions)); // wraps
        assert!(!in_firmware_memory(0x6b92_8000, 8, &[])); // no map
        assert!(!in_firmware_memory(0, 8, &[(u64::MAX, 2)])); // a region that wraps
    }

    /// Review #16 item 4: the conversion is checked, never truncated, and a
    /// rate below 1 MHz is no rate.
    #[test]
    fn an_unrepresentable_time_is_unavailable() {
        // Janus's input: 2^63 ticks at 500 Hz is 2^64 ms (it was Some(0)).
        assert_eq!(tsc_ms(1u64 << 63, 500), None);
        // At a real rate the largest reading still fits.
        assert_eq!(tsc_ms(u64::MAX, MIN_TSC_HZ), Some(u64::MAX / 1000));
        assert_eq!(tsc_ms(MIN_TSC_HZ, MIN_TSC_HZ), Some(1000));
        assert_eq!(tsc_ms(1_000_000, MIN_TSC_HZ - 1), None);
        assert_eq!(tsc_ms(5, 0), None);
        // A slow, misread rate says so instead of a time.
        let slow = BootTimes {
            tsc_hz: 500,
            tsc_kernel_entry: 1 << 63,
            tsc_desktop_start: (1 << 63) + 1,
            ..Default::default()
        };
        assert!(!power_on_origin(&slow));
        let mut line = String::new();
        write_kernel_line(&mut line, &slow, Some(FpdtError::NoFpdt)).unwrap();
        assert!(line.contains("TSC time unavailable at 500 Hz"), "{line}");
        let mut line = String::new();
        write_milestone(&mut line, "Home", &slow, 7).unwrap();
        assert_eq!(line, "Home at TSC 7 (TSC time unavailable at 500 Hz).");
    }

    /// Firmware after the kernel's entry (the TSC was reset after the
    /// handoff): no negative interval, the origin is the TSC reset.
    #[test]
    fn firmware_after_entry_is_not_power_on() {
        let mut t = after_handoff();
        t.tsc_kernel_entry = 1; // ~0 ms, before the 4.12 s handoff
        assert!(!power_on_origin(&t));
        let mut line = String::new();
        write_summary(&mut line, "Home", &t, 28_000_000_000).unwrap();
        assert!(line.starts_with("Since TSC reset: "), "{line}");
        assert!(!line.contains("firmware"), "{line}");
    }

    /// Janus's #16 re-check: Limine's own map calls MMIO "reserved", so the
    /// firmware-RAM regions come from the raw UEFI map. Three hand-laid
    /// 48-byte `EFI_MEMORY_DESCRIPTOR`s (UEFI 2.10 §7.2; type u32 at 0,
    /// physical start u64 at 8, virtual start at 16, page count at 24,
    /// attribute at 32, then 8 bytes of a newer descriptor's tail):
    /// - EfiReservedMemoryType (0) at 0x6b92_0000, 16 pages, write-back
    ///   cacheable (attribute 0xF): the FBPT's home;
    /// - EfiMemoryMappedIO (11) at 0xfee0_0000, 1 page: Janus's input;
    /// - EfiACPIMemoryNVS (10) at 0x7f00_0000, 2 pages.
    #[rustfmt::skip]
    const EFI_MAP: [u8; 144] = [
        // reserved, 0x6b920000, 16 pages, attribute UC|WC|WT|WB (0xF)
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x92, 0x6b,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x0f, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        // MMIO, 0xfee00000, 1 page
        0x0b, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0xe0, 0xfe,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x80,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        // ACPI NVS, 0x7f000000, 2 pages
        0x0a, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x7f,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,  0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn mmio_is_never_firmware_ram() {
        let mut out = [(0u64, 0u64); 8];
        let n = efi_firmware_regions(&EFI_MAP, 48, &mut out);
        assert_eq!(&out[..n], &[(0x6b92_0000, 0x1_0000), (0x7f00_0000, 0x2000)]);
        // Janus's input: an FBPT pointer into the LAPIC's MMIO page.
        assert!(!in_firmware_memory(0xfee0_0000, 8, &out[..n]));
        // The real FBPT home and an NVS table are readable.
        assert!(in_firmware_memory(0x6b92_8000, 8, &out[..n]));
        assert!(in_firmware_memory(0x7f00_1000, 0x1000, &out[..n]));
        // A descriptor size too small for the layout yields nothing; so does
        // a BIOS boot (no map).
        assert_eq!(efi_firmware_regions(&EFI_MAP, 32, &mut out), 0);
        assert_eq!(efi_firmware_regions(&[], 48, &mut out), 0);
        // Unusable memory (8) and boot-services data (4) are not firmware RAM.
        let mut map = EFI_MAP;
        map[0] = 8;
        map[96] = 4;
        assert_eq!(efi_firmware_regions(&map, 48, &mut out), 0);
        // A full `out` stops the walk (later ranges then refused).
        let mut one = [(0u64, 0u64); 1];
        assert_eq!(efi_firmware_regions(&EFI_MAP, 48, &mut one), 1);
        // A page count whose length overflows is skipped.
        let mut map = EFI_MAP;
        map[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
        let n = efi_firmware_regions(&map, 48, &mut out);
        assert_eq!(&out[..n], &[(0x7f00_0000, 0x2000)]);
    }

    /// Janus's second #16 re-check: a reserved (type 0) descriptor is also
    /// how firmware describes hardware the OS must not touch. His input:
    /// type 0 at 0xfee00000, one page, attribute 0. Reserved and runtime
    /// data count as firmware RAM only when write-back cacheable.
    #[test]
    fn reserved_hardware_is_not_firmware_ram() {
        let mut map = [0u8; 48];
        map[8..16].copy_from_slice(&0xfee0_0000u64.to_le_bytes());
        map[24..32].copy_from_slice(&1u64.to_le_bytes());
        let mut out = [(0u64, 0u64); 4];
        assert_eq!(efi_firmware_regions(&map, 48, &mut out), 0);
        assert!(!in_firmware_memory(0xfee0_0000, 8, &out[..0]));
        // Uncacheable only (a typical MMIO attribute, with RUNTIME): refused.
        map[32..40].copy_from_slice(&0x8000_0000_0000_0001u64.to_le_bytes());
        assert_eq!(efi_firmware_regions(&map, 48, &mut out), 0);
        // Write-back cacheable: RAM the firmware reserved.
        map[32..40].copy_from_slice(&0x0fu64.to_le_bytes());
        assert_eq!(efi_firmware_regions(&map, 48, &mut out), 1);
        // The rule itself: ACPI reclaim/NVS always; 0 and 6 only with WB;
        // nothing else, whatever the attribute.
        assert!(efi_is_firmware_ram(9, 0) && efi_is_firmware_ram(10, 0));
        assert!(!efi_is_firmware_ram(0, 0x1) && efi_is_firmware_ram(0, 0x8));
        assert!(!efi_is_firmware_ram(6, 0) && efi_is_firmware_ram(6, 0xf));
        for kind in [1, 2, 3, 4, 5, 7, 8, 11, 12, 13] {
            assert!(!efi_is_firmware_ram(kind, 0xf), "type {kind}");
        }
    }

    /// Janus's second #16 re-check: the kernel cast the `Response` wrapper,
    /// so it read (revision, map, size) as (map, size, descriptor size).
    /// Built here as the real limine 0.6.3 type, from its five words:
    /// revision 0, map 0xffff_8000_0010_0000, size 4800, descriptor size 48,
    /// version 1.
    #[test]
    fn the_memmap_fields_come_from_the_response_data() {
        let words: [u64; 5] = [0, 0xffff_8000_0010_0000, 4800, 48, 1];
        // SAFETY: `Response<T>` is `repr(C)` { revision: u64, data: T } and
        // `EfiMemmapRespData` is four u64s: five words in all.
        let resp: &limine::request::Response<limine::request::EfiMemmapRespData> =
            unsafe { &*(words.as_ptr() as *const _) };
        let data: &limine::request::EfiMemmapRespData = resp;
        assert_eq!(data.desc_size, 48);
        // SAFETY: `data` is the response data, four readable u64s.
        let fields = unsafe { efi_memmap_fields(data) };
        assert_eq!(fields, (0xffff_8000_0010_0000, 4800, 48));
        // The old cast (the wrapper itself) is off by one word.
        // SAFETY: the wrapper is five readable u64s.
        assert_eq!(
            unsafe { efi_memmap_fields(resp) },
            (0, 0xffff_8000_0010_0000, 4800)
        );
    }

    #[test]
    fn the_memmap_address_lands_in_the_hhdm() {
        let hhdm = 0xffff_8000_0000_0000;
        // The protocol's HHDM address is used as given.
        assert_eq!(
            efi_memmap_virt(0xffff_8000_0010_0000, hhdm),
            Some(0xffff_8000_0010_0000)
        );
        // A physical one (the crate's base-revision-3 note) is moved in.
        assert_eq!(
            efi_memmap_virt(0x10_0000, hhdm),
            Some(0xffff_8000_0010_0000)
        );
        assert_eq!(efi_memmap_virt(0x10_0000, u64::MAX), None);
    }
}
