// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3a: neo-crt's Rust bottom half (`docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md`).
//!
//! musl's portable C (built by `build.rs`) calls down into these functions.
//! Each one is written on the neoplutarch `std`, never on a raw syscall, so
//! this crate adds nothing to the PAL surface: whatever std may do, the C
//! runtime may do, and nothing more.
//!
//! What's here:
//! - the allocator family (`malloc`, `calloc`, `realloc`, `free`,
//!   `aligned_alloc`, `posix_memalign`, `memalign`) on std's global allocator;
//! - `errno`, one per thread;
//! - `clock_gettime` (`CLOCK_MONOTONIC` only; there is no wall clock);
<<<<<<< ours
//! - writes to fds 1 and 2, the stdout/stderr stream locks (`__lockfile`),
//!   `abort`, process exit;
//! - `getenv` (the bundle has no environment), `__lock`/`__unlock` for the few
//!   musl internals that take a lock, and the `atexit` family;
=======
//! - writes to fds 1 and 2, `abort`, process exit;
//! - `getenv` (the bundle has no environment), `__lock`/`__unlock` for the few
//!   musl internals that take a lock, and the `atexit` family;
//! - POSIX threads ([`pthread`], on std's threads and futex-backed locks);
>>>>>>> theirs
//! - the Itanium C++ ABI pieces libc++ expects from a C++ ABI library:
//!   function-local static guards (`__cxa_guard_*`) and the pure/deleted
//!   virtual traps. (No exceptions and no RTTI: libc++abi is not needed.)
//!
//! A program's Rust entry point calls [`init`] before any C or C++ runs:
//! nothing else runs the static constructors.

#![allow(clippy::missing_safety_doc)]

<<<<<<< ours
=======
pub mod pthread;

>>>>>>> theirs
use core::ffi::{c_char, c_int, c_long, c_void};
use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Instant;

// musl's errno values (include/bits/errno.h, arch/generic).
const EINVAL: c_int = 22;
const ENOMEM: c_int = 12;

// ── errno ────────────────────────────────────────────────────────────────

std::thread_local! {
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
}

/// musl's `errno` macro is `(*__errno_location())`. The cell lives as long as
/// its thread, which is as long as C code on that thread can hold the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn __errno_location() -> *mut c_int {
    ERRNO.with(|e| e.as_ptr())
}

/// musl's internal headers reach errno through this alias.
#[unsafe(no_mangle)]
pub extern "C" fn ___errno_location() -> *mut c_int {
    __errno_location()
}

<<<<<<< ours
fn set_errno(v: c_int) {
=======
pub(crate) fn set_errno(v: c_int) {
>>>>>>> theirs
    ERRNO.with(|e| e.set(v));
}

// ── allocation ───────────────────────────────────────────────────────────
//
// C frees without a size, so every block carries a header in the 16 bytes
// just below the pointer it hands out: the total size and alignment of the
// underlying std allocation. The header region is `max(16, align)` bytes, so
// the returned pointer keeps the requested alignment.

const HEADER: usize = 16;
const MIN_ALIGN: usize = 16;

#[repr(C)]
struct Header {
    total: usize,
    align: usize,
}

unsafe fn header_of(p: *mut u8) -> *mut Header {
    unsafe { p.sub(HEADER).cast::<Header>() }
}

fn front(align: usize) -> usize {
    align.max(HEADER)
}

unsafe fn raw_alloc(size: usize, align: usize, zero: bool) -> *mut c_void {
    let align = align.max(MIN_ALIGN);
    if !align.is_power_of_two() {
        set_errno(EINVAL);
        return core::ptr::null_mut();
    }
    let lead = front(align);
    let Some(total) = size.checked_add(lead) else {
        set_errno(ENOMEM);
        return core::ptr::null_mut();
    };
    let Ok(layout) = Layout::from_size_align(total, align) else {
        set_errno(ENOMEM);
        return core::ptr::null_mut();
    };
    let base = unsafe {
        if zero {
            alloc_zeroed(layout)
        } else {
            alloc(layout)
        }
    };
    if base.is_null() {
        set_errno(ENOMEM);
        return core::ptr::null_mut();
    }
    unsafe {
        let p = base.add(lead);
        header_of(p).write(Header { total, align });
        p.cast()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe { raw_alloc(size, MIN_ALIGN, false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(n: usize, size: usize) -> *mut c_void {
    match n.checked_mul(size) {
        Some(bytes) => unsafe { raw_alloc(bytes, MIN_ALIGN, true) },
        None => {
            set_errno(ENOMEM);
            core::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    unsafe {
        let p = p.cast::<u8>();
        let Header { total, align } = header_of(p).read();
        let base = p.sub(front(align));
        dealloc(base, Layout::from_size_align_unchecked(total, align));
    }
}

/// The usable size of a live block (glibc/musl `malloc_usable_size`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc_usable_size(p: *mut c_void) -> usize {
    if p.is_null() {
        return 0;
    }
    unsafe {
        let h = header_of(p.cast()).read();
        h.total - front(h.align)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return unsafe { malloc(size) };
    }
    unsafe {
        let old = malloc_usable_size(p);
        let align = header_of(p.cast()).read().align;
        // Move to a fresh block of the same alignment. (std's realloc would
        // keep the header but not let us grow the leading pad; one copy is
        // the simple, always-correct path.)
        let q = raw_alloc(size, align, false);
        if q.is_null() {
            return core::ptr::null_mut(); // C: the old block stays valid
        }
        core::ptr::copy_nonoverlapping(p.cast::<u8>(), q.cast::<u8>(), old.min(size));
        free(p);
        q
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aligned_alloc(align: usize, size: usize) -> *mut c_void {
<<<<<<< ours
    // C17 (DR 460) and C23: an alignment the implementation doesn't support
    // fails with NULL. Ours are the powers of two; 0 is not one, so it fails
    // here instead of being raised to MIN_ALIGN as malloc's is. A size that
    // isn't a multiple of the alignment is accepted: C17 dropped C11's rule
    // (undefined behaviour there), and glibc and musl both accept it, so
    // ported C++ relies on it (Chad #219; C3a THINK section 3).
    if !align.is_power_of_two() {
        set_errno(EINVAL);
        return core::ptr::null_mut();
    }
=======
>>>>>>> theirs
    unsafe { raw_alloc(size, align, false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memalign(align: usize, size: usize) -> *mut c_void {
    unsafe { raw_alloc(size, align, false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn posix_memalign(out: *mut *mut c_void, align: usize, size: usize) -> c_int {
    if !align.is_power_of_two() || align % core::mem::size_of::<*mut c_void>() != 0 {
        return EINVAL;
    }
    let p = unsafe { raw_alloc(size, align, false) };
    if p.is_null() {
        return ENOMEM;
    }
    unsafe { out.write(p) };
    0
}

// ── time ─────────────────────────────────────────────────────────────────

#[repr(C)]
pub struct Timespec {
<<<<<<< ours
    tv_sec: i64,
    tv_nsec: c_long,
=======
    pub(crate) tv_sec: i64,
    pub(crate) tv_nsec: c_long,
}

/// Zero on neo-crt's monotonic clock: the first read of it in this process.
/// `clock_gettime(CLOCK_MONOTONIC)` and every monotonic deadline
/// (`pthread_cond_timedwait`, `pthread_cond_clockwait`) count from here.
pub(crate) fn monotonic_epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
>>>>>>> theirs
}

const CLOCK_MONOTONIC: c_int = 1;

/// `CLOCK_MONOTONIC` counts from the first call in this process (POSIX leaves
/// the epoch unspecified). `CLOCK_REALTIME` and every other clock are
/// `EINVAL`: the bundle has no wall clock (std's `SystemTime` is unsupported
/// for the same reason).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn clock_gettime(clock: c_int, ts: *mut Timespec) -> c_int {
<<<<<<< ours
    static EPOCH: OnceLock<Instant> = OnceLock::new();
=======
>>>>>>> theirs
    if clock != CLOCK_MONOTONIC || ts.is_null() {
        set_errno(EINVAL);
        return -1;
    }
<<<<<<< ours
    let d = EPOCH.get_or_init(Instant::now).elapsed();
=======
    let d = monotonic_epoch().elapsed();
>>>>>>> theirs
    unsafe {
        ts.write(Timespec {
            tv_sec: d.as_secs() as i64,
            tv_nsec: d.subsec_nanos() as c_long,
        });
    }
    0
}

<<<<<<< ours
// ── stdio stream locks ───────────────────────────────────────────────────

/// Which thread holds each locked `FILE`, by the stream's address.
static STREAM_OWNERS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
static STREAM_FREED: Condvar = Condvar::new();

std::thread_local! {
    /// Its address names this thread to [`__lockfile`].
    static THREAD_TOKEN: u8 = const { 0 };
}

fn thread_token() -> usize {
    THREAD_TOKEN.with(|t| t as *const u8 as usize)
}

/// musl's `FLOCK`: take `f`'s lock for this thread. Returns 1 when this call
/// took it (the caller's `FUNLOCK` releases it), 0 when this thread already
/// holds it (a nested stdio call; musl's own contract). musl's version keeps
/// the owner's tid in `f->lock` and parks on its futex, both pthread
/// internals neo-crt doesn't have; this keeps the owner here instead. Only
/// streams with `lock >= 0` come here: neo-crt's stdout and stderr (CP-100).
#[unsafe(no_mangle)]
pub extern "C" fn __lockfile(f: *mut c_void) -> c_int {
    let (key, me) = (f as usize, thread_token());
    let mut owners = STREAM_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        match owners.iter().find(|(k, _)| *k == key) {
            None => {
                owners.push((key, me));
                return 1;
            }
            Some(&(_, owner)) if owner == me => return 0,
            Some(_) => owners = STREAM_FREED.wait(owners).unwrap_or_else(|e| e.into_inner()),
        }
    }
}

/// musl's `FUNLOCK`, after a [`__lockfile`] that returned 1.
#[unsafe(no_mangle)]
pub extern "C" fn __unlockfile(f: *mut c_void) {
    let key = f as usize;
    let mut owners = STREAM_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    owners.retain(|(k, _)| *k != key);
    drop(owners);
    STREAM_FREED.notify_all();
}

=======
>>>>>>> theirs
// ── output, exit, abort ──────────────────────────────────────────────────

/// Called by `neo_bottom.c`'s `__stdio_write`. Only fds 1 and 2 exist.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __neo_crt_write(fd: c_int, buf: *const u8, len: usize) -> c_long {
    let bytes = unsafe { core::slice::from_raw_parts(buf, len) };
    let r = match fd {
        1 => std::io::stdout().write(bytes),
        2 => std::io::stderr().write(bytes),
        _ => {
            set_errno(9); // EBADF
            return -1;
        }
    };
    match r {
        Ok(n) => n as c_long,
        Err(_) => {
            set_errno(5); // EIO
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __neo_crt_exit(code: c_int) -> ! {
    let _ = std::io::stdout().flush();
    std::process::exit(code)
}

#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    std::process::abort()
}

// ── environment ──────────────────────────────────────────────────────────

/// The bundle has no inherited environment (C2: an empty env).
#[unsafe(no_mangle)]
pub extern "C" fn getenv(_name: *const c_char) -> *mut c_char {
    core::ptr::null_mut()
}

// ── musl's internal lock ─────────────────────────────────────────────────
//
// `__lock`/`__unlock` guard a handful of musl internals (random()'s state).
// A test-and-set with a yield: those sections are a few instructions long and
// rarely contended. (musl's own version parks on a futex.)

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __lock(l: *mut c_int) {
    let a = unsafe { AtomicI32::from_ptr(l) };
    while a
        .compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        std::thread::yield_now();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __unlock(l: *mut c_int) {
    let a = unsafe { AtomicI32::from_ptr(l) };
    a.store(0, Ordering::Release);
}

// ── atexit ───────────────────────────────────────────────────────────────

/// Registered handlers never run: a bundle process ends by `exit` or by being
/// killed, and C++ static destructors at exit are not part of the contract
/// (SWGL and SpiderMonkey shut down explicitly). Accepting the registration
/// keeps `static` objects with destructors linkable.
#[unsafe(no_mangle)]
pub extern "C" fn __cxa_atexit(
    _f: Option<unsafe extern "C" fn(*mut c_void)>,
    _arg: *mut c_void,
    _dso: *mut c_void,
) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn atexit(_f: Option<extern "C" fn()>) -> c_int {
    0
}

// ── static constructors ──────────────────────────────────────────────────

unsafe extern "C" {
    // Defined by the linker (lld) around the output's .init_array.
    static __init_array_start: [Option<unsafe extern "C" fn()>; 0];
    static __init_array_end: [Option<unsafe extern "C" fn()>; 0];
}

/// Run the C/C++ static constructors (`.init_array`), once. A program's entry
/// point calls this before any C or C++ code: neo-crt has no C start-up code
/// of its own, and the Rust `_start` doesn't run them.
pub fn init() {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::AcqRel) {
        return;
    }
    unsafe {
        let mut f = (&raw const __init_array_start).cast::<Option<unsafe extern "C" fn()>>();
        let end = (&raw const __init_array_end).cast::<Option<unsafe extern "C" fn()>>();
        while f < end {
            if let Some(ctor) = f.read() {
                ctor();
            }
            f = f.add(1);
        }
    }
}

// ── C++ ABI: function-local statics ──────────────────────────────────────
//
// Itanium C++ ABI §3.3.2. The guard is 64 bits; the compiler's inline fast
// path tests byte 0 with an acquire load and calls `__cxa_guard_acquire`
// only while it is 0. Byte 0 = 1 once the object is built; byte 1 = 1 while
// some thread is building it. One process-wide mutex and condvar cover every
// guard (initialisation is rare); the mutex is never held while a
// constructor runs, so constructors may initialise other statics.

static GUARDS: Mutex<()> = Mutex::new(());
static GUARD_DONE: Condvar = Condvar::new();

unsafe fn guard_bytes(g: *mut u64) -> (&'static AtomicU8, &'static AtomicU8) {
    let b = g.cast::<u8>();
    unsafe { (AtomicU8::from_ptr(b), AtomicU8::from_ptr(b.add(1))) }
}

/// 1: the caller builds the object, then calls `__cxa_guard_release` (or
/// `_abort`). 0: it is already built. A thread that finds another one
/// building it waits until that one finishes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_guard_acquire(g: *mut u64) -> c_int {
    let (done, busy) = unsafe { guard_bytes(g) };
    if done.load(Ordering::Acquire) != 0 {
        return 0;
    }
    let mut lock = GUARDS.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if done.load(Ordering::Acquire) != 0 {
            return 0;
        }
        if busy.load(Ordering::Relaxed) == 0 {
            busy.store(1, Ordering::Relaxed);
            return 1;
        }
        lock = GUARD_DONE.wait(lock).unwrap_or_else(|e| e.into_inner());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_guard_release(g: *mut u64) {
    let (done, busy) = unsafe { guard_bytes(g) };
    let _lock = GUARDS.lock().unwrap_or_else(|e| e.into_inner());
    busy.store(0, Ordering::Relaxed);
    // Release: the object's construction happens-before any thread's
    // acquire load of byte 0 that sees 1.
    done.store(1, Ordering::Release);
    GUARD_DONE.notify_all();
}

/// The constructor didn't finish (with exceptions off this happens only if
/// the compiler emits it anyway): another thread may try again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __cxa_guard_abort(g: *mut u64) {
    let (_, busy) = unsafe { guard_bytes(g) };
    let _lock = GUARDS.lock().unwrap_or_else(|e| e.into_inner());
    busy.store(0, Ordering::Relaxed);
    GUARD_DONE.notify_all();
}

// ── C++ ABI: virtual-call traps ──────────────────────────────────────────

fn die(msg: &str) -> ! {
    let _ = std::io::stderr().write_all(msg.as_bytes());
    std::process::abort()
}

#[unsafe(no_mangle)]
pub extern "C" fn __cxa_pure_virtual() -> ! {
    die("neo-crt: pure virtual function called\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn __cxa_deleted_virtual() -> ! {
    die("neo-crt: deleted virtual function called\n")
}
