// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.1: userland thread spawn/join over the P2 syscalls, with the
//! TLS-FREE bootstrap trampoline the P2 amendment freezes.
//!
//! Every spawned thread starts in [`thread_bootstrap`] (no TLS exists yet),
//! which — BEFORE running any application code (blocker E) —
//!  1. queries its own stack bounds (`sys_thread_get_info(0)`), and
//!  2. establishes the per-thread control block: a stack-relative
//!     [`ThreadControlBlock`] at the stack base (`usable_low`) holding a magic,
//!     the recorded bounds, a per-thread **errno** cell, and the reserved
//!     C1.2 **TCB/key-block** seam — located WITHOUT `__thread`/FS-relative
//!     TLS, via the stack bounds `thread_get_info` returns. Only THEN does it
//!  3. call the application start routine and exit with its code.
//!
//! The control block is at `usable_low` (the lowest stack address, adjacent to
//! the guard page): the app's RSP starts at the top and grows down, so the
//! block is out of normal stack use; a stack overflow deep enough to reach it
//! would fault the guard first. Any thread — including the initial one — can
//! locate its own block with [`control_block`] (query bounds, read the block
//! at `usable_low`), giving a TLS-free per-thread errno now and the seam C1.2
//! fills with the real TCB/key layer.
//!
//! **C1.2 (P3).** The block's first word now points at the block itself and it
//! holds a pointer to the thread's key block (heap, allocated on first use);
//! the bootstrap (and [`init_main_thread_control_block`]) set FS.base to it
//! with `SYS_THREAD_SET_TLS`, so [`crate::ntls`] reaches it as `fs:[0]` with
//! no syscall. A thread ends through [`exit`], which runs the key destructors
//! (iterated, at most four passes) before `SYS_THREAD_EXIT`. C1.3: the block
//! and its installer live in the key layer std shares
//! ([`crate::ntls::install_at`]).

use alloc::boxed::Box;
use core::sync::atomic::AtomicI32;

use crate::ntls;
use crate::{
    ThreadInfoV1, sys_thread_create, sys_thread_exit, sys_thread_get_info, sys_thread_join,
};

/// Default stack for [`spawn`]: matches `std`'s `DEFAULT_MIN_STACK_SIZE`.
pub const DEFAULT_STACK_SIZE: usize = 64 * 1024;

pub use crate::ntls::{TCB_MAGIC, ThreadControlBlock};

/// Locate the calling thread's control block (installed by the bootstrap, or
/// by [`init_main_thread_control_block`] for the initial thread). Returns
/// `None` if bounds are unavailable or the block is uninitialized (the magic
/// is absent) — the negative-control signal.
pub fn control_block() -> Option<&'static ThreadControlBlock> {
    let info = sys_thread_get_info(0).ok()?;
    if info.usable_low == 0 {
        return None;
    }
    // SAFETY: `usable_low` is this thread's own mapped, writable stack base
    // (the kernel reported it); the block was written there before app entry.
    let tcb = unsafe { &*(info.usable_low as *const ThreadControlBlock) };
    if tcb.magic != TCB_MAGIC {
        return None;
    }
    Some(tcb)
}

/// The calling thread's per-thread `errno` cell, or `None` if the bootstrap
/// block is not established.
pub fn errno_cell() -> Option<&'static AtomicI32> {
    control_block().map(|tcb| &tcb.errno)
}

/// Install the calling thread's control block at its stack base and point
/// FS.base at it ([`ntls::install_at`]).
///
/// SAFETY: `info` is the caller's own stack, and no live data occupies its base
/// (true immediately at thread entry — RSP is at the top).
unsafe fn install_control_block(info: &ThreadInfoV1) -> bool {
    let tcb = info.usable_low as *mut ThreadControlBlock;
    // SAFETY: the caller's contract.
    unsafe { ntls::install_at(tcb, info.usable_low, info.usable_len) }.is_ok()
}

/// Establish the initial (main) thread's control block. The kernel populates
/// `thread_get_info` for the initial thread too, but no bootstrap trampoline
/// ran for it, so the runtime start (or a test) calls this once so the main
/// thread also has a TLS-free errno + bounds, and (C1.2) its FS.base and key
/// block. Calling it again resets the block, clearing the main thread's key
/// values (key values set before the first call carry over). `false` if the
/// bounds are unavailable or `SYS_THREAD_SET_TLS` refused (no C1 capability).
pub fn init_main_thread_control_block() -> bool {
    let Ok(info) = sys_thread_get_info(0) else {
        return false;
    };
    if info.usable_low == 0 {
        return false;
    }
    // SAFETY: the initial thread's own stack base, reported by the kernel.
    unsafe { install_control_block(&info) }
}

/// End the calling thread: run its TLS key destructors (C1.2 amendment 1,
/// iterated, at most four passes), then `SYS_THREAD_EXIT(code)`. Every thread
/// [`spawn`] starts ends here when its start routine returns; a thread may also
/// call it directly. Requires an installed control block (as every `ntls` call
/// does).
pub fn exit(code: i32) -> ! {
    ntls::run_destructors();
    ntls::release_block();
    sys_thread_exit(code)
}

struct ThreadStart {
    f: extern "C" fn(usize) -> i32,
    arg: usize,
}

/// The TLS-free entry every spawned thread runs first. Establishes the
/// per-thread control block (blocker E) BEFORE the application function.
/// `raw` is the `Box<ThreadStart>` pointer `spawn` leaked; ownership transfers
/// here.
extern "C" fn thread_bootstrap(raw: usize) -> ! {
    // (1)+(2): record bounds + install the TLS-free control block (magic,
    // bounds, errno, key slots) and point FS.base at it, before any
    // application code.
    if let Ok(info) = sys_thread_get_info(0)
        && info.usable_low != 0
    {
        // SAFETY: our own freshly-allocated stack base; RSP is at the top, so
        // nothing else occupies the base yet. The spawner held the C1
        // capability (thread_create checks it), so SYS_THREAD_SET_TLS
        // succeeds; if it ever did not, FS.base stays 0 and the first key
        // access faults loudly.
        let _ = unsafe { install_control_block(&info) };
    }

    // (3): run the application start routine, then exit with its code.
    // SAFETY: `raw` is the Box::into_raw pointer from `spawn`, passed through
    // the kernel's arg register untouched; this is its unique consumer.
    let start = unsafe { Box::from_raw(raw as *mut ThreadStart) };
    let code = (start.f)(start.arg);
    drop(start);
    exit(code)
}

/// A joinable spawned thread.
pub struct JoinHandle {
    tid: u64,
}

impl JoinHandle {
    pub fn tid(&self) -> u64 {
        self.tid
    }

    /// Block until the thread exits; returns its exit code.
    pub fn join(self) -> Result<i32, i64> {
        sys_thread_join(self.tid)
    }
}

/// Spawn `f(arg)` on a fresh kernel-allocated stack (guard page always on —
/// an overflow faults into the guard, contained to this process). The child
/// runs the TLS-free bootstrap ([`thread_bootstrap`]) first, so its control
/// block (bounds + errno seam) exists before `f` runs. Returns the join
/// handle or the kernel errno (`-EPERM` without the browser-runtime cap).
pub fn spawn(
    f: extern "C" fn(usize) -> i32,
    arg: usize,
    stack_size: usize,
) -> Result<JoinHandle, i64> {
    let start = Box::into_raw(Box::new(ThreadStart { f, arg }));
    match sys_thread_create(thread_bootstrap, start as usize, stack_size, true) {
        Ok(tid) => Ok(JoinHandle { tid }),
        Err(e) => {
            // SAFETY: creation failed — the kernel never ran the bootstrap,
            // so ownership never left this function.
            drop(unsafe { Box::from_raw(start) });
            Err(e)
        }
    }
}

/// OT-61 C1.1 (blocker E negative control): spawn `f(arg)` WITHOUT the
/// bootstrap — the kernel enters `f` directly, so no control block is
/// established. A thread spawned this way must observe [`control_block`] ==
/// `None`; the falsifiable proof that the bootstrap is what installs it.
/// Test-only.
#[cfg(feature = "neo-test")]
pub fn spawn_no_bootstrap(
    f: extern "C" fn(usize) -> !,
    arg: usize,
    stack_size: usize,
) -> Result<u64, i64> {
    sys_thread_create(f, arg, stack_size, true)
}
