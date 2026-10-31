// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 4, SWGL step S3a: SWGL rasterizes straight into a kernel
//! Atrium shared pixel buffer, and the compositor's read-only view of that
//! buffer sees the exact bytes (zero copy: the AtriumSwglRenderingContext
//! shape, `docs/THINK-2026-10-31-ot61-r4-swgl-floor.md` §3).
//!
//! The process plays both roles in one address space, through two handles:
//! - the root handle from `atrium::create` (all buffer rights) is the
//!   client: it maps the buffer read-write and hands that mapping to SWGL as
//!   its default framebuffer, with the descriptor's padded stride;
//! - a handle derived from it with `USE` and `MAP_READ | QUERY | PRESENT`
//!   (what the compositor is given; no `MAP_WRITE`, no `COMMIT`) maps the
//!   same frames read-only at a second address.
//!
//! The client fills its whole mapping (one page) with `0xCD`, so the stride
//! padding and the rest of the page are canaries. SWGL then draws the S1
//! pattern (`main.rs`: red, a scissored blue block, a scissored green strip
//! with alpha 0). After `get_color_buffer(0, flush = true)` and a commit of
//! the full-surface damage, every byte of the page, read through the
//! compositor's view, is compared with the hand-laid expectation. The
//! derived handle's attenuation is checked too: it cannot map the client
//! view or commit, and a damage rectangle past the descriptor is refused.
//!
//! Exit 0 on a pass. A failed check prints `FAIL(n)` and exits `n`.

#![no_main]
#![feature(neoplutarch_ext)]

use std::ffi::c_void;
use std::os::neoplutarch::atrium::{self, BufferDesc, META_USE, Role, rights};

use gleam::gl::{self, Gl};
use neo_std_pal::SyscallHeap;

mod pattern;
use pattern::{FILL, H, PAGE, STRIDE, W};

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ swgl-atrium-smoke ]";

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

const EPERM: i32 = 1;
const EINVAL: i32 = 22;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    neo_crt::init();
    println!(
        "{TAG} start: swgl 0.70.0 into an Atrium buffer, read back through the compositor's view"
    );

    let desc = BufferDesc::xrgb(W as u32, H as u32, STRIDE as u32);
    let client = must(
        1,
        "create a 7x5 XRGB buffer, stride 36",
        atrium::create(desc),
    );
    let rw = must(
        2,
        "map the client view (RW)",
        atrium::map(client, Role::Client),
    );
    // SAFETY: the client mapping covers the buffer's one page and is ours
    // until the unmap below.
    unsafe { std::ptr::write_bytes(rw, FILL, PAGE) };

    let compositor = must(
        3,
        "derive the compositor handle (USE; MAP_READ|QUERY|PRESENT)",
        atrium::derive(
            client,
            META_USE,
            rights::MAP_READ | rights::QUERY | rights::PRESENT,
        ),
    );
    let ro = must(
        4,
        "map the compositor view (RO)",
        atrium::map(compositor, Role::Compositor),
    );
    println!("{TAG} handles client={client} compositor={compositor}; views rw={rw:p} ro={ro:p}");
    check(5, "the two views are distinct mappings", rw != ro);
    check(
        6,
        "the compositor handle queries the same descriptor",
        atrium::query(compositor).ok() == Some(desc),
    );
    check(
        7,
        "the compositor handle cannot map the client view (EPERM)",
        atrium::map(compositor, Role::Client)
            .err()
            .and_then(|e| e.raw_os_error())
            == Some(EPERM),
    );
    check(
        8,
        "the compositor handle cannot commit (EPERM)",
        atrium::commit_damage(compositor, 0, 0, W as u16, H as u16)
            .err()
            .and_then(|e| e.raw_os_error())
            == Some(EPERM),
    );

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
        9,
        "gl calls raised no error",
        ctx.get_error() == gl::NO_ERROR,
    );

    let (ptr, w, h, stride) = ctx.get_color_buffer(0, true);
    check(
        10,
        "get_color_buffer is the Atrium client view, 7x5, stride 36",
        ptr == rw.cast::<c_void>() && w == W as i32 && h == H as i32 && stride == STRIDE as i32,
    );

    check(
        11,
        "damage past the descriptor is refused (EINVAL)",
        atrium::commit_damage(client, 5, 0, 3, 1)
            .err()
            .and_then(|e| e.raw_os_error())
            == Some(EINVAL),
    );
    must(
        12,
        "commit the full-surface damage",
        atrium::commit_damage(client, 0, 0, W as u16, H as u16),
    );

    // SAFETY: the compositor view covers the same page, mapped readable.
    let seen = unsafe { std::slice::from_raw_parts(ro, PAGE) };
    let first_bad = pattern::first_wrong(TAG, seen);
    check(
        13,
        "the compositor view == the hand-laid pattern (BGRA, no flip, padding and the rest of the page untouched)",
        first_bad.is_none(),
    );

    ctx.destroy();
    // SAFETY: neither view is used past this point.
    unsafe {
        must(14, "unmap the compositor view", atrium::unmap(ro));
        must(14, "unmap the client view", atrium::unmap(rw));
    }
    println!("{TAG} PASS");
    std::process::exit(0)
}
