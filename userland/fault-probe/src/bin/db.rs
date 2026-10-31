// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: setting RFLAGS.TF (single-step) raises #DB after the next instruction; exit 133 (128 + SIGTRAP).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-db ] raising #DB in ring 3\n");
    // SAFETY: deliberately traps; the kernel kills this process.
    unsafe { core::arch::asm!("pushfq", "or qword ptr [rsp], 0x100", "popfq", "nop", "nop",) };
    survived(b"[ fault-probe-db ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
