// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: a push through a non-canonical RSP raises #SS(0); exit 135 (128 + SIGBUS).
//! RSP = 0x8000_0000_0000_0008, so the push writes 0x8000_0000_0000_0000:
//! non-canonical (a canonical result would be a #PF instead).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-ss ] raising #SS in ring 3\n");
    // SAFETY: deliberately faults; RSP is restored by nothing because the
    // kernel kills this process at the push.
    unsafe {
        core::arch::asm!(
            "mov rsp, {bad}",
            "push rax",
            bad = in(reg) core::hint::black_box(0x8000_0000_0000_0008u64),
            options(nomem),
        )
    };
    survived(b"[ fault-probe-ss ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
