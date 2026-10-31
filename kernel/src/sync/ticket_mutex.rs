// SPDX-License-Identifier: MPL-2.0
//! A FIFO ticket spin lock (K1a-0, the `CAP_GRAPH` lock).
//!
//! Waiters are served in arrival order, so a lock that one rare operation
//! (a revoke) shares with a stream of frequent ones (derives, closes) cannot
//! starve the rare one: a waiter is admitted after at most the waiters that
//! took a ticket before it, one critical section each. `spin::Mutex` is a
//! test-and-set lock with no such bound.
//!
//! Not IRQ-safe by itself: callers hold it with interrupts off, and no
//! interrupt path takes it. It imports only `core`, so the host KAT crate
//! (`tools/cap-graph-logic-tests`) mounts it via `#[path]`.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

pub struct TicketMutex<T> {
    next: AtomicU32,
    serving: AtomicU32,
    value: UnsafeCell<T>,
}

// SAFETY: access to `value` is serialized by the ticket protocol.
unsafe impl<T: Send> Send for TicketMutex<T> {}
// SAFETY: as above; a guard is the only path to `&mut T`.
unsafe impl<T: Send> Sync for TicketMutex<T> {}

pub struct TicketMutexGuard<'a, T> {
    lock: &'a TicketMutex<T>,
    /// A guard hands out `&T` through `Deref`, so sharing the guard across
    /// threads shares `&T`: the guard may be `Sync` only when `T: Sync`
    /// (Janus F3). `&TicketMutex<T>` alone made it `Sync` for any `T: Send`.
    _shares_t: PhantomData<&'a T>,
}

impl<T> TicketMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            next: AtomicU32::new(0),
            serving: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    /// Take a ticket and spin until it is served.
    pub fn lock(&self) -> TicketMutexGuard<'_, T> {
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        while self.serving.load(Ordering::Acquire) != ticket {
            spin_loop();
        }
        TicketMutexGuard {
            lock: self,
            _shares_t: PhantomData,
        }
    }

    /// Tickets taken so far (the host KAT waits on it).
    #[cfg(test)]
    pub fn tickets_taken(&self) -> u32 {
        self.next.load(Ordering::SeqCst)
    }

    /// Take the lock only if it is free and nobody is queued. Used only by
    /// the `cap_graph_revoke` test hook (and the host KAT).
    #[allow(dead_code)]
    pub fn try_lock(&self) -> Option<TicketMutexGuard<'_, T>> {
        let serving = self.serving.load(Ordering::Acquire);
        self.next
            .compare_exchange(
                serving,
                serving.wrapping_add(1),
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .ok()
            .map(|_| TicketMutexGuard {
                lock: self,
                _shares_t: PhantomData,
            })
    }
}

impl<T> Deref for TicketMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: this guard holds the lock.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for TicketMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: this guard holds the lock exclusively.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for TicketMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Only the holder writes `serving`, so a plain increment is enough.
        let next = self.lock.serving.load(Ordering::Relaxed).wrapping_add(1);
        self.lock.serving.store(next, Ordering::Release);
    }
}
