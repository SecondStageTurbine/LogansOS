// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.3: `std::sys::thread::neoplutarch`, std's thread backend on the C1
//! thread syscalls.
//!
//! Overlaid into `library/std/src/sys/thread/neoplutarch.rs` by
//! `apply-overlay.py`. Every syscall goes through `sys::neoplutarch_rt` (the
//! single trampoline); this file holds none.
//!
//! **Spawn.** [`Thread::new`] first makes sure the spawning process is on the
//! FS.base key convention (`tls::ensure_initial_installed`; without the C1
//! capability that fails with `EPERM`, and so would the create), then creates
//! the thread on a kernel-allocated stack with a guard page. The new thread
//! starts in [`thread_start`], which installs its control block at its own
//! stack base and points FS.base at it BEFORE any std code runs, so the
//! thread's first thread-local access (`ThreadInit::init` sets the current
//! thread handle) already finds its own keys.
//!
//! **Exit.** When the Rust start routine returns, [`thread_start`] runs the TLS
//! key destructors (std's own thread-local destructors and `thread_cleanup` hang
//! off one of its keys) and frees the key block, then `SYS_THREAD_EXIT(0)`. The
//! result travels through std's own packet, not the exit code.
//!
//! **Join/detach.** [`Thread::join`] is `SYS_THREAD_JOIN`; dropping an unjoined
//! [`Thread`] detaches it (`SYS_THREAD_DETACH`), as pthread-based std does.
//!
//! **Not provided (stated, not faked).** `available_parallelism` stays an
//! error (no CPU-count syscall in the C1 manifest), `current_os_id` is `None`
//! and `set_name` keeps the name in std only (the C1 manifest has no set-name
//! manifest). `sleep` waits on a private futex word with a monotonic deadline.

use crate::ffi::CStr;
use crate::io;
use crate::num::NonZero;
use crate::ptr;
use crate::sync::atomic::AtomicU32;
use crate::sys::neoplutarch_rt::{self as rt, tls};
use crate::thread::ThreadInit;
use crate::time::Duration;

/// std's default thread stack (`RUST_MIN_STACK` is not read: no environment).
pub const DEFAULT_MIN_STACK_SIZE: usize = 256 * 1024;

const PAGE_SIZE: usize = 4096;

pub struct Thread {
    tid: u64,
}

// SAFETY: a tid is a plain number; the kernel serializes join/detach on it.
unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    // unsafe: see thread::Builder::spawn_unchecked for safety requirements
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        if let Err(e) = tls::ensure_initial_installed() {
            return Err(io::Error::from_raw_os_error(-e as i32));
        }
        let stack = stack.max(PAGE_SIZE).next_multiple_of(PAGE_SIZE);
        let data = Box::into_raw(init);
        let r = rt::thread_create(thread_start, data.expose_provenance(), stack);
        if r < 0 {
            // SAFETY: the thread was not created, so `data` was not consumed.
            drop(unsafe { Box::from_raw(data) });
            return Err(io::Error::from_raw_os_error(-r as i32));
        }
        Ok(Thread { tid: r as u64 })
    }

    pub fn join(self) {
        let tid = self.tid;
        crate::mem::forget(self); // joined, so never detached
        if let Err(e) = rt::thread_join(tid) {
            rtabort!("thread join failed: LogansOS errno {}", -e);
        }
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        let _ = rt::thread_detach(self.tid);
    }
}

/// The entry of every std-spawned thread. `data` is the `Box<ThreadInit>`
/// `Thread::new` leaked; ownership transfers here.
extern "C" fn thread_start(data: usize) -> ! {
    // Before any std code: this thread's control block, at its stack base.
    let Some((low, len)) = rt::stack_bounds() else {
        rtabort!("thread start: no stack bounds");
    };
    // SAFETY: our own fresh stack's base; RSP is at the top, nothing lives
    // at the base yet.
    if unsafe { tls::install_at(low as *mut tls::ThreadControlBlock, low, len) }.is_err() {
        rtabort!("thread start: SYS_THREAD_SET_TLS refused");
    }
    // SAFETY: recreating the box `Thread::new` leaked; this is its only consumer.
    let init = unsafe { Box::from_raw(ptr::with_exposed_provenance_mut::<ThreadInit>(data)) };
    let rust_start = init.init();
    rust_start();
    tls::thread_exit_cleanup();
    rt::thread_exit(0)
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    Err(io::Error::UNKNOWN_THREAD_COUNT)
}

pub fn current_os_id() -> Option<u64> {
    None
}

pub fn set_name(_name: &CStr) {
    // The name stays in std's `Thread` handle; the C1 manifest has no set-name
    // syscall.
}

pub fn yield_now() {
    rt::sched_yield();
}

/// Sleep at least `dur`: wait on a word nobody wakes until the monotonic
/// deadline, re-waiting after a spurious wake.
pub fn sleep(dur: Duration) {
    let Ok(start) = rt::monotonic_ns() else {
        panic!("can't sleep: no monotonic clock");
    };
    let deadline = u64::try_from(dur.as_nanos())
        .ok()
        .and_then(|ns| start.checked_add(ns))
        .unwrap_or(rt::DEADLINE_INFINITE);
    let word = AtomicU32::new(0);
    loop {
        let r = rt::futex_wait_until(word.as_ptr(), 0, deadline);
        if r == rt::ETIMEDOUT {
            return;
        }
        if r < 0 && r != rt::EAGAIN {
            panic!("can't sleep: SYS_FUTEX_WAIT errno {}", -r);
        }
        if rt::monotonic_ns().is_ok_and(|now| now >= deadline) {
            return;
        }
        // A spurious wake: wait again (the word is private and stays 0).
    }
}
