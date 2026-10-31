// SPDX-License-Identifier: MPL-2.0
//! B1c (F5, PM ruling): the dedicated real-LM inference stack.
//!
//! Thread stacks stay what they are on `main` (32 KiB; 256 KiB only in the
//! real-brain test builds). Every real-LM forward instead runs on this one
//! 256 KiB stack, so the product kernel never needs a deep stack anywhere
//! else:
//!
//! - **Mapped once, at boot**, next to the kernel heap, 4 KiB at a time,
//!   global, with unmapped guards on each side (`inference_stack_logic`).
//!   Mapping it before any user pmap exists means every pmap shares it and
//!   nothing is ever mapped or unmapped at run time (no shootdown).
//! - **One caller at a time.** `run` takes a yielding lock: a second
//!   generate waits (`yield_now`, with a short interrupt window each round so
//!   a waiting CPU still takes ticks and IPIs) rather than sharing the stack.
//! - **The owner is interruptible and preemptible for the whole call (B1c
//!   AAG J1, CP-64).** A syscall arrives at `IF=0` (SFMASK), and a forward
//!   takes seconds. Left at `IF=0`, the owner's CPU stays shootdown-ready but
//!   cannot ACK a shootdown IPI, so any other CPU's shootdown (a process reap,
//!   an unmap) panics after 50 ms; on the BSP it also stops the timer, which
//!   is the only tick source. So the closure runs with interrupts on, and the
//!   caller's `IF` is restored before it returns. Preemption on this stack is
//!   safe: the timer enters on its IST and copies its frame to the
//!   interrupted stack, a context switch saves `rsp` per thread (and, with
//!   `inference-preemptible`'s xsave, the FPU state), and `TSS.rsp0` / the
//!   per-CPU `kernel_rsp` come from the thread's own stack and are used only
//!   on ring-3 entry, which cannot happen inside the call.
//! - **No inference participant is killed while it owns resources (B1c AAG
//!   J3/J4, CP-65).** Every thread in an inference syscall carries a
//!   per-thread lifecycle marker, from the arm's entry to the syscall-boundary
//!   exit decision (`syscall::inference_syscall`). Force-kill never zombifies
//!   a marked thread: a holder or a waiter suspended in a ready queue owns
//!   `Arc<Model>`, prompt `Vec`s and tensor `Arc`s on its kernel stack, which a
//!   zombie reap would leak. Cancellation is cooperative instead: once the
//!   process is torn down, a waiter gives up in `acquire` without running the
//!   forward, the holder stops at its next token (`cancel_requested`), every
//!   local is dropped, and the thread exits at the boundary. The stack's own
//!   ownership (the `BUSY` lock plus the owner tid/pid) is separate from the
//!   marker. The three are separate atomics, published and cleared with
//!   interrupts masked; only tests read the owner tid/pid, so no safety
//!   decision rests on them.
//! - **Overflow fails closed.** The 64 KiB low guard turns an overflow into
//!   `#DF` (see the module docs of `inference_stack_logic`); the `#DF` handler
//!   reports `inference stack overflow` and halts. Two cases halt without
//!   that label (as a plain `#PF`): an underflow into the high guard (its
//!   `#PF` frame can be pushed), and a timer frame copy that faults on a
//!   nearly full stack. The kernel emits no stack probes, so a single frame
//!   over 64 KiB could still step past the guard; none exists in the forward.
//! - **Canary and high-water.** Each call starts from a canary plus fill
//!   pattern; afterwards the canary is checked (panic if hit) and the depth
//!   used is folded into a high-water mark the census reports.
//!
//! The 64 frames come from the boot frame allocator, like the heap's: they
//! are not heap memory.

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use x86_64::VirtAddr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, Translate,
};

use super::inference_stack_logic as logic;
use crate::serial_println;

/// The region base (the low guard page), or 0 before `init`.
static REGION: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicBool = AtomicBool::new(false);
/// The holder's thread id plus one, and its pid plus one (0: no holder).
static OWNER_TID: AtomicU64 = AtomicU64::new(0);
static OWNER_PID: AtomicU64 = AtomicU64::new(0);
static HIGH_WATER: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);

/// Map the inference stack. Runs once in single-threaded boot, right after the
/// heap, with the same mapper and boot frame allocator.
pub fn init<M, A>(mapper: &mut M, frame_alloc: &mut A)
where
    M: Mapper<Size4KiB> + Translate,
    A: FrameAllocator<Size4KiB>,
{
    let region =
        choose_region(mapper).expect("inference stack: no unmapped region beside the kernel heap");
    // GLOBAL: the region is never remapped, so its TLB entries may survive
    // CR3 switches like the rest of the kernel half.
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::NO_EXECUTE
        | PageTableFlags::GLOBAL;
    let first = Page::<Size4KiB>::containing_address(VirtAddr::new(logic::stack_bottom(region)));
    let last = Page::<Size4KiB>::containing_address(VirtAddr::new(logic::stack_top(region) - 1));
    for page in Page::range_inclusive(first, last) {
        let frame = frame_alloc
            .allocate_frame()
            .expect("inference stack: out of physical frames");
        // SAFETY: single-threaded boot before any user pmap or AP exists;
        // `choose_region` verified every page of the region (guards included)
        // is unmapped and outside the kernel image and the heap; the frame is
        // freshly allocated and owned by this mapping for the kernel's life.
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_alloc)
                .expect("inference stack: map_to failed")
                .flush();
        }
    }
    REGION.store(region, Ordering::Release);
    serial_println!(
        "[ kernel ] inference stack: {} KiB at {:#x}..{:#x} (guards {:#x}, {:#x})",
        logic::SIZE / 1024,
        logic::stack_bottom(region),
        logic::stack_top(region),
        region,
        logic::stack_top(region)
    );
}

/// The first free slot of `logic::region_candidates`. Every page of the
/// region must be unmapped and outside the kernel image and the heap.
fn choose_region<M: Translate>(mapper: &M) -> Option<u64> {
    let heap_lo = super::heap::heap_start().as_u64();
    let heap_hi = heap_lo.checked_add(super::heap::HEAP_SIZE as u64)?;
    let (image_lo, _) = super::heap::kernel_image_range();
    logic::region_candidates(heap_lo, heap_hi, image_lo)
        .into_iter()
        .flatten()
        .find(|&region| region_free(mapper, region, heap_lo, heap_hi))
}

fn region_free<M: Translate>(mapper: &M, region: u64, heap_lo: u64, heap_hi: u64) -> bool {
    let Some(end) = region.checked_add(logic::REGION_LEN) else {
        return false;
    };
    // Canonical upper half only.
    if region < 0xffff_8000_0000_0000 {
        return false;
    }
    let (image_lo, image_hi) = super::heap::kernel_image_range();
    let overlaps = |lo: u64, hi: u64| region < hi && lo < end;
    if overlaps(image_lo, image_hi) || overlaps(heap_lo, heap_hi) {
        return false;
    }
    let hhdm_end =
        super::hhdm_offset().saturating_add(super::HHDM_PHYS_END.load(Ordering::Acquire));
    if region < hhdm_end {
        return false;
    }
    let mut addr = region;
    while addr < end {
        if mapper.translate_addr(VirtAddr::new(addr)).is_some() {
            return false;
        }
        addr += 4096;
    }
    true
}

/// `#DF` support: does `addr` lie in one of the inference stack's guards?
pub fn guard_hit(addr: u64) -> Option<logic::GuardHit> {
    logic::guard_hit(REGION.load(Ordering::Acquire), addr)
}

/// The deepest use of the inference stack so far, in bytes.
pub fn high_water() -> usize {
    HIGH_WATER.load(Ordering::Acquire)
}

/// Completed calls on the inference stack.
pub fn calls() -> u64 {
    CALLS.load(Ordering::Acquire)
}

/// The thread holding the stack, if any.
#[cfg(feature = "neo-test")]
pub fn owner_thread() -> Option<u64> {
    OWNER_TID.load(Ordering::Acquire).checked_sub(1)
}

/// The holder's pid, if any. A separate atomic from the tid; test-only.
#[cfg(feature = "neo-test")]
pub fn owner_pid() -> Option<u32> {
    OWNER_PID
        .load(Ordering::Acquire)
        .checked_sub(1)
        .map(|pid| pid as u32)
}

/// Is the stack held right now (for the scenario's lock assertion)?
#[cfg(feature = "neo-test")]
pub fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// The lowest mapped stack byte (the canary's first byte).
#[cfg(feature = "neo-test")]
pub fn stack_bottom() -> u64 {
    logic::stack_bottom(REGION.load(Ordering::Acquire))
}

/// The mapped stack as bytes (lowest first).
///
/// # Safety
/// The caller holds `BUSY` and is not running on the stack.
unsafe fn stack_bytes(region: u64) -> &'static mut [u8] {
    // SAFETY: `init` mapped `[stack_bottom, stack_top)` writable for the
    // kernel's life; the caller's lock gives exclusive access.
    unsafe {
        core::slice::from_raw_parts_mut(
            logic::stack_bottom(region) as *mut u8,
            logic::SIZE as usize,
        )
    }
}

/// The outcome of the post-call check.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CallReport {
    pub depth: usize,
    pub canary_intact: bool,
}

/// Run `f` on the inference stack and return its result. Panics if the
/// canary was overwritten (the stack came within 64 bytes of its guard).
pub fn run<R, F: FnOnce() -> R>(f: F) -> R {
    let (result, report) = run_checked(f);
    finish(result, report)
}

/// `run` for an inference syscall: `None` if the caller's process was torn
/// down while it waited for the stack (it never ran `f`).
pub fn try_run<R, F: FnOnce() -> R>(f: F) -> Option<R> {
    let (result, report) = try_run_checked(f)?;
    Some(finish(result, report))
}

pub fn finish<R>(result: R, report: CallReport) -> R {
    if !report.canary_intact {
        panic!(
            "inference stack canary overwritten (depth {} of {} bytes)",
            report.depth,
            logic::SIZE
        );
    }
    result
}

/// `run`, returning the post-call report instead of panicking on a canary hit
/// (the scenario uses it to prove the canary check can fail). Kernel callers
/// are never cancelled.
pub fn run_checked<R, F: FnOnce() -> R>(f: F) -> (R, CallReport) {
    try_run_checked(f).expect("inference stack: a kernel caller was cancelled")
}

fn try_run_checked<R, F: FnOnce() -> R>(f: F) -> Option<(R, CallReport)> {
    let region = REGION.load(Ordering::Acquire);
    assert!(region != 0, "inference stack used before init");
    let held = Held::acquire()?;
    // SAFETY: the lock is held and we are on the caller's own stack.
    logic::prepare(unsafe { stack_bytes(region) });
    // SAFETY: the stack is mapped, exclusively ours (lock held), 16-aligned at
    // its top, and not in use by anything else.
    let result = unsafe {
        call_on_stack(logic::stack_top(region), || {
            interruptible(|| {
                #[cfg(feature = "neo-test-inference-kill")]
                test_park_while_armed();
                f()
            })
        })
    };
    // SAFETY: as above; the call has returned to the caller's stack.
    let stack = unsafe { stack_bytes(region) };
    let report = CallReport {
        depth: logic::depth_used(stack),
        canary_intact: logic::canary_intact(stack),
    };
    HIGH_WATER.fetch_max(report.depth, Ordering::AcqRel);
    CALLS.fetch_add(1, Ordering::AcqRel);
    drop(held);
    Some((result, report))
}

/// B1c AAG J3: has the calling thread's process been torn down? An inference
/// participant checks it while it waits for the stack and between tokens;
/// kernel threads are never cancelled.
pub fn cancel_requested() -> bool {
    let pid = crate::sched::current_pid();
    pid != crate::sched::process::KERNEL_PID && crate::sched::process::torn_down(pid)
}

/// The held stack: the lock plus the owner tid/pid, three separate atomics
/// published and cleared with interrupts masked (so a preempted holder never
/// leaves them half-set). Only tests read the owner tid/pid; no safety
/// decision rests on them (B1c AAG J4).
struct Held;

impl Held {
    /// The yielding lock. `None` if the caller's process is torn down while it
    /// waits (B1c AAG J3): it gives up without the stack.
    fn acquire() -> Option<Self> {
        let tid = crate::arch::x86_64::per_cpu::current_thread_id();
        let pid = crate::sched::current_pid().0;
        loop {
            // The release-race discriminator holds waiters here, before their
            // next CAS, while the holder finishes.
            #[cfg(feature = "neo-test-inference-kill")]
            while WAITER_GATE_CLOSED.load(Ordering::Acquire) {
                WAITER_GATED.fetch_add(1, Ordering::AcqRel);
                wait_round();
            }
            let won = crate::arch::x86_64::without_interrupts(|| {
                let won = BUSY
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok();
                if won {
                    OWNER_TID.store(tid + 1, Ordering::Release);
                    OWNER_PID.store(u64::from(pid) + 1, Ordering::Release);
                }
                won
            });
            if won {
                let held = Held;
                // B1c re-check #2 (C1): a waiter torn down while it waited may
                // win the stack after the holder releases it. It gives the
                // stack straight back and never enters the closure.
                if cancel_requested() {
                    #[cfg(feature = "neo-test-inference-kill")]
                    CANCELLED_AFTER_WIN.fetch_add(1, Ordering::AcqRel);
                    drop(held);
                    return None;
                }
                return Some(held);
            }
            if cancel_requested() {
                #[cfg(feature = "neo-test-inference-kill")]
                WAITER_CANCELLED.fetch_add(1, Ordering::AcqRel);
                return None;
            }
            #[cfg(feature = "neo-test-inference-kill")]
            WAITER_YIELDS.fetch_add(1, Ordering::AcqRel);
            wait_round();
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        crate::arch::x86_64::without_interrupts(|| {
            OWNER_PID.store(0, Ordering::Release);
            OWNER_TID.store(0, Ordering::Release);
            BUSY.store(false, Ordering::Release);
        });
    }
}

/// B1c AAG J1: the owner runs the whole call with interrupts on (the caller's
/// `IF` is restored before the call returns to its own stack).
#[cfg(not(feature = "neo-test-inference-shootdown-control"))]
fn interruptible<R>(f: impl FnOnce() -> R) -> R {
    crate::arch::x86_64::with_interrupts_enabled(f)
}

/// The scenario's negative control: the pre-fix owner keeps the caller's
/// `IF`, so a syscall-entry `IF=0` lasts the whole call.
#[cfg(feature = "neo-test-inference-shootdown-control")]
fn interruptible<R>(f: impl FnOnce() -> R) -> R {
    f()
}

/// One contended round of the yielding lock: never spin with the CPU
/// withheld from the holder. A syscall body runs at `IF=0` (SFMASK), so the
/// round yields, then opens a short interrupt window (ticks, shootdown and
/// reschedule IPIs) before restoring the caller's `IF`.
fn wait_round() {
    crate::sched::scheduler::yield_now();
    let was_enabled = x86_64::instructions::interrupts::are_enabled();
    x86_64::instructions::interrupts::enable();
    core::hint::spin_loop();
    if !was_enabled {
        x86_64::instructions::interrupts::disable();
    }
}

/// `inference_stack_kill`'s instrumentation: contended rounds, waiters that
/// gave up on a teardown, and the parked-holder hook.
#[cfg(feature = "neo-test-inference-kill")]
pub static WAITER_YIELDS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-kill")]
pub static WAITER_CANCELLED: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-kill")]
pub static CANCELLED_AFTER_WIN: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-kill")]
pub static WAITER_GATE_CLOSED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-inference-kill")]
pub static WAITER_GATED: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-kill")]
pub static PARK_ARMED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-inference-kill")]
pub static PARKED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-inference-kill")]
pub static PARK_LEFT_ON_CANCEL: AtomicBool = AtomicBool::new(false);

/// The holder parks here, on the inference stack with interrupts on (so it is
/// preemptible on the BSP), until the scenario disarms it or its process is
/// torn down.
#[cfg(feature = "neo-test-inference-kill")]
fn test_park_while_armed() {
    if !PARK_ARMED.load(Ordering::Acquire) {
        return;
    }
    PARKED.store(true, Ordering::Release);
    while PARK_ARMED.load(Ordering::Acquire) {
        if cancel_requested() {
            PARK_LEFT_ON_CANCEL.store(true, Ordering::Release);
            break;
        }
        core::hint::spin_loop();
    }
    PARKED.store(false, Ordering::Release);
}

/// Call `f` with `rsp` at `top`, then return to the caller's stack.
///
/// # Safety
/// `[top - SIZE, top)` is mapped, writable, 16-aligned at `top`, and used by
/// nothing else for the duration of the call.
unsafe fn call_on_stack<R, F: FnOnce() -> R>(top: u64, f: F) -> R {
    struct Frame<F, R> {
        f: Option<F>,
        result: Option<R>,
    }

    extern "C" fn trampoline<F: FnOnce() -> R, R>(frame: *mut u8) {
        // SAFETY: `frame` is the `&mut Frame` the caller passed in `rdi`; it
        // lives on the caller's stack, which is untouched until we return.
        let frame = unsafe { &mut *frame.cast::<Frame<F, R>>() };
        if let Some(f) = frame.f.take() {
            frame.result = Some(f());
        }
    }

    let mut frame = Frame {
        f: Some(f),
        result: None,
    };
    let entry: extern "C" fn(*mut u8) = trampoline::<F, R>;
    // SAFETY: the caller guarantees the new stack. The old `rsp` is saved on
    // the new stack and restored with `pop rsp`; `top - 16` keeps the SysV
    // 16-byte alignment at the `call`. `clobber_abi("C")` covers everything
    // the callee may clobber, including `rax`, which we use for the old `rsp`.
    unsafe {
        core::arch::asm!(
            "mov rax, rsp",
            "mov rsp, rsi",
            "push rax",
            "sub rsp, 8",
            "call rdx",
            "add rsp, 8",
            "pop rsp",
            in("rdi") (&mut frame as *mut Frame<F, R>).cast::<u8>(),
            in("rsi") top,
            in("rdx") entry,
            clobber_abi("C"),
        );
    }
    frame
        .result
        .take()
        .expect("inference stack: the call did not complete")
}
