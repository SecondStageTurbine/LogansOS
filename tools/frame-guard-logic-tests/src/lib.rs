// SPDX-License-Identifier: MPL-2.0
//! Host regression tests for the frame allocator's per-CPU reentrancy guard.
//!
//! OT-24: proves on the host — against the *real* guard source, mounted via
//! `#[path]` — that the per-CPU guard (a) does NOT false-trip when two distinct
//! CPUs enter `with_frame_alloc` concurrently (the bug the single global
//! `AtomicBool` caused during SMP bring-up), and (b) STILL panics on genuine
//! same-CPU reentry, which is the guard's reason to exist.
//!
//! The kernel reads the CPU id from `gs:[24]` (`current_cpu_id()`), which cannot
//! run on the host; the guard is therefore parameterized by a CPU index so the
//! host can simulate two distinct `current_cpu_id()` values entering the guard.

#![allow(dead_code)]

#[path = "../../../kernel/src/mm/frame_reentry_guard.rs"]
mod frame_reentry_guard;

#[cfg(test)]
mod tests {
    use super::frame_reentry_guard::ReentryGuard;

    /// OT-24 regression: two *different* CPUs entering concurrently must NOT
    /// trip each other's guard. With the old global `AtomicBool` the second
    /// `enter` observed `true` and panicked; with the per-CPU array it does not.
    #[test]
    fn distinct_cpus_do_not_false_trip() {
        let guard: ReentryGuard<4> = ReentryGuard::new();
        guard.enter(0);
        guard.enter(1); // different CPU, cpu 0 still inside — must NOT panic
        guard.enter(2);
        guard.enter(3);
        guard.exit(0);
        guard.exit(1);
        guard.exit(2);
        guard.exit(3);
    }

    /// The guard's actual purpose: genuine same-CPU reentry (a second `enter`
    /// with no intervening `exit`) still panics.
    #[test]
    #[should_panic(expected = "reentered on CPU 2")]
    fn same_cpu_reentry_still_panics() {
        let guard: ReentryGuard<4> = ReentryGuard::new();
        guard.enter(2);
        guard.enter(2); // same CPU, not exited — must panic
    }

    /// A CPU may re-enter after it exits — the flag is cleared on `exit`.
    #[test]
    fn reentry_after_exit_is_allowed() {
        let guard: ReentryGuard<2> = ReentryGuard::new();
        guard.enter(1);
        guard.exit(1);
        guard.enter(1); // fine: the previous occupancy was released
        guard.exit(1);
    }

    /// Each CPU's flag is independent: exiting one CPU does not release another.
    #[test]
    #[should_panic(expected = "reentered on CPU 0")]
    fn exit_of_one_cpu_does_not_release_another() {
        let guard: ReentryGuard<4> = ReentryGuard::new();
        guard.enter(0);
        guard.enter(1);
        guard.exit(1); // releasing cpu 1 must not release cpu 0
        guard.enter(0); // cpu 0 still inside — must panic
    }

    /// Out-of-range CPU id is rejected with a clear message (the bounds assert
    /// the fix added) rather than an opaque index-out-of-bounds panic.
    #[test]
    #[should_panic(expected = "out of range")]
    fn out_of_range_cpu_is_rejected() {
        let guard: ReentryGuard<2> = ReentryGuard::new();
        guard.enter(7);
    }
}
