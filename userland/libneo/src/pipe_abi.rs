// SPDX-License-Identifier: MPL-2.0
//! L1a.1 step 2a: the pipe syscalls' ABI as user space sees it (kernel
//! `pipe`; `ENOSYS` elsewhere). Host-tested from `tools/pipe-logic-tests`
//! against hand-laid bytes and against the kernel's packer
//! (`kernel/src/ipc/pipe_logic.rs`), Dexter's #56 F3.

pub const SYS_PIPE_CREATE: u64 = 0x1BA;
pub const SYS_PIPE_READ: u64 = 0x1BB;
pub const SYS_PIPE_WRITE: u64 = 0x1BC;
/// `sys_pipe_read` / `sys_pipe_write` flag: don't park (`EAGAIN` instead).
pub const PIPE_NONBLOCK: u64 = 1;

/// `SYS_PIPE_CREATE`'s packed return: the read end's handle in the low 32
/// bits, the write end's in the high 32. Returns `(read, write)`.
#[inline]
pub const fn pipe_split_ends(packed: u64) -> (u32, u32) {
    (packed as u32, (packed >> 32) as u32)
}
