// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.0 — real `std` randomness (`std_entropy` scenario, and the
//! kill-switch spawn in `std_real_smoke`).
//!
//! On a kernel with `kernel-rng`, `std`'s `fill_bytes` (behind `HashMap`'s
//! `RandomState`) reads the kernel CSPRNG through `SYS_GETRANDOM`. Exit 0 when
//! every check passes, else the check number. On a kernel without `kernel-rng`
//! the first `RandomState` aborts with 134: the same outcome as C0.5's
//! unsupported `fill_bytes`, which the `std_real_smoke` scenario requires.

#![no_main]
#![feature(restricted_std)]
#![feature(random)]

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::random::{DefaultRandomSource, RandomSource};

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ std-entropy ]";

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok: {what}");
    } else {
        println!("{TAG} FAIL({code}): {what}");
        std::process::exit(code);
    }
}

fn hash_with(state: &RandomState, value: u64) -> u64 {
    let mut h = state.build_hasher();
    value.hash(&mut h);
    h.finish()
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("{TAG} std RandomState / HashMap over SYS_GETRANDOM");
    // 1. A HashMap with std's default (random) hasher works.
    let mut m: HashMap<u32, u32> = HashMap::new();
    for i in 0..1_000 {
        m.insert(i, i * 3);
    }
    check(
        1,
        "HashMap::new (RandomState) inserts and finds 1000 keys",
        m.len() == 1_000 && (0..1_000).all(|i| m.get(&i) == Some(&(i * 3))),
    );
    // 2. Independent RandomStates hash the same value differently. (std keys
    //    them from one per-thread draw plus a counter, so this shows the
    //    keying works, not that the source is random; check 4 does that.)
    let states: Vec<RandomState> = (0..8).map(|_| RandomState::new()).collect();
    let mut hashes: Vec<u64> = states.iter().map(|s| hash_with(s, 42)).collect();
    hashes.sort_unstable();
    hashes.dedup();
    check(
        2,
        "8 RandomStates hash 42 to 8 different values",
        hashes.len() == 8,
    );
    // 4. The source itself: a fill larger than the kernel's 64 KiB per-call
    //    ceiling (the overlay's chunk loop: four calls, the last partial).
    //    Every chunk has nonzero bytes, and two fills differ. (Check 3, a
    //    RandomState against the all-zero-key hasher, was removed: std's key
    //    counter made it pass even with no entropy at all.)
    let mut a = vec![0u8; 200 * 1024];
    let mut b = vec![0u8; 200 * 1024];
    DefaultRandomSource.fill_bytes(&mut a);
    DefaultRandomSource.fill_bytes(&mut b);
    let every_chunk_filled = a.chunks(64 * 1024).all(|c| c.iter().any(|&x| x != 0));
    check(
        4,
        "a 200 KiB fill (4 kernel calls) leaves no chunk all-zero; two fills differ",
        every_chunk_filled && a != b,
    );
    println!("{TAG} PASS: std entropy via SYS_GETRANDOM");
    std::process::exit(0);
}
