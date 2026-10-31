// SPDX-License-Identifier: MPL-2.0
//! A9-D: the WM layers the busy emblem over Arthur's client as one more
//! straight-alpha layer: `neo_atrium::busy::place` centres it in the rect
//! Arthur reserved and clips it to the client, and `compose_layers_xrgb8888`
//! blends the shown frame there. Checked against an independent per-pixel
//! source-over, in floating point.

use super::atrium_wm::{Blend, Layer, Rect, compose_layers_xrgb8888};
use neo_atrium::busy::{FRAMES, SIDE, frame, place};

const W: usize = 160;
const H: usize = 120;
const BASE: u32 = 0x0010_2030;
const CLIENT: Rect = Rect::new(20, 10, 100, 90);
const CLIENT_PX: u32 = 0x001A_2130;

/// Source-over, straight alpha, rounded to nearest, written independently of
/// `atrium_wm::blend_over`.
fn over(dst: u32, src: u32) -> u32 {
    let a = f64::from(src >> 24) / 255.0;
    let mut out = 0;
    for shift in [0, 8, 16] {
        let s = f64::from((src >> shift) & 0xFF);
        let d = f64::from((dst >> shift) & 0xFF);
        out |= ((s * a + d * (1.0 - a)).round() as u32) << shift;
    }
    out
}

/// Compose base + client + the emblem reserved at `area` (client
/// coordinates), showing frame `k`, over the whole screen.
fn compose(area: Rect, k: usize) -> (Vec<u32>, Option<Rect>) {
    let base = vec![BASE; W * H];
    let client = vec![CLIENT_PX; CLIENT.w * CLIENT.h];
    let mut layers = vec![
        Layer {
            pixels: &base,
            width: W,
            height: H,
            stride: W,
            x: 0,
            y: 0,
            blend: Blend::Opaque,
        },
        Layer {
            pixels: &client,
            width: CLIENT.w,
            height: CLIENT.h,
            stride: CLIENT.w,
            x: CLIENT.x,
            y: CLIENT.y,
            blend: Blend::Opaque,
        },
    ];
    let screen_area = neo_atrium::Rect::new(CLIENT.x + area.x, CLIENT.y + area.y, area.w, area.h);
    let clip = neo_atrium::Rect::new(CLIENT.x, CLIENT.y, CLIENT.w, CLIENT.h);
    let placed = place(screen_area, clip);
    if let Some((d, offset)) = placed {
        layers.push(Layer {
            pixels: &frame(k)[offset..],
            width: d.w,
            height: d.h,
            stride: SIDE,
            x: d.x,
            y: d.y,
            blend: Blend::Alpha,
        });
    }
    let mut out = vec![0u32; W * H];
    let written =
        compose_layers_xrgb8888(&mut out, W, H, W, &layers, Rect::new(0, 0, W, H)).unwrap();
    assert_eq!(written, Rect::new(0, 0, W, H));
    (out, placed.map(|(d, _)| Rect::new(d.x, d.y, d.w, d.h)))
}

/// Inside the reserved rect every pixel is the frame over the client; the
/// rest of the screen is untouched; every frame lands the same way.
#[test]
fn emblem_blends_over_the_client_in_its_rect_only() {
    let area = Rect::new(30, 40, SIDE + 6, SIDE + 2);
    for k in [0, 1, FRAMES / 2, FRAMES - 1] {
        let (out, dest) = compose(area, k);
        let dest = dest.unwrap();
        assert_eq!(dest, Rect::new(CLIENT.x + 33, CLIENT.y + 41, SIDE, SIDE));
        let f = frame(k);
        for y in 0..H {
            for x in 0..W {
                let got = out[y * W + x];
                let want = if dest.contains(x, y) {
                    over(CLIENT_PX, f[(y - dest.y) * SIDE + x - dest.x])
                } else if CLIENT.contains(x, y) {
                    CLIENT_PX
                } else {
                    BASE
                };
                assert_eq!(got, want, "frame {k} at ({x},{y})");
            }
        }
    }
}

/// A rect at the client's bottom-right corner, running past it: the emblem
/// is clipped to the client and never reaches the base around it.
#[test]
fn emblem_is_clipped_to_the_client() {
    let area = Rect::new(CLIENT.w - SIDE / 2, CLIENT.h - SIDE / 2, SIDE, SIDE);
    let (out, dest) = compose(area, 3);
    let dest = dest.unwrap();
    assert_eq!(
        dest,
        Rect::new(
            CLIENT.right() - SIDE / 2,
            CLIENT.bottom() - SIDE / 2,
            SIDE / 2,
            SIDE / 2
        )
    );
    let f = frame(3);
    for y in 0..H {
        for x in 0..W {
            let got = out[y * W + x];
            if !CLIENT.contains(x, y) {
                assert_eq!(got, BASE, "({x},{y})");
            } else if dest.contains(x, y) {
                // The frame's top-left quarter: the clip took its right and
                // bottom halves.
                let src = f[(y - dest.y) * SIDE + x - dest.x];
                assert_eq!(got, over(CLIENT_PX, src), "({x},{y})");
            }
        }
    }
}
