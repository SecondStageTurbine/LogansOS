// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: an SSE divide by zero with MXCSR.ZM cleared raises #XM (CR4.OSXMMEXCPT is set); exit 136 (128 + SIGFPE).

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say, survived};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-xm ] raising #XM in ring 3\n");
    let mut mxcsr: u32 = 0;
    let one: f32 = 1.0;
    // SAFETY: deliberately faults; the kernel kills this process.
    unsafe {
        core::arch::asm!(
            "stmxcsr [{m}]",
            "and dword ptr [{m}], {unmask_zm}",
            "ldmxcsr [{m}]",
            "movss xmm0, [{one}]",
            "xorps xmm1, xmm1",
            "divss xmm0, xmm1", // 1.0 / 0.0: divide-by-zero (ZE), now unmasked
            m = in(reg) &mut mxcsr,
            one = in(reg) &one,
            unmask_zm = const !(1u32 << 9),
            out("xmm0") _,
            out("xmm1") _,
            options(nostack),
        )
    };
    survived(b"[ fault-probe-xm ]");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
