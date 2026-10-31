// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: `install_console`'s negative leg. A process spawned without
//! the Installer cap calls both installer syscalls; each must be `EPERM`.
//! It passes a token no list named, so even a kernel that skipped the cap
//! check would refuse the run rather than write.

use libneo::install::{SYS_INSTALL_LIST, SYS_INSTALL_RUN, action};
use libneo::{EPERM, raw3, sys_write};

const OK: &[u8] = b"[ neo-worker ] installer-probe list=EPERM run=EPERM\n";
const BAD: &[u8] =
    b"[ neo-worker ] installer-probe FAILED: a syscall without the cap was not EPERM\n";

pub fn run() -> i32 {
    // SAFETY: the raw syscall ABI; a null buffer of length 0 is valid.
    let list = unsafe { raw3(SYS_INSTALL_LIST, 0, 0, 0) };
    // SAFETY: as above; no pointer is passed.
    let run = unsafe { raw3(SYS_INSTALL_RUN, u64::MAX, u64::MAX, action::INSTALL as u64) };
    if list == EPERM && run == EPERM {
        let _ = sys_write(1, OK.as_ptr(), OK.len());
        0
    } else {
        let _ = sys_write(1, BAD.as_ptr(), BAD.len());
        1
    }
}
