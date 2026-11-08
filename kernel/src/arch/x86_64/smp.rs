// SPDX-License-Identifier: MPL-2.0
//! SMP application-processor bring-up via the Limine MP protocol.
//!
//! Sub-commit 2 brings APs from firmware-parked state into their per-CPU
//! scheduler bootstrap loops. APs do not receive routed user work yet.

use limine::mp::MpInfo;
use limine::request::MpRequest;

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::arch::halt_forever;
use crate::arch::x86_64::{self, apic, per_cpu};
use crate::mm;
use crate::sched;
use crate::serial_println;

/// Limine MP request. CP-68: x2APIC is requested (`MP_FLAG_X2APIC`), so
/// firmware that has x2APIC enabled or locked still boots: without the flag,
/// Limine must disable x2APIC before handoff and refuses to boot where it
/// cannot ("Kernel does not support x2APIC, but machine requires it"). Limine
/// enables x2APIC wherever the CPU supports it and reports the result in the
/// response's flags; `apic` drives that mode. The `xapic-only` kill-switch
/// keeps today's request (no flag): xAPIC wherever firmware allows it.
#[used]
#[unsafe(link_section = ".requests")]
pub static MP_REQUEST: MpRequest = MpRequest::new(MP_REQUEST_FLAGS);

#[cfg(not(feature = "xapic-only"))]
const MP_REQUEST_FLAGS: u64 = limine::mp::MP_FLAG_X2APIC;
#[cfg(feature = "xapic-only")]
const MP_REQUEST_FLAGS: u64 = 0;

/// Limine's MP response flag: x2APIC has been enabled.
const MP_RESPONSE_X2APIC: u32 = 1 << 0;

static BSP_APIC_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// AP readiness-barrier counter. Each AP increments this once it has finished ALL
/// per-CPU heap/buddy init (`slab::init_per_cpu` + its scheduler `Box`) and is
/// about to idle. `bring_up_aps` waits on it so the BSP does not run
/// `init_bsp`/`smoke_test` — which spawn IF=0, heap-allocating threads — while APs
/// are still contending the shared heap/buddy locks during their own init. The
/// race is invisible under QEMU's serialized TCG but deadlocks on real multi-core
/// silicon once the heap is large (the 768 MiB Stage-B heap + MAX_ORDER 16 widen
/// the lock-hold window). See `run_ap_bootstrap`.
static APS_READY: AtomicU32 = AtomicU32::new(0);

/// APs that could not run under the boot CPU's FPU save policy and stayed
/// offline (#52 review, item 1).
static APS_REJECTED: AtomicU32 = AtomicU32::new(0);

/// `xsave_ap_policy`: how many APs bringup counted as started.
#[cfg(feature = "neo-test-xsave-ap-policy")]
static APS_STARTED: AtomicU32 = AtomicU32::new(0);

/// `xsave_ap_policy`: (APs counted as started, APs that signalled ready).
#[cfg(feature = "neo-test-xsave-ap-policy")]
pub fn aps_started_ready_for_test() -> (u32, u32) {
    (
        APS_STARTED.load(Ordering::Acquire),
        APS_READY.load(Ordering::Acquire),
    )
}

/// `xsave_ap_policy`: whether `cpu_id` is in the registry IPIs and
/// `cpu_stop::stop_others` target.
#[cfg(feature = "neo-test-xsave-ap-policy")]
pub fn cpu_registered_for_test(cpu_id: u32) -> bool {
    apic::cpu_registered(cpu_id)
}

/// `xsave_ap_policy`: how many APs stayed offline for the FPU save policy.
#[cfg(feature = "neo-test-xsave-ap-policy")]
pub fn aps_rejected_for_test() -> u32 {
    APS_REJECTED.load(Ordering::Acquire)
}

/// Called by each AP from `run_ap_bootstrap` once it has completed per-CPU init,
/// releasing one count of the BSP's `bring_up_aps` readiness barrier.
pub fn signal_ap_ready() {
    APS_READY.fetch_add(1, Ordering::Release);
}

/// Total CPUs Limine reports (BSP + APs), clamped to `MAX_CPUS`; 1 if the MP
/// response is unavailable. Ungated (unlike `reported_cpu_count`, which is
/// scoped to specific test features) so the OT-34 fork-join can size its
/// partition count to the real core count at runtime, and the AVX2 census can
/// tell when every CPU has reported.
#[cfg(any(
    feature = "threaded-matmul",
    feature = "freeze-watchdog",
    feature = "phase9-quantized-matmul",
    feature = "phase9-real-quant"
))]
pub fn online_cpu_count() -> usize {
    MP_REQUEST
        .response()
        .map(|response| response.cpus().len())
        .unwrap_or(1)
        .min(per_cpu::MAX_CPUS)
}

#[cfg(any(
    feature = "kernel-sysstat",
    feature = "neo-test-irqsafe-mutex-bench",
    feature = "neo-test-phase2-perf-proxy-baseline",
    feature = "neo-test-ai-latency-oracle",
    feature = "neo-test-inference-preemptible-liveness"
))]
pub fn reported_cpu_count() -> usize {
    MP_REQUEST
        .response()
        .map(|response| response.cpus().len())
        .unwrap_or(1)
}

/// Initialize the BSP local APIC without starting APs.
///
/// Real PCI devices can deliver MSI/MSI-X interrupts during early block probe.
/// Their IDT handlers must be able to acknowledge with a local-APIC EOI even
/// though AP startup still waits until later in the boot sequence.
pub fn init_bsp_apic() {
    let response = match MP_REQUEST.response() {
        Some(response) => response,
        None => {
            serial_println!(
                "[ kernel ] smp: no MP response from Limine - BSP local APIC unavailable"
            );
            return;
        }
    };

    init_bsp_apic_once(response.bsp_lapic_id, response.flags);
}

fn init_bsp_apic_once(bsp_lapic_id: u32, response_flags: u32) {
    if BSP_APIC_INITIALIZED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        apic::init_bsp(bsp_lapic_id, response_flags & MP_RESPONSE_X2APIC != 0);
    }
}

/// Bring up secondary processors. Called by the BSP after `mm::init()` and
/// before `sched::init_bsp()`. APs construct their own per-CPU schedulers from
/// their terminal bootstrap step; they do not depend on BSP scheduler state.
pub fn bring_up_aps() {
    let response = match MP_REQUEST.response() {
        Some(response) => response,
        None => {
            serial_println!("[ kernel ] smp: no MP response from Limine - single-CPU boot");
            return;
        }
    };

    let bsp_lapic_id = response.bsp_lapic_id;
    init_bsp_apic_once(bsp_lapic_id, response.flags);

    let cpus = response.cpus();
    serial_println!(
        "[ kernel ] smp: Limine reports {} cpu(s); bsp_lapic_id={}",
        cpus.len(),
        bsp_lapic_id
    );

    let mut next_cpu_id = 1u32;
    let mut started = 0u32;
    for cpu in cpus {
        if cpu.lapic_id == bsp_lapic_id {
            continue;
        }

        let cpu_id = next_cpu_id;
        next_cpu_id += 1;
        if cpu_id as usize >= per_cpu::MAX_CPUS {
            serial_println!(
                "[ kernel ] smp: skipping cpu_id={} lapic_id={} - exceeds MAX_CPUS={}",
                cpu_id,
                cpu.lapic_id,
                per_cpu::MAX_CPUS
            );
            continue;
        }
        // Jarvis note 3: in xAPIC mode an ID above 255 cannot be named by an
        // IPI (reschedule, shootdown); never start a CPU nothing can reach.
        if !apic::ipi_addressable(cpu.lapic_id) {
            serial_println!(
                "[ kernel ] smp: skipping cpu_id={} lapic_id={} - above 255, not addressable in {} mode",
                cpu_id,
                cpu.lapic_id,
                apic::mode().name()
            );
            continue;
        }

        apic::register_lapic_id(cpu_id, cpu.lapic_id);
        serial_println!(
            "[ kernel ] smp: starting cpu_id={} lapic_id={}",
            cpu_id,
            cpu.lapic_id
        );
        // OT-41 / CP-29: serialize AP bringup. Start THIS AP, then wait for it to
        // finish ALL per-CPU init (slab + scheduler Box — both buddy/heap users)
        // and signal `APS_READY` BEFORE starting the next AP. Concurrent AP init
        // deadlocks on real multi-core silicon (invisible under QEMU's serialized
        // TCG): (1) the APs contend the shared buddy/heap lock with a wide hold
        // window (768 MiB heap + MAX_ORDER 16), and (2) any TLB-shootdown IPI
        // raised during a heap op targets sibling APs still in per-CPU init with
        // IF=0 (init_traps_ap does not `sti`) — they cannot ACK, so the sender
        // spin-waits forever. What each guard actually covers (do NOT conflate —
        // AAG 2026-07-19): this serialization closes vector (1) ALONE — only one
        // AP allocates at a time, so there is no AP-vs-AP buddy-lock contention.
        // Vector (2) (shootdown to an IF=0 AP) is closed by OT-59's I-16
        // shootdown-ready mask (`shootdown_ready_cpus_mask`), NOT by this
        // ordering: `signal_ap_ready()` is called just BEFORE `enable_interrupts()`,
        // so a just-signaled AP still has an IF=0 tail — but it isn't in the
        // shootdown-ready mask until it `mark_tlb_shootdown_ready()` after `sti`,
        // so no shootdown can target it. Keep BOTH guards: removing the I-16 mask
        // on the belief that serialization covers the IF=0 case would reopen
        // vector (2). (Degrade path below is diagnostic-preserving but does NOT
        // recover a boot if a wedged AP holds the global buddy lock.)
        const AP_READY_SPIN_LIMIT: u64 = 4_000_000_000;
        let expected = started + 1;
        // The rejection baseline is read BEFORE `bootstrap` publishes the AP's
        // entry: a rejecting AP increments APS_REJECTED as its first act, which
        // can land before any load the BSP makes after `bootstrap`. Read late,
        // the rejection is lost: the BSP waits out the spin limit and counts
        // the AP as started (#57 re-check, relay 274). Serialized bringup means
        // no other AP is in flight, so the next increment is this AP's.
        let rejected_at_launch = APS_REJECTED.load(Ordering::Acquire);
        cpu.bootstrap(ap_entry, cpu_id as u64);
        started += 1;
        // `xsave_ap_policy`: hold the BSP here until this AP has decided, so
        // the window the late read loses is always open (deterministic).
        #[cfg(feature = "neo-test-xsave-ap-policy")]
        for _ in 0..AP_READY_SPIN_LIMIT {
            if APS_REJECTED.load(Ordering::Acquire) != rejected_at_launch
                || APS_READY.load(Ordering::Acquire) >= expected
            {
                break;
            }
            core::hint::spin_loop();
        }
        #[cfg(not(feature = "neo-test-xsave-ap-policy-late-sample"))]
        let rejected_before = rejected_at_launch;
        // The revert-check: the baseline read after `bootstrap`, as before.
        #[cfg(feature = "neo-test-xsave-ap-policy-late-sample")]
        let rejected_before = APS_REJECTED.load(Ordering::Acquire);
        let mut spins: u64 = 0;
        while APS_READY.load(Ordering::Acquire) < expected
            && APS_REJECTED.load(Ordering::Acquire) == rejected_before
            && spins < AP_READY_SPIN_LIMIT
        {
            core::hint::spin_loop();
            spins += 1;
        }
        if APS_REJECTED.load(Ordering::Acquire) != rejected_before {
            // It halted in `ap_entry` and never signals ready: not started,
            // and no longer a CPU anything may send an IPI or NMI to.
            started -= 1;
            apic::unregister_lapic_id(cpu_id);
            serial_println!(
                "[ kernel ] smp: cpu_id={} lapic_id={} left offline - it cannot run under the FPU save policy",
                cpu_id,
                cpu.lapic_id
            );
        } else if APS_READY.load(Ordering::Acquire) < expected {
            serial_println!(
                "[ kernel ] smp: cpu_id={} did not signal ready within limit - proceeding (AP degraded)",
                cpu_id
            );
        }
    }

    serial_println!(
        "[ kernel ] smp: started {} AP(s) (serialized bringup)",
        started
    );
    #[cfg(feature = "neo-test-xsave-ap-policy")]
    APS_STARTED.store(started, Ordering::Release);

    // Backstop barrier: with serialized bringup every started AP has already
    // signaled `APS_READY` by here, so this is normally a no-op. Kept to emit the
    // ready-count line and to fail-safe (warn + proceed degraded) rather than brick
    // the boot if an AP was skipped/degraded above.
    let mut spins: u64 = 0;
    const BARRIER_LIMIT: u64 = 4_000_000_000;
    while APS_READY.load(Ordering::Acquire) < started && spins < BARRIER_LIMIT {
        core::hint::spin_loop();
        spins += 1;
    }
    let ready = APS_READY.load(Ordering::Acquire);
    if ready >= started {
        serial_println!(
            "[ kernel ] smp: AP ready-barrier cleared ({}/{} AP(s) up)",
            ready,
            started
        );
    } else {
        serial_println!(
            "[ kernel ] smp: AP ready-barrier TIMEOUT - only {}/{} AP(s) signaled; an AP is stuck in per-CPU init - proceeding degraded",
            ready,
            started
        );
    }
}

/// Limine AP handoff target. Must never return.
unsafe extern "C" fn ap_entry(info: &MpInfo) -> ! {
    // CR0/CR4 are CPU-local. Enable SSE/SMEP/SMAP before this AP executes any
    // meaningful Rust path that might use compiler-emitted SIMD or user-access
    // policy state.
    // #52 review item 1: an AP that cannot run under the boot CPU's FPU save
    // policy stays offline (it never changes the policy). It halts before it
    // touches any shared state, and the bringup loop moves on.
    // SAFETY: this AP owns its CR0/CR4; nothing else runs on it yet.
    if !unsafe { x86_64::enable_sse() } {
        APS_REJECTED.fetch_add(1, Ordering::Release);
        halt_forever();
    }

    let cpu_id = info.extra_argument() as u32;
    if cpu_id as usize >= per_cpu::MAX_CPUS {
        serial_println!(
            "[ kernel ] smp: AP invalid cpu_id={} (lapic_id={}, processor_id={}) - halting",
            cpu_id,
            info.lapic_id,
            info.processor_id
        );
        halt_forever();
    }

    per_cpu::init_ap(cpu_id);

    let observed = per_cpu::current_cpu_id();
    if observed != cpu_id {
        serial_println!(
            "[ kernel ] smp: AP cpu_id={} sentinel MISMATCH (gs:[24] reads {}) - halting",
            cpu_id,
            observed
        );
        halt_forever();
    }

    x86_64::init_traps_ap(cpu_id);
    mm::slab::init_per_cpu(cpu_id);

    serial_println!(
        "[ kernel ] smp: AP cpu_id={} ready (lapic_id={}, processor_id={})",
        cpu_id,
        info.lapic_id,
        info.processor_id
    );

    sched::run_ap_bootstrap(cpu_id);
}
