// SPDX-License-Identifier: MPL-2.0
//! DS-1c: the Lens — a read-only directory instrument bloomed from Arthur's
//! `list /` action. The session broker (neo-init) spawns it with its
//! session channel (`MAIN`) and one directory opened `LOOKUP | READDIR`
//! (handle 2; handle 1 is a closed placeholder, since file I/O reserves fd 1
//! for stdout) — nothing else. It lists that directory, draws the names
//! into the buffer the broker lends it, commits once and then waits on its
//! session channel until the broker closes it. It never holds an input focus.
//!
//! Chad DS-1c F1: the buffer is the WM's, created once and lent to each Lens
//! (map-write, query, commit; never grant), so a Lens that fails — or is
//! retried — leaves the WM no new mapping or handle; the Lens's own handle
//! and mapping go with its process.
//!
//! The same binary is DS-1b's probe: spawned with no handles, it exits 1 at
//! once, as before.

use libneo::atrium_launch::{LENS_HEIGHT, LENS_NAMESPACE, LENS_WIDTH};
use libneo::atrium_protocol::{
    Commit, CreateSurface, Identity, ImportAttachBuffer, OP_COMMIT, OP_CREATE_SURFACE,
    OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Request, decode_reply, encode_request,
};
use libneo::{
    ATRIUM_COMMIT, ATRIUM_MAP_WRITE, ATRIUM_QUERY, CapRights, EBADF, FORMAT_XRGB8888_LE,
    FileHandle, Handle, NEO_DIRENT_KIND_DIRECTORY, encode_buffer_desc, encode_damage_rect,
    meta_rights, parse_dirent_v1, sys_buffer_commit_damage, sys_buffer_map, sys_readdir, sys_recv,
    sys_send,
};
use neo_text::fonts::INTER_REGULAR;
use neo_text::layout::draw_line;
use neo_text::{Canvas, Font};

const MAIN: Handle = Handle(0);
/// Index 1 is a closed placeholder: file I/O reserves fd 1 (stdout).
const DIR: FileHandle = FileHandle(2);

const WIDTH: usize = LENS_WIDTH as usize;
const HEIGHT: usize = LENS_HEIGHT as usize;
const STRIDE_BYTES: usize = WIDTH * 4;
const BUFFER_BYTES: u64 = (WIDTH * HEIGHT * 4) as u64;
const NS: u64 = LENS_NAMESPACE;
const BUFFER_CAP_PROVENANCE: u64 = 0x4C45_4E53_4255_0001;

/// The scene's content floor and label colours (neo-atrium spec v1.4).
const FLOOR: u32 = 0x000A_0E27;
const PRIMARY: u32 = 0x00FF_FFFF;
const SECONDARY: u32 = 0x00C8_CDD2;
const INSET: f32 = 16.0;
const LINE: usize = 22;
const MAX_ENTRIES: usize = 15;
const NAME_MAX: usize = 48;

pub fn run() -> i32 {
    let create_identity = Identity {
        namespace_id: NS,
        surface_id: 0,
        surface_cap_provenance: 0,
    };
    let (bytes, len) = encode_request(
        1,
        Request::Create(CreateSurface {
            identity: create_identity,
            role: ROLE_TEXT,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            client_tag: u64::from(b'L'),
        }),
    );
    match sys_send(MAIN, &bytes[..len]) {
        n if n == len as i64 => {}
        // DS-1b's probe: no session handle.
        EBADF => return 1,
        _ => return 10,
    }
    let identity = match recv_reply(MAIN, OP_CREATE_SURFACE, 1) {
        Ok(identity) => identity,
        Err(code) => return 11 + code,
    };
    let surface = match recv_surface_cap() {
        Ok(surface) => surface,
        Err(code) => return 15 + code,
    };

    let buffer = match recv_lent_buffer(surface) {
        Ok(buffer) => buffer,
        Err(code) => return 20 + code,
    };
    let pixels = match sys_buffer_map(buffer, 0) {
        Ok(ptr) => ptr as *mut u32,
        Err(_) => return 25,
    };
    // SAFETY: the RW mapping covers the validated descriptor: exactly
    // WIDTH * HEIGHT tightly packed pixels. Only this Lens writes it while
    // it lives (the WM maps it read-only; one Lens at a time).
    let pixels = unsafe { core::slice::from_raw_parts_mut(pixels, WIDTH * HEIGHT) };
    if let Err(code) = draw(pixels) {
        return code;
    }
    let full = encode_damage_rect(0, 0, WIDTH as u16, HEIGHT as u16);
    if sys_buffer_commit_damage(buffer, &full) != 0 {
        return 30;
    }

    if !send_request(
        surface,
        2,
        Request::ImportAttach(ImportAttachBuffer {
            identity,
            transfer_id: 1,
            buffer_cap_provenance: BUFFER_CAP_PROVENANCE,
        }),
    ) {
        return 31;
    }
    if recv_reply(surface, OP_IMPORT_ATTACH_BUFFER, 2).is_err() {
        return 34;
    }
    if !send_request(
        surface,
        3,
        Request::Commit(Commit {
            identity,
            client_serial: 1,
        }),
    ) || recv_reply(surface, OP_COMMIT, 3).is_err()
    {
        return 35;
    }

    // Static from here: wait until the broker ends the session. Only its
    // close (EPIPE) is a clean end; any other error is the Lens's own exit
    // code, so "lens ended exit=0" means the Lens stayed up until then.
    let mut byte = [0u8; 1];
    loop {
        match sys_recv(MAIN, &mut byte) {
            libneo::EPIPE => return 0,
            n if n < 0 => return 40,
            _ => {}
        }
    }
}

/// The floor, the path, then up to `MAX_ENTRIES` names (directories end in
/// `/`) and a count of the rest.
fn draw(pixels: &mut [u32]) -> Result<(), i32> {
    pixels.fill(FLOOR);
    let mut font = Font::from_bytes(INTER_REGULAR).ok_or(22)?;
    let mut canvas = Canvas::new(pixels, WIDTH, HEIGHT, WIDTH).ok_or(23)?;
    let clip = neo_text::Rect::new(0, 0, WIDTH as i32, HEIGHT as i32);
    draw_line(
        &mut canvas,
        &mut font,
        12.0,
        "/",
        INSET,
        26,
        SECONDARY,
        clip,
    );
    let mut buf = [0u8; 512];
    let mut cursor = 0u64;
    let mut shown = 0usize;
    let mut more = 0usize;
    loop {
        let n = sys_readdir(DIR, &mut buf, &mut cursor);
        if n == 0 {
            break;
        }
        if n < 0 {
            // 200 + errno: the broker logs it as the Lens's exit.
            return Err(200 + (-n).min(55) as i32);
        }
        let mut off = 0usize;
        while off < n as usize {
            let entry = parse_dirent_v1(&buf[off..n as usize]).map_err(|_| 25)?;
            off += entry.record_len as usize;
            if shown == MAX_ENTRIES {
                more += 1;
                continue;
            }
            let mut name = [0u8; NAME_MAX + 1];
            let take = entry.name.len().min(NAME_MAX);
            name[..take].copy_from_slice(&entry.name[..take]);
            let mut len = take;
            if entry.kind == NEO_DIRENT_KIND_DIRECTORY {
                name[len] = b'/';
                len += 1;
            }
            let text = core::str::from_utf8(&name[..len]).unwrap_or("?");
            let base = (54 + shown * LINE) as i32;
            draw_line(
                &mut canvas,
                &mut font,
                14.0,
                text,
                INSET,
                base,
                PRIMARY,
                clip,
            );
            shown += 1;
        }
    }
    let footer = if more > 0 {
        "and more\u{2026}"
    } else if shown == 0 {
        "(empty)"
    } else {
        ""
    };
    if !footer.is_empty() {
        let base = (54 + shown * LINE) as i32;
        draw_line(
            &mut canvas,
            &mut font,
            12.0,
            footer,
            INSET,
            base,
            SECONDARY,
            clip,
        );
    }
    Ok(())
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

/// The broker lends its Lens buffer over the surface channel (O / R / grant
/// / A); it must be exactly the Lens surface's descriptor.
fn recv_lent_buffer(surface: Handle) -> Result<Handle, i32> {
    let mut byte = [0u8; 1];
    if sys_recv(surface, &mut byte) != 1 || byte[0] != b'O' {
        return Err(1);
    }
    if sys_send(surface, b"R") != 1 {
        return Err(2);
    }
    let lent = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_WRITE | ATRIUM_QUERY | ATRIUM_COMMIT,
    );
    let buffer = libneo::sys_recv_grant(surface, lent).map_err(|_| 3)?;
    let expected = encode_buffer_desc(
        WIDTH as u32,
        HEIGHT as u32,
        STRIDE_BYTES as u32,
        FORMAT_XRGB8888_LE,
        BUFFER_BYTES,
    );
    let mut desc = [0u8; libneo::BUFFER_DESC_V1_LEN];
    if libneo::sys_buffer_query(buffer, &mut desc) != 0 || desc != expected {
        return Err(4);
    }
    if sys_send(surface, b"A") != 1 {
        return Err(4);
    }
    Ok(buffer)
}

/// The broker grants the surface channel over the session channel
/// (O / R / grant / A, as for Arthur).
fn recv_surface_cap() -> Result<Handle, i32> {
    let mut byte = [0u8; 1];
    if sys_recv(MAIN, &mut byte) != 1 || byte[0] != b'O' {
        return Err(1);
    }
    if sys_send(MAIN, b"R") != 1 {
        return Err(2);
    }
    let surface = libneo::sys_recv_grant(MAIN, libneo::FULL_CHANNEL_RIGHTS).map_err(|_| 3)?;
    if sys_send(MAIN, b"A") != 1 {
        return Err(4);
    }
    Ok(surface)
}
