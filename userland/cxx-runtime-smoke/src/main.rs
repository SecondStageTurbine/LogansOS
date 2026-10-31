// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3a: `cxx-runtime-smoke`, the browser bundle's private C/C++ runtime
//! (`userland/neo-crt`) on-target
//! (`docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md`).
//!
//! The C checks live in `c/crt_checks.c`, each against an independent
//! oracle (see that file's header), and the C++ ones in `c/cxx_checks.cpp`.
//! This binary runs them in order, then runs three on several threads at
//! once (and, from C3b, the pthread and libc++ thread checks in
//! `c/pthread_checks.c` and `c/cxx_thread_checks.cpp`): `errno` must be per thread, the allocator must hold up under
//! concurrent use, and a function-local static must be built exactly once
//! while every thread waits for it.
//!
//! Exit 0 when every check passes. A failed check prints `FAIL(n)` and exits
//! `n` (1..99).

#![no_main]

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ crt-smoke ]";
const THREADS: i32 = 6;

unsafe extern "C" {
    fn crt_check_format() -> i32;
    fn crt_check_scan() -> i32;
    fn crt_check_math() -> i32;
    fn crt_check_strings() -> i32;
    fn crt_check_alloc() -> i32;
    fn crt_check_clock() -> i32;
    fn crt_check_stdout() -> i32;
    fn crt_errno_probe(id: i32) -> i32;
    fn crt_malloc_stress(id: i32) -> i32;
    fn cxx_check_globals() -> i32;
    fn cxx_check_containers() -> i32;
    fn cxx_check_strings() -> i32;
    fn cxx_check_objects() -> i32;
    fn cxx_check_clock() -> i32;
    fn cxx_static_guard_probe(id: i32) -> i32;
    fn cxx_static_guard_builds() -> i32;
    fn crt_stdout_marker() -> i32;
    fn crt_stdio_hammer(id: i32) -> i32;
    fn crt_stdio_end_runs();
    // musl's stdout, and neo-crt's stream lock (CP-100).
    static stdout: *mut c_void;
    fn __lockfile(f: *mut c_void) -> i32;
    fn __unlockfile(f: *mut c_void);
    fn pt_check_threads() -> i32;
    fn pt_check_mutex_kinds() -> i32;
    fn pt_check_condvar() -> i32;
    fn pt_check_keys() -> i32;
    fn pt_check_once() -> i32;
    fn pt_check_rwlock() -> i32;
    fn cxx_check_threads() -> i32;
    fn cxx_check_condition_variable() -> i32;
    fn cxx_check_sync() -> i32;
}

fn check(n: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({n}) {what}");
    } else {
        println!("{TAG} FAIL({n}) {what}");
        std::process::exit(n);
    }
}

/// A C check: 0 is a pass; otherwise the C side has printed which item failed.
fn c_check(n: i32, what: &str, f: unsafe extern "C" fn() -> i32) {
    let item = unsafe { f() };
    if item != 0 {
        println!("{TAG} (failed item c{item})");
    }
    check(n, what, item == 0);
}

fn on_threads(probe: unsafe extern "C" fn(i32) -> i32) -> bool {
    let handles: Vec<_> = (0..THREADS)
        .map(|id| thread::spawn(move || unsafe { probe(id) } == 1))
        .collect();
    handles.into_iter().all(|h| h.join().unwrap_or(false))
}

/// CP-100: while this thread holds stdout's lock, a second thread's `fputs`
/// to stdout must wait, and must finish once the lock is let go; and the
/// holder taking it again is the nested case (0, nothing to release).
fn stdout_lock_excludes() -> bool {
    let f = unsafe { stdout };
    if unsafe { __lockfile(f) } != 1 {
        return false;
    }
    let nested = unsafe { __lockfile(f) } == 0;
    let done = Arc::new(AtomicBool::new(false));
    let seen = done.clone();
    let writer = thread::spawn(move || {
        let ok = unsafe { crt_stdout_marker() } == 1;
        seen.store(true, Ordering::Release);
        ok
    });
    thread::sleep(Duration::from_millis(200));
    let waited = !done.load(Ordering::Acquire);
    unsafe { __unlockfile(f) };
    let wrote = writer.join().unwrap_or(false);
    println!("{TAG} stdout lock: nested={nested} writer_waited={waited} writer_ok={wrote}");
    nested && waited && wrote && done.load(Ordering::Acquire)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // The C and C++ static constructors, before any C or C++ runs.
    neo_crt::init();
    println!("{TAG} start: musl 1.2.5 + libc++ 18.1.8 over neo-crt's std bottom half");
    c_check(1, "printf family == hand-written strings", crt_check_format);
    c_check(
        2,
        "strtod/strtol/sscanf == independent bit patterns",
        crt_check_scan,
    );
    c_check(
        3,
        "libm: exact where IEEE is, else within 1 ulp",
        crt_check_math,
    );
    c_check(
        4,
        "strings, qsort/bsearch, rand48 sequence, strerror",
        crt_check_strings,
    );
    c_check(
        5,
        "malloc/realloc/calloc/aligned_alloc/posix_memalign",
        crt_check_alloc,
    );
    c_check(
        6,
        "clock_gettime: monotonic yes, realtime EINVAL",
        crt_check_clock,
    );
    c_check(7, "stdout through musl's FILE", crt_check_stdout);
    check(
        8,
        "errno is per thread (6 threads)",
        on_threads(crt_errno_probe),
    );
    check(
        9,
        "malloc/free from 6 threads at once, contents intact",
        on_threads(crt_malloc_stress),
    );
    c_check(10, "C++: a global constructor ran", cxx_check_globals);
    c_check(
        11,
        "C++: vector/sort/unordered_map/map",
        cxx_check_containers,
    );
    c_check(
        12,
        "C++: string, to_string, to_chars, from_chars",
        cxx_check_strings,
    );
    c_check(
        13,
        "C++: unique_ptr/shared_ptr/function/optional/variant/virtual/new(nothrow)",
        cxx_check_objects,
    );
    c_check(14, "C++: steady_clock", cxx_check_clock);
    check(
        15,
        "C++: one function-local static, 6 threads, built once",
        on_threads(cxx_static_guard_probe) && unsafe { cxx_static_guard_builds() } == 1,
    );
    check(
        16,
        "stdout's lock holds a second thread's fputs until it's let go",
        stdout_lock_excludes(),
    );
    let ran = on_threads(crt_stdio_hammer);
    unsafe { crt_stdio_end_runs() };
    check(17, "6 threads print 600-byte runs to stdout at once", ran);
    c_check(
        19,
        "pthread: create/join, a mutex that excludes, self/equal",
        pt_check_threads,
    );
    c_check(
        20,
        "pthread: recursive, error-checking and normal mutexes",
        pt_check_mutex_kinds,
    );
    c_check(
        21,
        "pthread: cond wake, monotonic timeout, realtime refused",
        pt_check_condvar,
    );
    c_check(
        22,
        "pthread: keys per thread, destructors at exit",
        pt_check_keys,
    );
    c_check(23, "pthread: once under 6 threads", pt_check_once);
    c_check(24, "pthread: rwlock", pt_check_rwlock);
    c_check(
        25,
        "C++: std::thread + std::mutex, hardware_concurrency",
        cxx_check_threads,
    );
    c_check(
        26,
        "C++: condition_variable wait_for timeout and wake",
        cxx_check_condition_variable,
    );
    c_check(
        27,
        "C++: call_once, shared_mutex, atomic wait/notify",
        cxx_check_sync,
    );
    println!("{TAG} PASS");
    std::process::exit(0)
}
