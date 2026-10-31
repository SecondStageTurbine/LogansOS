// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: an x87 divide by zero with FCW.ZM cleared leaves a pending exception, which the next waiting x87 instruction raises as #MF (needs CR0.NE); exit 136 (128 + SIGFPE).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-mf ] raising #MF in ring 3\n");
    let fcw: u16 = 0x037f & !(1 << 2); // default control word, ZM unmasked
    // SAFETY: deliberately faults; the kernel kills this process.
    unsafe {
        core::arch::asm!(
            "fninit",
            "fldcw [{fcw}]",
            "fld1",
            "fldz",
            "fdivp st(1), st",
            "fwait",
            fcw = in(reg) &fcw,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        )
    };
    survived(b"[ fault-probe-mf ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
