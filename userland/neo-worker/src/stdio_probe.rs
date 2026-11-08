// SPDX-License-Identifier: MPL-2.0
//! L1a.1 step 2b: the `pipe_stdio` scenario's probe child
//! (`docs/THINK-2026-11-08-l1a1-stdio-selection.md` §4). neo-init's
//! `pipe-stdio-test` terminal spawns it with `SYS_SPAWN_STDIO`.
//!
//! Two modes, chosen by where its stdin is (`SYS_STDIO_INFO`):
//! - **shell's child** (stdin in slot 1: `neo-shell`'s `spawn` put its
//!   channel in slot 0): take the shell's ping, write `probe-line` to fd 1,
//!   reply, exit 0 (leg 8);
//! - **command** (stdin in slot 0): read one byte from fd 0 and run it:
//!   `i` writes `SYS_STDIO_INFO`'s 8 bytes to fd 1 (leg 7); `s` runs the
//!   stale-stdout cases (leg 5; the result is the exit code, since stdout is
//!   gone); `t` maps the term page and writes back its magic and size, `T`
//!   replaces the term slot with another buffer and tries the map again
//!   (leg 9).

use libneo::{
    CapRights, EAGAIN, EBADF, FileHandle, Handle, PIPE_NONBLOCK, PIPE_RIGHT_WRITE, STDIN_FD,
    STDOUT_FD, meta_rights, sys_channel_close, sys_close, sys_derive, sys_pipe_create,
    sys_pipe_read, sys_read_fd, sys_recv, sys_send, sys_stdio_info, sys_stdio_info_raw, sys_write,
};

fn out(bytes: &[u8]) -> i64 {
    sys_write(STDOUT_FD, bytes.as_ptr(), bytes.len())
}

pub fn run() -> i32 {
    let Ok(info) = sys_stdio_info() else {
        return 90;
    };
    if info.stdin == Some(1) {
        return shell_child();
    }
    let mut cmd = [0u8; 1];
    if sys_read_fd(STDIN_FD, &mut cmd) != 1 {
        return 91;
    }
    match cmd[0] {
        b'i' => info_bytes(),
        b's' => stale_stdout(),
        b't' => term_read(),
        b'T' => term_stale(),
        _ => 92,
    }
}

/// Leg 8: `neo-shell`'s `spawn` child protocol, with fd 1 the shell's stdout.
fn shell_child() -> i32 {
    let chan = Handle(0);
    let mut ping = [0u8; 64];
    if sys_recv(chan, &mut ping) < 0 {
        return 80;
    }
    if out(b"probe-line\n") != 11 {
        return 81;
    }
    if sys_send(chan, b"probe-ok") < 0 {
        return 82;
    }
    0
}

/// Leg 7: the raw record, so the terminal compares the kernel's bytes.
fn info_bytes() -> i32 {
    let mut raw = [0u8; 8];
    if sys_stdio_info_raw(raw.as_mut_ptr() as u64, raw.len()) != 0 {
        return 70;
    }
    if out(&raw) != 8 { 71 } else { 0 }
}

/// Leg 5: a selected stdout whose slot goes stale is `EBADF` (−9) however
/// the slot is reused, and nothing reaches what now sits there.
fn stale_stdout() -> i32 {
    // (c) a write end reused into slot 1: make P2 (ends in 2 and 3), free
    // slot 1, derive P2's write end into it (the lowest free slot).
    let Ok((p2_rd, p2_wr)) = sys_pipe_create(512) else {
        return 50;
    };
    if sys_channel_close(Handle(1)) != 0 {
        return 51;
    }
    let rights = CapRights::new(meta_rights::USE, PIPE_RIGHT_WRITE);
    match sys_derive(p2_wr, rights) {
        Ok(Handle(1)) => {}
        _ => return 52,
    }
    if out(b"x") != EBADF {
        return 53;
    }
    let mut b = [0u8; 1];
    if sys_pipe_read(p2_rd, &mut b, PIPE_NONBLOCK) != EAGAIN {
        return 54;
    }
    // (a) closed: slot 1 empty.
    if sys_close(FileHandle(1)) != 0 {
        return 55;
    }
    if out(b"x") != EBADF {
        return 56;
    }
    // (b) a read end reused into it: a new pipe's read end lands in slot 1.
    match sys_pipe_create(512) {
        Ok((Handle(1), _)) => {}
        _ => return 57,
    }
    if out(b"x") != EBADF {
        return 58;
    }
    0
}

/// Leg 9: the term page maps, and shows what the terminal wrote.
fn term_read() -> i32 {
    let Ok(p) = libneo::sys_stdio_term_map(0) else {
        return 60;
    };
    // SAFETY: the kernel mapped one 4 KiB page at `p`, writable; we read
    // the magic word and the columns and rows (terminal THINK §3.2).
    let (magic, cols, rows) = unsafe {
        (
            core::ptr::read_volatile(p as *const u32),
            core::ptr::read_volatile(p.add(12) as *const u16),
            core::ptr::read_volatile(p.add(14) as *const u16),
        )
    };
    let mut line = [0u8; 12];
    line[..4].copy_from_slice(&magic.to_le_bytes());
    line[4..6].copy_from_slice(&cols.to_le_bytes());
    line[6..8].copy_from_slice(&rows.to_le_bytes());
    line[8..].copy_from_slice(b"term");
    if out(&line) != 12 { 61 } else { 0 }
}

/// Leg 9: a buffer that only sits in the term page's old slot is refused.
fn term_stale() -> i32 {
    if sys_close(FileHandle(2)) != 0 {
        return 62;
    }
    let desc = libneo::encode_buffer_desc(1024, 1, 4096, libneo::FORMAT_XRGB8888_LE, 4096);
    match libneo::sys_buffer_create(&desc) {
        Ok(Handle(2)) => {}
        _ => return 63,
    }
    let r = match libneo::sys_stdio_term_map(0) {
        Ok(_) => 0,
        Err(e) => e,
    };
    let code = r.to_le_bytes();
    if out(&code) != 8 { 64 } else { 0 }
}
