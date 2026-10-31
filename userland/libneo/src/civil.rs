// SPDX-License-Identifier: MPL-2.0
//! Epoch → civil time (OT-85): a `CLOCK_REALTIME` reading as a calendar date,
//! weekday and 12-hour time, and its English text for the desktop. Pure: no
//! allocation, no syscall; the probe and the Atrium readout call the same
//! functions. Host KATs: `tools/rtc-logic-tests` (mounted by `#[path]`).
//!
//! **No offset.** The kernel seeds realtime from the RTC read as local wall
//! time (`kernel/src/arch/x86_64/rtc_logic.rs`), so a reading is local wall
//! time counted as if UTC: it is converted here with no time zone, no DST
//! rule and no UTC offset, and shows exactly the wall time the RTC reported.
//!
//! Proleptic Gregorian calendar, 1970-01-01 onward (realtime is unsigned).

pub const NS_PER_SECOND: u64 = 1_000_000_000;

/// A civil instant. `weekday`: 0 = Sunday .. 6 = Saturday.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    pub year: u32,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `(year, month, day)` of the day `days` after 1970-01-01. The count is
/// shifted to days since 0000-03-01 (719 468 of them before the epoch), so
/// years run March to February and the leap day ends the year; a 400-year
/// era is 146 097 days, and inside it the year of the era removes the
/// 4/100/400-year leap corrections before dividing by 365. The day of that
/// shifted year then splits into months by the 153-day five-month cycle.
pub const fn civil_from_days(days: u64) -> (u32, u8, u8) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // March = 0 .. February = 11
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year as u32, month as u8, day as u8)
}

/// The civil instant `secs` seconds after 1970-01-01T00:00:00.
pub const fn from_unix_seconds(secs: u64) -> Civil {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        // 1970-01-01 was a Thursday (4).
        weekday: ((days + 4) % 7) as u8,
        hour: (rem / 3_600) as u8,
        minute: (rem % 3_600 / 60) as u8,
        second: (rem % 60) as u8,
    }
}

/// The civil instant of a `CLOCK_REALTIME` reading (ns).
pub const fn from_realtime_ns(ns: u64) -> Civil {
    from_unix_seconds(ns / NS_PER_SECOND)
}

/// Fixed-capacity ASCII text (no allocation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn as_str(&self) -> &str {
        // Only ASCII is ever pushed.
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }

    /// Append `s`, truncated at capacity.
    pub fn push(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if self.len == N {
                return;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    /// Append `n` in decimal, at least `width` digits (zero-padded).
    pub fn push_num(&mut self, n: u32, width: usize) {
        let mut digits = [0u8; 10];
        let mut i = 0;
        let mut v = n;
        loop {
            digits[i] = b'0' + (v % 10) as u8;
            i += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        while i < width && i < digits.len() {
            digits[i] = b'0';
            i += 1;
        }
        while i > 0 {
            i -= 1;
            if self.len == N {
                return;
            }
            self.buf[self.len] = digits[i];
            self.len += 1;
        }
    }
}

impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// "Wednesday, September 30, 2026" is the longest (29 bytes).
pub type DateText = Text<32>;
/// "12:59 PM" is the longest (8 bytes).
pub type TimeText = Text<8>;

impl Civil {
    pub const fn weekday_name(&self) -> &'static str {
        WEEKDAYS[self.weekday as usize % 7]
    }

    pub const fn month_name(&self) -> &'static str {
        MONTHS[(self.month as usize + 11) % 12]
    }

    /// The hour on a 12-hour clock (1..=12) and whether it is PM.
    pub const fn hour12(&self) -> (u8, bool) {
        let h = self.hour % 12;
        (if h == 0 { 12 } else { h }, self.hour >= 12)
    }

    /// "Wednesday, October 21, 2026".
    pub fn date_text(&self) -> DateText {
        let mut t = DateText::new();
        t.push(self.weekday_name());
        t.push(", ");
        t.push(self.month_name());
        t.push(" ");
        t.push_num(u32::from(self.day), 1);
        t.push(", ");
        t.push_num(self.year, 4);
        t
    }

    /// "5:48 PM" (12-hour, minutes zero-padded, no seconds).
    pub fn time_text(&self) -> TimeText {
        let (h, pm) = self.hour12();
        let mut t = TimeText::new();
        t.push_num(u32::from(h), 1);
        t.push(":");
        t.push_num(u32::from(self.minute), 2);
        t.push(if pm { " PM" } else { " AM" });
        t
    }

    /// Whether `other` falls in the same minute (the readout repaints only
    /// when it does not).
    pub const fn same_minute(&self, other: &Civil) -> bool {
        self.year == other.year
            && self.month == other.month
            && self.day == other.day
            && self.hour == other.hour
            && self.minute == other.minute
    }
}
