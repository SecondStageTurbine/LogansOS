// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.1: userland synchronization over the two kernel futex ops —
//! the frozen convergence (`RATIFY-2026-07-22-c1-abi-amended-freeze.md`):
//! **futex → {mutex, condvar, rwlock}, all userland**, no separate sync
//! syscalls. These are the no_std layers the C1 batteries exercise; the
//! real-`std`/pthread bindings ride the C0.5 overlay at C2/C3 and bind the
//! same two ops.
//!
//! Algorithms are the standard futex shapes (three-state mutex; sequence
//! condvar; reader-count/writer-bit rwlock whose unlock path CONSUMES the
//! `futex_wake` return count — the preserved-wake-count amendment).

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    CLOCK_MONOTONIC, FUTEX_DEADLINE_INFINITE, sys_clock_gettime, sys_futex_wait, sys_futex_wake,
};

const ETIMEDOUT: i64 = -110;

/// Convert a relative timeout into the ABSOLUTE monotonic deadline the
/// kernel takes — computed ONCE, so spurious re-waits do not restart the
/// clock (the pthread_cond_timedwait discipline the freeze mandates).
pub fn deadline_after_ns(rel_ns: u64) -> u64 {
    match sys_clock_gettime(CLOCK_MONOTONIC) {
        Ok(now) => now.saturating_add(rel_ns),
        // A broken clock degrades to "no deadline" rather than a spurious
        // instant timeout; the batteries assert the clock separately.
        Err(_) => FUTEX_DEADLINE_INFINITE,
    }
}

// ── Mutex ──────────────────────────────────────────────────────────────────

const M_UNLOCKED: u32 = 0;
const M_LOCKED: u32 = 1;
const M_CONTENDED: u32 = 2;

/// The three-state futex mutex (0 = unlocked, 1 = locked/no-waiters,
/// 2 = locked/maybe-waiters): uncontended lock/unlock is a single CAS with
/// no syscall; the contended path parks on the state word.
pub struct Mutex {
    state: AtomicU32,
}

impl Mutex {
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(M_UNLOCKED),
        }
    }

    fn state_ptr(&self) -> *const u32 {
        self.state.as_ptr() as *const u32
    }

    pub fn lock(&self) {
        if self
            .state
            .compare_exchange(M_UNLOCKED, M_LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
        self.lock_contended();
    }

    fn lock_contended(&self) {
        loop {
            // Advertise contention, then park while the word still reads
            // CONTENDED. The value re-check inside futex_wait (under the
            // kernel queue guard) closes the check-then-park window.
            let prev = self.state.swap(M_CONTENDED, Ordering::Acquire);
            if prev == M_UNLOCKED {
                return; // we took the lock (word now CONTENDED — unlock wakes)
            }
            let _ = sys_futex_wait(self.state_ptr(), M_CONTENDED, FUTEX_DEADLINE_INFINITE);
        }
    }

    pub fn unlock(&self) {
        if self.state.swap(M_UNLOCKED, Ordering::Release) == M_CONTENDED {
            let _ = sys_futex_wake(self.state_ptr(), 1);
        }
    }

    /// Run `f` under the lock.
    pub fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        self.lock();
        let r = f();
        self.unlock();
        r
    }
}

impl Default for Mutex {
    fn default() -> Self {
        Self::new()
    }
}

// ── Condvar ────────────────────────────────────────────────────────────────

/// Sequence-counter condvar. `wait` snapshots the sequence UNDER the mutex,
/// releases it, parks while the sequence is unchanged, then reacquires —
/// a notify between unlock and park bumps the sequence, so the park refuses
/// (`EAGAIN`) and the wakeup is never lost.
pub struct Condvar {
    seq: AtomicU32,
}

impl Condvar {
    pub const fn new() -> Self {
        Self {
            seq: AtomicU32::new(0),
        }
    }

    fn seq_ptr(&self) -> *const u32 {
        self.seq.as_ptr() as *const u32
    }

    /// Wait until notified or the ABSOLUTE monotonic `deadline_ns` passes.
    /// Returns `false` on timeout. Spurious returns are permitted — callers
    /// re-check their predicate under the mutex (both `std` and Mozilla's
    /// condvar consumers do exactly this).
    pub fn wait_deadline(&self, mutex: &Mutex, deadline_ns: u64) -> bool {
        let snapshot = self.seq.load(Ordering::Acquire);
        mutex.unlock();
        let woken = !matches!(
            sys_futex_wait(self.seq_ptr(), snapshot, deadline_ns),
            Err(ETIMEDOUT)
        );
        mutex.lock();
        woken
    }

    pub fn wait(&self, mutex: &Mutex) {
        let _ = self.wait_deadline(mutex, FUTEX_DEADLINE_INFINITE);
    }

    pub fn notify_one(&self) {
        self.seq.fetch_add(1, Ordering::Release);
        let _ = sys_futex_wake(self.seq_ptr(), 1);
    }

    pub fn notify_all(&self) {
        self.seq.fetch_add(1, Ordering::Release);
        let _ = sys_futex_wake(self.seq_ptr(), u32::MAX);
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

// ── RwLock ─────────────────────────────────────────────────────────────────

/// Writer-present bit; the low 31 bits count active readers.
const W_BIT: u32 = 1 << 31;

/// Futex rwlock: readers count in the state word, writers claim `W_BIT`.
/// The write-unlock path wakes ONE waiter first (a would-be writer) and
/// CONSUMES the `futex_wake` return count for the writer→reader fallback:
/// zero woken ⇒ no parked writer ⇒ wake ALL parked readers. This is the
/// preserved-wake-count consumer the P1 amendment freezes
/// (`std::sys::sync::rwlock::futex` does the same fallback dance).
pub struct RwLock {
    state: AtomicU32,
}

impl RwLock {
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
        }
    }

    fn state_ptr(&self) -> *const u32 {
        self.state.as_ptr() as *const u32
    }

    pub fn read_lock(&self) {
        loop {
            let s = self.state.load(Ordering::Relaxed);
            if s & W_BIT == 0 {
                if self
                    .state
                    .compare_exchange_weak(s, s + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return;
                }
                continue;
            }
            let _ = sys_futex_wait(self.state_ptr(), s, FUTEX_DEADLINE_INFINITE);
        }
    }

    pub fn read_unlock(&self) {
        let prev = self.state.fetch_sub(1, Ordering::Release);
        // Last reader out: a writer may be parked on the now-zero word.
        if prev == 1 {
            let _ = sys_futex_wake(self.state_ptr(), 1);
        }
    }

    pub fn write_lock(&self) {
        loop {
            let s = self.state.load(Ordering::Relaxed);
            if s == 0 {
                if self
                    .state
                    .compare_exchange_weak(0, W_BIT, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return;
                }
                continue;
            }
            let _ = sys_futex_wait(self.state_ptr(), s, FUTEX_DEADLINE_INFINITE);
        }
    }

    pub fn write_unlock(&self) {
        self.state.store(0, Ordering::Release);
        // Wake one (writer-preferring); if the count says NOBODY was parked
        // there, fall back to waking everyone (readers parked on the old
        // W_BIT value would otherwise strand). The wake-count return is
        // load-bearing here — this is the amendment's consumer.
        match sys_futex_wake(self.state_ptr(), 1) {
            Ok(0) | Err(_) => {
                let _ = sys_futex_wake(self.state_ptr(), u32::MAX);
            }
            Ok(_) => {}
        }
    }
}

impl Default for RwLock {
    fn default() -> Self {
        Self::new()
    }
}
