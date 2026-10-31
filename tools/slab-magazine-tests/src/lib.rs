// SPDX-License-Identifier: MPL-2.0
//! Host-tier regression for the per-CPU slab `Magazine` ring (OT-29).
//!
//! OT-29 was a stride bug in `Magazine::push`/`pop`: the ring stores `*mut T`
//! element pointers in a buffer sized `array::<*mut T>(capacity)`, but `buffer`
//! is typed `*mut T`, so `(*buffer).add(i)` strided by `size_of::<T>()` instead
//! of the pointer size. For any `T > 8 bytes` the 2nd+ cached element wrote
//! **out of bounds**. The slab's own unit tests never caught it because they use
//! an 8-byte `TestObject` (where the wrong stride accidentally equals the
//! pointer size). This crate closes that gap with a **>8-byte element**.
//!
//! **Why a round-trip assertion alone is not enough (the subtle part):** the
//! stride bug is *symmetric* — `push` and `pop` both compute the same wrong
//! `i * size_of::<T>()` offset — so a FIFO push→pop reads each value back from
//! the very slot it was written to and **round-trips correctly even on the buggy
//! code**, while silently scribbling out-of-bounds heap. The defect is the OOB
//! *write*, invisible to the ring's public API. So this test also installs a
//! **guard (red-zone) global allocator**: inside a short `TRACKING` window it
//! grows exactly the one `Magazine::new` buffer allocation with a sentinel-filled
//! tail and records it; after the pushes the test verifies that red-zone **in
//! place** (the buggy stride's out-of-bounds write corrupts it) and then drops
//! the magazine for real (the dealloc hook rebuilds the grown layout for the
//! tracked block, so `Magazine::Drop`'s plain-layout free stays sound). A
//! deterministic catch in plain `cargo test` — no Miri, no change to
//! `kernel/src/mm/slab.rs`.
//!
//! The real `Magazine` is `#[path]`-mounted from `kernel/src/mm/slab.rs` (it is
//! self-contained — `alloc` + `UnsafeCell`, no kernel deps); the kernel
//! dependencies the *rest* of `slab.rs` references are satisfied by the shims
//! below so the file compiles unchanged on the host.

#![allow(dead_code)]
#![allow(unexpected_cfgs)]
// The mounted `slab.rs` re-imports `crate::serial_println`, which is redundant
// with the shim `macro_rules!` already in textual scope (cannot edit slab.rs).
#![allow(unused_imports)]

extern crate alloc;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// === Guard (red-zone) allocator — detects the OOB write deterministically =====
//
// To avoid growing every allocation in the process, the allocator only
// red-zones allocations made inside an explicit `TRACKING` window. The test
// opens the window for exactly the one `Magazine::new` buffer allocation,
// records it, then verifies the red-zone in place (not on free). When the
// magazine is dropped, `Magazine::Drop` (fixed under OT-32) deallocs the
// buffer with its ORIGINAL (ungrown) layout — so `dealloc` recognizes the
// tracked pointer and rebuilds the grown layout before freeing. Every other
// allocation in the process is a plain, layout-consistent `System`
// alloc/dealloc.

const GUARD_BYTES: usize = 128;
const SENTINEL: u8 = 0xC7;

static TRACKING: AtomicBool = AtomicBool::new(false);
static TRACKED_PTR: AtomicUsize = AtomicUsize::new(0);
static TRACKED_SIZE: AtomicUsize = AtomicUsize::new(0);

/// Open the red-zone window (the next allocation is the tracked one).
pub fn track_on() {
    TRACKED_PTR.store(0, Ordering::SeqCst);
    TRACKED_SIZE.store(0, Ordering::SeqCst);
    TRACKING.store(true, Ordering::SeqCst);
}
/// Close the window.
pub fn track_off() {
    TRACKING.store(false, Ordering::SeqCst);
}
/// The tracked allocation's address (0 = nothing captured). Lets tests assert
/// the capture actually happened, so a missed window can't pass vacuously.
pub fn tracked_ptr() -> usize {
    TRACKED_PTR.load(Ordering::SeqCst)
}
/// Verify the tracked allocation's red-zone in place. Returns true if it was
/// overwritten (an out-of-bounds write past the buffer).
pub fn tracked_guard_tripped() -> bool {
    let p = TRACKED_PTR.load(Ordering::SeqCst);
    let size = TRACKED_SIZE.load(Ordering::SeqCst);
    if p == 0 {
        return false;
    }
    let base = p as *const u8;
    for i in 0..GUARD_BYTES {
        // SAFETY: `[size, size+GUARD)` is the red-zone of a still-live grown alloc.
        if unsafe { *base.add(size + i) } != SENTINEL {
            return true;
        }
    }
    false
}

// === OT-32: Magazine::Drop free-the-buffer check (thread-local, isolated) =======
//
// The Drop test must observe WHICH pointer `Magazine::Drop` frees and assert it
// is the heap buffer, not `self.buffer.get()` (the `UnsafeCell`'s own address).
// This state is THREAD-LOCAL and const-initialised (alloc-free), so the global
// allocator hook acts only on the test's own thread: a concurrent test or the
// libtest harness allocating/freeing on another thread is never recorded,
// grown, or swallowed. No mutex needed, and the existing red-zone test (which
// uses the global `TRACKING` path) is untouched. The `Magazine::new` buffer is
// captured PLAIN (not grown) so `Drop`'s `dealloc` layout matches and a correct
// free succeeds; a wrong pointer is recorded and SWALLOWED (freeing a non-heap
// pointer through `System` would be UB — so the pre-fix bug fails the assert
// cleanly instead of aborting the process).
thread_local! {
    static DROP_TRACK: Cell<bool> = const { Cell::new(false) };
    static DROP_CHECK: Cell<bool> = const { Cell::new(false) };
    static DROP_BUFFER_PTR: Cell<usize> = const { Cell::new(0) };
    static DROP_FREED_BUFFER: Cell<bool> = const { Cell::new(false) };
    static DROP_FREED_WRONG: Cell<usize> = const { Cell::new(0) };
}

/// Reset the per-thread Drop-test state.
pub fn drop_reset() {
    DROP_BUFFER_PTR.with(|c| c.set(0));
    DROP_FREED_BUFFER.with(|c| c.set(false));
    DROP_FREED_WRONG.with(|c| c.set(0));
}
/// Capture the next allocation on this thread as the tracked buffer (plain).
pub fn drop_track_on() {
    DROP_TRACK.with(|c| c.set(true));
}
pub fn drop_track_off() {
    DROP_TRACK.with(|c| c.set(false));
}
/// Intercept deallocations on this thread (the `Magazine::Drop` free).
pub fn drop_check_on() {
    DROP_CHECK.with(|c| c.set(true));
}
pub fn drop_check_off() {
    DROP_CHECK.with(|c| c.set(false));
}
pub fn drop_buffer_ptr() -> usize {
    DROP_BUFFER_PTR.with(|c| c.get())
}
/// True iff `Magazine::Drop` deallocated the captured buffer pointer.
pub fn drop_freed_buffer() -> bool {
    DROP_FREED_BUFFER.with(|c| c.get())
}
/// The (non-buffer) pointer `Magazine::Drop` tried to free, if any (0 = none).
pub fn drop_freed_wrong() -> usize {
    DROP_FREED_WRONG.with(|c| c.get())
}

struct GuardAlloc;

// SAFETY: delegates to the OS `System` allocator (no re-entrancy). Only the one
// tracked allocation is grown; when it is freed (by `Magazine::Drop`, which
// passes the original ungrown layout), `dealloc` matches the pointer and
// rebuilds the same grown layout used at alloc time. Every other (ptr, layout)
// pair is passed to `System` unchanged, so the alloc/dealloc layouts always
// match. The OT-32 Drop hooks touch only const-init thread-locals (alloc-free)
// and free the real buffer with its original layout.
unsafe impl GlobalAlloc for GuardAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return unsafe { System.alloc(layout) };
        }
        // OT-32 Drop test: capture this thread's one tracked buffer alloc PLAIN
        // (not grown), so Drop's `dealloc` layout matches the allocation.
        if DROP_TRACK.with(|c| c.get()) {
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                DROP_BUFFER_PTR.with(|c| c.set(p as usize));
                DROP_TRACK.with(|c| c.set(false));
            }
            return p;
        }
        if !TRACKING.load(Ordering::SeqCst) {
            return unsafe { System.alloc(layout) };
        }
        let grown = match Layout::from_size_align(layout.size() + GUARD_BYTES, layout.align()) {
            Ok(l) => l,
            Err(_) => return unsafe { System.alloc(layout) },
        };
        let p = unsafe { System.alloc(grown) };
        if !p.is_null() {
            unsafe { core::ptr::write_bytes(p.add(layout.size()), SENTINEL, GUARD_BYTES) };
            TRACKED_PTR.store(p as usize, Ordering::SeqCst);
            TRACKED_SIZE.store(layout.size(), Ordering::SeqCst);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // OT-32 Drop test (this thread only): free the real buffer; record +
        // SWALLOW any other pointer (the pre-fix cell/stack address) so a
        // wrong-pointer free can't UB-abort the test.
        if DROP_CHECK.with(|c| c.get()) {
            if ptr as usize == DROP_BUFFER_PTR.with(|c| c.get()) {
                DROP_FREED_BUFFER.with(|c| c.set(true));
                unsafe { System.dealloc(ptr, layout) };
            } else {
                DROP_FREED_WRONG.with(|c| c.set(ptr as usize));
            }
            return;
        }
        // The tracked block was allocated GROWN (red-zone tail), but its owner
        // (`Magazine::Drop`) deallocs with the original ungrown layout — rebuild
        // the grown layout so the (ptr, layout) pair matches the allocation.
        if ptr as usize == TRACKED_PTR.load(Ordering::SeqCst) {
            TRACKED_PTR.store(0, Ordering::SeqCst);
            TRACKED_SIZE.store(0, Ordering::SeqCst);
            if let Ok(grown) = Layout::from_size_align(layout.size() + GUARD_BYTES, layout.align())
            {
                unsafe { System.dealloc(ptr, grown) };
                return;
            }
        }
        // Every other block was allocated with its exact `layout`.
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GUARD_GLOBAL: GuardAlloc = GuardAlloc;

// === Shims for the kernel deps the rest of slab.rs references ==================
//
// Only `Magazine` is exercised; these make `slab.rs` compile on the host (the
// Slab/SlabPerCpuState machinery is compiled-but-unused). Empty structs are
// `Send + Sync`, satisfying the slab's trait bounds.

macro_rules! serial_println {
    ($($t:tt)*) => {{}};
}
pub(crate) use serial_println;

pub mod sched {
    pub mod thread {
        pub struct Thread;
    }
}
pub mod arch {
    pub mod x86_64 {
        pub mod per_cpu {
            pub fn slab_state() -> *mut () {
                core::ptr::null_mut()
            }
            pub fn set_slab_state_for_cpu(_cpu_id: u32, _state: *mut ()) {}
            pub fn current_thread_id() -> u64 {
                0
            }
        }
    }
}
pub mod mm {
    /// Shim for the `debug_assert!` in `slab::init_per_cpu` (not exercised here).
    pub fn with_frame_alloc<F: FnOnce(&()) -> bool>(f: F) -> bool {
        f(&())
    }
}

#[path = "../../../kernel/src/mm/slab.rs"]
mod slab;

// === The regression =============================================================

#[cfg(test)]
mod tests {
    use super::slab::Magazine;
    use super::{
        drop_buffer_ptr, drop_check_off, drop_check_on, drop_freed_buffer, drop_freed_wrong,
        drop_reset, drop_track_off, drop_track_on, track_off, track_on, tracked_guard_tripped,
        tracked_ptr,
    };

    /// 24 bytes — larger than a pointer, so the OT-29 stride bug strides wide.
    #[derive(Clone, Copy)]
    #[repr(C)]
    struct Big {
        a: u64,
        b: u64,
        c: u64,
    }

    #[test]
    fn magazine_large_element_push_pop_no_oob() {
        assert_eq!(core::mem::size_of::<Big>(), 24, "Big must be >8 bytes");

        // Open the red-zone window for exactly the one `Magazine::new` buffer
        // allocation. capacity 9 ⇒ a 72-byte (9 × 8) pointer buffer; the red-zone
        // sits at [72, 200). With the OT-29 stride bug, push index 3 (the 4th)
        // writes at byte offset 3 × size_of::<Big>() = 72 — into the red-zone —
        // and indices 4/5 at 96/120 (also inside it). The fixed code strides by
        // 8, writing at 0/8/16/24/32/40, all inside the buffer.
        track_on();
        let mag: Magazine<Big> = Magazine::new(9);
        track_off();
        assert_ne!(
            tracked_ptr(),
            0,
            "the Magazine::new buffer alloc was not captured (vacuous test)"
        );

        let ptrs: [*mut Big; 6] = [
            0x1000 as *mut Big,
            0x2000 as *mut Big,
            0x3000 as *mut Big,
            0x4000 as *mut Big,
            0x5000 as *mut Big,
            0x6000 as *mut Big,
        ];
        for p in ptrs {
            assert!(
                !mag.push(p),
                "unexpected magazine overflow (cap 9, 6 pushes)"
            );
        }
        assert_eq!(mag.len(), 6);

        // FIFO round-trip. The stride bug is *symmetric* (push and pop use the
        // same wrong offset), so this passes even on the buggy code — the OOB
        // *write*, caught by the red-zone check below, is the actual defect.
        for p in ptrs {
            assert_eq!(mag.pop(), Some(p), "FIFO round-trip value mismatch");
        }
        assert!(mag.is_empty());

        // Verify the red-zone while the buffer is still live, then drop the
        // magazine for real — `Magazine::Drop` (fixed under OT-32) frees the
        // buffer with its ungrown layout, and the dealloc hook rebuilds the
        // grown layout for the tracked pointer. Historical note: this test
        // used to `mem::forget` the magazine to dodge the pre-fix Drop bug
        // (it freed the `UnsafeCell`'s address, not the buffer).
        let tripped = tracked_guard_tripped();
        drop(mag);

        assert!(
            !tripped,
            "Magazine wrote OUT OF BOUNDS for a >8-byte element — the OT-29 \
             stride bug: push/pop strided by size_of::<T>() instead of the \
             pointer size, overrunning the capacity*8-byte buffer."
        );
    }

    /// OT-32: `Magazine::Drop` must free the heap buffer, not the `UnsafeCell`'s
    /// own address. This FAILS against the pre-fix `dealloc(self.buffer.get(),…)`
    /// (which frees the cell/stack address — `drop_freed_buffer()` stays false)
    /// and PASSES after the fix (`*self.buffer.get()` — the buffer is freed).
    /// Verified the fail-against-buggy direction by reverting the one-char fix.
    #[test]
    fn magazine_drop_frees_the_buffer_not_the_cell() {
        drop_reset();

        // Capture the one `Magazine::new` buffer allocation (plain, not grown, so
        // the Drop dealloc layout matches and a correct free succeeds).
        drop_track_on();
        let mag: Magazine<Big> = Magazine::new(4);
        drop_track_off();
        let buffer_ptr = drop_buffer_ptr();
        assert_ne!(
            buffer_ptr, 0,
            "the Magazine::new buffer alloc was not captured"
        );

        // Exercise the ring so Drop runs against a used magazine (push/pop add no
        // allocations — they store raw pointers into the existing buffer).
        assert!(!mag.push(0x1000 as *mut Big));
        assert!(!mag.push(0x2000 as *mut Big));
        assert_eq!(mag.pop(), Some(0x1000 as *mut Big));

        // Drop with dealloc interception on: the hook frees the buffer for real
        // ONLY if Drop hands it the buffer pointer; any other pointer (the
        // pre-fix cell/stack address) is recorded and swallowed (no UB).
        drop_check_on();
        drop(mag);
        drop_check_off();

        assert!(
            drop_freed_buffer(),
            "Magazine::Drop did NOT free the heap buffer (OT-32): it deallocated \
             {:#x} instead of the buffer at {:#x}. The pre-fix bug freed \
             `self.buffer.get()` (the UnsafeCell's own address), not \
             `*self.buffer.get()` (the buffer it points at).",
            drop_freed_wrong(),
            buffer_ptr,
        );
    }
}
