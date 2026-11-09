// SPDX-License-Identifier: MPL-2.0
//! Cooperative + preemptive round-robin scheduler.
//!
//! Phase 1.2 shipped the cooperative path (`yield_now`). Phase 1.3 adds:
//!   - `on_tick()` — called from the timer ISR; preempts if the ready queue
//!     is non-empty by delegating to `yield_now`.
//!   - `exit() -> !` — terminates the current thread. Moves it to the
//!     `zombies` queue (NOT re-enqueued in `ready`), then switches away. The
//!     zombie is later reclaimed by `reap()`.
//!   - `reap()` — returns each zombie to THREAD_CACHE after dropping its
//!     owned kernel stack. Called from the BSP bootstrap thread between yields.
//!
//! Locking: each CPU owns one `PerCpuScheduler` behind its own Mutex. The
//! Mutex is held only across that CPU's queue manipulation; we explicitly drop
//! the `MutexGuard` BEFORE the asm `context_switch` call.
//!
//! Storage: fixed-capacity rings of `Box<Thread>`, NOT inline `Thread`
//! values. We capture `*mut u64` pointers to a Thread's `rsp` field BEFORE
//! pushing the Box back into a scheduler-owned ring. Moving a ring slot moves
//! only the Box pointer; the Thread allocation and captured `rsp` location
//! remain stable. Inline storage would invalidate the pointer at enqueue.
//!
//! Resume from preempt vs. resume from yield: when a thread is later resumed
//! via cooperative `context_switch`, control returns to the call site of
//! `yield_now`. For a thread that yielded cooperatively that's whatever its
//! own code did. For a thread that was PREEMPTED, that call site is inside
//! `on_tick`, which returns to `timer_rust_handler`, which returns to the
//! asm wrapper at `fxrstor` + GP-pop + `iretq`. Either way the cooperative
//! frame is correct; the asm wrapper handles the rest.

use alloc::boxed::Box;
use core::mem;
use core::ops::DerefMut;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::PhysFrame;

use crate::arch::x86_64::context::{
    context_switch, context_switch_with_release1, context_switch_with_release2,
};
use crate::arch::x86_64::per_cpu::{self, PER_CPU};
use crate::arch::x86_64::{gdt, pic, pit, without_interrupts};
use crate::serial_println;
use crate::sync::SwitchMutex;
use crate::sync::irqsafe_mutex::IrqSafeGuard;
use crate::sync::switch_mutex::SwitchMutexGuard;

use super::process::{self, KERNEL_PID, ProcessId};
use super::thread::{Thread, ThreadState};
#[cfg(feature = "neo-test")]
use super::wake_trace;

/// Fixed scheduler-owned queue capacity per CPU. Ready and zombie queues both
/// use this bound; queue operations never grow or allocate under the scheduler
/// lock.
const SCHEDULER_RING_CAPACITY: usize = 64;
const _: () = assert!(SCHEDULER_RING_CAPACITY > 0);
type SchedulerThreadRing = ThreadRing<SCHEDULER_RING_CAPACITY>;
/// Scheduler-owned weak-token parking slots per CPU. These are intentionally
/// fixed-size so the block path never allocates while holding scheduler locks.
const TOKEN_PARKED_SLOTS: usize = 16;

/// Timer frequency in Hz. 100 → 10ms tick.
const TIMER_HZ: u32 = 100;
const HANDOFF_PREEMPT_DEFER_TICKS: u8 = 32;

static PREEMPT_DISABLE_DEPTH: [AtomicU32; per_cpu::MAX_CPUS] =
    [const { AtomicU32::new(0) }; per_cpu::MAX_CPUS];

pub(crate) struct PreemptionGuard {
    cpu_id: usize,
}

impl Drop for PreemptionGuard {
    fn drop(&mut self) {
        let previous = PREEMPT_DISABLE_DEPTH[self.cpu_id].fetch_sub(1, Ordering::Release);
        debug_assert!(previous != 0, "preemption-disable depth underflow");
    }
}

/// Prevent timer-driven context switches on this CPU while leaving IF and
/// non-timer interrupt delivery unchanged. The guard is nestable and must not
/// cross a cooperative context switch.
pub(crate) fn disable_preemption() -> PreemptionGuard {
    let cpu_id = per_cpu::current_cpu_id() as usize;
    let previous = PREEMPT_DISABLE_DEPTH[cpu_id].fetch_add(1, Ordering::Acquire);
    debug_assert!(previous != u32::MAX, "preemption-disable depth overflow");
    PreemptionGuard { cpu_id }
}

#[cfg(feature = "neo-test-instrumentation")]
static CONTEXT_SWITCH_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-instrumentation")]
static SCHEDULING_DECISION_COUNT: AtomicU64 = AtomicU64::new(0);

/// OT-49 completeness evidence (AAG B1): per-CPU count of idle-thread loop
/// entries. Bumped (relaxed — no lock, no alloc, I-14(a)-clean) each iteration
/// of `idle_thread_entry`, i.e. every time a thread left this CPU with an empty
/// ready queue and the idle thread took over. A test scenario reads the total
/// and folds it into its PASS string, so a green gate *demonstrates* the
/// park→idle path fired (`idle_entered > 0`) instead of only proving "did not
/// panic". Per-CPU so the evidence also shows idle is reached on the BSP only —
/// APs busy-yield in `run_ap_bootstrap` and never park (the ISC-4 reconciliation).
#[cfg(feature = "neo-test")]
static IDLE_ENTERED: [AtomicU64; per_cpu::MAX_CPUS] =
    [const { AtomicU64::new(0) }; per_cpu::MAX_CPUS];

/// OT-22: each CPU's idle-thread id, published once at scheduler install, so
/// the tick can tell "this CPU is idle" from one relaxed load (the per-CPU
/// `current_thread_id` is never `u64::MAX` once a scheduler is installed: the
/// idle thread has a real id, which left the old `idle_ticks` test at ~0).
static IDLE_TID: [AtomicU64; per_cpu::MAX_CPUS] =
    [const { AtomicU64::new(u64::MAX) }; per_cpu::MAX_CPUS];

/// OT-22 witness: ticks that found this CPU running its idle thread.
#[cfg(feature = "neo-test")]
static IDLE_TICKS: [AtomicU64; per_cpu::MAX_CPUS] =
    [const { AtomicU64::new(0) }; per_cpu::MAX_CPUS];

/// Is this CPU running its idle thread right now? Lock-free (the tick path).
pub fn current_is_idle() -> bool {
    let cpu = per_cpu::current_cpu_id() as usize;
    cpu < per_cpu::MAX_CPUS
        && per_cpu::current_per_cpu()
            .current_thread_id
            .load(Ordering::Relaxed)
            == IDLE_TID[cpu].load(Ordering::Relaxed)
}

/// OT-22 witness: ticks CPU `cpu` spent in its idle thread, since boot.
#[cfg(feature = "neo-test")]
#[allow(dead_code)] // read by the desktop idle scenarios
pub fn debug_idle_ticks(cpu: usize) -> u64 {
    IDLE_TICKS.get(cpu).map_or(0, |c| c.load(Ordering::Relaxed))
}

/// Per-CPU scheduler state. One instance per CPU, heap-allocated during BSP
/// scheduler init or AP bootstrap and then intentionally leaked for kernel
/// lifetime. `PER_CPU[cpu_id].scheduler_ptr` points at this object.
pub struct PerCpuScheduler {
    pub cpu_id: u32,
    /// Scheduler/context-switch special: the switch shim releases this guard's
    /// token on the new stack after queue/current ownership has moved.
    inner: SwitchMutex<PerCpuSchedulerInner>,
    /// Lock-free count of zombies waiting to be reaped. Lets `reap()` skip
    /// this CPU's scheduler lock entirely on the steady-state empty path.
    zombie_count: AtomicUsize,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ThreadWakeToken {
    pub cpu_id: u32,
    pub slot: u32,
    pub generation: u64,
    /// The ID of the thread that parked (consumed by `park_thread_token`).
    /// Used by `wake_trace::note_event` to record the correct PARK tid;
    /// `current_thread_id()` is already overwritten to the next thread by
    /// the time the installer closure runs.
    pub tid: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum WakeTokenResult {
    Woken,
    Gone,
    Stale,
}

struct PerCpuSchedulerInner {
    current: Box<Thread>,
    ready: SchedulerThreadRing,
    /// Threads that called `exit()`. The local reaper drains this and Drops
    /// each Box, freeing the heap-backed stack.
    zombies: SchedulerThreadRing,
    /// Threads parked by non-owning wake-token clients such as storage I/O.
    /// The scheduler owns the Box here; external rendezvous objects only
    /// receive `ThreadWakeToken`, which is generation-checked on wake.
    token_parked: [TokenParkSlot; TOKEN_PARKED_SLOTS],
    /// Per-CPU idle thread (OT-49). The always-available switch target for a
    /// thread that is *leaving* the CPU (exit or any `block_current_*` park)
    /// when the ready queue is empty. `Some` whenever a non-idle thread is
    /// `current`; `None` only while the idle thread is itself `current`
    /// (running its `sti; hlt` loop). It is never enqueued in `ready` — it is
    /// not ordinary schedulable work — so it round-trips through this slot, not
    /// the ready ring.
    idle_thread: Option<Box<Thread>>,
    /// `id.0` of this CPU's idle thread, so `rotate` can recognise when the
    /// *outgoing* thread is the idle thread (a timer tick can preempt the
    /// halted idle thread) and return it to `idle_thread` instead of the ready
    /// ring.
    idle_tid: u64,
    /// OT-61 C1.1 (AAG blocker A): count of live (admitted, not-yet-reaped)
    /// threads on this CPU — everything in `ready` plus the running thread
    /// plus every blocked/token-parked thread. Admission (`try_push_ready` /
    /// `push_ready_or_panic`) is capped at `SCHEDULER_RING_CAPACITY`, so
    /// `ready.len() <= live_count <= capacity` always holds. A wake therefore
    /// pushes a live-but-not-ready thread into a ring that provably has room
    /// (`ready.len() < live_count <= capacity`) — the token/futex/join wake
    /// can never overflow the ring and panic, and no valid parked waiter is
    /// dropped or stranded under ready-ring backpressure.
    ///
    /// The field itself is `cfg`-gated, not merely its logic: with
    /// `browser-runtime` off this struct has no `live_count` at all, so the
    /// default kernel keeps its exact previous shape (the kill-switch OFF
    /// equality is a real property, not an aspiration).
    #[cfg(feature = "browser-runtime")]
    live_count: usize,
}

struct TokenParkSlot {
    generation: u64,
    thread: Option<Box<Thread>>,
}

pub trait ContextSwitchRelease {
    fn release_token_for_context_switch(&mut self) -> *const AtomicBool;
}

impl<T> ContextSwitchRelease for SwitchMutexGuard<'_, T> {
    #[inline(always)]
    fn release_token_for_context_switch(&mut self) -> *const AtomicBool {
        SwitchMutexGuard::release_token_for_context_switch(self)
    }
}

impl<T> ContextSwitchRelease for IrqSafeGuard<'_, T> {
    #[inline(always)]
    fn release_token_for_context_switch(&mut self) -> *const AtomicBool {
        IrqSafeGuard::release_token_for_context_switch(self)
    }
}

impl TokenParkSlot {
    const fn empty() -> Self {
        Self {
            generation: 0,
            thread: None,
        }
    }
}

struct ThreadRing<const N: usize> {
    slots: [Option<Box<Thread>>; N],
    head: usize,
    len: usize,
}

impl<const N: usize> ThreadRing<N> {
    fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
            head: 0,
            len: 0,
        }
    }

    const fn capacity(&self) -> usize {
        N
    }

    const fn len(&self) -> usize {
        self.len
    }

    const fn is_empty(&self) -> bool {
        self.len == 0
    }

    const fn is_full(&self) -> bool {
        self.len == N
    }

    fn push_back(&mut self, thread: Box<Thread>) -> Result<(), Box<Thread>> {
        if self.is_full() {
            return Err(thread);
        }
        let idx = self.slot_index(self.len);
        debug_assert!(self.slots[idx].is_none(), "ready ring tail slot occupied");
        self.slots[idx] = Some(thread);
        self.len += 1;
        Ok(())
    }

    fn push_front(&mut self, thread: Box<Thread>) -> Result<(), Box<Thread>> {
        if self.is_full() {
            return Err(thread);
        }
        self.head = (self.head + N - 1) % N;
        debug_assert!(
            self.slots[self.head].is_none(),
            "ready ring head slot occupied"
        );
        self.slots[self.head] = Some(thread);
        self.len += 1;
        Ok(())
    }

    fn pop_front(&mut self) -> Option<Box<Thread>> {
        if self.is_empty() {
            return None;
        }
        let thread = self.slots[self.head].take();
        self.head = self.slot_index(1);
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        }
        debug_assert!(thread.is_some(), "ready ring occupied slot missing thread");
        thread
    }

    /// The first thread of `pid` a force-kill may zombify from the ready
    /// queue: a protected one ([`kill_protected`]) is skipped; it cancels
    /// cooperatively and exits at the syscall boundary instead. So is one
    /// inside a syscall (OT-93, [`ready_kill_safe`]).
    fn position_by_pid_killable(&self, pid: ProcessId) -> Option<usize> {
        (0..self.len).find(|&offset| {
            self.slots[self.slot_index(offset)]
                .as_ref()
                .is_some_and(|thread| {
                    thread.pid == pid && !kill_protected(thread) && ready_kill_safe(thread)
                })
        })
    }

    /// OT-93: threads of `pid` in this queue that a force-kill leaves to exit
    /// at their syscall boundary (Ready inside a syscall).
    fn count_by_pid_in_syscall(&self, pid: ProcessId) -> usize {
        (0..self.len)
            .filter(|&offset| {
                self.slots[self.slot_index(offset)]
                    .as_ref()
                    .is_some_and(|thread| thread.pid == pid && !ready_kill_safe(thread))
            })
            .count()
    }

    fn remove_at(&mut self, offset: usize) -> Option<Box<Thread>> {
        if offset >= self.len {
            return None;
        }
        let removed_idx = self.slot_index(offset);
        let removed = self.slots[removed_idx].take();
        for shift in offset..(self.len - 1) {
            let to = self.slot_index(shift);
            let from = self.slot_index(shift + 1);
            self.slots[to] = self.slots[from].take();
        }
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        }
        debug_assert!(
            removed.is_some(),
            "ready ring remove_at found empty occupied slot"
        );
        removed
    }

    const fn slot_index(&self, offset: usize) -> usize {
        (self.head + offset) % N
    }
}

/// Bring up the BSP scheduler and start preemption. After this returns:
///   - The BSP's per-CPU scheduler is installed.
///   - PIT is programmed at TIMER_HZ.
///   - PIC IRQ0 (timer) is unmasked, so the FIRST tick may fire immediately.
///   - The BSP is marked TLB-shootdown-ready only after its scheduler exists
///     and the caller has already enabled interrupts.
///
/// Must be called exactly once on the BSP, after the heap is up and after
/// `init_traps()`.
pub fn init_bsp() {
    init_cpu_scheduler(0, "main", "BSP");
    // Zenbook lane: the LAPIC timer when it can be calibrated, else (and
    // with the feature off) the PIT through PIC IRQ0, unchanged.
    #[cfg(feature = "lapic-timer")]
    let lapic = crate::arch::x86_64::lapic_timer::start(TIMER_HZ);
    #[cfg(not(feature = "lapic-timer"))]
    let lapic = false;
    if !lapic {
        pit::init(TIMER_HZ);
        // Unmask IRQ0 LAST so the first tick can never fire before the
        // scheduler is installed and PIT is programmed.
        // `neo-test-lapic-timer-pit-gated` (manual control): a PIT whose
        // ticks never arrive, simulated by leaving IRQ0 masked, so the
        // fallback check below must report NO TICK SOURCE.
        if !cfg!(feature = "neo-test-lapic-timer-pit-gated") {
            pic::unmask_irq(0);
        }
        // CP-69 (Jarvis): the fallback is only a tick if it ticks. On client
        // silicon with the 8254 gated it never does: say so, on serial and
        // screen, and stop, rather than freeze with nothing preempting.
        #[cfg(feature = "lapic-timer")]
        verify_pit_fallback();
    }
    per_cpu::mark_tlb_shootdown_ready();
    serial_println!(
        "[ kernel ] sched: preemption ready (cpu=0, {} @ {} Hz)",
        if lapic { "LAPIC timer" } else { "PIT" },
        TIMER_HZ
    );
    // With `lapic-timer` every tick source is verified above (and named on
    // screen); this PIT-only report is for builds without it.
    #[cfg(all(feature = "fb-progress-console", not(feature = "lapic-timer")))]
    report_first_tick();
}

/// Zenbook lane (NOTE §4a): on newer Intel client platforms the 8254 PIT
/// can be clock-gated, or the 8259 route to the BSP absent, and then the
/// kernel tick never fires and the boot freezes with no message. Wait a
/// bounded TSC interval for the first tick and say which, on the screen.
#[cfg(all(feature = "fb-progress-console", not(feature = "lapic-timer")))]
fn report_first_tick() {
    use crate::arch::x86_64::timer::TICKS;
    // About one second at 3 GHz; the PIT ticks every 10 ms when it works.
    const BUDGET_CYCLES: u64 = 3_000_000_000;
    let rdtsc = || {
        // SAFETY: RDTSC has no side effects.
        unsafe { core::arch::x86_64::_rdtsc() }
    };
    let start_ticks = TICKS.load(Ordering::Acquire);
    let start = rdtsc();
    loop {
        let elapsed = rdtsc().wrapping_sub(start);
        if TICKS.load(Ordering::Acquire) != start_ticks {
            serial_println!(
                "[ kernel ] timer: PIT/PIC tick alive (first tick after {} TSC cycles)",
                elapsed
            );
            return;
        }
        if elapsed > BUDGET_CYCLES {
            serial_println!(
                "[ kernel ] timer: NO TICK after {} TSC cycles - the PIT is gated or the 8259 route is missing; the kernel will not preempt (NOTE 4a: LAPIC timer lane)",
                elapsed
            );
            return;
        }
        core::hint::spin_loop();
    }
}

/// `lapic-timer`: the PIT/PIC fallback must deliver a tick within half a
/// second, or the BSP halts with a named line. Nothing could preempt or time
/// out without one. The wait is sized from a calibrated TSC rate, else from
/// the 10 GHz bound (Jarvis R1): it may run long, never short.
#[cfg(feature = "lapic-timer")]
fn verify_pit_fallback() {
    use crate::arch::x86_64::{lapic_timer, timer::TICKS};
    let budget = lapic_timer::budget_hz() / 2;
    let rdtsc = || {
        // SAFETY: RDTSC has no side effects.
        unsafe { core::arch::x86_64::_rdtsc() }
    };
    let ticks = TICKS.load(Ordering::Acquire);
    let t0 = rdtsc();
    while rdtsc().wrapping_sub(t0) < budget {
        if TICKS.load(Ordering::Acquire) != ticks {
            serial_println!(
                "[ kernel ] timer: PIT/PIC fallback tick alive (lapic={})",
                lapic_timer::failure()
            );
            return;
        }
        core::hint::spin_loop();
    }
    serial_println!(
        "[ kernel ] timer: NO TICK SOURCE (lapic={}, pit=gated) - halting: nothing can preempt or time out",
        lapic_timer::failure()
    );
    // A test build ends with a named verdict instead of a hang.
    #[cfg(feature = "neo-test")]
    crate::test_harness::no_tick_source_fail();
    #[cfg(not(feature = "neo-test"))]
    crate::arch::halt_forever();
}

/// Initialize an AP's per-CPU scheduler and run its bootstrap loop forever.
/// APs do not receive routed work in sub-commit 2; the loop only yields, which
/// no-ops while the queue is empty. BSP remains responsible for reaping.
pub fn run_ap_bootstrap(cpu_id: u32) -> ! {
    let name = ap_bootstrap_name(cpu_id);
    init_cpu_scheduler(cpu_id, name, "AP");
    // Release one count of the BSP's bring_up_aps() readiness barrier: this AP has
    // finished all per-CPU heap/buddy init (slab + scheduler) and is about to idle,
    // so it no longer contends the heap with the BSP's smoke_test/scenario thread
    // spawns. (Closes the multi-core heap-lock race the 768 MiB Stage-B heap exposed
    // on silicon.)
    crate::arch::x86_64::smp::signal_ap_ready();
    // APs need IF=1 once their IDT, LAPIC, slab state, and scheduler are
    // installed so cross-CPU IPIs such as Q7.c TLB shootdowns can be delivered.
    crate::arch::x86_64::enable_interrupts();
    per_cpu::mark_tlb_shootdown_ready();
    loop {
        // OT-34: take a forward-matmul partition if the BSP published one for this
        // CPU. A no-op (single relaxed-then-acquire load) when idle; feature-off
        // removes this hook entirely so the loop is bit-for-bit the pre-OT-34 idle.
        //
        // OT-49 (AAG B2) — ISC-4 coupling: this busy-yield loop is the AP's
        // *current* thread and today it NEVER parks (`yield_now` keeps it running
        // when the ready queue is empty), so the AP's per-CPU OT-49 idle thread is
        // normally never selected. The SAME hook is also carried into
        // `idle_thread_entry` (via `poll_ap_matmul_work`) so the CP-19 seam stays
        // closed even if an AP ever *does* park (an AP-efficiency pass, OT-47's
        // 8-core lane): a parked AP that wakes on a tick still drains its work slot.
        // Both call sites share one hook so they cannot drift.
        poll_ap_matmul_work(cpu_id);
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::ap_tick(cpu_id);
        yield_now();
        core::hint::spin_loop();
    }
}

/// OT-34 (CP-19 guard) — the single AP matmul-work-slot poll hook, shared by BOTH
/// AP idle loops (`run_ap_bootstrap`'s busy-yield and `idle_thread_entry`'s
/// park/halt) so the poll can never be present in one and missing in the other
/// (the exact CP-19 trap: an AP that reaches a halt path without polling the
/// published work slot sleeps through the assignment). Feature-off compiles to an
/// empty inline no-op, so the pre-OT-34 idle path is byte-for-bit unchanged. On
/// the BSP (`cpu_id == 0`) `ap_poll` returns immediately — the BSP serves
/// partition 0 inline — so this is a cheap no-op on the BSP idle thread.
#[inline(always)]
fn poll_ap_matmul_work(cpu_id: u32) {
    #[cfg(feature = "threaded-matmul")]
    crate::ai::par_matmul::ap_poll(cpu_id);
    #[cfg(not(feature = "threaded-matmul"))]
    let _ = cpu_id;
}

fn ap_bootstrap_name(cpu_id: u32) -> &'static str {
    match cpu_id {
        1 => "ap-1-main",
        2 => "ap-2-main",
        3 => "ap-3-main",
        4 => "ap-4-main",
        5 => "ap-5-main",
        6 => "ap-6-main",
        7 => "ap-7-main",
        _ => "ap-main",
    }
}

/// Static name for a CPU's idle thread. Mirrors `ap_bootstrap_name`: named for
/// cpus 0..=7, a shared fallback beyond (MAX_CPUS is 64 but only present CPUs
/// ever install a scheduler).
fn idle_thread_name(cpu_id: u32) -> &'static str {
    match cpu_id {
        0 => "idle-cpu0",
        1 => "idle-cpu1",
        2 => "idle-cpu2",
        3 => "idle-cpu3",
        4 => "idle-cpu4",
        5 => "idle-cpu5",
        6 => "idle-cpu6",
        7 => "idle-cpu7",
        _ => "idle-cpu",
    }
}

fn init_cpu_scheduler(
    cpu_id: u32,
    bootstrap_name: &'static str,
    role: &'static str,
) -> &'static PerCpuScheduler {
    let idx = cpu_id as usize;
    assert!(idx < per_cpu::MAX_CPUS, "sched: cpu_id exceeds MAX_CPUS");
    let current = Thread::adopt_current(bootstrap_name);
    let current_thread_id = current.id.0;

    // OT-49: give this CPU an idle thread — the always-available switch target
    // for a thread leaving the CPU when nothing else is runnable. Its stack and
    // slab slot are allocated here, at scheduler-install time, well after the
    // heap and slab are up (adopt_current above already uses the slab), so no
    // allocation happens on the halt/wake hot path.
    let idle = Thread::new(idle_thread_name(cpu_id), idle_thread_entry);
    let idle_tid = idle.id.0;
    IDLE_TID[idx].store(idle_tid, Ordering::Relaxed);

    let scheduler = Box::new(PerCpuScheduler {
        cpu_id,
        inner: SwitchMutex::new(PerCpuSchedulerInner {
            current,
            ready: SchedulerThreadRing::new(),
            zombies: SchedulerThreadRing::new(),
            token_parked: core::array::from_fn(|_| TokenParkSlot::empty()),
            idle_thread: Some(idle),
            idle_tid,
            #[cfg(feature = "browser-runtime")]
            live_count: 0,
        }),
        zombie_count: AtomicUsize::new(0),
    });
    let ptr = Box::into_raw(scheduler);

    let old = PER_CPU[idx]
        .scheduler_ptr
        .swap(ptr.cast(), Ordering::AcqRel);
    assert!(old.is_null(), "sched: per-CPU scheduler initialized twice");
    per_cpu::set_current_thread_id(current_thread_id);

    let scheduler = unsafe { &*ptr };
    serial_println!(
        "[ kernel ] sched: {} per-CPU scheduler ready (cpu={}, bootstrap='{}', ready_queue empty)",
        role,
        scheduler.cpu_id,
        bootstrap_name
    );
    scheduler
}

/// Return the scheduler for the current CPU. Panics if the calling CPU has not
/// installed its scheduler pointer yet; that indicates a boot-order bug.
fn current_scheduler() -> &'static PerCpuScheduler {
    let cpu_id = per_cpu::current_cpu_id();
    scheduler_for_cpu(cpu_id)
        .unwrap_or_else(|| panic!("current_scheduler: cpu_id={} not initialized", cpu_id))
}

fn scheduler_for_cpu(cpu_id: u32) -> Option<&'static PerCpuScheduler> {
    let idx = cpu_id as usize;
    if idx >= per_cpu::MAX_CPUS {
        return None;
    }
    let ptr = PER_CPU[idx]
        .scheduler_ptr
        .load(Ordering::Acquire)
        .cast::<PerCpuScheduler>();
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { &*ptr })
}

/// Spawn a new kernel thread that will start executing `entry` on its first
/// switch-in. Pushes the thread onto the current CPU's ready queue.
pub fn spawn(name: &'static str, entry: extern "C" fn() -> !) {
    let thread = Thread::new(name, entry);
    let id = thread.id;
    push_ready_or_panic(thread, "sched::spawn");
    serial_println!("[ kernel ] sched: spawned '{}' (id={})", name, id.0);
}

/// Try to push a pre-built thread onto the current CPU's ready queue. Used by
/// `process::spawn_user` which constructs Thread::new_user externally (so the
/// spawn path can validate the ELF first and fail before touching the
/// scheduler).
pub fn try_push_ready(thread: Box<Thread>) -> Result<(), Box<Thread>> {
    let id = thread.id;
    // The admission chokepoint is selected by cfg-ing the WHOLE closure, not by
    // a forwarding wrapper or an inner `let`: at opt-level 0 a forwarder cost 21
    // bytes of .text here and even a `let r = …; r` binding cost 2 (both
    // measured), either of which would break the kill switch's OFF equality.
    // Written this way the OFF closure is character-identical to the pre-C1.1
    // one, so the default kernel's .text is byte-for-byte unchanged.
    #[cfg(feature = "browser-runtime")]
    let result = without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        admit_ready_locked(sched, &mut s, thread)
    });
    #[cfg(not(feature = "browser-runtime"))]
    let result = without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        push_ready_locked(sched, &mut s, thread, ReadyPlacement::Back)
    });
    if result.is_ok() {
        serial_println!("[ kernel ] sched: enqueued user thread (id={})", id.0);
    }
    result
}

/// OT-61 C1.1 (AAG blocker A): admit a NEW thread to the ready ring. Distinct
/// from the re-entry pushes (`wake_*`, yield rotate) because admission is the
/// only operation that raises the live-thread total. Under `browser-runtime`
/// it is capped at `SCHEDULER_RING_CAPACITY` so `live_count <= capacity`
/// stays invariant, which in turn guarantees every wake/yield re-entry push
/// has a free ready slot (they never raise `live_count`).
///
/// Exists ONLY under `browser-runtime`; the OFF build calls `push_ready_locked`
/// directly at each call site (a forwarder, even `#[inline(always)]`, cost
/// measurable .text at opt-level 0). So the default kernel carries neither this
/// function nor any `live_count` state — see the field's own note.
#[cfg(feature = "browser-runtime")]
fn admit_ready_locked(
    sched: &PerCpuScheduler,
    s: &mut PerCpuSchedulerInner,
    thread: Box<Thread>,
) -> Result<(), Box<Thread>> {
    if s.live_count >= SCHEDULER_RING_CAPACITY {
        return Err(thread);
    }
    let result = push_ready_locked(sched, s, thread, ReadyPlacement::Back);
    if result.is_ok() {
        s.live_count += 1;
    }
    result
}

/// Infallible wrapper for internal tests and kernel paths that rely on ready
/// capacity as an invariant rather than an ABI-visible error.
#[allow(dead_code)]
pub fn push_ready(thread: Box<Thread>) {
    push_ready_or_panic(thread, "sched::push_ready");
}

/// Push a scheduler-owned kthread onto the current CPU's ready queue.
#[allow(dead_code)]
pub(super) fn push_ready_kthread(thread: Box<Thread>) {
    let id = thread.id;
    let name = thread.name;
    push_ready_or_panic(thread, "sched::push_ready_kthread");
    serial_println!(
        "[ kernel ] sched: enqueued kthread '{}' (id={})",
        name,
        id.0
    );
}

fn push_ready_or_panic(thread: Box<Thread>, context: &'static str) {
    let id = thread.id;
    // Kernel-thread admission goes through the same live-count accounting as
    // `try_push_ready` (blocker A) so `ready.len() <= live_count` holds for
    // every ready thread, not only user threads. Kernel threads are bounded
    // and never approach the cap, so the panic-on-full stays a fail-loud
    // backstop.
    // Whole-closure cfg, same reason as `try_push_ready` above (OFF equality).
    #[cfg(feature = "browser-runtime")]
    let pushed = without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        admit_ready_locked(sched, &mut s, thread)
    });
    #[cfg(not(feature = "browser-runtime"))]
    let pushed = without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        push_ready_locked(sched, &mut s, thread, ReadyPlacement::Back)
    });
    if let Err(thread) = pushed {
        let _ = Box::into_raw(thread);
        panic!(
            "{}: ready ring full (cap={}, thread_id={})",
            context, SCHEDULER_RING_CAPACITY, id.0
        );
    }
}

#[derive(Copy, Clone)]
enum ReadyPlacement {
    Back,
    Front,
}

fn push_ready_locked(
    sched: &PerCpuScheduler,
    s: &mut PerCpuSchedulerInner,
    thread: Box<Thread>,
    placement: ReadyPlacement,
) -> Result<(), Box<Thread>> {
    stamp_thread_cpu(&thread, sched.cpu_id);
    let result = match placement {
        ReadyPlacement::Back => s.ready.push_back(thread),
        ReadyPlacement::Front => s.ready.push_front(thread),
    };
    if result.is_ok() {
        crate::observability::record_ready_depth(sched.cpu_id, s.ready.len());
    }
    result
}

fn push_ready_locked_or_panic(
    sched: &PerCpuScheduler,
    s: &mut PerCpuSchedulerInner,
    thread: Box<Thread>,
    placement: ReadyPlacement,
    context: &'static str,
) {
    if let Err(thread) = push_ready_locked(sched, s, thread, placement) {
        let _ = Box::into_raw(thread);
        panic!(
            "{}: ready ring full (cap={}, len={})",
            context,
            s.ready.capacity(),
            s.ready.len()
        );
    }
}

fn push_zombie_locked(
    sched: &PerCpuScheduler,
    s: &mut PerCpuSchedulerInner,
    thread: Box<Thread>,
    context: &'static str,
) {
    if let Err(thread) = s.zombies.push_back(thread) {
        let _ = Box::into_raw(thread);
        panic!(
            "{}: zombie ring full (cap={}, len={})",
            context,
            s.zombies.capacity(),
            s.zombies.len()
        );
    }
    sched.zombie_count.fetch_add(1, Ordering::Release);
}

#[allow(dead_code)]
pub fn debug_scheduler_ring_capacity() -> usize {
    SCHEDULER_RING_CAPACITY
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_current_ready_len() -> usize {
    without_interrupts(|| {
        let sched = current_scheduler();
        sched.inner.lock().ready.len()
    })
}

/// Number of threads parked in this CPU's wake-token table (Blocked, off the ready
/// queue). Used by the Phase 7 BUILD-2L live-path gate to prove a deferred caller
/// **actually parked** (is not busy-waiting): a spinning thread would be Ready/Running,
/// not token-parked.
#[allow(dead_code)]
pub fn debug_token_parked_count() -> usize {
    without_interrupts(|| {
        let sched = current_scheduler();
        let s = sched.inner.lock();
        s.token_parked
            .iter()
            .filter(|slot| slot.thread.is_some())
            .count()
    })
}

/// OT-49 (AAG B1): total idle-thread loop entries across all CPUs. A scenario
/// folds this into its PASS string; `> 0` proves the park→idle path fired (see
/// `IDLE_ENTERED`). Sums the per-CPU counters; APs contribute 0 (they never park).
#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_idle_entered_total() -> u64 {
    IDLE_ENTERED.iter().map(|c| c.load(Ordering::Relaxed)).sum()
}

/// Shared queue-rotation primitive used by both `yield_now` and `exit`.
/// Holds the lock just long enough to swap `current` with the front of the
/// ready queue, place the previous current into `dest` (either `ready` for
/// yield or `zombies` for exit), and capture the `*mut u64` to its rsp slot
/// for the asm `context_switch`.
///
/// **Phase 1.4: cross-process switches** — when `prev.pid != next.pid`,
/// performs the canonical 3-step ordering BEFORE returning the switch
/// params: (1) update TSS.rsp0 + PER_CPU.kernel_rsp to next thread's
/// kernel stack top, (2) Cr3::write next process's pmap. The asm switch
/// follows in the caller. Order is load-bearing; see ISC-64b. Kernel
/// process (pid=0) has no Pmap — CR3 stays at the kernel PML4 set at
/// boot — so we skip the Cr3::write when next.pid == KERNEL_PID.
///
/// Returns switch parameters or `None` if the ready queue is empty. The caller
/// holds the scheduler guard `s` and decides WHEN to release it relative to the
/// asm switch:
///
/// - `yield_now`/idle call [`rotate`], which drops the guard BEFORE the switch
///   (pre-OT-31 behaviour, unchanged).
/// - `exit` holds the guard ACROSS the switch and releases it on the new stack,
///   so a concurrent `reap()` cannot free the zombie's stack while this CPU is
///   still on it (OT-31).
///
/// This helper returns a bare `SwitchParams` — NEVER a `(SwitchParams, guard)`
/// tuple. That is load-bearing: the kernel keeps compiler SSE enabled (the AI
/// matmul relies on it), so a large-aggregate move on this hot path can lower to
/// `movups %xmm*`, which would clobber the OUTGOING thread's live FPU before
/// `switch_context` saves it (caught by `xsave_ymm_context_switch`). Keeping the
/// return small and the guard a caller-owned local avoids that SSE lowering.
/// This helper never touches an AI-governance lock (I-14a).
fn rotate_compute(
    s: &mut PerCpuSchedulerInner,
    prev_state: ThreadState,
    dest: WhichQueue,
) -> Option<SwitchParams> {
    let sched = current_scheduler();
    let next = match s.ready.pop_front() {
        Some(next) => next,
        None => match dest {
            // Cooperative yield (`yield_now`) and the idle thread's own
            // re-check both land here with nothing else runnable: yield_now
            // keeps the current thread running (historical no-op) and the idle
            // thread halts — either way there is no switch, so return None.
            WhichQueue::Ready => return None,
            // exit(): the current thread is dying and MUST leave the CPU, so
            // fall back to this CPU's always-available idle thread (OT-49).
            // `idle_thread` is None only while the idle thread is itself current
            // — which never calls exit() — so a missing idle thread here is a
            // real invariant violation, not the empty-ready-queue case.
            WhichQueue::Zombie => s
                .idle_thread
                .take()
                .expect("rotate(exit): ready queue empty and idle thread already current"),
        },
    };
    record_scheduling_decision();
    let mut prev = mem::replace(&mut s.current, next);
    prev.state = prev_state;
    s.current.state = ThreadState::Running;
    per_cpu::set_current_thread_id(s.current.id.0);
    #[cfg(feature = "browser-runtime")]
    per_cpu::set_current_pid(u64::from(s.current.pid.0));
    crate::observability::record_dispatch(s.ready.len());
    let next_rsp = s.current.rsp;
    #[cfg(feature = "xsave-fpu")]
    let next_fpu = s.current.fpu_state_ptr();

    // Phase 1.4: handle cross-process boundary work BEFORE the asm switch.
    // - TSS.rsp0 + PER_CPU.kernel_rsp must point at next's kernel stack
    //   so any subsequent ring 0 transition (SYSCALL or int from user) lands
    //   on the right stack.
    // - CR3 swap when crossing into a different process (with its own Pmap).
    //   Kernel process (pid=0) has no Pmap; CR3 stays put.
    if prev.pid != s.current.pid {
        cross_process_prep(&s.current);
    } else if s.current.pid != KERNEL_PID {
        // Same user pid (multi-thread within one user process — Phase 1.5+),
        // still need TSS.rsp0 update because each thread has its own kernel
        // stack. Route through the CR3 cache as well so an initially-unknown
        // per-CPU cache is corrected and redundant same-Pmap loads are skipped.
        same_process_user_prep(&s.current);
    }

    let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
    #[cfg(feature = "xsave-fpu")]
    let prev_fpu = prev.fpu_state_ptr();
    // Where does the outgoing thread go? The idle thread is never enqueued as
    // ordinary ready work (it would then hold a ready slot and be schedulable),
    // so it returns to its per-CPU `idle_thread` slot. This branch is reached
    // when a wake races the timer and `on_tick` preempts the halted idle thread
    // (dest == Ready). Every other thread goes to `dest`: ready for a yield,
    // zombies for an exit.
    if prev.id.0 == s.idle_tid {
        debug_assert!(
            s.idle_thread.is_none(),
            "rotate: idle slot occupied while the idle thread was current"
        );
        s.idle_thread = Some(prev);
    } else {
        match dest {
            WhichQueue::Ready => {
                push_ready_locked_or_panic(sched, &mut *s, prev, ReadyPlacement::Back, "rotate");
            }
            WhichQueue::Zombie => {
                push_zombie_locked(sched, &mut *s, prev, "rotate");
            }
        }
    }
    Some(SwitchParams {
        prev_rsp_ptr,
        next_rsp,
        #[cfg(feature = "xsave-fpu")]
        prev_fpu,
        #[cfg(feature = "xsave-fpu")]
        next_fpu,
    })
}

/// `yield_now`/idle rotation: run [`rotate_compute`] under the scheduler lock and
/// release the lock BEFORE the caller switches (the pre-OT-31 behaviour,
/// unchanged). The guard is a local dropped here, so the returned `SwitchParams`
/// is used by the caller with the lock already free.
fn rotate(prev_state: ThreadState, dest: WhichQueue) -> Option<SwitchParams> {
    // OT-74: the scheduler lock is shared with the timer ISR; holding it with
    // IF=1 is a single-CPU deadlock. Both callers (yield, idle) mask first.
    debug_assert!(
        !crate::arch::x86_64::interrupts_enabled(),
        "sched::rotate entered with IF=1"
    );
    let sched = current_scheduler();
    let mut s = sched.inner.lock();
    let params = rotate_compute(&mut s, prev_state, dest);
    // OT-61 C1.1 hang discriminator: mirror this CPU's dispatch state into
    // lock-free atomics while the scheduler lock is STILL HELD, so a later dump
    // can read every CPU's ready depth and current thread **without taking a
    // remote scheduler lock** — no cross-CPU deadlock surface and no lockdep
    // interaction from the timer ISR. `ROTATE` is the scheduler heartbeat: a CPU
    // with `READY_LEN > 0` and a frozen `ROTATE` is a stranded-wake CPU
    // (CP-15/CP-21), which is the whole question this hunt has to answer.
    #[cfg(feature = "neo-test")]
    wake_trace::note_rotate(
        sched.cpu_id,
        s.ready.len(),
        s.current.id.0,
        s.current.pid != KERNEL_PID,
    );
    drop(s);
    params
}

/// Update TSS.rsp0 and PER_CPU.kernel_rsp to the kernel stack top of `next`,
/// then load `next`'s process pmap into CR3 (if it has one). Order matters
/// per ISC-64b: stack-top updates first (so an interrupt fired between the
/// updates lands on the correct kernel stack), CR3 swap second.
fn cross_process_prep(next: &Thread) {
    cross_thread_kstack(next);
    let (frame, flags) = address_space_for_thread(next);
    switch_to_address_space(frame, flags);
    // A kernel thread never runs ring 3 and kernel code does not use FS, so it
    // leaves FS.base alone; the next switch into a user thread writes it.
    #[cfg(feature = "browser-runtime")]
    if next.pid != KERNEL_PID {
        load_user_fs_base(next.fs_base);
    }
}

fn same_process_user_prep(next: &Thread) {
    cross_thread_kstack(next);
    let (frame, flags) = address_space_for_thread(next);
    switch_to_address_space(frame, flags);
    #[cfg(feature = "browser-runtime")]
    load_user_fs_base(next.fs_base);
}

/// OT-61 C1.2 (P3): load `base` into this CPU's `IA32_FS_BASE`. Every switch
/// into a user thread runs this (both prep paths), so a thread sees its own
/// FS.base after a context switch or a migration, and a thread that never set
/// one sees 0, never another thread's or another process's.
///
/// Always written, never skipped by a cache (Jarvis, C1.2 review). Ring 3 can
/// change the hardware FS.base behind the kernel's back: loading a selector
/// into FS (`mov fs, ax` with the flat user data selector, `gdt.rs`) sets the
/// base from the descriptor, which is 0, with or without FSGSBASE (SDM Vol. 3A
/// §3.4.4). A per-CPU "already loaded" cache then lies: a switch back into any
/// thread whose value equals the cached one, in this process or another,
/// skips the WRMSR and runs it with base 0; and `SYS_THREAD_SET_TLS` with the
/// cached value returns 0 without loading it. One WRMSR per switch into a user
/// thread costs on the order of a hundred cycles (serializing; not measured on
/// our silicon), small next to the CR3 load the same switch makes.
///
/// So the hardware equals the thread's field after every switch-in and every
/// `SYS_THREAD_SET_TLS`. A thread that loads FS itself changes only its own
/// view until its next switch-in or set, which put the field back.
#[cfg(feature = "browser-runtime")]
fn load_user_fs_base(base: u64) {
    // SAFETY: `base` is 0 or a canonical user address that
    // `SYS_THREAD_SET_TLS` validated; WRMSR to IA32_FS_BASE takes any
    // canonical value and changes only how ring 3 resolves `fs:` operands.
    unsafe { x86_64::registers::model_specific::Msr::new(IA32_FS_BASE).write(base) };
}

/// The FS-segment base MSR.
#[cfg(feature = "browser-runtime")]
const IA32_FS_BASE: u32 = 0xC000_0100;

/// OT-61 C1.2 (P3): set the calling thread's FS.base and load it now. The
/// thread's field, the MSR and the per-CPU cache change together under this
/// CPU's scheduler lock with interrupts off, so a preemption can't observe one
/// without the others: the next switch-in reloads from the field either way.
#[cfg(feature = "browser-runtime")]
pub fn set_current_fs_base(base: u64) {
    without_interrupts(|| {
        let mut s = current_scheduler().inner.lock();
        s.current.fs_base = base;
        load_user_fs_base(base);
    });
}

/// OT-61 C1.2 test-only (`threads_tls`): one parked thread, so a thread on
/// another CPU can wake it with `wake_thread_on_current_cpu`, the migrating wake
/// the IPC handoff uses, and the battery can check FS.base after a real
/// cross-CPU migration. Parking uses `block_current_into_with_guard`, so this
/// lock is released only after the switch has saved the parked thread's RSP (the
/// IPC discipline): a waker can never take a thread that is still running. A
/// thread parked here is invisible to the kill scans; the battery always
/// unparks it before it ends, and nothing else uses this.
#[cfg(feature = "neo-test-threads-tls")]
static TLS_MIGRATION_PARK: crate::sync::IrqSafeMutex<Option<Box<Thread>>> =
    crate::sync::IrqSafeMutex::new(None);

/// Park the calling thread in [`TLS_MIGRATION_PARK`] until
/// [`test_tls_unpark_onto_current_cpu`] wakes it. `-EBUSY` if a thread is
/// already parked.
#[cfg(feature = "neo-test-threads-tls")]
pub fn test_tls_park_current() -> i64 {
    let guard = TLS_MIGRATION_PARK.lock();
    if guard.is_some() {
        return crate::errno::EBUSY;
    }
    block_current_into_with_guard(guard, |slot, prev| *slot = Some(prev))
}

/// Wake the thread parked in [`TLS_MIGRATION_PARK`] onto THIS CPU. `false` if
/// none is parked yet.
#[cfg(feature = "neo-test-threads-tls")]
pub fn test_tls_unpark_onto_current_cpu() -> bool {
    let parked = TLS_MIGRATION_PARK.lock().take();
    match parked {
        Some(thread) => {
            wake_thread_on_current_cpu(thread, 0);
            true
        }
        None => false,
    }
}

/// OT-61 C1.2 test support: the calling thread's recorded FS.base and this
/// CPU's live `IA32_FS_BASE`, read together with interrupts off.
#[cfg(all(feature = "browser-runtime", feature = "neo-test"))]
pub fn current_fs_base_and_msr() -> (u64, u64) {
    without_interrupts(|| {
        let s = current_scheduler().inner.lock();
        // SAFETY: RDMSR of IA32_FS_BASE has no side effects.
        let msr = unsafe { x86_64::registers::model_specific::Msr::new(IA32_FS_BASE).read() };
        (s.current.fs_base, msr)
    })
}

fn address_space_for_thread(next: &Thread) -> (PhysFrame, Cr3Flags) {
    next.pmap
        .as_ref()
        .map(|pmap| pmap.cr3())
        .unwrap_or_else(process::kernel_cr3)
}

fn switch_to_address_space(frame: PhysFrame, flags: Cr3Flags) {
    let pc = per_cpu::current_per_cpu();
    let target = per_cpu::encode_cr3(frame, flags);
    let active = pc.active_pmap_cr3.load(Ordering::Relaxed);

    if active == 0 || active != target {
        // SAFETY: every Pmap in the registry was constructed via Pmap::new
        // which copies kernel-half PML4 entries, so kernel pages remain
        // accessible immediately after any CR3 swap. The cache store follows
        // the hardware write per SC-3C D1/B2.a.
        unsafe { Cr3::write(frame, flags) };
        pc.active_pmap_cr3.store(target, Ordering::Relaxed);
        #[cfg(feature = "neo-test-instrumentation")]
        pc.cr3_writes_performed.fetch_add(1, Ordering::Relaxed);
    } else {
        #[cfg(feature = "neo-test-instrumentation")]
        pc.cr3_writes_skipped.fetch_add(1, Ordering::Relaxed);
    }
}

/// Update TSS.rsp0 and PER_CPU.kernel_rsp to the kernel stack top of
/// `next`. Used both on cross-process transitions and (Phase 1.5+) on
/// same-process cross-thread transitions.
fn cross_thread_kstack(next: &Thread) {
    if let Some(rsp) = next.kernel_stack_top() {
        gdt::set_kernel_rsp(rsp);
        per_cpu::set_kernel_rsp(rsp);
    }
    // None case: bootstrap "main" kernel thread — owns no Box stack;
    // TSS.rsp0 / per_cpu untouched. Main thread is never the target of a
    // syscall (it doesn't run user code), so this is safe.
}

#[cfg(feature = "neo-test-instrumentation")]
pub struct Cr3WriteCounters {
    pub skipped: u64,
    pub performed: u64,
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn reset_current_cr3_write_counters() {
    let pc = per_cpu::current_per_cpu();
    pc.cr3_writes_skipped.store(0, Ordering::Relaxed);
    pc.cr3_writes_performed.store(0, Ordering::Relaxed);
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn current_cr3_write_counters() -> Cr3WriteCounters {
    let pc = per_cpu::current_per_cpu();
    Cr3WriteCounters {
        skipped: pc.cr3_writes_skipped.load(Ordering::Relaxed),
        performed: pc.cr3_writes_performed.load(Ordering::Relaxed),
    }
}

#[cfg(feature = "neo-test-instrumentation")]
pub struct Phase2PerfProxyCounters {
    pub context_switches: u64,
    pub scheduling_decisions: u64,
    pub cr3_writes_performed: u64,
    pub cr3_writes_skipped: u64,
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn reset_phase2_perf_proxy_counters() {
    CONTEXT_SWITCH_COUNT.store(0, Ordering::Relaxed);
    SCHEDULING_DECISION_COUNT.store(0, Ordering::Relaxed);
    reset_current_cr3_write_counters();
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn phase2_perf_proxy_counters() -> Phase2PerfProxyCounters {
    let cr3 = current_cr3_write_counters();
    Phase2PerfProxyCounters {
        context_switches: CONTEXT_SWITCH_COUNT.load(Ordering::Relaxed),
        scheduling_decisions: SCHEDULING_DECISION_COUNT.load(Ordering::Relaxed),
        cr3_writes_performed: cr3.performed,
        cr3_writes_skipped: cr3.skipped,
    }
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn context_switch_count_for_test() -> u64 {
    CONTEXT_SWITCH_COUNT.load(Ordering::Acquire)
}

#[inline(always)]
fn record_context_switch() {
    #[cfg(feature = "neo-test-instrumentation")]
    CONTEXT_SWITCH_COUNT.fetch_add(1, Ordering::Relaxed);
    crate::observability::record_context_switch();
}

#[inline(always)]
fn record_scheduling_decision() {
    #[cfg(feature = "neo-test-instrumentation")]
    SCHEDULING_DECISION_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[derive(Copy, Clone)]
enum WhichQueue {
    Ready,
    Zombie,
}

struct SwitchParams {
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    #[cfg(feature = "xsave-fpu")]
    prev_fpu: *mut u8,
    #[cfg(feature = "xsave-fpu")]
    next_fpu: *mut u8,
}

unsafe fn switch_context(params: SwitchParams) {
    #[cfg(not(feature = "xsave-fpu"))]
    {
        // SAFETY: caller upholds the scheduler/context_switch invariants for
        // the captured previous RSP slot and next saved RSP.
        unsafe { context_switch(params.prev_rsp_ptr, params.next_rsp) };
    }

    #[cfg(feature = "xsave-fpu")]
    {
        // SAFETY: caller upholds the scheduler/context_switch invariants. The
        // FPU pointers are captured from live Thread boxes before ownership is
        // moved into scheduler queues or parking holders, so the save areas
        // remain heap-stable across the asm switch.
        unsafe {
            context_switch(
                params.prev_rsp_ptr,
                params.next_rsp,
                params.prev_fpu,
                params.next_fpu,
            )
        };
    }
}

unsafe fn switch_context_with_release2(
    params: SwitchParams,
    channel_lock: *const AtomicBool,
    sched_lock: *const AtomicBool,
) {
    #[cfg(not(feature = "xsave-fpu"))]
    {
        // SAFETY: caller upholds the release-on-new-stack contract.
        unsafe {
            context_switch_with_release2(
                params.prev_rsp_ptr,
                params.next_rsp,
                channel_lock,
                sched_lock,
            )
        };
    }

    #[cfg(feature = "xsave-fpu")]
    {
        // SAFETY: caller upholds the release-on-new-stack contract and the FPU
        // save-area pointer lifetime described in `switch_context`.
        unsafe {
            context_switch_with_release2(
                params.prev_rsp_ptr,
                params.next_rsp,
                params.prev_fpu,
                params.next_fpu,
                channel_lock,
                sched_lock,
            )
        };
    }
}

/// The single-lock sibling of [`switch_context_with_release2`], used by
/// [`exit`]. Switches to `next_rsp`, then releases the one scheduler lock byte
/// on the NEW stack — so a concurrent `reap()` (which needs that same lock to
/// pop and `Thread::free` the zombie) cannot free a stack this CPU is still
/// executing on. See OT-31.
unsafe fn switch_context_with_release1(params: SwitchParams, sched_lock: *const AtomicBool) {
    #[cfg(not(feature = "xsave-fpu"))]
    {
        // SAFETY: caller upholds the release-on-new-stack contract.
        unsafe { context_switch_with_release1(params.prev_rsp_ptr, params.next_rsp, sched_lock) };
    }

    #[cfg(feature = "xsave-fpu")]
    {
        // SAFETY: caller upholds the release-on-new-stack contract and the FPU
        // save-area pointer lifetime described in `switch_context`.
        unsafe {
            context_switch_with_release1(
                params.prev_rsp_ptr,
                params.next_rsp,
                params.prev_fpu,
                params.next_fpu,
                sched_lock,
            )
        };
    }
}

enum TokenTakeResult {
    Thread(Box<Thread>),
    Gone,
    Stale,
}

impl PerCpuSchedulerInner {
    fn free_token_slot(&self) -> Option<usize> {
        self.token_parked
            .iter()
            .position(|slot| slot.thread.is_none())
    }

    fn park_thread_token(
        &mut self,
        cpu_id: u32,
        slot_idx: usize,
        thread: Box<Thread>,
    ) -> ThreadWakeToken {
        let slot = &mut self.token_parked[slot_idx];
        debug_assert!(
            slot.thread.is_none(),
            "park_thread_token: slot must be empty"
        );
        // Finding 2(d): widened to u64. Generation establishes token OWNERSHIP,
        // not lifetime, and a u32 counter wraps after ~4e9 parks on one slot —
        // a theoretical ABA where a stale token matches a recycled slot. At u64
        // the wrap is unreachable for any real uptime, so the ownership check
        // cannot be defeated by counter reuse. (Lifetime is handled separately
        // by the `Thread::process` anchor — generation was never sufficient for
        // it, which is precisely what the cross-CPU UAF exposed.)
        slot.generation = slot.generation.wrapping_add(1).max(1);
        let tid = thread.id.0;
        slot.thread = Some(thread);
        // OT-61 C1.1 hang discriminator: every token park, counted per owning
        // CPU. Paired against `TOKEN_WOKEN`/`STALE`/`GONE`, a park surplus on
        // one CPU is a waiter that never got its wake.
        #[cfg(feature = "neo-test")]
        wake_trace::note_token_park(cpu_id);
        ThreadWakeToken {
            cpu_id,
            slot: slot_idx as u32,
            generation: slot.generation,
            tid,
        }
    }

    fn take_thread_by_token(&mut self, token: ThreadWakeToken) -> TokenTakeResult {
        let Some(slot) = self.token_parked.get_mut(token.slot as usize) else {
            return TokenTakeResult::Stale;
        };
        if slot.generation != token.generation {
            return TokenTakeResult::Stale;
        }
        match slot.thread.take() {
            Some(thread) => TokenTakeResult::Thread(thread),
            None => TokenTakeResult::Gone,
        }
    }

    fn take_token_thread_by_pid(&mut self, pid: ProcessId) -> Option<Box<Thread>> {
        // CP-66: an admission-parked thread is cancelled (woken with a verdict
        // by `admission::cancel_pid`), never taken raw.
        let slot = self.token_parked.iter_mut().find(|slot| {
            slot.thread
                .as_ref()
                .is_some_and(|thread| thread.pid == pid && !kill_protected(thread))
        })?;
        slot.thread.take()
    }

    #[cfg(any(feature = "neo-test", feature = "pipe"))]
    fn token_slot_occupied(&self, token: ThreadWakeToken) -> bool {
        self.token_parked
            .get(token.slot as usize)
            .is_some_and(|slot| slot.generation == token.generation && slot.thread.is_some())
    }
}

/// Cooperative yield. Rotates the current thread to the back of the ready
/// queue and switches into the front. No-op if the ready queue is empty.
pub fn yield_now() {
    yield_now_recording(YieldKind::Voluntary);
}

#[derive(Copy, Clone)]
enum YieldKind {
    Voluntary,
    TimerPreemption,
}

fn yield_now_recording(kind: YieldKind) {
    without_interrupts(|| {
        let switch_params = rotate(ThreadState::Ready, WhichQueue::Ready);
        // MutexGuard from rotate() is dropped at the end of that fn.
        if let Some(switch_params) = switch_params {
            match kind {
                YieldKind::Voluntary => crate::observability::record_voluntary_yield(),
                YieldKind::TimerPreemption => crate::observability::record_timer_preemption(),
            }
            record_context_switch();
            // SAFETY: see rotate() and module-level comment.
            unsafe { switch_context(switch_params) };
        }
    });
}

/// Called from the timer ISR's Rust handler. Phase 1.3 always preempts when
/// the ready queue is non-empty. The `yield_now` call is a no-op when the
/// queue is empty, so we don't bother with a pre-check. We're already inside
/// the timer asm wrapper, which entered with IF=0 (interrupt gate) — but
/// `yield_now` wraps in `without_interrupts` defensively.
pub fn on_tick() {
    crate::observability::record_scheduler_tick();
    #[cfg(feature = "neo-test")]
    if current_is_idle() {
        IDLE_TICKS[per_cpu::current_cpu_id() as usize].fetch_add(1, Ordering::Relaxed);
    }
    // OT-49 (AAG B4): deterministic idle-entry probe wake. Test-only — the cfg
    // removes this call entirely from a production kernel, so the timer hot path
    // is bit-for-bit unchanged (ISC-3).
    #[cfg(feature = "neo-test-ot49-idle-entry-wake")]
    idle_probe_tick_wake();
    if PREEMPT_DISABLE_DEPTH[per_cpu::current_cpu_id() as usize].load(Ordering::Acquire) != 0 {
        return;
    }
    if consume_preempt_defer() {
        return;
    }
    yield_now_recording(YieldKind::TimerPreemption);
}

/// OT-tick-1 step 1 (relay 381, test only): where every thread the
/// schedulers hold is, and the BSP's preemption gates, printed from the BSP's
/// tick. Per CPU: its `PREEMPT_DISABLE_DEPTH`, then its current thread, its
/// ready ring in order and its token-parked threads, each as
/// `tid/pid/state/defer`. A thread in none of them is blocked outside the
/// scheduler (a channel, a futex, a wait-any). Takes each scheduler lock with
/// a try: a busy CPU is reported as `locked`, never waited on.
#[cfg(feature = "neo-test-tick-diag")]
pub(crate) fn tick_diag_dump(now: u64) {
    for idx in 0..per_cpu::MAX_CPUS {
        let Some(sched) = scheduler_for_cpu(idx as u32) else {
            continue;
        };
        let depth = PREEMPT_DISABLE_DEPTH[idx].load(Ordering::Relaxed);
        let Some(s) = sched.inner.try_lock() else {
            crate::serial_println_isr!("TICKDIAG t={} cpu={} depth={} locked", now, idx, depth);
            continue;
        };
        let row = |role: &str, t: &Thread| {
            crate::serial_println_isr!(
                "TICKDIAG t={} cpu={} {} tid={} pid={} state={:?} defer={}",
                now,
                idx,
                role,
                t.id.0,
                t.pid.0,
                t.state,
                t.preempt_defer_ticks
            );
        };
        crate::serial_println_isr!(
            "TICKDIAG t={} cpu={} depth={} ready={} idle_tid={}",
            now,
            idx,
            depth,
            s.ready.len(),
            s.idle_tid
        );
        row("cur", &s.current);
        let n = s.ready.capacity();
        for k in 0..n {
            if let Some(t) = s.ready.slots[(s.ready.head + k) % n].as_ref() {
                row("ready", t);
            }
        }
        for slot in s.token_parked.iter() {
            if let Some(t) = slot.thread.as_ref() {
                row("parked", t);
            }
        }
    }
}

/// OT-49 (AAG B4): one-shot wake token for the deterministic idle-entry probe.
/// `park_current_until_tick` stores the parking thread's token here; the next
/// `on_tick` takes it and wakes the thread. Test-only — never compiled into a
/// production kernel.
#[cfg(feature = "neo-test-ot49-idle-entry-wake")]
static IDLE_PROBE_TOKEN: SwitchMutex<Option<ThreadWakeToken>> = SwitchMutex::new(None);

/// OT-49 (AAG B4): park the current thread until the next timer tick. When the
/// caller is the only runnable thread on its CPU (the idle-entry scenario), the
/// park finds an empty ready queue and falls back to the per-CPU idle thread
/// (the OT-49 fix), which `sti;hlt`s until the tick fires and `on_tick` wakes
/// this token — a deterministic park→idle→timer-IRQ-wake with no I/O completion
/// race. The scenario reads `debug_idle_entered_total` around this call and
/// hard-asserts a non-zero delta, so a green gate demonstrates idle was entered.
#[cfg(feature = "neo-test-ot49-idle-entry-wake")]
pub fn park_current_until_tick() {
    // OT-74: `IDLE_PROBE_TOKEN` is also taken by the timer ISR
    // (`idle_probe_tick_wake`), so acquire it with IF already off.
    without_interrupts(|| {
        let _ = block_current_with_token_and_guard(IDLE_PROBE_TOKEN.lock(), |slot, token| {
            *slot = Some(token);
        });
    });
}

/// OT-49 (AAG B4): wake the one-shot probe token if a thread parked via
/// `park_current_until_tick`. Called once per tick from `on_tick`; a no-op
/// (one lock + `None`) when no probe is armed. `IDLE_PROBE_TOKEN` is released
/// before `wake_thread_token` takes the scheduler lock — no lock-order inversion.
#[cfg(feature = "neo-test-ot49-idle-entry-wake")]
fn idle_probe_tick_wake() {
    let token = IDLE_PROBE_TOKEN.lock().take();
    if let Some(token) = token {
        wake_thread_token(token, 0);
    }
}

/// B1c AAG J3/J4: mark (or unmark) the running thread as an inference
/// participant (`Thread::inference_lifecycle`).
#[cfg(feature = "inference-stack")]
pub fn set_current_inference_lifecycle(on: bool) {
    without_interrupts(|| {
        current_scheduler().inner.lock().current.inference_lifecycle = on;
    });
}

/// Whether a force-kill scan (ready queue or token table) must leave `thread`
/// where it is: its kernel stack owns `Arc`s that a zombie reap would leak, so
/// it cancels cooperatively and exits at the syscall boundary. Two owners:
/// CP-66, the accelerator admission section (graph `SYS_INFER_RUN`), and B1c's
/// CP-65, an inference-lifecycle participant (generate / inventory, the
/// shared inference stack). Each control build restores its own pre-fix rule.
fn kill_protected(thread: &Thread) -> bool {
    (crate::ai::admission::CANCEL_ON && thread.admission_section) || lifecycle_protected(thread)
}

/// OT-93 (CP-95): whether a force-kill may zombify this Ready thread, freeing
/// its kernel stack raw. Only when it resumes in ring 3: a thread Ready inside
/// a syscall (preempted with interrupts on, yielding, woken, or the sender
/// a rendezvous handed the CPU to) resumes in a syscall frame that may own
/// `Arc`s and `Vec`s, which a raw free would leak. It is left to exit at its
/// boundary instead (`leave_syscall`). A parked thread is not covered here:
/// a park site owns nothing (CP-30), so the parked-thread scans still take
/// it raw.
#[cfg(not(feature = "neo-test-kill-ready-in-kernel-control"))]
fn ready_kill_safe(thread: &Thread) -> bool {
    !thread.in_syscall
}

/// `kill_ready_in_kernel_control`: the pre-OT-93 rule, which zombified any
/// Ready thread.
#[cfg(feature = "neo-test-kill-ready-in-kernel-control")]
fn ready_kill_safe(_thread: &Thread) -> bool {
    true
}

/// Whether the running thread's process has been force-killed (OT-93). Read
/// under this CPU's scheduler lock, through the thread's own process anchor.
fn current_force_killed(s: &PerCpuSchedulerInner) -> bool {
    s.current
        .process
        .as_ref()
        .is_some_and(|proc| proc.is_force_killed())
}

/// Whether the running thread must not start a user wait: its process was
/// force-killed (OT-93), or (with `browser-runtime`) a fault kill or exit
/// group is retiring it. The kill and retire scans now leave a sibling that is
/// Ready inside a syscall; one that parked after their drains would never be
/// woken.
fn current_must_not_park(s: &PerCpuSchedulerInner) -> bool {
    s.current.process.as_ref().is_some_and(|proc| {
        #[cfg(feature = "browser-runtime")]
        if proc.is_retiring() {
            return true;
        }
        proc.is_force_killed()
    })
}

/// OT-93: syscall entry (`syscall_dispatch`). Clears the handoff preemption
/// credit, as before, and marks the thread as inside a syscall, so a
/// force-kill that finds it Ready leaves it to its boundary. A thread whose
/// process was force-killed while it was in ring 3 ends here; it holds
/// nothing yet.
pub fn enter_syscall() {
    let killed = without_interrupts(|| {
        let mut s = current_scheduler().inner.lock();
        s.current.preempt_defer_ticks = 0;
        s.current.in_syscall = true;
        current_force_killed(&s)
    });
    if killed {
        exit();
    }
}

/// OT-93: the syscall's return decision, with interrupts already off (the
/// caller disables them, CP-93). The syscall's frames have unwound, so a
/// thread whose process was force-killed while it was inside ends here. The
/// clear and the check are one section under the scheduler lock: a kill
/// either published `force_killed` before it (seen here) or finds the thread
/// running and outside the syscall, and its IPI is taken in ring 3.
pub fn leave_syscall() {
    let killed = {
        let mut s = current_scheduler().inner.lock();
        s.current.in_syscall = false;
        current_force_killed(&s)
    };
    if killed {
        exit();
    }
}

/// OT-93: the timer and reschedule-IPI handlers, when they interrupted
/// CPL 3: a thread whose process was force-killed ends there. In ring 3 it
/// holds no kernel lock and owns no kernel frame (CP-37, CP-86). One load
/// while no force-killed process exists.
pub fn exit_current_if_force_killed() {
    if !process::forced_kill_in_flight() {
        return;
    }
    let killed = without_interrupts(|| current_force_killed(&current_scheduler().inner.lock()));
    if killed {
        exit();
    }
}

/// OT-93: after `terminate`'s scans, the threads of `pid` it left alive:
/// Ready inside a syscall, or running on some CPU. Sends the reschedule IPI to
/// every other CPU running one; the handler ends it if it interrupted ring 3,
/// and from ring 0 its syscall return does. Returns how many there are.
pub fn kick_kill_survivors(pid: ProcessId) -> usize {
    let me = per_cpu::current_cpu_id();
    let mut survivors = 0;
    let mut kick = [false; per_cpu::MAX_CPUS];
    without_interrupts(|| {
        for (cpu, pc) in PER_CPU.iter().enumerate() {
            let ptr = pc
                .scheduler_ptr
                .load(Ordering::Acquire)
                .cast::<PerCpuScheduler>();
            if ptr.is_null() {
                continue;
            }
            // SAFETY: as in `kill_thread_by_pid`.
            let sched = unsafe { &*ptr };
            let s = sched.inner.lock();
            survivors += s.ready.count_by_pid_in_syscall(pid);
            if s.current.pid == pid {
                survivors += 1;
                if cpu as u32 != me {
                    kick[cpu] = true;
                }
            }
        }
    });
    for (cpu, &k) in kick.iter().enumerate() {
        if k {
            crate::arch::x86_64::apic::send_reschedule_ipi(cpu as u32);
        }
    }
    survivors
}

/// B1c AAG J3/J4 (CP-65): an inference participant, holder or waiter.
#[cfg(all(
    feature = "inference-stack",
    not(feature = "neo-test-inference-kill-control")
))]
fn lifecycle_protected(thread: &Thread) -> bool {
    thread.inference_lifecycle
}

/// `inference_stack_kill_control`: the pre-J3 rule, which skipped only the
/// stack's holder and so zombified a suspended waiter.
#[cfg(feature = "neo-test-inference-kill-control")]
fn lifecycle_protected(thread: &Thread) -> bool {
    crate::mm::inference_stack::owner_thread() == Some(thread.id.0)
}

#[cfg(not(feature = "inference-stack"))]
fn lifecycle_protected(_thread: &Thread) -> bool {
    false
}

/// CP-67: run a blocking channel operation (`op`, which may park the thread in
/// the channel) with `channel` owned by the running thread rather than the
/// caller's frame. A thread force-killed while parked is extracted from the
/// endpoint and reaped with its stack freed raw, so an `Arc<Channel>` on that
/// stack would never be dropped: one leaked `Channel` per kill. Owned by the
/// `Thread`, it is dropped by `Thread::free` instead; a thread that returns
/// (rendezvous, `EPIPE` on close, or never parked) takes it back here.
#[cfg(not(feature = "neo-test-ipc-park-kill-control"))]
pub fn with_channel_anchor<R>(
    channel: alloc::sync::Arc<crate::ipc::Channel>,
    op: impl FnOnce(&alloc::sync::Arc<crate::ipc::Channel>) -> R,
) -> R {
    let anchor: *const alloc::sync::Arc<crate::ipc::Channel> = without_interrupts(|| {
        let mut s = current_scheduler().inner.lock();
        debug_assert!(
            s.current.ipc_anchor.is_none(),
            "with_channel_anchor: nested IPC anchor"
        );
        s.current.ipc_anchor.insert(channel) as *const _
    });
    // SAFETY: the slot is a field of the running thread's boxed `Thread`,
    // whose storage does not move while the thread parks, migrates and
    // resumes. Only this thread writes it (here, below) until it is a reaped
    // zombie, and `Thread::free` cannot run while the thread can still
    // execute this frame.
    let ret = op(unsafe { &*anchor });
    let anchor = without_interrupts(|| current_scheduler().inner.lock().current.ipc_anchor.take());
    // Dropped outside the scheduler lock: it may be the channel's last count.
    drop(anchor);
    ret
}

/// CP-67 for the pipe (L1a.1 step 2a): run a blocking pipe operation with
/// `pipe` owned by the running thread rather than the caller's frame, as
/// [`with_channel_anchor`] does for a channel. A thread force-killed while
/// parked in the pipe is reaped with its stack freed raw; owned by the
/// `Thread`, the `Arc` is dropped by `Thread::free` instead.
#[cfg(all(feature = "pipe", not(feature = "neo-test-pipe-park-kill-control")))]
pub fn with_pipe_anchor<R>(
    pipe: alloc::sync::Arc<crate::ipc::pipe::PipeObject>,
    op: impl FnOnce(&alloc::sync::Arc<crate::ipc::pipe::PipeObject>) -> R,
) -> R {
    let anchor: *const alloc::sync::Arc<crate::ipc::pipe::PipeObject> = without_interrupts(|| {
        let mut s = current_scheduler().inner.lock();
        debug_assert!(
            s.current.pipe_anchor.is_none(),
            "with_pipe_anchor: nested pipe anchor"
        );
        s.current.pipe_anchor.insert(pipe) as *const _
    });
    // SAFETY: as in `with_channel_anchor`: the slot is a field of the running
    // thread's boxed `Thread`, which doesn't move while the thread parks,
    // migrates and resumes; only this thread writes it until it is a reaped
    // zombie, and `Thread::free` can't run while this frame can still run.
    let ret = op(unsafe { &*anchor });
    let anchor = without_interrupts(|| current_scheduler().inner.lock().current.pipe_anchor.take());
    // Dropped outside the scheduler lock: it may be the pipe's last count.
    drop(anchor);
    ret
}

/// `pipe_park_kill_control`: the pre-fix shape, the pipe owned by the
/// syscall frame across the park (the per-kill leak counter must catch it).
#[cfg(all(feature = "pipe", feature = "neo-test-pipe-park-kill-control"))]
pub fn with_pipe_anchor<R>(
    pipe: alloc::sync::Arc<crate::ipc::pipe::PipeObject>,
    op: impl FnOnce(&alloc::sync::Arc<crate::ipc::pipe::PipeObject>) -> R,
) -> R {
    op(&pipe)
}

/// `ipc_park_kill_control`: the pre-fix shape, the channel owned by the
/// syscall frame across the park.
#[cfg(feature = "neo-test-ipc-park-kill-control")]
pub fn with_channel_anchor<R>(
    channel: alloc::sync::Arc<crate::ipc::Channel>,
    op: impl FnOnce(&alloc::sync::Arc<crate::ipc::Channel>) -> R,
) -> R {
    op(&channel)
}

/// CP-66: mark (or unmark) the current thread as inside the accelerator
/// admission section (see `Thread::admission_section`).
pub fn set_current_admission_section(on: bool) {
    without_interrupts(|| {
        current_scheduler().inner.lock().current.admission_section = on;
    });
}

pub fn defer_current_preempt_for_handoff() {
    without_interrupts(|| {
        current_scheduler().inner.lock().current.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;
    });
}

fn consume_preempt_defer() -> bool {
    without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        if s.current.preempt_defer_ticks == 0 {
            return false;
        }
        s.current.preempt_defer_ticks -= 1;
        true
    })
}

/// Terminate the current thread. Moves it to the zombie queue (NOT
/// re-enqueued in ready), then switches to the next ready thread. Never
/// returns.
///
/// SAFETY of the Box-pin trick (OT-31): `rotate_locked` pushes this thread's
/// `Box<Thread>` into `zombies` under the scheduler lock, and that lock is held
/// ACROSS the switch below and released only on the NEW stack
/// (`context_switch_with_release1`). The asm writes `*prev_rsp_ptr` and the FPU
/// save area INTO the (heap-pinned) zombie Box while this CPU is still on the
/// zombie's stack; because `reap()` must take the same scheduler lock to pop and
/// `Thread::free` a zombie, it cannot reclaim that stack until the lock is
/// released on the new stack — i.e. until this CPU is off it. (Pre-OT-31 the
/// lock was dropped BEFORE the switch, leaving a cross-CPU window in which reap
/// could free a stack still in active use — the bug the OT-49 AAG confirmed.)
pub fn exit() -> ! {
    without_interrupts(|| {
        // rotate_locked() falls back to this CPU's idle thread when the ready
        // queue is empty (OT-49), so a dying thread always has somewhere to
        // switch to. The only way it returns None here is the WhichQueue::Zombie
        // idle-missing panic firing first, so this .expect() is unreachable in
        // practice — kept as a fail-loud backstop.
        //
        // OT-31: the zombie is published to `zombies` under the scheduler lock
        // (rotate_compute) held here, and that lock is kept held ACROSS the switch
        // below and released on the NEW stack (context_switch_with_release1).
        // Because reap() must take this same lock to pop and `Thread::free` a
        // zombie, it cannot free this stack until this CPU has left it — closing
        // the cross-CPU use-after-free the OT-49 AAG confirmed. Mirrors the IPC
        // block paths' release-on-new-stack discipline.
        let sched = current_scheduler();
        let mut guard = sched.inner.lock();
        let switch_params = rotate_compute(&mut guard, ThreadState::Zombie, WhichQueue::Zombie)
            .expect(
                "sched::exit: rotate returned None for a dying thread (idle-thread fallback broken)",
            );

        // OT-31 (ISC-2) instrument — neo-test only, absent in production builds.
        // When armed it widens the exit->switch window so the concurrent-reap
        // scenario can observe whether reap can free the zombie before this CPU
        // leaves its stack, and (for the instrument's negative control) can
        // reproduce the pre-OT-31 release-before-switch ordering.
        #[cfg(feature = "neo-test-ot31-exit-reap-race")]
        if ot31_instrument_armed() {
            let cpu = per_cpu::current_cpu_id() as usize;
            if ot31_use_prefix_ordering() {
                // NEGATIVE CONTROL: reproduce the pre-OT-31 release-before-switch
                // ordering. Open the window BEFORE releasing the lock (so the
                // zombie is never visible to reap with the window still closed —
                // which would free it, a real UAF, and miss the detection), then
                // release the lock, exposing the zombie. Keep the window open
                // THROUGH the switch: any reap that pops the zombie always
                // quarantines it (never frees), so the register/xsave writes the
                // switch performs land in live memory. The scenario's per-round
                // reset clears the window before the fixed round.
                ot31_open_window(cpu);
                drop(guard);
                ot31_widen();
                record_context_switch();
                // SAFETY: pre-OT-31 ordering under test. prev_rsp_ptr/prev_fpu
                // point into the zombie Box, kept alive by the reap instrument's
                // quarantine for the duration of this switch.
                unsafe { switch_context(switch_params) };
                unreachable!("sched::exit: OT-31 negative-control switch returned");
            }
            // FIXED ordering WITH widening: hold the lock across the widen so a
            // concurrent reap is serialized behind it, then close the window
            // before the release-on-new-stack switch. reap cannot pop the zombie
            // until the lock releases on the new stack (this CPU already off its
            // stack), so it never observes the window open.
            ot31_open_window(cpu);
            ot31_widen();
            ot31_close_window(cpu);
        }

        let sched_lock_ptr = guard.release_token_for_context_switch();
        core::mem::forget(guard);
        record_context_switch();
        // SAFETY: all fallible bookkeeping is done; the Zombie thread is no
        // longer in `ready` or `current`. prev_rsp_ptr/prev_fpu point into the
        // zombie Box, which lives (heap-pinned) in this CPU's zombie ring. The
        // scheduler lock byte is released on the NEW stack after `mov rsp`, so no
        // other CPU can pop+free this Box until this CPU is off its stack.
        unsafe { switch_context_with_release1(switch_params, sched_lock_ptr) };
    });
    unreachable!("sched::exit: returned from context_switch — scheduler invariant broken")
}

/// Per-CPU idle thread entry (OT-49). Runs only when a CPU has nothing else to
/// run — the last runnable thread parked on I/O and the ready queue drained. It
/// is the always-available switch target the four park sites (`exit` and the
/// three `block_current_*`) fall back to instead of panicking. It halts with
/// interrupts enabled until an interrupt — an OT-43 I/O completion, a timer
/// tick, or a reschedule IPI — makes a thread runnable, then hands the CPU to
/// it. It never blocks, never exits, and (I-14a) takes no AI-governance lock:
/// its only lock is the per-CPU scheduler lock, held briefly inside `rotate`.
///
/// **ISC-4 coupling (AAG B2) — CP-19 guard is now carried here.** This idle
/// thread is reachable only on a CPU whose current thread *parks*. Today that is
/// the BSP only: APs run `run_ap_bootstrap`'s busy-yield loop, which never parks,
/// so an AP's idle thread is normally never selected. But per CP-19
/// (`docs/concurrency-pitfalls.md`) the OT-34 fix carries the matmul work-slot
/// poll (`poll_ap_matmul_work`) into this loop too, so the seam stays closed if a
/// future build ever routes an AP through it (an AP-efficiency pass, OT-47's
/// 8-core lane, or unifying the two idle mechanisms): a parked AP that wakes on a
/// tick still drains its work slot instead of sleeping through the assignment.
/// The poll runs at the loop TOP with IF=1, *before* the IF-off ready-queue
/// check, so the CP-22/CP-28 `sti; hlt` check-then-halt sequence below is
/// completely unchanged. Correctness never depends on this hook firing — the
/// BSP's steal-back (CP-20 guard) always completes the forward even if a parked
/// AP never wakes in time — so this is defense-in-depth, not the primary path.
extern "C" fn idle_thread_entry() -> ! {
    loop {
        // OT-34 (CP-19 guard): drain any published matmul work slot for this CPU
        // before parking. IF=1 here (loop top) — a work partition is pure compute,
        // exactly as in `run_ap_bootstrap`. No-op on the BSP and when no work is
        // pending; feature-off removes it entirely. Kept ABOVE the IF-off check so
        // the check-then-halt (`sti; hlt`) discipline is untouched.
        poll_ap_matmul_work(per_cpu::current_cpu_id());
        // OT-49 (AAG B1): record that the idle thread ran, so a test scenario can
        // prove the park→idle path was exercised. Relaxed, no lock, no alloc —
        // keeps the idle path I-14(a)-clean.
        #[cfg(feature = "neo-test")]
        IDLE_ENTERED[per_cpu::current_cpu_id() as usize].fetch_add(1, Ordering::Relaxed);
        // Check the ready queue with interrupts DISABLED so a wake cannot be
        // lost between the check and the halt. If a thread became runnable
        // before this point we observe it here and switch to it; if one becomes
        // runnable after, its IRQ is still pending (IF=0) and the `sti; hlt`
        // below is guaranteed to catch it.
        crate::arch::x86_64::disable_interrupts();
        // rotate() recognises the idle thread as `prev` and returns it to the
        // per-CPU idle slot (never the ready ring); it returns None when the
        // ready queue is empty (dest == Ready), which is our cue to halt.
        match rotate(ThreadState::Ready, WhichQueue::Ready) {
            Some(switch_params) => {
                record_context_switch();
                // SAFETY: same invariants as yield_now. rotate() dropped the
                // scheduler lock before returning and stashed the idle thread's
                // Box in the idle slot, so its saved RSP location stays
                // heap-stable across this switch.
                unsafe { switch_context(switch_params) };
                // Resumed here — as the idle thread — when a park site (or a
                // preempting tick) later switches back to us. Loop and re-arm.
            }
            None => {
                // Hang discriminator: count the halts per CPU. A CPU that keeps
                // halting while a waiter it owns stays parked says the wake
                // never arrived; a CPU that never halts is busy, which is the
                // CP-21 stranding precondition.
                #[cfg(feature = "neo-test")]
                wake_trace::note_idle_halt();
                // Nothing runnable. Enable interrupts and halt atomically; the
                // next wake IRQ resumes us right after the `hlt`.
                crate::arch::x86_64::enable_interrupts_and_halt();
            }
        }
    }
}

/// The next thread to switch to when the current thread is *leaving* the CPU
/// (any `block_current_*` park): the front of the ready queue, or — when the
/// ready queue is empty — this CPU's always-available idle thread (OT-49). A
/// park site, unlike a cooperative `yield_now`, has no runnable `current` to
/// fall back on, so it must always have something to switch to; the idle thread
/// is that something. This replaces the pre-OT-49 `.expect("ready queue empty")`
/// panic that started firing once OT-43 retired the virtio-blk polling worker
/// (the accidental always-ready fallback).
///
/// A `None` idle slot means the idle thread is *itself* current — but the idle
/// thread never calls a park site — so it is a genuine invariant violation and
/// still panics, with a message distinct from the (now-handled) empty-queue
/// case.
fn next_runnable_or_idle(s: &mut PerCpuSchedulerInner, context: &'static str) -> Box<Thread> {
    if let Some(next) = s.ready.pop_front() {
        return next;
    }
    s.idle_thread.take().unwrap_or_else(|| {
        panic!(
            "{}: ready queue empty and idle thread already current — scheduler invariant violated",
            context
        )
    })
}

/// Read the calling thread's pid. Used by the IPC syscalls to look up the
/// caller's `Arc<Process>` (and therefore `Arc<Pmap>`) for the rendezvous-
/// time translate. Cheap — one local scheduler lock + field read.
pub fn current_pid() -> ProcessId {
    without_interrupts(|| current_scheduler().inner.lock().current.pid)
}

/// OT-61 C2.2: the current thread belongs to a process with a memory budget
/// (its account or one above it has a limit). Kernel threads never do. Read
/// through the thread's own Pmap, so no process-registry lookup.
#[cfg(feature = "mem-quota")]
pub fn current_mem_budgeted() -> bool {
    without_interrupts(|| {
        current_scheduler()
            .inner
            .lock()
            .current
            .pmap
            .as_ref()
            .is_some_and(|p| p.mem_budgeted())
    })
}

/// Park the current thread inside a caller-managed structure.
///
/// Atomically:
///   1. Pop the next ready thread; install as new current.
///   2. Mark prev `Blocked` and hand its `Box<Thread>` to `installer`.
///      The closure is expected to move the Box into a holder it owns
///      (typically an IPC `EndpointState` variant). The closure runs
///      while interrupts are disabled and the scheduler lock is held —
///      keep it tight.
///   3. Drop the scheduler lock, switch stacks via `context_switch`.
///   4. (Later, when the holder calls `wake_thread`) resume here on the
///      newly-Ready thread; read & return its `blocked_return` slot,
///      which the waker wrote prior to push_ready.
///
/// **Lock ordering caveat — SMP retrofit point.** This primitive is
/// the *block* path; its installer typically re-acquires the holder's
/// own lock (e.g., `Channel.state`) while the scheduler lock is still held.
/// The matching *wake* path (`wake_thread` called from inside an IPC
/// syscall) takes the holder's lock first, then the scheduler lock. That is a
/// classic lock-order inversion and would deadlock on SMP once cross-CPU IPC
/// is enabled.
///
/// Phase 1.5b is single-CPU and every IPC syscall wraps its critical
/// section in `without_interrupts`, so no concurrent acquirer can
/// observe the inverted order. Phase 2 sub-commit 3a must restructure to
/// either (a) acquire both locks in a fixed order with a per-channel-then-
/// SCHED rule and a parking-without-SCHED-held primitive, or (b) adopt
/// a release-on-the-new-stack pattern (ManuallyDrop + asm release) so
/// the scheduler-held window is closed before any other CPU can reach it.
///
/// Per ADR-004, this is the primitive every future blocking syscall
/// (futex, file I/O, inference deadlines, signal wait) consumes.
#[allow(dead_code)]
pub fn block_current_into<F>(installer: F) -> i64
where
    F: FnOnce(Box<Thread>),
{
    without_interrupts(|| {
        let switch_params = {
            let sched = current_scheduler();
            let mut s = sched.inner.lock();
            let next = next_runnable_or_idle(&mut s, "block_current_into");
            record_scheduling_decision();
            let mut prev = mem::replace(&mut s.current, next);
            prev.state = ThreadState::Blocked;
            prev.preempt_defer_ticks = 0;
            s.current.state = ThreadState::Running;
            per_cpu::set_current_thread_id(s.current.id.0);
            #[cfg(feature = "browser-runtime")]
            per_cpu::set_current_pid(u64::from(s.current.pid.0));
            crate::observability::record_dispatch(s.ready.len());
            let next_rsp = s.current.rsp;
            #[cfg(feature = "xsave-fpu")]
            let next_fpu = s.current.fpu_state_ptr();

            // Cross-process / cross-thread bookkeeping (same as rotate()).
            if prev.pid != s.current.pid {
                cross_process_prep(&s.current);
            } else if s.current.pid != KERNEL_PID {
                same_process_user_prep(&s.current);
            }

            // Capture rsp slot pointer BEFORE handing prev to installer —
            // `prev` is moved into the installer's holder, and the Box
            // payload's heap address is stable, so the pointer survives.
            let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
            #[cfg(feature = "xsave-fpu")]
            let prev_fpu = prev.fpu_state_ptr();
            installer(prev);
            SwitchParams {
                prev_rsp_ptr,
                next_rsp,
                #[cfg(feature = "xsave-fpu")]
                prev_fpu,
                #[cfg(feature = "xsave-fpu")]
                next_fpu,
            }
        };
        // Scheduler lock released by the block above. Now switch.
        // SAFETY: same invariants as yield_now; prev's Box payload is
        // heap-pinned wherever installer placed it.
        record_context_switch();
        unsafe { switch_context(switch_params) };

        // Resumed here. Our blocked_return slot was written by the waker.
        current_scheduler().inner.lock().current.blocked_return
    })
}

#[allow(dead_code)]
pub fn block_current_into_with_guard<T, G, F>(mut channel_guard: G, installer: F) -> i64
where
    G: DerefMut<Target = T> + ContextSwitchRelease,
    F: FnOnce(&mut T, Box<Thread>),
{
    without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        // OT-93: a thread a force-kill left to its boundary does not start a
        // user wait (IPC, process wait); nothing would wake it. The installer
        // is dropped unrun, and the syscall unwinds to its return, which exits.
        if current_must_not_park(&s) {
            drop(s);
            drop(channel_guard);
            return crate::errno::ESRCH;
        }
        let next = next_runnable_or_idle(&mut s, "block_current_into_with_guard");
        record_scheduling_decision();
        let mut prev = mem::replace(&mut s.current, next);
        prev.state = ThreadState::Blocked;
        prev.preempt_defer_ticks = 0;
        s.current.state = ThreadState::Running;
        per_cpu::set_current_thread_id(s.current.id.0);
        #[cfg(feature = "browser-runtime")]
        per_cpu::set_current_pid(u64::from(s.current.pid.0));
        crate::observability::record_dispatch(s.ready.len());
        let next_rsp = s.current.rsp;
        #[cfg(feature = "xsave-fpu")]
        let next_fpu = s.current.fpu_state_ptr();

        if prev.pid != s.current.pid {
            cross_process_prep(&s.current);
        } else if s.current.pid != KERNEL_PID {
            same_process_user_prep(&s.current);
        }

        let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
        #[cfg(feature = "xsave-fpu")]
        let prev_fpu = prev.fpu_state_ptr();
        installer(&mut *channel_guard, prev);
        let switch_params = SwitchParams {
            prev_rsp_ptr,
            next_rsp,
            #[cfg(feature = "xsave-fpu")]
            prev_fpu,
            #[cfg(feature = "xsave-fpu")]
            next_fpu,
        };

        let channel_lock_ptr = channel_guard.release_token_for_context_switch();
        let sched_lock_ptr = s.release_token_for_context_switch();
        core::mem::forget(channel_guard);
        core::mem::forget(s);

        // SAFETY: all fallible and panic-capable bookkeeping completed before
        // either guard was forgotten. Both pointers are lock bytes from
        // context-switch-release guards held by this CPU. The asm switches to
        // `next_rsp` and releases the caller guard first, then scheduler,
        // before resuming.
        record_context_switch();
        unsafe { switch_context_with_release2(switch_params, channel_lock_ptr, sched_lock_ptr) };

        current_scheduler().inner.lock().current.blocked_return
    })
}

#[allow(dead_code)]
pub fn block_current_with_token_and_guard<T, G, F>(mut caller_guard: G, install: F) -> i64
where
    G: DerefMut<Target = T> + ContextSwitchRelease,
    F: FnOnce(&mut T, ThreadWakeToken),
{
    without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        let slot_idx = s
            .free_token_slot()
            .expect("block_current_with_token_and_guard: token parked table full");
        let next = next_runnable_or_idle(&mut s, "block_current_with_token_and_guard");
        record_scheduling_decision();
        let mut prev = mem::replace(&mut s.current, next);
        prev.state = ThreadState::Blocked;
        prev.preempt_defer_ticks = 0;
        s.current.state = ThreadState::Running;
        per_cpu::set_current_thread_id(s.current.id.0);
        #[cfg(feature = "browser-runtime")]
        per_cpu::set_current_pid(u64::from(s.current.pid.0));
        crate::observability::record_dispatch(s.ready.len());
        let next_rsp = s.current.rsp;
        #[cfg(feature = "xsave-fpu")]
        let next_fpu = s.current.fpu_state_ptr();

        if prev.pid != s.current.pid {
            cross_process_prep(&s.current);
        } else if s.current.pid != KERNEL_PID {
            same_process_user_prep(&s.current);
        }

        let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
        #[cfg(feature = "xsave-fpu")]
        let prev_fpu = prev.fpu_state_ptr();
        let token = s.park_thread_token(sched.cpu_id, slot_idx, prev);
        install(&mut *caller_guard, token);
        let switch_params = SwitchParams {
            prev_rsp_ptr,
            next_rsp,
            #[cfg(feature = "xsave-fpu")]
            prev_fpu,
            #[cfg(feature = "xsave-fpu")]
            next_fpu,
        };

        let caller_lock_ptr = caller_guard.release_token_for_context_switch();
        let sched_lock_ptr = s.release_token_for_context_switch();
        core::mem::forget(caller_guard);
        core::mem::forget(s);

        // SAFETY: the previous thread's Box has been moved into the
        // scheduler-owned token table, so the saved RSP pointer remains
        // stable. Both locks are released by the switch shim on the new stack.
        record_context_switch();
        unsafe { switch_context_with_release2(switch_params, caller_lock_ptr, sched_lock_ptr) };

        current_scheduler().inner.lock().current.blocked_return
    })
}

/// OT-61 C1.1: fallible variant of [`block_current_with_token_and_guard`] for
/// USER-TRIGGERABLE blocking paths (the futex, thread join). The infallible
/// primitive `.expect()`s a free token slot — acceptable for bounded kernel
/// daemons, but a user process that parks enough threads must get `-EAGAIN`
/// back, never a kernel panic (the kernel is panic=abort; a user-reachable
/// panic is a full-system DoS — the C1.0 AAG lesson). On `Err` the caller's
/// guard is returned untouched (nothing was installed, nothing switched).
#[cfg(any(feature = "browser-runtime", feature = "wait-multiplex"))]
pub fn try_block_current_with_token_and_guard<T, G, F>(
    mut caller_guard: G,
    install: F,
) -> Result<i64, G>
where
    G: DerefMut<Target = T> + ContextSwitchRelease,
    F: FnOnce(&mut T, ThreadWakeToken),
{
    without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        // OT-93: as in `block_current_into_with_guard` (a futex or wait-any
        // park after the kill's purge would never be woken).
        if current_must_not_park(&s) {
            drop(s);
            drop(caller_guard);
            return Ok(crate::errno::ESRCH);
        }
        let Some(slot_idx) = s.free_token_slot() else {
            drop(s);
            return Err(caller_guard);
        };
        let next = next_runnable_or_idle(&mut s, "try_block_current_with_token_and_guard");
        record_scheduling_decision();
        let mut prev = mem::replace(&mut s.current, next);
        prev.state = ThreadState::Blocked;
        prev.preempt_defer_ticks = 0;
        s.current.state = ThreadState::Running;
        per_cpu::set_current_thread_id(s.current.id.0);
        #[cfg(feature = "browser-runtime")]
        per_cpu::set_current_pid(u64::from(s.current.pid.0));
        crate::observability::record_dispatch(s.ready.len());
        let next_rsp = s.current.rsp;
        #[cfg(feature = "xsave-fpu")]
        let next_fpu = s.current.fpu_state_ptr();

        if prev.pid != s.current.pid {
            cross_process_prep(&s.current);
        } else if s.current.pid != KERNEL_PID {
            same_process_user_prep(&s.current);
        }

        let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
        #[cfg(feature = "xsave-fpu")]
        let prev_fpu = prev.fpu_state_ptr();
        let token = s.park_thread_token(sched.cpu_id, slot_idx, prev);
        install(&mut *caller_guard, token);
        let switch_params = SwitchParams {
            prev_rsp_ptr,
            next_rsp,
            #[cfg(feature = "xsave-fpu")]
            prev_fpu,
            #[cfg(feature = "xsave-fpu")]
            next_fpu,
        };

        let caller_lock_ptr = caller_guard.release_token_for_context_switch();
        let sched_lock_ptr = s.release_token_for_context_switch();
        core::mem::forget(caller_guard);
        core::mem::forget(s);

        // SAFETY: identical contract to `block_current_with_token_and_guard`
        // — the parked Box lives in the scheduler-owned token table, both
        // locks are released by the switch shim on the new stack.
        record_context_switch();
        unsafe { switch_context_with_release2(switch_params, caller_lock_ptr, sched_lock_ptr) };

        Ok(current_scheduler().inner.lock().current.blocked_return)
    })
}

/// Phase 1.5b: re-enqueue a previously-blocked thread with a return value.
///
/// The matching peer (or the process-exit cleanup path) calls this after
/// extracting the parked Box from its holder. Sets the thread's state to
/// `Ready`, writes `return_value` into its `blocked_return` slot (which
/// the resumed thread will read once `block_current_into` returns), and
/// pushes onto the back of the current CPU's ready queue.
///
/// Self-masking (OT-74): the scheduler lock is taken by the timer ISR, so it
/// must never be held with IF=1. IPC syscalls already call this with IF=0, but
/// `complete_process_waiters` reaches it from `reap()` on preemptible
/// (IF=1) paths — a tick landing while this held the lock spun the ISR on it
/// forever (`arthur_reversibility`'s post-`exit` hang). `without_interrupts`
/// is a no-op restore for IF=0 callers.
pub fn wake_thread(mut thread: Box<Thread>, return_value: i64) {
    prepare_woken_thread(&mut thread, return_value);
    without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        push_ready_locked_or_panic(sched, &mut s, thread, ReadyPlacement::Back, "wake_thread");
    });
}

/// Same as `wake_thread`, but the resumed thread is placed at the front of
/// the ready queue. IPC rendezvous completion uses this with an immediate
/// yield to implement the documented same-CPU handoff invariant: the blocked
/// peer resumes before the completing syscall returns to userspace. The
/// bounded defer credit lets the peer reach its next syscall before a stale
/// timer tick makes it observable as Ready to the original producer.
#[cfg(not(feature = "browser-runtime"))]
pub fn wake_thread_front(mut thread: Box<Thread>, return_value: i64) {
    prepare_woken_thread(&mut thread, return_value);
    thread.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;
    // Self-masking for the same reason as `wake_thread` (OT-74).
    without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        push_ready_locked_or_panic(
            sched,
            &mut s,
            thread,
            ReadyPlacement::Front,
            "wake_thread_front",
        );
    });
}

/// OT-61 C1.1 (cross-model AAG finding 1): the `browser-runtime` variant, which
/// **transfers the ready-ring reservation** when this wake migrates a thread.
///
/// This path is the IPC rendezvous handoff, and it deliberately enqueues the
/// parked peer on the **completer's** CPU — threads are not pinned to their
/// creator. Before this fix it pushed straight onto the current CPU with no
/// accounting, so a thread admitted (and counted) on CPU A could be woken onto
/// CPU B: A kept a phantom reservation forever while B gained a ready thread it
/// had never reserved. That breaks `ready.len() <= live_count` on B — which is
/// the exact invariant blocker A's fix rests on — so the "impossible" full-ring
/// wake panic was still reachable under true SMP. A Claude-only panel missed
/// this; a Codex cross-check (Janus) built the interleaving.
///
/// The accounting is deliberately split across two lock scopes rather than
/// holding both per-CPU scheduler locks at once. Two CPUs migrating toward each
/// other would give a classic ABBA deadlock, and the transient state a split
/// leaves is an **over-count on the source**, which is conservative: it can only
/// make the source admit less, never break `ready.len() <= live_count`. Trading
/// a deadlock seam for a bounded conservative skew is the right direction.
#[cfg(feature = "browser-runtime")]
pub fn wake_thread_front(mut thread: Box<Thread>, return_value: i64) {
    prepare_woken_thread(&mut thread, return_value);
    thread.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;

    let src_cpu = thread.cpu_id.load(Ordering::Acquire);
    let mut wake_on_src = false;

    without_interrupts(|| {
        let dst = current_scheduler();

        // Same CPU: the reservation is already here, nothing to transfer.
        if src_cpu == dst.cpu_id {
            let mut s = dst.inner.lock();
            push_ready_locked_or_panic(
                dst,
                &mut s,
                thread,
                ReadyPlacement::Front,
                "wake_thread_front",
            );
            return;
        }

        // Destination capacity check FIRST. `live_count < CAP` plus the
        // invariant gives `ready.len() <= live_count < CAP`, so the push below
        // provably fits and the panicking enqueue stays unreachable.
        let mut d = dst.inner.lock();
        if d.live_count < SCHEDULER_RING_CAPACITY {
            d.live_count += 1;
            push_ready_locked_or_panic(
                dst,
                &mut d,
                thread,
                ReadyPlacement::Front,
                "wake_thread_front",
            );
            drop(d);

            // Release the source's now-transferred reservation in its OWN lock
            // scope (see the ABBA note above).
            if let Some(src) = scheduler_for_cpu(src_cpu) {
                let mut s = src.inner.lock();
                s.live_count = s.live_count.saturating_sub(1);
            }
            return;
        }
        drop(d);

        // Destination saturated — do NOT force it over capacity. Wake on the
        // SOURCE, which still holds this thread's reservation, so
        // `src.ready.len() <= src.live_count - 1 < CAP` guarantees room.
        match scheduler_for_cpu(src_cpu) {
            Some(src) => {
                let mut s = src.inner.lock();
                push_ready_locked_or_panic(
                    src,
                    &mut s,
                    thread,
                    ReadyPlacement::Front,
                    "wake_thread_front(src-fallback)",
                );
                wake_on_src = true;
            }
            None => {
                // Source CPU never came up: admit fresh here so the thread is
                // never dropped. Admission accounts it on this CPU.
                let mut d2 = dst.inner.lock();
                if let Err(t) = admit_ready_locked(dst, &mut d2, thread) {
                    let _ = Box::into_raw(t);
                    panic!("wake_thread_front: no CPU can accept a migrating wake");
                }
            }
        }
    });

    if wake_on_src {
        crate::arch::x86_64::apic::send_reschedule_ipi(src_cpu);
    }
}

/// Q4 rendezvous wake helper: enqueue the parked peer on the completer's
/// current CPU and give it front-of-queue handoff priority.
pub fn wake_thread_on_current_cpu(thread: Box<Thread>, return_value: i64) {
    wake_thread_front(thread, return_value);
}

pub fn wake_thread_token(token: ThreadWakeToken, return_value: i64) -> WakeTokenResult {
    without_interrupts(|| {
        let Some(sched) = scheduler_for_cpu(token.cpu_id) else {
            return WakeTokenResult::Gone;
        };
        let result = {
            let mut s = sched.inner.lock();
            match s.take_thread_by_token(token) {
                TokenTakeResult::Thread(mut thread) => {
                    prepare_woken_thread(&mut thread, return_value);
                    push_ready_locked_or_panic(
                        sched,
                        &mut s,
                        thread,
                        ReadyPlacement::Back,
                        "wake_thread_token",
                    );
                    WakeTokenResult::Woken
                }
                TokenTakeResult::Gone => WakeTokenResult::Gone,
                TokenTakeResult::Stale => WakeTokenResult::Stale,
            }
        };

        // OT-61 C1.1 hang discriminator: attribute the OUTCOME of every token
        // wake to the token's CPU. `Woken` vs `Stale`/`Gone` is the difference
        // between "the waiter is now Ready on that CPU" and "the wake found
        // nothing" — indistinguishable from userland, opposite bugs.
        #[cfg(feature = "neo-test")]
        {
            let (k, note): (u64, fn(u32)) = match result {
                WakeTokenResult::Woken => (
                    wake_trace::kind::WOKEN,
                    wake_trace::note_token_woken as fn(u32),
                ),
                WakeTokenResult::Stale => (
                    wake_trace::kind::STALE,
                    wake_trace::note_token_stale as fn(u32),
                ),
                WakeTokenResult::Gone => (
                    wake_trace::kind::GONE,
                    wake_trace::note_token_gone as fn(u32),
                ),
            };
            note(token.cpu_id);
            wake_trace::note_event(k, token.cpu_id, token.slot, 0, 0);
        }

        if result == WakeTokenResult::Woken && token.cpu_id != per_cpu::current_cpu_id() {
            crate::arch::x86_64::apic::send_reschedule_ipi(token.cpu_id);
        }

        result
    })
}

/// Force-retire a parked thread without waking it.
///
/// Blocked IPC threads are owned by their endpoint state rather than a
/// scheduler queue. Forced process teardown extracts the parked `Box<Thread>`
/// and hands it here so the normal zombie reaper can reclaim the thread.
pub fn zombify_thread(mut thread: Box<Thread>) {
    without_interrupts(|| {
        debug_assert!(
            thread.state == ThreadState::Blocked,
            "zombify_thread: thread state must be Blocked (was {:?})",
            thread.state
        );
        thread.state = ThreadState::Zombie;
        thread.preempt_defer_ticks = 0;
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        stamp_thread_cpu(&thread, sched.cpu_id);
        push_zombie_locked(sched, &mut s, thread, "zombify_thread");
    });
}

/// OT-98: whether a thread of `pid` waits in a zombie ring, that is, it exited
/// and the BSP has not reaped it yet. A thread moves to its CPU's ring under
/// that CPU's scheduler lock, held through the switch away from it, so a scan
/// that takes each lock sees it as that CPU's current thread or in the ring.
pub fn zombie_pending(pid: ProcessId) -> bool {
    without_interrupts(|| {
        (0..per_cpu::MAX_CPUS as u32).any(|cpu_id| {
            scheduler_for_cpu(cpu_id).is_some_and(|sched| {
                let s = sched.inner.lock();
                (0..s.zombies.len()).any(|offset| {
                    s.zombies.slots[s.zombies.slot_index(offset)]
                        .as_ref()
                        .is_some_and(|thread| thread.pid == pid)
                })
            })
        })
    })
}

pub fn zombify_parked_thread_by_pid(pid: ProcessId) -> bool {
    without_interrupts(|| {
        for cpu_id in 0..per_cpu::MAX_CPUS as u32 {
            let Some(sched) = scheduler_for_cpu(cpu_id) else {
                continue;
            };
            let mut s = sched.inner.lock();
            let Some(mut thread) = s.take_token_thread_by_pid(pid) else {
                continue;
            };
            debug_assert!(
                thread.state == ThreadState::Blocked,
                "zombify_parked_thread_by_pid: thread state must be Blocked (was {:?})",
                thread.state
            );
            thread.state = ThreadState::Zombie;
            thread.preempt_defer_ticks = 0;
            stamp_thread_cpu(&thread, sched.cpu_id);
            push_zombie_locked(sched, &mut s, thread, "zombify_parked_thread_by_pid");
            return true;
        }
        false
    })
}

fn prepare_woken_thread(thread: &mut Thread, return_value: i64) {
    debug_assert!(
        thread.state == ThreadState::Blocked,
        "wake_thread: thread state must be Blocked (was {:?})",
        thread.state
    );
    thread.state = ThreadState::Ready;
    thread.blocked_return = return_value;
    thread.preempt_defer_ticks = 0;
}

fn stamp_thread_cpu(thread: &Thread, cpu_id: u32) {
    thread.cpu_id.store(cpu_id, Ordering::Relaxed);
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn place_thread_on_cpu(thread: Box<Thread>, target_cpu: u32) -> Result<(), &'static str> {
    let result = without_interrupts(|| {
        let idx = target_cpu as usize;
        if idx >= per_cpu::MAX_CPUS {
            return Err("target_cpu_exceeds_max");
        }

        let ptr = PER_CPU[idx]
            .scheduler_ptr
            .load(Ordering::Acquire)
            .cast::<PerCpuScheduler>();
        if ptr.is_null() {
            return Err("target_scheduler_not_initialized");
        }

        // SAFETY: non-null scheduler_ptr values are installed by
        // init_cpu_scheduler via Box::into_raw and live for kernel lifetime.
        let sched = unsafe { &*ptr };
        let mut s = sched.inner.lock();
        if s.ready.is_full() {
            return Err("ready_ring_full");
        }
        // CP-35: this is an ADMISSION onto `target_cpu` — a thread enters that
        // CPU's live population here — so it must take a reservation, exactly
        // like `try_push_ready`. Missing it left `ready.len() > live_count` on
        // the target (caught by the migration battery's own assertion at seq=1,
        // `ready=1 live=0`). The rule CP-35 states is "enumerate EVERY path that
        // puts a thread on a CPU"; this was the fourth such path.
        #[cfg(feature = "browser-runtime")]
        if s.live_count >= SCHEDULER_RING_CAPACITY {
            return Err("cpu_live_capacity_full");
        }
        stamp_thread_cpu(&thread, target_cpu);
        s.ready
            .push_back(thread)
            .map_err(|_| "ready_ring_full_after_room_check")?;
        #[cfg(feature = "browser-runtime")]
        {
            s.live_count += 1;
        }
        Ok(())
    });

    if result.is_ok() {
        crate::arch::x86_64::apic::send_reschedule_ipi(target_cpu);
    }

    result
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn cpu_scheduler_ready(cpu_id: u32) -> bool {
    let idx = cpu_id as usize;
    if idx >= per_cpu::MAX_CPUS {
        return false;
    }
    !PER_CPU[idx].scheduler_ptr.load(Ordering::Acquire).is_null()
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
/// OT-61 C1.1 (cross-model AAG finding 1): assert the ready-ring reservation
/// invariant on EVERY initialized CPU — `ready.len() <= live_count <= CAP`.
///
/// This is the property blocker A's no-panic guarantee rests on, and before the
/// migration fix it was silently false on any CPU that received an IPC handoff
/// from another CPU: the destination gained a ready thread it never reserved
/// (`ready.len() > live_count`) while the source kept a phantom reservation.
/// The pre-existing cross-CPU batteries drove that migration but never asserted
/// this, which is exactly why a green SMP-4 gate coexisted with a real bug.
///
/// Returns `(ok, offending_cpu, ready_len, live_count)` so a failing scenario
/// can report which CPU drifted rather than just "something is wrong".
#[cfg(feature = "browser-runtime")]
pub fn debug_live_count_invariant() -> (bool, u32, usize, usize) {
    without_interrupts(|| {
        for pc in PER_CPU.iter() {
            let ptr = pc
                .scheduler_ptr
                .load(Ordering::Acquire)
                .cast::<PerCpuScheduler>();
            if ptr.is_null() {
                continue;
            }
            let sched = unsafe { &*ptr };
            let s = sched.inner.lock();
            let ready_len = s.ready.len();
            let live = s.live_count;
            if ready_len > live || live > SCHEDULER_RING_CAPACITY {
                return (false, sched.cpu_id, ready_len, live);
            }
        }
        (true, 0, 0, 0)
    })
}

pub fn debug_total_zombies() -> usize {
    let mut total = 0;
    for pc in PER_CPU.iter() {
        let ptr = pc
            .scheduler_ptr
            .load(Ordering::Acquire)
            .cast::<PerCpuScheduler>();
        if ptr.is_null() {
            continue;
        }
        let sched = unsafe { &*ptr };
        total += sched.zombie_count.load(Ordering::Acquire);
    }
    total
}

/// L1a.1 step 2a: is `token`'s thread still parked on it? A pipe's full
/// waiter list drops the tokens of threads killed while parked. Callers may
/// hold a lock that orders before the scheduler's (the pipe's).
#[cfg(feature = "pipe")]
pub fn token_still_parked(token: ThreadWakeToken) -> bool {
    without_interrupts(|| {
        let Some(sched) = scheduler_for_cpu(token.cpu_id) else {
            return false;
        };
        sched.inner.lock().token_slot_occupied(token)
    })
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_token_occupied(token: ThreadWakeToken) -> bool {
    without_interrupts(|| {
        let Some(sched) = scheduler_for_cpu(token.cpu_id) else {
            return false;
        };
        sched.inner.lock().token_slot_occupied(token)
    })
}

// ============================================================================
// OT-31 (ISC-2) exit()/reap race instrument — neo-test only.
//
// A falsifiable instrument for the cross-CPU exit()-vs-reap() use-after-free.
// The `neo-test-ot31-exit-reap-race` scenario arms it, places a victim thread
// on an AP that calls exit(), and loops reap() on the BSP. `exit()` widens the
// exit->switch window; reap(), when it pops a zombie, checks whether that
// zombie's exiting CPU is still inside its window:
//   * PREFIX ordering (negative control): exit releases the scheduler lock
//     BEFORE the switch (the pre-OT-31 ordering), so reap CAN pop mid-window ->
//     OBSERVED_OPEN set + the Box is quarantined (kept alive, never freed) so
//     the in-flight switch writes do not hit freed memory (the race is observed
//     without a real UAF).
//   * FIXED ordering (the OT-31 fix): exit holds the lock across the switch, so
//     reap blocks on the lock and only pops after the window closed -> never
//     OBSERVED_OPEN. This is the empirical red(prefix)->green(fixed) proof.
//
// Every item here is compiled ONLY under the scenario feature; production
// exit()/reap() have exactly one ordering and no window/quarantine state.
// ============================================================================

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
static OT31_ARMED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
static OT31_USE_PREFIX: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
static OT31_WINDOW_OPEN: [AtomicBool; per_cpu::MAX_CPUS] =
    [const { AtomicBool::new(false) }; per_cpu::MAX_CPUS];
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
static OT31_REAP_OBSERVED_OPEN: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
static OT31_REAP_EXAMINED: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
static OT31_QUARANTINE: SwitchMutex<Option<Box<Thread>>> = SwitchMutex::new(None);

/// Bounded widen budget. Long enough that the BSP is scheduled (and runs reap)
/// while an AP holds the exit window open under TCG round-robin. The prefix
/// round breaks out early via the reap handshake; only the fixed round — where
/// reap is blocked on the held lock and never sets the flag — pays the full
/// budget, once.
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
const OT31_WIDEN_BUDGET: u64 = 1_000_000;

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
#[inline(always)]
fn ot31_instrument_armed() -> bool {
    OT31_ARMED.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
#[inline(always)]
fn ot31_use_prefix_ordering() -> bool {
    OT31_USE_PREFIX.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
#[inline(always)]
fn ot31_open_window(cpu: usize) {
    if cpu < per_cpu::MAX_CPUS {
        OT31_WINDOW_OPEN[cpu].store(true, Ordering::Release);
    }
}

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
#[inline(always)]
fn ot31_close_window(cpu: usize) {
    if cpu < per_cpu::MAX_CPUS {
        OT31_WINDOW_OPEN[cpu].store(false, Ordering::Release);
    }
}

/// Hold the exit->switch window open long enough for a concurrent BSP reap to
/// run. Breaks out as soon as reap reports it observed the open window (the
/// prefix round) so the negative control is fast; the fixed round — where reap
/// is blocked on the held scheduler lock and never sets the flag — spins the
/// full bounded budget then returns.
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
fn ot31_widen() {
    for _ in 0..OT31_WIDEN_BUDGET {
        if OT31_REAP_OBSERVED_OPEN.load(Ordering::Acquire) {
            break;
        }
        core::hint::spin_loop();
    }
}

/// reap()-side detection. Counts every examined zombie; if the zombie's exiting
/// CPU is still inside its exit window, records the race and quarantines the Box
/// (returns `None` so reap skips the free). Otherwise returns the Box for a
/// normal free.
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
fn ot31_reap_examine(zombie: Box<Thread>) -> Option<Box<Thread>> {
    OT31_REAP_EXAMINED.fetch_add(1, Ordering::Relaxed);
    let cpu = zombie.cpu_id.load(Ordering::Acquire) as usize;
    if cpu < per_cpu::MAX_CPUS && OT31_WINDOW_OPEN[cpu].load(Ordering::Acquire) {
        OT31_REAP_OBSERVED_OPEN.store(true, Ordering::Release);
        // Quarantine — keep the Box alive so the exiting CPU's in-flight switch
        // writes land in live memory (no real UAF). One racy zombie per run; a
        // second (not expected) is leaked via into_raw rather than freed.
        let mut slot = OT31_QUARANTINE.lock();
        if slot.is_none() {
            *slot = Some(zombie);
        } else {
            let _ = Box::into_raw(zombie);
        }
        return None;
    }
    Some(zombie)
}

/// Arm the instrument for one round. `prefix == true` selects the pre-OT-31
/// release-before-switch ordering (the negative control); `false` selects the
/// OT-31 fixed ordering.
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
pub fn debug_ot31_arm(prefix: bool) {
    OT31_USE_PREFIX.store(prefix, Ordering::Release);
    OT31_ARMED.store(true, Ordering::Release);
}

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
pub fn debug_ot31_disarm() {
    OT31_ARMED.store(false, Ordering::Release);
}

/// Reset the per-round observation state (call before each round).
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
pub fn debug_ot31_reset_observation() {
    OT31_REAP_OBSERVED_OPEN.store(false, Ordering::Release);
    OT31_REAP_EXAMINED.store(0, Ordering::Release);
    for flag in OT31_WINDOW_OPEN.iter() {
        flag.store(false, Ordering::Release);
    }
}

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
pub fn debug_ot31_observed_open_window() -> bool {
    OT31_REAP_OBSERVED_OPEN.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-ot31-exit-reap-race")]
pub fn debug_ot31_reap_examined() -> u64 {
    OT31_REAP_EXAMINED.load(Ordering::Acquire)
}

/// Free the quarantined zombie (if any). Safe to call once both rounds are done
/// and the exiting CPU is long off its stack; keeps the scenario leak-clean for
/// the closing `frame_audit`.
#[cfg(feature = "neo-test-ot31-exit-reap-race")]
pub fn debug_ot31_drain_quarantine() {
    let zombie = OT31_QUARANTINE.lock().take();
    if let Some(zombie) = zombie {
        Thread::free(zombie);
    }
}

/// Yank the (single) thread belonging to `pid` out of any initialized CPU's
/// ready queue and move it to that scheduler's zombie queue. Returns `true` on
/// success, `false` if no Ready-state thread for that pid is present.
///
/// Blocked IPC threads are invisible to scheduler queues; `process::terminate`
/// handles that fallback by walking the target's drained channel handles and
/// passing the parked thread to `zombify_thread`.
///
/// Caller must ensure `pid != current_pid()` — yanking the currently-
/// running thread would be self-kill, which `sys_kill` rejects upstream.
pub fn kill_thread_by_pid(pid: ProcessId) -> bool {
    without_interrupts(|| {
        for pc in PER_CPU.iter() {
            let ptr = pc
                .scheduler_ptr
                .load(Ordering::Acquire)
                .cast::<PerCpuScheduler>();
            if ptr.is_null() {
                continue;
            }

            let sched = unsafe { &*ptr };
            let mut s = sched.inner.lock();
            // CP-65 / CP-66: an inference participant or an admission caller
            // (holder or waiter) stays queued; it cancels and exits itself at
            // the syscall boundary (`kill_protected`).
            let Some(idx) = s.ready.position_by_pid_killable(pid) else {
                continue;
            };
            let mut victim = s
                .ready
                .remove_at(idx)
                .expect("kill_thread_by_pid: position said it was there");
            victim.state = ThreadState::Zombie;
            push_zombie_locked(sched, &mut s, victim, "kill_thread_by_pid");
            return true;
        }

        false
    })
}

/// Drain all initialized CPU zombie queues from the BSP, returning each thread
/// object to THREAD_CACHE. Thread::free drops the owned kernel stack before
/// caching the Thread storage.
///
/// Q9 rollback keeps Q3's per-CPU containers but removes AP reaper
/// participation. If called on an AP, this is a no-op.
/// OT-98 (the PM's bounce, item 2): the pid of the thread the reaper has
/// popped from a zombie ring and not yet freed, or 0. The reaper is the BSP's
/// alone, so one slot. Set under the ring's scheduler lock, cleared after the
/// thread's anchor is released: a kill's scan that misses the thread in the
/// ring and then reads this, then the process's thread count, finds it.
static REAPING_PID: AtomicU32 = AtomicU32::new(0);

/// OT-98: the process whose thread the reaper is freeing right now, if any.
pub fn reaping() -> Option<ProcessId> {
    match REAPING_PID.load(Ordering::SeqCst) {
        0 => None,
        pid => Some(ProcessId(pid)),
    }
}

pub fn reap() {
    if per_cpu::current_cpu_id() != 0 {
        return;
    }

    for pc in PER_CPU.iter() {
        let ptr = pc
            .scheduler_ptr
            .load(Ordering::Acquire)
            .cast::<PerCpuScheduler>();
        if ptr.is_null() {
            continue;
        }
        let sched = unsafe { &*ptr };
        if sched.zombie_count.load(Ordering::Acquire) == 0 {
            continue;
        }

        loop {
            let zombie = without_interrupts(|| {
                let mut s = sched.inner.lock();
                let zombie = s.zombies.pop_front();
                if let Some(z) = zombie.as_ref() {
                    // OT-98: published before this lock is released (`reaping`).
                    REAPING_PID.store(z.pid.0, Ordering::SeqCst);
                    sched.zombie_count.fetch_sub(1, Ordering::Release);
                    // Blocker A: a reaped thread leaves the live population, so
                    // it releases its reserved ready slot. Every zombie was an
                    // admitted thread (exit or force-zombify from a counted
                    // state); the idle thread never becomes a zombie.
                    #[cfg(feature = "browser-runtime")]
                    {
                        s.live_count = s.live_count.saturating_sub(1);
                    }
                }
                zombie
            });
            let Some(zombie) = zombie else {
                break;
            };
            // OT-31 (ISC-2) instrument — neo-test only. If this zombie's exiting
            // CPU is still inside its exit->switch window, freeing it now would be
            // the cross-CPU UAF. Record it and quarantine the Box (keep it alive)
            // instead of freeing. Under the OT-31 fix reap can never reach here
            // with the window open (it blocks on the scheduler lock), so this
            // never fires in the fixed ordering — only in the negative control.
            #[cfg(feature = "neo-test-ot31-exit-reap-race")]
            let zombie = match ot31_reap_examine(zombie) {
                Some(z) => z,
                None => {
                    REAPING_PID.store(0, Ordering::SeqCst);
                    continue;
                }
            };
            let pid = zombie.pid;
            // OT-98 leg F: the window between the pop and the free.
            #[cfg(feature = "neo-test-kill-at-exit")]
            crate::test_harness::kae_after_reap_pop(pid);
            #[cfg(feature = "browser-runtime")]
            let tid = zombie.id.0;
            serial_println!(
                "[ kernel ] sched: reaped '{}' (id={})",
                zombie.name,
                zombie.id.0
            );
            Thread::free(zombie);
            // Its anchor is released (`Process::live_threads`, SeqCst), so
            // the mark can go.
            REAPING_PID.store(0, Ordering::SeqCst);
            // OT-93: a killed process whose last thread this was completes now
            // (the PM's #204 bounce, item 1).
            super::process::thread_reaped(pid);
            // If this thread was the last in a user process, deregister the
            // Process (which Drops its Pmap). Under `browser-runtime` the
            // per-process thread refcount decides "last" (OT-61 C1.1); the
            // default build keeps the exact Phase-1.4 one-thread-per-process
            // call (kill-switch: bit-for-bit).
            #[cfg(not(feature = "browser-runtime"))]
            super::process::on_thread_reap(pid);
            #[cfg(feature = "browser-runtime")]
            super::process::on_thread_reap_threaded(pid, tid);
            // OT-83 F1: a fault kill staged its lines without flushing; write
            // them out here, in normal context (never waits).
            crate::log::flush_pending();
        }
    }
}

/// Nested [`reap_nested`] calls in progress (on CPU 0, the only CPU that
/// reaps). While nonzero, a reap's teardown leaves `Process` drops to the
/// reaper's top level (#226 bounce): see [`in_nested_reap`].
static NESTED_REAPS: AtomicU32 = AtomicU32::new(0);

/// [`reap`] from a context that may hold other locks: the NeoFS retry paths
/// (`inode.rs`, `cache.rs`, under `entries` and the cache's `drain` guard) and
/// a kthread join. A zombie it pops still has its teardown run, but no
/// `Process` drops there (its own last `Arc`, or the deferred queue): they go
/// to the next [`reap_and_drain`]. The control
/// (`neo-test-nested-reap-control`) reaps as a plain [`reap`].
pub fn reap_nested() {
    if per_cpu::current_cpu_id() != 0 {
        return;
    }
    #[cfg(not(feature = "neo-test-nested-reap-control"))]
    NESTED_REAPS.fetch_add(1, Ordering::AcqRel);
    reap();
    #[cfg(not(feature = "neo-test-nested-reap-control"))]
    NESTED_REAPS.fetch_sub(1, Ordering::AcqRel);
}

/// `cap_graph_revoke` F1g: run `f` as if inside a [`reap_nested`].
#[cfg(feature = "neo-test-cap-graph-revoke")]
pub fn nested_reap_for_test(f: impl FnOnce()) {
    NESTED_REAPS.fetch_add(1, Ordering::AcqRel);
    f();
    NESTED_REAPS.fetch_sub(1, Ordering::AcqRel);
}

/// Whether this CPU is inside a [`reap_nested`].
pub fn in_nested_reap() -> bool {
    per_cpu::current_cpu_id() == 0 && NESTED_REAPS.load(Ordering::Acquire) != 0
}

/// The reaper's top level (the kernel main loops): [`reap`], then the
/// processes a `CAP_GRAPH` pin held last (Janus #127), dropped here on an
/// ordinary pass even when no teardown follows. The NeoFS retry paths reap
/// under their locks with [`reap_nested`] instead (Janus #182 item 2), where
/// a `Process` must not drop.
pub fn reap_and_drain() {
    reap();
    if per_cpu::current_cpu_id() == 0 {
        super::process::drop_deferred_processes_if_any();
    }
}
