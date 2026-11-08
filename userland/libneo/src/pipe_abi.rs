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

/// L1a.1 step 2b (`docs/THINK-2026-11-08-l1a1-stdio-selection.md`): spawn a
/// child with its stdin, stdout and term page named by index into the
/// inherited handles.
pub const SYS_SPAWN_STDIO: u64 = 0x1BD;
/// The caller's own stdio slots, as an 8-byte record.
pub const SYS_STDIO_INFO: u64 = 0x1BE;
/// Map the selected term page (`SYS_BUFFER_MAP`'s role word).
pub const SYS_STDIO_TERM_MAP: u64 = 0x1BF;

/// The stdio word's "none" index byte.
pub const STDIO_NO_INDEX: u8 = 0xFF;

/// `SYS_SPAWN_STDIO`'s stdio word: byte 0 stdin, byte 1 stdout, byte 2 the
/// term page, each an index into the spawn's handles or `None`.
#[inline]
pub const fn stdio_word(stdin: Option<u8>, stdout: Option<u8>, term: Option<u8>) -> u64 {
    const fn b(i: Option<u8>) -> u64 {
        match i {
            Some(v) => v as u64,
            None => STDIO_NO_INDEX as u64,
        }
    }
    b(stdin) | (b(stdout) << 8) | (b(term) << 16)
}

/// `SYS_STDIO_INFO`'s record, decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StdioInfo {
    /// A stdio was selected at spawn (even if a slot has since gone).
    pub selected: bool,
    pub stdin: Option<u32>,
    pub stdout: Option<u32>,
    pub term: Option<u32>,
}

/// Decode `SYS_STDIO_INFO`'s 8 bytes: three little-endian `u16` slots
/// (`0xFFFF` for none), then the flags (bit 0: selected).
#[inline]
pub const fn decode_stdio_info(r: [u8; 8]) -> StdioInfo {
    const fn slot(lo: u8, hi: u8) -> Option<u32> {
        let v = u16::from_le_bytes([lo, hi]);
        if v == 0xFFFF { None } else { Some(v as u32) }
    }
    StdioInfo {
        selected: u16::from_le_bytes([r[6], r[7]]) & 1 != 0,
        stdin: slot(r[0], r[1]),
        stdout: slot(r[2], r[3]),
        term: slot(r[4], r[5]),
    }
}
