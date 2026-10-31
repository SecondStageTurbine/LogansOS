// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.2/C1.3: the one TLS key layer — per-thread keys over FS.base, with
//! destructors at thread exit — shared by `std` and libneo.
//!
//! **One source, two consumers.** `apply-overlay.py` copies this file into
//! rust-src as `std::sys::neoplutarch_rt::tls` (std's `thread_local::key` arm
//! and `std::thread` use it), and `libneo::ntls` includes it with `#[path]`.
//! So std's keys and libneo's (and a future pthread layer's) are one table and
//! one destructor driver. The file is dependency-free (`core` only) and issues
//! no syscall: its parent module (`super`) supplies
//! - `alloc_zeroed(Layout) -> *mut u8` and `dealloc(*mut u8, Layout)`, the
//!   consumer's global allocator;
//! - `set_tls(base: u64) -> i64`, `SYS_THREAD_SET_TLS` (0 or a negative errno).
//!
//! **Layout.** Each thread's [`ThreadControlBlock`] starts with a pointer to
//! itself and holds a pointer to its [`KeyBlock`]; the thread sets FS.base to
//! the control block, so [`tcb`] reads it as `fs:[0]` without a syscall (the
//! x86-64 TCB self-pointer convention). `fn tcb` is the A4 gate's one
//! sanctioned `asm!` outside the syscall trampolines (rule G3). The key block
//! (128 slots, about 2 KiB) is heap-allocated, zeroed, on the thread's first
//! [`set`] and freed by [`release_block`] at exit, so it never sits on the
//! stack. No `__thread`, no `PT_TLS`: the key-only rule.
//!
//! **The initial thread.** A spawned thread installs its block (at its stack
//! base) with [`install_at`] before any application code. The initial thread
//! has no bootstrap, so until FS.base is installed for it the process has one
//! thread, and [`current_tcb`] returns a static block ([`INITIAL`]) instead of
//! reading `fs:[0]`. Before the first thread is created, the spawner calls
//! [`ensure_initial_installed`], which points the initial thread's FS.base at
//! that same static block and flips [`current_tcb`] to `fs:[0]` for good. So a
//! single-threaded program (or one without the C1 capability, whose
//! `SYS_THREAD_SET_TLS` fails) still has working keys, and the switch keeps the
//! initial thread's values.
//!
//! **Keys.** A process-wide registry of [`MAX_KEYS`] entries, each a
//! destructor and a generation. Key 0 is never handed out (`std`'s "not yet
//! created" sentinel). A thread's slot stores a value with the generation it was
//! set under; [`get`] returns it only while that generation is current, and
//! [`delete`] bumps it. So a deleted key's values stay invisible after the index
//! is reused, and their destructors never run against the new key.
//!
//! **Destructors (amendment 1).** At thread exit, [`run_destructors`] scans the
//! keys; for each live key whose slot holds a non-null current-generation value
//! it clears the slot BEFORE calling the destructor, so a destructor sees null
//! unless it sets the key again. It repeats the scan while any destructor ran,
//! up to [`DESTRUCTOR_ITERATIONS`] passes (POSIX's
//! `PTHREAD_DESTRUCTOR_ITERATIONS`), then returns; the caller exits. `std`
//! relies on this: its key-path cleanup sets its key again from inside its own
//! destructor and runs `thread_cleanup` only on the next pass.

use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

/// Marks an initialized [`ThreadControlBlock`]. `b"NEOTCB\0\0"` little-endian.
pub const TCB_MAGIC: u64 = 0x0000_4243_544f_454e;

/// The per-thread control block: at a spawned thread's stack base, or the
/// static [`INITIAL`] for the initial thread.
#[repr(C)]
pub struct ThreadControlBlock {
    /// The block's own address. FS.base points here, so `fs:[0]` yields the
    /// block.
    pub self_ptr: u64,
    /// [`TCB_MAGIC`] once initialized.
    pub magic: u64,
    /// The thread's stack bounds, recorded at install (0 for [`INITIAL`]).
    pub usable_low: u64,
    pub usable_len: u64,
    /// A per-thread `errno` cell.
    pub errno: AtomicI32,
    /// This thread's key values: null until its first [`set`], then a heap
    /// block freed at thread exit.
    pub keys: *mut KeyBlock,
}

// The block sits at every spawned thread's stack base, so it must stay small
// (the key slots live on the heap). An inline 2 KiB key block once overran an
// 8 KiB thread stack while the bootstrap built it (`threads_guard_fault`).
const _: () = assert!(core::mem::size_of::<ThreadControlBlock>() <= 64);

struct InitialTcb(UnsafeCell<ThreadControlBlock>);

// SAFETY: only the initial thread touches the block's fields (its keys and
// errno); other threads only ever see its address.
unsafe impl Sync for InitialTcb {}

/// The initial thread's control block (the module docs' "initial thread").
static INITIAL: InitialTcb = InitialTcb(UnsafeCell::new(ThreadControlBlock {
    self_ptr: 0,
    magic: TCB_MAGIC,
    usable_low: 0,
    usable_len: 0,
    errno: AtomicI32::new(0),
    keys: ptr::null_mut(),
}));

/// Set once the calling process's FS.base convention is live: from then on
/// every thread's block is reached through `fs:[0]`. While it is clear the
/// process has exactly one thread.
static FS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// The calling thread's control block via `fs:[0]`. The A4 gate's one
/// sanctioned `asm!` outside the syscall trampolines: its template must be
/// exactly this load (G3). Never inlined: xtask's ELF backstop requires every
/// FS-relative instruction in a std-real ELF to sit inside this symbol.
#[inline(never)]
fn tcb() -> *mut ThreadControlBlock {
    let block: u64;
    // SAFETY: reads 8 bytes at FS.base, which this thread installed to its
    // control block, whose first word is its own address.
    unsafe {
        core::arch::asm!(
            "mov {}, qword ptr fs:[0]",
            out(reg) block,
            options(nostack, preserves_flags, readonly)
        );
    }
    block as *mut ThreadControlBlock
}

/// The calling thread's control block.
///
/// Before [`ensure_initial_installed`] (or [`install_at`]) has run, the process
/// has one thread and this is [`INITIAL`]. After it, every thread has its block
/// installed (spawned threads do it before any application code), so this
/// reads `fs:[0]`. A thread started without the bootstrap has FS.base 0 and
/// faults on the read: a loud, process-contained failure, never a silent read
/// of another thread's keys.
#[inline]
pub fn current_tcb() -> *mut ThreadControlBlock {
    if FS_INSTALLED.load(Ordering::Acquire) {
        tcb()
    } else {
        INITIAL.0.get()
    }
}

/// Point the initial thread's FS.base at [`INITIAL`] and switch
/// [`current_tcb`] to `fs:[0]`, keeping the initial thread's key values. A
/// spawner calls it before creating the process's first thread. `Ok` if FS.base
/// is (or already was) installed; otherwise the `SYS_THREAD_SET_TLS` errno
/// (e.g. no C1 capability), and the process must stay single-threaded.
///
/// Only call it from a thread that may spawn. While FS is not installed that is
/// the initial thread, the only one.
pub fn ensure_initial_installed() -> Result<(), i64> {
    if FS_INSTALLED.load(Ordering::Acquire) {
        return Ok(());
    }
    let block = INITIAL.0.get();
    // SAFETY: the process has one thread (FS is not installed), and it is us.
    unsafe { (*block).self_ptr = block as u64 };
    let r = super::set_tls(block as u64);
    if r < 0 {
        return Err(r);
    }
    FS_INSTALLED.store(true, Ordering::Release);
    Ok(())
}

/// Install the calling thread's control block at `at` (the base of its own
/// stack, `usable_low`) and point FS.base at it. A spawned thread's bootstrap
/// calls it before any application code; libneo's initial thread may call it
/// too. While FS is not yet installed the caller is the initial thread, and it
/// keeps its key values; otherwise the block starts with none. `Ok` or the
/// `SYS_THREAD_SET_TLS` errno.
///
/// # Safety
/// `at` is the caller's own mapped, writable stack base with room for a
/// `ThreadControlBlock`, and nothing live occupies it (true at thread entry,
/// with RSP at the top).
pub unsafe fn install_at(
    at: *mut ThreadControlBlock,
    usable_low: u64,
    usable_len: u64,
) -> Result<(), i64> {
    let initial = !FS_INSTALLED.load(Ordering::Acquire);
    let keys = if initial {
        // SAFETY: one thread, us: [`INITIAL`] is our block until now.
        unsafe { core::mem::replace(&mut (*INITIAL.0.get()).keys, ptr::null_mut()) }
    } else {
        ptr::null_mut()
    };
    // SAFETY: the caller's contract.
    unsafe {
        ptr::write(
            at,
            ThreadControlBlock {
                self_ptr: at as u64,
                magic: TCB_MAGIC,
                usable_low,
                usable_len,
                errno: AtomicI32::new(0),
                keys,
            },
        );
    }
    let r = super::set_tls(at as u64);
    if r < 0 {
        if initial {
            // SAFETY: still one thread, us; the new block was never
            // published, so hand the keys back to [`INITIAL`].
            unsafe { (*INITIAL.0.get()).keys = keys };
        }
        return Err(r);
    }
    FS_INSTALLED.store(true, Ordering::Release);
    Ok(())
}

/// Keys per process: POSIX's `_POSIX_THREAD_KEYS_MAX`.
pub const MAX_KEYS: usize = 128;

/// Destructor passes at thread exit: POSIX's `PTHREAD_DESTRUCTOR_ITERATIONS`.
pub const DESTRUCTOR_ITERATIONS: usize = 4;

/// A key's destructor, as `std`'s `LazyKey` and `pthread_key_create` take it.
pub type Destructor = unsafe extern "C" fn(*mut u8);

/// A TLS key: an index into the registry, never 0.
pub type Key = usize;

const FREE: u32 = 0;
const CLAIMED: u32 = 1;
const LIVE: u32 = 2;

struct KeyEntry {
    state: AtomicU32,
    generation: AtomicU32,
    /// The destructor as a `usize` (0 = none).
    dtor: AtomicUsize,
}

static KEYS: [KeyEntry; MAX_KEYS] = [const {
    KeyEntry {
        state: AtomicU32::new(FREE),
        generation: AtomicU32::new(0),
        dtor: AtomicUsize::new(0),
    }
}; MAX_KEYS];

/// One thread's value for one key, with the key generation it was set under.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Slot {
    pub value: *mut u8,
    pub generation: u32,
}

impl Slot {
    pub const EMPTY: Self = Self {
        value: ptr::null_mut(),
        generation: 0,
    };
}

/// A thread's key slots. All-zero is the empty block (null values, generation
/// 0). Only the owning thread reads or writes it.
#[repr(C)]
pub struct KeyBlock {
    pub slots: [Slot; MAX_KEYS],
}

/// This thread's key block, or null before its first [`set`].
#[inline]
fn current_keys() -> *mut KeyBlock {
    // SAFETY: `current_tcb` is this thread's live control block.
    unsafe { (*current_tcb()).keys }
}

/// This thread's key block, allocated zeroed on first use; `None` if the heap
/// is exhausted.
fn current_keys_or_alloc() -> Option<*mut KeyBlock> {
    let tcb = current_tcb();
    // SAFETY: this thread's live control block; only this thread writes `keys`.
    unsafe {
        if (*tcb).keys.is_null() {
            let block = super::alloc_zeroed(Layout::new::<KeyBlock>()) as *mut KeyBlock;
            if block.is_null() {
                return None;
            }
            (*tcb).keys = block;
        }
        Some((*tcb).keys)
    }
}

/// Free this thread's key block, if it has one. The thread's exit path calls
/// it after [`run_destructors`]; no key access may follow on this thread.
pub fn release_block() {
    let tcb = current_tcb();
    // SAFETY: this thread's control block; the block came from `alloc_zeroed`
    // with this layout and nothing references it after this point.
    unsafe {
        let block = (*tcb).keys;
        if !block.is_null() {
            (*tcb).keys = ptr::null_mut();
            super::dealloc(block as *mut u8, Layout::new::<KeyBlock>());
        }
    }
}

/// Create a key with an optional destructor. `None` when all [`MAX_KEYS`] are
/// in use (`std` aborts with "out of TLS keys"; pthread returns `EAGAIN`).
pub fn create(dtor: Option<Destructor>) -> Option<Key> {
    for (key, entry) in KEYS.iter().enumerate().skip(1) {
        if entry
            .state
            .compare_exchange(FREE, CLAIMED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            entry
                .dtor
                .store(dtor.map_or(0, |f| f as usize), Ordering::Relaxed);
            entry.state.store(LIVE, Ordering::Release);
            return Some(key);
        }
    }
    None
}

/// Delete `key`: its values on every thread become invisible and their
/// destructors will not run (POSIX `pthread_key_delete` semantics). The index
/// may be handed out again by a later [`create`]. `false` if `key` is not live.
pub fn delete(key: Key) -> bool {
    let Some(entry) = KEYS.get(key).filter(|_| key != 0) else {
        return false;
    };
    if entry
        .state
        .compare_exchange(LIVE, CLAIMED, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return false;
    }
    entry.generation.fetch_add(1, Ordering::Relaxed);
    entry.dtor.store(0, Ordering::Relaxed);
    entry.state.store(FREE, Ordering::Release);
    true
}

/// The current generation of a live key, or `None`.
#[inline]
pub fn live_generation(key: Key) -> Option<u32> {
    let entry = KEYS.get(key).filter(|_| key != 0)?;
    (entry.state.load(Ordering::Acquire) == LIVE).then(|| entry.generation.load(Ordering::Relaxed))
}

/// Set the calling thread's value for `key`. `false` if `key` is not live, or
/// the thread's key block could not be allocated.
pub fn set(key: Key, value: *mut u8) -> bool {
    let Some(generation) = live_generation(key) else {
        return false;
    };
    let Some(block) = current_keys_or_alloc() else {
        return false;
    };
    // SAFETY: this thread's own key block; nothing else writes it.
    unsafe { (*block).slots[key] = Slot { value, generation } };
    true
}

/// The calling thread's value for `key`: null if unset, set under a deleted
/// generation, or `key` is not live.
pub fn get(key: Key) -> *mut u8 {
    let Some(generation) = live_generation(key) else {
        return ptr::null_mut();
    };
    let block = current_keys();
    if block.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: this thread's own key block.
    let slot = unsafe { (*block).slots[key] };
    if slot.generation == generation {
        slot.value
    } else {
        ptr::null_mut()
    }
}

/// Run the calling thread's key destructors (amendment 1): clear each slot
/// before calling its destructor, rescan while any ran, at most
/// [`DESTRUCTOR_ITERATIONS`] passes. Returns the number of passes that ran a
/// destructor. Called by the thread's exit path before `SYS_THREAD_EXIT`.
pub fn run_destructors() -> usize {
    let block = current_keys();
    if block.is_null() {
        return 0; // never set a key: nothing to destroy
    }
    // SAFETY: this thread's own key block.
    unsafe { run_destructors_on(&mut *block) }
}

/// [`run_destructors`] over an explicit block (the host KAT's entry).
///
/// # Safety
/// `block` is the calling thread's key block, or one nothing else is using.
pub unsafe fn run_destructors_on(block: &mut KeyBlock) -> usize {
    let mut passes = 0;
    while passes < DESTRUCTOR_ITERATIONS {
        let mut ran = false;
        for (key, slot_ref) in block.slots.iter_mut().enumerate().skip(1) {
            let Some(generation) = live_generation(key) else {
                continue;
            };
            let slot = *slot_ref;
            if slot.value.is_null() || slot.generation != generation {
                continue;
            }
            let dtor = KEYS[key].dtor.load(Ordering::Relaxed);
            *slot_ref = Slot::EMPTY;
            if dtor != 0 {
                // SAFETY: `dtor` is the `Destructor` `create` stored for this
                // live generation.
                let f: Destructor = unsafe { core::mem::transmute::<usize, Destructor>(dtor) };
                unsafe { f(slot.value) };
                ran = true;
            }
        }
        if !ran {
            break;
        }
        passes += 1;
    }
    passes
}

/// End-of-thread cleanup for the calling thread: [`run_destructors`], then
/// [`release_block`]. The thread exits right after; no key access may follow.
pub fn thread_exit_cleanup() {
    run_destructors();
    release_block();
}
