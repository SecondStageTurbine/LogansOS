// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 4, SWGL step S1: upstream SWGL (WebRender 0.70's software GL)
//! rasterizing into a caller-owned buffer on-target, over neo-crt.
//!
//! The buffer has a padded stride (8 bytes past each 28-byte row) and 16
//! canary bytes before and after; every byte starts as `0xCD`. Through gleam
//! calls, SWGL clears the whole framebuffer red, then (scissored) a 3x2 block
//! blue and a 2-column strip green with alpha 0. The pattern is asymmetric in
//! both axes, so a transposed, flipped or tight-stride write lands bytes in
//! the wrong place. After `get_color_buffer(0, flush = true)` (SWGL defers
//! clears), every byte is compared with the hand-laid expectation below:
//! - pixels in SWGL's BGRA memory order (the `tools/swgl-atrium-oracle`
//!   contract): red `[00,00,FF,FF]`, blue `[FF,00,00,FF]`, green-with-alpha-0
//!   `[00,FF,00,00]`; every colour component is 0.0 or 1.0, so no rounding
//!   question arises;
//! - row y at byte offset `y * stride` (GL's y = 0 is the first row in SWGL's
//!   memory; no flip);
//! - the stride padding and both canaries still `0xCD`.
//!
//! Exit 0 on a pass. A failed check prints `FAIL(n)` and exits `n`.

#![no_main]

use std::ffi::c_void;

use gleam::gl::{self, Gl};
use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ swgl-smoke ]";

const W: usize = 7;
const H: usize = 5;
const STRIDE: usize = W * 4 + 8;
const CANARY: usize = 16;
const FILL: u8 = 0xCD;

/// The expected framebuffer, row 0 (GL y = 0) first. R: red, B: blue,
/// G: green with alpha 0.
///   - the whole surface was cleared red;
///   - scissor (x 1, y 1, 3 wide, 2 high) cleared blue;
///   - scissor (x 5, y 0, 2 wide, 5 high) cleared green.
const PATTERN: [&str; H] = ["RRRRRGG", "RBBBRGG", "RBBBRGG", "RRRRRGG", "RRRRRGG"];

/// SWGL's BGRA memory order, written out by hand.
fn bgra(c: u8) -> [u8; 4] {
    match c {
        b'R' => [0x00, 0x00, 0xFF, 0xFF],
        b'B' => [0xFF, 0x00, 0x00, 0xFF],
        b'G' => [0x00, 0xFF, 0x00, 0x00],
        _ => unreachable!(),
    }
}

fn check(n: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({n}) {what}");
    } else {
        println!("{TAG} FAIL({n}) {what}");
        std::process::exit(n);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    neo_crt::init();
    println!("{TAG} start: swgl 0.70.0 (WebRender's software GL) over neo-crt");

    let mut mem = vec![FILL; CANARY + STRIDE * H + CANARY];
    let fb = unsafe { mem.as_mut_ptr().add(CANARY) };

    let ctx = swgl::Context::create();
    ctx.make_current();
    ctx.init_default_framebuffer(
        0,
        0,
        W as i32,
        H as i32,
        STRIDE as i32,
        fb.cast::<c_void>(),
    );
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
    check(1, "gl calls raised no error", ctx.get_error() == gl::NO_ERROR);

    let (ptr, w, h, stride) = ctx.get_color_buffer(0, true);
    println!("{TAG} color buffer: {ptr:p} {w}x{h} stride {stride} (caller's {fb:p})");
    check(
        2,
        "get_color_buffer is the caller's buffer, 7x5, stride 36",
        ptr == fb.cast::<c_void>() && w == W as i32 && h == H as i32 && stride == STRIDE as i32,
    );

    let mut want = vec![FILL; CANARY + STRIDE * H + CANARY];
    for (y, row) in PATTERN.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let at = CANARY + y * STRIDE + x * 4;
            want[at..at + 4].copy_from_slice(&bgra(c));
        }
    }
    let first_bad = mem.iter().zip(&want).position(|(a, b)| a != b);
    if let Some(i) = first_bad {
        println!(
            "{TAG} first wrong byte at {i} (row {}, byte {} of the row): got {:#04x} want {:#04x}",
            (i as isize - CANARY as isize).div_euclid(STRIDE as isize),
            (i as isize - CANARY as isize).rem_euclid(STRIDE as isize),
            mem[i],
            want[i]
        );
        for y in 0..H {
            let row = &mem[CANARY + y * STRIDE..CANARY + (y + 1) * STRIDE];
            println!("{TAG} row {y}: {row:02x?}");
        }
    }
    check(
        3,
        "every byte == the hand-laid pattern (BGRA, no flip, padding and canaries untouched)",
        first_bad.is_none(),
    );

    ctx.destroy();
    println!("{TAG} PASS");
    std::process::exit(0)
}
