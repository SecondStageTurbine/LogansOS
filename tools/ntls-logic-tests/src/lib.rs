// SPDX-License-Identifier: MPL-2.0
//! Host KAT for the OT-61 TLS key layer,
//! `userland/neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs` (the one file
//! std and libneo both use), mounted through `#[path]` so it runs the exact code
//! userland runs. It covers the rules amendment 1 sets for the destructor driver
//! (clear before the call, rescan while any ran, at most four passes) and the
//! generation check, on an explicit key block, and (C1.3) the initial thread's
//! static block, which needs no FS.base. The per-thread FS.base path runs in
//! QEMU (`threads_tls`, `std_threads`). One test, because the key registry is
//! process-wide.

#![allow(dead_code)]

#[path = "../../../userland/neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs"]
mod ntls;

// What the shared layer takes from its parent module (here, the crate root).

use std::alloc::Layout;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

/// `SYS_THREAD_SET_TLS`'s answer. The host never succeeds: a success would
/// switch the layer to reading `fs:[0]`, which on the host is not a control
/// block.
static SET_TLS_RESULT: AtomicI64 = AtomicI64::new(-1);
static SET_TLS_CALLS: AtomicUsize = AtomicUsize::new(0);
static LIVE_BLOCKS: AtomicUsize = AtomicUsize::new(0);

unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    LIVE_BLOCKS.fetch_add(1, Ordering::Relaxed);
    unsafe { std::alloc::alloc_zeroed(layout) }
}

unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    LIVE_BLOCKS.fetch_sub(1, Ordering::Relaxed);
    unsafe { std::alloc::dealloc(ptr, layout) }
}

fn set_tls(_base: u64) -> i64 {
    SET_TLS_CALLS.fetch_add(1, Ordering::Relaxed);
    SET_TLS_RESULT.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::ntls::*;
    use super::{LIVE_BLOCKS, SET_TLS_CALLS};
    use core::ptr;
    use core::sync::atomic::AtomicUsize;
    use core::sync::atomic::Ordering;

    static BLOCK: core::sync::atomic::AtomicPtr<KeyBlock> =
        core::sync::atomic::AtomicPtr::new(ptr::null_mut());
    static ONCE_KEY: AtomicUsize = AtomicUsize::new(0);
    static REARM_KEY: AtomicUsize = AtomicUsize::new(0);
    static FOREVER_KEY: AtomicUsize = AtomicUsize::new(0);
    static ONCE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static REARM_CALLS: AtomicUsize = AtomicUsize::new(0);
    static FOREVER_CALLS: AtomicUsize = AtomicUsize::new(0);
    static DELETED_CALLS: AtomicUsize = AtomicUsize::new(0);
    static SAW_UNCLEARED: AtomicUsize = AtomicUsize::new(0);

    fn block() -> &'static mut KeyBlock {
        unsafe { &mut *BLOCK.load(Ordering::Relaxed) }
    }

    fn put(key: Key, value: usize) {
        let generation = live_generation(key).expect("live key");
        block().slots[key] = Slot {
            value: value as *mut u8,
            generation,
        };
    }

    unsafe extern "C" fn once(_: *mut u8) {
        let key = ONCE_KEY.load(Ordering::Relaxed);
        if !block().slots[key].value.is_null() {
            SAW_UNCLEARED.fetch_add(1, Ordering::Relaxed);
        }
        ONCE_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets its key again on its first two calls, like std's key-path cleanup
    /// (DEFER, then RUN): it must run three times, in three passes.
    unsafe extern "C" fn rearm(_: *mut u8) {
        let key = REARM_KEY.load(Ordering::Relaxed);
        if !block().slots[key].value.is_null() {
            SAW_UNCLEARED.fetch_add(1, Ordering::Relaxed);
        }
        if REARM_CALLS.fetch_add(1, Ordering::Relaxed) < 2 {
            put(key, 2);
        }
    }

    /// Always sets its key again: the pass cap must stop it at four.
    unsafe extern "C" fn forever(_: *mut u8) {
        FOREVER_CALLS.fetch_add(1, Ordering::Relaxed);
        put(FOREVER_KEY.load(Ordering::Relaxed), 3);
    }

    unsafe extern "C" fn deleted(_: *mut u8) {
        DELETED_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn destructors_clear_first_iterate_cap_and_respect_generation() {
        let mut storage = Box::new(KeyBlock {
            slots: [Slot::EMPTY; MAX_KEYS],
        });
        BLOCK.store(&mut *storage, Ordering::Relaxed);

        // Key 0 is never handed out.
        let once_key = create(Some(once)).unwrap();
        assert_ne!(once_key, 0);
        ONCE_KEY.store(once_key, Ordering::Relaxed);
        let rearm_key = create(Some(rearm)).unwrap();
        REARM_KEY.store(rearm_key, Ordering::Relaxed);

        // A value set under a deleted generation: invisible to a key created
        // on the same index, and its destructor never runs.
        let stale = create(Some(deleted)).unwrap();
        put(stale, 9);
        assert!(delete(stale));
        assert!(!delete(stale), "deleting twice must fail");
        let reused = create(None).unwrap();
        assert_eq!(reused, stale, "the freed index is reused first");
        assert_ne!(
            block().slots[reused].generation,
            live_generation(reused).unwrap()
        );

        put(once_key, 1);
        put(rearm_key, 2);
        let passes = unsafe { run_destructors_on(block()) };
        assert_eq!(
            ONCE_CALLS.load(Ordering::Relaxed),
            1,
            "a non-rearming destructor runs once"
        );
        assert_eq!(
            REARM_CALLS.load(Ordering::Relaxed),
            3,
            "a rearming destructor runs again"
        );
        assert_eq!(passes, 3);
        assert_eq!(
            SAW_UNCLEARED.load(Ordering::Relaxed),
            0,
            "slots are cleared before the call"
        );
        assert_eq!(
            DELETED_CALLS.load(Ordering::Relaxed),
            0,
            "a deleted key's destructor ran"
        );
        assert!(block().slots[once_key].value.is_null());
        assert!(block().slots[rearm_key].value.is_null());

        // The cap: a destructor that always sets its key again runs exactly
        // DESTRUCTOR_ITERATIONS times and its last value is left behind.
        let forever_key = create(Some(forever)).unwrap();
        FOREVER_KEY.store(forever_key, Ordering::Relaxed);
        put(forever_key, 3);
        let passes = unsafe { run_destructors_on(block()) };
        assert_eq!(passes, DESTRUCTOR_ITERATIONS);
        assert_eq!(FOREVER_CALLS.load(Ordering::Relaxed), DESTRUCTOR_ITERATIONS);
        assert!(!block().slots[forever_key].value.is_null());

        // C1.3, the initial thread before FS.base is installed: keys work on
        // the static block, with no `fs:[0]` read.
        let k = create(Some(once)).unwrap();
        assert!(get(k).is_null());
        assert_eq!(LIVE_BLOCKS.load(Ordering::Relaxed), 0);
        assert!(set(k, 0x42 as *mut u8));
        assert_eq!(get(k), 0x42 as *mut u8);
        assert_eq!(
            LIVE_BLOCKS.load(Ordering::Relaxed),
            1,
            "one block, on first set"
        );
        // A refused SYS_THREAD_SET_TLS (no C1 capability) leaves the process
        // on the static block, values intact; so does a refused install_at,
        // which hands the keys back.
        assert_eq!(ensure_initial_installed(), Err(-1));
        assert_eq!(get(k), 0x42 as *mut u8);
        let mut spare = core::mem::MaybeUninit::<ThreadControlBlock>::uninit();
        assert_eq!(unsafe { install_at(spare.as_mut_ptr(), 0, 0) }, Err(-1));
        assert_eq!(get(k), 0x42 as *mut u8, "a failed install_at kept the keys");
        assert_eq!(SET_TLS_CALLS.load(Ordering::Relaxed), 2);
        // Thread-exit cleanup on the static block: the destructor runs once
        // with the slot already clear, and the block is freed.
        let before = ONCE_CALLS.load(Ordering::Relaxed);
        ONCE_KEY.store(k, Ordering::Relaxed);
        thread_exit_cleanup();
        assert_eq!(ONCE_CALLS.load(Ordering::Relaxed), before + 1);
        assert_eq!(LIVE_BLOCKS.load(Ordering::Relaxed), 0, "block freed");
        assert!(get(k).is_null());
        assert!(delete(k));

        // Exhaustion: create fails once every index is in use, and recovers
        // after a delete.
        let mut made = Vec::new();
        while let Some(k) = create(None) {
            made.push(k);
        }
        // Live before: once, rearm, reused, forever; usable indices are 1..MAX_KEYS.
        assert_eq!(made.len() + 4, MAX_KEYS - 1);
        assert!(delete(made[0]));
        assert_eq!(create(None), Some(made[0]));
    }
}
