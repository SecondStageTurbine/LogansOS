// SPDX-License-Identifier: MPL-2.0
//! Pure logic of the boot-time RTC epoch seed (OT-85, feature `rtc-epoch`):
//! decoding the MC146818 register file, the read protocol's agreement loop,
//! and the civil-date-to-Unix arithmetic. Shared with the host KATs in
//! `tools/rtc-logic-tests` (mounted by `#[path]`), so nothing here touches a
//! port; `rtc.rs` does the I/O.
//!
//! Source, read for principle (no code taken): the Motorola MC146818A data
//! sheet ("Address Map", "Registers A-D", "Update Cycle"), as every PC CMOS
//! clock since the AT implements it:
//! - time/date registers 0x00 seconds, 0x02 minutes, 0x04 hours, 0x07 day of
//!   month, 0x08 month, 0x09 year (two digits). The day-of-week register 0x06
//!   is not read: firmware leaves it wrong often enough, and the weekday is
//!   computed from the date anyway.
//! - register A bit 7 UIP: an update is in progress (or begins within 244 us);
//!   the time registers are unreliable while it is set.
//! - register B bit 2 DM: 1 = binary, 0 = BCD; bit 1 24/12: 1 = 24-hour.
//!   In 12-hour mode the hour is 1..12 with bit 7 set for PM, in both BCD and
//!   binary data modes.
//! - register D bit 7 VRT: valid RAM and time; 0 means the clock lost power
//!   and its contents are not to be trusted.
//! - the century is not in the chip's time registers; ACPI names the CMOS
//!   index of a century byte in the FADT (`CENTURY`, 0 = none), and PCs put
//!   it at 0x32. Without one, or with one that does not read 19..=21, the
//!   year is 2000 + yy.
//!
//! **The RTC is local time (OT-85 decision).** The target dual-boots Windows,
//! which keeps the RTC in local wall time. We read it the same way and store
//! "the wall time the RTC reported" as the epoch base; there is no time zone
//! or DST rule anywhere. A `CLOCK_REALTIME` value is therefore local wall time
//! counted as if it were UTC seconds since 1970, and every consumer formats it
//! with no offset.

/// CMOS register indices.
pub const REG_SECONDS: u8 = 0x00;
pub const REG_MINUTES: u8 = 0x02;
pub const REG_HOURS: u8 = 0x04;
pub const REG_DAY: u8 = 0x07;
pub const REG_MONTH: u8 = 0x08;
pub const REG_YEAR: u8 = 0x09;
pub const REG_A: u8 = 0x0A;
pub const REG_B: u8 = 0x0B;
pub const REG_D: u8 = 0x0D;

pub const A_UIP: u8 = 1 << 7;
pub const B_BINARY: u8 = 1 << 2;
pub const B_24H: u8 = 1 << 1;
pub const D_VRT: u8 = 1 << 7;
/// 12-hour mode: the PM flag in the hours register.
pub const HOUR_PM: u8 = 1 << 7;

/// The last year accepted (see [`Invalid::YearOutOfRange`]).
pub const MAX_YEAR: u32 = 2199;

/// Snapshots read before the agreement loop gives up.
pub const MAX_READS: u32 = 8;

/// One read of the register file, raw (as the chip reports it). `century` is
/// `None` when there is no century register to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Raw {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day: u8,
    pub month: u8,
    pub year: u8,
    pub century: Option<u8>,
    pub reg_b: u8,
    pub reg_d: u8,
}

/// The wall time the RTC reports, decoded and range-checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Why a seed was refused (each a named serial reason).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// Register D's VRT bit is clear: the clock lost power.
    PowerLost,
    /// A field is not a valid BCD byte in BCD mode.
    BadBcd,
    /// A field is out of range (or the date does not exist).
    OutOfRange,
    /// The year is before 1970 (no Unix-epoch base) or past 2199 (realtime
    /// in ns must stay a positive `i64`, the syscall's return; 2262 is the
    /// wall, and no PC century byte reads 22).
    YearOutOfRange,
}

/// The agreement loop's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read {
    /// Two consecutive snapshots agreed; `reads` were taken in all.
    Stable { raw: Raw, reads: u32 },
    /// No two consecutive snapshots agreed within [`MAX_READS`].
    Unstable,
    /// The reader could not get a snapshot (UIP never cleared).
    Stuck,
}

/// Read until two consecutive snapshots are identical (the update cycle can
/// land between the reads of one snapshot; two equal ones bracket a moment
/// with no update in it). `read` returns `None` when UIP never clears.
pub fn read_stable(mut read: impl FnMut() -> Option<Raw>, max_reads: u32) -> Read {
    let Some(mut last) = read() else {
        return Read::Stuck;
    };
    let mut reads = 1;
    while reads < max_reads {
        let Some(next) = read() else {
            return Read::Stuck;
        };
        reads += 1;
        if next == last {
            return Read::Stable { raw: next, reads };
        }
        last = next;
    }
    Read::Unstable
}

/// A field in the chip's data mode: binary as is, BCD checked digit by digit.
pub fn field(v: u8, binary: bool) -> Option<u32> {
    if binary {
        return Some(u32::from(v));
    }
    let (hi, lo) = (v >> 4, v & 0x0F);
    if hi > 9 || lo > 9 {
        return None;
    }
    Some(u32::from(hi) * 10 + u32::from(lo))
}

/// The hour on a 24-hour clock. 12-hour mode: 1..12 in the low bits, the PM
/// flag in bit 7; 12 AM is 0, 12 PM is 12.
pub fn hour24(v: u8, binary: bool, h24: bool) -> Result<u32, Invalid> {
    if h24 {
        let h = field(v, binary).ok_or(Invalid::BadBcd)?;
        return if h < 24 {
            Ok(h)
        } else {
            Err(Invalid::OutOfRange)
        };
    }
    let pm = v & HOUR_PM != 0;
    let h = field(v & !HOUR_PM, binary).ok_or(Invalid::BadBcd)?;
    if !(1..=12).contains(&h) {
        return Err(Invalid::OutOfRange);
    }
    Ok(h % 12 + if pm { 12 } else { 0 })
}

pub const fn is_leap(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

pub const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Decode and check a snapshot. The century register, when present and
/// reading 19..=21, holds the century in the chip's data mode; otherwise the
/// year is 2000 + yy.
pub fn decode(raw: &Raw) -> Result<WallTime, Invalid> {
    if raw.reg_d & D_VRT == 0 {
        return Err(Invalid::PowerLost);
    }
    let binary = raw.reg_b & B_BINARY != 0;
    let h24 = raw.reg_b & B_24H != 0;
    let f = |v| field(v, binary).ok_or(Invalid::BadBcd);
    let second = f(raw.seconds)?;
    let minute = f(raw.minutes)?;
    let hour = hour24(raw.hours, binary, h24)?;
    let day = f(raw.day)?;
    let month = f(raw.month)?;
    let yy = f(raw.year)?;
    // A century byte is used only when it reads 19, 20 or 21 (the years this
    // seed accepts). Firmware that names one in the FADT but never maintains
    // it (a stale 0x00 or 0x19, or a non-BCD byte) is a known field trap; it
    // must not cost a good clock, so it is ignored and the year is 2000 + yy.
    let century = raw
        .century
        .and_then(|c| field(c, binary))
        .filter(|c| (19..=21).contains(c))
        .unwrap_or(20);
    if second > 59 || minute > 59 || yy > 99 {
        return Err(Invalid::OutOfRange);
    }
    let year = century * 100 + yy;
    if !(1970..=MAX_YEAR).contains(&year) {
        return Err(Invalid::YearOutOfRange);
    }
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return Err(Invalid::OutOfRange);
    }
    Ok(WallTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

/// Days from 1970-01-01 to `year-month-day` in the proleptic Gregorian
/// calendar, for `year >= 1970` (no negative path is needed: [`decode`]
/// refuses earlier years). The year is shifted to start in March so the leap
/// day is the last day of the shifted year; then the day count is the
/// 400-year eras, the years since the era began (with their leap days), and
/// the day of the shifted year (153-day five-month cycles).
pub const fn days_from_civil(year: u32, month: u32, day: u32) -> u64 {
    let y = (if month <= 2 { year - 1 } else { year }) as u64;
    let era = y / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = ((month + 9) % 12) as u64; // March = 0 .. February = 11
    let doy = (153 * mp + 2) / 5 + day as u64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    // 719_468 = days from 0000-03-01 to 1970-01-01.
    era * 146_097 + doe - 719_468
}

/// Seconds since 1970-01-01T00:00:00 of `t`, read as UTC (OT-85: the wall
/// time the RTC reported, no offset).
pub const fn unix_seconds(t: &WallTime) -> u64 {
    days_from_civil(t.year, t.month, t.day) * 86_400
        + t.hour as u64 * 3_600
        + t.minute as u64 * 60
        + t.second as u64
}

/// The realtime base: the RTC instant less the monotonic time already elapsed
/// when it was read, so that `realtime = base + monotonic` reads the RTC's
/// time at the moment of the read. Saturating: monotonic past the RTC instant
/// (impossible for a valid year) clamps at 0, which is boot-relative.
pub const fn base_ns(unix_s: u64, monotonic_ns: u64) -> u64 {
    unix_s
        .saturating_mul(1_000_000_000)
        .saturating_sub(monotonic_ns)
}
