// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 4, SWGL step S3b, the compositor side: a separate process that
//! receives the client's buffer over a channel and reads it through its own
//! read-only mapping. It renders nothing and links no SWGL code; the
//! expectation is the hand-laid page (`pattern.rs`).
//!
//! It asks for more than it is offered (every buffer right and every meta
//! right) and must get exactly the client's offer: `USE` and
//! `MAP_READ | QUERY | PRESENT`. What that handle can't do is asked:
//! - map the client (RW) view: `EPERM`;
//! - commit damage: `EPERM`;
//! - derive a narrower handle: `EPERM` (no `DERIVE`);
//! - grant it onwards: `EPERM` (no `GRANT`).
//!
//! The verdict goes back to the client as one byte (0 = pass, else the
//! failed check's number), so a failure here fails the client too. A check
//! that fails before the grant arrives just exits: the channel closes and
//! the client's grant gets `EPIPE`.
//!
//! Exit 0 on a pass; a failed check prints `FAIL(n)` and exits `n`.

#![no_main]
#![feature(neoplutarch_ext)]

use std::os::neoplutarch::atrium::{
    self, BufferDesc, META_DERIVE, META_GRANT, META_USE, Role, rights,
};
use std::os::neoplutarch::io::RawHandle;
use std::os::neoplutarch::{HandleKind, channel, handle_info, handles};
use std::sync::atomic::{AtomicU32, Ordering};

use neo_std_pal::SyscallHeap;

mod pattern;
use pattern::{H, PAGE, STRIDE, W};

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ swgl-grant-compositor ]";

const EPERM: i32 = 1;

/// The channel to the client once the grant has come (`NONE` before): a
/// failed check reports to it.
static CHAN: AtomicU32 = AtomicU32::new(NONE);
const NONE: RawHandle = RawHandle::MAX;

fn fail(n: i32, what: &str) -> ! {
    println!("{TAG} FAIL({n}) {what}");
    let chan = CHAN.load(Ordering::Relaxed);
    if chan != NONE {
        let _ = channel::send(chan, &[n as u8]);
    }
    std::process::exit(n);
}

fn check(n: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({n}) {what}");
    } else {
        fail(n, what);
    }
}

fn must<T>(n: i32, what: &str, r: std::io::Result<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => fail(n, &format!("{what}: {e:?}")),
    }
}

fn refused(r: std::io::Result<impl Sized>, errno: i32) -> bool {
    r.err().and_then(|e| e.raw_os_error()) == Some(errno)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("{TAG} start: receive the client's buffer, read it through a read-only view");

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

    let all =
        rights::MAP_WRITE | rights::MAP_READ | rights::QUERY | rights::PRESENT | rights::COMMIT;
    let buf = must(
        2,
        "receive the granted buffer (asking for every right)",
        channel::recv_grant(chan, META_USE | META_DERIVE | META_GRANT, all),
    );
    CHAN.store(chan, Ordering::Relaxed);

    let info = must(3, "the granted handle's record", handle_info(buf));
    println!(
        "{TAG} granted handle {buf}: {:?}, meta {:#04x}, rights {:#04x}",
        info.kind, info.cap_meta, info.cap_semantic
    );
    check(
        3,
        "it is a buffer with exactly the offer: USE; MAP_READ|QUERY|PRESENT",
        info.kind == HandleKind::Buffer
            && info.cap_meta == META_USE
            && info.cap_semantic == rights::MAP_READ | rights::QUERY | rights::PRESENT,
    );
    check(
        4,
        "it queries the client's descriptor (7x5, stride 36)",
        atrium::query(buf).ok() == Some(BufferDesc::xrgb(W as u32, H as u32, STRIDE as u32)),
    );
    check(
        5,
        "it cannot map the client view (EPERM)",
        refused(atrium::map(buf, Role::Client), EPERM),
    );
    check(
        6,
        "it cannot commit (EPERM)",
        refused(atrium::commit_damage(buf, 0, 0, W as u16, H as u16), EPERM),
    );
    check(
        7,
        "it cannot derive (EPERM, no DERIVE)",
        refused(atrium::derive(buf, META_USE, rights::MAP_READ), EPERM),
    );
    check(
        8,
        "it cannot be granted onwards (EPERM, no GRANT)",
        refused(channel::grant(chan, buf, META_USE, rights::MAP_READ), EPERM),
    );

    let ro = must(
        9,
        "map the compositor view (RO)",
        atrium::map(buf, Role::Compositor),
    );
    // SAFETY: the compositor view covers the buffer's page, mapped readable,
    // until the unmap below.
    let seen = unsafe { std::slice::from_raw_parts(ro, PAGE) };
    check(
        10,
        "the view == the hand-laid pattern (BGRA, no flip, padding and the rest of the page untouched)",
        pattern::first_wrong(TAG, seen).is_none(),
    );
    // SAFETY: the view isn't used past this point.
    unsafe { must(11, "unmap the compositor view", atrium::unmap(ro)) };

    must(12, "send the pass verdict", channel::send(chan, &[0]));
    println!("{TAG} PASS");
    std::process::exit(0)
}
