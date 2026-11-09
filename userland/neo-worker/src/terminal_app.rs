// SPDX-License-Identifier: MPL-2.0
//! L1a.3: the terminal window (`docs/THINK-2026-10-31-l1a-terminal-window.md`
//! §2-§8), `neo-worker` built with `terminal-app`.
//!
//! The WM (neo-init's client table) spawns it with its session channel
//! (`MAIN`) and nothing else, as it does the probe app. It runs the same
//! ADR-073 handshake, then makes its child's stdio: a 4 KiB stdin pipe, a
//! 64 KiB stdout pipe and a 4 KiB term page (§3.2, §3.3), and spawns the child
//! (`neo-shell`, or a test child) with them chosen at spawn (L1a.1). The child
//! gets the stdin read end, the stdout write end and the page, nothing else:
//! never the stdin write end, the surface channel or a focus (§3, §6).
//!
//! Then it serves turns, as the probe app does: out of front it parks in its
//! session receive for the WM's `K` (a try, which lands only on a parked
//! receiver). In front it parks in `SYS_WAIT_ANY` on its focus and the stdout
//! pipe with a 20 ms deadline, and on every wake, in order (§3.2, §3.3):
//! 1. applies the child's mode request, if it made a new one;
//! 2. reads at most 16 KiB of the child's output into the screen;
//! 3. serves its keys: through the encoder and the line discipline into a
//!    non-blocking write to the child's stdin; Ctrl+Shift+C kills the child;
//! 4. sees whether the child has exited;
//! 5. redraws the rows that changed and commits their band.
//!
//! It never blocks on its child: its writes to stdin are tries, and a full
//! pipe drops the key with the bell. It never sends input of its own: the
//! only bytes on the child's stdin are its own focus's keys, through the
//! discipline, and the three answerbacks (§5.4, §6). While the session is
//! locked its focus receive fails, so it parks for its next turn, which the
//! WM gives it at unlock; what was typed while locked never reaches it.
//!
//! When its session ends (`EPIPE` on `MAIN`) it kills the child and exits 0.
//!
//! Spawned with a stdio chosen (a test child), the same binary is
//! [`super::terminal_child`] instead.

use alloc::string::String;
use alloc::vec::Vec;
use core::num::NonZeroU16;
use core::sync::atomic::AtomicU32;
use libneo::atrium_protocol::{
    CommitDamage, CreateSurface, FOCUS_REASON_USER_ACTIVATION, FocusRequest, Identity,
    ImportAttachBuffer, OP_COMMIT_DAMAGE, OP_CREATE_SURFACE, OP_FOCUS_REQUEST,
    OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Request, decode_reply, encode_request,
};
use libneo::wait_abi::{WAIT_KIND_INPUT, WAIT_KIND_PIPE, WaitItem};
use libneo::{
    ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CLOCK_MONOTONIC,
    CapRights, EAGAIN, EPERM, EPIPE, ESRCH, FORMAT_XRGB8888_LE, Handle, INPUT_EVENT_V1_LEN,
    PIPE_NONBLOCK, ProcessHandle, WAIT_FLAG_WNOHANG, encode_buffer_desc, encode_damage_rect,
    meta_rights, sys_buffer_commit_damage, sys_buffer_create, sys_buffer_map, sys_channel_close,
    sys_clock_gettime, sys_grant, sys_input_focus_create, sys_input_focus_mint_activator,
    sys_input_focus_recv, sys_kill, sys_pipe_create, sys_pipe_read, sys_pipe_write, sys_recv,
    sys_send, sys_spawn_stdio, sys_try_recv, sys_wait, sys_wait_any,
};
use neo_term_core::page::{PAGE_LEN, Page, Size, WORDS};
use neo_terminal::dump::Dumper;
use neo_terminal::render::Renderer;
use neo_terminal::session::{KeyAction, READ_BUDGET, Session, Sink, Write};

const MAIN: Handle = Handle(0);

/// The WM's client size (neo-init's `CLIENT_W` x `CLIENT_H`).
const WIDTH: usize = 640;
const HEIGHT: usize = 400;
const STRIDE_BYTES: usize = WIDTH * 4;
const BUFFER_BYTES: u64 = (WIDTH * HEIGHT * 4) as u64;
const NS: u64 = libneo::atrium_launch::TERMINAL_NAMESPACE;
const BUFFER_CAP_PROVENANCE: u64 = libneo::atrium_launch::TERMINAL_BUFFER_PROVENANCE;

/// The pipes' capacities (§3.3).
const STDIN_CAPACITY: u64 = 4 * 1024;
const STDOUT_CAPACITY: u64 = 64 * 1024;
/// The wake period in front: a mode request is in force within this (§3.2).
const WAKE_NS: u64 = 20_000_000;
/// The grid read-back on serial at most this often while output streams.
const DUMP_NS: u64 = 250_000_000;
/// How many of the first wakes and commits are logged.
const TRACE: u64 = 4;
/// Focus events served per wake (a focus queue holds at most 64).
const KEYS_PER_WAKE: usize = 64;

/// The child: `neo-shell`, or (test builds) the worker's terminal child.
#[cfg(not(feature = "terminal-child-worker"))]
const CHILD: &str = "shell";
#[cfg(feature = "terminal-child-worker")]
const CHILD: &str = "worker";

/// The child's stdin, written only by tries (§4).
struct Stdin(Option<Handle>);

impl Sink for Stdin {
    fn try_write(&mut self, bytes: &[u8]) -> Write {
        let Some(h) = self.0 else {
            return Write::Gone;
        };
        match sys_pipe_write(h, bytes, PIPE_NONBLOCK) {
            n if n >= 0 => Write::Wrote(n as usize),
            EAGAIN => Write::Wrote(0),
            _ => Write::Gone,
        }
    }
}

/// The running child.
struct Child {
    process: ProcessHandle,
    stdin: Stdin,
    stdout: Option<Handle>,
    exited: bool,
}

pub fn run() -> i32 {
    let mut request_id = 0u64;
    let mut next_id = || {
        request_id += 1;
        request_id
    };

    let create_id = next_id();
    if !send_request(
        MAIN,
        create_id,
        Request::Create(CreateSurface {
            identity: Identity {
                namespace_id: NS,
                surface_id: 0,
                surface_cap_provenance: 0,
            },
            role: ROLE_TEXT,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            client_tag: u64::from(b'T'),
        }),
    ) {
        return 10;
    }
    let identity = match recv_reply(MAIN, OP_CREATE_SURFACE, create_id) {
        Ok(identity) => identity,
        Err(code) => return 11 + code,
    };
    let surface = match recv_grant(MAIN) {
        Ok(surface) => surface,
        Err(code) => return 15 + code,
    };
    // The byte the WM reads where Arthur proves his scanout bypass refused;
    // the terminal holds no scanout authority to prove.
    if sys_send(MAIN, b"B") != 1 {
        return 20;
    }

    let desc = encode_buffer_desc(
        WIDTH as u32,
        HEIGHT as u32,
        STRIDE_BYTES as u32,
        FORMAT_XRGB8888_LE,
        BUFFER_BYTES,
    );
    let Ok(buffer) = sys_buffer_create(&desc) else {
        return 21;
    };
    let Ok(mapping) = sys_buffer_map(buffer, 0) else {
        return 22;
    };
    // SAFETY: this process's RW view of a buffer whose descriptor is exactly
    // WIDTH x HEIGHT tightly packed XRGB8888 pixels; never unmapped while the
    // process lives, and only this thread writes it.
    let pixels = unsafe { core::slice::from_raw_parts_mut(mapping as *mut u32, WIDTH * HEIGHT) };
    let Some(mut renderer) = Renderer::new(WIDTH, HEIGHT) else {
        return 23;
    };
    let geo = renderer.geometry();
    let (Some(rows), Some(cols)) = (NonZeroU16::new(geo.rows), NonZeroU16::new(geo.cols)) else {
        return 23;
    };
    let mut session = Session::new(rows, cols);
    if renderer.draw_all(session.screen(), pixels).is_none() {
        return 23;
    }

    let import_id = next_id();
    if !send_request(
        surface,
        import_id,
        Request::ImportAttach(ImportAttachBuffer {
            identity,
            transfer_id: 1,
            buffer_cap_provenance: BUFFER_CAP_PROVENANCE,
        }),
    ) {
        return 24;
    }
    let mut ready = [0u8; 1];
    if sys_recv(surface, &mut ready) != 1 || ready[0] != b'R' {
        return 25;
    }
    let compositor = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    if sys_grant(surface, buffer, compositor) != 0
        || recv_reply(surface, OP_IMPORT_ATTACH_BUFFER, import_id).is_err()
    {
        return 26;
    }
    let mut serial = 1u64;
    if commit(
        surface,
        buffer,
        identity,
        next_id(),
        serial,
        (0, HEIGHT as u16),
    )
    .is_err()
    {
        return 27;
    }

    let Ok(focus) = sys_input_focus_create() else {
        return 28;
    };
    let Ok(activator) = sys_input_focus_mint_activator(focus) else {
        return 29;
    };
    let focus_id = next_id();
    if !send_request(
        surface,
        focus_id,
        Request::Focus(FocusRequest {
            identity,
            reason: FOCUS_REASON_USER_ACTIVATION,
            request_serial: 1,
        }),
    ) || recv_reply(surface, OP_FOCUS_REQUEST, focus_id).is_err()
    {
        return 30;
    }
    if sys_grant(
        MAIN,
        activator,
        CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE),
    ) != 0
    {
        return 31;
    }

    // The term page (§3.2): the terminal's own RW mapping; the child maps
    // its copy with SYS_STDIO_TERM_MAP.
    let page_desc = encode_buffer_desc(1024, 1, 4096, FORMAT_XRGB8888_LE, PAGE_LEN as u64);
    let Ok(term) = sys_buffer_create(&page_desc) else {
        return 40;
    };
    let Ok(page_ptr) = sys_buffer_map(term, 0) else {
        return 41;
    };
    // SAFETY: a 4096-byte RW mapping of the page buffer, 4-aligned (a page),
    // never unmapped while the process lives; every access is an atomic word
    // (the child writes its words concurrently, §3.2).
    let words = unsafe { core::slice::from_raw_parts(page_ptr as *const AtomicU32, WORDS) };
    let Some(page) = Page::new(words) else {
        return 42;
    };
    page.init(Size {
        cols: geo.cols,
        rows: geo.rows,
        width_px: WIDTH as u16,
        height_px: HEIGHT as u16,
    });
    let mut child = match spawn_child(term) {
        Ok(child) => child,
        Err(code) => return code,
    };
    session.reset_for_child();
    log_ready(&geo, child.process);

    let mut dumper = Dumper::new();
    let mut last_dump = 0u64;
    let mut turns = 0u64;
    let mut dropped_logged = 0u64;
    let mut in_front = false;
    // Wakes in front, so a scenario can see End process land within one.
    let mut wakes = 0u64;
    loop {
        if !in_front {
            let mut byte = [0u8; 1];
            match sys_recv(MAIN, &mut byte) {
                1 if byte[0] == b'K' => {
                    // Discard what the queue kept from before the turn
                    // (bounded: a focus queue holds at most 64 events).
                    let mut event = [0u8; INPUT_EVENT_V1_LEN];
                    let mut discarded = 0u64;
                    while discarded < 256 && sys_input_focus_recv(focus, &mut event) == 0 {
                        discarded += 1;
                    }
                    if sys_send(MAIN, b"D") != 1 {
                        end_child(&child);
                        return 32;
                    }
                    turns += 1;
                    in_front = true;
                    log_pair(b"TERM turn n=", turns, b" discarded=", discarded);
                }
                1 => {
                    end_child(&child);
                    return 33;
                }
                EPIPE => {
                    end_child(&child);
                    return 0;
                }
                _ => {
                    end_child(&child);
                    return 34;
                }
            }
        }

        let mut items = [WaitItem {
            handle: focus.0,
            kind: WAIT_KIND_INPUT,
        }; 2];
        let mut n = 1;
        if let Some(out) = child.stdout {
            items[1] = WaitItem {
                handle: out.0,
                kind: WAIT_KIND_PIPE,
            };
            n = 2;
        }
        let deadline = now_ns().saturating_add(WAKE_NS);
        let ready = sys_wait_any(&items[..n], deadline);
        if ready == EAGAIN {
            libneo::sys_sched_yield();
        }
        wakes += 1;
        // The first wakes, for a scenario's diagnosis: each step's end, in
        // microseconds since the wake.
        let woke = now_ns();
        let trace = |step: &[u8]| {
            if wakes <= TRACE {
                log_pair(b"TERM step=", 0, step, now_ns().saturating_sub(woke) / 1000);
            }
        };
        if wakes <= TRACE {
            log_signed_pair(b"TERM wake n=", wakes, b" ready=", ready);
        }

        // 1. The child's mode request, before any key of this wake.
        if let Some((mode, q)) = page.poll_mode_request(session.mode_seq()) {
            session.set_mode(mode, q, &mut child.stdin);
            page.ack_mode(session.mode(), q);
            log_pair(
                b"TERM mode bits=",
                u64::from(mode.bits()),
                b" seq=",
                u64::from(q),
            );
        }

        trace(b" mode us=");
        // 2. At most READ_BUDGET bytes of output (§3.3).
        let mut streaming = false;
        if let Some(out) = child.stdout {
            let mut buf = [0u8; 4096];
            let mut taken = 0;
            while taken < READ_BUDGET {
                match sys_pipe_read(out, &mut buf, PIPE_NONBLOCK) {
                    n if n > 0 => {
                        session.output(&buf[..n as usize], &mut child.stdin);
                        taken += n as usize;
                        streaming = true;
                    }
                    0 => {
                        // EOF: every writer is gone.
                        let _ = sys_channel_close(out);
                        child.stdout = None;
                        break;
                    }
                    _ => break,
                }
            }
        }

        trace(b" output us=");
        // 3. Keys.
        for _ in 0..KEYS_PER_WAKE {
            let mut event = [0u8; INPUT_EVENT_V1_LEN];
            match sys_input_focus_recv(focus, &mut event) {
                0 if event[0] == libneo::INPUT_EVENT_KEY && event[1] & 1 != 0 => {
                    let code = u16::from_le_bytes([event[2], event[3]]);
                    let mods = event[4];
                    let Ok(usage) = u8::try_from(code) else {
                        continue;
                    };
                    match session.key(usage, mods, &mut child.stdin) {
                        KeyAction::None => {}
                        KeyAction::EndProcess => {
                            if !child.exited {
                                let _ = sys_kill(child.process);
                                log_pair(b"TERM end-process wake=", wakes, b" dropped=", 0);
                            }
                        }
                        KeyAction::CloseStdin => {
                            if let Some(h) = child.stdin.0.take() {
                                let _ = sys_channel_close(h);
                                log_line(b"TERM stdin closed\n");
                            }
                        }
                    }
                }
                // A release, a pointer record: nothing to do.
                0 => {}
                EAGAIN => break,
                // The keyboard is another client's, or the session is
                // locked: park for the next turn.
                EPERM => {
                    in_front = false;
                    break;
                }
                _ => {
                    end_child(&child);
                    return 35;
                }
            }
        }
        if session.dropped() != dropped_logged {
            dropped_logged = session.dropped();
            log_pair(b"TERM stdin full dropped=", dropped_logged, b" bell=", 1);
        }
        let _ = session.take_bell();

        trace(b" keys us=");
        // 4. The child's exit (a killed child keeps no code: ESRCH).
        if !child.exited {
            match sys_wait(child.process, WAIT_FLAG_WNOHANG) {
                Ok(code) => {
                    child.exited = true;
                    session.child_exited(Some(i64::from(code)));
                    log_value(b"TERM child exited value=", i64::from(code));
                }
                Err(EAGAIN) => {}
                Err(ESRCH) | Err(_) => {
                    child.exited = true;
                    session.child_exited(None);
                    log_pair(
                        b"TERM child ended wake=",
                        wakes,
                        b" dropped=",
                        session.dropped(),
                    );
                }
            }
        }

        trace(b" exit us=");
        // 5. Draw what changed and commit its band.
        let drawn = renderer.draw(session.screen(), pixels);
        trace(b" draw us=");
        if let Some(band) = drawn {
            serial += 1;
            if serial <= TRACE + 1 {
                log_pair(
                    b"TERM commit y=",
                    u64::from(band.0),
                    b" h=",
                    u64::from(band.1),
                );
            }
            if commit(surface, buffer, identity, next_id(), serial, band).is_err() {
                end_child(&child);
                return 36;
            }
            if serial <= TRACE + 1 {
                log_pair(b"TERM commit done serial=", serial, b" wake=", wakes);
            }
        }
        let now = now_ns();
        if !streaming || now.saturating_sub(last_dump) >= DUMP_NS {
            last_dump = now;
            dump_rows(&mut dumper, &session);
        }

        // The session ending closes MAIN (EPIPE).
        if sys_try_recv(MAIN, &mut [0u8; 1]) == EPIPE {
            end_child(&child);
            return 0;
        }
    }
}

/// The child on its pipes and the page (stdin 0, stdout 1, term 2). The
/// terminal keeps the stdin write end and the stdout read end; the child's
/// ends are its copies alone once the terminal closes its own.
fn spawn_child(term: Handle) -> Result<Child, i32> {
    let (in_rd, in_wr) = sys_pipe_create(STDIN_CAPACITY).map_err(|_| 43)?;
    let (out_rd, out_wr) = sys_pipe_create(STDOUT_CAPACITY).map_err(|_| 44)?;
    let spawned = sys_spawn_stdio(CHILD, &[in_rd, out_wr, term], Some(0), Some(1), Some(2));
    let _ = sys_channel_close(in_rd);
    let _ = sys_channel_close(out_wr);
    let (_pid, process) = spawned.map_err(|_| 45)?;
    Ok(Child {
        process,
        stdin: Stdin(Some(in_wr)),
        stdout: Some(out_rd),
        exited: false,
    })
}

/// The terminal's own exit ends its child (§3.3: never outlives its window).
fn end_child(child: &Child) {
    if !child.exited {
        let _ = sys_kill(child.process);
    }
}

fn now_ns() -> u64 {
    sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0)
}

/// The rows that changed, as `TERM row=N |text|` lines (the scenarios'
/// read-back of what the window shows).
fn dump_rows(dumper: &mut Dumper, session: &Session) {
    for (row, text) in dumper.changed(session.screen()) {
        let mut line: Vec<u8> = Vec::with_capacity(text.len() + 24);
        line.extend_from_slice(b"TERM row=");
        push_dec(&mut line, u64::from(row));
        line.extend_from_slice(b" |");
        line.extend_from_slice(printable(&text).as_bytes());
        line.extend_from_slice(b"|\n");
        log_line(&line);
    }
}

/// A row's text with anything that would end the serial line escaped.
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

fn log_ready(geo: &neo_terminal::render::Geometry, _child: ProcessHandle) {
    let mut line: Vec<u8> = Vec::new();
    line.extend_from_slice(b"TERM ready cols=");
    push_dec(&mut line, u64::from(geo.cols));
    line.extend_from_slice(b" rows=");
    push_dec(&mut line, u64::from(geo.rows));
    line.extend_from_slice(b" child=");
    line.extend_from_slice(CHILD.as_bytes());
    line.push(b'\n');
    log_line(&line);
}

fn commit(
    surface: Handle,
    buffer: Handle,
    identity: Identity,
    request_id: u64,
    serial: u64,
    (y, height): (u16, u16),
) -> Result<(), ()> {
    let damage = encode_damage_rect(0, y, WIDTH as u16, height);
    if sys_buffer_commit_damage(buffer, &damage) != 0 {
        return Err(());
    }
    let request = Request::CommitDamage(CommitDamage {
        identity,
        client_serial: serial,
        x: 0,
        y,
        width: WIDTH as u16,
        height,
    });
    if !send_request(surface, request_id, request) {
        return Err(());
    }
    recv_reply(surface, OP_COMMIT_DAMAGE, request_id)
        .map(|_| ())
        .map_err(|_| ())
}

fn send_request(channel: Handle, request_id: u64, request: Request) -> bool {
    let (bytes, len) = encode_request(request_id, request);
    sys_send(channel, &bytes[..len]) == len as i64
}

fn recv_reply(channel: Handle, opcode: u16, request_id: u64) -> Result<Identity, i32> {
    let mut bytes = [0u8; REPLY_LEN];
    if sys_recv(channel, &mut bytes) != REPLY_LEN as i64 {
        return Err(1);
    }
    let reply = decode_reply(&bytes).map_err(|_| 2)?;
    if reply.opcode != opcode || reply.request_id != request_id || reply.status != 0 {
        return Err(3);
    }
    Ok(reply.identity)
}

/// The broker grants a channel over `chan` (O / R / grant / A).
fn recv_grant(chan: Handle) -> Result<Handle, i32> {
    let mut byte = [0u8; 1];
    if sys_recv(chan, &mut byte) != 1 || byte[0] != b'O' {
        return Err(1);
    }
    if sys_send(chan, b"R") != 1 {
        return Err(2);
    }
    let granted = libneo::sys_recv_grant(chan, libneo::FULL_CHANNEL_RIGHTS).map_err(|_| 3)?;
    if sys_send(chan, b"A") != 1 {
        return Err(4);
    }
    Ok(granted)
}

fn push_dec(out: &mut Vec<u8>, mut value: u64) {
    let mut digits = [0u8; 20];
    let mut n = 0;
    loop {
        digits[n] = b'0' + (value % 10) as u8;
        n += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    out.extend(digits[..n].iter().rev());
}

fn log_pair(prefix: &[u8], a: u64, middle: &[u8], b: u64) {
    let mut line: Vec<u8> = Vec::new();
    line.extend_from_slice(prefix);
    push_dec(&mut line, a);
    line.extend_from_slice(middle);
    push_dec(&mut line, b);
    line.push(b'\n');
    log_line(&line);
}

fn log_signed_pair(prefix: &[u8], a: u64, middle: &[u8], b: i64) {
    let mut line: Vec<u8> = Vec::new();
    line.extend_from_slice(prefix);
    push_dec(&mut line, a);
    line.extend_from_slice(middle);
    if b < 0 {
        line.push(b'-');
    }
    push_dec(&mut line, b.unsigned_abs());
    line.push(b'\n');
    log_line(&line);
}

fn log_value(prefix: &[u8], value: i64) {
    let mut line: Vec<u8> = Vec::new();
    line.extend_from_slice(prefix);
    if value < 0 {
        line.push(b'-');
    }
    push_dec(&mut line, value.unsigned_abs());
    line.push(b'\n');
    log_line(&line);
}

fn log_line(bytes: &[u8]) {
    let _ = libneo::sys_write(1, bytes.as_ptr(), bytes.len());
}
