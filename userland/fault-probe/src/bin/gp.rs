// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: `hlt` is privileged, so at CPL 3 it raises #GP(0); exit 139 (128 + SIGSEGV).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-gp ] raising #GP in ring 3\n");
    // SAFETY: deliberately faults; the kernel kills this process.
    unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    survived(b"[ fault-probe-gp ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
