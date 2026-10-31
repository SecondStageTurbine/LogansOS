// SPDX-License-Identifier: MPL-2.0
//! OT-61 browser C0 gate — **`std-pal-smoke`** (Janus's falsifiable go/no-go).
//!
//! Proves, on the first-class `x86_64-unknown-neoplutarch` target, that the
//! LogansOS PAL's **wired primitives close**:
//! - **print** — `neo_std_pal::write_fd(STDOUT, …)` (raw `SYS_WRITE`);
//! - **allocate + free** — `Box`/`Vec` over the `SyscallHeap` global allocator
//!   (raw `SYS_MAP_REGION`, R+W+**NX** — A1: no W^X);
//! - **exit cleanly** — `neo_std_pal::exit(0)` (raw `SYS_EXIT`).
//!
//! It also proves an **unsupported PAL entry fails DETERMINISTICALLY and
//! LOUDLY** (the `unsupported-demo` feature calls a C1 stub → exit 90 + the
//! named symbol on stderr) — the "loud absence" property, a PASS not a defect.
//!
//! **Tier note (honest scope).** This is the *primitive tier*: it stands on
//! `neo-std-pal` directly (NOT libneo, proving the PAL is self-standing) and is
//! `no_std` because the **real-`std`-linked** tier requires the rust-src
//! `library/std/src/sys/*` overlay (the C0-completion sub-rung — its exact
//! remaining surface is enumerated in the C0 FINDINGS). The primitive tier
//! proves the wiring + the loud-absence property that the std tier then inherits.

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use neo_std_pal::{STDOUT_FD, SyscallHeap};

extern crate alloc;
use alloc::vec::Vec;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

/// A panic in the smoke is a deterministic loud exit (never a silent hang).
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    neo_std_pal::write_stderr(b"[ std-pal-smoke ] PANIC\n");
    neo_std_pal::exit(101)
}

/// The allocator-error path is likewise a deterministic loud exit.
#[alloc_error_handler]
fn alloc_error(_layout: core::alloc::Layout) -> ! {
    neo_std_pal::write_stderr(b"[ std-pal-smoke ] ALLOC ERROR\n");
    neo_std_pal::exit(102)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // 1. print
    neo_std_pal::write_fd(STDOUT_FD, b"[ std-pal-smoke ] print: ok\n");

    // 2. allocate + free (over the NX SyscallHeap)
    let mut v: Vec<u32> = Vec::new();
    for i in 0..1024u32 {
        v.push(i.wrapping_mul(2654435761)); // Knuth hash — nontrivial, forces real heap use
    }
    let checksum = v.iter().fold(0u32, |a, &x| a ^ x);
    drop(v); // free back to the heap
    if checksum == 0 {
        // Astronomically unlikely; guards against the loop being optimized out.
        neo_std_pal::write_stderr(b"[ std-pal-smoke ] alloc checksum zero (unexpected)\n");
        neo_std_pal::exit(103);
    }
    neo_std_pal::write_fd(STDOUT_FD, b"[ std-pal-smoke ] heap alloc+free: ok\n");

    // The loud-absence demonstration: a deliberately-unsupported C1 PAL entry
    // must fail deterministically by name (exit 90). Default build skips it and
    // exits 0 — the two together prove BOTH gate properties.
    #[cfg(feature = "unsupported-demo")]
    {
        neo_std_pal::write_fd(
            STDOUT_FD,
            b"[ std-pal-smoke ] calling an unsupported C1 PAL entry...\n",
        );
        neo_std_pal::thread_new_unsupported(); // -> exit 90 + named symbol on stderr
    }

    // 3. exit cleanly
    neo_std_pal::write_fd(STDOUT_FD, b"[ std-pal-smoke ] PASS: print+alloc+exit\n");
    neo_std_pal::exit(0)
}
