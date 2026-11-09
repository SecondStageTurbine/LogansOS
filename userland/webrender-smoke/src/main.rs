// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 4, SWGL step S2: upstream WebRender 0.70 renders one display
//! list through SWGL into a caller-owned buffer, on-target
//! (`docs/THINK-2026-10-31-ot61-r4-swgl-floor.md` §2).
//!
//! The scene is 64x48 device pixels on a white clear colour:
//! 1. a solid red rect, (2,2)-(18,12);
//! 2. a blue rect (20,2)-(40,30) clipped to (24,6)-(32,14);
//! 3. a green rect 4x3 at the origin of a reference frame that scales by 2
//!    and sits at (2,32): it covers (2,32)-(10,38);
//! 4. a horizontal black-to-white linear gradient, (12,40)-(44,46);
//! 5. a 4x2 image of eight distinct colours, drawn 1:1 with pixelated
//!    sampling at (48,36);
//! 6. "Hi" in black, Roboto Slab (Apache-2.0, `third_party/ot61-swgl/fonts`)
//!    at 16 px, baseline origin (42,24): glyphs through the neoplutarch font
//!    backend (skrifa + zeno).
//!
//! The oracle is derived by hand from that list, not from a render:
//! - items 1, 2, 3 and 5 are compared exactly, pixel by pixel, over the whole
//!   surface: every pixel not covered is the white clear colour;
//! - the gradient is compared by its ends (within 16 of black and of white)
//!   and by being non-decreasing left to right, the same on every row;
//! - the text by values read from the font's own tables with a separate
//!   parser (not skrifa): `cmap` gives 'H' = glyph 63 and 'i' = glyph 255,
//!   which the backend's glyph lookup must return; `head`/`hhea`/`hmtx` give
//!   upem 2048, ascender 2146, descender -555 and advances 1578 + 652, so at
//!   16 px the run lies within x 42..59.4 and y 7.2..28.3. Some pixels in that
//!   box must be dark, and every pixel of the free area around it
//!   (x 40..64, y 0..36, outside the blue clip) must stay white.
//!
//! Orientation: WebRender draws its main framebuffer in GL's convention, so
//! the scene's row y is memory row `47 - y`. A wrong orientation fails, and
//! the failure says whether the flipped reading would have matched.
//!
//! Exit 0 on a pass. A failed check prints `FAIL(n)` and exits `n`.

#![no_main]

use std::ffi::c_void;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

use gleam::gl;
use neo_std_pal::SyscallHeap;
use webrender::api::units::*;
use webrender::api::*;
use webrender::{RenderApiSender, Transaction, WebRenderOptions, create_webrender_instance};

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ wr-smoke ]";
const W: usize = 64;
const H: usize = 48;
const STRIDE: usize = W * 4;
/// The text run's possible pixels, rounded outwards by one pixel: x 42..59.4
/// and y 7.2..28.3 (see the module doc).
const TEXT_BOX: (usize, usize, usize, usize) = (41, 6, 61, 30);
/// Roboto Slab's glyphs for 'H' and 'i', from its `cmap` (module doc).
const GLYPH_H: u32 = 63;
const GLYPH_I: u32 = 255;
/// The run's pen positions: 'H' at 42, 'i' one 'H' advance later
/// (1578 / 2048 * 16 px).
const PEN_H: f32 = 42.0;
const PEN_I: f32 = 42.0 + 1578.0 / 2048.0 * 16.0;
const BASELINE: f32 = 24.0;
static FONT: &[u8] =
    include_bytes!("../../../third_party/ot61-swgl/fonts/robotoslab/RobotoSlab-wght.ttf");

fn check(n: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({n}) {what}");
    } else {
        println!("{TAG} FAIL({n}) {what}");
        std::process::exit(n);
    }
}

/// Wakes the main thread when WebRender has a frame ready.
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

/// BGRA bytes of an opaque colour.
const fn bgra(r: u8, g: u8, b: u8) -> [u8; 4] {
    [b, g, r, 0xFF]
}
const WHITE: [u8; 4] = bgra(0xFF, 0xFF, 0xFF);
const RED: [u8; 4] = bgra(0xFF, 0, 0);
const BLUE: [u8; 4] = bgra(0, 0, 0xFF);
const GREEN: [u8; 4] = bgra(0, 0xFF, 0);
/// The image's texels, row 0 first, in RGBA order as handed to WebRender
/// (its `ImageFormat::BGRA8` is filled below from these).
const IMAGE: [[u8; 3]; 8] = [
    [0x10, 0x20, 0x30],
    [0x40, 0x50, 0x60],
    [0x70, 0x80, 0x90],
    [0xA0, 0xB0, 0xC0],
    [0xC1, 0x11, 0x22],
    [0x33, 0xC2, 0x44],
    [0x55, 0x66, 0xC3],
    [0x77, 0x88, 0x99],
];

/// The hand-derived expectation for every pixel except the gradient band:
/// `None` inside the gradient (checked separately), else the exact colour.
fn expected(x: usize, y: usize) -> Option<[u8; 4]> {
    let inside = |x0, y0, x1, y1| x >= x0 && x < x1 && y >= y0 && y < y1;
    if inside(12, 40, 44, 46) || inside(TEXT_BOX.0, TEXT_BOX.1, TEXT_BOX.2, TEXT_BOX.3) {
        return None;
    }
    if inside(48, 36, 52, 38) {
        let [r, g, b] = IMAGE[(y - 36) * 4 + (x - 48)];
        return Some(bgra(r, g, b));
    }
    if inside(2, 32, 10, 38) {
        return Some(GREEN);
    }
    if inside(24, 6, 32, 14) {
        return Some(BLUE);
    }
    if inside(2, 2, 18, 12) {
        return Some(RED);
    }
    Some(WHITE)
}

fn pixel(buf: &[u8], x: usize, y: usize, flipped: bool) -> [u8; 4] {
    let row = if flipped { H - 1 - y } else { y };
    let at = row * STRIDE + x * 4;
    [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
}

/// The first pixel that disagrees with the exact part of the oracle.
fn first_mismatch(buf: &[u8], flipped: bool) -> Option<(usize, usize, [u8; 4], [u8; 4])> {
    for y in 0..H {
        for x in 0..W {
            if let Some(want) = expected(x, y) {
                let got = pixel(buf, x, y, flipped);
                if got != want {
                    return Some((x, y, got, want));
                }
            }
        }
    }
    None
}

fn build_scene(
    api_sender: &RenderApiSender,
    notifier: &Notifier,
) -> (webrender::RenderApi, DocumentId, Vec<Option<u32>>) {
    let mut api = api_sender.create_api();
    let size = DeviceIntSize::new(W as i32, H as i32);
    let doc = api.add_document(size);
    let pipeline = PipelineId(0, 0);
    let root = SpaceAndClipInfo::root_scroll(pipeline);
    let mut txn = Transaction::new();

    // The font, first on its own transaction, so the backend can answer the
    // glyph lookup below.
    let font_key = api.generate_font_key();
    let mut font_txn = Transaction::new();
    font_txn.add_raw_font(font_key, FONT.to_vec(), 0);
    api.send_transaction(doc, font_txn);
    let glyph_ids = api.get_glyph_indices(font_key, "Hi");
    let font_instance = api.generate_font_instance_key();
    txn.add_font_instance(font_instance, font_key, 16.0, None, None, Vec::new());

    let image_key = api.generate_image_key();
    let mut texels = Vec::with_capacity(8 * 4);
    for [r, g, b] in IMAGE {
        texels.extend_from_slice(&bgra(r, g, b));
    }
    txn.add_image(
        image_key,
        ImageDescriptor::new(4, 2, ImageFormat::BGRA8, ImageDescriptorFlags::IS_OPAQUE),
        ImageData::new(texels),
        None,
    );

    let mut b = DisplayListBuilder::new(pipeline);
    b.begin();
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        LayoutRect::new(LayoutPoint::new(x0, y0), LayoutPoint::new(x1, y1))
    };

    // 1. solid red
    let r1 = rect(2.0, 2.0, 18.0, 12.0);
    b.push_rect(
        &CommonItemProperties::new(r1, root),
        r1,
        ColorF::new(1.0, 0.0, 0.0, 1.0),
    );

    // 2. blue, clipped
    let clip = b.define_clip_rect(root.spatial_id, rect(24.0, 6.0, 32.0, 14.0));
    let chain = b.define_clip_chain(None, [clip]);
    let clipped = SpaceAndClipInfo {
        spatial_id: root.spatial_id,
        clip_chain_id: chain,
    };
    let r2 = rect(20.0, 2.0, 40.0, 30.0);
    b.push_rect(
        &CommonItemProperties::new(r2, clipped),
        r2,
        ColorF::new(0.0, 0.0, 1.0, 1.0),
    );

    // 3. green under a scale-by-2 reference frame at (2,32)
    let frame = b.push_reference_frame(
        LayoutPoint::new(2.0, 32.0),
        root.spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(LayoutTransform::scale(2.0, 2.0, 1.0)),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: true,
            should_snap: false,
            paired_with_perspective: false,
        },
    );
    let in_frame = SpaceAndClipInfo {
        spatial_id: frame,
        clip_chain_id: ClipChainId::INVALID,
    };
    let r3 = rect(0.0, 0.0, 4.0, 3.0);
    b.push_rect(
        &CommonItemProperties::new(r3, in_frame),
        r3,
        ColorF::new(0.0, 1.0, 0.0, 1.0),
    );
    b.pop_reference_frame();

    // 4. a black-to-white gradient
    let r4 = rect(12.0, 40.0, 44.0, 46.0);
    let gradient = b.create_gradient(
        LayoutPoint::new(0.0, 0.0),
        LayoutPoint::new(32.0, 0.0),
        vec![
            GradientStop {
                offset: 0.0,
                color: ColorF::new(0.0, 0.0, 0.0, 1.0),
            },
            GradientStop {
                offset: 1.0,
                color: ColorF::new(1.0, 1.0, 1.0, 1.0),
            },
        ],
        ExtendMode::Clamp,
    );
    b.push_gradient(
        &CommonItemProperties::new(r4, root),
        r4,
        gradient,
        r4.size(),
        LayoutSize::zero(),
    );

    // 5. the image, 1:1, pixelated
    let r5 = rect(48.0, 36.0, 52.0, 38.0);
    b.push_image(
        &CommonItemProperties::new(r5, root),
        r5,
        ImageRendering::Pixelated,
        AlphaType::PremultipliedAlpha,
        image_key,
        ColorF::WHITE,
    );

    // 6. "Hi"
    let r6 = rect(40.0, 0.0, 64.0, 36.0);
    b.push_text(
        &CommonItemProperties::new(r6, root),
        r6,
        &[
            GlyphInstance {
                index: GLYPH_H,
                point: LayoutPoint::new(PEN_H, BASELINE),
            },
            GlyphInstance {
                index: GLYPH_I,
                point: LayoutPoint::new(PEN_I, BASELINE),
            },
        ],
        font_instance,
        ColorF::BLACK,
        None,
    );

    txn.set_display_list(Epoch(0), b.end());
    txn.set_root_pipeline(pipeline);
    txn.generate_frame(0, true, false, RenderReasons::empty());
    api.send_transaction(doc, txn);
    notifier.wait();
    (api, doc, glyph_ids)
}

/// WebRender's frames are large (`Shaders::new` alone reserves 10 KB in a
/// debug build), so the smoke runs on a thread with a 16 MiB stack, as
/// Firefox runs WebRender on threads of its own; the process's main stack
/// is small.
const RENDER_STACK: usize = 16 * 1024 * 1024;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    neo_crt::init();
    let worker = std::thread::Builder::new()
        .name("wr-smoke".into())
        .stack_size(RENDER_STACK)
        .spawn(run);
    match worker.map(|h| h.join()) {
        Ok(Ok(())) => std::process::exit(0),
        _ => {
            println!("{TAG} FAIL(99) the render thread didn't start or didn't finish");
            std::process::exit(99)
        }
    }
}

fn run() {
    println!("{TAG} start: webrender 0.70.0 on swgl 0.70.0, over neo-crt");

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
        // An Intel-driver workaround (clear picture caches by drawing quads
        // with depth func ALWAYS). SWGL rasterizes only LESS/LEQUAL depth
        // tests and asserts on anything else, and a software rasterizer has
        // no driver to work around, so it stays off on SWGL.
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
            check(1, "WebRender starts on SWGL", false);
            unreachable!()
        }
    };
    check(1, "WebRender starts on SWGL", true);

    let (api, _doc, glyph_ids) = build_scene(&sender, &notifier);
    println!("{TAG} glyph lookup for \"Hi\": {glyph_ids:?}");
    check(
        6,
        "the font backend maps 'H' and 'i' to the cmap's glyphs 63 and 255",
        glyph_ids == [Some(GLYPH_H), Some(GLYPH_I)],
    );
    renderer.update();
    let rendered = renderer.render(DeviceIntSize::new(W as i32, H as i32), 0);
    if let Err(e) = &rendered {
        println!("{TAG} render: {e:?}");
    }
    check(2, "a frame renders", rendered.is_ok());

    let (ptr, w, h, stride) = ctx.get_color_buffer(0, true);
    check(
        3,
        "the frame is in the caller's buffer, 64x48, stride 256",
        ptr == fb.as_mut_ptr().cast::<c_void>()
            && w == W as i32
            && h == H as i32
            && stride == STRIDE as i32,
    );

    let bad = first_mismatch(&fb, true);
    if let Some((x, y, got, want)) = bad {
        println!("{TAG} first wrong pixel at scene ({x},{y}): got {got:02x?} want {want:02x?}");
        let other = first_mismatch(&fb, false);
        println!(
            "{TAG} read without the GL flip: {}",
            match other {
                None => "every exact pixel matches (so the orientation is the only difference)"
                    .to_string(),
                Some((x, y, g, w)) =>
                    format!("also wrong, first at ({x},{y}) got {g:02x?} want {w:02x?}"),
            }
        );
    }
    check(
        4,
        "solid, clipped, transformed and image pixels == the hand-derived scene",
        bad.is_none(),
    );

    let mut gradient_ok = true;
    for y in 40..46 {
        let row: Vec<u8> = (12..44).map(|x| pixel(&fb, x, y, true)[2]).collect();
        let monotone = row.windows(2).all(|p| p[0] <= p[1]);
        let ends = row[0] <= 16 && row[row.len() - 1] >= 0xEF;
        let first_row: Vec<u8> = (12..44).map(|x| pixel(&fb, x, 40, true)[2]).collect();
        if !monotone || !ends || row != first_row {
            println!("{TAG} gradient row {y}: {row:02x?}");
            gradient_ok = false;
        }
    }
    check(
        5,
        "gradient: black to white, non-decreasing, same on every row",
        gradient_ok,
    );

    drop(api);
    renderer.deinit();
    // The text: dark pixels inside its box (the box is left out of the exact
    // check above, which found every pixel around it white).
    let (x0, y0, x1, y1) = TEXT_BOX;
    let dark = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&fb, x, y, true)[2] < 0x80)
        .count();
    println!("{TAG} text: {dark} dark pixels inside its box");
    check(
        7,
        "text: dark pixels inside the run's box, white all around it",
        dark >= 20,
    );

    println!("{TAG} PASS");
}
