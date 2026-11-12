// SPDX-License-Identifier: MPL-2.0
//! OT-61 L5: `neo-browser`, the first page in a LogansOS window
//! (`docs/THINK-2026-11-08-ot61-first-page-window.md`).
//!
//! An A1a client of neo-init's desktop session, like the probe app
//! (`userland/neo-worker/src/probe_app.rs`), on real std. The session
//! channel is its only channel. In order:
//!
//! 1. It joins the session at once, because the WM's handshake is bounded
//!    (2 s from the spawn):
//!    - `CREATE_SURFACE`, then the surface channel by grant (O / R / grant / A);
//!    - `B`;
//!    - it makes its 640 x 400 XRGB Atrium buffer, fills it white, and sends
//!      `IMPORT_ATTACH_BUFFER`, then `R`, then the buffer granted with `USE`
//!      and `MAP_READ | QUERY | PRESENT`;
//!    - a full-surface commit of the white frame (`commit_damage`, then
//!      `COMMIT_DAMAGE`);
//!    - `FOCUS_REQUEST`, with an ACTIVATE-only activator granted on the
//!      session channel.
//! 2. It answers the WM's first `K` (the launch brings it to the front).
//! 3. It lays out L2's oracle page (`neo-static-page/src/oracle.rs`) at a
//!    640 x 400 viewport, and has WebRender on SWGL render it into the
//!    mapping (`l3::render_into`). SWGL writes in GL row order, bottom row
//!    first, so it flips the rows.
//! 4. It checks every pixel against the hand-derived frame: white, and
//!    `#b`'s 2 px black ring on its border box x 8..122, y 90..144. That box
//!    doesn't depend on the viewport width (CSS 2.1 §10.3.3).
//! 5. It logs the frame's FNV-1a hash over its RGB bytes, row by row, and
//!    commits the frame (serial 2). The WM presents a shown client's commit
//!    before replying, so after the reply the page is on screen. The
//!    scenario hashes the screen's client rect the same way, and the two
//!    must match.
//!
//! The browser never presents: the WM does. After the handshake it answers
//! the WM's `K` (keyboard handed over) with `D`, as the probe app does, and
//! exits 0 when the session channel closes. A failure prints `FAIL(n)` and
//! exits `n`.

#![no_main]
#![feature(random)]
#![feature(neoplutarch_ext)]

mod l3;

use std::io::ErrorKind;
use std::os::neoplutarch::atrium::{self, BufferDesc, META_USE, Role, rights};
use std::os::neoplutarch::io::RawHandle;
use std::os::neoplutarch::{HandleKind, channel, handles, input};

use nmc1_client::{
    FOCUS_REASON_USER_ACTIVATION, Identity, Message, OP_COMMIT_DAMAGE, OP_CREATE_SURFACE,
    OP_FOCUS_REQUEST, OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT,
};

pub const TAG: &str = "[ neo-browser ]";

const W: usize = 640;
const H: usize = 400;
const STRIDE: usize = W * 4;

/// The browser's surface namespace and buffer provenance (libneo's
/// `atrium_launch::BROWSER_*`, which the WM keys its import by).
const NAMESPACE: u64 = 0x4252_5753_4150_0001;
const BUFFER_PROVENANCE: u64 = 0x4252_5753_4255_0001;

/// The kernel's channel rights a granted surface channel carries (libneo's
/// `FULL_CHANNEL_RIGHTS`): every meta right, and SEND | RECV | CLOSE.
const CHANNEL_META: u8 = 0x0F;
const CHANNEL_SEMANTIC: u8 = 0x07;

pub fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({code}) {what}");
    } else {
        println!("{TAG} FAIL({code}) {what}");
        std::process::exit(code);
    }
}

fn must<T>(code: i32, what: &str, r: std::io::Result<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => {
            println!("{TAG} FAIL({code}) {what}: {e:?}");
            std::process::exit(code);
        }
    }
}

/// getrandom 0.4's custom backend, as in `main.rs`.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom04::Error> {
    use std::random::{DefaultRandomSource, RandomSource};
    // SAFETY: getrandom passes a writable buffer of `len` bytes.
    DefaultRandomSource.fill_bytes(unsafe { std::slice::from_raw_parts_mut(dest, len) });
    Ok(())
}

/// The hand-derived frame, top row first: `#b`'s border ring is black,
/// everything else white. Pixels are XRGB8888 in memory: B, G, R, X.
fn expected(x: usize, y: usize) -> [u8; 3] {
    let inside = |x0, y0, x1, y1| x >= x0 && x < x1 && y >= y0 && y < y1;
    if inside(8, 90, 122, 144) && !inside(10, 92, 120, 142) {
        [0, 0, 0]
    } else {
        [0xFF, 0xFF, 0xFF]
    }
}

/// FNV-1a 64 over the frame's RGB bytes, row by row (the scenario computes
/// the same over the screendump's client rect).
fn frame_hash(buf: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for y in 0..H {
        for x in 0..W {
            let p = &buf[y * STRIDE + x * 4..][..3];
            for b in [p[2], p[1], p[0]] {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

fn send(chan: RawHandle, m: &Message) -> bool {
    matches!(channel::send(chan, m.as_bytes()), Ok(n) if n == m.len)
}

/// The WM's reply to request `id` (`opcode`), which must be a success.
fn reply(code: i32, chan: RawHandle, opcode: u16, id: u64) -> Identity {
    let mut b = [0u8; REPLY_LEN];
    let n = must(code, "receive the WM's reply", channel::recv(chan, &mut b));
    match nmc1_client::decode_reply(&b[..n]) {
        Ok(r) if r.opcode == opcode && r.request_id == id && r.status == 0 => r.identity,
        other => {
            println!("{TAG} FAIL({code}) the WM's reply to opcode {opcode}: {other:?}");
            std::process::exit(code);
        }
    }
}

/// The WM grants a channel over `chan`: O, R, the grant, A.
fn recv_channel(code: i32, chan: RawHandle) -> RawHandle {
    let mut b = [0u8; 1];
    let offered = matches!(channel::recv(chan, &mut b), Ok(1)) && b[0] == b'O';
    check(code, "the WM offers the surface channel (O)", offered);
    must(code, "answer R", channel::send(chan, b"R"));
    let got = must(
        code,
        "receive the surface channel",
        channel::recv_grant(chan, CHANNEL_META, CHANNEL_SEMANTIC),
    );
    must(code, "acknowledge A", channel::send(chan, b"A"));
    got
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    neo_crt::init();
    println!("{TAG} start: the oracle page in a LogansOS window (640x400)");

    let session = handles()
        .find(|(_, info)| info.kind == HandleKind::Channel)
        .map(|(h, _)| h);
    let Some(session) = session else {
        // Spawned with no session channel: nothing to join.
        check(80, "the launcher handed us a session channel", false);
        unreachable!()
    };

    // 1. Join the session at once: the WM's handshake is bounded (2 s from
    // the spawn, neo-init's `HANDSHAKE_NS`), and a render takes far longer
    // under TCG (the first on-target run, 2026-11-08: the WM gave up on us).
    // The first frame is plain white, so it costs nothing to draw.
    let mut id = 0u64;
    let mut next = || {
        id += 1;
        id
    };
    let unnamed = Identity {
        namespace_id: NAMESPACE,
        surface_id: 0,
        surface_cap_provenance: 0,
    };
    let create = next();
    let m = nmc1_client::create_surface(create, unnamed, ROLE_TEXT, W as u32, H as u32, 0x42);
    check(85, "send CREATE_SURFACE", send(session, &m));
    let identity = reply(86, session, OP_CREATE_SURFACE, create);
    let surface = recv_channel(87, session);
    must(88, "send B", channel::send(session, b"B"));

    let desc = BufferDesc::xrgb(W as u32, H as u32, STRIDE as u32);
    let buffer = must(81, "create the 640x400 XRGB buffer", atrium::create(desc));
    let base = must(82, "map it read-write", atrium::map(buffer, Role::Client));
    // SAFETY: the client mapping covers the buffer's STRIDE * H bytes and is
    // this process's until it exits; only this thread touches it, apart from
    // the render thread, which the main thread joins before reading it.
    let fb = unsafe { std::slice::from_raw_parts_mut(base, STRIDE * H) };
    fb.fill(0xFF);

    let import = next();
    let m = nmc1_client::import_attach_buffer(import, identity, 1, BUFFER_PROVENANCE);
    check(89, "send IMPORT_ATTACH_BUFFER", send(surface, &m));
    let mut b = [0u8; 1];
    let ready = matches!(channel::recv(surface, &mut b), Ok(1)) && b[0] == b'R';
    check(90, "the WM is ready for the buffer (R)", ready);
    must(
        91,
        "grant the buffer: USE, MAP_READ | QUERY | PRESENT",
        channel::grant(
            surface,
            buffer,
            META_USE,
            rights::MAP_READ | rights::QUERY | rights::PRESENT,
        ),
    );
    reply(92, surface, OP_IMPORT_ATTACH_BUFFER, import);
    let mut serial = 1u64;
    commit_frame(93, buffer, surface, identity, next(), serial);
    println!("{TAG} ok(94) the WM took the first (white) frame");

    let focus = must(97, "create an input focus", input::create());
    let activator = must(98, "mint its activator", input::mint_activator(focus));
    let focus_id = next();
    let m = nmc1_client::focus_request(focus_id, identity, FOCUS_REASON_USER_ACTIVATION, 1);
    check(99, "send FOCUS_REQUEST", send(surface, &m));
    reply(100, surface, OP_FOCUS_REQUEST, focus_id);
    must(
        101,
        "grant the WM the ACTIVATE-only activator",
        channel::grant(session, activator, META_USE, input::ACTIVATE),
    );
    println!("{TAG} ready");

    // 2. The WM brings a launched client to the front at once, and hands it
    // the keyboard: its `K` waits a bounded time (`DISCARD_NS`, 1 s) for our
    // `D`, so it is answered before the render.
    turn(session, focus);

    // 3-5. Render, flip, check, commit. Layout runs on an 8 MiB thread, as
    // Servo's script thread does (layout-smoke's L2). The mapping goes over
    // as an address: the thread is joined before the mapping is touched again.
    let addr = base as usize;
    let rendered = std::thread::Builder::new()
        .name("static-page".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let mut page =
                neo_static_page::StaticPage::new(neo_static_page::oracle::PAGE, W as f32, H as f32);
            if !page.reflow(W as f32, H as f32) {
                return Err("reflow");
            }
            // SAFETY: as above; the main thread waits in `join`.
            let fb = unsafe { std::slice::from_raw_parts_mut(addr as *mut u8, STRIDE * H) };
            l3::render_into(&page, fb, W, H, STRIDE).map_err(|_| "render")
        })
        .expect("spawn the static-page thread")
        .join();
    match rendered {
        Ok(Ok(())) => check(83, "the page rendered into the Atrium buffer", true),
        Ok(Err(stage)) => {
            println!("{TAG} {stage} failed");
            check(83, "the page rendered into the Atrium buffer", false);
        }
        Err(_) => check(83, "the static-page thread finished", false),
    }
    for y in 0..H / 2 {
        let (top, bottom) = fb.split_at_mut((H - 1 - y) * STRIDE);
        top[y * STRIDE..(y + 1) * STRIDE].swap_with_slice(&mut bottom[..STRIDE]);
    }
    let mut wrong = 0usize;
    let mut first = None;
    for y in 0..H {
        for x in 0..W {
            let p = &fb[y * STRIDE + x * 4..][..3];
            let got = [p[2], p[1], p[0]];
            if got != expected(x, y) {
                wrong += 1;
                first.get_or_insert((x, y, got));
            }
        }
    }
    if let Some((x, y, got)) = first {
        println!("{TAG} {wrong} wrong pixels; first at ({x},{y}): rgb {got:02x?}");
    }
    check(
        84,
        "every pixel of the 640x400 buffer == the hand-derived frame, top row first",
        wrong == 0,
    );
    println!("{TAG} frame fnv1a64={:016x}", frame_hash(fb));
    serial += 1;
    commit_frame(95, buffer, surface, identity, next(), serial);
    // The WM presents a shown client's commit before it replies, so the page
    // is on screen now (the scenario's capture waits for this line).
    println!("{TAG} ok(96) the WM took the page frame");

    // The WM's later turns; a closed session channel ends the browser.
    loop {
        turn(session, focus);
    }
}

/// Wait for the WM's next `K` (it hands us the keyboard), drain what queued
/// before it, and answer `D`. A closed session channel ends the browser.
fn turn(session: RawHandle, focus: RawHandle) {
    let mut byte = [0u8; 1];
    match channel::recv(session, &mut byte) {
        Ok(1) if byte[0] == b'K' => {
            let mut drained = 0;
            while drained < 256 && input::recv(focus).is_ok() {
                drained += 1;
            }
            must(102, "answer D", channel::send(session, b"D"));
            println!("{TAG} turn: discarded {drained}");
        }
        Ok(_) => check(103, "the WM sent only K", false),
        Err(e) if e.kind() == ErrorKind::BrokenPipe => {
            println!("{TAG} session closed");
            std::process::exit(0)
        }
        Err(e) => {
            println!("{TAG} FAIL(104) session receive: {e:?}");
            std::process::exit(104);
        }
    }
}

/// Commit the whole surface: on the buffer, then `COMMIT_DAMAGE` with
/// `serial` (strictly increasing) on the surface channel, and the WM's
/// reply. Checks `code` and `code + 1`.
fn commit_frame(
    code: i32,
    buffer: RawHandle,
    surface: RawHandle,
    identity: Identity,
    request_id: u64,
    serial: u64,
) {
    must(
        code,
        "commit the whole surface's damage on the buffer",
        atrium::commit_damage(buffer, 0, 0, W as u16, H as u16),
    );
    let m = nmc1_client::commit_damage(request_id, identity, serial, (0, 0, W as u16, H as u16));
    check(code, "send COMMIT_DAMAGE", send(surface, &m));
    reply(code + 1, surface, OP_COMMIT_DAMAGE, request_id);
}
