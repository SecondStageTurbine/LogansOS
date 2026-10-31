// SPDX-License-Identifier: MPL-2.0
//! OT-61 browser C0.5 — the **panic arm** smoke.
//!
//! A real-`std` binary (same build as `std-real-smoke`) that panics on purpose.
//! Under `panic=abort` the path is std's default panic hook (message to
//! `panic_output()`, which the overlay routes to stderr) → `panic_abort` →
//! `std::rt::__rust_abort` → `sys::abort_internal`, which the overlay routes to
//! `SYS_EXIT(134)`. The kernel scenario passes only if this process exits
//! **134**: not 0 (the panic was swallowed), and not a hang (std's stock
//! `abort_internal` is `ud2`, and the kernel answers a user `#UD` by halting
//! the machine).

#![no_main]
#![feature(restricted_std)]

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("[ std-real-panic ] about to panic (expect exit 134)");
    let v: Vec<u8> = Vec::new();
    // Index out of bounds: a real std panic, not a direct `panic!` call.
    let byte = v[std::hint::black_box(3)];
    println!("[ std-real-panic ] FAIL: the panic returned ({byte})");
    std::process::exit(1);
}
