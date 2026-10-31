// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: an integer divide by zero raises #DE; exit 136 (128 + SIGFPE).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-de ] raising #DE in ring 3\n");
    // SAFETY: deliberately faults; the kernel kills this process.
    unsafe {
        core::arch::asm!(
            "xor edx, edx",
            "xor eax, eax",
            "div ecx",
            in("ecx") core::hint::black_box(0u32),
            out("eax") _,
            out("edx") _,
            options(nomem, nostack),
        )
    };
    survived(b"[ fault-probe-de ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
