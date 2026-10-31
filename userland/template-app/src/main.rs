// SPDX-License-Identifier: MPL-2.0
//! `template-app` — minimal "Hello, LogansOS" userland binary.
//!
//! This crate demonstrates the complete build configuration for a LogansOS
//! userland program: `#![no_std]`, custom target spec, linker script, and
//! a single syscall (`sys_write` to stdout). Copy this directory as the
//! starting point for your own LogansOS application.

#![no_std]
#![no_main]

use libneo::{sys_exit, sys_write};

/// LogansOS entry point. Called by the kernel after ELF segment load.
/// Must be `pub extern "C"` and return `!` (never returns).
pub extern "C" fn _start() -> ! {
    let message = b"Hello, LogansOS!\n";

    // Write to stdout (fd 1). sys_write returns bytes written on success,
    // or a negative errno on failure.
    let result = sys_write(1, message.as_ptr(), message.len());

    // Exit with 0 on success, or propagate the error code.
    let code = if result >= 0 { 0i32 } else { -1i32 };
    sys_exit(code);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(255);
}
