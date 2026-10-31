// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A thread parker for LogansOS (`target_os = "neoplutarch"`), built on std's
//! `Mutex` and `Condvar`. On this target those are futex-based (the kernel's
//! `SYS_FUTEX_WAIT`/`SYS_FUTEX_WAKE`), so a parked thread blocks in the kernel
//! instead of spinning. The shape follows `unix.rs`: `unpark_lock` takes the
//! mutex and holds it until `unpark` has cleared the flag and signalled, so the
//! parked thread (and its `ThreadData`) cannot go away in between.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Instant;

pub struct ThreadParker {
    should_park: Mutex<bool>,
    condvar: Condvar,
}

impl ThreadParker {
    #[inline]
    fn lock(&self) -> MutexGuard<'_, bool> {
        // panic=abort on this target, so poisoning cannot be observed; take
        // the guard either way.
        self.should_park.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl super::ThreadParkerT for ThreadParker {
    type UnparkHandle = UnparkHandle;

    // `Mutex::new` and `Condvar::new` are const and make no syscall.
    const IS_CHEAP_TO_CONSTRUCT: bool = true;

    #[inline]
    fn new() -> ThreadParker {
        ThreadParker {
            should_park: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    #[inline]
    unsafe fn prepare_park(&self) {
        *self.lock() = true;
    }

    #[inline]
    unsafe fn timed_out(&self) -> bool {
        *self.lock()
    }

    #[inline]
    unsafe fn park(&self) {
        let mut guard = self.lock();
        while *guard {
            guard = self
                .condvar
                .wait(guard)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    #[inline]
    unsafe fn park_until(&self, timeout: Instant) -> bool {
        let mut guard = self.lock();
        while *guard {
            let now = Instant::now();
            if now >= timeout {
                return false;
            }
            guard = self
                .condvar
                .wait_timeout(guard, timeout - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }

    #[inline]
    unsafe fn unpark_lock(&self) -> UnparkHandle {
        let guard = self.lock();
        UnparkHandle {
            // SAFETY: the guard is dropped in `unpark`, on this thread, before
            // the parked thread can return from `park*` (it needs the mutex to
            // see the cleared flag), so the parker outlives the guard.
            guard: unsafe { std::mem::transmute::<MutexGuard<'_, bool>, MutexGuard<'static, bool>>(guard) },
            condvar: &self.condvar,
        }
    }
}

pub struct UnparkHandle {
    guard: MutexGuard<'static, bool>,
    condvar: *const Condvar,
}

impl super::UnparkHandleT for UnparkHandle {
    #[inline]
    unsafe fn unpark(mut self) {
        *self.guard = false;
        // SAFETY: the mutex is still held, so the parker is alive (see
        // `unpark_lock`). Signal before unlocking, as `unix.rs` does.
        unsafe { (*self.condvar).notify_one() };
        drop(self.guard);
    }
}

#[inline]
pub fn thread_yield() {
    thread::yield_now();
}
