// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-2: a record of the threads `pthread_create` made, for the
//! SpiderMonkey smoke's helper-thread checks
//! (`docs/THINK-2026-11-12-ot61-c3c2-helper-threads.md` §4.1).
//!
//! - `created`: `pthread_create` calls that started a thread.
//! - `exited`: those threads that returned from their start routine and ran
//!   their key destructors.
//! - `joined`: `pthread_join` calls that completed.
//! - `wakes` / `timeouts`: condvar waits **by those threads** that returned
//!   because they were signalled, or because their deadline passed. The main
//!   thread's waits, and those of threads std made, aren't counted.
//!
//! The wake count is what tells "the pool started" from "the pool did work":
//! SpiderMonkey's pool parks idle helpers on a condvar and signals one per
//! task (or all at shutdown), so a signalled return before shutdown means a
//! helper was handed a task. A wait that only ever times out (a missing
//! wake, rescued by its deadline: pitfall CP-58) counts as a timeout, never
//! as a wake. neo-crt's condvar returns only when its sequence moves or its
//! deadline passes (`seqcond.rs`), so there are no spurious wakes to count.
//!
//! Plain counters, no lock. Every update and read is `SeqCst`: the counts are
//! read across threads while the pool runs, and the cost doesn't matter.
//!
//! This module uses only `core`, so the host tests (`tools/neo-crt-logic-tests`)
//! mount it by path and test the code the target runs.

use core::sync::atomic::{AtomicU64, Ordering::SeqCst};

/// A copy of the counts at one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreadCounts {
    pub created: u64,
    pub exited: u64,
    pub joined: u64,
    pub wakes: u64,
    pub timeouts: u64,
}

pub struct ThreadRecord {
    created: AtomicU64,
    exited: AtomicU64,
    joined: AtomicU64,
    wakes: AtomicU64,
    timeouts: AtomicU64,
}

impl ThreadRecord {
    pub const fn new() -> Self {
        Self {
            created: AtomicU64::new(0),
            exited: AtomicU64::new(0),
            joined: AtomicU64::new(0),
            wakes: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
        }
    }

    pub fn created(&self) {
        self.created.fetch_add(1, SeqCst);
    }

    pub fn exited(&self) {
        self.exited.fetch_add(1, SeqCst);
    }

    pub fn joined(&self) {
        self.joined.fetch_add(1, SeqCst);
    }

    /// A condvar wait returned. `ours` is whether the waiting thread is one
    /// `pthread_create` made; other threads' waits aren't recorded.
    pub fn cond_returned(&self, ours: bool, timed_out: bool) {
        if !ours {
            return;
        }
        if timed_out {
            self.timeouts.fetch_add(1, SeqCst);
        } else {
            self.wakes.fetch_add(1, SeqCst);
        }
    }

    pub fn counts(&self) -> ThreadCounts {
        ThreadCounts {
            created: self.created.load(SeqCst),
            exited: self.exited.load(SeqCst),
            joined: self.joined.load(SeqCst),
            wakes: self.wakes.load(SeqCst),
            timeouts: self.timeouts.load(SeqCst),
        }
    }
}

impl Default for ThreadRecord {
    fn default() -> Self {
        Self::new()
    }
}
