// SPDX-License-Identifier: MPL-2.0
//! OT-61 L3: the display list layout sent for the L2 oracle page, rendered
//! by WebRender 0.70 on SWGL into a caller-owned buffer (#25's recipe,
//! `userland/webrender-smoke`), and every pixel checked.
//!
//! The oracle page (`neo-static-page/src/oracle.rs`) has no text and no
//! background colours, and one border: `#b`'s 2 px solid black, on the
//! border box x 8..122, y 90..144 (L2 checked that box). All its edges are
//! whole CSS px at a 1.0 device scale, so nothing is anti-aliased: each of
//! the 800 x 600 pixels is exactly black (inside the border ring) or white
//! (WebRender's clear colour, as no element paints a background).

use std::ffi::c_void;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

use gleam::gl;
use webrender::api::units::DeviceIntSize;
use webrender::api::{
    ColorF, DocumentId, FramePublishId, FrameReadyParams, RenderNotifier, RenderReasons,
};
use webrender::{Transaction, WebRenderOptions, create_webrender_instance};

use crate::{TAG, check};

const W: usize = 800;
const H: usize = 600;
const STRIDE: usize = W * 4;

/// BGRA bytes of an opaque colour (SWGL's memory order).
const fn bgra(r: u8, g: u8, b: u8) -> [u8; 4] {
    [b, g, r, 0xFF]
}
const WHITE: [u8; 4] = bgra(0xFF, 0xFF, 0xFF);
const BLACK: [u8; 4] = bgra(0, 0, 0);

/// The hand-derived frame: `#b`'s border ring is black, everything else white.
fn expected(x: usize, y: usize) -> [u8; 4] {
    let inside = |x0, y0, x1, y1| x >= x0 && x < x1 && y >= y0 && y < y1;
    // Border box 8,90 114x54; the border is 2 px on every side.
    if inside(8, 90, 122, 144) && !inside(10, 92, 120, 142) {
        BLACK
    } else {
        WHITE
    }
}

fn pixel(buf: &[u8], x: usize, y: usize, flipped: bool) -> [u8; 4] {
    let row = if flipped { H - 1 - y } else { y };
    let at = row * STRIDE + x * 4;
    [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
}

/// The first pixel that disagrees with the expected frame, and how many do.
fn mismatches(buf: &[u8], flipped: bool) -> (Option<(usize, usize, [u8; 4], [u8; 4])>, usize) {
    let mut first = None;
    let mut n = 0;
    for y in 0..H {
        for x in 0..W {
            let (got, want) = (pixel(buf, x, y, flipped), expected(x, y));
            if got != want {
                n += 1;
                first.get_or_insert((x, y, got, want));
            }
        }
    }
    (first, n)
}

/// Wakes the waiting thread when WebRender has a frame ready.
#[derive(Clone)]
struct Notifier(Arc<(Mutex<bool>, Condvar)>);

impl RenderNotifier for Notifier {
    fn clone(&self) -> Box<dyn RenderNotifier> {
        Box::new(Clone::clone(self))
    }
    fn wake_up(&self, _composite_needed: bool) {}
    fn new_frame_ready(&self, _: DocumentId, _: FramePublishId, _: &FrameReadyParams) {
        let (m, cv) = &*self.0;
        *m.lock().unwrap() = true;
        cv.notify_all();
    }
}

impl Notifier {
    fn wait(&self) {
        let (m, cv) = &*self.0;
        let mut ready = m.lock().unwrap();
        while !*ready {
            ready = cv.wait(ready).unwrap();
        }
        *ready = false;
    }
}

/// Render the page's display list and check every pixel (checks 60-62).
pub fn render_and_check(page: &neo_static_page::StaticPage) {
    let taken = page.take_display_list();
    check(
        60,
        "layout's display list was taken for WebRender",
        taken.is_some(),
    );
    let (info, list) = taken.expect("checked");

    let mut fb = vec![0x5Au8; STRIDE * H];
    let ctx = swgl::Context::create();
    ctx.make_current();
    ctx.init_default_framebuffer(
        0,
        0,
        W as i32,
        H as i32,
        STRIDE as i32,
        fb.as_mut_ptr().cast::<c_void>(),
    );
    let gl: Rc<dyn gl::Gl> = Rc::new(ctx);
    let notifier = Notifier(Arc::new((Mutex::new(false), Condvar::new())));
    let opts = WebRenderOptions {
        clear_color: ColorF::WHITE,
        panic_on_gl_error: true,
        // As in #25: SWGL rasterizes only LESS/LEQUAL depth tests, so the
        // Intel clear-with-quads workaround stays off.
        clear_caches_with_quads: false,
        ..WebRenderOptions::default()
    };
    let (mut renderer, sender) = match create_webrender_instance(
        gl.clone(),
        Box::new(Clone::clone(&notifier)),
        opts,
        None,
    ) {
        Ok(r) => r,
        Err(e) => {
            println!("{TAG} create_webrender_instance: {e:?}");
            check(
                61,
                "WebRender starts on SWGL and renders the page's frame",
                false,
            );
            unreachable!()
        }
    };

    let mut api = sender.create_api();
    let size = DeviceIntSize::new(W as i32, H as i32);
    let doc = api.add_document(size);
    let mut txn = Transaction::new();
    txn.set_display_list(info.epoch.into(), (info.pipeline_id, list));
    txn.set_root_pipeline(info.pipeline_id);
    txn.generate_frame(0, true, false, RenderReasons::empty());
    api.send_transaction(doc, txn);
    notifier.wait();
    renderer.update();
    let rendered = renderer.render(size, 0);
    if let Err(e) = &rendered {
        println!("{TAG} render: {e:?}");
    }
    // SWGL queues draws; asking for the colour buffer with `flush` runs
    // them into our buffer (as webrender-smoke does before its read).
    let (ptr, w, h, stride) = ctx.get_color_buffer(0, true);
    let ours = ptr == fb.as_mut_ptr().cast::<c_void>()
        && (w, h, stride) == (W as i32, H as i32, STRIDE as i32);
    if !ours {
        println!("{TAG} colour buffer {ptr:?} {w}x{h} stride {stride}");
    }
    check(
        61,
        "WebRender renders the page's frame into our 800x600 buffer",
        rendered.is_ok() && ours,
    );

    let (first, n) = mismatches(&fb, true);
    if let Some((x, y, got, want)) = first {
        println!("{TAG} {n} wrong pixels; first at ({x},{y}): got {got:02x?} want {want:02x?}");
        let (other, m) = mismatches(&fb, false);
        println!(
            "{TAG} read without the GL flip: {m} wrong{}",
            other.map_or(String::new(), |(x, y, g, w)| format!(
                ", first at ({x},{y}) got {g:02x?} want {w:02x?}"
            ))
        );
    }
    check(
        62,
        "every pixel == the hand-derived frame (white; #b's 2 px border black)",
        n == 0,
    );

    drop(api);
    renderer.deinit();
}
