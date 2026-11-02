// SPDX-License-Identifier: MPL-2.0
//! OT-90 P3b, K4 (`power-stop-cpus`): stop every other CPU before power-off
//! or restart (`docs/THINK-2026-10-27-ot90-shutdown-restart.md` §5).
//!
//! The shutdown thread, with interrupts off, sends the STOP IPI to every
//! other started CPU and waits up to [`STOP_WAIT_US`] for each to park. A CPU
//! parks in the handler: it acknowledges the interrupt (EOI first, CP-7),
//! marks itself parked and halts with interrupts off for good. One that never
//! takes the IPI (interrupts off for longer than the wait) gets an NMI, and
//! the NMI handler parks it the same way ([`park_from_nmi`]).
//!
//! **What a parked CPU can hold.** The STOP IPI is taken only with interrupts
//! on, so a CPU it parks holds no IRQ-safe lock (those are held with
//! interrupts off), and nothing it was doing with interrupts off is cut
//! short. It can hold a plain spin lock taken with interrupts on (the log
//! sink's, for one); a CPU the NMI parks can hold anything. So after K4 the
//! shutdown CPU takes no shared lock at all: it prints to the UART without
//! its lock (`serial::enter_stopped_world`), allocates nothing, maps nothing,
//! reads the power plan from a copy taken before K4, and sends no TLB
//! shootdown: K4 first closes shootdowns for good
//! (`tlb_shootdown::close_for_stop`, CP-64).
//!
//! **Holdouts.** A CPU that takes neither the STOP IPI nor the NMI (wedged in
//! an NMI handler, or never answering) is still running, so nothing after K4
//! may assume this is the only CPU. With holdouts the UART is still written
//! lock-free (bytes may interleave, nothing waits), K5's masking, which
//! assumes one CPU at the I/O APIC and PCI config ports, is skipped, and K7
//! goes straight to the power-off or reset, which ends every CPU. Each wait
//! is bounded twice: by the PM timer and by a poll count
//! ([`POLLS_PER_US`]), so a frozen timer can't hold K4.
//!
//! **Parked, not discarded** (THINK §10). A parked CPU's per-CPU state is
//! left alone, so a later resume path can restart it.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use crate::arch::x86_64::{apic, per_cpu};

/// How long K4 waits for the STOP IPI's parks before the NMI, and again for
/// the NMI's.
pub const STOP_WAIT_US: u64 = 100_000;

/// The independent deadline (review #6 item 2): each wait also ends after
/// `STOP_WAIT_US * POLLS_PER_US` polls, each with a write to the POST port
/// (about 1 µs on real hardware), so a PM timer that reads a constant can't
/// hold K4 forever. 4 polls per µs keeps the timed wait the one that ends
/// first on a working timer (a TCG vCPU polls fast) while bounding a frozen
/// one at well under a second.
const POLLS_PER_US: u64 = 4;

const NOT_PARKED: u8 = 0;
const PARKED_BY_IPI: u8 = 1;
const PARKED_BY_NMI: u8 = 2;

/// Set while K4 runs and after: the NMI handler parks quietly instead of
/// reporting a fault.
static STOPPING: AtomicBool = AtomicBool::new(false);
/// The CPU running the shutdown; it never parks.
static STOPPER: AtomicU32 = AtomicU32::new(u32::MAX);
static PARKED: [AtomicU8; per_cpu::MAX_CPUS] =
    [const { AtomicU8::new(NOT_PARKED) }; per_cpu::MAX_CPUS];

/// K4's outcome, one bit per CPU index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StopReport {
    /// The CPUs asked to stop (every started CPU but this one).
    pub targets: u64,
    pub parked_by_ipi: u64,
    pub parked_by_nmi: u64,
    /// Targets that parked neither way.
    pub holdouts: u64,
}

/// K4: stop every other started CPU. Call with interrupts off, from the
/// shutdown thread; `spin_us(us, done)` waits up to `us` microseconds for
/// `done` without taking a lock (the ACPI PM timer, `acpi::power`).
pub fn stop_others(spin_us: impl Fn(u64, &mut dyn FnMut() -> bool) -> bool) -> StopReport {
    let me = apic::cpu_from_own_lapic_id().unwrap_or(0);
    STOPPER.store(me, Ordering::Release);
    STOPPING.store(true, Ordering::Release);
    let mut report = StopReport::default();
    for cpu in 0..per_cpu::MAX_CPUS as u32 {
        if cpu != me && apic::cpu_registered(cpu) {
            report.targets |= 1 << cpu;
        }
    }
    for cpu in bits(report.targets) {
        apic::send_stop_ipi(cpu);
    }
    let all_parked = |targets: u64| bits(targets).all(|cpu| parked(cpu) != NOT_PARKED);
    let targets = report.targets;
    let wait = |spin_us: &dyn Fn(u64, &mut dyn FnMut() -> bool) -> bool| {
        let mut polls = 0u64;
        spin_us(STOP_WAIT_US, &mut || {
            if all_parked(targets) {
                return true;
            }
            polls += 1;
            // SAFETY: the POST port; a write there has no effect but time.
            unsafe { x86_64::instructions::port::Port::<u8>::new(0x80).write(0) };
            polls >= STOP_WAIT_US * POLLS_PER_US
        });
        all_parked(targets)
    };
    if !wait(&spin_us) {
        for cpu in bits(report.targets).filter(|&cpu| parked(cpu) == NOT_PARKED) {
            apic::send_nmi_ipi(cpu);
        }
        wait(&spin_us);
    }
    for cpu in bits(report.targets) {
        match parked(cpu) {
            PARKED_BY_IPI => report.parked_by_ipi |= 1 << cpu,
            PARKED_BY_NMI => report.parked_by_nmi |= 1 << cpu,
            _ => report.holdouts |= 1 << cpu,
        }
    }
    report
}

/// The CPU indices of `mask`'s set bits.
pub fn bits(mask: u64) -> impl Iterator<Item = u32> {
    (0..64u32).filter(move |&cpu| mask & (1 << cpu) != 0)
}

fn parked(cpu: u32) -> u8 {
    PARKED[cpu as usize].load(Ordering::Acquire)
}

/// The STOP IPI's handler (vector `apic::STOP_IPI_VECTOR`). Never returns.
pub extern "x86-interrupt" fn stop_ipi(_frame: x86_64::structures::idt::InterruptStackFrame) {
    // CP-7: acknowledge first, so the LAPIC isn't left in service.
    apic::end_of_interrupt();
    #[cfg(feature = "neo-test-ot90-stop-holdout")]
    if holdout_test_ignores_ipi() {
        return;
    }
    #[cfg(feature = "neo-test-ot90-stop-wedged")]
    if is_highest_cpu() {
        return;
    }
    park(PARKED_BY_IPI);
}

/// The NMI handler's first step: during a shutdown, park quietly (this CPU
/// never took the STOP IPI). Returns only when no shutdown is stopping CPUs,
/// or on the stopping CPU itself.
pub fn park_from_nmi() {
    if !STOPPING.load(Ordering::Acquire) {
        return;
    }
    #[cfg(feature = "neo-test-ot90-stop-wedged")]
    if is_highest_cpu() {
        wedged_in_nmi();
    }
    match apic::cpu_from_own_lapic_id() {
        Some(cpu) if cpu != STOPPER.load(Ordering::Acquire) => park(PARKED_BY_NMI),
        _ => {}
    }
}

fn park(by: u8) -> ! {
    if let Some(cpu) = apic::cpu_from_own_lapic_id() {
        PARKED[cpu as usize].store(by, Ordering::Release);
    }
    loop {
        // SAFETY: interrupts stay off; only an NMI, INIT or SMI ends a `hlt`
        // here, and each comes back to it.
        unsafe { core::arch::asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

/// `ot90_stop_wedged`: whether this is the highest-numbered started CPU.
#[cfg(feature = "neo-test-ot90-stop-wedged")]
fn is_highest_cpu() -> bool {
    let Some(me) = apic::cpu_from_own_lapic_id() else {
        return false;
    };
    let highest = (0..per_cpu::MAX_CPUS as u32)
        .rev()
        .find(|&cpu| apic::cpu_registered(cpu))
        .unwrap_or(0);
    me == highest && me != STOPPER.load(Ordering::Acquire)
}

/// `ot90_stop_wedged`: the highest AP, in its NMI handler, takes the UART
/// lock and never lets go or acknowledges (an NMI-blocked CPU wedged
/// mid-dump, the review's case): a holdout holding `SERIAL`.
#[cfg(feature = "neo-test-ot90-stop-wedged")]
fn wedged_in_nmi() -> ! {
    crate::drivers::console::serial::hold_forever_for_test();
    loop {
        core::hint::spin_loop();
    }
}

/// `ot90_stop_holdout`: the highest-numbered AP ignores the STOP IPI once, as
/// a CPU with interrupts off for the whole wait would, so the NMI must park
/// it.
#[cfg(feature = "neo-test-ot90-stop-holdout")]
fn holdout_test_ignores_ipi() -> bool {
    static IGNORED: AtomicBool = AtomicBool::new(false);
    let Some(me) = apic::cpu_from_own_lapic_id() else {
        return false;
    };
    let highest = (0..per_cpu::MAX_CPUS as u32)
        .rev()
        .find(|&cpu| apic::cpu_registered(cpu))
        .unwrap_or(0);
    me == highest && !IGNORED.swap(true, Ordering::AcqRel)
}
