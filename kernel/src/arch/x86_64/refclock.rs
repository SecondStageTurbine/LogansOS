// SPDX-License-Identifier: MPL-2.0
//! Reference clocks for calibrating the LAPIC timer (`lapic-timer`, CP-69):
//! clocks whose rate is fixed by the platform and that do not depend on the
//! TSC or on each other.
//!
//! In order of preference:
//! - the HPET (address from the ACPI `HPET` table; its period is in its
//!   capabilities register), present on Intel client silicon even when the
//!   8254 is clock-gated;
//! - the ACPI PM timer (port from the FADT, 3.579545 MHz);
//! - PIT channel 2, polled one-shot (times out cleanly if the PIT is gated):
//!   a cross-check only, never the sole source of a rate (#84 round 4).
//!
//! `measure` reports how far any counter (the TSC, the LAPIC count) advanced
//! over about `WINDOW_NS` of a reference clock.

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};

use x86_64::PhysAddr;
use x86_64::instructions::port::Port;

use crate::acpi::timers_logic::{
    HPET_CAPS, HPET_COUNTER, HpetRegs, PM_TIMER_HZ, hpet_caps, hpet_hz, hpet_take_ownership,
    pit_one_shot_completed,
};
use crate::mm;

/// A reference clock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefClock {
    Hpet,
    PmTimer,
    Pit,
}

impl RefClock {
    #[allow(dead_code)] // the lapic_timer scenarios
    pub const fn name(self) -> &'static str {
        match self {
            RefClock::Hpet => "hpet",
            RefClock::PmTimer => "acpi-pm",
            RefClock::Pit => "pit",
        }
    }
}

/// How long one measurement lasts: 10 ms (also the PIT one-shot's length).
pub const WINDOW_NS: u64 = 10_000_000;
const PIT_HZ: u64 = 1_193_182;
/// Give up on a clock that has not advanced after this many TSC cycles
/// (about a second on any x86 this kernel runs on; no exact rate needed).
const TIMEOUT_TSC_CYCLES: u64 = 3_000_000_000;

fn rdtsc() -> u64 {
    // SAFETY: RDTSC has no side effects.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Before OUT2 goes LOW after programming, give up after this many TSC
/// cycles (a real PIT drives it low within a clock, under a microsecond).
const PIT_LOW_TIMEOUT_TSC_CYCLES: u64 = TIMEOUT_TSC_CYCLES / 1000;
/// A slower TSC than any x86 this kernel runs on (500 MHz). It bounds a PIT
/// window from below without knowing the TSC rate being calibrated.
const TSC_HZ_FLOOR: u64 = 500_000_000;
/// A PIT window shorter than this many TSC cycles lasted under half its
/// 10 ms at any TSC of at least `TSC_HZ_FLOOR`: refused (Jarvis R4, round 3).
const PIT_MIN_WINDOW_TSC_CYCLES: u64 = TSC_HZ_FLOOR * (WINDOW_NS / 2) / 1_000_000_000;
/// Two OUT2 polls further apart than this many TSC cycles (a tenth of the
/// window at `TSC_HZ_FLOOR`) mean the poller was held off and may have
/// missed the edge: the sample is refused (#84 round 4).
const PIT_MAX_POLL_GAP_TSC_CYCLES: u64 = TSC_HZ_FLOOR * (WINDOW_NS / 10) / 1_000_000_000;

/// The mapped HPET block, for `hpet_take_ownership`.
struct MmioHpet(u64);

impl HpetRegs for MmioHpet {
    fn read(&mut self, offset: usize) -> u64 {
        // SAFETY: a register of the HPET block the ACPI table names, mapped
        // uncached by `discover`; offsets come from the HPET spec.
        unsafe { read_volatile((self.0 as usize + offset) as *const u64) }
    }

    fn write(&mut self, offset: usize, value: u64) {
        // SAFETY: as in `read`; `hpet_take_ownership` writes only the
        // general configuration, comparator configuration and status
        // registers, in the order the HPET spec needs.
        unsafe { write_volatile((self.0 as usize + offset) as *mut u64, value) }
    }
}

/// The HPET block's virtual address (0: none), counter rate, 64-bit counter.
static HPET_VIRT: AtomicU64 = AtomicU64::new(0);
static HPET_HZ: AtomicU64 = AtomicU64::new(0);
static HPET_WIDE: AtomicBool = AtomicBool::new(false);
/// The PM timer port (0: none) and whether its counter is 32 bits wide.
static PM_PORT: AtomicU16 = AtomicU16::new(0);
static PM_WIDE: AtomicBool = AtomicBool::new(false);

/// Find and enable the HPET and the ACPI PM timer. Call once on the BSP
/// before `sched::init_bsp` (the LAPIC timer calibrates there).
pub fn discover(rsdp_phys: u64) {
    let hw = crate::acpi::timer_hardware(rsdp_phys);
    if let Some(phys) = hw.hpet_phys {
        let virt = mm::map_kernel_mmio_page(PhysAddr::new(phys & !0xfff)).as_u64() + (phys & 0xfff);
        // SAFETY: the HPET register block the ACPI table names, mapped
        // uncached above; the capabilities register is read-only.
        let caps = unsafe { read_volatile((virt as usize + HPET_CAPS) as *const u64) };
        match hpet_caps(caps) {
            Some((period_fs, wide)) => {
                // Jarvis R2: stop it, give IRQ0 back to the 8254, silence
                // every comparator firmware armed, then start the counter.
                let took = hpet_take_ownership(&mut MmioHpet(virt), caps);
                if took.was_enabled || took.was_legacy || took.comparators_quieted != 0 {
                    crate::serial_println!(
                        "[ kernel ] timer: hpet taken over from firmware (was enabled={} legacy-routing={} comparator irqs disabled={})",
                        took.was_enabled,
                        took.was_legacy,
                        took.comparators_quieted
                    );
                }
                HPET_HZ.store(hpet_hz(period_fs), Ordering::Release);
                HPET_WIDE.store(wide, Ordering::Release);
                HPET_VIRT.store(virt, Ordering::Release);
                crate::serial_println!(
                    "[ kernel ] timer: hpet at 0x{:x} hz={} counter={}-bit",
                    phys,
                    hpet_hz(period_fs),
                    if wide { 64 } else { 32 }
                );
            }
            None => {
                crate::serial_println!(
                    "[ kernel ] timer: hpet at 0x{:x} has an implausible period (caps=0x{:x}) - not used",
                    phys,
                    caps
                );
            }
        }
    }
    if let Some(pm) = hw.pm_timer {
        PM_WIDE.store(pm.wide, Ordering::Release);
        PM_PORT.store(pm.port, Ordering::Release);
    }
}

/// The reference clocks found, best first (the PIT is always listed: it is
/// only known to be gated once a measurement times out).
pub fn available() -> ([RefClock; 3], usize) {
    let mut out = [RefClock::Pit; 3];
    let mut n = 0;
    // `neo-test-lapic-timer-pit-only` (manual control): a platform with no
    // HPET and no PM timer, where the PIT must not calibrate alone.
    if cfg!(feature = "neo-test-lapic-timer-pit-only") {
        return (out, 1);
    }
    if HPET_VIRT.load(Ordering::Acquire) != 0 {
        out[n] = RefClock::Hpet;
        n += 1;
    }
    if PM_PORT.load(Ordering::Acquire) != 0 {
        out[n] = RefClock::PmTimer;
        n += 1;
    }
    out[n] = RefClock::Pit;
    (out, n + 1)
}

/// A free-running reference clock: (current count, counter mask, rate).
fn read_free_running(clock: RefClock) -> Option<(u64, u64, u64)> {
    match clock {
        RefClock::Hpet => {
            let virt = HPET_VIRT.load(Ordering::Acquire);
            if virt == 0 {
                return None;
            }
            let wide = HPET_WIDE.load(Ordering::Acquire);
            // SAFETY: the mapped HPET block; the main counter is read only.
            let count = unsafe { read_volatile((virt as usize + HPET_COUNTER) as *const u64) };
            let mask = if wide { u64::MAX } else { u32::MAX as u64 };
            Some((count & mask, mask, HPET_HZ.load(Ordering::Acquire)))
        }
        RefClock::PmTimer => {
            let port = PM_PORT.load(Ordering::Acquire);
            if port == 0 {
                return None;
            }
            // SAFETY: the PM timer port the FADT names; reading has no side
            // effect.
            let count = unsafe { Port::<u32>::new(port).read() } as u64;
            let mask = if PM_WIDE.load(Ordering::Acquire) {
                u32::MAX as u64
            } else {
                0x00ff_ffff
            };
            Some((count & mask, mask, PM_TIMER_HZ))
        }
        RefClock::Pit => None,
    }
}

/// Over about `window_ns` of `clock`, how far `sample` advanced: returns
/// `(delta, elapsed_ns)`, or `None` if the clock is absent or never
/// advanced (a gated PIT, a dead counter). `sample` must be non-decreasing
/// over the window. The PIT measures in back-to-back 10 ms one-shots.
pub fn measure(
    clock: RefClock,
    window_ns: u64,
    sample: &mut dyn FnMut() -> u64,
) -> Option<(u64, u64)> {
    if clock == RefClock::Pit {
        let shots = window_ns.div_ceil(WINDOW_NS).max(1);
        let s0 = sample();
        let mut elapsed = 0;
        for _ in 0..shots {
            elapsed += measure_pit(&mut || 0)?.1;
        }
        return Some((sample().wrapping_sub(s0), elapsed));
    }
    let (r0, mask, hz) = read_free_running(clock)?;
    if hz == 0 {
        return None;
    }
    let target = (hz as u128 * window_ns as u128 / 1_000_000_000) as u64;
    let s0 = sample();
    let t0 = rdtsc();
    loop {
        let (r, _, _) = read_free_running(clock)?;
        let ticks = r.wrapping_sub(r0) & mask;
        if ticks >= target {
            let s1 = sample();
            let ns = (ticks as u128 * 1_000_000_000 / hz as u128) as u64;
            return Some((s1.wrapping_sub(s0), ns));
        }
        if rdtsc().wrapping_sub(t0) > TIMEOUT_TSC_CYCLES {
            return None;
        }
        core::hint::spin_loop();
    }
}

/// PIT channel 2 as a 10 ms one-shot polled through port 0x61 (OUT2, bit 5).
fn measure_pit(sample: &mut dyn FnMut() -> u64) -> Option<(u64, u64)> {
    const COUNT: u16 = (PIT_HZ * WINDOW_NS / 1_000_000_000) as u16;
    let mut ctrl = Port::<u8>::new(0x61);
    let mut cmd = Port::<u8>::new(0x43);
    let mut ch2 = Port::<u8>::new(0x42);
    // SAFETY: legacy PIT/port-0x61 I/O on the BSP during single-threaded
    // boot; channel 0 is not touched.
    unsafe {
        let v = ctrl.read();
        ctrl.write((v & !0x02) | 0x01); // speaker off, gate 2 high
        cmd.write(0xb0); // channel 2, lobyte/hibyte, mode 0, binary
        ch2.write((COUNT & 0xff) as u8);
        ch2.write((COUNT >> 8) as u8);
    }
    let s0 = sample();
    // Jarvis R4: OUT2 must go LOW (programmed) and then HIGH (terminal
    // count), each on time, and the window not far too short.
    let completed = pit_one_shot_completed(
        // SAFETY: reading port 0x61 (system control port B) has no side
        // effect.
        &mut || unsafe { ctrl.read() },
        &mut rdtsc,
        PIT_LOW_TIMEOUT_TSC_CYCLES,
        PIT_MIN_WINDOW_TSC_CYCLES,
        TIMEOUT_TSC_CYCLES,
        PIT_MAX_POLL_GAP_TSC_CYCLES,
    );
    if !completed {
        return None;
    }
    let s1 = sample();
    Some((s1.wrapping_sub(s0), COUNT as u64 * 1_000_000_000 / PIT_HZ))
}
