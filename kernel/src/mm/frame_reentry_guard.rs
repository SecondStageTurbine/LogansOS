// SPDX-License-Identifier: MPL-2.0
//! Per-CPU reentrancy guard for the global frame allocator (debug builds only).
//!
//! OT-24: the original guard was a single global `AtomicBool`, so any
//! concurrent entry of `with_frame_alloc` from a second AP during SMP bring-up
//! looked like same-CPU reentry and panicked — a debug-only false positive
//! (absorbed by the test harness's retry), never a release bug, since release
//! builds compile no guard at all. The fix makes the guard *per-CPU*: an
//! `[AtomicBool; N]` indexed by `current_cpu_id()`. Same-CPU reentry — the real
//! bug the guard exists to catch — still panics; two different CPUs each
//! entering independently do not trip each other.
//!
//! This file is deliberately self-contained (no `crate::` dependencies, only
//! `core::sync::atomic`) so the host regression suite
//! `tools/frame-guard-logic-tests` can `#[path]`-mount this exact source and
//! exercise the real guard logic with *simulated* CPU ids — the independent
//! oracle for the cross-CPU-no-false-panic / same-CPU-still-panics behavior,
//! which cannot be expressed on the host against the inline `gs:[24]` read.

use core::sync::atomic::{AtomicBool, Ordering};

/// Per-CPU "inside `with_frame_alloc`" flags. `N` is the maximum number of CPUs;
/// the kernel instantiates it as `ReentryGuard<{ per_cpu::MAX_CPUS }>`.
pub struct ReentryGuard<const N: usize> {
    in_use: [AtomicBool; N],
}

impl<const N: usize> ReentryGuard<N> {
    /// Construct a guard with every CPU marked "outside the allocator".
    /// `const` so it can initialize a `static`.
    pub const fn new() -> Self {
        Self {
            in_use: [const { AtomicBool::new(false) }; N],
        }
    }

    /// Mark `cpu` as inside `with_frame_alloc`.
    ///
    /// Panics if `cpu` is already inside (same-CPU reentry): closing over
    /// `with_frame_alloc` from within its own closure would re-lock the
    /// `FRAME_ALLOC` mutex the caller already holds and spin forever, and would
    /// corrupt the intrusive free-list. Concurrent entry from a *different*
    /// `cpu` is fine — each index is independent.
    pub fn enter(&self, cpu: usize) {
        assert!(
            cpu < N,
            "frame reentry guard: cpu id {cpu} out of range (N = {N})"
        );
        assert!(
            !self.in_use[cpu].swap(true, Ordering::AcqRel),
            "with_frame_alloc reentered on CPU {cpu} — frame-allocator API is non-reentrant"
        );
    }

    /// Mark `cpu` as no longer inside `with_frame_alloc`.
    pub fn exit(&self, cpu: usize) {
        self.in_use[cpu].store(false, Ordering::Release);
    }
}

impl<const N: usize> Default for ReentryGuard<N> {
    fn default() -> Self {
        Self::new()
    }
}
