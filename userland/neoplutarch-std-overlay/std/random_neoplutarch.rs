// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.0: `std::sys::random::neoplutarch`, std's entropy source on the
//! kernel CSPRNG (`SYS_GETRANDOM`).
//!
//! Overlaid into `library/std/src/sys/random/neoplutarch.rs` by
//! `apply-overlay.py`. `fill_bytes` is what `HashMap`'s `RandomState` (through
//! std's generic `hashmap_random_keys`) and any `std`-level randomness use.
//!
//! The kernel fills at most `MAX_GETRANDOM_LEN` (64 KiB) per call and never
//! returns partial bytes, so this loops in chunks of that size. A kernel built
//! without `kernel-rng` answers `ENOSYS`: then this panics by name, which under
//! `panic=abort` exits 134, exactly what C0.5's `unsupported::fill_bytes` did.
//! There is no fallback to a weaker source.

use crate::sys::neoplutarch_rt as rt;

/// The kernel's per-call ceiling (`kernel/src/rng/mod.rs`, `MAX_GETRANDOM_LEN`).
const CHUNK: usize = 64 * 1024;

pub fn fill_bytes(bytes: &mut [u8]) {
    for chunk in bytes.chunks_mut(CHUNK) {
        let r = rt::getrandom(chunk);
        if r != chunk.len() as i64 {
            panic!("neoplutarch: no entropy (SYS_GETRANDOM returned {r})");
        }
    }
}
