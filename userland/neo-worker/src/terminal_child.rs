// SPDX-License-Identifier: MPL-2.0
//! L1a.3 test only: the terminal's child when a scenario needs one other than
//! `neo-shell` (`terminal-child-worker`). The terminal spawns this same
//! binary with its stdio chosen (stdin and stdout pipes, the term page); with
//! a stdio selected, `neo-worker`'s `terminal-app` runs this instead of the
//! terminal. Which child it is, is chosen at build time:
//!
//! - `terminal-test-echo`: a plain cooked child. Prints `child ready`, then for
//!   every line it reads prints `got=` and the line's bytes in hex (the `\n`
//!   included), so a scenario reads on the grid exactly what reached its
//!   stdin; on EOF prints `child eof` and exits 0.
//! - `terminal-test-flood`: prints `flood start`, then writes 10 MiB of
//!   numbered lines to stdout (it blocks whenever the 64 KiB pipe is full:
//!   the terminal's backpressure, §3.3), then `flood done`, then waits for
//!   EOF. The scenario ends it with End process mid-flood.

use libneo::{STDIN_FD, sys_read_fd, sys_write};

pub fn run() -> i32 {
    #[cfg(feature = "terminal-test-flood")]
    return flood();
    #[cfg(not(feature = "terminal-test-flood"))]
    echo()
}

/// All of `bytes` to stdout (a pipe write may take part of a large buffer).
fn out(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        match sys_write(1, bytes.as_ptr(), bytes.len()) {
            n if n > 0 => bytes = &bytes[n as usize..],
            libneo::EAGAIN => libneo::sys_sched_yield(),
            _ => return false,
        }
    }
    true
}

/// One line per line read, its bytes in hex.
#[cfg_attr(feature = "terminal-test-flood", allow(dead_code))]
fn echo() -> i32 {
    if !out(b"child ready\n") {
        return 2;
    }
    let mut line = [0u8; 128];
    let mut len = 0usize;
    loop {
        let mut byte = [0u8; 1];
        match sys_read_fd(STDIN_FD, &mut byte) {
            1 => {
                if len < line.len() {
                    line[len] = byte[0];
                    len += 1;
                }
                if byte[0] == b'\n' {
                    if !report(&line[..len]) {
                        return 3;
                    }
                    len = 0;
                }
            }
            0 => {
                if len > 0 && !report(&line[..len]) {
                    return 3;
                }
                return if out(b"child eof\n") { 0 } else { 4 };
            }
            libneo::EAGAIN => libneo::sys_sched_yield(),
            _ => return 5,
        }
    }
}

fn report(bytes: &[u8]) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = [0u8; 4 + 2 * 128 + 1];
    text[..4].copy_from_slice(b"got=");
    let mut n = 4;
    for &b in bytes {
        text[n] = HEX[usize::from(b >> 4)];
        text[n + 1] = HEX[usize::from(b & 0xf)];
        n += 2;
    }
    text[n] = b'\n';
    out(&text[..n + 1])
}

/// 10 MiB of output, then wait for EOF.
#[cfg_attr(not(feature = "terminal-test-flood"), allow(dead_code))]
fn flood() -> i32 {
    const TOTAL: usize = 10 * 1024 * 1024;
    const LINE: usize = 64;
    if !out(b"flood start\n") {
        return 2;
    }
    let mut chunk = [b'.'; 4096];
    let mut written = 0usize;
    let mut number = 0u32;
    while written < TOTAL {
        for line in chunk.chunks_mut(LINE) {
            line[..6].copy_from_slice(b"flood ");
            let mut v = number;
            for d in (6..12).rev() {
                line[d] = b'0' + (v % 10) as u8;
                v /= 10;
            }
            line[LINE - 1] = b'\n';
            number = number.wrapping_add(1);
        }
        if !out(&chunk) {
            return 3;
        }
        written += chunk.len();
    }
    if !out(b"flood done\n") {
        return 4;
    }
    let mut byte = [0u8; 1];
    loop {
        match sys_read_fd(STDIN_FD, &mut byte) {
            0 => return 0,
            n if n > 0 => {}
            libneo::EAGAIN => libneo::sys_sched_yield(),
            _ => return 5,
        }
    }
}
