// SPDX-License-Identifier: MPL-2.0
//! Pure rules for the LAPIC timer tick (`lapic-timer`), shared with the host
//! KATs in `tools/lapic-timer-logic-tests` (mounted by `#[path]`).
//!
//! Sources, read for principle (Intel SDM Vol. 3A ch. 11 "APIC Timer";
//! Vol. 3B ch. 18 "Time-Stamp Counter"; CPUID leaves 0x15/0x16):
//! - CPUID 0x15: TSC/crystal ratio EBX/EAX, crystal Hz in ECX (0 = not
//!   enumerated). TSC Hz = ECX * EBX / EAX.
//! - CPUID 0x16: EAX = processor base frequency in MHz (an approximation of
//!   the TSC rate on parts that do not enumerate 0x15's crystal).
//! - LVT timer mode bits 18:17: 00 one-shot, 01 periodic, 10 TSC-deadline.
//! - Divide configuration 0b1011 = divide by 1.

/// LVT timer mode field values (bits 18:17).
pub const LVT_PERIODIC: u32 = 0b01 << 17;
pub const LVT_TSC_DEADLINE: u32 = 0b10 << 17;
/// LVT mask bit.
pub const LVT_MASKED: u32 = 1 << 16;
/// Divide configuration: divide by 1.
pub const DIVIDE_BY_1: u32 = 0b1011;

/// TSC frequency from CPUID leaf 0x15 (`eax`, `ebx`, `ecx`), if enumerated.
pub const fn tsc_hz_from_leaf15(eax: u32, ebx: u32, ecx: u32) -> Option<u64> {
    if eax == 0 || ebx == 0 || ecx == 0 {
        return None;
    }
    Some(ecx as u64 * ebx as u64 / eax as u64)
}

/// The processor BASE frequency from CPUID leaf 0x16 EAX (MHz). Intel
/// documents it as a specification value that need not match any clock the
/// processor runs: an ESTIMATE of the TSC rate, used only when nothing else
/// is available, and then reported as such (`RateChoice::Estimated`).
pub const fn tsc_hz_from_leaf16(eax: u32) -> Option<u64> {
    let mhz = eax & 0xffff;
    if mhz == 0 {
        None
    } else {
        Some(mhz as u64 * 1_000_000)
    }
}

/// Clock cycles (TSC or LAPIC) per tick at `tick_hz`; `None` when the period
/// would not fit the 32-bit LAPIC initial count (periodic) or is zero.
pub const fn period(clock_hz: u64, tick_hz: u32) -> Option<u64> {
    if tick_hz == 0 {
        return None;
    }
    let p = clock_hz / tick_hz as u64;
    if p == 0 { None } else { Some(p) }
}

/// The LAPIC initial count for a periodic tick, if it fits 32 bits.
pub const fn periodic_initial_count(lapic_hz: u64, tick_hz: u32) -> Option<u32> {
    match period(lapic_hz, tick_hz) {
        Some(p) if p <= u32::MAX as u64 => Some(p as u32),
        _ => None,
    }
}

/// TSC cycles to wait for the first tick of a freshly armed mode before
/// calling it dead: five tick periods (50 ms at 100 Hz) at `budget_hz`,
/// which comes from `budget_tsc_hz`.
pub const fn first_tick_budget(budget_hz: u64, tick_hz: u32) -> u64 {
    if tick_hz == 0 {
        return budget_hz;
    }
    budget_hz / tick_hz as u64 * 5
}

/// An upper bound on the TSC rate of any x86 this kernel runs on (10 GHz).
pub const TSC_HZ_BOUND: u64 = 10_000_000_000;

/// The TSC rate that sizes time budgets: the first-tick waits and the PIT
/// fallback's wait. Only a calibrated rate (exact, or measured against an
/// independent clock, and cross-checked when a second source exists) is
/// trusted for this. Otherwise it is `TSC_HZ_BOUND`. A budget that is too
/// long only delays a demotion, but one that is too short disarms a working
/// tick. So a rate that was refused (`Mismatch`, Jarvis R1: CPUID 0x15
/// saying 24 MHz for a 2.4 GHz TSC gave a 0.5 ms first-tick wait) or never
/// checked (`Estimated`) never sizes one.
pub fn budget_tsc_hz(choice: &RateChoice) -> u64 {
    match choice {
        RateChoice::Calibrated { rate, .. } if rate.hz != 0 => rate.hz.min(TSC_HZ_BOUND),
        _ => TSC_HZ_BOUND,
    }
}

/// Which tick source to run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    /// TSC-deadline mode, re-armed `period` TSC cycles after each tick.
    TscDeadline { period: u64 },
    /// Periodic mode with this initial count (divide by 1).
    Periodic { initial_count: u32 },
    /// Nothing could be calibrated: keep the PIT/PIC tick.
    Pit,
}

/// Choose the tick: TSC-deadline needs the CPUID flag and a TSC rate;
/// periodic needs a LAPIC rate that gives a 32-bit count. A mode that is
/// armed but never ticks is demoted by planning again without it: a dead
/// TSC-deadline mode with `deadline_supported = false` (periodic, else PIT),
/// a dead periodic mode with `lapic_hz = None` too (PIT).
pub const fn plan(
    deadline_supported: bool,
    tsc_hz: Option<u64>,
    lapic_hz: Option<u64>,
    tick_hz: u32,
) -> Plan {
    if deadline_supported
        && let Some(tsc) = tsc_hz
        && let Some(p) = period(tsc, tick_hz)
    {
        return Plan::TscDeadline { period: p };
    }
    if let Some(l) = lapic_hz
        && let Some(c) = periodic_initial_count(l, tick_hz)
    {
        return Plan::Periodic { initial_count: c };
    }
    Plan::Pit
}

/// Where a rate came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RateSource {
    /// CPUID 0x15 (crystal x ratio): exact by definition.
    Leaf15,
    /// Measured against the HPET.
    Hpet,
    /// Measured against the ACPI PM timer.
    PmTimer,
    /// Measured against PIT channel 2.
    Pit,
    /// CPUID 0x16 base frequency: an estimate.
    Leaf16Estimate,
}

impl RateSource {
    pub const fn name(self) -> &'static str {
        match self {
            RateSource::Leaf15 => "cpuid15",
            RateSource::Hpet => "hpet",
            RateSource::PmTimer => "acpi-pm",
            RateSource::Pit => "pit",
            RateSource::Leaf16Estimate => "estimated(cpuid16)",
        }
    }
}

/// A rate and its source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rate {
    pub hz: u64,
    pub source: RateSource,
}

/// The calibration outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RateChoice {
    /// An exact or measured rate, cross-checked against a second
    /// independent source when there is one (`checked`).
    Calibrated { rate: Rate, checked: Option<Rate> },
    /// Only CPUID 0x16 was available: an uncross-checked estimate.
    Estimated(Rate),
    /// Two sources disagree by more than `CROSS_CHECK_PERCENT`: trust neither.
    Mismatch { primary: Rate, check: Rate },
    /// Only PIT channel 2 measured it: never trusted alone (#84 round 4,
    /// R4-P2b: a CPU held off across terminal count reads a 20 ms window as
    /// 10 ms, a 2x rate, and nothing local can tell). The PIT only checks
    /// another source.
    PitOnly(Rate),
    /// Nothing to go on.
    Unknown,
}

/// The largest disagreement two rate sources may have.
pub const CROSS_CHECK_PERCENT: u64 = 5;

/// Whether `a` is within `percent` of `b`.
pub const fn within_percent(a: u64, b: u64, percent: u64) -> bool {
    let diff = a.abs_diff(b) as u128;
    diff * 100 <= b as u128 * percent as u128
}

/// A rate from a measurement: `delta` counts in `elapsed_ns`.
pub const fn rate_per_second(delta: u64, elapsed_ns: u64) -> Option<u64> {
    if elapsed_ns == 0 || delta == 0 {
        return None;
    }
    Some((delta as u128 * 1_000_000_000 / elapsed_ns as u128) as u64)
}

/// Choose a rate. `exact` is CPUID 0x15's (or `None`); `measured` holds the
/// rates measured against independent clocks, best first (HPET, PM timer,
/// PIT); `estimate` is CPUID 0x16's. The primary is the exact rate, else the
/// best measurement that is not the PIT's; it is cross-checked against the
/// best measurement that is not itself (the PIT included), and refused when
/// they differ by more than 5%. A PIT measurement alone is `PitOnly`, never
/// `Calibrated`. CPUID 0x16 is used only when nothing else exists, and is
/// reported as an estimate.
pub fn choose_rate(exact: Option<u64>, measured: &[Rate], estimate: Option<u64>) -> RateChoice {
    let primary = match exact {
        Some(hz) => Some(Rate {
            hz,
            source: RateSource::Leaf15,
        }),
        None => measured
            .iter()
            .copied()
            .find(|m| m.source != RateSource::Pit),
    };
    let Some(primary) = primary else {
        if let Some(pit) = measured
            .iter()
            .copied()
            .find(|m| m.source == RateSource::Pit)
        {
            return RateChoice::PitOnly(pit);
        }
        return match estimate {
            Some(hz) => RateChoice::Estimated(Rate {
                hz,
                source: RateSource::Leaf16Estimate,
            }),
            None => RateChoice::Unknown,
        };
    };
    let check = measured
        .iter()
        .copied()
        .find(|m| m.source != primary.source);
    match check {
        Some(check) if !within_percent(check.hz, primary.hz, CROSS_CHECK_PERCENT) => {
            RateChoice::Mismatch { primary, check }
        }
        checked => RateChoice::Calibrated {
            rate: primary,
            checked,
        },
    }
}
