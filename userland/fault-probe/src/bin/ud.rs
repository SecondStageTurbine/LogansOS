// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: `ud2` raises #UD; the process must die with 132 (128 + SIGILL).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-ud ] raising #UD in ring 3\n");
    // SAFETY: deliberately faults; the kernel kills this process.
    unsafe { core::arch::asm!("ud2", options(nomem, nostack)) };
    survived(b"[ fault-probe-ud ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
