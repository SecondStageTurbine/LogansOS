// SPDX-License-Identifier: MPL-2.0
//! A9-D: pre-render the busy emblem's frames.
//!
//! The inputs are two straight-alpha RGBA8888 layers, the still star and the
//! ring arcs, written by `tools/branding/generate.py` at twice the frame size
//! on a square centred on the ring. Frame `k` is the ring turned clockwise by
//! `k * 360 / FRAMES` degrees about that centre, under the star. Each output
//! pixel averages 4x4 bilinear samples, taken in premultiplied alpha; the
//! result is straight-alpha `0xAARRGGBB` (`atrium_wm::Blend::Alpha`). `libm`
//! keeps the trigonometry, and so the frames, identical on every host.

use std::fmt::Write as _;
use std::path::Path;

const FRAMES: usize = 36;
const SIDE: usize = 44;
const LAYER: usize = 88;
const SUB: usize = 4;

fn layer(path: &Path) -> Vec<[f64; 4]> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(bytes.len(), LAYER * LAYER * 4, "{}", path.display());
    bytes
        .chunks_exact(4)
        .map(|p| {
            let a = f64::from(p[3]) / 255.0;
            [
                f64::from(p[0]) / 255.0 * a,
                f64::from(p[1]) / 255.0 * a,
                f64::from(p[2]) / 255.0 * a,
                a,
            ]
        })
        .collect()
}

/// Bilinear sample of a premultiplied layer at (x, y), pixel centres at
/// half-integers; transparent outside.
fn sample(layer: &[[f64; 4]], x: f64, y: f64) -> [f64; 4] {
    let fx = x - 0.5;
    let fy = y - 0.5;
    let x0 = libm::floor(fx);
    let y0 = libm::floor(fy);
    let (tx, ty) = (fx - x0, fy - y0);
    let mut out = [0.0; 4];
    for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
        for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
            let (px, py) = (x0 as i64 + dx, y0 as i64 + dy);
            if px < 0 || py < 0 || px >= LAYER as i64 || py >= LAYER as i64 {
                continue;
            }
            let p = layer[py as usize * LAYER + px as usize];
            for c in 0..4 {
                out[c] += p[c] * wx * wy;
            }
        }
    }
    out
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let star_path = dir.join("busy-emblem-star-88.rgba");
    let ring_path = dir.join("busy-emblem-ring-88.rgba");
    println!("cargo:rerun-if-changed={}", star_path.display());
    println!("cargo:rerun-if-changed={}", ring_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    let star = layer(&star_path);
    let ring = layer(&ring_path);

    let scale = LAYER as f64 / SIDE as f64;
    let centre = LAYER as f64 / 2.0;
    let mut src = String::new();
    writeln!(src, "pub const FRAMES: usize = {FRAMES};").unwrap();
    writeln!(src, "pub const SIDE: usize = {SIDE};").unwrap();
    writeln!(
        src,
        "pub static PIXELS: [u32; {}] = [",
        FRAMES * SIDE * SIDE
    )
    .unwrap();
    for k in 0..FRAMES {
        // Screen y points down, so a positive angle turns clockwise; sample
        // the ring at the inverse rotation.
        let theta = 2.0 * core::f64::consts::PI * k as f64 / FRAMES as f64;
        let (s, c) = (libm::sin(theta), libm::cos(theta));
        for py in 0..SIDE {
            for px in 0..SIDE {
                let mut acc = [0.0; 4];
                for j in 0..SUB {
                    for i in 0..SUB {
                        let x = (px as f64 + (i as f64 + 0.5) / SUB as f64) * scale;
                        let y = (py as f64 + (j as f64 + 0.5) / SUB as f64) * scale;
                        let (dx, dy) = (x - centre, y - centre);
                        let rx = centre + c * dx + s * dy;
                        let ry = centre - s * dx + c * dy;
                        let r = sample(&ring, rx, ry);
                        let t = sample(&star, x, y);
                        for ch in 0..4 {
                            acc[ch] += t[ch] + r[ch] * (1.0 - t[3]);
                        }
                    }
                }
                let n = (SUB * SUB) as f64;
                let a = acc[3] / n;
                let a8 = libm::round(a * 255.0).clamp(0.0, 255.0) as u32;
                let chan = |v: f64| {
                    if a8 == 0 {
                        0
                    } else {
                        libm::round(v / n / a * 255.0).clamp(0.0, 255.0) as u32
                    }
                };
                let pixel = a8 << 24 | chan(acc[0]) << 16 | chan(acc[1]) << 8 | chan(acc[2]);
                write!(src, "{pixel:#010x},").unwrap();
            }
            src.push('\n');
        }
    }
    src.push_str("];\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("busy_emblem_frames.rs");
    std::fs::write(out, src).unwrap();
}
