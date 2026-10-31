// SPDX-License-Identifier: MPL-2.0
//! Default-off, read-only kernel observability surface for ADR-014 / OT-9.
//!
//! When `kernel-observability` is disabled, every producer in this module is an
//! inline no-op. When enabled, hot-path producers use per-CPU atomics and a
//! fixed overwrite-oldest trace ring; the only allocator lock read happens on
//! the explicit snapshot path.

#![cfg_attr(not(feature = "kernel-observability"), allow(dead_code))]

#[cfg(feature = "kernel-observability")]
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const KERNEL_OBSERVABILITY_V1_VERSION: u32 = 3;
pub const KERNEL_OBSERVABILITY_MAX_CPUS: usize = crate::arch::x86_64::per_cpu::MAX_CPUS;
pub const KERNEL_OBSERVABILITY_TRACE_EVENTS: usize = 16;

pub const TRACE_KIND_DISPATCH: u32 = 1;
pub const TRACE_KIND_PAGE_FAULT: u32 = 2;
pub const TRACE_KIND_SYSCALL: u32 = 3;

#[cfg(feature = "kernel-observability")]
const SYSCALL_CLASS_CORE: u64 = 0;
#[cfg(feature = "kernel-observability")]
const SYSCALL_CLASS_AI: u64 = 1;
#[cfg(feature = "kernel-observability")]
const SYSCALL_CLASS_NET: u64 = 2;
#[cfg(feature = "kernel-observability")]
const SYSCALL_CLASS_OTHER: u64 = 3;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SchedulerStatsV1 {
    pub cpu_id: u32,
    pub _pad0: u32,
    pub context_switches: u64,
    pub dispatches: u64,
    pub ready_queue_depth: u64,
    pub run_ticks: u64,
    pub idle_ticks: u64,
    pub timer_preemptions: u64,
    pub voluntary_yields: u64,
    pub ready_queue_high_water: u64,
}

impl SchedulerStatsV1 {
    pub const fn empty() -> Self {
        Self {
            cpu_id: 0,
            _pad0: 0,
            context_switches: 0,
            dispatches: 0,
            ready_queue_depth: 0,
            run_ticks: 0,
            idle_ticks: 0,
            timer_preemptions: 0,
            voluntary_yields: 0,
            ready_queue_high_water: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TraceEventV1 {
    pub seq: u64,
    pub ticks: u64,
    pub kind: u32,
    pub cpu_id: u32,
    pub arg0: u64,
    pub arg1: u64,
}

impl TraceEventV1 {
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            ticks: 0,
            kind: 0,
            cpu_id: 0,
            arg0: 0,
            arg1: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KernelObservabilityV1 {
    pub version: u32,
    pub cpu_count: u32,
    pub trace_capacity: u32,
    pub trace_count: u32,
    pub heap_current_bytes: u64,
    pub heap_high_water_bytes: u64,
    pub heap_alloc_failures: u64,
    pub syscall_total: u64,
    pub syscall_ipc: u64,
    pub syscall_file: u64,
    pub syscall_ai: u64,
    pub syscall_net: u64,
    pub syscall_other: u64,
    pub frames_used: u64,
    pub buddy_free_pages: u64,
    pub buddy_largest_free_order: u32,
    pub _pad0: u32,
    pub sched: [SchedulerStatsV1; KERNEL_OBSERVABILITY_MAX_CPUS],
    pub trace: [TraceEventV1; KERNEL_OBSERVABILITY_TRACE_EVENTS],
}

impl KernelObservabilityV1 {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            cpu_count: 0,
            trace_capacity: KERNEL_OBSERVABILITY_TRACE_EVENTS as u32,
            trace_count: 0,
            heap_current_bytes: 0,
            heap_high_water_bytes: 0,
            heap_alloc_failures: 0,
            syscall_total: 0,
            syscall_ipc: 0,
            syscall_file: 0,
            syscall_ai: 0,
            syscall_net: 0,
            syscall_other: 0,
            frames_used: 0,
            buddy_free_pages: 0,
            buddy_largest_free_order: 0,
            _pad0: 0,
            sched: [SchedulerStatsV1::empty(); KERNEL_OBSERVABILITY_MAX_CPUS],
            trace: [TraceEventV1::empty(); KERNEL_OBSERVABILITY_TRACE_EVENTS],
        }
    }
}

#[cfg(feature = "kernel-observability")]
struct TraceSlot {
    seq: AtomicU64,
    ticks: AtomicU64,
    kind: AtomicU32,
    cpu_id: AtomicU32,
    arg0: AtomicU64,
    arg1: AtomicU64,
}

#[cfg(feature = "kernel-observability")]
impl TraceSlot {
    // Intentional: this const is consumed only as `[const { TraceSlot::EMPTY }; N]`
    // to initialize the trace ring. A `static` cannot be used in that position, and
    // the interior mutability is the point — each slot's atomics are written after
    // placement. The lint's hazard (copying a const and mutating the copy) does not
    // apply to an array initializer.
    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY: Self = Self {
        seq: AtomicU64::new(0),
        ticks: AtomicU64::new(0),
        kind: AtomicU32::new(0),
        cpu_id: AtomicU32::new(0),
        arg0: AtomicU64::new(0),
        arg1: AtomicU64::new(0),
    };

    fn snapshot_if_seq(&self, expected: u64) -> Option<TraceEventV1> {
        if self.seq.load(Ordering::Acquire) != expected {
            return None;
        }
        let event = TraceEventV1 {
            seq: expected,
            ticks: self.ticks.load(Ordering::Relaxed),
            kind: self.kind.load(Ordering::Relaxed),
            cpu_id: self.cpu_id.load(Ordering::Relaxed),
            arg0: self.arg0.load(Ordering::Relaxed),
            arg1: self.arg1.load(Ordering::Relaxed),
        };
        if self.seq.load(Ordering::Acquire) != expected {
            return None;
        }
        Some(event)
    }
}

#[cfg(feature = "kernel-observability")]
static TRACE_NEXT_SEQ: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "kernel-observability")]
static TRACE_RING: [TraceSlot; KERNEL_OBSERVABILITY_TRACE_EVENTS] =
    [const { TraceSlot::EMPTY }; KERNEL_OBSERVABILITY_TRACE_EVENTS];

#[cfg(feature = "kernel-observability")]
static HEAP_CURRENT_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static HEAP_HIGH_WATER_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static HEAP_ALLOC_FAILURES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "kernel-observability")]
static SYSCALL_TOTAL: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static SYSCALL_IPC: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static SYSCALL_FILE: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static SYSCALL_AI: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static SYSCALL_NET: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kernel-observability")]
static SYSCALL_OTHER: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
pub fn record_context_switch() {
    #[cfg(feature = "kernel-observability")]
    {
        crate::arch::x86_64::per_cpu::current_per_cpu()
            .sched_context_switches
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
pub fn record_dispatch(_ready_depth: usize) {
    #[cfg(feature = "kernel-observability")]
    {
        let pc = crate::arch::x86_64::per_cpu::current_per_cpu();
        pc.sched_dispatches.fetch_add(1, Ordering::Relaxed);
        pc.sched_ready_queue_depth
            .store(_ready_depth as u64, Ordering::Relaxed);
        record_trace(
            TRACE_KIND_DISPATCH,
            pc.current_thread_id.load(Ordering::Relaxed),
            _ready_depth as u64,
        );
    }
}

#[inline(always)]
pub fn record_ready_depth(_cpu_id: u32, _ready_depth: usize) {
    #[cfg(feature = "kernel-observability")]
    {
        let idx = _cpu_id as usize;
        if idx < crate::arch::x86_64::per_cpu::MAX_CPUS {
            let pc = &crate::arch::x86_64::per_cpu::PER_CPU[idx];
            let depth = _ready_depth as u64;
            pc.sched_ready_queue_high_water
                .fetch_max(depth, Ordering::Relaxed);
            pc.sched_ready_queue_depth.store(depth, Ordering::Relaxed);
        }
    }
}

#[inline(always)]
pub fn record_scheduler_tick() {
    #[cfg(feature = "kernel-observability")]
    {
        let pc = crate::arch::x86_64::per_cpu::current_per_cpu();
        // OT-22 (CP-59): idle means "running this CPU's idle thread" (it
        // has a real tid), not "no current thread", which never happens once
        // a scheduler is installed. Behind `wait-multiplex` so the counters
        // are unchanged with it off (the kill-switch).
        #[cfg(feature = "wait-multiplex")]
        let running = !crate::sched::scheduler::current_is_idle();
        #[cfg(not(feature = "wait-multiplex"))]
        let running = pc.current_thread_id.load(Ordering::Relaxed) != u64::MAX;
        if running {
            pc.sched_run_ticks.fetch_add(1, Ordering::Relaxed);
        } else {
            pc.sched_idle_ticks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[inline(always)]
pub fn record_timer_preemption() {
    #[cfg(feature = "kernel-observability")]
    {
        crate::arch::x86_64::per_cpu::current_per_cpu()
            .sched_timer_preemptions
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
pub fn record_voluntary_yield() {
    #[cfg(feature = "kernel-observability")]
    {
        crate::arch::x86_64::per_cpu::current_per_cpu()
            .sched_voluntary_yields
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
pub fn record_heap_alloc(_size: usize) {
    #[cfg(feature = "kernel-observability")]
    {
        let new = HEAP_CURRENT_BYTES.fetch_add(_size as u64, Ordering::Relaxed) + _size as u64;
        update_heap_high_water(new);
    }
}

#[inline(always)]
pub fn record_heap_alloc_failure() {
    #[cfg(feature = "kernel-observability")]
    {
        HEAP_ALLOC_FAILURES.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
pub fn record_heap_dealloc(_size: usize) {
    #[cfg(feature = "kernel-observability")]
    {
        HEAP_CURRENT_BYTES.fetch_sub(_size as u64, Ordering::Relaxed);
    }
}

#[inline(always)]
pub fn record_syscall(_num: u64) {
    #[cfg(feature = "kernel-observability")]
    {
        record_syscall_counter(_num);
        record_trace(TRACE_KIND_SYSCALL, syscall_class(_num), _num);
    }
}

#[inline(always)]
pub fn record_page_fault(_cr2: u64, _err_bits: u64) {
    #[cfg(feature = "kernel-observability")]
    {
        record_trace(TRACE_KIND_PAGE_FAULT, _cr2, _err_bits);
    }
}

pub fn snapshot() -> KernelObservabilityV1 {
    let mut out = KernelObservabilityV1::empty();
    out.version = KERNEL_OBSERVABILITY_V1_VERSION;

    #[cfg(feature = "kernel-observability")]
    {
        fill_scheduler_snapshot(&mut out);
        fill_trace_snapshot(&mut out);
        out.heap_current_bytes = HEAP_CURRENT_BYTES.load(Ordering::Relaxed);
        out.heap_high_water_bytes = HEAP_HIGH_WATER_BYTES.load(Ordering::Relaxed);
        out.heap_alloc_failures = HEAP_ALLOC_FAILURES.load(Ordering::Relaxed);
        fill_syscall_snapshot(&mut out);
        out.frames_used = crate::mm::frames_in_use();
        let buddy = crate::mm::frame::observability_free_summary();
        out.buddy_free_pages = buddy.free_pages;
        out.buddy_largest_free_order = buddy.largest_order;
    }

    out
}

pub fn dump_trace_to_serial() {
    #[cfg(feature = "kernel-observability")]
    {
        let snapshot = snapshot();
        let count = snapshot.trace_count as usize;
        for event in snapshot.trace.iter().take(count) {
            crate::serial_println!(
                "[ kernel ] observe: trace seq={} kind={} cpu={} ticks={} arg0={} arg1={}",
                event.seq,
                event.kind,
                event.cpu_id,
                event.ticks,
                event.arg0,
                event.arg1
            );
        }
    }
}

#[cfg(feature = "kernel-observability")]
fn record_syscall_counter(num: u64) {
    SYSCALL_TOTAL.fetch_add(1, Ordering::Relaxed);
    syscall_domain_counter(num).fetch_add(1, Ordering::Relaxed);
}

#[cfg(feature = "kernel-observability")]
fn syscall_domain_counter(num: u64) -> &'static AtomicU64 {
    match num {
        4 | 5 | 10 | 11 => &SYSCALL_IPC,
        12 | 13 | 14 | 15 | 16 | 17 | 18 | 23 => &SYSCALL_FILE,
        0x100..=0x17f => &SYSCALL_AI,
        0x180..=0x1ff => &SYSCALL_NET,
        _ => &SYSCALL_OTHER,
    }
}

#[cfg(feature = "kernel-observability")]
fn update_heap_high_water(value: u64) {
    let mut observed = HEAP_HIGH_WATER_BYTES.load(Ordering::Relaxed);
    while value > observed {
        match HEAP_HIGH_WATER_BYTES.compare_exchange_weak(
            observed,
            value,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(next) => observed = next,
        }
    }
}

#[cfg(feature = "kernel-observability")]
fn record_trace(kind: u32, arg0: u64, arg1: u64) {
    let seq = TRACE_NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let slot = &TRACE_RING[seq as usize % KERNEL_OBSERVABILITY_TRACE_EVENTS];
    slot.ticks.store(
        crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    slot.kind.store(kind, Ordering::Relaxed);
    slot.cpu_id.store(
        crate::arch::x86_64::per_cpu::current_cpu_id(),
        Ordering::Relaxed,
    );
    slot.arg0.store(arg0, Ordering::Relaxed);
    slot.arg1.store(arg1, Ordering::Relaxed);
    slot.seq.store(seq, Ordering::Release);
}

#[cfg(feature = "kernel-observability")]
fn syscall_class(num: u64) -> u64 {
    match num {
        0x000..=0x0ff => SYSCALL_CLASS_CORE,
        0x100..=0x17f => SYSCALL_CLASS_AI,
        0x180..=0x1ff => SYSCALL_CLASS_NET,
        _ => SYSCALL_CLASS_OTHER,
    }
}

#[cfg(feature = "kernel-observability")]
fn fill_syscall_snapshot(out: &mut KernelObservabilityV1) {
    out.syscall_total = SYSCALL_TOTAL.load(Ordering::Relaxed);
    out.syscall_ipc = SYSCALL_IPC.load(Ordering::Relaxed);
    out.syscall_file = SYSCALL_FILE.load(Ordering::Relaxed);
    out.syscall_ai = SYSCALL_AI.load(Ordering::Relaxed);
    out.syscall_net = SYSCALL_NET.load(Ordering::Relaxed);
    out.syscall_other = SYSCALL_OTHER.load(Ordering::Relaxed);
}

#[cfg(feature = "kernel-observability")]
fn fill_scheduler_snapshot(out: &mut KernelObservabilityV1) {
    let mut count = 0u32;
    for (idx, pc) in crate::arch::x86_64::per_cpu::PER_CPU.iter().enumerate() {
        if pc.scheduler_ptr.load(Ordering::Acquire).is_null() {
            continue;
        }
        count += 1;
        let ready_queue_depth = pc.sched_ready_queue_depth.load(Ordering::Relaxed);
        let ready_queue_high_water = pc
            .sched_ready_queue_high_water
            .load(Ordering::Relaxed)
            .max(ready_queue_depth);
        out.sched[idx] = SchedulerStatsV1 {
            cpu_id: idx as u32,
            _pad0: 0,
            context_switches: pc.sched_context_switches.load(Ordering::Relaxed),
            dispatches: pc.sched_dispatches.load(Ordering::Relaxed),
            ready_queue_depth,
            run_ticks: pc.sched_run_ticks.load(Ordering::Relaxed),
            idle_ticks: pc.sched_idle_ticks.load(Ordering::Relaxed),
            timer_preemptions: pc.sched_timer_preemptions.load(Ordering::Relaxed),
            voluntary_yields: pc.sched_voluntary_yields.load(Ordering::Relaxed),
            ready_queue_high_water,
        };
    }
    out.cpu_count = count;
}

/// Test-only: the cpu id of the newest trace event still in the ring with this
/// `kind` and `arg0`. Reads the slots directly, so it is usable from exception
/// context, unlike `snapshot()`, which takes the frame-allocator lock. It
/// builds no `TraceEventV1`: it was written while the #PF handler's callees
/// ran on a stack misaligned by 8, where a 16-byte struct copy compiled to
/// `movaps` and #GP'd (CP-46, since fixed).
///
/// `record_trace` does not invalidate a slot before rewriting it, so a stable
/// `seq` alone cannot rule out a writer for `seq + N` midway through the slot.
/// A writer claims its seq with `fetch_add` before touching the slot, so if
/// `TRACE_NEXT_SEQ` has not passed `seq + N` after the field loads, no such
/// writer exists (sound on x86-TSO, the only target).
#[cfg(feature = "neo-test-threads-guard-fault-observability")]
pub fn latest_trace_cpu_id(kind: u32, arg0: u64) -> Option<u32> {
    const N: u64 = KERNEL_OBSERVABILITY_TRACE_EVENTS as u64;
    let last_seq = TRACE_NEXT_SEQ.load(Ordering::Acquire).saturating_sub(1);
    let count = last_seq.min(N);
    for back in 0..count {
        let seq = last_seq - back;
        let slot = &TRACE_RING[seq as usize % KERNEL_OBSERVABILITY_TRACE_EVENTS];
        if slot.seq.load(Ordering::Acquire) != seq
            || slot.kind.load(Ordering::Relaxed) != kind
            || slot.arg0.load(Ordering::Relaxed) != arg0
        {
            continue;
        }
        let cpu_id = slot.cpu_id.load(Ordering::Relaxed);
        core::sync::atomic::fence(Ordering::Acquire);
        if slot.seq.load(Ordering::Relaxed) == seq
            && TRACE_NEXT_SEQ.load(Ordering::Relaxed) <= seq + N
        {
            return Some(cpu_id);
        }
    }
    None
}

#[cfg(feature = "kernel-observability")]
fn fill_trace_snapshot(out: &mut KernelObservabilityV1) {
    let next_seq = TRACE_NEXT_SEQ.load(Ordering::Acquire);
    let last_seq = next_seq.saturating_sub(1);
    let count = last_seq.min(KERNEL_OBSERVABILITY_TRACE_EVENTS as u64);
    let start = last_seq.saturating_sub(count).saturating_add(1);
    let mut written = 0usize;
    for seq in start..=last_seq {
        if written >= KERNEL_OBSERVABILITY_TRACE_EVENTS {
            break;
        }
        let slot = &TRACE_RING[seq as usize % KERNEL_OBSERVABILITY_TRACE_EVENTS];
        if let Some(event) = slot.snapshot_if_seq(seq) {
            out.trace[written] = event;
            written += 1;
        }
    }
    out.trace_count = written as u32;
}
