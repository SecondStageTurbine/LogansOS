// SPDX-License-Identifier: MPL-2.0
//! `prefill_perf`'s per-process "last surface commit" slot: the pid and the
//! commit's TSC in ONE word, so a single store publishes both and two
//! processes sharing a slot can only evict each other, never mix.
//!
//! Pure: host-tested by `tools/ai-ops-tests` through `#[path]`.

/// The pid's share of the word (its low 20 bits). The TSC keeps its high 44
/// bits: a resolution of 2^20 cycles (1.05 ms at 1 GHz, 0.52 ms at 2 GHz).
pub const PID_MASK: u64 = (1 << 20) - 1;

/// The slot word for `pid`'s commit at `tsc`.
pub const fn commit_word(pid: u64, tsc: u64) -> u64 {
    (tsc & !PID_MASK) | (pid & PID_MASK)
}

/// The commit TSC a slot word holds for `pid`; `None` when the slot is empty
/// or holds another process's commit (it was evicted).
pub const fn commit_tsc(word: u64, pid: u64) -> Option<u64> {
    if word != 0 && word & PID_MASK == pid & PID_MASK {
        Some(word & !PID_MASK)
    } else {
        None
    }
}
