// SPDX-License-Identifier: MPL-2.0
//! OT-83 probe: the SECOND process. It runs after a probe was killed by its
//! exception and exits 0, showing the kernel and scheduler are still live.

#![no_std]
#![no_main]

use fault_probe::{panic_exit, say};
use libneo::sys_exit;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say(b"[ fault-probe-ok ] second process ran after the fault-kill\n");
    sys_exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    panic_exit()
}
