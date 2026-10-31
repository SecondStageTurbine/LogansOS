// SPDX-License-Identifier: MPL-2.0
//! Kernel-thread timed sleep without `wait-multiplex` (`tick-sleep`, CP-81).
//!
//! `wait_any::sleep_ticks` is the only timed park, and it is compiled only with
//! `wait-multiplex`, which the product image does not build. Without it every
//! kernel-side "nap" (`usb::nap_ticks`, the touchpad's polled fallbacks, the
//! boot input check) was `yield_now` + spin until the deadline tick: a thread
//! that is runnable on every tick for as long as it waits.
//!
//! This is the smallest timed wake: [`SLOTS`] sleepers in one table, each a
//! deadline tick and a wake token. The timer tick ([`on_tick_expire`]) takes
//! every due token and wakes its thread after dropping the table lock.
//!
//! - **No lost wake (CP-8/CP-14).** The sleeper checks its deadline and takes a
//!   slot under `TABLE`, with IF off, and the guard is released only by the
//!   context switch that parks it. A tick on another CPU needs `TABLE` to see
//!   the slot, so it finds the token installed; a tick on this CPU cannot run
//!   in between (IF off).
//! - **Lock order.** `TABLE` → scheduler, in both the sleeper (its park) and
//!   the tick (`wake_thread_token` after `TABLE` is dropped). No governance
//!   lock (I-14(a)).
//! - **Table full.** The sleeper falls back to the old yield loop and counts
//!   it (`full`, `spins`), so a full table is slower, never wrong.
//! - **Stop.** A kthread stop request does not end a sleep early; the sleep
//!   still ends at its deadline.
//!
//! Feature off: not compiled, and every caller keeps its yield loop.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::arch::x86_64::timer::TICKS;
use crate::sched::scheduler::{self, ThreadWakeToken};
use crate::sync::IrqSafeMutex;

/// Sleepers at once. Today's users: the USB port service, the touchpad's
/// polled fallback and the boot input check, at most one each.
pub const SLOTS: usize = 8;

#[derive(Clone, Copy)]
struct Sleeper {
    deadline: u64,
    token: ThreadWakeToken,
}

static TABLE: IrqSafeMutex<[Option<Sleeper>; SLOTS]> = IrqSafeMutex::new([None; SLOTS]);
/// Installed sleepers: the tick's fast path is one relaxed load of this.
static SLEEPING: AtomicUsize = AtomicUsize::new(0);
/// The earliest installed deadline (`u64::MAX` with none).
static EARLIEST: AtomicU64 = AtomicU64::new(u64::MAX);

/// Sleeps that parked, sleeps that found the table full, and yield passes
/// made by those (the overlay's `sleep` field).
static PARKED: AtomicU64 = AtomicU64::new(0);
static FULL: AtomicU64 = AtomicU64::new(0);
static SPINS: AtomicU64 = AtomicU64::new(0);

/// Sleep at least `ticks` timer ticks (at least one).
pub fn sleep_ticks(ticks: u64) {
    sleep_until(TICKS.load(Ordering::Relaxed).saturating_add(ticks.max(1)));
}

/// Sleep until the tick count reaches `deadline`. Returns at once if it
/// already has.
pub fn sleep_until(deadline: u64) {
    if park_until(deadline) == Park::Full {
        FULL.fetch_add(1, Ordering::Relaxed);
        while TICKS.load(Ordering::Relaxed) < deadline {
            SPINS.fetch_add(1, Ordering::Relaxed);
            super::yield_now();
            core::hint::spin_loop();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Park {
    /// Parked, and woken by the tick at or after the deadline.
    Slept,
    /// The deadline had already passed: not parked.
    Due,
    /// No free slot: not parked.
    Full,
}

/// Park until `deadline`.
#[cfg(not(feature = "neo-test-tick-sleep-control"))]
fn park_until(deadline: u64) -> Park {
    // IF off for the whole check-and-park: the context switch that releases
    // the guard requires an IF-off caller, and this CPU's tick cannot run
    // between the check and the install.
    crate::arch::x86_64::without_interrupts(|| {
        let table = TABLE.lock();
        if TICKS.load(Ordering::Relaxed) >= deadline {
            return Park::Due;
        }
        let Some(free) = table.iter().position(Option::is_none) else {
            return Park::Full;
        };
        PARKED.fetch_add(1, Ordering::Relaxed);
        let _ = scheduler::block_current_with_token_and_guard(table, |slots, token| {
            slots[free] = Some(Sleeper { deadline, token });
            SLEEPING.fetch_add(1, Ordering::Relaxed);
            EARLIEST.fetch_min(deadline, Ordering::Relaxed);
        });
        Park::Slept
    })
}

/// Control (`neo-test-tick-sleep-control`): never parks, so every sleep is
/// the old yield loop. `tick_sleep_selftest` must FAIL `tick_sleep_not_parked`.
#[cfg(feature = "neo-test-tick-sleep-control")]
fn park_until(_deadline: u64) -> Park {
    Park::Full
}

/// Timer tick: wake every sleeper whose deadline has passed.
pub fn on_tick_expire() {
    if SLEEPING.load(Ordering::Relaxed) == 0 {
        return;
    }
    #[cfg(not(any(
        feature = "neo-test-tick-sleep-early-control",
        feature = "neo-test-tick-sleep-no-wake-control"
    )))]
    let now = TICKS.load(Ordering::Relaxed);
    // Control (`neo-test-tick-sleep-early-control`): every sleeper is woken
    // one tick before its deadline. The selftest must FAIL (`woken_early`).
    #[cfg(feature = "neo-test-tick-sleep-early-control")]
    let now = TICKS.load(Ordering::Relaxed) + 1;
    // Control (`neo-test-tick-sleep-no-wake-control`): nothing is ever due,
    // so no sleeper is woken. The selftest must FAIL `tick_sleep_not_parked`
    // within its own wait, not hang to the scenario's timeout.
    #[cfg(feature = "neo-test-tick-sleep-no-wake-control")]
    let now = 0;
    if now < EARLIEST.load(Ordering::Relaxed) {
        return;
    }
    let mut due: [Option<ThreadWakeToken>; SLOTS] = [None; SLOTS];
    {
        let mut table = TABLE.lock();
        let mut earliest = u64::MAX;
        for (slot, out) in table.iter_mut().zip(due.iter_mut()) {
            match *slot {
                Some(s) if s.deadline <= now => {
                    // The selftest's own sleeper (its token names the worker
                    // thread): the real tick of its FIRST expiry. Another
                    // sleeper with the same deadline cannot overwrite it.
                    #[cfg(feature = "neo-test-usb-hotplug")]
                    if s.token.tid == SELFTEST_TID.load(Ordering::Acquire) {
                        let _ = SELFTEST_EXPIRED_AT.compare_exchange(
                            0,
                            TICKS.load(Ordering::Relaxed).max(1),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        );
                    }
                    *out = Some(s.token);
                    *slot = None;
                    SLEEPING.fetch_sub(1, Ordering::Relaxed);
                }
                Some(s) => earliest = earliest.min(s.deadline),
                None => {}
            }
        }
        EARLIEST.store(earliest, Ordering::Relaxed);
    }
    for token in due.iter().flatten() {
        let _ = scheduler::wake_thread_token(*token, 0);
    }
}

/// `(parked, full, spins)` for the overlay.
pub fn counters() -> (u64, u64, u64) {
    (
        PARKED.load(Ordering::Relaxed),
        FULL.load(Ordering::Relaxed),
        SPINS.load(Ordering::Relaxed),
    )
}

/// The selftest worker's thread id (`u64::MAX`: none) and the real tick at
/// which the tick first expired that thread's token (0: not yet). "Early" is
/// judged at expiry, not only at resume: a thread woken a tick early can
/// still run a tick late (Chad, relay #61). The witness is keyed to the
/// worker's own token, not its deadline: the USB port service can install a
/// different token with the same deadline after the worker's expiry, and a
/// deadline match would let that later expiry overwrite the early one
/// (Chad, relay #70).
#[cfg(feature = "neo-test-usb-hotplug")]
static SELFTEST_TID: AtomicU64 = AtomicU64::new(u64::MAX);
#[cfg(feature = "neo-test-usb-hotplug")]
static SELFTEST_EXPIRED_AT: AtomicU64 = AtomicU64::new(0);

/// The discriminator (`usb_hotplug`): a test thread sleeps 20 ticks. It must
/// park (its token appears in the table), run again no earlier than its
/// deadline, and run again at all, woken by the tick. A deadline already
/// passed returns without parking. Returns `(parked, woken_early, woken,
/// past_skips, slept_ticks)`.
///
/// "Early" is judged twice against the thread's own deadline, never by a
/// possibly slow observer (Chad, relay #56): the real tick at which the tick
/// expired its token (relay #61: a token expired a tick early can still
/// resume a tick late), and the tick it recorded on resuming.
/// A thread never woken is detached, not joined, so the caller fails at once
/// with its own reason instead of hanging to the scenario's timeout.
#[cfg(feature = "neo-test-usb-hotplug")]
pub fn selftest() -> (bool, bool, bool, bool, u64) {
    use crate::sched::kthread::{self, KthreadExitState};
    static WOKE_AT: AtomicU64 = AtomicU64::new(0);
    static START: AtomicU64 = AtomicU64::new(0);
    static DEADLINE: AtomicU64 = AtomicU64::new(0);
    const SLEEP: u64 = 20;
    let ticks_now = || TICKS.load(Ordering::Relaxed);
    // SAFETY: RDTSC has no side effect.
    let tsc = || unsafe { core::arch::x86_64::_rdtsc() };
    // Each pass leaves interrupts on for a real window (CP-73), and the TSC
    // bound ends the wait even if the tick stops.
    let wait_until = |ticks: u64, done: &dyn Fn() -> bool| {
        let deadline = ticks_now().saturating_add(ticks);
        let tsc_deadline = tsc().saturating_add(ticks.saturating_mul(20_000_000));
        while !done() && ticks_now() < deadline && tsc() < tsc_deadline {
            for _ in 0..2_000 {
                core::hint::spin_loop();
            }
            super::yield_now();
        }
        done()
    };
    // The worker's own slot (the port service sleeps in the same table, and
    // can have the same deadline): its token names the worker thread.
    let installed = || {
        let tid = SELFTEST_TID.load(Ordering::Acquire);
        let deadline = DEADLINE.load(Ordering::Acquire);
        deadline != u64::MAX
            && crate::arch::x86_64::without_interrupts(|| {
                TABLE
                    .lock()
                    .iter()
                    .flatten()
                    .any(|s| s.token.tid == tid && s.deadline == deadline)
            })
    };
    WOKE_AT.store(0, Ordering::Release);
    DEADLINE.store(u64::MAX, Ordering::Release);
    SELFTEST_EXPIRED_AT.store(0, Ordering::Release);
    let job = |_stop| {
        SELFTEST_TID.store(
            crate::arch::x86_64::per_cpu::current_thread_id(),
            Ordering::Release,
        );
        let start = TICKS.load(Ordering::Relaxed);
        START.store(start, Ordering::Release);
        DEADLINE.store(start + SLEEP, Ordering::Release);
        sleep_until(start + SLEEP);
        WOKE_AT.store(TICKS.load(Ordering::Relaxed).max(1), Ordering::Release);
        KthreadExitState::Success
    };
    let handle = kthread::spawn_owned("tick-sleep-test", job);
    let parked = wait_until(SLEEP / 2, &installed);
    let woken = wait_until(200, &|| WOKE_AT.load(Ordering::Acquire) != 0);
    if !woken {
        // Still parked (or never ran): do not wait for it.
        let _ = kthread::detach(handle);
        return (parked, false, false, false, 0);
    }
    let _ = kthread::join(&handle);
    let woke_at = WOKE_AT.load(Ordering::Acquire);
    let deadline = DEADLINE.load(Ordering::Acquire);
    let expired_at = SELFTEST_EXPIRED_AT.load(Ordering::Acquire);
    SELFTEST_TID.store(u64::MAX, Ordering::Release);
    // Early if the tick expired its token before the deadline (0: no expiry
    // seen, so not woken by the tick) or it resumed before the deadline.
    let woken_early = expired_at < deadline || woke_at < deadline;
    let slept = woke_at.saturating_sub(START.load(Ordering::Acquire));
    // A deadline already passed: returns on this thread without parking.
    let past_skips = park_until(ticks_now()) == Park::Due;
    (parked, woken_early, woken, past_skips, slept)
}
