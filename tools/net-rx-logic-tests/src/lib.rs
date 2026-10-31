// SPDX-License-Identifier: MPL-2.0
//! Host KATs for OT-96 R1, the wire receive demux (`net/rx_logic.rs`): one
//! hand-laid frame per accepted path, a malformed-input corpus with one case
//! per bound, and a deterministic mutation sweep that must never panic.

#[path = "../../../kernel/src/net/rx_logic.rs"]
pub mod rx_logic;

#[path = "../../../kernel/src/net/egress_logic.rs"]
pub mod egress_logic;

pub mod frames;

pub mod hardening_frames;

#[cfg(test)]
mod tests;

/// Chad #189: a counting allocator, per thread, so a KAT can measure one
/// call's allocations while other tests run.
#[cfg(test)]
mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static BYTES: Cell<usize> = const { Cell::new(0) };
    }

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = BYTES.try_with(|b| b.set(b.get() + layout.size()));
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
            let _ = BYTES.try_with(|b| b.set(b.get() + new));
            unsafe { System.realloc(ptr, layout, new) }
        }
    }

    #[global_allocator]
    static GLOBAL: Counting = Counting;

    /// Bytes this thread has allocated so far.
    pub fn bytes() -> usize {
        BYTES.with(|b| b.get())
    }
}

#[cfg(test)]
mod hardening_tests;
