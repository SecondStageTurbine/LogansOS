// SPDX-License-Identifier: MPL-2.0
//! L1a.1 step 2b: the `pipe_stdio` scenario's terminal side
//! (`docs/THINK-2026-11-08-l1a1-stdio-selection.md` §4). Real ring-3 code
//! making the real syscalls, as a terminal would: it spawns the real
//! `neo-shell`, and neo-worker's `pipe-stdio-test` probe, with
//! `SYS_SPAWN_STDIO`, and reports each leg as a `PIPE_STDIO <leg> ...` line
//! on its own console (fd 1, which is serial: neo-init has no selection).
//!
//! Waits are bounded by the TSC (this scenario's kernel has no clock
//! syscall), so a byte that never arrives is a named failure, not a hang.
//! The leg with no selection runs last: everything before its marker must
//! not reach serial (xtask's `require_serial_absent` on that prefix).

use alloc::vec::Vec;
use libneo::{
    EAGAIN, EBADF, EFAULT, EINVAL, EPIPE, ESRCH, Handle, PIPE_NONBLOCK, ProcessHandle, StdioInfo,
    sys_chan_create, sys_channel_close, sys_kill, sys_pipe_create, sys_pipe_read, sys_pipe_write,
    sys_spawn, sys_spawn_stdio, sys_spawn_stdio_word, sys_stdio_info, sys_stdio_info_raw, sys_wait,
    sys_write,
};

/// The bound on one wait, in TSC cycles: seconds on QEMU's TCG clock, so a
/// slow runner still makes it and a missing byte still ends the leg.
const WAIT_CYCLES: u64 = 1 << 35;

fn say(line: &[u8]) {
    sys_write(1, line.as_ptr(), line.len());
}

fn tsc() -> u64 {
    // SAFETY: RDTSC is unprivileged on LogansOS (CR4.TSD clear).
    unsafe { core::arch::x86_64::_rdtsc() }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Read from `rd` (non-blocking, polled) until `acc` holds `needle`, the
/// pipe reaches EOF, or the wait bound passes. True on `needle`.
fn read_until(rd: Handle, needle: &[u8], acc: &mut Vec<u8>) -> bool {
    let start = tsc();
    let mut buf = [0u8; 512];
    loop {
        if contains(acc, needle) {
            return true;
        }
        match sys_pipe_read(rd, &mut buf, PIPE_NONBLOCK) {
            0 => return false,
            n if n > 0 => acc.extend_from_slice(&buf[..n as usize]),
            EAGAIN => {
                if tsc().wrapping_sub(start) > WAIT_CYCLES {
                    return false;
                }
                core::hint::spin_loop();
            }
            _ => return false,
        }
    }
}

/// Read `rd` to EOF within the bound. True at EOF.
fn read_to_eof(rd: Handle, acc: &mut Vec<u8>) -> bool {
    let start = tsc();
    let mut buf = [0u8; 512];
    loop {
        match sys_pipe_read(rd, &mut buf, PIPE_NONBLOCK) {
            0 => return true,
            n if n > 0 => acc.extend_from_slice(&buf[..n as usize]),
            EAGAIN => {
                if tsc().wrapping_sub(start) > WAIT_CYCLES {
                    return false;
                }
                core::hint::spin_loop();
            }
            _ => return false,
        }
    }
}

fn write_all(wr: Handle, bytes: &[u8]) -> bool {
    sys_pipe_write(wr, bytes, 0) == bytes.len() as i64
}

fn yes(b: bool) -> &'static [u8] {
    if b { b"true" } else { b"false" }
}

fn say_i64(v: i64) {
    let mut buf = [0u8; 24];
    let mut i = buf.len();
    let neg = v < 0;
    let mut u = v.unsigned_abs();
    loop {
        i -= 1;
        buf[i] = b'0' + (u % 10) as u8;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    if neg {
        i -= 1;
        buf[i] = b'-';
    }
    say(&buf[i..]);
}

/// A terminal's two pipes: (stdin read, stdin write, stdout read, stdout write).
fn pipes() -> Option<(Handle, Handle, Handle, Handle)> {
    let (in_rd, in_wr) = sys_pipe_create(4 * 1024).ok()?;
    let (out_rd, out_wr) = sys_pipe_create(64 * 1024).ok()?;
    Some((in_rd, in_wr, out_rd, out_wr))
}

/// The child's ends are its copies alone once the terminal closes its own.
fn close(h: Handle) {
    let _ = sys_channel_close(h);
}

struct Child {
    process: ProcessHandle,
    in_wr: Handle,
    out_rd: Handle,
}

/// Spawn `name` on a fresh pair of pipes as stdin (0) and stdout (1), plus
/// `extra` handles after them (the term page).
fn spawn_on_pipes(name: &str, term: Option<Handle>) -> Result<Child, i64> {
    let (in_rd, in_wr, out_rd, out_wr) = pipes().ok_or(-1i64)?;
    let r = match term {
        None => sys_spawn_stdio(name, &[in_rd, out_wr], Some(0), Some(1), None),
        Some(t) => sys_spawn_stdio(name, &[in_rd, out_wr, t], Some(0), Some(1), Some(2)),
    };
    close(in_rd);
    close(out_wr);
    let (_, process) = r?;
    Ok(Child {
        process,
        in_wr,
        out_rd,
    })
}

fn leg_round_and_eof() -> bool {
    // Leg 1: the prompt and the reply arrive in the stdout pipe.
    let Ok(c) = spawn_on_pipes("shell", None) else {
        say(b"PIPE_STDIO fail pipe_stdio_spawn_failed\n");
        return false;
    };
    let mut acc = Vec::new();
    let prompt = read_until(c.out_rd, b"neo> ", &mut acc);
    if !prompt {
        say(b"PIPE_STDIO fail pipe_stdio_prompt_not_in_pipe\n");
        return false;
    }
    let sent = write_all(c.in_wr, b"echo pipe-ok\n");
    // The echoed command line is `echo pipe-ok\n`; the reply follows it.
    let reply = sent && read_until(c.out_rd, b"\npipe-ok\n", &mut acc);
    say(b"PIPE_STDIO round prompt=");
    say(yes(prompt));
    say(b" reply=");
    say(yes(reply));
    say(b"\n");
    if !reply {
        return false;
    }
    // Leg 2: closing stdin is EOF to the shell, which exits with 2; its
    // stdout then reads EOF after what was buffered.
    close(c.in_wr);
    let code = sys_wait(c.process, 0).unwrap_or(-1);
    let out_eof = read_to_eof(c.out_rd, &mut acc);
    close(c.out_rd);
    say(b"PIPE_STDIO eof exit=");
    say_i64(i64::from(code));
    say(b" out_eof=");
    say(yes(out_eof));
    say(b"\n");
    code == 2 && out_eof
}

fn leg_kill() -> bool {
    // Leg 3: killed while parked on fd 0; both of its endpoints go.
    let Ok(c) = spawn_on_pipes("shell", None) else {
        return bail(b"leg_kill:1");
    };
    let mut acc = Vec::new();
    // The prompt alone isn't a park: the shell writes it before it reads fd 0
    // (Janus #72 F2). Wait until the kernel has the shell token-parked on its
    // stdin pipe (the pipe's only reader) before the kill.
    if !read_until(c.out_rd, b"neo> ", &mut acc) {
        return bail(b"leg_kill:prompt");
    }
    let parked = wait_parked(c.in_wr);
    if !parked {
        return bail(b"leg_kill:park_not_seen");
    }
    let kill = sys_kill(c.process);
    // Gone either way: a wait that caught the exit has its code; once the kill
    // has finished, a killed process keeps no code and the wait is ESRCH.
    let wait = sys_wait(c.process, 0);
    let reaped = matches!(wait, Ok(_) | Err(ESRCH));
    let out_eof = read_to_eof(c.out_rd, &mut acc);
    let in_epipe = sys_pipe_write(c.in_wr, b"x", PIPE_NONBLOCK) == EPIPE;
    close(c.in_wr);
    close(c.out_rd);
    say(b"PIPE_STDIO kill parked=");
    say(yes(parked));
    say(b" kill=");
    say_i64(kill);
    say(b" reaped=");
    say(yes(reaped));
    say(b" out_eof=");
    say(yes(out_eof));
    say(b" in_epipe=");
    say(yes(in_epipe));
    say(b"\n");
    parked && kill == 0 && reaped && out_eof && in_epipe
}

/// Start the probe and send it one command byte.
fn probe(cmd: u8, term: Option<Handle>) -> Option<Child> {
    let c = spawn_on_pipes("worker", term).ok()?;
    if !write_all(c.in_wr, &[cmd]) {
        return None;
    }
    Some(c)
}

fn finish(c: Child) -> i32 {
    close(c.in_wr);
    close(c.out_rd);
    sys_wait(c.process, 0).unwrap_or(-1)
}

fn leg_stale() -> bool {
    // Leg 5: the probe's three stale-stdout cases; its exit code says which
    // failed (0: none).
    let Some(c) = probe(b's', None) else {
        return bail(b"leg_stale:1");
    };
    let code = finish(c);
    say(b"PIPE_STDIO stale exit=");
    say_i64(i64::from(code));
    say(b"\n");
    code == 0
}

/// Buffer rights bits (the kernel's `atrium::logic::buffer_rights`).
const MAP_WRITE: u8 = 1 << 0;
const MAP_READ: u8 = 1 << 1;

/// A `bytes`-byte XRGB buffer. A buffer may be no wider than the framebuffer
/// (`validate_desc`'s `ExceedsOutput`), so rows are at most 1024 pixels:
/// 4096 bytes is 1024 x 1, 4092 is 1023 x 1, 8192 is 1024 x 2.
fn buffer(bytes: u32) -> Option<Handle> {
    let pixels = bytes / 4;
    let width = pixels.min(1024);
    let height = pixels / width;
    let desc = libneo::encode_buffer_desc(
        width,
        height,
        width * 4,
        libneo::FORMAT_XRGB8888_LE,
        u64::from(bytes),
    );
    libneo::sys_buffer_create(&desc).ok()
}

/// `SYS_TEST_PIPE_PARKED` (0xF0B, `pipe_stdio` kernels only): the readers
/// token-parked on the pipe `h` is an end of.
const SYS_TEST_PIPE_PARKED: u64 = 0xF0B;

/// Poll (TSC-bounded) until exactly one reader is parked on `h`'s pipe.
fn wait_parked(h: Handle) -> bool {
    let start = tsc();
    loop {
        // SAFETY: one integer argument; the kernel resolves the handle.
        let n = unsafe { libneo::raw3(SYS_TEST_PIPE_PARKED, u64::from(h.0), 0, 0) };
        if n == 1 {
            return true;
        }
        if n < 0 || tsc().wrapping_sub(start) > WAIT_CYCLES {
            return false;
        }
        core::hint::spin_loop();
    }
}

/// A leg's setup step failed: name it on serial, so the run says which.
fn bail(step: &[u8]) -> bool {
    say(b"PIPE_STDIO fail step=");
    say(step);
    say(b"\n");
    false
}

fn leg_refusals() -> bool {
    // Leg 6: every refusal is EINVAL, and no child is made (no process handle
    // is installed: the next spawn's handle takes the same slot).
    let Some((in_rd, in_wr, out_rd, out_wr)) = pipes() else {
        return bail(b"leg_refusals:1");
    };
    let Ok((chan, chan_peer)) = sys_chan_create() else {
        return bail(b"leg_refusals:2");
    };
    let (Some(t4096), Some(t4092), Some(t8192)) = (buffer(4096), buffer(4092), buffer(8192)) else {
        return bail(b"leg_refusals:3");
    };
    let ro = libneo::sys_derive(
        t4096,
        libneo::CapRights::new(
            libneo::meta_rights::USE | libneo::meta_rights::GRANT,
            MAP_READ,
        ),
    );
    let Ok(t_ro) = ro else {
        return bail(b"leg_refusals:4");
    };
    let w = |a: Option<u8>, b: Option<u8>, c: Option<u8>| libneo::stdio_word(a, b, c);
    let cases: [(&[Handle], u64); 16] = [
        // stdin naming a write end, a channel, a buffer.
        (&[out_wr, out_wr], w(Some(0), None, None)),
        (&[chan], w(Some(0), None, None)),
        (&[t4096], w(Some(0), None, None)),
        // stdout naming a read end, a channel, a buffer.
        (&[in_rd], w(None, Some(0), None)),
        (&[chan], w(None, Some(0), None)),
        (&[t4096], w(None, Some(0), None)),
        // term naming a pipe; term pages of 4092 and 8192 bytes; no MAP_WRITE.
        (&[in_rd], w(None, None, Some(0))),
        (&[t4092], w(None, None, Some(0))),
        (&[t8192], w(None, None, Some(0))),
        (&[t_ro], w(None, None, Some(0))),
        // one index for two roles; an index >= n; bit 24.
        (&[in_rd, out_wr], w(Some(0), Some(0), None)),
        (&[in_rd], w(Some(0), Some(1), None)),
        (&[in_rd], w(Some(1), None, None)),
        (&[in_rd, out_wr], w(Some(0), Some(1), None) | (1 << 24)),
        (&[in_rd, out_wr], w(Some(0), Some(1), None) | (1 << 63)),
        (&[in_rd, out_wr, t4096], w(Some(0), Some(1), Some(1))),
    ];
    // The lowest free slot now; a refused spawn that still made a child would
    // fill it with the child's process handle.
    let lowest = || {
        sys_chan_create().map(|(a, b)| {
            close(a);
            close(b);
            a.0
        })
    };
    let before = lowest();
    let mut bad = 0u32;
    for (handles, word) in cases {
        match sys_spawn_stdio_word("worker", handles, word) {
            Err(EINVAL) => {}
            _ => bad += 1,
        }
    }
    let after = lowest();
    for h in [
        in_rd, in_wr, out_rd, out_wr, chan, chan_peer, t4096, t4092, t8192, t_ro,
    ] {
        close(h);
    }
    say(b"PIPE_STDIO refusals cases=16 not_einval=");
    say_i64(i64::from(bad));
    say(b"\n");
    bad == 0 && before.is_ok() && before == after
}

fn leg_info() -> bool {
    // Leg 7: the probe's kernel bytes against the hand-laid oracle row; the
    // terminal's own (no selection); the errors; an all-none spawn.
    let Some(t) = buffer(4096) else {
        return bail(b"leg_info:1");
    };
    let Some(c) = probe(b'i', Some(t)) else {
        return bail(b"leg_info:2");
    };
    close(t);
    let mut acc = Vec::new();
    let mut got = false;
    let start = tsc();
    let mut buf = [0u8; 8];
    while acc.len() < 8 && tsc().wrapping_sub(start) < WAIT_CYCLES {
        match sys_pipe_read(c.out_rd, &mut buf, PIPE_NONBLOCK) {
            n if n > 0 => acc.extend_from_slice(&buf[..n as usize]),
            EAGAIN => core::hint::spin_loop(),
            _ => break,
        }
    }
    if acc.len() >= 8 {
        got = acc[..8] == [0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00];
    }
    let probe_code = finish(c);
    let me: Result<StdioInfo, i64> = sys_stdio_info();
    let self_none = me
        .is_ok_and(|i| !i.selected && i.stdin.is_none() && i.stdout.is_none() && i.term.is_none());
    let mut raw = [0u8; 8];
    let mut self_bytes = [0u8; 8];
    let self_raw = sys_stdio_info_raw(self_bytes.as_mut_ptr() as u64, 8) == 0
        && self_bytes == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00];
    let len7 = sys_stdio_info_raw(raw.as_mut_ptr() as u64, 7);
    let out1 = sys_stdio_info_raw(1, 8);
    // An all-none spawn of the probe is SYS_SPAWN's child: with no selection
    // it prints its record on the console (serial), and xtask pins that line
    // to the no-selection row.
    let allnone = sys_spawn_stdio_word("worker", &[], 0x00FF_FFFF)
        .map(|(_, p)| sys_wait(p, 0) == Ok(0))
        .unwrap_or(false);
    say(b"PIPE_STDIO info probe=");
    say(yes(got && probe_code == 0));
    say(b" self=");
    say(yes(self_none && self_raw));
    say(b" len7=");
    say_i64(len7);
    say(b" out1=");
    say_i64(out1);
    say(b" allnone=");
    say(yes(allnone));
    say(b"\n");
    got && probe_code == 0 && self_none && self_raw && len7 == EINVAL && out1 == EFAULT && allnone
}

fn leg_shell_child() -> bool {
    // Leg 8: the shell names its pipes for its child; the child's line
    // arrives in the terminal's stdout pipe.
    let Ok(c) = spawn_on_pipes("shell", None) else {
        return bail(b"leg_shell_child:1");
    };
    let mut acc = Vec::new();
    let prompt = read_until(c.out_rd, b"neo> ", &mut acc);
    let sent = prompt && write_all(c.in_wr, b"spawn worker\n");
    let line = sent && read_until(c.out_rd, b"probe-line\n", &mut acc);
    let relay = line && read_until(c.out_rd, b"probe-ok", &mut acc);
    let code = finish(c);
    say(b"PIPE_STDIO shell_child line=");
    say(yes(line));
    say(b" relay=");
    say(yes(relay));
    say(b"\n");
    let _ = code;
    line && relay
}

fn leg_term() -> bool {
    // Leg 9: the term page maps and shows what the terminal wrote; a buffer
    // that only sits in its old slot is refused.
    let Some(t) = buffer(4096) else {
        return bail(b"leg_term:1");
    };
    let Ok(p) = libneo::sys_buffer_map(t, 0) else {
        return bail(b"leg_term:2");
    };
    // The terminal THINK's §3.2 header: magic `NTP1`, version 1, 80 x 25.
    // SAFETY: `p` is our writable mapping of the 4 KiB buffer.
    unsafe {
        core::ptr::write_volatile(p as *mut u32, 0x3150_544E);
        core::ptr::write_volatile(p.add(4) as *mut u16, 1);
        core::ptr::write_volatile(p.add(12) as *mut u16, 80);
        core::ptr::write_volatile(p.add(14) as *mut u16, 25);
    }
    let Some(c) = probe(b't', Some(t)) else {
        return bail(b"leg_term:3");
    };
    let mut acc = Vec::new();
    let mapped = read_until(c.out_rd, b"term", &mut acc);
    let shown = mapped && acc.len() >= 12 && acc[..8] == [0x4e, 0x54, 0x50, 0x31, 80, 0, 25, 0];
    let map_code = finish(c);
    let Some(c) = probe(b'T', Some(t)) else {
        return bail(b"leg_term:4");
    };
    close(t);
    let mut acc = Vec::new();
    let start = tsc();
    let mut buf = [0u8; 8];
    while acc.len() < 8 && tsc().wrapping_sub(start) < WAIT_CYCLES {
        match sys_pipe_read(c.out_rd, &mut buf, PIPE_NONBLOCK) {
            n if n > 0 => acc.extend_from_slice(&buf[..n as usize]),
            EAGAIN => core::hint::spin_loop(),
            _ => break,
        }
    }
    let stale = if acc.len() >= 8 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&acc[..8]);
        i64::from_le_bytes(b)
    } else {
        0
    };
    let stale_code = finish(c);
    say(b"PIPE_STDIO term map=");
    say(yes(shown && map_code == 0));
    say(b" stale=");
    say_i64(stale);
    say(b"\n");
    shown && map_code == 0 && stale == EBADF && stale_code == 0
}

fn leg_unselected() -> bool {
    // Leg 4, run last: no selection is today's spawn. The shell's prompt goes
    // to serial; its fd-0 read is EBADF (no selection, no serial stdin), so it
    // exits with 2.
    say(b"PIPE_STDIO unselected begin\n");
    let Ok((local, child)) = sys_chan_create() else {
        return bail(b"leg_unselected:1");
    };
    let Ok((_, process)) = sys_spawn("shell", &[child]) else {
        return bail(b"leg_unselected:2");
    };
    let code = sys_wait(process, 0).unwrap_or(-1);
    close(local);
    say(b"PIPE_STDIO unselected exit=");
    say_i64(i64::from(code));
    say(b"\n");
    code == 2
}

/// One leg: its name, and the run that says whether it passed.
type Leg = (&'static [u8], fn() -> bool);

pub fn run() -> i32 {
    let legs: [Leg; 8] = [
        (b"round", leg_round_and_eof),
        (b"kill", leg_kill),
        (b"stale", leg_stale),
        (b"refusals", leg_refusals),
        (b"info", leg_info),
        (b"shell_child", leg_shell_child),
        (b"term", leg_term),
        (b"unselected", leg_unselected),
    ];
    for (i, (name, leg)) in legs.iter().enumerate() {
        if !leg() {
            say(b"PIPE_STDIO fail leg=");
            say(name);
            say(b"\n");
            return 10 + i as i32;
        }
    }
    say(b"PIPE_STDIO pass legs=round,eof,kill,stale,refusals,info,shell_child,term,unselected\n");
    0
}
