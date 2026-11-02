// SPDX-License-Identifier: MPL-2.0
//! The BSP's kernel tick from the local APIC timer (`lapic-timer`), instead
//! of the 8254 PIT through the 8259 PIC (Zenbook lane, NOTE §4a: newer Intel
//! client platforms may clock-gate the PIT or lack the 8259 route, and then
//! the kernel never ticks).
//!
//! It fires the SAME vector the PIT used (32), so the existing timer entry
//! (`timer::timer_isr_entry`: IST switch, ring-3 prologue, FPU save) is
//! unchanged; only the end-of-interrupt differs (`eoi`). It works in xAPIC
//! and x2APIC mode through `apic`'s register path. The PIC's IRQ0 stays
//! masked. Rules and calibration math: `lapic_timer_logic`.
//!
//! Calibration (CP-69; Jarvis's review of #84):
//! - TSC rate: CPUID 0x15 (crystal x ratio, exact), else measured against an
//!   independent clock (`refclock`: HPET, ACPI PM timer), and cross-checked
//!   against a second independent source when one exists (PIT channel 2
//!   included). The PIT is never the only source (#84 round 4: a CPU held
//!   off across its terminal count reads a long window as 10 ms): with only
//!   the PIT, nothing is calibrated and the PIT/PIC tick runs instead. A
//!   disagreement beyond 5% disqualifies what depends on that rate: a
//!   disputed TSC rate rules out TSC-deadline mode, a disputed LAPIC rate
//!   rules out periodic mode. CPUID 0x16 (the base
//!   frequency) is only a last-resort ESTIMATE, logged as
//!   `rate=estimated(cpuid16)` and never used: it neither sets a tick
//!   period nor calibrates the LAPIC (Jarvis R1, round 3).
//! - Mode: TSC-deadline when CPUID.01H:ECX[24] says so; otherwise periodic,
//!   with the LAPIC rate from CPUID 0x15's crystal or measured against the
//!   same clocks, cross-checked the same way.
//! - Nothing calibrated: `start` returns false (`failure` says why) and the
//!   caller starts the PIT/PIC tick, which it then verifies.
//! - Each armed mode must deliver its first tick within five periods, or it
//!   is disarmed and demoted (TSC-deadline, periodic, PIT). Those waits are
//!   sized from a calibrated TSC rate, else from a 10 GHz bound, never from
//!   a refused or estimated rate (Jarvis R1): they may run long, never
//!   short. The
//!   `lapic_timer_demote` scenario drives that chain with a silent timer.
//!
//! Kill-switch: feature off ⇒ none of this is compiled; the tick is PIT/PIC.

use core::arch::x86_64::{__cpuid, __cpuid_count, _rdtsc};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use x86_64::registers::model_specific::Msr;

use crate::arch::x86_64::apic;
use crate::arch::x86_64::lapic_timer_logic::{
    CROSS_CHECK_PERCENT, DIVIDE_BY_1, LVT_MASKED, LVT_PERIODIC, LVT_TSC_DEADLINE, Plan, Rate,
    RateChoice, RateSource, TSC_HZ_BOUND, budget_tsc_hz, choose_rate, first_tick_budget, plan,
    rate_per_second, tsc_hz_from_leaf15, tsc_hz_from_leaf16,
};
use crate::arch::x86_64::refclock::{self, RefClock};
use crate::serial_println;

/// The vector the PIT/PIC tick used, and the timer entry is installed on.
const TIMER_VECTOR: u32 = 32;
const IA32_TSC_DEADLINE: u32 = 0x6e0;

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// TSC cycles per tick in TSC-deadline mode; 0 in periodic mode.
static DEADLINE_PERIOD: AtomicU64 = AtomicU64::new(0);
/// The TSC rate the tick is programmed with (0 if unknown or refused), for
/// tests and diagnostics.
static TSC_HZ: AtomicU64 = AtomicU64::new(0);
/// The TSC rate that sizes time budgets (`budget_tsc_hz`): a calibrated
/// rate, else the upper bound, including before calibration runs.
static BUDGET_TSC_HZ: AtomicU64 = AtomicU64::new(TSC_HZ_BOUND);
/// Modes armed that never ticked and were demoted.
static DEMOTIONS: AtomicU64 = AtomicU64::new(0);
/// The calibrated LAPIC count rate (divide by 1; 0 if unknown or unused).
static LAPIC_HZ: AtomicU64 = AtomicU64::new(0);
/// Why the LAPIC tick is not running ("none" while it is, or untried).
static FAILURE: spin::Mutex<&'static str> = spin::Mutex::new("none");

fn set_failure(reason: &'static str) {
    *FAILURE.lock() = reason;
}

/// Why the LAPIC tick is not running, for the no-tick-source halt line.
pub fn failure() -> &'static str {
    *FAILURE.lock()
}

/// The calibrated LAPIC count rate, or 0.
#[allow(dead_code)] // the lapic_timer scenarios
pub fn lapic_hz() -> u64 {
    LAPIC_HZ.load(Ordering::Acquire)
}

fn rdtsc() -> u64 {
    // SAFETY: RDTSC has no side effects.
    unsafe { _rdtsc() }
}

/// Whether the LAPIC timer is the tick.
#[allow(dead_code)] // the lapic_timer scenarios
#[inline]
pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// The calibrated TSC rate in Hz, or 0.
#[allow(dead_code)] // the lapic_timer_tick scenario's rate check
pub fn tsc_hz() -> u64 {
    TSC_HZ.load(Ordering::Acquire)
}

/// The TSC rate to size a wait for a tick with: never below the real rate
/// (Jarvis R1), so a working tick is never declared dead early.
pub fn budget_hz() -> u64 {
    BUDGET_TSC_HZ.load(Ordering::Acquire)
}

/// How many armed modes were demoted for never ticking.
#[allow(dead_code)] // the lapic_timer_demote scenario
pub fn demotions() -> u64 {
    DEMOTIONS.load(Ordering::Acquire)
}

/// The tick's mode, for tests and diagnostics.
#[allow(dead_code)] // the lapic_timer_tick scenarios
pub fn mode_name() -> &'static str {
    if !active() {
        "pit"
    } else if DEADLINE_PERIOD.load(Ordering::Acquire) != 0 {
        "tsc-deadline"
    } else {
        "periodic"
    }
}

/// End of a vector-32 tick, called from the timer handler with IF=0. The
/// source is read from the local APIC's in-service bit, not from `active`:
/// a LAPIC tick that lands just after `disarm` must still EOI the LAPIC, or
/// its ISR bit stays set and blocks every vector of its priority class.
/// - LAPIC tick: re-arm the deadline (TSC-deadline mode), then EOI the LAPIC.
/// - PIT/PIC tick (ExtINT, no ISR bit): EOI the PIC.
pub fn eoi() {
    if !apic::in_service(TIMER_VECTOR as u8) {
        crate::arch::x86_64::pic::end_of_interrupt(0);
        return;
    }
    let period = DEADLINE_PERIOD.load(Ordering::Relaxed);
    if period != 0 {
        // SAFETY: IA32_TSC_DEADLINE exists (CPUID.01H:ECX[24], checked at
        // start); writing it arms the next one-shot deadline.
        unsafe { Msr::new(IA32_TSC_DEADLINE).write(rdtsc().wrapping_add(period)) };
    }
    apic::end_of_interrupt();
}

fn max_leaf() -> u32 {
    __cpuid(0).eax
}

/// CPUID 0x15 as (eax, ebx, ecx), if the leaf exists.
fn leaf15() -> Option<(u32, u32, u32)> {
    if max_leaf() < 0x15 {
        return None;
    }
    let r = __cpuid_count(0x15, 0);
    Some((r.eax, r.ebx, r.ecx))
}

fn leaf16_eax() -> Option<u32> {
    if max_leaf() < 0x16 {
        return None;
    }
    Some(__cpuid_count(0x16, 0).eax)
}

fn deadline_supported() -> bool {
    __cpuid(1).ecx & (1 << 24) != 0
}

/// Rates `sample` advances at, measured against up to two independent
/// reference clocks, best first (HPET, PM timer, PIT). A clock that does not
/// advance (a gated PIT) is skipped.
fn measure_rates(sample: &mut dyn FnMut() -> u64) -> ([Rate; 2], usize) {
    let mut out = [Rate {
        hz: 0,
        source: RateSource::Pit,
    }; 2];
    let mut n = 0;
    let (clocks, count) = refclock::available();
    for clock in &clocks[..count] {
        if n == out.len() {
            break;
        }
        let measured = refclock::measure(*clock, refclock::WINDOW_NS, sample)
            .and_then(|(delta, ns)| rate_per_second(delta, ns));
        if let Some(hz) = measured {
            out[n] = Rate {
                hz,
                source: match clock {
                    RefClock::Hpet => RateSource::Hpet,
                    RefClock::PmTimer => RateSource::PmTimer,
                    RefClock::Pit => RateSource::Pit,
                },
            };
            n += 1;
        }
    }
    (out, n)
}

/// The LAPIC timer's count, as a rising sample: a masked one-shot from
/// `u32::MAX`, divide by 1.
fn lapic_count_sample() -> u64 {
    (u32::MAX - apic::lapic_timer_current()) as u64
}

/// Log a calibration outcome; `what` is "tsc" or "lapic".
fn log_choice(what: &str, choice: &RateChoice) {
    match choice {
        RateChoice::Calibrated {
            rate,
            checked: Some(c),
        } => {
            serial_println!(
                "[ kernel ] timer: {} rate={} source={} checked={}:{} (within {}%)",
                what,
                rate.hz,
                rate.source.name(),
                c.source.name(),
                c.hz,
                CROSS_CHECK_PERCENT
            );
        }
        RateChoice::Calibrated {
            rate,
            checked: None,
        } => {
            serial_println!(
                "[ kernel ] timer: {} rate={} source={} checked=none (no second clock)",
                what,
                rate.hz,
                rate.source.name()
            );
        }
        RateChoice::Estimated(rate) => {
            serial_println!(
                "[ kernel ] timer: {} rate=estimated(cpuid16) hz={} - no independent clock to check it; not used",
                what,
                rate.hz
            );
        }
        RateChoice::Mismatch { primary, check } => {
            serial_println!(
                "[ kernel ] timer: {} rate MISMATCH {}={} vs {}={} (beyond {}%) - not trusted",
                what,
                primary.source.name(),
                primary.hz,
                check.source.name(),
                check.hz,
                CROSS_CHECK_PERCENT
            );
        }
        RateChoice::PitOnly(rate) => {
            serial_println!(
                "[ kernel ] timer: {} rate={} measured by the PIT alone - not trusted (the PIT only cross-checks)",
                what,
                rate.hz
            );
        }
        RateChoice::Unknown => {
            serial_println!(
                "[ kernel ] timer: {} rate unknown (no CPUID leaf, no clock)",
                what
            );
        }
    }
}

/// The TSC rate: CPUID 0x15, else measured against the reference clocks,
/// cross-checked; CPUID 0x16 only as a last-resort estimate (Jarvis P2-a).
fn calibrate_tsc() -> RateChoice {
    let exact = leaf15().and_then(|(a, b, c)| tsc_hz_from_leaf15(a, b, c));
    let (measured, n) = measure_rates(&mut rdtsc);
    let estimate = leaf16_eax().and_then(tsc_hz_from_leaf16);
    choose_rate(exact, &measured[..n], estimate)
}

/// The LAPIC timer's count rate (divide by 1): CPUID 0x15's crystal, else
/// measured against the reference clocks, cross-checked. With neither,
/// `Unknown`: an estimated TSC rate never calibrates it (Jarvis R1, round
/// 3), so periodic mode is refused and the PIT is tried.
fn calibrate_lapic() -> RateChoice {
    let crystal = leaf15()
        .map(|(_, _, crystal)| crystal as u64)
        .filter(|&hz| hz != 0);
    apic::lapic_timer_set_divide(DIVIDE_BY_1);
    apic::lapic_timer_set_lvt(LVT_MASKED | TIMER_VECTOR); // one-shot, masked
    // A full 32-bit count lasts seconds at any real LAPIC rate; both
    // ~10 ms measurements fit in it.
    apic::lapic_timer_set_initial(u32::MAX);
    let (measured, n) = measure_rates(&mut lapic_count_sample);
    apic::lapic_timer_set_initial(0);
    choose_rate(crystal, &measured[..n], None)
}

/// OT-90 P3b, K5: stop this CPU's tick before power-off or restart.
#[cfg(feature = "power-stop-cpus")]
pub fn disarm_for_shutdown() {
    disarm();
}

/// Mask and stop the LAPIC timer: a mode that never ticked is disarmed
/// before the next is tried.
fn disarm() {
    ACTIVE.store(false, Ordering::Release);
    apic::lapic_timer_set_lvt(LVT_MASKED | TIMER_VECTOR);
    apic::lapic_timer_set_initial(0);
    if DEADLINE_PERIOD.swap(0, Ordering::AcqRel) != 0 {
        // SAFETY: as in `eoi`; writing 0 disarms the deadline.
        unsafe { Msr::new(IA32_TSC_DEADLINE).write(0) };
    }
}

/// The LVT word for `mode`. The `lapic_timer_demote` scenario arms its
/// first mode masked, a timer that never fires, to drive the demotion.
fn lvt(mode: u32, first: bool) -> u32 {
    #[cfg(feature = "neo-test-lapic-timer-demote")]
    if first {
        return mode | LVT_MASKED | TIMER_VECTOR;
    }
    let _ = first;
    mode | TIMER_VECTOR
}

/// Arm `plan` (TSC-deadline or periodic).
fn arm(plan: Plan, first: bool) {
    match plan {
        Plan::TscDeadline { period } => {
            DEADLINE_PERIOD.store(period, Ordering::Release);
            ACTIVE.store(true, Ordering::Release);
            apic::lapic_timer_set_lvt(lvt(LVT_TSC_DEADLINE, first));
            // SDM: an MFENCE orders the LVT write before the first deadline.
            // SAFETY: fence only.
            unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)) };
            // SAFETY: as in `eoi`.
            unsafe { Msr::new(IA32_TSC_DEADLINE).write(rdtsc().wrapping_add(period)) };
        }
        Plan::Periodic { initial_count } => {
            DEADLINE_PERIOD.store(0, Ordering::Release);
            ACTIVE.store(true, Ordering::Release);
            apic::lapic_timer_set_divide(DIVIDE_BY_1);
            apic::lapic_timer_set_lvt(lvt(LVT_PERIODIC, first));
            apic::lapic_timer_set_initial(initial_count);
        }
        Plan::Pit => {}
    }
}

/// Whether a tick arrives within `budget` TSC cycles. `None` with IF=0
/// (nothing can arrive, so nothing is proven either way).
fn ticked_within(budget: u64) -> Option<u64> {
    use crate::arch::x86_64::timer::TICKS;
    if !x86_64::instructions::interrupts::are_enabled() {
        return None;
    }
    // `lapic_timer_demote` control: the pre-verification behaviour, where an
    // armed mode was trusted without its first tick.
    #[cfg(feature = "neo-test-lapic-timer-no-verify")]
    {
        let _ = budget;
        return None;
    }
    #[allow(unreachable_code)]
    let ticks = TICKS.load(Ordering::Acquire);
    let t0 = rdtsc();
    loop {
        let elapsed = rdtsc().wrapping_sub(t0);
        if TICKS.load(Ordering::Acquire) != ticks {
            return Some(elapsed);
        }
        if elapsed > budget {
            return Some(u64::MAX);
        }
        core::hint::spin_loop();
    }
}

/// Program the BSP's LAPIC timer as the kernel tick at `tick_hz`, proving
/// each mode by its first tick (NOTE §4a: QEMU's TCG has no TSC-deadline
/// mode, so silicon is the first place that mode runs). A mode that does not
/// tick within `first_tick_budget` is disarmed and demoted: TSC-deadline,
/// then periodic, then the PIT. Returns false when the PIT/PIC tick must be
/// started by the caller. Called on the BSP with the scheduler installed.
pub fn start(tick_hz: u32) -> bool {
    // `lapic_timer_tick` control: the not-calibrated fallback, forced.
    #[cfg(feature = "neo-test-lapic-timer-control")]
    {
        serial_println!(
            "[ kernel ] timer: LAPIC tick NOT calibrated (forced by control) - falling back to PIT/PIC"
        );
        set_failure("forced-off");
        let _ = tick_hz;
        return false;
    }
    #[allow(unreachable_code)]
    let tsc = calibrate_tsc();
    log_choice("tsc", &tsc);
    let mut deadline = deadline_supported();
    let tsc_hz = match tsc {
        RateChoice::Calibrated { rate, .. } => Some(rate.hz),
        // An unverified estimate never sets the tick rate (Jarvis R1, round
        // 3): TSC-deadline mode would program its period from it.
        RateChoice::Estimated(_) => {
            if deadline {
                serial_println!(
                    "[ kernel ] timer: TSC rate only estimated - TSC-deadline mode not used"
                );
            }
            deadline = false;
            None
        }
        // Only TSC-deadline mode programs in TSC cycles: a disputed TSC rate
        // disqualifies that mode, not the periodic tick (whose rate is
        // calibrated on its own). Neither disputed rate is used for
        // anything, the budgets included (Jarvis R1).
        RateChoice::Mismatch { .. } => {
            if deadline {
                serial_println!("[ kernel ] timer: TSC rate disputed - TSC-deadline mode not used");
            }
            deadline = false;
            None
        }
        RateChoice::PitOnly(_) | RateChoice::Unknown => None,
    };
    TSC_HZ.store(tsc_hz.unwrap_or(0), Ordering::Release);
    let budget_hz = budget_tsc_hz(&tsc);
    BUDGET_TSC_HZ.store(budget_hz, Ordering::Release);
    // Periodic mode needs the LAPIC count rate: only computed when periodic
    // can be the plan.
    let mut lapic_hz: Option<Option<u64>> = None;
    let mut first = true;
    loop {
        if (!deadline || tsc_hz.is_none()) && lapic_hz.is_none() {
            let choice = calibrate_lapic();
            log_choice("lapic", &choice);
            lapic_hz = Some(match choice {
                RateChoice::Calibrated { rate, .. } => Some(rate.hz),
                RateChoice::Estimated(_) => None,
                RateChoice::Mismatch { .. } => {
                    set_failure("lapic-rate-mismatch");
                    None
                }
                // Only the PIT could time it: run the PIT/PIC tick itself,
                // which needs no calibration (#84 round 4).
                RateChoice::PitOnly(_) => {
                    set_failure("pit-only-clock");
                    None
                }
                RateChoice::Unknown => None,
            });
            LAPIC_HZ.store(lapic_hz.flatten().unwrap_or(0), Ordering::Release);
        }
        let lapic = lapic_hz.flatten();
        let chosen = plan(deadline, tsc_hz, lapic, tick_hz);
        let name = match chosen {
            Plan::TscDeadline { .. } => "tsc-deadline",
            Plan::Periodic { .. } => "periodic",
            Plan::Pit => {
                serial_println!(
                    "[ kernel ] timer: LAPIC tick NOT calibrated (tsc_hz={:?} deadline={} lapic_hz={:?}) - falling back to PIT/PIC",
                    tsc_hz,
                    deadline,
                    lapic
                );
                if failure() == "none" {
                    set_failure(if DEMOTIONS.load(Ordering::Acquire) != 0 {
                        "no-first-tick"
                    } else {
                        "uncalibrated"
                    });
                }
                return false;
            }
        };
        arm(chosen, first);
        first = false;
        let budget = first_tick_budget(budget_hz, tick_hz);
        let seen = ticked_within(budget);
        if seen == Some(u64::MAX) {
            disarm();
            DEMOTIONS.fetch_add(1, Ordering::AcqRel);
            serial_println!(
                "[ kernel ] timer: LAPIC tick mode={} gave NO TICK in {} TSC cycles - disarmed, demoting",
                name,
                budget
            );
            match chosen {
                Plan::TscDeadline { .. } => deadline = false,
                _ => lapic_hz = Some(None),
            }
            continue;
        }
        let (count, unit) = match chosen {
            Plan::TscDeadline { period } => (period, "period"),
            Plan::Periodic { initial_count } => (initial_count as u64, "initial_count"),
            Plan::Pit => (0, ""),
        };
        serial_println!(
            "[ kernel ] timer: LAPIC tick mode={} hz={} {}={} tsc_hz={} lapic_hz={} apic={} first_tick={}",
            name,
            tick_hz,
            unit,
            count,
            tsc_hz.unwrap_or(0),
            lapic.unwrap_or(0),
            apic::mode().name(),
            match seen {
                Some(c) => alloc::format!("{c}cyc"),
                None => alloc::string::String::from("unverified"),
            }
        );
        // `lapic_timer_demote` control: the armed mode was trusted without
        // its first tick. Name the resulting hang instead of leaving it to
        // QEMU's timeout.
        #[cfg(feature = "neo-test-lapic-timer-no-verify")]
        if seen.is_none() && no_tick_for(first_tick_budget(budget_hz, tick_hz) * 20) {
            crate::test_harness::lapic_timer_no_tick_fail();
        }
        return true;
    }
}

/// `lapic_timer_demote` control: whether `TICKS` stays still for `budget`
/// TSC cycles (IF=1).
#[cfg(feature = "neo-test-lapic-timer-no-verify")]
fn no_tick_for(budget: u64) -> bool {
    use crate::arch::x86_64::timer::TICKS;
    let ticks = TICKS.load(Ordering::Acquire);
    let t0 = rdtsc();
    while rdtsc().wrapping_sub(t0) < budget {
        if TICKS.load(Ordering::Acquire) != ticks {
            return false;
        }
        core::hint::spin_loop();
    }
    true
}
