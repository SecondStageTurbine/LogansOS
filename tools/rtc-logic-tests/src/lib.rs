// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the OT-85 RTC epoch seed's pure logic:
//! `kernel/src/arch/x86_64/rtc_logic.rs` (MC146818 decode, the agreement
//! loop, days-from-civil), `kernel/src/acpi/rtc_fadt_logic.rs` (the FADT's
//! century index and "CMOS RTC Not Present" flag) and
//! `userland/libneo/src/civil.rs` (epoch → civil date/time and its text, the
//! probe's and the Atrium readout's formatter).
//!
//! Both modules are mounted through `#[path]`, so these tests run the exact
//! code the kernel runs. Every expected value is worked out by hand in the
//! comment beside it (day counts from year lengths, register images from the
//! MC146818A data sheet's register map, FADT bytes from ACPI 6.5 Table 5.9),
//! never computed from the code under test.

#![allow(dead_code)]

#[path = "../../../kernel/src/arch/x86_64/rtc_logic.rs"]
mod rtc_logic;

#[path = "../../../kernel/src/acpi/rtc_fadt_logic.rs"]
mod rtc_fadt_logic;

#[path = "../../../userland/libneo/src/civil.rs"]
mod civil;

#[cfg(test)]
mod tests {
    use super::rtc_fadt_logic::{self, RtcFadt};
    use super::rtc_logic::{
        Invalid, Raw, Read, WallTime, base_ns, days_from_civil, decode, field, hour24, read_stable,
        unix_seconds,
    };

    /// QEMU's mc146818rtc at `-rtc base=2026-10-21T17:48:00`: BCD, 24-hour
    /// (register B = 0x02), VRT set (register D = 0x80), century byte 0x20.
    fn qemu_pinned() -> Raw {
        Raw {
            seconds: 0x00,
            minutes: 0x48,
            hours: 0x17,
            day: 0x21,
            month: 0x10,
            year: 0x26,
            century: Some(0x20),
            reg_b: 0x02,
            reg_d: 0x80,
        }
    }

    fn wall(year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> WallTime {
        WallTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    // ── days_from_civil: hand-counted day numbers ──────────────────────────

    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        // 1970 has 365 days.
        assert_eq!(days_from_civil(1971, 1, 1), 365);
    }

    #[test]
    fn y2k_and_its_leap_day() {
        // 1970..1999 = 30 years * 365 + 7 leap days (1972, 76, 80, 84, 88,
        // 92, 96) = 10_957.
        assert_eq!(days_from_civil(2000, 1, 1), 10_957);
        // 2000 is a leap year (divisible by 400): Jan 31 + Feb 28 days in,
        // the 29th exists.
        assert_eq!(days_from_civil(2000, 2, 29), 10_957 + 31 + 28);
        assert_eq!(days_from_civil(2000, 3, 1), 10_957 + 31 + 29);
    }

    #[test]
    fn twenty_one_hundred_is_not_a_leap_year() {
        // 1970..2099 = 130 years * 365 + 32 leap days (1972..2096 step 4,
        // 2000 included) = 47_482.
        assert_eq!(days_from_civil(2100, 1, 1), 47_482);
        assert_eq!(days_from_civil(2100, 2, 28), 47_482 + 31 + 27);
        // 2100 is divisible by 100, not 400: Feb 28 is followed by Mar 1.
        assert_eq!(days_from_civil(2100, 3, 1), 47_482 + 31 + 28);
    }

    #[test]
    fn month_and_year_boundaries() {
        // 1970..2025 = 56 years * 365 + 14 leap days (1972..2024) = 20_454.
        assert_eq!(days_from_civil(2026, 1, 1), 20_454);
        assert_eq!(days_from_civil(2026, 1, 31), 20_454 + 30);
        assert_eq!(days_from_civil(2026, 2, 1), 20_454 + 31);
        assert_eq!(days_from_civil(2026, 12, 31), 20_454 + 364);
        assert_eq!(days_from_civil(2027, 1, 1), 20_454 + 365);
        // 2024 is a leap year: 1970..2023 = 54 * 365 + 13 = 19_723.
        assert_eq!(days_from_civil(2024, 2, 29), 19_723 + 31 + 28);
        assert_eq!(days_from_civil(2024, 3, 1), 19_723 + 31 + 29);
    }

    #[test]
    fn the_pinned_instant() {
        // 2026-10-21: Jan..Sep of a common year = 31+28+31+30+31+30+31+31+30
        // = 273 days, + 20 = 293 days after Jan 1; 20_454 + 293 = 20_747.
        assert_eq!(days_from_civil(2026, 10, 21), 20_747);
        // 20_747 * 86_400 + 17 * 3_600 + 48 * 60 = 1_792_540_800 + 64_080.
        assert_eq!(unix_seconds(&wall(2026, 10, 21, 17, 48, 0)), 1_792_604_880);
        // 1970-01-01 was a Thursday; 20_747 = 7 * 2_963 + 6, Thursday + 6
        // days is a Wednesday (the probe and libneo::civil agree on that).
        assert_eq!(days_from_civil(2026, 10, 21) % 7, 6);
    }

    #[test]
    fn base_subtracts_the_elapsed_monotonic_time() {
        // Read 3 ticks (30 ms) after boot: base = instant - 30 ms.
        assert_eq!(
            base_ns(1_792_604_880, 30_000_000),
            1_792_604_879_970_000_000
        );
        // At tick 0 the base is the instant itself.
        assert_eq!(base_ns(1, 0), 1_000_000_000);
        // Saturates (boot-relative) rather than wrapping.
        assert_eq!(base_ns(0, 5), 0);
        // The last accepted year (2199-12-31T23:59:59) still fits an i64 in ns.
        let last = unix_seconds(&wall(2199, 12, 31, 23, 59, 59));
        assert!(base_ns(last, 0) < i64::MAX as u64);
    }

    // ── decode: register images from the data sheet ────────────────────────

    #[test]
    fn qemu_pinned_bcd_24h_decodes() {
        assert_eq!(decode(&qemu_pinned()), Ok(wall(2026, 10, 21, 17, 48, 0)));
    }

    #[test]
    fn binary_mode_reads_bytes_as_numbers() {
        // Register B = 0x06: DM (bit 2) binary, bit 1 24-hour.
        let raw = Raw {
            seconds: 59,
            minutes: 48,
            hours: 17,
            day: 21,
            month: 10,
            year: 26,
            century: Some(20),
            reg_b: 0x06,
            reg_d: 0x80,
        };
        assert_eq!(decode(&raw), Ok(wall(2026, 10, 21, 17, 48, 59)));
    }

    #[test]
    fn twelve_hour_mode_uses_bit_seven_for_pm() {
        // BCD 12-hour: register B = 0x00. 5 PM = 0x05 | 0x80.
        assert_eq!(hour24(0x85, false, false), Ok(17));
        assert_eq!(hour24(0x05, false, false), Ok(5));
        // 12 AM is midnight (0), 12 PM is noon (12).
        assert_eq!(hour24(0x12, false, false), Ok(0));
        assert_eq!(hour24(0x92, false, false), Ok(12));
        // Binary 12-hour: 11 PM = 11 | 0x80 = 0x8B.
        assert_eq!(hour24(0x8B, true, false), Ok(23));
        assert_eq!(hour24(12, true, false), Ok(0));
        // 0 and 13 do not exist on a 12-hour clock.
        assert_eq!(hour24(0x00, false, false), Err(Invalid::OutOfRange));
        assert_eq!(hour24(0x13, false, false), Err(Invalid::OutOfRange));
        let mut raw = qemu_pinned();
        raw.reg_b = 0x00;
        raw.hours = 0x85;
        assert_eq!(decode(&raw), Ok(wall(2026, 10, 21, 17, 48, 0)));
    }

    #[test]
    fn bcd_digits_are_checked() {
        assert_eq!(field(0x59, false), Some(59));
        assert_eq!(field(0x1A, false), None);
        assert_eq!(field(0xA1, false), None);
        assert_eq!(field(0x1A, true), Some(26));
        let mut raw = qemu_pinned();
        raw.minutes = 0x4F;
        assert_eq!(decode(&raw), Err(Invalid::BadBcd));
    }

    #[test]
    fn no_century_register_means_twenty_hundred() {
        let mut raw = qemu_pinned();
        raw.century = None;
        assert_eq!(decode(&raw), Ok(wall(2026, 10, 21, 17, 48, 0)));
        // A 1999 clock with its century byte reads 1999.
        raw.century = Some(0x19);
        raw.year = 0x99;
        assert_eq!(decode(&raw).map(|w| w.year), Ok(1999));
        // 1969 has no Unix-epoch base: refused, not wrapped.
        raw.year = 0x69;
        assert_eq!(decode(&raw), Err(Invalid::YearOutOfRange));
        // 2199 is the last year; 21 is the last century byte used.
        raw.century = Some(0x21);
        raw.year = 0x99;
        assert_eq!(decode(&raw).map(|w| w.year), Ok(2199));
    }

    #[test]
    fn an_unmaintained_century_byte_is_ignored() {
        // Field trap: the FADT names a century byte the firmware never
        // writes. 0x00 (year 0026), 0x22 (2226, past the i64-ns wall), a
        // stale 0x99, a non-BCD 0x2A: each ignored, the year is 2000 + 26.
        for c in [0x00, 0x22, 0x99, 0x2A, 0xFF] {
            let mut raw = qemu_pinned();
            raw.century = Some(c);
            assert_eq!(
                decode(&raw),
                Ok(wall(2026, 10, 21, 17, 48, 0)),
                "century byte {c:#04x}"
            );
        }
    }

    #[test]
    fn garbage_is_refused() {
        // A floating bus: every register 0xFF (binary + 24-hour by register
        // B's bits, VRT set): the hour 255 is out of range.
        let ff = Raw {
            seconds: 0xFF,
            minutes: 0xFF,
            hours: 0xFF,
            day: 0xFF,
            month: 0xFF,
            year: 0xFF,
            century: Some(0xFF),
            reg_b: 0xFF,
            reg_d: 0xFF,
        };
        assert_eq!(decode(&ff), Err(Invalid::OutOfRange));
        // All zero: VRT clear, the clock lost power.
        assert_eq!(decode(&Raw::default()), Err(Invalid::PowerLost));
        // VRT set but the date is zero: day 0 / month 0 do not exist.
        let zero = Raw {
            reg_d: 0x80,
            ..Raw::default()
        };
        assert_eq!(decode(&zero), Err(Invalid::OutOfRange));
    }

    #[test]
    fn dates_that_do_not_exist_are_refused() {
        let mut raw = qemu_pinned();
        // 2026-02-29 (2026 is common).
        raw.month = 0x02;
        raw.day = 0x29;
        assert_eq!(decode(&raw), Err(Invalid::OutOfRange));
        // 2100-02-29 (not leap).
        raw.century = Some(0x21);
        raw.year = 0x00;
        assert_eq!(decode(&raw), Err(Invalid::OutOfRange));
        // 2000-02-29 (leap).
        raw.century = Some(0x20);
        raw.year = 0x00;
        assert_eq!(decode(&raw).map(|w| (w.month, w.day)), Ok((2, 29)));
        // April 31, month 13, second 60, minute 60.
        let mut raw = qemu_pinned();
        raw.month = 0x04;
        raw.day = 0x31;
        assert_eq!(decode(&raw), Err(Invalid::OutOfRange));
        let mut raw = qemu_pinned();
        raw.month = 0x13;
        assert_eq!(decode(&raw), Err(Invalid::OutOfRange));
        let mut raw = qemu_pinned();
        raw.seconds = 0x60;
        assert_eq!(decode(&raw), Err(Invalid::OutOfRange));
        let mut raw = qemu_pinned();
        raw.minutes = 0x60;
        assert_eq!(decode(&raw), Err(Invalid::OutOfRange));
    }

    // ── read_stable: the double-read agreement loop ────────────────────────

    fn reader(seq: Vec<Option<Raw>>) -> impl FnMut() -> Option<Raw> {
        let mut it = seq.into_iter();
        move || it.next().flatten()
    }

    #[test]
    fn two_equal_reads_are_stable() {
        let a = qemu_pinned();
        assert_eq!(
            read_stable(reader(vec![Some(a), Some(a)]), 8),
            Read::Stable { raw: a, reads: 2 }
        );
    }

    #[test]
    fn an_update_between_reads_takes_another_pair() {
        // The second ticks over between the first and second snapshots: the
        // pair disagrees, the next one agrees on the new second.
        let a = qemu_pinned();
        let b = Raw { seconds: 0x01, ..a };
        assert_eq!(
            read_stable(reader(vec![Some(a), Some(b), Some(b)]), 8),
            Read::Stable { raw: b, reads: 3 }
        );
    }

    #[test]
    fn never_agreeing_reads_are_unstable() {
        let mut seq = Vec::new();
        for s in 0..8u8 {
            seq.push(Some(Raw {
                seconds: s,
                ..qemu_pinned()
            }));
        }
        assert_eq!(read_stable(reader(seq), 8), Read::Unstable);
        // The bound is exact: the 8th read is the last one taken.
        let mut calls = 0;
        let _ = read_stable(
            || {
                calls += 1;
                Some(Raw {
                    seconds: calls as u8,
                    ..qemu_pinned()
                })
            },
            8,
        );
        assert_eq!(calls, 8);
    }

    #[test]
    fn a_stuck_update_cycle_is_reported() {
        assert_eq!(read_stable(reader(vec![None]), 8), Read::Stuck);
        assert_eq!(
            read_stable(reader(vec![Some(qemu_pinned()), None]), 8),
            Read::Stuck
        );
    }

    // ── the FADT: hand-laid table bytes ────────────────────────────────────

    /// A 244-byte (ACPI 2.0+) FADT: "FACP", length 244 (0xF4), revision 3,
    /// `CENTURY` at 108, `IAPC_BOOT_ARCH` at 109..111, flags at 112..116.
    fn fadt(revision: u8, century: u8, boot_arch: [u8; 2], flags: [u8; 4]) -> Vec<u8> {
        let mut t = vec![0u8; 244];
        t[0..4].copy_from_slice(b"FACP");
        t[4..8].copy_from_slice(&[0xF4, 0x00, 0x00, 0x00]);
        t[8] = revision;
        t[108] = century;
        t[109..111].copy_from_slice(&boot_arch);
        t[112..116].copy_from_slice(&flags);
        t
    }

    #[test]
    fn fadt_century_and_rtc_present() {
        // QEMU q35: century 0x32, IAPC_BOOT_ARCH 0x0002 (8042 only).
        let t = fadt(3, 0x32, [0x02, 0x00], [0, 0, 0, 0]);
        assert_eq!(
            rtc_fadt_logic::parse(&t),
            Some(RtcFadt {
                century_index: Some(0x32),
                rtc_not_present: false,
            })
        );
    }

    #[test]
    fn fadt_cmos_rtc_not_present_bit_five() {
        // IAPC_BOOT_ARCH 0x0020 = bit 5; hardware-reduced (flags bit 20 =
        // byte 114 bit 4 = 0x10): century ignored.
        let t = fadt(6, 0x32, [0x20, 0x00], [0x00, 0x00, 0x10, 0x00]);
        assert_eq!(
            rtc_fadt_logic::parse(&t),
            Some(RtcFadt {
                century_index: None,
                rtc_not_present: true,
            })
        );
        // Bit 5 alone, not reduced.
        let t = fadt(6, 0x00, [0x20, 0x00], [0, 0, 0, 0]);
        assert_eq!(
            rtc_fadt_logic::parse(&t),
            Some(RtcFadt {
                century_index: None,
                rtc_not_present: true,
            })
        );
    }

    #[test]
    fn fadt_revision_one_has_no_boot_arch() {
        // ACPI 1.0 (revision 1): byte 109 is reserved; 0x20 there means nothing.
        let mut t = fadt(1, 0x32, [0x20, 0x00], [0, 0, 0, 0]);
        t.truncate(116);
        assert_eq!(
            rtc_fadt_logic::parse(&t),
            Some(RtcFadt {
                century_index: Some(0x32),
                rtc_not_present: false,
            })
        );
    }

    #[test]
    fn not_a_fadt_or_too_short() {
        let mut t = fadt(3, 0x32, [0, 0], [0, 0, 0, 0]);
        t[0..4].copy_from_slice(b"APIC");
        assert_eq!(rtc_fadt_logic::parse(&t), None);
        let t = fadt(3, 0x32, [0, 0], [0, 0, 0, 0]);
        assert_eq!(rtc_fadt_logic::parse(&t[..108]), None);
        // Long enough for CENTURY only: no boot-arch, no flags.
        assert_eq!(
            rtc_fadt_logic::parse(&t[..109]),
            Some(RtcFadt {
                century_index: Some(0x32),
                rtc_not_present: false,
            })
        );
    }

    // ── libneo::civil: epoch → date, weekday, 12-hour time, text ──────────

    use super::civil::{self, Civil};

    fn text(c: &Civil) -> (String, String) {
        (
            c.date_text().as_str().to_string(),
            c.time_text().as_str().to_string(),
        )
    }

    #[test]
    fn civil_epoch_itself() {
        // 1970-01-01T00:00:00, a Thursday (4), 12:00 AM.
        let c = civil::from_unix_seconds(0);
        assert_eq!(
            c,
            Civil {
                year: 1970,
                month: 1,
                day: 1,
                weekday: 4,
                hour: 0,
                minute: 0,
                second: 0,
            }
        );
        assert_eq!(
            text(&c),
            ("Thursday, January 1, 1970".into(), "12:00 AM".into())
        );
        // The last second of the first day.
        let c = civil::from_unix_seconds(86_399);
        assert_eq!((c.day, c.hour, c.minute, c.second), (1, 23, 59, 59));
        assert_eq!(c.time_text().as_str(), "11:59 PM");
    }

    #[test]
    fn civil_pinned_instant_is_wednesday_5_48_pm() {
        // 1_792_604_880 = 2026-10-21T17:48:00 (worked out in the_pinned_instant).
        let c = civil::from_realtime_ns(1_792_604_880 * civil::NS_PER_SECOND);
        assert_eq!((c.year, c.month, c.day), (2026, 10, 21));
        assert_eq!(c.weekday, 3, "Wednesday");
        assert_eq!(
            text(&c),
            ("Wednesday, October 21, 2026".into(), "5:48 PM".into())
        );
        // 59.99 s later it is still 5:48; one ns more and it is 5:49.
        let edge = 1_792_604_880 * civil::NS_PER_SECOND + 59 * civil::NS_PER_SECOND + 999_999_999;
        assert_eq!(
            civil::from_realtime_ns(edge).time_text().as_str(),
            "5:48 PM"
        );
        assert_eq!(
            civil::from_realtime_ns(edge + 1).time_text().as_str(),
            "5:49 PM"
        );
        assert!(c.same_minute(&civil::from_realtime_ns(edge)));
        assert!(!c.same_minute(&civil::from_realtime_ns(edge + 1)));
    }

    #[test]
    fn civil_leap_days_and_century_years() {
        // 2000-02-29 = day 11_016 (y2k_and_its_leap_day); a Tuesday:
        // 11_016 = 7 * 1_573 + 5, Thursday + 5 = Tuesday (2).
        let c = civil::from_unix_seconds(11_016 * 86_400);
        assert_eq!((c.year, c.month, c.day, c.weekday), (2000, 2, 29, 2));
        assert_eq!(c.date_text().as_str(), "Tuesday, February 29, 2000");
        let c = civil::from_unix_seconds(11_017 * 86_400);
        assert_eq!((c.year, c.month, c.day), (2000, 3, 1));
        // 2100-02-28 = day 47_540, and the next day is March 1 (no leap day).
        let c = civil::from_unix_seconds(47_540 * 86_400);
        assert_eq!((c.year, c.month, c.day), (2100, 2, 28));
        let c = civil::from_unix_seconds(47_541 * 86_400);
        assert_eq!((c.year, c.month, c.day), (2100, 3, 1));
        // 2024-02-29 = day 19_782.
        let c = civil::from_unix_seconds(19_782 * 86_400);
        assert_eq!((c.year, c.month, c.day), (2024, 2, 29));
    }

    #[test]
    fn civil_month_and_year_boundaries() {
        // 2026-01-31 (20_484) -> 2026-02-01 (20_485); 2026-12-31 (20_818) ->
        // 2027-01-01 (20_819).
        for (days, ymd) in [
            (20_484, (2026, 1, 31)),
            (20_485, (2026, 2, 1)),
            (20_818, (2026, 12, 31)),
            (20_819, (2027, 1, 1)),
            (20_454 + 31 + 28 + 31 + 29, (2026, 4, 30)),
            (20_454 + 31 + 28 + 31 + 30, (2026, 5, 1)),
        ] {
            let c = civil::from_unix_seconds(days * 86_400);
            assert_eq!((c.year, c.month, c.day), ymd, "day {days}");
        }
        assert_eq!(
            civil::from_unix_seconds(20_819 * 86_400)
                .date_text()
                .as_str(),
            "Friday, January 1, 2027"
        );
    }

    #[test]
    fn civil_twelve_hour_clock() {
        let at = |h: u64, m: u64| {
            civil::from_unix_seconds(20_747 * 86_400 + h * 3_600 + m * 60)
                .time_text()
                .as_str()
                .to_string()
        };
        assert_eq!(at(0, 0), "12:00 AM");
        assert_eq!(at(0, 5), "12:05 AM");
        assert_eq!(at(9, 7), "9:07 AM");
        assert_eq!(at(11, 59), "11:59 AM");
        assert_eq!(at(12, 0), "12:00 PM");
        assert_eq!(at(13, 1), "1:01 PM");
        assert_eq!(at(23, 59), "11:59 PM");
    }

    #[test]
    fn civil_longest_texts_fit_unclipped() {
        // Wednesday + September: the longest date (29 bytes).
        // 2026-09-30 = 20_454 + 272 = 20_726; 20_726 = 7 * 2_960 + 6: Wednesday.
        let c = civil::from_unix_seconds(20_726 * 86_400 + 12 * 3_600 + 59 * 60);
        assert_eq!(c.date_text().as_str(), "Wednesday, September 30, 2026");
        assert_eq!(c.date_text().as_str().len(), 29);
        assert_eq!(c.time_text().as_str(), "12:59 PM");
    }

    /// Two independent implementations agree: the kernel's days-from-civil
    /// (the seed) and libneo's civil-from-days (the display) invert each
    /// other on every day 1970..=2199.
    #[test]
    fn kernel_and_libneo_calendars_round_trip() {
        let last = days_from_civil(2199, 12, 31);
        for days in 0..=last {
            let (y, m, d) = civil::civil_from_days(days);
            assert_eq!(days_from_civil(y, u32::from(m), u32::from(d)), days);
        }
        // And the kernel's decode of every RTC date maps back to itself.
        let mut days = 0;
        for y in 1970..=2199u32 {
            for m in 1..=12u32 {
                for d in 1..=super::rtc_logic::days_in_month(y, m) {
                    assert_eq!(days_from_civil(y, m, d), days);
                    days += 1;
                }
            }
        }
        assert_eq!(days, last + 1);
    }
}
