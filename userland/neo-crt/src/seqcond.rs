// SPDX-License-Identifier: MPL-2.0
//! The sequence condvar behind `pthread_cond_t` (`pthread.rs`), std only, so
//! the host KATs in `tools/neo-crt-logic-tests` mount this exact file.
//!
//! A waiter records the sequence number, releases the caller's mutex, and
//! sleeps until the number changes. `signal` and `broadcast` bump the number
//! and wake **while still holding the sequence lock** (CP-106's neighbour in
//! the C3b table; Chad's review of #67, item 2). That ordering is the whole
//! correctness argument: a waiter can only record a sequence number, and so
//! only start waiting, while it holds the lock. So when `notify_one` runs,
//! every thread the underlying condvar can wake recorded the old number and
//! returns to its caller. If the lock were dropped before the wake (the
//! first version did that), a thread arriving in the gap could record the
//! new number, park, and be the one woken: it sees no change and sleeps
//! again, and the waiter the signal was for sleeps on (our kernel's futex
//! wakes in queue-slot order, `kernel/src/sched/futex.rs`, so the late
//! waiter can be first in line).

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// What `SeqCond` needs from a condvar: std's in production, a model in the
/// host KATs (one that wakes in a chosen order).
pub trait Cv {
    fn wait<'a>(&self, m: &'a Mutex<u64>, g: MutexGuard<'a, u64>) -> MutexGuard<'a, u64>;
    fn wait_timeout<'a>(
        &self,
        m: &'a Mutex<u64>,
        g: MutexGuard<'a, u64>,
        d: Duration,
    ) -> MutexGuard<'a, u64>;
    fn notify_one(&self);
    fn notify_all(&self);
}

impl Cv for Condvar {
    fn wait<'a>(&self, _m: &'a Mutex<u64>, g: MutexGuard<'a, u64>) -> MutexGuard<'a, u64> {
        Condvar::wait(self, g).unwrap_or_else(|e| e.into_inner())
    }

    fn wait_timeout<'a>(
        &self,
        _m: &'a Mutex<u64>,
        g: MutexGuard<'a, u64>,
        d: Duration,
    ) -> MutexGuard<'a, u64> {
        Condvar::wait_timeout(self, g, d)
            .unwrap_or_else(|e| e.into_inner())
            .0
    }

    fn notify_one(&self) {
        Condvar::notify_one(self)
    }

    fn notify_all(&self) {
        Condvar::notify_all(self)
    }
}

pub struct SeqCond<C> {
    pub seq: Mutex<u64>,
    pub cv: C,
}

fn lock(m: &Mutex<u64>) -> MutexGuard<'_, u64> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl<C: Cv> SeqCond<C> {
    pub const fn new(cv: C) -> Self {
        SeqCond {
            seq: Mutex::new(0),
            cv,
        }
    }

    /// Record the sequence number, then call `release` (it unlocks the
    /// caller's mutex) with the sequence lock still held, then sleep until
    /// the number changes or `until` passes. `Ok(true)` if it timed out.
    /// The caller re-locks its own mutex afterwards.
    pub fn wait<E>(
        &self,
        release: impl FnOnce() -> Result<(), E>,
        until: Option<Instant>,
    ) -> Result<bool, E> {
        let mut g = lock(&self.seq);
        let seen = *g;
        release()?;
        while *g == seen {
            g = match until {
                None => self.cv.wait(&self.seq, g),
                Some(t) => {
                    let now = Instant::now();
                    if now >= t {
                        return Ok(true);
                    }
                    self.cv.wait_timeout(&self.seq, g, t - now)
                }
            };
        }
        Ok(false)
    }

    /// Wake at least one thread waiting at the time of the call.
    pub fn signal(&self) {
        let mut g = lock(&self.seq);
        *g = g.wrapping_add(1);
        self.cv.notify_one();
        drop(g);
    }

    /// Wake every thread waiting at the time of the call.
    pub fn broadcast(&self) {
        let mut g = lock(&self.seq);
        *g = g.wrapping_add(1);
        self.cv.notify_all();
        drop(g);
    }
}
