// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 4, SWGL step S3b, the client side: a browser-shaped process
//! renders with SWGL into an Atrium buffer and hands a separate compositor
//! process a read-only view of it over a channel.
//!
//! The kernel scenario (`swgl_grant_smoke`) gives this process one end of a
//! channel and `swgl-grant-compositor` the other. Here:
//! - create the buffer (7x5, stride 36), map it RW, fill the page with
//!   `0xCD`, and draw S1's pattern into it with SWGL (`pattern.rs`);
//! - flush, and commit the full-surface damage;
//! - grant the buffer over the channel offering `USE` and
//!   `MAP_READ | QUERY | PRESENT`: no `MAP_WRITE`, no `COMMIT`, no `DERIVE`,
//!   no `GRANT`. The rendezvous orders the render before the compositor's
//!   first read;
//! - receive the compositor's verdict: one byte, 0 for a pass, else the
//!   number of its failed check.
//!
//! Exit 0 when the compositor passed. A failed check prints `FAIL(n)` and
//! exits `n`; a compositor failure `v` exits `100 + v`.

#![no_main]
#![feature(neoplutarch_ext)]

use std::ffi::c_void;
use std::os::neoplutarch::atrium::{self, BufferDesc, META_USE, Role, rights};
use std::os::neoplutarch::{HandleKind, channel, handles};

use gleam::gl::{self, Gl};
use neo_std_pal::SyscallHeap;

mod pattern;
use pattern::{FILL, H, PAGE, STRIDE, W};

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ swgl-grant-client ]";

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

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    neo_crt::init();
    println!("{TAG} start: render into an Atrium buffer, grant the compositor a read-only view");

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

    let desc = BufferDesc::xrgb(W as u32, H as u32, STRIDE as u32);
    let buf = must(
        2,
        "create a 7x5 XRGB buffer, stride 36",
        atrium::create(desc),
    );
    let rw = must(
        3,
        "map the client view (RW)",
        atrium::map(buf, Role::Client),
    );
    // SAFETY: the client mapping covers the buffer's one page and is ours
    // until the unmap below.
    unsafe { std::ptr::write_bytes(rw, FILL, PAGE) };

    let ctx = swgl::Context::create();
    ctx.make_current();
    ctx.init_default_framebuffer(0, 0, W as i32, H as i32, STRIDE as i32, rw.cast::<c_void>());
    ctx.clear_color(1.0, 0.0, 0.0, 1.0);
    ctx.clear(gl::COLOR_BUFFER_BIT);
    ctx.enable(gl::SCISSOR_TEST);
    ctx.scissor(1, 1, 3, 2);
    ctx.clear_color(0.0, 0.0, 1.0, 1.0);
    ctx.clear(gl::COLOR_BUFFER_BIT);
    ctx.scissor(5, 0, 2, 5);
    ctx.clear_color(0.0, 1.0, 0.0, 0.0);
    ctx.clear(gl::COLOR_BUFFER_BIT);
    ctx.disable(gl::SCISSOR_TEST);
    check(
        4,
        "gl calls raised no error",
        ctx.get_error() == gl::NO_ERROR,
    );
    let (ptr, ..) = ctx.get_color_buffer(0, true);
    check(
        5,
        "SWGL flushed into the Atrium client view",
        ptr == rw.cast::<c_void>(),
    );
    must(
        6,
        "commit the full-surface damage",
        atrium::commit_damage(buf, 0, 0, W as u16, H as u16),
    );

    must(
        7,
        "grant the compositor USE; MAP_READ|QUERY|PRESENT",
        channel::grant(
            chan,
            buf,
            META_USE,
            rights::MAP_READ | rights::QUERY | rights::PRESENT,
        ),
    );
    println!("{TAG} granted; waiting for the compositor's verdict");

    let mut verdict = [0xFFu8; 1];
    let n = must(
        8,
        "receive the compositor's verdict",
        channel::recv(chan, &mut verdict),
    );
    check(8, "the verdict is one byte", n == 1);
    if verdict[0] != 0 {
        println!(
            "{TAG} FAIL(compositor {}) the compositor's check failed",
            verdict[0]
        );
        std::process::exit(100 + verdict[0] as i32);
    }
    println!("{TAG} ok(9) the compositor read the pattern through its own view");

    ctx.destroy();
    // SAFETY: the client view isn't used past this point.
    unsafe { must(10, "unmap the client view", atrium::unmap(rw)) };
    println!("{TAG} PASS");
    std::process::exit(0)
}
