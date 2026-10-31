// SPDX-License-Identifier: MPL-2.0
//! OT-61 C0.5 — the `neoplutarch` arm of `std::sys::alloc`.
//!
//! Overlaid into `library/std/src/sys/alloc/neoplutarch.rs`. Provides the
//! `impl GlobalAlloc for System` that `std` requires for the target. The
//! real-`std` smoke installs its own `#[global_allocator]`
//! (`neo_std_pal::SyscallHeap`), so `System` is not the live allocator there;
//! it is still a correct allocator if a program uses it. It is a minimal
//! **leaky** map-region allocator: one `SYS_MAP_REGION` per allocation, and
//! dealloc leaks (C0.5 has no `sys_unmap`, the same leak-until-exit discipline
//! as the PAL heap; A2's per-process quota is a reserved rung). Failure is
//! loud: a null return makes std call `handle_alloc_error`, which aborts
//! through `abort_internal` (exit 134).
//!
//! The syscall goes through `crate::sys::neoplutarch_rt` (the overlay's single
//! A4-gated trampoline). A1: the mapping is R+W+**NX**.

use crate::alloc::{GlobalAlloc, Layout, System};
use crate::sys::neoplutarch_rt;

const PAGE: usize = 4096;

// SAFETY: each allocation gets its own fresh, exclusively-owned, zeroed,
// page-aligned map-region span; dealloc leaks (C0.5 has no unmap).
// The attribute is upstream's, on every platform's impl: it is required once
// std is built as a known target (OT-61 C2.3b), not only `restricted_std`.
#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SYS_MAP_REGION returns page-aligned memory, which satisfies any
        // alignment <= PAGE. Refuse larger alignments (null => loud
        // handle_alloc_error) rather than hand back a mis-aligned pointer.
        if layout.align() > PAGE {
            return crate::ptr::null_mut();
        }
        let Some(size) = layout.size().checked_add(PAGE - 1) else {
            return crate::ptr::null_mut();
        };
        let size = size & !(PAGE - 1);
        let size = if size == 0 { PAGE } else { size };
        neoplutarch_rt::map_region(size)
    }

    #[inline]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Leak: no sys_unmap in C0.5 (reserved with the C1 region tracker).
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SYS_MAP_REGION already zeroes.
        unsafe { self.alloc(layout) }
    }
}
