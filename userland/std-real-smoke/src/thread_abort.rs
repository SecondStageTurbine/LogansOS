// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.3 — a std worker thread's panic ends the WHOLE process
//! (`std_threads` scenario, third spawn).
//!
//! The target is `panic=abort`: a panic on any thread runs std's
//! `abort_internal` → `SYS_EXIT(134)`. With sibling threads alive, the kernel
//! must end the process (its exit group), not just the panicking thread. Before
//! that, the panicking thread alone exited and the main thread stayed parked in
//! `join` forever. Here the main thread joins the panicker while one sibling is
//! parked in a futex wait (a `Condvar` nobody signals) and another spins on
//! `yield_now`. Expected: exit 134, every thread gone. Any return from the join
//! exits 60.

#![no_main]
#![feature(restricted_std)]

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let pair = Arc::new((Mutex::new(()), Condvar::new()));
    let p = pair.clone();
    let _parked = thread::spawn(move || {
        let guard = p.0.lock().unwrap();
        // Never signalled: parked in SYS_FUTEX_WAIT until the process ends.
        let _woken = p.1.wait(guard);
    });
    let _spinner = thread::spawn(|| {
        loop {
            thread::yield_now();
        }
    });
    thread::sleep(Duration::from_millis(20));
    println!("[ std-thread-abort ] a worker panics; the whole process must exit 134");
    let panicker = thread::spawn(|| {
        panic!("deliberate worker panic (C1.3 exit-group check)");
    });
    let _ = panicker.join();
    println!("[ std-thread-abort ] FAIL(60): join returned; the panic did not end the process");
    std::process::exit(60);
}
