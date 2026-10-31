// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.2 (P3): per-thread TLS keys over FS.base, with destructors at
//! thread exit.
//!
//! The kernel ABI is one setter (`SYS_THREAD_SET_TLS`); everything else lives
//! in userland, as the amended P3 freezes it. Both consumer key APIs map onto
//! it one to one: `std`'s `thread_local::key::{create, set, get, destroy}` and
//! pthread's `pthread_key_create`/`setspecific`/`getspecific`/`key_delete`.
//!
//! **C1.3: one implementation.** The key layer itself is
//! `userland/neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs`, the same file
//! the std overlay installs as `std::sys::neoplutarch_rt::tls`; this module
//! includes it and supplies the three things it takes from its parent: the
//! allocator (libneo's global heap) and `SYS_THREAD_SET_TLS`. See that file for
//! the layout, the key registry and the destructor rules.

use core::alloc::Layout;

#[path = "../../neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs"]
mod shared;

pub use shared::*;

/// The shared layer's allocator: libneo's global heap.
///
/// # Safety
/// `layout` has a non-zero size.
unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    // SAFETY: the caller's contract.
    unsafe { alloc::alloc::alloc_zeroed(layout) }
}

/// # Safety
/// `ptr` came from [`alloc_zeroed`] with this `layout`.
unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    // SAFETY: the caller's contract.
    unsafe { alloc::alloc::dealloc(ptr, layout) }
}

/// `SYS_THREAD_SET_TLS(base)`: 0 or a negative errno.
fn set_tls(base: u64) -> i64 {
    match crate::sys_thread_set_tls(base) {
        Ok(()) => 0,
        Err(e) => e,
    }
}
