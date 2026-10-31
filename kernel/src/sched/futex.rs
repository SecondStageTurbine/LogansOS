// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.1 (P1): the futex — a kernel wait-queue keyed by the futex word's
//! PHYSICAL address, built on the named futex substrate
//! (`try_block_current_with_token_and_guard` + `wake_thread_token`).
//!
//! ABI (frozen, `Plans/RATIFY-2026-07-22-c1-abi-amended-freeze.md`):
//!   - `futex_wait(uaddr, expected, deadline_monotonic_ns)` — the deadline is
//!     an ABSOLUTE monotonic instant on the C1.0 clock (`u64::MAX` = infinite,
//!     `0` = already-expired). Returns `0` woken, `-ETIMEDOUT`, or `-EAGAIN`
//!     when `*uaddr != expected` at entry (the lost-wakeup guard).
//!   - `futex_wake(uaddr, count)` — returns the number of threads actually
//!     woken (PRESERVED: Rust's rwlock uses the count for its writer→reader
//!     fallback).
//!
//! Keying by physical address (`Pmap::translate`) means aliased mappings of
//! the same frame share one queue, and two processes' distinct words never
//! collide.
//!
//! Lost-wakeup discipline (CP-8/CP-14/CP-17): the `*uaddr == expected`
//! re-check happens UNDER the queue guard, and
//! `try_block_current_with_token_and_guard` releases that guard atomically
//! with the context switch (on the new stack). A concurrent `futex_wake`
//! either sees the parked token (and wakes it) or runs before the waiter
//! takes the guard — in which case the waiter observes the already-changed
//! value and returns `-EAGAIN`. There is no instant where the waiter is
//! committed-to-sleep, lock-free, and still on-CPU.
//!
//! Lock order (CP-1): FUTEX_QUEUE (leaf) → scheduler lock. Every path that
//! takes both takes them in that order (wait: queue guard held into the
//! block primitive; wake/expire: queue guard dropped BEFORE
//! `wake_thread_token` takes the scheduler lock — the CP-16 discipline).
//! Nothing takes the scheduler lock and then the futex queue. The futex
//! path takes NO AI-governance lock (I-14a).
//!
//! Timeouts: a per-waiter absolute tick deadline, scanned from the timer
//! tick (`on_tick_expire`). Two atomics (`TIMED_WAITERS`,
//! `EARLIEST_DEADLINE_TICKS`) keep the no-timed-waiter tick path to one
//! relaxed load.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::arch::x86_64::timer::TICKS;
use crate::arch::x86_64::with_user_access;
use crate::errno::{EAGAIN, ESRCH, ETIMEDOUT};
use crate::sync::IrqSafeMutex;

use super::process::ProcessId;
use super::scheduler::{self, ThreadWakeToken, WakeTokenResult};

/// Nanoseconds per timer tick (100 Hz — mirrors `sys_clock_gettime_impl`).
const NS_PER_TICK: u64 = 10_000_000;

/// Sentinel: no timeout.
pub const DEADLINE_INFINITE: u64 = u64::MAX;

/// Bounded waiter table. 64 concurrent futex waiters is far above any C1
/// battery (and the per-CPU scheduler token table backing them holds 16 per
/// CPU); a full table fails closed with `-EAGAIN` (the caller re-checks and
/// retries), never a panic — user input must not be able to panic the kernel.
const FUTEX_WAITERS_MAX: usize = 64;

struct FutexWaiter {
    /// Queue key. For a `SYS_FUTEX_WAIT` waiter this is the **physical
    /// address** of the 4-byte user word (aliased mappings share a queue).
    /// For a `thread_join` waiter (OT-61 C1.1 blocker F — join converges on
    /// this same futex queue, not a private join-token store) it is the
    /// synthetic per-thread key `join_key(target_tid)` whose bit 63 is set so
    /// it can never collide with a physical address.
    key: u64,
    /// Scheduler park token; generation-checked, so a wake racing teardown
    /// is a safe no-op (`Stale`/`Gone`).
    token: ThreadWakeToken,
    /// Absolute deadline in `TICKS` units; `u64::MAX` = no timeout.
    deadline_ticks: u64,
    /// Owning process, so forced teardown can purge this entry.
    pid: ProcessId,
}

/// OT-61 C1.1 (blocker F): synthetic join key namespace. Physical addresses
/// on x86-64 have bit 63 clear (they are < 2^52), so setting bit 63 gives a
/// key space disjoint from every `SYS_FUTEX_WAIT` physical-address key. Join
/// converges on the futex wait/wake mechanism by parking on `join_key(tid)`;
/// `thread_exit` wakes the same key. No separate join-wait registry exists.
pub const JOIN_KEY_FLAG: u64 = 1 << 63;

/// The futex queue key a `thread_join` on `tid` parks on / `thread_exit` of
/// `tid` wakes. Derived purely from the globally-unique thread id.
pub fn join_key(tid: u64) -> u64 {
    JOIN_KEY_FLAG | (tid & (JOIN_KEY_FLAG - 1))
}

struct FutexQueue {
    slots: [Option<FutexWaiter>; FUTEX_WAITERS_MAX],
}

impl FutexQueue {
    const fn new() -> Self {
        Self {
            slots: [const { None }; FUTEX_WAITERS_MAX],
        }
    }
}

static FUTEX_QUEUE: IrqSafeMutex<FutexQueue> = IrqSafeMutex::new(FutexQueue::new());

/// Count of waiters with a finite deadline — the timer-tick fast-path gate.
static TIMED_WAITERS: AtomicUsize = AtomicUsize::new(0);
/// Earliest finite deadline (ticks). Monotonically folded down on install;
/// recomputed on removal. `u64::MAX` when no timed waiter exists.
static EARLIEST_DEADLINE_TICKS: AtomicU64 = AtomicU64::new(u64::MAX);

/// Convert an absolute monotonic ns deadline to an absolute tick deadline,
/// rounding UP so the kernel never times out BEFORE the requested instant
/// (10 ms tick granularity means it may time out up to one tick late).
fn deadline_ns_to_ticks(deadline_ns: u64) -> u64 {
    if deadline_ns == DEADLINE_INFINITE {
        return u64::MAX;
    }
    deadline_ns.div_ceil(NS_PER_TICK)
}

fn recompute_earliest(q: &FutexQueue) {
    let mut earliest = u64::MAX;
    for slot in q.slots.iter().flatten() {
        if slot.deadline_ticks < earliest {
            earliest = slot.deadline_ticks;
        }
    }
    EARLIEST_DEADLINE_TICKS.store(earliest, Ordering::Relaxed);
}

/// Core wait primitive: park the current thread on futex queue `key` until
/// woken (or the absolute `deadline_ns` passes), UNLESS `should_park()` — the
/// value re-check, evaluated **under the queue guard** — reports the
/// condition already changed (returns `false`), in which case return
/// `-EAGAIN` without parking. This under-the-guard re-check is the CP-8/CP-14
/// lost-wakeup discipline: a concurrent `wake` on the same key either sees
/// the installed token (and wakes it) or ran before this point (and
/// `should_park` observes its store and refuses to park). The compare and the
/// park are atomic against a concurrent wake because both hold the queue
/// guard, which releases atomically with the context switch on the new stack.
///
/// Holds **no owned `Arc`/`Box` across the park** (CP-30) — the caller passes
/// a `Copy` key and a borrow-free closure — so a thread force-zombified while
/// parked here leaks nothing.
pub fn wait_on_key<F: Fn() -> bool>(key: u64, should_park: F, deadline_ns: u64) -> i64 {
    let deadline_ticks = deadline_ns_to_ticks(deadline_ns);
    // Read the pid BEFORE taking the queue guard: current_pid() takes the
    // scheduler lock, and while queue→scheduler is the documented order,
    // there is no reason to nest here at all.
    let pid = super::current_pid();
    #[cfg(feature = "neo-test-threads-exit-group")]
    late_park_hold(pid);
    // C1.3b (CP-86): the caller's process, fetched before the queue lock (no
    // PROCESSES lock nests inside it) and dropped before the park (CP-30).
    let process = super::process::get(pid);

    let queue = FUTEX_QUEUE.lock();

    // A thread of a process being retired must not park after the retire's
    // purge (which takes this lock after marking the process): it would never
    // be woken. Refuse; its syscall return then retires it.
    if process.is_some_and(|p| p.is_retiring()) {
        return ESRCH;
    }

    // The value/condition re-check under the queue guard (CP-8/CP-14).
    if !should_park() {
        return EAGAIN;
    }

    // Absolute deadline already reached (including the `0` = already-expired
    // sentinel): the ABI says report the timeout AFTER the value check.
    if deadline_ticks != u64::MAX && TICKS.load(Ordering::Relaxed) >= deadline_ticks {
        return ETIMEDOUT;
    }

    let Some(slot_idx) = queue.slots.iter().position(|s| s.is_none()) else {
        // Table full: fail closed, let the caller re-check + retry
        // (spurious-wake-shaped). Never a panic — user input must not panic.
        return EAGAIN;
    };

    let timed = deadline_ticks != u64::MAX;

    // Park: the token is installed into the queue slot while BOTH the queue
    // guard and the scheduler lock are held; both release atomically on the
    // new stack (the I-11 no-lost-wakeup primitive). `Err` = the per-CPU
    // token table is full — fail closed with EAGAIN, waking nobody.
    match scheduler::try_block_current_with_token_and_guard(queue, |q, token| {
        // Hang discriminator: record WHICH thread parked on WHICH key with a
        // token on WHICH CPU, under the queue guard, before the switch. Costs
        // one relaxed store; read back only if the watchdog fires.
        #[cfg(feature = "neo-test")]
        super::wake_trace::note_event(
            super::wake_trace::kind::PARK,
            token.cpu_id,
            token.slot,
            token.tid,
            key,
        );
        q.slots[slot_idx] = Some(FutexWaiter {
            key,
            token,
            deadline_ticks,
            pid,
        });
        if timed {
            TIMED_WAITERS.fetch_add(1, Ordering::Relaxed);
            EARLIEST_DEADLINE_TICKS.fetch_min(deadline_ticks, Ordering::Relaxed);
        }
    }) {
        Ok(ret) => ret,
        Err(_queue_guard) => EAGAIN,
    }
}

/// C1.3b test-only: the thread armed by `SYS_TEST_FUTEX_LATE_PARK` (its tid,
/// or `u64::MAX`).
#[cfg(feature = "neo-test-threads-exit-group")]
static LATE_PARK_TID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(u64::MAX);

/// `SYS_TEST_FUTEX_LATE_PARK`: arm the calling thread. Returns 0.
#[cfg(feature = "neo-test-threads-exit-group")]
pub fn arm_late_park_for_test() -> i64 {
    LATE_PARK_TID.store(
        crate::arch::x86_64::per_cpu::current_thread_id(),
        Ordering::Release,
    );
    0
}

/// For the armed thread only (once): before the queue lock, wait (interrupts
/// on, no lock held) until `pid` is being retired, then 3 more ticks so the
/// retire's futex purge has run. Bounded at 1,000 ticks.
#[cfg(feature = "neo-test-threads-exit-group")]
fn late_park_hold(pid: super::process::ProcessId) {
    let me = crate::arch::x86_64::per_cpu::current_thread_id();
    if LATE_PARK_TID
        .compare_exchange(me, u64::MAX, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    // Syscalls run with IF=0; hold with interrupts on so TLB shootdowns and
    // IPIs are served meanwhile (CP-64). No lock is held here.
    let was_enabled = crate::arch::x86_64::interrupts_enabled();
    x86_64::instructions::interrupts::enable();
    let start = TICKS.load(Ordering::Relaxed);
    let mut seen: Option<u64> = None;
    loop {
        let now = TICKS.load(Ordering::Relaxed);
        if now.saturating_sub(start) > 1_000 {
            break;
        }
        match seen {
            Some(at) if now >= at + 3 => break,
            Some(_) => {}
            None => {
                if super::process::get(pid).is_some_and(|p| p.is_retiring()) {
                    seen = Some(now);
                }
            }
        }
        core::hint::spin_loop();
    }
    if !was_enabled {
        x86_64::instructions::interrupts::disable();
    }
}

/// `SYS_FUTEX_WAIT` body. `uaddr` has been range-validated and
/// alignment-checked by the syscall layer; `paddr` is its physical address
/// (the queue key), pre-translated by the caller so that **no owned
/// `Arc<Pmap>` is on this thread's stack across the block** (CP-30). The
/// value re-check reads `uaddr` directly: this is the caller's thread, so its
/// address space (CR3) is active and the read needs no Arc.
pub fn wait(paddr: u64, uaddr: u64, expected: u32, deadline_ns: u64) -> i64 {
    // should_park == "the word still holds `expected`" (unchanged ⇒ park).
    // SAFETY: `uaddr` is a validated, 4-aligned user address in the caller's
    // live address space; SMAP is opened for exactly this read, under the
    // queue guard (inside `wait_on_key`).
    wait_on_key(
        paddr,
        || with_user_access(|| unsafe { (uaddr as *const u32).read_volatile() }) == expected,
        deadline_ns,
    )
}

/// `SYS_FUTEX_WAKE` body (and the `thread_exit` join wake): wake up to
/// `count` waiters parked on queue `key` — a physical address for a user
/// futex, or `join_key(tid)` for a join. Returns the number woken.
pub fn wake(key: u64, count: u32) -> i64 {
    if count == 0 {
        return 0;
    }

    // Collect matching tokens under the queue guard, then wake OUTSIDE it
    // (CP-16: never call into the scheduler wake path with a foreign lock
    // held when it can be avoided). A token whose thread was already torn
    // down wakes as Stale/Gone and does not count.
    let mut tokens: [Option<ThreadWakeToken>; FUTEX_WAITERS_MAX] = [None; FUTEX_WAITERS_MAX];
    let mut taken = 0usize;
    {
        let mut queue = FUTEX_QUEUE.lock();
        let mut removed_timed = false;

        for slot in queue.slots.iter_mut() {
            if taken >= count as usize {
                break;
            }
            if let Some(waiter) = slot
                && waiter.key == key
            {
                let waiter = slot.take().expect("checked Some above");
                if waiter.deadline_ticks != u64::MAX {
                    TIMED_WAITERS.fetch_sub(1, Ordering::Relaxed);
                    removed_timed = true;
                }
                // Hang discriminator: this waiter WAS targeted by a wake. The
                // separation matters — "never targeted by any wake" and "woken
                // but never dispatched" are different bugs with the same
                // silence, and the ring distinguishes them (`wake-pick` present
                // vs absent for a given slot).
                #[cfg(feature = "neo-test")]
                super::wake_trace::note_event(
                    super::wake_trace::kind::WAKE_PICK,
                    waiter.token.cpu_id,
                    waiter.token.slot,
                    0,
                    key,
                );
                tokens[taken] = Some(waiter.token);
                taken += 1;
            }
        }
        if removed_timed {
            recompute_earliest(&queue);
        }
    }

    let mut woken = 0i64;
    for token in tokens.iter().take(taken).flatten() {
        if scheduler::wake_thread_token(*token, 0) == WakeTokenResult::Woken {
            woken += 1;
        }
    }
    woken
}

/// Timer-tick expiry scan. Called from the timer interrupt path (before the
/// preemption rotate), so the fast path must be cheap: one relaxed load when
/// no timed waiter exists, two when none has expired.
pub fn on_tick_expire() {
    if TIMED_WAITERS.load(Ordering::Relaxed) == 0 {
        return;
    }
    let now = TICKS.load(Ordering::Relaxed);
    if now < EARLIEST_DEADLINE_TICKS.load(Ordering::Relaxed) {
        return;
    }

    let mut tokens: [Option<ThreadWakeToken>; FUTEX_WAITERS_MAX] = [None; FUTEX_WAITERS_MAX];
    let mut taken = 0usize;
    {
        let mut queue = FUTEX_QUEUE.lock();
        for slot in queue.slots.iter_mut() {
            if let Some(waiter) = slot
                && waiter.deadline_ticks <= now
            {
                let waiter = slot.take().expect("checked Some above");
                TIMED_WAITERS.fetch_sub(1, Ordering::Relaxed);
                #[cfg(feature = "neo-test")]
                super::wake_trace::note_event(
                    super::wake_trace::kind::TIMEOUT,
                    waiter.token.cpu_id,
                    waiter.token.slot,
                    0,
                    waiter.key,
                );
                tokens[taken] = Some(waiter.token);
                taken += 1;
            }
        }
        recompute_earliest(&queue);
    }

    for token in tokens.iter().take(taken).flatten() {
        // A racing futex_wake cannot hold the same entry (it was removed
        // under the guard above); a racing teardown makes this Stale/Gone —
        // a safe no-op either way (generation-checked, CP-12).
        let _ = scheduler::wake_thread_token(*token, ETIMEDOUT);
    }
}

/// OT-61 C1.1 hang discriminator: a cheap fingerprint of the wait queue —
/// `(waiter_count, hash)`. The watchdog (`wake_trace::on_tick_watchdog`) calls
/// this once every ~2.5 s from the timer tick and dumps full cross-CPU state
/// when the value stops changing while waiters remain: a **frozen non-empty
/// futex queue is the signature of this hang**, and it is a state healthy
/// contention never holds for seconds (the battery's own deadline checks wait
/// milliseconds).
///
/// Folds in the token identity, not just the key, so a queue whose *waiters*
/// were swapped for different ones at the same addresses does not read as
/// frozen.
#[cfg(feature = "neo-test")]
pub fn waiter_fingerprint() -> (usize, u64) {
    let queue = FUTEX_QUEUE.lock();
    let mut count = 0usize;
    let mut hash = 0u64;
    for w in queue.slots.iter().flatten() {
        count += 1;
        hash = hash
            .rotate_left(7)
            .wrapping_add(w.key)
            .wrapping_add(u64::from(w.token.cpu_id) << 32)
            .wrapping_add(u64::from(w.token.slot) << 16)
            .wrapping_add(w.token.generation);
    }
    (count, hash)
}

/// OT-61 C1.1 hang discriminator: list every still-parked waiter.
///
/// **This is the single most decisive line in a hang dump.** The silent
/// `threads_futex` hang cannot today distinguish "15/16 were never woken" from
/// "15/16 were woken and then never dispatched" — identical user-visible
/// silence, opposite fixes. A waiter *still listed here* at hang time was never
/// woken; a waiter *absent* here was woken and is stranded on some CPU's ready
/// queue (read that off the `wake_trace` per-CPU rows).
///
/// Serial-only printing (`serial_println_isr!`) because this runs in the timer
/// ISR; see `crate::drivers::console::serial::_print_isr`. Taking
/// `FUTEX_QUEUE` here introduces no new lock discipline — `on_tick_expire`
/// already locks it from this exact context.
#[cfg(feature = "neo-test")]
pub fn dump_waiters() {
    let now = TICKS.load(Ordering::Relaxed);
    let queue = FUTEX_QUEUE.lock();
    let mut any = false;
    for (idx, w) in queue.slots.iter().enumerate() {
        let Some(w) = w else { continue };
        any = true;
        let is_join = w.key & JOIN_KEY_FLAG != 0;
        crate::serial_println_isr!(
            "[wake-trace] parked slot{} key={:#x} kind={} tokcpu={} tokslot={} gen={} pid={} deadline={} now={}",
            idx,
            w.key,
            if is_join { "join" } else { "futex" },
            w.token.cpu_id,
            w.token.slot,
            w.token.generation,
            w.pid.0,
            w.deadline_ticks,
            now,
        );
    }
    if !any {
        crate::serial_println_isr!("[wake-trace] parked: NONE (queue empty)");
    }
}

/// OT-61 C2.3b discriminator: how many of `pid`'s threads are parked in
/// `SYS_FUTEX_WAIT` right now (joins excluded). `servo_closure_smoke` samples
/// it to tell a thread parker that blocks from one that spins.
#[cfg(feature = "neo-test-servo-closure-smoke")]
pub fn futex_waiters_of(pid: ProcessId) -> usize {
    let queue = FUTEX_QUEUE.lock();
    queue
        .slots
        .iter()
        .flatten()
        .filter(|w| w.pid == pid && w.key & JOIN_KEY_FLAG == 0)
        .count()
}

/// Forced-teardown hook: drop every queue entry owned by `pid`. The parked
/// threads themselves are extracted and zombified by the caller
/// (`zombify_parked_thread_by_pid` walks the scheduler token tables); this
/// only prevents the dead entries from leaking queue slots and prevents a
/// late `futex_wake` from re-readying a thread of a dying process.
pub fn purge_pid(pid: ProcessId) {
    let mut queue = FUTEX_QUEUE.lock();
    let mut removed_timed = false;
    for slot in queue.slots.iter_mut() {
        if let Some(waiter) = slot
            && waiter.pid == pid
        {
            let waiter = slot.take().expect("checked Some above");
            if waiter.deadline_ticks != u64::MAX {
                TIMED_WAITERS.fetch_sub(1, Ordering::Relaxed);
                removed_timed = true;
            }
        }
    }
    if removed_timed {
        #[cfg(feature = "neo-test-threads-guard-fault-timed")]
        PURGED_TIMED_RECOMPUTES.fetch_add(1, Ordering::Relaxed);
        recompute_earliest(&queue);
    }
}

/// CP-46 test witness: `purge_pid` calls that removed a timed waiter and so
/// ran `recompute_earliest`. In `threads_guard_fault_timed` that happens on
/// the #PF fault-kill path, which is what the scenario must exercise.
#[cfg(feature = "neo-test-threads-guard-fault-timed")]
static PURGED_TIMED_RECOMPUTES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-threads-guard-fault-timed")]
pub fn purged_timed_recomputes_for_test() -> u64 {
    PURGED_TIMED_RECOMPUTES.load(Ordering::Relaxed)
}

/// C1.1 test-only: return the number of waiters parked on queue `key`.
/// Used by the threads_futex barrier to confirm all waiters are enqueued
/// before the waker starts. Ground truth from the kernel queue, not a
/// pre-syscall counter.
#[cfg(feature = "neo-test")]
pub fn queue_len_for_key(key: u64) -> usize {
    let queue = FUTEX_QUEUE.lock();
    queue
        .slots
        .iter()
        .filter(|s| matches!(s.as_ref(), Some(w) if w.key == key))
        .count()
}
