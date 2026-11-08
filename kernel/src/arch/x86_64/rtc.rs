// SPDX-License-Identifier: MPL-2.0
//! The boot-time RTC epoch seed (OT-85, feature `rtc-epoch`): read the
//! MC146818 CMOS clock once through ports 0x70/0x71 and keep the result as
//! `CLOCK_REALTIME`'s base, so realtime = base + monotonic (C1 design P4:
//! one clock source, a boot epoch). The decoding and the agreement loop are
//! `rtc_logic`'s (host-KAT'd); this file is the port I/O and the state.
//!
//! **Local time.** The RTC is read as local wall time, because the Zenbook
//! dual-boots Windows, which keeps it that way. There is no time zone, no DST
//! rule and no UTC offset: the base is "the wall time the RTC reported", and
//! the display formats it with no offset (see `rtc_logic`'s module doc).
//! With `rtc-offset` (THINK-2026-11-02-time-zones §2.1) that reading starts
//! **unconfirmed**: the session, which has the zone, confirms it as UTC once
//! ([`confirm`]), moving the base by the RTC's offset from UTC (0 for an RTC
//! that holds UTC). The kernel still has no zone and no DST rule.
//!
//! **Write once, monotonic untouched.** [`seed`] runs once on the BSP at boot,
//! before any user process. It stores the base and then publishes it; a
//! second call does nothing. Monotonic time never reads this state, so the
//! seed cannot move it. Queries are one atomic load (a leaf read: no lock).
//!
//! **Fallback.** An invalid or unstable clock, a stuck update cycle, or a
//! FADT that says there is no CMOS RTC leaves the base at 0: realtime stays
//! boot-relative exactly as before this rung. One serial line names why, and
//! the boot goes on.

use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;

use super::rtc_logic::{self, Invalid, Raw, Read};

/// The realtime base in ns; meaningful only once [`STATE`] is `SEEDED`.
static BASE_NS: AtomicU64 = AtomicU64::new(0);

const UNSEEDED: u8 = 0;
const CLAIMED: u8 = 1;
const SEEDED: u8 = 2;
const FALLBACK: u8 = 3;
static STATE: AtomicU8 = AtomicU8::new(UNSEEDED);

/// Register-A polls before an update cycle counts as stuck. The cycle lasts
/// under 2 ms; each poll is two port accesses (about 1-2 us on silicon).
#[cfg(not(feature = "rtc-epoch-force-invalid"))]
const UIP_POLLS: u32 = 100_000;

#[cfg_attr(feature = "rtc-epoch-force-invalid", allow(dead_code))]
const INDEX: u16 = 0x70;
#[cfg_attr(feature = "rtc-epoch-force-invalid", allow(dead_code))]
const DATA: u16 = 0x71;

/// `rtc-offset`: whether the session has confirmed realtime as UTC
/// ([`confirm`]). 0 unconfirmed, 1 a confirm in progress, 2 confirmed.
#[cfg(feature = "rtc-offset")]
static CONFIRM: AtomicU8 = AtomicU8::new(0);

/// Why [`confirm`] refused.
#[cfg(feature = "rtc-offset")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmError {
    /// No RTC epoch (the fallback): realtime is boot-relative, not a reading.
    NotSeeded,
    /// Already confirmed (write once).
    Already,
    /// The offset is out of range.
    Range,
}

/// `rtc-offset` (THINK-2026-11-02-time-zones §2.1, the PM's option 3): the
/// session, which knows the zone and whether the RTC holds local time,
/// confirms realtime as UTC once. `offset_s` is the RTC's offset from UTC
/// (0 when the RTC holds UTC). The base moves by the offset in one atomic
/// store, published before the confirmed flag (release), so a reader that
/// sees the flag (acquire) reads the moved base. Monotonic is untouched.
#[cfg(feature = "rtc-offset")]
pub fn confirm(offset_s: i64) -> Result<(), ConfirmError> {
    if STATE.load(Ordering::Acquire) != SEEDED {
        return Err(ConfirmError::NotSeeded);
    }
    if CONFIRM
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(ConfirmError::Already);
    }
    let Some(base) = rtc_logic::confirmed_base_ns(BASE_NS.load(Ordering::Relaxed), offset_s) else {
        CONFIRM.store(0, Ordering::Release);
        return Err(ConfirmError::Range);
    };
    BASE_NS.store(base, Ordering::Release);
    CONFIRM.store(2, Ordering::Release);
    crate::serial_println!("rtc: realtime confirmed as utc offset_s={}", offset_s);
    Ok(())
}

/// Whether realtime is confirmed UTC. Read this before the clock: a reader
/// that sees `true` then reads the confirmed base.
#[cfg(feature = "rtc-offset")]
pub fn confirmed() -> bool {
    CONFIRM.load(Ordering::Acquire) == 2
}

/// `CLOCK_REALTIME`'s base: the seeded one, else 0 (boot-relative).
pub fn realtime_base_ns() -> u64 {
    if STATE.load(Ordering::Acquire) == SEEDED {
        BASE_NS.load(Ordering::Relaxed)
    } else {
        0
    }
}

#[cfg_attr(feature = "rtc-epoch-force-invalid", allow(dead_code))]
/// Read CMOS register `reg` (NMI-disable bit 7 of the index left clear).
fn cmos(reg: u8) -> u8 {
    let mut index = Port::<u8>::new(INDEX);
    let mut data = Port::<u8>::new(DATA);
    // SAFETY: the CMOS index/data ports; only this boot-time seed touches
    // them, with interrupts off, so no one else's index write can intervene.
    unsafe {
        index.write(reg & 0x7F);
        data.read()
    }
}

/// One snapshot of the register file, taken once UIP is clear; `None` if it
/// never clears. `century` is read from `century_index` when there is one.
#[cfg(not(feature = "rtc-epoch-force-invalid"))]
fn snapshot(century_index: Option<u8>) -> Option<Raw> {
    let mut polls = 0;
    while cmos(rtc_logic::REG_A) & rtc_logic::A_UIP != 0 {
        polls += 1;
        if polls >= UIP_POLLS {
            return None;
        }
        core::hint::spin_loop();
    }
    Some(Raw {
        seconds: cmos(rtc_logic::REG_SECONDS),
        minutes: cmos(rtc_logic::REG_MINUTES),
        hours: cmos(rtc_logic::REG_HOURS),
        day: cmos(rtc_logic::REG_DAY),
        month: cmos(rtc_logic::REG_MONTH),
        year: cmos(rtc_logic::REG_YEAR),
        century: century_index.map(cmos),
        reg_b: cmos(rtc_logic::REG_B),
        reg_d: cmos(rtc_logic::REG_D),
    })
}

/// Test hook (`rtc-epoch-force-invalid`): every register reads 0xFF, as a
/// floating bus does when no clock answers. The real validator then refuses
/// it and the seed takes the fallback.
#[cfg(feature = "rtc-epoch-force-invalid")]
fn snapshot(century_index: Option<u8>) -> Option<Raw> {
    Some(Raw {
        seconds: 0xFF,
        minutes: 0xFF,
        hours: 0xFF,
        day: 0xFF,
        month: 0xFF,
        year: 0xFF,
        century: century_index.map(|_| 0xFF),
        reg_b: 0xFF,
        reg_d: 0xFF,
    })
}

fn fallback(reason: &str) {
    STATE.store(FALLBACK, Ordering::Release);
    crate::serial_println!("[ kernel ] rtc: fallback reason={reason} realtime=boot-relative");
}

/// Seed the realtime base from the RTC (once; later calls do nothing).
/// `rsdp_phys` reaches the FADT's century index and "CMOS RTC Not Present"
/// flag; 0 (or unreadable tables) means neither: no century register.
pub fn seed(rsdp_phys: u64) {
    if STATE
        .compare_exchange(UNSEEDED, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let fadt = crate::acpi::rtc_fadt::rtc_fadt(rsdp_phys);
    if fadt.is_some_and(|f| f.rtc_not_present) {
        fallback("acpi-cmos-rtc-not-present");
        return;
    }
    let century_index = fadt.and_then(|f| f.century_index);
    let (read, mono_ns) = interrupts::without_interrupts(|| {
        let read = rtc_logic::read_stable(|| snapshot(century_index), rtc_logic::MAX_READS);
        // Monotonic at the read: 10 ms per tick (as `sys_clock_gettime_impl`).
        let ticks = super::timer::TICKS.load(Ordering::Relaxed);
        (read, ticks.saturating_mul(10_000_000))
    });
    let (raw, reads) = match read {
        Read::Stable { raw, reads } => (raw, reads),
        Read::Unstable => return fallback("unstable"),
        Read::Stuck => return fallback("uip-stuck"),
    };
    let wall = match rtc_logic::decode(&raw) {
        Ok(wall) => wall,
        Err(Invalid::PowerLost) => return fallback("power-lost"),
        Err(Invalid::BadBcd) => return fallback("bad-bcd"),
        Err(Invalid::OutOfRange) => return fallback("out-of-range"),
        Err(Invalid::YearOutOfRange) => return fallback("year-out-of-range"),
    };
    let base = rtc_logic::base_ns(rtc_logic::unix_seconds(&wall), mono_ns);
    BASE_NS.store(base, Ordering::Relaxed);
    STATE.store(SEEDED, Ordering::Release);
    crate::serial_println!(
        "[ kernel ] rtc: seeded wall={:04}-{:02}-{:02}T{:02}:{:02}:{:02} as=local mode={}/{} century={:#04x} reads={} base_ns={}",
        wall.year,
        wall.month,
        wall.day,
        wall.hour,
        wall.minute,
        wall.second,
        if raw.reg_b & rtc_logic::B_BINARY != 0 {
            "binary"
        } else {
            "bcd"
        },
        if raw.reg_b & rtc_logic::B_24H != 0 {
            "24h"
        } else {
            "12h"
        },
        century_index.unwrap_or(0),
        reads,
        base
    );
}
