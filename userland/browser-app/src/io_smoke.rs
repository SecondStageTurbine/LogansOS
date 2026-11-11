// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 6: the std surface a window's loop runs on, checked on-target
//! (`browser_io_smoke`). A window polls two things each turn, the WM's
//! session channel and its own input focus, and must park on neither.
//!
//! The kernel gives this process one end of a channel and `browser-io-peer`
//! the other. Here:
//! - `channel::try_recv` on the channel before the peer has sent is
//!   `WouldBlock`, deterministically: the peer sends only after this
//!   process's "go";
//! - after the go, polling `try_recv` (yielding between tries) takes the
//!   peer's `W` exactly once;
//! - `input::create` gives a focus whose handle can `RECEIVE` and not
//!   `ACTIVATE`; `input::recv` on it is `PermissionDenied`, because no WM has
//!   made it the live route;
//! - `input::mint_activator` gives its sibling: `ACTIVATE` only, `USE|GRANT`,
//!   no `DERIVE`. The sibling can't receive, and can't mint a sibling of its
//!   own (both `PermissionDenied`).
//!
//! Exit 0 on a pass; a failed check prints `FAIL(n)` and exits `n`.

#![no_main]
#![feature(neoplutarch_ext)]

use std::io::ErrorKind;
use std::os::neoplutarch::{HandleKind, channel, handle_info, handles, input};

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ browser-io-smoke ]";

/// The meta rights (the kernel's `meta_rights`).
const META_USE: u8 = 1 << 0;
const META_DERIVE: u8 = 1 << 1;
const META_GRANT: u8 = 1 << 2;

fn check(n: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({n}) {what}");
    } else {
        println!("{TAG} FAIL({n}) {what}");
        std::process::exit(n);
    }
}

fn must<T>(n: i32, what: &str, r: std::io::Result<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => {
            println!("{TAG} FAIL({n}) {what}: {e:?}");
            std::process::exit(n);
        }
    }
}

fn kind_of<T>(r: std::io::Result<T>) -> Option<ErrorKind> {
    r.err().map(|e| e.kind())
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("{TAG} start: try_recv and input focus from std");

    let chans: Vec<_> = handles()
        .filter(|(_, info)| info.kind == HandleKind::Channel)
        .map(|(h, _)| h)
        .collect();
    check(
        1,
        "the launcher handed exactly one channel",
        chans.len() == 1,
    );
    let chan = chans[0];

    let mut buf = [0u8; 4];
    check(
        2,
        "try_recv before the peer sends is WouldBlock",
        kind_of(channel::try_recv(chan, &mut buf)) == Some(ErrorKind::WouldBlock),
    );
    must(3, "send the peer its go", channel::send(chan, b"G"));
    let mut tries = 0u64;
    let got = loop {
        tries += 1;
        match channel::try_recv(chan, &mut buf) {
            Ok(n) => break n,
            Err(e) if e.kind() == ErrorKind::WouldBlock && tries < 1_000_000 => {
                std::thread::yield_now()
            }
            Err(e) => {
                println!("{TAG} FAIL(4) try_recv after the go: {e:?} after {tries} tries");
                std::process::exit(4);
            }
        }
    };
    println!("{TAG} try_recv took the message after {tries} tries");
    check(4, "try_recv takes the peer's W", got == 1 && buf[0] == b'W');

    let focus = must(5, "create an input focus", input::create());
    let info = must(5, "the focus's record", handle_info(focus));
    println!(
        "{TAG} focus {focus}: {:?}, meta {:#04x}, rights {:#04x}",
        info.kind, info.cap_meta, info.cap_semantic
    );
    check(
        5,
        "the focus receives and can't activate",
        info.kind == HandleKind::InputFocus && info.cap_semantic == input::RECEIVE,
    );
    check(
        6,
        "recv on a focus no WM made live is PermissionDenied",
        kind_of(input::recv(focus)) == Some(ErrorKind::PermissionDenied),
    );

    let activator = must(7, "mint the activator", input::mint_activator(focus));
    let info = must(7, "the activator's record", handle_info(activator));
    println!(
        "{TAG} activator {activator}: {:?}, meta {:#04x}, rights {:#04x}",
        info.kind, info.cap_meta, info.cap_semantic
    );
    check(
        7,
        "the activator is ACTIVATE only, USE|GRANT, no DERIVE",
        info.kind == HandleKind::InputFocus
            && info.cap_semantic == input::ACTIVATE
            && info.cap_meta == META_USE | META_GRANT
            && info.cap_meta & META_DERIVE == 0,
    );
    check(
        8,
        "the activator can't receive (PermissionDenied)",
        kind_of(input::recv(activator)) == Some(ErrorKind::PermissionDenied),
    );
    check(
        9,
        "the activator can't mint an activator (PermissionDenied)",
        kind_of(input::mint_activator(activator)) == Some(ErrorKind::PermissionDenied),
    );

    println!("{TAG} PASS");
    std::process::exit(0)
}
