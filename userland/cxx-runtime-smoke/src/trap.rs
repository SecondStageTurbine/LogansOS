// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3a: `cxx-hardening-trap`, the hardening twin of `cxx-runtime-smoke`.
//!
//! Reads `std::vector<int>(4)[4]` through libc++. neo-crt builds libc++ with
//! the fast hardening mode on (`neo-crt/cxx/__config_site`), so the bounds
//! check traps with `ud2` and the kernel ends this process with exit 132
//! (128 + SIGILL). If hardening were off, the read would return and this
//! binary would exit 0, which the scenario rejects.

#![no_main]

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

unsafe extern "C" {
    fn cxx_out_of_bounds(index: i32) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    neo_crt::init();
    println!("[ crt-trap ] reading std::vector<int>(4)[4]");
    let v = unsafe { cxx_out_of_bounds(std::hint::black_box(4)) };
    println!("[ crt-trap ] no trap: read {v} (hardening is off)");
    std::process::exit(0)
}
