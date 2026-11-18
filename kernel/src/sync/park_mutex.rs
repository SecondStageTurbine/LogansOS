// SPDX-License-Identifier: MPL-2.0
//! OT-101: a mutex whose waiters park instead of spinning.
//!
//! `SwitchMutex` spins: a holder that waits on a device (a block read, a
//! drain) keeps every other acquirer spinning for the whole wait, and a
//! preempted holder strands them (CP-63). A `ParkMutex` acquirer that finds
//! the lock taken parks in the scheduler's token table and is woken by the
//! unlock, so the CPU runs something else meanwhile.
//!
//! Only thread context may acquire one: the slow path parks. The holder may
//! park too (a device wait under the lock is the point). Never acquire one in
//! an interrupt handler, or while holding a spinning lock, an `IrqSafeMutex`
//! guard, or anything a parked holder would need back.
//!
//! **Lost wakeup (CP-14).** The waiter re-checks the lock under the waiter
//! queue's guard and parks through `try_block_current_with_token_and_guard`,
//! which installs its token and releases that guard atomically with the
//! context switch. The unlock clears the lock and pops a token under the
//! same guard. So an unlock either runs before the waiter's re-check (which
//! then takes the lock) or finds the waiter's token (and wakes it). There is
//! no instant where a waiter has committed to sleep, holds no guard and is
//! still on the CPU (the futex's discipline, `sched/futex.rs`).
//!
//! **Lock order (CP-1, CP-16).** Waiter queue guard → scheduler lock, on the
//! park. The unlock pops under the queue guard and wakes after dropping it.
//!
//! **Fairness.** A woken waiter competes with new acquirers (it can lose to a
//! fast-path acquire); one that loses parks again at the front of the queue,
//! so the oldest waiter is the next one woken.
//!
//! **Kill (CP-30).** A waiter's frames own kernel state (whatever its caller
//! holds), so it is marked `park_owns_state` while it waits: a force-kill
//! scan leaves it, it is woken by an unlock (the holder always finishes), and
//! it exits at its syscall boundary. A thread that must not park (OT-93:
//! its process is being killed) does not park; it yields and retries.
//!
//! **Token slots.** A full per-CPU token table (16 slots) makes the park
//! fail; the waiter yields and retries, which still lets the holder run.

use alloc::collections::VecDeque;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use super::IrqSafeMutex;
use crate::arch::x86_64::without_interrupts;
use crate::sched::{self, ThreadWakeToken, WakeTokenResult};

pub struct ParkMutex<T> {
    locked: AtomicBool,
    waiters: IrqSafeMutex<VecDeque<ThreadWakeToken>>,
    /// The scenario's counters and its runtime revert (`ParkProbe`).
    #[cfg(feature = "neo-test")]
    probe: Option<&'static ParkProbe>,
    inner: UnsafeCell<T>,
}

pub struct ParkMutexGuard<'a, T> {
    mutex: &'a ParkMutex<T>,
    _not_send: PhantomData<*const ()>,
}

// SAFETY: moving the mutex moves the protected value; access to `inner`
// still needs the lock.
unsafe impl<T: Send> Send for ParkMutex<T> {}

// SAFETY: `locked` serializes access to `inner`; `T: Send` because the value
// is reached from different CPUs over the mutex's lifetime.
unsafe impl<T: Send> Sync for ParkMutex<T> {}

/// OT-101's scenario: what one mutex's acquirers did, and a runtime switch
/// back to spinning (the fix reverted) for the control leg.
#[cfg(feature = "neo-test")]
pub struct ParkProbe {
    /// Set while the mutex is held.
    pub held: AtomicBool,
    /// Waits that parked (each counts once per park).
    pub parks: core::sync::atomic::AtomicU64,
    /// Rounds an acquirer spun on a held lock (only in the reverted mode).
    pub spin_rounds: core::sync::atomic::AtomicU64,
    /// The reverted mode: acquire by spinning, as `SwitchMutex` does.
    pub spin_instead: AtomicBool,
}

#[cfg(feature = "neo-test")]
impl ParkProbe {
    pub const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            parks: core::sync::atomic::AtomicU64::new(0),
            spin_rounds: core::sync::atomic::AtomicU64::new(0),
            spin_instead: AtomicBool::new(false),
        }
    }
}

impl<T> ParkMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            waiters: IrqSafeMutex::new(VecDeque::new()),
            #[cfg(feature = "neo-test")]
            probe: None,
            inner: UnsafeCell::new(value),
        }
    }

    /// As `new`, reporting to `probe` (test builds).
    #[cfg(feature = "neo-test")]
    pub const fn new_probed(value: T, probe: &'static ParkProbe) -> Self {
        Self {
            locked: AtomicBool::new(false),
            waiters: IrqSafeMutex::new(VecDeque::new()),
            probe: Some(probe),
            inner: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> ParkMutexGuard<'_, T> {
        #[cfg(feature = "neo-test")]
        if let Some(probe) = self.probe
            && probe.spin_instead.load(Ordering::SeqCst)
        {
            while !self.try_acquire() {
                probe.spin_rounds.fetch_add(1, Ordering::Relaxed);
                core::hint::spin_loop();
            }
            return self.acquired();
        }
        if !self.try_acquire() {
            self.lock_slow();
        }
        self.acquired()
    }

    pub fn try_lock(&self) -> Option<ParkMutexGuard<'_, T>> {
        self.try_acquire().then(|| self.acquired())
    }

    fn try_acquire(&self) -> bool {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    fn acquired(&self) -> ParkMutexGuard<'_, T> {
        #[cfg(feature = "neo-test")]
        if let Some(probe) = self.probe {
            probe.held.store(true, Ordering::SeqCst);
        }
        ParkMutexGuard {
            mutex: self,
            _not_send: PhantomData,
        }
    }

    #[cold]
    fn lock_slow(&self) {
        enum Step {
            Acquired,
            Woken,
            Retry,
        }
        sched::set_current_park_owns_state(true);
        let mut woken_before = false;
        loop {
            let step = without_interrupts(|| {
                let waiters = self.waiters.lock();
                if self.try_acquire() {
                    return Step::Acquired;
                }
                let front = woken_before;
                match sched::try_block_current_with_token_and_guard(waiters, |w, token| {
                    if front {
                        w.push_front(token);
                    } else {
                        w.push_back(token);
                    }
                }) {
                    // Woken by an unlock (it passes 0).
                    Ok(ret) if ret >= 0 => Step::Woken,
                    // OT-93: this thread must not park (its process is being
                    // killed); nothing was installed.
                    Ok(_) => Step::Retry,
                    // The token table is full; nothing was installed.
                    Err(waiters) => {
                        drop(waiters);
                        Step::Retry
                    }
                }
            });
            match step {
                Step::Acquired => break,
                Step::Woken => {
                    woken_before = true;
                    #[cfg(feature = "neo-test")]
                    if let Some(probe) = self.probe {
                        probe.parks.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Step::Retry => sched::yield_now(),
            }
        }
        sched::set_current_park_owns_state(false);
    }

    fn unlock(&self) {
        #[cfg(feature = "neo-test")]
        if let Some(probe) = self.probe {
            probe.held.store(false, Ordering::SeqCst);
        }
        let mut next = without_interrupts(|| {
            let mut waiters = self.waiters.lock();
            self.locked.store(false, Ordering::Release);
            waiters.pop_front()
        });
        // Wake one waiter, after the queue guard is dropped (CP-16). A token
        // whose thread is no longer parked (stale or gone) wakes nobody, so
        // the next one is woken instead: a free lock never leaves a waiter
        // parked.
        while let Some(token) = next {
            if sched::wake_thread_token(token, 0) == WakeTokenResult::Woken {
                return;
            }
            next = without_interrupts(|| self.waiters.lock().pop_front());
        }
    }
}

impl<T> Deref for ParkMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard proves the lock is held.
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<T> DerefMut for ParkMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard proves the lock is held, and `&mut self` makes
        // this the only reference through it.
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<T> Drop for ParkMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}
