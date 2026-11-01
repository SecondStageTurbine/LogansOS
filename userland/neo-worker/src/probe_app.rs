// SPDX-License-Identifier: MPL-2.0
//! OT-88 A1a: the probe app, a second focusable WM client for the app-model
//! scenarios (`docs/THINK-2026-10-29-ot88-app-model.md` §2.3, §7). The
//! session broker (neo-init) spawns it with its session channel (`MAIN`)
//! and nothing else.
//!
//! It runs the generic ADR-073 handshake neo-init's `connect` expects, as
//! Arthur does: a `ROLE_TEXT` surface, the surface channel granted, the `B`
//! byte, its own buffer imported read-only by the WM, a first commit of the
//! whole surface, a focus request, and an ACTIVATE-only activator granted to
//! the WM. Then it serves turns:
//! - **`K`** on the session channel: the WM has moved the keyboard route to
//!   it. It discards what its queue held from before (`atrium_handoff`'s
//!   discard step), and answers `D`.
//! - **A key press** on its own focus: it counts it, logs
//!   `PROBEAPP key code=C count=N`, redraws its count and commits.
//! - **The session channel closing** (`EPIPE`): it exits 0.
//!
//! `probe-app-exit-after-key` (test only): it exits 7 after its first key,
//! for the per-client fault scenario. The WM-liveness modes (app-model THINK
//! §2.3 items 7 and 8), test only:
//! - `probe-app-hang-reply`: after its first key it sends a commit and never
//!   receives the reply (it spins);
//! - `probe-app-stall-handshake`: it never sends its `CREATE` (it spins);
//! - `probe-app-drop-reply`: after its first key it sends a commit and exits
//!   9 before the reply.

use libneo::atrium_protocol::{
    CommitDamage, CreateSurface, FOCUS_REASON_USER_ACTIVATION, FocusRequest, Identity,
    ImportAttachBuffer, OP_COMMIT_DAMAGE, OP_CREATE_SURFACE, OP_FOCUS_REQUEST,
    OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Request, decode_reply, encode_request,
};
use libneo::{
    ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, EAGAIN, EBADF,
    EPERM, EPIPE, FORMAT_XRGB8888_LE, Handle, INPUT_EVENT_V1_LEN, encode_buffer_desc,
    encode_damage_rect, meta_rights, sys_buffer_commit_damage, sys_buffer_create, sys_buffer_map,
    sys_grant, sys_input_focus_create, sys_input_focus_mint_activator, sys_input_focus_recv,
    sys_recv, sys_send, sys_try_recv,
};
use neo_text::fonts::INTER_REGULAR;
use neo_text::layout::draw_line;
use neo_text::{Canvas, Font};

const MAIN: Handle = Handle(0);

/// The WM's client size (neo-init's `CLIENT_W` x `CLIENT_H`).
const WIDTH: usize = 640;
const HEIGHT: usize = 400;
const STRIDE_BYTES: usize = WIDTH * 4;
const BUFFER_BYTES: u64 = (WIDTH * HEIGHT * 4) as u64;
const NS: u64 = libneo::atrium_launch::PROBE_APP_NAMESPACE;
const BUFFER_CAP_PROVENANCE: u64 = libneo::atrium_launch::PROBE_APP_BUFFER_PROVENANCE;

/// The scene's content floor and label colours (neo-atrium spec v1.4).
const FLOOR: u32 = 0x000A_0E27;
const PRIMARY: u32 = 0x00FF_FFFF;
const SECONDARY: u32 = 0x00C8_CDD2;

/// The liveness modes' end state: run, ignoring everything, until killed.
fn spin_forever() -> ! {
    loop {
        libneo::sys_sched_yield();
    }
}

pub fn run() -> i32 {
    // Only as the probe app (spawned with its session channel): the same
    // binary is DS-1b's handle-less probe, which must still exit at once.
    if cfg!(feature = "probe-app-stall-handshake") && sys_try_recv(MAIN, &mut [0u8; 1]) != EBADF {
        log_line(b"PROBEAPP stalling handshake\n");
        spin_forever();
    }
    let mut request_id = 0u64;
    let mut next_id = || {
        request_id += 1;
        request_id
    };

    let create_id = next_id();
    let (bytes, len) = encode_request(
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
            client_tag: u64::from(b'P'),
        }),
    );
    match sys_send(MAIN, &bytes[..len]) {
        n if n == len as i64 => {}
        // Spawned with no handles (DS-1b's probe, which is this binary in a
        // probe-app image): exit 1 at once, as the Lens does.
        EBADF => return 1,
        _ => return 10,
    }
    let identity = match recv_reply(MAIN, OP_CREATE_SURFACE, create_id) {
        Ok(identity) => identity,
        Err(code) => return 11 + code,
    };
    let surface = match recv_grant(MAIN) {
        Ok(surface) => surface,
        Err(code) => return 15 + code,
    };
    // The WM reads this byte where Arthur proves its scanout bypass refused;
    // the probe app holds no scanout authority to prove.
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
    if draw(pixels, 0, 0).is_err() {
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
    if commit(surface, buffer, identity, next_id(), serial).is_err() {
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
    log_line(b"PROBEAPP ready\n");

    let mut keys = 0u64;
    let mut turns = 0u64;
    // Out of front (its focus answers EPERM) a client has nothing to receive
    // but the WM's `K`, so it parks in its session receive: the WM's `K` is a
    // try (A0k), which only lands on a parked receiver (app-model THINK §2.3
    // item 7). In front it polls, keys and the session channel both.
    let mut in_front = false;
    loop {
        let mut byte = [0u8; 1];
        let got = if in_front {
            sys_try_recv(MAIN, &mut byte)
        } else {
            sys_recv(MAIN, &mut byte)
        };
        match got {
            1 if byte[0] == b'K' => {
                // PR #15 review item 2 (test only): die mid-handoff, after
                // the WM swapped the route to us and before our `D`.
                if cfg!(feature = "probe-app-die-on-discard") {
                    log_line(b"PROBEAPP dying before discard ack\n");
                    return 5;
                }
                // Discard what the queue kept from before the turn (bounded:
                // a focus queue holds at most 64 events).
                let mut event = [0u8; INPUT_EVENT_V1_LEN];
                let mut discarded = 0;
                while discarded < 256 && sys_input_focus_recv(focus, &mut event) == 0 {
                    discarded += 1;
                }
                if sys_send(MAIN, b"D") != 1 {
                    return 32;
                }
                turns += 1;
                in_front = true;
                log_count(b"PROBEAPP turn n=", turns, b" discarded=", discarded as u64);
            }
            1 => return 33,
            EAGAIN => {}
            EPIPE => return 0,
            _ => return 34,
        }
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        match sys_input_focus_recv(focus, &mut event) {
            0 if event[0] == 1 && event[1] & 1 != 0 => {
                keys += 1;
                let code = u16::from_le_bytes([event[2], event[3]]);
                log_count(b"PROBEAPP key code=", u64::from(code), b" count=", keys);
                if draw(pixels, keys, code).is_err() {
                    return 35;
                }
                serial += 1;
                if cfg!(any(
                    feature = "probe-app-hang-reply",
                    feature = "probe-app-drop-reply"
                )) {
                    let full = encode_damage_rect(0, 0, WIDTH as u16, HEIGHT as u16);
                    let _ = sys_buffer_commit_damage(buffer, &full);
                    let request = Request::CommitDamage(CommitDamage {
                        identity,
                        client_serial: serial,
                        x: 0,
                        y: 0,
                        width: WIDTH as u16,
                        height: HEIGHT as u16,
                    });
                    if !send_request(surface, next_id(), request) {
                        return 38;
                    }
                    if cfg!(feature = "probe-app-drop-reply") {
                        // Exit, which closes every endpoint (a plain close of
                        // this handle can't: SYS_CLOSE refuses index 1 as
                        // stdout, a kernel quirk reported with this leg).
                        log_line(b"PROBEAPP exiting before reply\n");
                        return 9;
                    } else {
                        log_line(b"PROBEAPP ignoring reply\n");
                    }
                    spin_forever();
                }
                if commit(surface, buffer, identity, next_id(), serial).is_err() {
                    return 36;
                }
                if cfg!(feature = "probe-app-exit-after-key") {
                    return 7;
                }
            }
            // A release, a pointer record, nothing queued, or (EPERM) the
            // keyboard is another client's.
            0 | EAGAIN => {}
            EPERM => in_front = false,
            _ => return 37,
        }
        libneo::sys_sched_yield();
    }
}

/// The floor, a title, and the key count with the last key's code.
fn draw(pixels: &mut [u32], keys: u64, code: u16) -> Result<(), ()> {
    pixels.fill(FLOOR);
    let mut font = Font::from_bytes(INTER_REGULAR).ok_or(())?;
    let mut canvas = Canvas::new(pixels, WIDTH, HEIGHT, WIDTH).ok_or(())?;
    let clip = neo_text::Rect::new(0, 0, WIDTH as i32, HEIGHT as i32);
    draw_line(
        &mut canvas,
        &mut font,
        14.0,
        "Probe app",
        24.0,
        40,
        PRIMARY,
        clip,
    );
    let mut line = [0u8; 48];
    line[..5].copy_from_slice(b"keys ");
    let mut len = 5;
    len += write_dec(&mut line[len..], keys);
    line[len..len + 6].copy_from_slice(b"  last");
    len += 6;
    line[len] = b' ';
    len += 1;
    len += write_dec(&mut line[len..], u64::from(code));
    let text = core::str::from_utf8(&line[..len]).map_err(|_| ())?;
    draw_line(
        &mut canvas,
        &mut font,
        12.0,
        text,
        24.0,
        70,
        SECONDARY,
        clip,
    );
    Ok(())
}

fn commit(
    surface: Handle,
    buffer: Handle,
    identity: Identity,
    request_id: u64,
    serial: u64,
) -> Result<(), ()> {
    let full = encode_damage_rect(0, 0, WIDTH as u16, HEIGHT as u16);
    if sys_buffer_commit_damage(buffer, &full) != 0 {
        return Err(());
    }
    let request = Request::CommitDamage(CommitDamage {
        identity,
        client_serial: serial,
        x: 0,
        y: 0,
        width: WIDTH as u16,
        height: HEIGHT as u16,
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

fn write_dec(out: &mut [u8], mut value: u64) -> usize {
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
    for i in 0..n {
        out[i] = digits[n - 1 - i];
    }
    n
}

fn log_count(prefix: &[u8], a: u64, middle: &[u8], b: u64) {
    let mut line = [0u8; 96];
    let mut len = prefix.len();
    line[..len].copy_from_slice(prefix);
    len += write_dec(&mut line[len..], a);
    line[len..len + middle.len()].copy_from_slice(middle);
    len += middle.len();
    len += write_dec(&mut line[len..], b);
    line[len] = b'\n';
    log_line(&line[..len + 1]);
}

fn log_line(bytes: &[u8]) {
    let _ = libneo::sys_write(1, bytes.as_ptr(), bytes.len());
}
