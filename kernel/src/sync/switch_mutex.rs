// SPDX-License-Identifier: MPL-2.0
//! Spin mutex with an explicit release token for stack-switch unlocks.
//!
//! `SwitchMutex` intentionally mirrors the small subset of `spin::Mutex`
//! semantics the scheduler and IPC channel need, while keeping ownership of
//! the lock byte inside the kernel. Q6's asm release helper must not depend
//! on private layout inside an external crate.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "kernel-diagnostic")]
use crate::arch::x86_64::per_cpu;
#[cfg(feature = "kernel-diagnostic")]
use crate::sync::lock_owner;
use crate::sync::lock_owner::{LockOwner, LockOwnerSnapshot};

/// Lightweight debug-build lock-order assertion (Phase 7 BUILD-P4a, §9.12
/// discriminator #3). Ordered `SwitchMutex`es declare a level via
/// [`SwitchMutex::new_ordered`]; an acquire whose level is ≤ the highest level
/// the **current thread** already holds panics *at the offending acquire* — a
/// lock-order inversion becomes a localized panic, not a mystery hang.
///
/// The documented global order (extends `admission.rs`'s
/// `QUEUES → TABLE → scheduler`):
///
/// | level | lock |
/// |-------|------|
/// | 10 | admission `QUEUES` |
/// | 15 | a tensor-owning slot (e.g. the reservation gate test slot) |
/// | 20 | `ai::reservation` `RESERVATIONS` |
/// | 22 | accounting `INFLIGHT` |
/// | 25 | accounting `TABLE` |
/// | 28 | accounting `RECENCY` | (levels are 1..=31 — the held-set is a u32 bitmask)
///
/// `TABLE` and `RECENCY` are leaf-like (never nested within each other); the
/// scheduler's internal lock is acquired after any of these (parking releases
/// the caller's guard atomically) and stays unordered. Unordered mutexes
/// (`order == 0`) are exempt — the assertion scopes to the AI-residency locks,
/// which never run in interrupt context.
///
/// Tracking is **per-thread** (not per-CPU): a preempted holder's entry belongs
/// to its thread id and cannot false-positive a different thread that runs on
/// the same CPU. The table is fixed-size and lock-free; if it fills, tracking
/// degrades to skip (best-effort diagnostic, never a correctness gate).
#[cfg(debug_assertions)]
mod lockdep {
    use core::sync::atomic::{AtomicU64, Ordering};

    /// Each slot packs `thread_id (high 32) | held-level bitmask (low 32)`.
    /// A slot is owned by exactly one live thread; only that thread mutates
    /// its mask (plain stores), other threads only read or CAS-claim empties.
    static HELD: [AtomicU64; 64] = [const { AtomicU64::new(0) }; 64];

    fn thread_key() -> u64 {
        // Truncation is fine for diagnostics: thread ids are small.
        (crate::arch::x86_64::per_cpu::current_thread_id() as u32 as u64) << 32
    }

    /// Returns the acquirer's thread key. The mutex stores it and the release
    /// clears **by that key**: the context-switch release path runs after the
    /// scheduler has already advanced `current_thread_id` to the next thread,
    /// so a current-identity release would clear the wrong thread's bit and
    /// leave the parked acquirer's bit stale - the gate's second run caught
    /// exactly that as a false inversion on wake.
    #[track_caller]
    pub fn acquire(level: u8) -> u64 {
        debug_assert!((1..=31).contains(&level));
        let key = thread_key();
        let bit = 1u32 << level;
        for slot in &HELD {
            let cur = slot.load(Ordering::Acquire);
            if cur != 0 && (cur & 0xffff_ffff_0000_0000) == key {
                let mask = cur as u32;
                if mask != 0 {
                    let highest = 31 - mask.leading_zeros();
                    assert!(
                        u32::from(level) > highest,
                        "lock-order inversion: acquiring level {} while holding level {} (held mask {:#x})",
                        level,
                        highest,
                        mask,
                    );
                }
                slot.store(cur | u64::from(bit), Ordering::Release);
                return key;
            }
        }
        for slot in &HELD {
            if slot
                .compare_exchange(0, key | u64::from(bit), Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return key;
            }
        }
        // Table full: degrade to skip — diagnostics, never a correctness gate.
        key
    }

    /// Clear `level` for the thread identified by `key` (the value `acquire`
    /// returned - NOT the current thread, which may already be the switched-to
    /// thread on the context-switch release path). Idempotent, so it is safe
    /// from both the guard `Drop` and the token-release path.
    pub fn release_key(key: u64, level: u8) {
        let bit = 1u32 << level;
        for slot in &HELD {
            let cur = slot.load(Ordering::Acquire);
            if cur != 0 && (cur & 0xffff_ffff_0000_0000) == key {
                let mask = (cur as u32) & !bit;
                let next = if mask == 0 { 0 } else { key | u64::from(mask) };
                slot.store(next, Ordering::Release);
                return;
            }
        }
    }
}

pub struct SwitchMutex<T> {
    lock: AtomicBool,
    owner: LockOwner,
    /// Lock-order level for the debug lockdep (0 = unordered/exempt).
    order: u8,
    /// The lockdep key of the live acquirer (debug only): release clears by
    /// this stored key, never by current-thread identity (see lockdep docs).
    #[cfg(debug_assertions)]
    lockdep_owner: core::sync::atomic::AtomicU64,
    inner: UnsafeCell<T>,
}

pub struct SwitchMutexGuard<'a, T> {
    mutex: &'a SwitchMutex<T>,
    _not_send: PhantomData<*const ()>,
}

// SAFETY: moving the mutex transfers ownership of the protected value. Access
// to `inner` still requires acquiring `lock`.
unsafe impl<T: Send> Send for SwitchMutex<T> {}

// SAFETY: shared references serialize mutable access through the AtomicBool
// lock; guarded access requires `T: Send` because values may be accessed from
// different CPUs over the mutex's lifetime.
unsafe impl<T: Send> Sync for SwitchMutex<T> {}

impl<T> SwitchMutex<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            owner: LockOwner::new(),
            order: 0,
            #[cfg(debug_assertions)]
            lockdep_owner: core::sync::atomic::AtomicU64::new(0),
            inner: UnsafeCell::new(value),
        }
    }

    /// A mutex participating in the documented global lock order (BUILD-P4a,
    /// §9.12): debug builds panic at any acquire whose `order` is not strictly
    /// greater than every order the current thread already holds. `order` must
    /// be 1..=31; 0 (via [`SwitchMutex::new`]) is unordered/exempt.
    #[allow(dead_code)]
    #[inline(always)]
    pub const fn new_ordered(value: T, order: u8) -> Self {
        Self {
            lock: AtomicBool::new(false),
            owner: LockOwner::new(),
            order,
            #[cfg(debug_assertions)]
            lockdep_owner: core::sync::atomic::AtomicU64::new(0),
            inner: UnsafeCell::new(value),
        }
    }

    #[track_caller]
    #[inline(always)]
    pub fn lock(&self) -> SwitchMutexGuard<'_, T> {
        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.lock.load(Ordering::Relaxed) {
                spin_loop();
            }
        }
        #[cfg(debug_assertions)]
        if self.order != 0 {
            let key = lockdep::acquire(self.order);
            self.lockdep_owner
                .store(key, core::sync::atomic::Ordering::Relaxed);
        }
        #[cfg(feature = "kernel-diagnostic")]
        self.record_owner_acquire();
        SwitchMutexGuard {
            mutex: self,
            _not_send: PhantomData,
        }
    }

    #[allow(dead_code)]
    #[track_caller]
    #[inline(always)]
    pub fn try_lock(&self) -> Option<SwitchMutexGuard<'_, T>> {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            #[cfg(debug_assertions)]
            if self.order != 0 {
                let key = lockdep::acquire(self.order);
                self.lockdep_owner
                    .store(key, core::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(feature = "kernel-diagnostic")]
            self.record_owner_acquire();
            Some(SwitchMutexGuard {
                mutex: self,
                _not_send: PhantomData,
            })
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn owner_snapshot(&self) -> Option<LockOwnerSnapshot> {
        self.owner.read_consistent()
    }

    #[cfg(feature = "kernel-diagnostic")]
    #[track_caller]
    #[inline(always)]
    fn record_owner_acquire(&self) {
        const SITE: u32 = crate::lock_site!();
        self.owner
            .acquire(per_cpu::current_cpu_id(), current_thread_id_u32(), SITE);
    }
}

impl<'a, T> SwitchMutexGuard<'a, T> {
    #[allow(dead_code)]
    pub fn release_token(&self) -> *const AtomicBool {
        &self.mutex.lock
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn release_token_for_context_switch(&mut self) -> *const AtomicBool {
        #[cfg(debug_assertions)]
        if self.mutex.order != 0 {
            lockdep::release_key(
                self.mutex
                    .lockdep_owner
                    .load(core::sync::atomic::Ordering::Relaxed),
                self.mutex.order,
            );
        }
        #[cfg(feature = "kernel-diagnostic")]
        self.mutex.owner.release();
        &self.mutex.lock
    }
}

impl<T> Deref for SwitchMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: this guard exists only after acquiring the lock. The guard
        // is !Send, and SwitchMutex permits shared access to `inner` only
        // while exactly one guard is live.
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<T> DerefMut for SwitchMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: this mutable reference is tied to the unique guard that
        // holds the lock. No other reference to `inner` can be produced until
        // the guard releases the lock.
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<T> Drop for SwitchMutexGuard<'_, T> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if self.mutex.order != 0 {
            lockdep::release_key(
                self.mutex
                    .lockdep_owner
                    .load(core::sync::atomic::Ordering::Relaxed),
                self.mutex.order,
            );
        }
        #[cfg(feature = "kernel-diagnostic")]
        self.mutex.owner.release();
        self.mutex.lock.store(false, Ordering::Release);
    }
}

#[cfg(feature = "kernel-diagnostic")]
fn current_thread_id_u32() -> u32 {
    per_cpu::current_thread_id()
        .try_into()
        .unwrap_or(lock_owner::UNOWNED)
}
