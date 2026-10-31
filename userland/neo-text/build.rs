// SPDX-License-Identifier: MPL-2.0
//! Generates the sRGB <-> linear lookup tables used for gamma-correct glyph
//! blending (IEC 61966-2-1 transfer function, computed in f64 on the host).
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

/// Linear light is stored as 12-bit integers (0..=4095).
const LINEAR_MAX: f64 = 4095.0;

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(l: f64) -> f64 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

fn main() {
    let mut out = String::new();
    out.push_str("/// sRGB 8-bit channel -> 12-bit linear light.\n");
    out.push_str("pub static SRGB_TO_LINEAR: [u16; 256] = [");
    for c in 0..256 {
        let v = (to_linear(c as f64 / 255.0) * LINEAR_MAX).round() as u16;
        write!(out, "{v},").unwrap();
    }
    out.push_str("];\n/// 12-bit linear light -> sRGB 8-bit channel.\n");
    out.push_str("pub static LINEAR_TO_SRGB: [u8; 4096] = [");
    for l in 0..4096 {
        let v = (to_srgb(l as f64 / LINEAR_MAX) * 255.0).round() as u8;
        write!(out, "{v},").unwrap();
    }
    out.push_str("];\n");
    let path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("gamma_tables.rs");
    fs::write(path, out).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
