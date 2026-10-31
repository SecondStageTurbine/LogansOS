// SPDX-License-Identifier: MPL-2.0
//! Pure parsing of the ACPI tables that locate the platform's independent
//! clocks (CP-69, Jarvis's #84 review: the LAPIC timer must be calibrated
//! against a clock that is not the TSC and does not depend on the 8254 PIT,
//! which client silicon may gate). Shared with the host KATs in
//! `tools/acpi-timer-logic-tests` (mounted by `#[path]`).
//!
//! Sources, read for principle (ACPI 6.5 §5.2.9 "FADT", the IA-PC HPET
//! specification 1.0a §3.2.4 "ACPI table"):
//! - FADT (`FACP`): `PM_TMR_BLK` (u32 I/O port) at byte 76, `PM_TMR_LEN` at
//!   91 (4 when present), flags at 112 (bit 8 `TMR_VAL_EXT`: 32-bit counter,
//!   else 24-bit; bit 20 `HW_REDUCED_ACPI`: the fixed-hardware fields at
//!   bytes 46-108 and 148-232, both PM timer forms among them, must be
//!   ignored); ACPI 2.0+ `X_PM_TMR_BLK` generic address at 208 (address
//!   space 1 = system I/O, 64-bit address at +4). The PM timer runs at
//!   3.579545 MHz.
//! - HPET table: the event-timer block's generic address at byte 40
//!   (address space 0 = system memory, 64-bit address at +4 = byte 44).
//!
//! Also the pure parts of driving those clocks (`arch::x86_64::refclock`),
//! so the host KATs run them against register images and port traces:
//! taking the HPET over from firmware, and the PIT channel-2 one-shot's
//! completion test.

/// The ACPI PM timer's fixed rate.
pub const PM_TIMER_HZ: u64 = 3_579_545;

/// The ACPI PM timer: its I/O port and whether its counter is 32 bits wide
/// (else 24).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PmTimer {
    pub port: u16,
    pub wide: bool,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// FADT flags: the PM timer counts 32 bits (else 24).
const TMR_VAL_EXT: u32 = 1 << 8;
/// FADT flags: hardware-reduced ACPI, no fixed hardware (no PM timer).
pub const HW_REDUCED_ACPI: u32 = 1 << 20;

/// The PM timer from a FADT, preferring the 64-bit `X_PM_TMR_BLK` when it
/// names a system-I/O port, else `PM_TMR_BLK`. `None` when neither is set,
/// or when the FADT says hardware-reduced: then both fields are to be
/// ignored whatever they hold (Jarvis R3), so neither is read. A short
/// (ACPI 1.0) FADT with no flags field is not hardware-reduced.
pub fn fadt_pm_timer(fadt: &[u8]) -> Option<PmTimer> {
    if fadt.get(..4)? != b"FACP" {
        return None;
    }
    let flags = u32_at(fadt, 112).unwrap_or(0);
    if flags & HW_REDUCED_ACPI != 0 {
        return None;
    }
    let wide = flags & TMR_VAL_EXT != 0;
    // X_PM_TMR_BLK: space id 1 (system I/O), a port below 64 Ki.
    if fadt.len() >= 220
        && fadt[208] == 1
        && let Some(addr) = u64_at(fadt, 212)
        && addr != 0
        && addr <= u16::MAX as u64
    {
        return Some(PmTimer {
            port: addr as u16,
            wide,
        });
    }
    let port = u32_at(fadt, 76)?;
    let len = *fadt.get(91)?;
    if port == 0 || port > u16::MAX as u32 || len != 4 {
        return None;
    }
    Some(PmTimer {
        port: port as u16,
        wide,
    })
}

/// The HPET block's physical address from the HPET table, if it names
/// system memory.
pub fn hpet_base(hpet: &[u8]) -> Option<u64> {
    if hpet.get(..4)? != b"HPET" || *hpet.get(40)? != 0 {
        return None;
    }
    match u64_at(hpet, 44)? {
        0 => None,
        base => Some(base),
    }
}

/// HPET general-capabilities register (offset 0): the counter period in
/// femtoseconds (bits 63:32; the spec caps it at 100 ns) and whether the
/// main counter is 64 bits wide (bit 13). `None` for an implausible period.
pub fn hpet_caps(caps: u64) -> Option<(u64, bool)> {
    let period_fs = caps >> 32;
    if period_fs == 0 || period_fs > 100_000_000 {
        return None;
    }
    Some((period_fs, caps & (1 << 13) != 0))
}

/// HPET counter rate from its period in femtoseconds.
pub const fn hpet_hz(period_fs: u64) -> u64 {
    1_000_000_000_000_000 / period_fs
}

// HPET registers (IA-PC HPET 1.0a §2.3) and the bits taking it over uses.
pub const HPET_CAPS: usize = 0x000;
pub const HPET_CONFIG: usize = 0x010;
pub const HPET_INT_STATUS: usize = 0x020;
pub const HPET_COUNTER: usize = 0x0f0;
/// Comparator n's configuration register: 0x100 + 0x20 * n.
pub const fn hpet_timer_config(n: usize) -> usize {
    0x100 + 0x20 * n
}
/// General configuration: the main counter runs, comparators may interrupt.
pub const HPET_ENABLE_CNF: u64 = 1 << 0;
/// General configuration: legacy replacement, comparators 0/1 take IRQ0/IRQ8
/// from the 8254 and the RTC.
pub const HPET_LEG_RT_CNF: u64 = 1 << 1;
/// Comparator configuration: level-triggered (else edge-triggered).
pub const HPET_TN_INT_TYPE_CNF: u64 = 1 << 1;
/// Comparator configuration: the comparator may interrupt.
pub const HPET_TN_INT_ENB_CNF: u64 = 1 << 2;

/// How many comparators the HPET has (capabilities bits 12:8, plus one).
pub const fn hpet_comparators(caps: u64) -> usize {
    ((caps >> 8) & 0x1f) as usize + 1
}

/// The HPET's registers, as volatile MMIO in the kernel and as an image in
/// the host KATs.
pub trait HpetRegs {
    fn read(&mut self, offset: usize) -> u64;
    fn write(&mut self, offset: usize, value: u64);
}

/// What firmware had left set when the HPET was taken over (for the log).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HpetTakeover {
    pub was_enabled: bool,
    pub was_legacy: bool,
    /// Comparators whose interrupt was enabled, and is now disabled.
    pub comparators_quieted: u32,
}

/// Take the HPET over from firmware to read its main counter by polling
/// (Jarvis R2). Setting `ENABLE_CNF` lets every comparator whose interrupt
/// is enabled fire, and legacy replacement takes IRQ0 from the 8254, which
/// the PIT fallback needs (HPET 1.0a §2.3.5, §2.4.2). So, in order:
/// 1. stop the counter and clear legacy replacement (giving IRQ0/IRQ8 back
///    to the 8254 and the RTC), whatever state firmware left;
/// 2. disable every comparator's interrupt;
/// 3. clear the interrupt status a level-triggered comparator latched
///    (write 1 to clear; an edge-triggered comparator's bit must be written
///    0, HPET 1.0a §2.3.6, Jarvis R2-P3);
/// 4. start the counter, with no comparator able to interrupt.
///
/// Other bits are kept.
pub fn hpet_take_ownership(regs: &mut dyn HpetRegs, caps: u64) -> HpetTakeover {
    let config = regs.read(HPET_CONFIG);
    let stopped = config & !(HPET_ENABLE_CNF | HPET_LEG_RT_CNF);
    regs.write(HPET_CONFIG, stopped);
    let mut quieted = 0;
    let mut level = 0u64;
    for n in 0..hpet_comparators(caps) {
        let at = hpet_timer_config(n);
        let timer = regs.read(at);
        if timer & HPET_TN_INT_TYPE_CNF != 0 {
            level |= 1 << n;
        }
        if timer & HPET_TN_INT_ENB_CNF != 0 {
            regs.write(at, timer & !HPET_TN_INT_ENB_CNF);
            quieted += 1;
        }
    }
    let status = regs.read(HPET_INT_STATUS) & level;
    if status != 0 {
        regs.write(HPET_INT_STATUS, status);
    }
    regs.write(HPET_CONFIG, stopped | HPET_ENABLE_CNF);
    HpetTakeover {
        was_enabled: config & HPET_ENABLE_CNF != 0,
        was_legacy: config & HPET_LEG_RT_CNF != 0,
        comparators_quieted: quieted,
    }
}

/// Port 0x61 bit 5: PIT channel 2's output (OUT2).
pub const PORT61_OUT2: u8 = 1 << 5;

/// Whether a PIT channel-2 one-shot just programmed in mode 0 ran to its
/// terminal count, polled through port 0x61. Mode 0 drives OUT2 LOW when
/// programmed and HIGH at terminal count, so a finished window needs both
/// edges, each timed on the read that sees it (Jarvis R4, round 3: a level
/// first seen after its deadline is late, not on time). All times are in
/// `now`'s units, from the call:
/// - LOW by `low_timeout` (on a real PIT it is low at once);
/// - then HIGH no sooner than `min_window` and no later than `high_timeout`;
/// - and no two polls more than `max_gap` apart: a poller held off (an SMI,
///   a VM exit) may have missed when the edge came, so the window's length
///   is unknown (#84 round 4, defence in depth: the PIT is also never the
///   only calibration source).
///
/// A port stuck HIGH (0xff from an absent or gated PIT) or stuck LOW, a late
/// edge, a window far shorter than programmed, or a stalled poll is no
/// window. The bounds are coarse (the caller does not know the clock rate it
/// is calibrating); finer errors are the cross-check's to catch.
pub fn pit_one_shot_completed(
    read_61: &mut dyn FnMut() -> u8,
    now: &mut dyn FnMut() -> u64,
    low_timeout: u64,
    min_window: u64,
    high_timeout: u64,
    max_gap: u64,
) -> bool {
    let t0 = now();
    let mut last = t0;
    let mut low_seen = false;
    loop {
        let out2 = read_61() & PORT61_OUT2;
        let t = now();
        let elapsed = t.wrapping_sub(t0);
        if t.wrapping_sub(last) > max_gap {
            return false;
        }
        last = t;
        if !low_seen {
            if elapsed > low_timeout {
                return false;
            }
            low_seen = out2 == 0;
        } else {
            if elapsed > high_timeout {
                return false;
            }
            if out2 != 0 {
                return elapsed >= min_window;
            }
        }
        core::hint::spin_loop();
    }
}
