// SPDX-License-Identifier: MPL-2.0
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

use x86_64::VirtAddr;
use x86_64::registers::control::Cr3Flags;
use x86_64::registers::model_specific::{GsBase, KernelGsBase};
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::tss::TaskStateSegment;

use crate::mm::tlb_shootdown::ShootdownPayload;
use crate::serial_println;

/// Per-CPU state slots reserved for Phase 2 SMP.
pub const MAX_CPUS: usize = 64;

#[allow(dead_code)]
pub const KERNEL_RSP_OFFSET: usize = 0;
#[allow(dead_code)]
pub const USER_RSP_SAVE_OFFSET: usize = 8;
#[allow(dead_code)]
pub const SLAB_STATE_OFFSET: usize = 16;
#[allow(dead_code)]
pub const CPU_ID_OFFSET: usize = 24;

/// Per-CPU data block addressed through IA32_GS_BASE in kernel mode.
///
/// Offsets 0 and 8 are consumed by syscall entry/exit assembly. Keep the
/// layout in sync with `kernel/src/arch/x86_64/syscall_entry.S`.
#[repr(C, align(64))]
pub struct PerCpu {
    pub kernel_rsp: UnsafeCell<u64>,
    pub user_rsp_save: UnsafeCell<u64>,
    pub slab_state: UnsafeCell<*mut ()>,
    pub cpu_id: UnsafeCell<u32>,
    _pad1: u32,
    pub active_pmap_cr3: AtomicU64,
    pub scheduler_ptr: AtomicPtr<()>,
    pub tss_ptr: AtomicPtr<TaskStateSegment>,
    pub current_thread_id: AtomicU64,
    pub pending_shootdown: AtomicPtr<ShootdownPayload>,
    pub tlb_shootdown_ready: AtomicBool,
    /// OT-61 C1.3b: the pid of the thread this CPU last switched in, stored
    /// under its scheduler lock on every switch. A process retire reads it on
    /// every CPU to send the reschedule IPI to the ones running a sibling
    /// (CP-86). 0 (the kernel) until the first switch.
    #[cfg(feature = "browser-runtime")]
    pub current_pid: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub cr3_writes_skipped: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub cr3_writes_performed: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_ipis_received: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_request_sent_tsc: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_last_ack_cycles: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_max_ack_cycles: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_retry_ipis_sent: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_timeouts: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_self_invalidations: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_full_flushes: AtomicU64,
    #[cfg(feature = "neo-test-instrumentation")]
    pub shootdown_invlpg_pages: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_context_switches: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_dispatches: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_ready_queue_depth: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_ready_queue_high_water: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_run_ticks: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_idle_ticks: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_timer_preemptions: AtomicU64,
    #[cfg(feature = "kernel-observability")]
    pub sched_voluntary_yields: AtomicU64,
}

unsafe impl Sync for PerCpu {}

impl PerCpu {
    #[allow(clippy::declare_interior_mutable_const)] // per-CPU area template — UnsafeCell is the point
    pub const EMPTY: Self = Self {
        kernel_rsp: UnsafeCell::new(0),
        user_rsp_save: UnsafeCell::new(0),
        slab_state: UnsafeCell::new(core::ptr::null_mut()),
        cpu_id: UnsafeCell::new(0),
        _pad1: 0,
        active_pmap_cr3: AtomicU64::new(0),
        scheduler_ptr: AtomicPtr::new(core::ptr::null_mut()),
        tss_ptr: AtomicPtr::new(core::ptr::null_mut()),
        current_thread_id: AtomicU64::new(u64::MAX),
        pending_shootdown: AtomicPtr::new(core::ptr::null_mut()),
        tlb_shootdown_ready: AtomicBool::new(false),
        #[cfg(feature = "browser-runtime")]
        current_pid: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        cr3_writes_skipped: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        cr3_writes_performed: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_ipis_received: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_request_sent_tsc: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_last_ack_cycles: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_max_ack_cycles: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_retry_ipis_sent: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_timeouts: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_self_invalidations: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_full_flushes: AtomicU64::new(0),
        #[cfg(feature = "neo-test-instrumentation")]
        shootdown_invlpg_pages: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_context_switches: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_dispatches: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_ready_queue_depth: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_ready_queue_high_water: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_run_ticks: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_idle_ticks: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_timer_preemptions: AtomicU64::new(0),
        #[cfg(feature = "kernel-observability")]
        sched_voluntary_yields: AtomicU64::new(0),
    };
}

pub static PER_CPU: [PerCpu; MAX_CPUS] = [const { PerCpu::EMPTY }; MAX_CPUS];

/// Read the current CPU's id from gs:[24]. Single instruction.
///
/// Caller must have GS-base wired to the calling CPU's PerCpu slot; safe in
/// any kernel context after `init_bsp()` (BSP) or `init_ap()` (AP).
#[inline]
pub fn current_cpu_id() -> u32 {
    let cpu_id: u32;
    unsafe {
        core::arch::asm!(
            "mov {0:e}, gs:[24]",
            out(reg) cpu_id,
            options(nostack, readonly, preserves_flags),
        );
    }
    cpu_id
}

/// Return a reference to the current CPU's PerCpu slot.
#[inline]
pub fn current_per_cpu() -> &'static PerCpu {
    &PER_CPU[current_cpu_id() as usize]
}

#[inline]
pub fn encode_cr3(frame: PhysFrame, flags: Cr3Flags) -> u64 {
    frame.start_address().as_u64() | flags.bits()
}

#[inline]
#[allow(dead_code)]
pub fn current_thread_id() -> u64 {
    current_per_cpu().current_thread_id.load(Ordering::Relaxed)
}

#[inline]
pub fn set_current_thread_id(thread_id: u64) {
    current_per_cpu()
        .current_thread_id
        .store(thread_id, Ordering::Relaxed);
}

/// OT-61 C1.3b: publish the pid of the thread this CPU is switching in
/// ([`PerCpu::current_pid`]). Called under the CPU's scheduler lock.
#[cfg(feature = "browser-runtime")]
#[inline]
pub fn set_current_pid(pid: u64) {
    current_per_cpu().current_pid.store(pid, Ordering::Release);
}

/// OT-61 C1.3b: the pid CPU `cpu` last switched in (see
/// [`PerCpu::current_pid`]).
#[cfg(feature = "browser-runtime")]
#[inline]
pub fn current_pid_of(cpu: usize) -> u64 {
    PER_CPU[cpu].current_pid.load(Ordering::Acquire)
}

pub fn init_bsp() {
    init_cpu(0, "BSP");
}

/// Initialize the calling AP's GS-base. Sub-commit 1a defines this entry point;
/// sub-commit 1b wires it into AP startup.
pub fn init_ap(cpu_id: u32) {
    init_cpu(cpu_id, "AP");
}

fn init_cpu(cpu_id: u32, role: &str) {
    let idx = cpu_id as usize;
    assert!(idx < MAX_CPUS, "per_cpu: cpu_id exceeds MAX_CPUS");

    let pc = &PER_CPU[idx];
    unsafe {
        *pc.cpu_id.get() = cpu_id;
    }

    let per_cpu_addr = pc as *const PerCpu as u64;

    GsBase::write(VirtAddr::new(per_cpu_addr));
    KernelGsBase::write(VirtAddr::new(0));

    serial_println!(
        "[ kernel ] per_cpu: PER_CPU[{}] at 0x{:016x} → IA32_GS_BASE ({})",
        cpu_id,
        per_cpu_addr,
        role
    );
}

#[inline]
pub fn set_kernel_rsp(rsp: u64) {
    let pc = current_per_cpu();
    unsafe {
        *pc.kernel_rsp.get() = rsp;
    }
}

#[inline]
pub fn slab_state() -> *mut () {
    let pc = current_per_cpu();
    unsafe { *pc.slab_state.get() }
}

#[inline]
#[allow(dead_code)]
pub fn set_slab_state(ptr: *mut ()) {
    let pc = current_per_cpu();
    unsafe {
        *pc.slab_state.get() = ptr;
    }
}

#[inline]
pub fn set_slab_state_for_cpu(cpu_id: u32, ptr: *mut ()) {
    let idx = cpu_id as usize;
    assert!(idx < MAX_CPUS, "per_cpu: cpu_id exceeds MAX_CPUS");
    unsafe {
        *PER_CPU[idx].slab_state.get() = ptr;
    }
}

#[inline]
pub fn mark_tlb_shootdown_ready() {
    // Call only after this CPU has IDT/LAPIC/per-CPU/slab/scheduler state and IF=1.
    set_current_tlb_shootdown_ready(true);
}

#[inline]
pub fn set_current_tlb_shootdown_ready(ready: bool) -> bool {
    current_per_cpu()
        .tlb_shootdown_ready
        .swap(ready, Ordering::AcqRel)
}

#[inline]
pub fn tlb_shootdown_ready(cpu_id: u32) -> bool {
    let idx = cpu_id as usize;
    if idx >= MAX_CPUS {
        return false;
    }
    PER_CPU[idx].tlb_shootdown_ready.load(Ordering::Acquire)
}
