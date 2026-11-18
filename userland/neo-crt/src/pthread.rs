// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3b: neo-crt's POSIX threads subset, on the neoplutarch std
//! (`docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md` §6 item 1).
//!
//! The one threading implementation in the bundle stays std's (ADR-078 A3):
//! threads are `std::thread`, every block is a std `Mutex`/`Condvar` (the
//! futex underneath, C1.1), keys are a std `thread_local!` whose destructor
//! runs the key destructors. So this adds no syscall.
//!
//! Layout: musl's `pthread_mutex_t`, `pthread_cond_t`, `pthread_rwlock_t`
//! are caller-owned and start all-zero (the `*_INITIALIZER`s). Each holds, in
//! its first pointer, a lazily built boxed state, installed by
//! compare-and-swap so two first users agree on one box; `*_destroy` frees
//! it. Attribute objects hold their settings in their `__attr` word.
//!
//! What's here is what SpiderMonkey's POSIX threading layer
//! (`js/src/threading/posix`) and libc++'s pthread thread API use: create,
//! join, detach, self, equal; mutexes (normal, recursive, error-checking,
//! timed); condition variables with `CLOCK_MONOTONIC` (and the realtime
//! default refused, there being no wall clock); keys with destructors; once;
//! rwlocks; and the attribute calls they make.

use core::ffi::{c_char, c_int, c_void};
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle, ThreadId};
use std::time::{Duration, Instant};

use crate::seqcond::SeqCond;
use crate::thread_record::ThreadRecord;
use crate::{Timespec, monotonic_epoch};

// musl's values (include/bits/errno.h, include/pthread.h, include/time.h).
const EAGAIN: c_int = 11;
const EBUSY: c_int = 16;
const EINVAL: c_int = 22;
const EDEADLK: c_int = 35;
const EPERM: c_int = 1;
const ESRCH: c_int = 3;
const ETIMEDOUT: c_int = 110;
const ENOMEM: c_int = 12;
const MUTEX_NORMAL: u32 = 0;
const MUTEX_RECURSIVE: u32 = 1;
const MUTEX_ERRORCHECK: u32 = 2;
const CREATE_DETACHED: u32 = 1;
const CLOCK_REALTIME: c_int = 0;
const CLOCK_MONOTONIC: c_int = 1;
const KEYS_MAX: usize = 128;
const DESTRUCTOR_ITERATIONS: usize = 4;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The lazily built state behind a caller-owned object's first pointer.
unsafe fn state<T>(slot: *mut c_void, make: impl FnOnce() -> T) -> &'static T {
    let p = unsafe { AtomicPtr::<T>::from_ptr(slot.cast()) };
    let cur = p.load(Ordering::Acquire);
    if !cur.is_null() {
        return unsafe { &*cur };
    }
    let fresh = Box::into_raw(Box::new(make()));
    match p.compare_exchange(
        core::ptr::null_mut(),
        fresh,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => unsafe { &*fresh },
        Err(won) => {
            drop(unsafe { Box::from_raw(fresh) });
            unsafe { &*won }
        }
    }
}

unsafe fn free_state<T>(slot: *mut c_void) {
    let p = unsafe { AtomicPtr::<T>::from_ptr(slot.cast()) };
    let cur = p.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if !cur.is_null() {
        drop(unsafe { Box::from_raw(cur) });
    }
}

/// An absolute `CLOCK_MONOTONIC` time (neo-crt's clock: zero at its first
/// read) as an `Instant`.
fn deadline(ts: &Timespec) -> Option<Instant> {
    if ts.tv_nsec < 0 || ts.tv_nsec >= 1_000_000_000 || ts.tv_sec < 0 {
        return None;
    }
    let at = Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32);
    Some(monotonic_epoch() + at)
}

// ── threads ──────────────────────────────────────────────────────────────

/// A thread as C sees it: `pthread_t` is a pointer to this, kept alive by
/// one reference per holder (the creator until join/detach, and the thread
/// itself through `SELF`).
pub struct PThread {
    handle: Mutex<Option<JoinHandle<usize>>>,
    id: ThreadId,
    /// `pthread_setname_np`'s name: at most 15 bytes and a NUL, as Linux.
    name: Mutex<[u8; NAME_LEN]>,
}

/// A thread name's buffer: 15 bytes and the terminating NUL (Linux's
/// `TASK_COMM_LEN`).
const NAME_LEN: usize = 16;
const ERANGE: c_int = 34;

std::thread_local! {
    static SELF: RefCell<Option<Arc<PThread>>> = const { RefCell::new(None) };
    /// Whether this thread is one `pthread_create` made (C3c-2's record
    /// counts only those threads' condvar waits).
    static OURS: Cell<bool> = const { Cell::new(false) };
}

/// OT-61 C3c-2: threads made, exited and joined, and their condvar wakes
/// (`thread_record.rs`).
pub(crate) static THREADS: ThreadRecord = ThreadRecord::new();

type StartFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

#[repr(C)]
pub struct PthreadAttr {
    stack: usize,
    detach: u32,
    _rest: [u32; 11],
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_create(
    out: *mut *const PThread,
    attr: *const PthreadAttr,
    start: StartFn,
    arg: *mut c_void,
) -> c_int {
    let (stack, detached) = if attr.is_null() {
        (0, false)
    } else {
        let a = unsafe { &*attr };
        (a.stack, a.detach == CREATE_DETACHED)
    };
    let mut b = thread::Builder::new();
    if stack != 0 {
        b = b.stack_size(stack);
    }
    let arg = arg as usize;
    // The record exists before the thread runs, so both sides share it.
    let slot: Arc<Mutex<Option<Arc<PThread>>>> = Arc::new(Mutex::new(None));
    let ready = Arc::new((Mutex::new(false), Condvar::new()));
    let (slot2, ready2) = (slot.clone(), ready.clone());
    let spawned = b.spawn(move || {
        {
            let (m, cv) = &*ready2;
            let mut r = lock(m);
            while !*r {
                r = cv.wait(r).unwrap_or_else(|e| e.into_inner());
            }
        }
        let me = lock(&slot2).take();
        SELF.with(|s| *s.borrow_mut() = me);
        OURS.with(|o| o.set(true));
        let ret = unsafe { start(arg as *mut c_void) } as usize;
        crate::pthread::run_key_destructors();
        THREADS.exited();
        ret
    });
    let handle = match spawned {
        Ok(h) => h,
        Err(_) => return EAGAIN,
    };
    THREADS.created();
    let rec = Arc::new(PThread {
        id: handle.thread().id(),
        handle: Mutex::new(Some(handle)),
        name: Mutex::new([0; NAME_LEN]),
    });
    *lock(&slot) = Some(rec.clone());
    {
        let (m, cv) = &*ready;
        *lock(m) = true;
        cv.notify_one();
    }
    if detached {
        lock(&rec.handle).take();
        // The thread keeps its own reference through SELF.
        unsafe { out.write(Arc::as_ptr(&rec)) };
        drop(rec);
    } else {
        unsafe { out.write(Arc::into_raw(rec)) };
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_join(t: *const PThread, ret: *mut *mut c_void) -> c_int {
    if t.is_null() {
        return ESRCH;
    }
    let rec = unsafe { &*t };
    if rec.id == thread::current().id() {
        return EDEADLK;
    }
    let Some(h) = lock(&rec.handle).take() else {
        return EINVAL; // detached, or already joined
    };
    let v = h.join().unwrap_or(0);
    THREADS.joined();
    if !ret.is_null() {
        unsafe { ret.write(v as *mut c_void) };
    }
    drop(unsafe { Arc::from_raw(t) }); // the creator's reference
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_detach(t: *const PThread) -> c_int {
    if t.is_null() {
        return ESRCH;
    }
    let rec = unsafe { &*t };
    if lock(&rec.handle).take().is_none() {
        return EINVAL;
    }
    drop(unsafe { Arc::from_raw(t) });
    0
}

/// The calling thread's record. A thread neo-crt didn't create (the main
/// thread, or a Rust `std::thread`) gets one on first use, with no handle.
#[unsafe(no_mangle)]
pub extern "C" fn pthread_self() -> *const PThread {
    SELF.with(|s| {
        let mut s = s.borrow_mut();
        let rec = s.get_or_insert_with(|| {
            Arc::new(PThread {
                handle: Mutex::new(None),
                id: thread::current().id(),
                name: Mutex::new([0; NAME_LEN]),
            })
        });
        Arc::as_ptr(rec)
    })
}

/// `pthread_setname_np`: stores the name in the thread's record, cut to 15
/// bytes as Linux's comm field is, and returns 0. A name is an advisory label
/// (nothing here shows it yet), and SpiderMonkey release-asserts that naming
/// succeeds (PosixThread.cpp), so a long name is truncated, not refused (PM
/// decision on #88, relay 351).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_setname_np(t: *const PThread, name: *const c_char) -> c_int {
    if t.is_null() || name.is_null() {
        return EINVAL;
    }
    let bytes = unsafe { core::ffi::CStr::from_ptr(name) }.to_bytes();
    let n = bytes.len().min(NAME_LEN - 1);
    let rec = unsafe { &*t };
    let mut stored = lock(&rec.name);
    *stored = [0; NAME_LEN];
    stored[..n].copy_from_slice(&bytes[..n]);
    0
}

/// `pthread_getname_np`: the stored name, NUL-terminated, or `ERANGE` if
/// `len` can't hold it (Linux's behaviour). A thread never named reads "".
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_getname_np(t: *const PThread, buf: *mut c_char, len: usize) -> c_int {
    if t.is_null() || buf.is_null() {
        return EINVAL;
    }
    let stored = *lock(&unsafe { &*t }.name);
    let n = stored.iter().position(|&b| b == 0).unwrap_or(NAME_LEN - 1);
    if len < n + 1 {
        return ERANGE;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(stored.as_ptr(), buf.cast::<u8>(), n);
        *buf.add(n) = 0;
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_equal(a: *const PThread, b: *const PThread) -> c_int {
    (a == b) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn sched_yield() -> c_int {
    thread::yield_now();
    0
}

// ── thread attributes ────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_attr_init(a: *mut PthreadAttr) -> c_int {
    unsafe {
        a.write(PthreadAttr {
            stack: 0,
            detach: 0,
            _rest: [0; 11],
        })
    };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_attr_destroy(_a: *mut PthreadAttr) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_attr_setstacksize(a: *mut PthreadAttr, size: usize) -> c_int {
    if size < 16 * 1024 {
        return EINVAL;
    }
    unsafe { (*a).stack = size };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_attr_getstacksize(
    a: *const PthreadAttr,
    size: *mut usize,
) -> c_int {
    unsafe { size.write((*a).stack) };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_attr_setdetachstate(a: *mut PthreadAttr, state: c_int) -> c_int {
    if state != 0 && state != 1 {
        return EINVAL;
    }
    unsafe { (*a).detach = state as u32 };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_attr_getdetachstate(
    a: *const PthreadAttr,
    state: *mut c_int,
) -> c_int {
    unsafe { state.write((*a).detach as c_int) };
    0
}

// ── mutexes ──────────────────────────────────────────────────────────────

struct MutexState {
    kind: u32,
    held: Mutex<Held>,
    freed: Condvar,
}

#[derive(Default)]
struct Held {
    owner: Option<ThreadId>,
    depth: u32,
}

/// `pthread_mutexattr_t` is one word: the kind.
#[repr(C)]
pub struct MutexAttr {
    kind: u32,
}

#[repr(C)]
pub struct PthreadMutex {
    state: *mut c_void,
    kind: u32,
    _rest: [u32; 7],
}

unsafe fn mutex_state(m: *mut PthreadMutex) -> &'static MutexState {
    let kind = unsafe { (*m).kind };
    unsafe {
        state(core::ptr::addr_of_mut!((*m).state).cast(), || MutexState {
            kind,
            held: Mutex::new(Held::default()),
            freed: Condvar::new(),
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutexattr_init(a: *mut MutexAttr) -> c_int {
    unsafe { a.write(MutexAttr { kind: MUTEX_NORMAL }) };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_mutexattr_destroy(_a: *mut MutexAttr) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutexattr_settype(a: *mut MutexAttr, kind: c_int) -> c_int {
    if !(0..=2).contains(&kind) {
        return EINVAL;
    }
    unsafe { (*a).kind = kind as u32 };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutex_init(m: *mut PthreadMutex, a: *const MutexAttr) -> c_int {
    let kind = if a.is_null() {
        MUTEX_NORMAL
    } else {
        unsafe { (*a).kind }
    };
    unsafe {
        m.write(PthreadMutex {
            state: core::ptr::null_mut(),
            kind,
            _rest: [0; 7],
        })
    };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutex_destroy(m: *mut PthreadMutex) -> c_int {
    let s = unsafe { mutex_state(m) };
    if lock(&s.held).owner.is_some() {
        return EBUSY;
    }
    unsafe { free_state::<MutexState>(core::ptr::addr_of_mut!((*m).state).cast()) };
    0
}

fn acquire(s: &MutexState, until: Option<Instant>, try_only: bool) -> c_int {
    let me = thread::current().id();
    let mut h = lock(&s.held);
    if h.owner == Some(me) {
        match s.kind {
            MUTEX_RECURSIVE => {
                h.depth += 1;
                return 0;
            }
            // POSIX: trylock on a held mutex is EBUSY, whoever holds it.
            _ if try_only => return EBUSY,
            MUTEX_ERRORCHECK => return EDEADLK,
            _ => {} // NORMAL: relocking deadlocks, as POSIX says
        }
    }
    while h.owner.is_some() {
        if try_only {
            return EBUSY;
        }
        h = match until {
            None => s.freed.wait(h).unwrap_or_else(|e| e.into_inner()),
            Some(t) => {
                let now = Instant::now();
                if now >= t {
                    return ETIMEDOUT;
                }
                s.freed
                    .wait_timeout(h, t - now)
                    .unwrap_or_else(|e| e.into_inner())
                    .0
            }
        };
    }
    h.owner = Some(me);
    h.depth = 1;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutex_lock(m: *mut PthreadMutex) -> c_int {
    acquire(unsafe { mutex_state(m) }, None, false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutex_trylock(m: *mut PthreadMutex) -> c_int {
    acquire(unsafe { mutex_state(m) }, None, true)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutex_timedlock(
    m: *mut PthreadMutex,
    at: *const Timespec,
) -> c_int {
    // POSIX measures a timed lock against CLOCK_REALTIME; there is none.
    let _ = (m, at);
    EINVAL
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_mutex_unlock(m: *mut PthreadMutex) -> c_int {
    let s = unsafe { mutex_state(m) };
    let mut h = lock(&s.held);
    if h.owner != Some(thread::current().id()) {
        return EPERM;
    }
    h.depth -= 1;
    if h.depth == 0 {
        h.owner = None;
        drop(h);
        s.freed.notify_one();
    }
    0
}

// ── condition variables ──────────────────────────────────────────────────
//
// A sequence number under the condvar's own mutex. A waiter takes that
// mutex, reads the number, then releases the user's mutex, then waits for
// the number to change; a signaller takes the same mutex to bump it. So a
// signal sent after the waiter released the user mutex is never lost: the
// signaller can't bump until the waiter is waiting.

struct CondState {
    clock: c_int,
    sc: SeqCond<Condvar>,
}

#[repr(C)]
pub struct CondAttr {
    clock: u32,
}

#[repr(C)]
pub struct PthreadCond {
    state: *mut c_void,
    clock: u32,
    _rest: [u32; 9],
}

unsafe fn cond_state(c: *mut PthreadCond) -> &'static CondState {
    let clock = unsafe { (*c).clock } as c_int;
    unsafe {
        state(core::ptr::addr_of_mut!((*c).state).cast(), || CondState {
            clock,
            sc: SeqCond::new(Condvar::new()),
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_condattr_init(a: *mut CondAttr) -> c_int {
    unsafe {
        a.write(CondAttr {
            clock: CLOCK_REALTIME as u32,
        })
    };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_condattr_destroy(_a: *mut CondAttr) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_condattr_setclock(a: *mut CondAttr, clock: c_int) -> c_int {
    if clock != CLOCK_REALTIME && clock != CLOCK_MONOTONIC {
        return EINVAL;
    }
    unsafe { (*a).clock = clock as u32 };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_init(c: *mut PthreadCond, a: *const CondAttr) -> c_int {
    let clock = if a.is_null() {
        CLOCK_REALTIME as u32
    } else {
        unsafe { (*a).clock }
    };
    unsafe {
        c.write(PthreadCond {
            state: core::ptr::null_mut(),
            clock,
            _rest: [0; 9],
        })
    };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_destroy(c: *mut PthreadCond) -> c_int {
    unsafe { free_state::<CondState>(core::ptr::addr_of_mut!((*c).state).cast()) };
    0
}

/// The sequence logic (and why signalling wakes under the sequence lock) is
/// `seqcond.rs`; this adds the caller's mutex around it.
unsafe fn cond_wait(c: *mut PthreadCond, m: *mut PthreadMutex, until: Option<Instant>) -> c_int {
    let s = unsafe { cond_state(c) };
    let release = || match unsafe { pthread_mutex_unlock(m) } {
        0 => Ok(()),
        r => Err(r),
    };
    let timed_out = match s.sc.wait(release, until) {
        Ok(t) => t,
        Err(r) => return r,
    };
    // `try_with`: a key destructor can wait on a condvar after this thread's
    // std TLS is gone; such a wait just isn't counted.
    THREADS.cond_returned(OURS.try_with(Cell::get).unwrap_or(false), timed_out);
    let r = unsafe { pthread_mutex_lock(m) };
    if r != 0 {
        return r;
    }
    if timed_out { ETIMEDOUT } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_wait(c: *mut PthreadCond, m: *mut PthreadMutex) -> c_int {
    unsafe { cond_wait(c, m, None) }
}

/// Only a `CLOCK_MONOTONIC` condvar may time out (set with
/// `pthread_condattr_setclock`): the default clock, `CLOCK_REALTIME`, has no
/// source here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_timedwait(
    c: *mut PthreadCond,
    m: *mut PthreadMutex,
    at: *const Timespec,
) -> c_int {
    let s = unsafe { cond_state(c) };
    if s.clock != CLOCK_MONOTONIC || at.is_null() {
        return EINVAL;
    }
    let Some(t) = deadline(unsafe { &*at }) else {
        return EINVAL;
    };
    unsafe { cond_wait(c, m, Some(t)) }
}

/// glibc's (and POSIX 2024's) wait against a named clock. libc++ uses it,
/// when `_LIBCPP_HAS_COND_CLOCKWAIT` is set (neo-crt's `__config_site`), for
/// every `condition_variable` wait with a timeout, with `CLOCK_MONOTONIC`:
/// the only clock that times out here. (musl 1.2.5 doesn't declare it;
/// `cxx/pthread.h` does.)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_clockwait(
    c: *mut PthreadCond,
    m: *mut PthreadMutex,
    clock: c_int,
    at: *const Timespec,
) -> c_int {
    if clock != CLOCK_MONOTONIC || at.is_null() {
        return EINVAL;
    }
    let Some(t) = deadline(unsafe { &*at }) else {
        return EINVAL;
    };
    unsafe { cond_wait(c, m, Some(t)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_signal(c: *mut PthreadCond) -> c_int {
    unsafe { cond_state(c) }.sc.signal();
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_cond_broadcast(c: *mut PthreadCond) -> c_int {
    unsafe { cond_state(c) }.sc.broadcast();
    0
}

// ── keys ─────────────────────────────────────────────────────────────────

type Dtor = Option<unsafe extern "C" fn(*mut c_void)>;

/// Each key: whether it's live, its destructor, and a generation so a value
/// set under a deleted key isn't seen through a new one in the same slot.
struct Keys {
    live: [bool; KEYS_MAX],
    dtor: [Dtor; KEYS_MAX],
    generation: [u32; KEYS_MAX],
}

static KEYS: Mutex<Keys> = Mutex::new(Keys {
    live: [false; KEYS_MAX],
    dtor: [None; KEYS_MAX],
    generation: [0; KEYS_MAX],
});

struct Slots {
    values: [(u32, usize); KEYS_MAX],
    /// Destructor rounds this thread has had, counted across both paths
    /// (`run_key_destructors` and this `Drop`), so together they run at most
    /// `PTHREAD_DESTRUCTOR_ITERATIONS` (#67 re-check, Chad).
    rounds: usize,
}

/// Count one more destructor round in `rounds`; false once the thread has had
/// them all.
fn next_round(rounds: &mut usize) -> bool {
    if *rounds < DESTRUCTOR_ITERATIONS {
        *rounds += 1;
        true
    } else {
        false
    }
}

/// Any thread that touched a key, however it was made (a `std::thread`, or
/// libc++ through `pthread_create`), runs its key destructors when std tears
/// its TLS down: std registers this `Drop` on the thread's first key access.
/// Threads made by `pthread_create` have already run them
/// (`run_key_destructors`, with every slot still reachable), so this finds
/// nothing left there. Here the slots are being destroyed, so a destructor
/// that sets a value again gets `ENOMEM` and there is one round, and none
/// if `run_key_destructors` already used all
/// `PTHREAD_DESTRUCTOR_ITERATIONS` (a destructor that always re-arms runs 4
/// times in all, not 5). The main thread returning from `main` runs none, as
/// POSIX has it for `exit`.
impl Drop for Slots {
    fn drop(&mut self) {
        let Slots { values, rounds } = self;
        destructor_rounds(
            || next_round(rounds),
            |i, g| take_value(&mut values[i], g),
        );
    }
}

/// Clear one slot if it holds a non-null value under generation `g`, and
/// return that value (0 if there is none).
fn take_value(slot: &mut (u32, usize), g: u32) -> usize {
    let (sg, v) = *slot;
    if sg == g && v != 0 {
        *slot = (g, 0);
        v
    } else {
        0
    }
}

std::thread_local! {
    static SLOTS: RefCell<Slots> = const {
        RefCell::new(Slots { values: [(0, 0); KEYS_MAX], rounds: 0 })
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_key_create(key: *mut u32, dtor: Dtor) -> c_int {
    let mut k = lock(&KEYS);
    let Some(i) = k.live.iter().position(|l| !l) else {
        return EAGAIN;
    };
    k.live[i] = true;
    k.dtor[i] = dtor;
    k.generation[i] = k.generation[i].wrapping_add(1);
    unsafe { key.write(i as u32) };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_key_delete(key: u32) -> c_int {
    let mut k = lock(&KEYS);
    let i = key as usize;
    if i >= KEYS_MAX || !k.live[i] {
        return EINVAL;
    }
    k.live[i] = false;
    k.dtor[i] = None;
    0
}

fn generation(i: usize) -> Option<u32> {
    let k = lock(&KEYS);
    k.live[i].then_some(k.generation[i])
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_getspecific(key: u32) -> *mut c_void {
    let i = key as usize;
    if i >= KEYS_MAX {
        return core::ptr::null_mut();
    }
    let Some(g) = generation(i) else {
        return core::ptr::null_mut();
    };
    SLOTS
        .try_with(|s| {
            let (sg, v) = s.borrow().values[i];
            if sg == g {
                v as *mut c_void
            } else {
                core::ptr::null_mut()
            }
        })
        .unwrap_or(core::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn pthread_setspecific(key: u32, value: *const c_void) -> c_int {
    let i = key as usize;
    if i >= KEYS_MAX {
        return EINVAL;
    }
    let Some(g) = generation(i) else {
        return EINVAL;
    };
    match SLOTS.try_with(|s| s.borrow_mut().values[i] = (g, value as usize)) {
        Ok(()) => 0,
        Err(_) => ENOMEM, // the thread's slots are already gone (it is exiting)
    }
}

/// Run the calling thread's key destructors: up to
/// `PTHREAD_DESTRUCTOR_ITERATIONS` rounds, each clearing a non-null value and
/// passing it to its key's destructor (POSIX). Called when a thread made by
/// `pthread_create` returns from its start routine.
pub fn run_key_destructors() {
    destructor_rounds(
        || {
            SLOTS
                .try_with(|s| next_round(&mut s.borrow_mut().rounds))
                .unwrap_or(false)
        },
        |i, g| {
            SLOTS
                .try_with(|s| take_value(&mut s.borrow_mut().values[i], g))
                .unwrap_or(0)
        },
    );
}

/// Rounds while `next_round()` allows (it counts the thread's rounds, at most
/// `PTHREAD_DESTRUCTOR_ITERATIONS`): each takes every live key's non-null
/// value through `take(index, generation)` (which clears it) and passes it to
/// the key's destructor; stop after a round that ran none.
fn destructor_rounds(
    mut next_round: impl FnMut() -> bool,
    mut take: impl FnMut(usize, u32) -> usize,
) {
    while next_round() {
        let mut ran = false;
        for i in 0..KEYS_MAX {
            let (g, dtor) = {
                let k = lock(&KEYS);
                if !k.live[i] {
                    continue;
                }
                (k.generation[i], k.dtor[i])
            };
            let Some(dtor) = dtor else { continue };
            let v = take(i, g);
            if v != 0 {
                unsafe { dtor(v as *mut c_void) };
                ran = true;
            }
        }
        if !ran {
            break;
        }
    }
}

// ── once ─────────────────────────────────────────────────────────────────

static ONCE_LOCK: Mutex<()> = Mutex::new(());
static ONCE_DONE: Condvar = Condvar::new();

/// `pthread_once_t` is an int: 0 not run, 1 running, 2 done.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_once(once: *mut c_int, init: extern "C" fn()) -> c_int {
    let st = unsafe { AtomicU32::from_ptr(once.cast()) };
    if st.load(Ordering::Acquire) == 2 {
        return 0;
    }
    let mut g = lock(&ONCE_LOCK);
    loop {
        match st.load(Ordering::Acquire) {
            2 => return 0,
            0 => {
                st.store(1, Ordering::Relaxed);
                drop(g);
                init();
                let _g = lock(&ONCE_LOCK);
                st.store(2, Ordering::Release);
                ONCE_DONE.notify_all();
                return 0;
            }
            _ => g = ONCE_DONE.wait(g).unwrap_or_else(|e| e.into_inner()),
        }
    }
}

// ── rwlocks ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct RwHeld {
    readers: u32,
    writer: bool,
    waiting_writers: u32,
}

struct RwState {
    held: Mutex<RwHeld>,
    changed: Condvar,
}

#[repr(C)]
pub struct PthreadRwlock {
    state: *mut c_void,
    _rest: [u32; 12],
}

unsafe fn rw_state(l: *mut PthreadRwlock) -> &'static RwState {
    unsafe {
        state(core::ptr::addr_of_mut!((*l).state).cast(), || RwState {
            held: Mutex::new(RwHeld::default()),
            changed: Condvar::new(),
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_init(l: *mut PthreadRwlock, _a: *const c_void) -> c_int {
    unsafe {
        l.write(PthreadRwlock {
            state: core::ptr::null_mut(),
            _rest: [0; 12],
        })
    };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_destroy(l: *mut PthreadRwlock) -> c_int {
    unsafe { free_state::<RwState>(core::ptr::addr_of_mut!((*l).state).cast()) };
    0
}

/// Writers are preferred: a reader waits while a writer waits, so a steady
/// stream of readers can't starve a writer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_rdlock(l: *mut PthreadRwlock) -> c_int {
    let s = unsafe { rw_state(l) };
    let mut h = lock(&s.held);
    while h.writer || h.waiting_writers > 0 {
        h = s.changed.wait(h).unwrap_or_else(|e| e.into_inner());
    }
    h.readers += 1;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_tryrdlock(l: *mut PthreadRwlock) -> c_int {
    let s = unsafe { rw_state(l) };
    let mut h = lock(&s.held);
    if h.writer || h.waiting_writers > 0 {
        return EBUSY;
    }
    h.readers += 1;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_wrlock(l: *mut PthreadRwlock) -> c_int {
    let s = unsafe { rw_state(l) };
    let mut h = lock(&s.held);
    h.waiting_writers += 1;
    while h.writer || h.readers > 0 {
        h = s.changed.wait(h).unwrap_or_else(|e| e.into_inner());
    }
    h.waiting_writers -= 1;
    h.writer = true;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_trywrlock(l: *mut PthreadRwlock) -> c_int {
    let s = unsafe { rw_state(l) };
    let mut h = lock(&s.held);
    if h.writer || h.readers > 0 {
        return EBUSY;
    }
    h.writer = true;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_rwlock_unlock(l: *mut PthreadRwlock) -> c_int {
    let s = unsafe { rw_state(l) };
    let mut h = lock(&s.held);
    if h.writer {
        h.writer = false;
    } else if h.readers > 0 {
        h.readers -= 1;
    } else {
        return EPERM;
    }
    drop(h);
    s.changed.notify_all();
    0
}

// ── sleeping and the CPU count ───────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nanosleep(req: *const Timespec, rem: *mut Timespec) -> c_int {
    let r = unsafe { &*req };
    if r.tv_nsec < 0 || r.tv_nsec >= 1_000_000_000 || r.tv_sec < 0 {
        crate::set_errno(EINVAL);
        return -1;
    }
    thread::sleep(Duration::new(r.tv_sec as u64, r.tv_nsec as u32));
    if !rem.is_null() {
        unsafe {
            rem.write(Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            })
        };
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn usleep(us: u32) -> c_int {
    thread::sleep(Duration::from_micros(us as u64));
    0
}

const SC_NPROCESSORS_CONF: c_int = 83;
const SC_NPROCESSORS_ONLN: c_int = 84;
const SC_PAGESIZE: c_int = 30;

/// The few `sysconf` names the bundle's C++ asks for.
#[unsafe(no_mangle)]
pub extern "C" fn sysconf(name: c_int) -> i64 {
    match name {
        SC_NPROCESSORS_CONF | SC_NPROCESSORS_ONLN => {
            thread::available_parallelism().map_or(1, |n| n.get() as i64)
        }
        SC_PAGESIZE => 4096,
        _ => {
            crate::set_errno(EINVAL);
            -1
        }
    }
}
