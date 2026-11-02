// SPDX-License-Identifier: MPL-2.0
//! OT-90 P3a (L1.4): the clean shutdown before power-off or restart
//! (`power-clean`). Design: `docs/THINK-2026-10-27-ot90-shutdown-restart.md`
//! §5. It runs in its own kernel thread ([`start`]), never in a caller's
//! syscall frame, and every step has a bound; a step that overruns is logged
//! and the sequence goes on, so shutdown never hangs on one:
//!
//! - **K1:** terminate every user process but the caller's, and reap them,
//!   for at most [`K1_TICKS`]; the survivors are logged by pid.
//! - **K2:** every mounted filesystem freezes its writes, drains its cache
//!   with a no-progress bound of [`K2_STALL_TICKS`], and stops its writeback
//!   worker (`fs::shutdown_mounts`). Each mount's outcome is **clean** or
//!   **unclean** (with the dirty count), logged and never retried into a
//!   hang. Only writable NeoFS mounts drain, and they write only blocks the
//!   write guard admitted: a disk the guard hasn't armed (the Zenbook's
//!   Windows NVMe) is neither written nor flushed (decision D1).
//! - **K3:** flush the log sink.
//! - **K7:** P2's S5 write or reset chain (`acpi::power`).
//!
//! - **K4** (`power-stop-cpus`, P3b): stop every other CPU (`cpu_stop`): the
//!   STOP IPI, then an NMI for any that didn't park in 100 ms. From here the
//!   kernel prints to the UART only and takes no shared lock.
//! - **K5** (`power-stop-cpus`): stop this CPU's LAPIC tick and mask every
//!   I/O APIC entry (prepared before K4: no mapping after it).
//!
//! - **K6** (`power-quiesce`): stop each guard-armed disk's controller,
//!   clear every function's Bus Master Enable, then turn the IOMMU off
//!   (`drivers::quiesce`; planned before K4).
//!
//! Without `power-stop-cpus`, P2's paths run with the other CPUs live, as
//! they already do.
//!
//! **The whole of K1-K3 is bounded too** (Chad #199 item 2). K2's bound is
//! checked between drain passes, so a block request whose completion never
//! comes (a device that accepts a write and goes quiet) would hold K2 in its
//! wait for good; so would the writeback worker's join, an `fsync` holding
//! the drain lock in such a wait, or K3's sink lock and its write. So K1-K3
//! run in their own kernel thread (`power-clean`), and the shutdown thread
//! waits for it for at most [`DEADLINE_TICKS`]. Past that it abandons the
//! work where it stands, logs the step it was in, and goes on to K4 (or
//! straight to K7 without `power-stop-cpus`). The abandoned thread keeps
//! whatever locks it holds; nothing after the deadline takes one of them,
//! and K4 parks its CPU if it runs.
//!
//! **After K1-K3, done or abandoned, the log sink is closed**
//! (`log::close_sink`) before the power path runs: K3 was its last flush,
//! and nothing after it locks the sink or writes its disk. P2's power-off
//! and restart print and call `log::flush()` themselves, with no deadline of
//! their own; a sink disk that stopped answering after K3 would otherwise
//! hold them for good (#267, Chad 3b). The UART still gets every line.
//!
//! **Lock discipline.** No step here holds a lock across another: K1 calls
//! `terminate` and `reap` one process at a time, K2 releases the mount table
//! before it drains, and every wait yields (interrupts on, as P2's long waits
//! need, CP-64).

use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use crate::sched::process::{self, ProcessId};

/// K1's bound: 2 s of timer ticks (100 Hz).
pub const K1_TICKS: u64 = 200;
/// K2's no-progress bound per mount: 5 s.
pub const K2_STALL_TICKS: u64 = 500;
/// The bound on K1-K3 together: 20 s. Past it, K7 runs without them.
pub const DEADLINE_TICKS: u64 = 2_000;

/// What the shutdown ends in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    PowerOff = 1,
    Restart = 2,
}

static KIND: AtomicU8 = AtomicU8::new(0);
/// The pid K1 spares (the caller), or 0 for none.
static KEEP: AtomicU32 = AtomicU32::new(0);
/// The step `power-clean` is in ([`Step`]), for the shutdown thread.
static STEP: AtomicU8 = AtomicU8::new(Step::NotStarted as u8);

/// Where K1-K3 stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Step {
    NotStarted = 0,
    K1 = 1,
    K2 = 2,
    K3 = 3,
    Done = 4,
}

impl Step {
    fn now() -> Self {
        match STEP.load(Ordering::Acquire) {
            1 => Self::K1,
            2 => Self::K2,
            3 => Self::K3,
            4 => Self::Done,
            _ => Self::NotStarted,
        }
    }
}

fn ticks() -> u64 {
    crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed)
}

/// Whether a shutdown has started.
#[allow(dead_code)] // `SYS_POWER`'s EBUSY (`power-cap`)
pub fn running() -> bool {
    KIND.load(Ordering::Acquire) != 0
}

/// Start the shutdown in its own kernel thread, sparing `keep` in K1. The
/// thread ends in S5 or a reset; the caller carries on until K1 reaches it
/// (or forever, when it is the spared one). Returns whether this call
/// started it: a second call while one runs does nothing and returns false.
pub fn start(kind: Kind, keep: Option<ProcessId>) -> bool {
    if KIND
        .compare_exchange(0, kind as u8, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    KEEP.store(keep.map_or(0, |p| p.0), Ordering::Release);
    let id = crate::sched::kthread::spawn_detached("power-shutdown", |_stop| run());
    crate::serial_println!(
        "[ power ] shutdown: started kind={:?} keep={} (kthread slot={})",
        kind,
        KEEP.load(Ordering::Acquire),
        id.slot()
    );
    true
}

fn run() -> ! {
    let kind = match KIND.load(Ordering::Acquire) {
        2 => Kind::Restart,
        _ => Kind::PowerOff,
    };
    let keep = match KEEP.load(Ordering::Acquire) {
        0 => None,
        pid => Some(ProcessId(pid)),
    };
    let start = ticks();
    crate::sched::kthread::spawn_detached("power-clean", move |_stop| {
        clean(kind, keep);
        crate::sched::kthread::KthreadExitState::Success
    });
    let abandoned = loop {
        let step = Step::now();
        if step == Step::Done {
            break None;
        }
        if ticks().saturating_sub(start) > deadline_ticks() {
            break Some(step);
        }
        crate::sched::yield_now();
    };
    // The stuck-sink legs: the sink's disk stops answering right after K3.
    #[cfg(any(
        feature = "neo-test-ot90-stuck-sink-off",
        feature = "neo-test-ot90-stuck-sink-restart"
    ))]
    {
        crate::drivers::block::virtio_blk::debug_set_completion_hold(true);
        crate::serial_println!("[ power ] test: the sink's disk stops completing requests");
    }
    // Nothing below may touch the sink: its lock may be held by the
    // abandoned thread, and its disk may never answer again. The control
    // (`neo-test-ot90-stuck-sink-no-close`) leaves it open and must hang.
    if !cfg!(feature = "neo-test-ot90-stuck-sink-no-close") {
        crate::log::close_sink();
    }
    if let Some(step) = abandoned {
        crate::serial_println!(
            "[ power ] shutdown: deadline passed in {:?} after {} ticks; going on without it",
            step,
            ticks().saturating_sub(start)
        );
    }
    #[cfg(feature = "power-stop-cpus")]
    stop_world();
    match kind {
        Kind::PowerOff => super::power::power_off(),
        Kind::Restart => super::power::restart(),
    }
}

/// [`DEADLINE_TICKS`]. The stuck-disk scenario uses 4 s (TCG's ticks run
/// slow while the disk is stuck: 20 s of them took about two minutes), and
/// its control has none (the run must then hang).
fn deadline_ticks() -> u64 {
    if cfg!(feature = "neo-test-ot90-stuck-disk-no-deadline") {
        u64::MAX
    } else if cfg!(feature = "neo-test-ot90-stuck-disk") {
        400
    } else {
        DEADLINE_TICKS
    }
}

/// K1-K3, in `power-clean`.
fn clean(kind: Kind, keep: Option<ProcessId>) {
    STEP.store(Step::K1 as u8, Ordering::Release);
    let survivors = k1_terminate(keep);
    let mut clean = survivors == 0;
    STEP.store(Step::K2 as u8, Ordering::Release);
    for (name, report) in crate::fs::shutdown_mounts(K2_STALL_TICKS) {
        let mount_clean = report.clean();
        clean &= mount_clean;
        crate::serial_println!(
            "[ power ] K2 mount=/{} clean={} writable={} writers_settled={} drained={} dirty={} drain_error={} worker_exit={}",
            name,
            u8::from(mount_clean),
            u8::from(report.writable),
            u8::from(report.writers_settled),
            report.drained_blocks,
            report.dirty_blocks,
            u8::from(report.drain_error),
            report.worker_exit
        );
    }
    crate::serial_println!(
        "[ power ] shutdown: kind={:?} clean={} survivors={}",
        kind,
        u8::from(clean),
        survivors
    );
    STEP.store(Step::K3 as u8, Ordering::Release);
    crate::log::flush();
    crate::serial_println!("[ power ] K3 log flushed");
    STEP.store(Step::Done as u8, Ordering::Release);
}

/// K4 and K5. Everything they need is read and mapped first, while the other
/// CPUs still run; then TLB shootdowns are closed for good (CP-64), and from
/// there interrupts stay off on this CPU until K7, nothing is allocated or
/// mapped, and no shared lock is taken (the UART is written lock-free, the
/// power plan read from its copy). With holdouts (CPUs that took neither the
/// STOP IPI nor the NMI) K5 is skipped: its masking assumes one CPU at the
/// I/O APIC and PCI config ports, and K7's power-off or reset ends every CPU
/// anyway (`cpu_stop`, Holdouts).
#[cfg(feature = "power-stop-cpus")]
fn stop_world() {
    use crate::arch::x86_64::cpu_stop;
    let timer = super::power::stop_timer();
    // K6's plan is read now, while every CPU runs (it takes driver locks
    // and scans PCI). It is never dropped: K7 doesn't return.
    #[cfg(feature = "power-quiesce")]
    let quiesce = crate::drivers::quiesce::Plan::prepare();
    let ioapics =
        super::madt(super::power::rsdp()).map(|m| crate::arch::x86_64::ioapic::MaskPlan::new(&m));
    crate::serial_println!("[ power ] K4 start");
    // `ot90_stop_shootdown`: start the AP's sender and let it run, so one is
    // likely in flight at the close (interrupts are still on here).
    #[cfg(feature = "neo-test-ot90-stop-shootdown")]
    {
        use core::sync::atomic::Ordering;
        crate::test_harness::OT90_STORM_GO.store(true, Ordering::Release);
        let start = ticks();
        while crate::test_harness::OT90_STORM_SENT.load(Ordering::Acquire) < 50
            && ticks().saturating_sub(start) < 500
        {
            core::hint::spin_loop();
        }
        crate::serial_println!(
            "[ power ] test: storm sent={} before the close",
            crate::test_harness::OT90_STORM_SENT.load(Ordering::Acquire)
        );
    }
    // Waits out any shootdown in flight (acknowledging it), then IF=0 for good.
    crate::mm::tlb_shootdown::close_for_stop();
    #[cfg(not(feature = "neo-test-ot90-stop-wedged"))]
    let report = cpu_stop::stop_others(|us, done| super::power::spin_us(timer.as_ref(), us, done));
    // `ot90_stop_wedged`: a PM timer that never advances; only `done` (and
    // with it K4's poll count) can end a wait.
    #[cfg(feature = "neo-test-ot90-stop-wedged")]
    let report = {
        let _ = timer;
        cpu_stop::stop_others(|_, done| {
            loop {
                if done() {
                    return true;
                }
            }
        })
    };
    crate::drivers::console::serial::enter_stopped_world();
    crate::serial_println!(
        "[ power ] K4 targets={:#x} parked_by_ipi={:#x} parked_by_nmi={:#x} holdouts={:#x} shootdowns=closed",
        report.targets,
        report.parked_by_ipi,
        report.parked_by_nmi,
        report.holdouts
    );
    if report.holdouts != 0 {
        super::power::note_holdouts();
        crate::serial_println!(
            "[ power ] K5 skipped: holdouts={:#x} still run; straight to K7",
            report.holdouts
        );
        return;
    }
    #[cfg(feature = "lapic-timer")]
    crate::arch::x86_64::lapic_timer::disarm_for_shutdown();
    let masked = ioapics.map(|plan| plan.mask_all());
    let pci = crate::drivers::pci::mask_interrupts_for_stop();
    crate::serial_println!(
        "[ power ] K5 lapic_timer={} ioapic={} pci_functions={} msi_disabled={} msix_masked={} msi_left_on={}",
        if cfg!(feature = "lapic-timer") {
            "disarmed"
        } else {
            "none"
        },
        IoapicMasked(masked),
        pci.functions,
        pci.msi_disabled,
        pci.msix_masked,
        pci.left_on
    );
    #[cfg(feature = "power-quiesce")]
    quiesce.run(&|us, done| super::power::spin_us(timer.as_ref(), us, done));
}

/// K5's I/O APIC part of the log line: `masked=N`, or `no-madt` when the
/// firmware has no MADT (so there was nothing to mask by).
#[cfg(feature = "power-stop-cpus")]
struct IoapicMasked(Option<u32>);

#[cfg(feature = "power-stop-cpus")]
impl core::fmt::Display for IoapicMasked {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(n) => write!(f, "masked={n}"),
            None => write!(f, "no-madt"),
        }
    }
}

/// K1: terminate every user process but `keep`, reaping as they go, for at
/// most [`K1_TICKS`]. Returns how many outlived the bound (logged by pid).
fn k1_terminate(keep: Option<ProcessId>) -> usize {
    let start = ticks();
    let mut terminated = 0usize;
    loop {
        let pids: alloc::vec::Vec<ProcessId> = process::user_pids()
            .into_iter()
            .filter(|pid| Some(*pid) != keep)
            .collect();
        if pids.is_empty() {
            crate::serial_println!(
                "[ power ] K1 terminated={} survivors=0 ticks={}",
                terminated,
                ticks().saturating_sub(start)
            );
            return 0;
        }
        if ticks().saturating_sub(start) > K1_TICKS {
            crate::serial_println!(
                "[ power ] K1 bound passed: survivors={} pids={:?}",
                pids.len(),
                pids.iter().map(|p| p.0).collect::<alloc::vec::Vec<_>>()
            );
            return pids.len();
        }
        for pid in &pids {
            if process::terminate(*pid) == 0 {
                terminated += 1;
            }
        }
        crate::sched::reap();
        crate::sched::yield_now();
    }
}
