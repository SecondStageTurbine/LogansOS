// SPDX-License-Identifier: MPL-2.0
//! `icon-raster write`: regenerate the committed blobs from assets/icons.
//! `icon-raster check`: fail if any committed blob differs (and print each
//! blob's BLAKE3, the values the tests pin).

use icon_raster::{NAMES, SIZES, blake3_hex, blob_path, rasterize, svg_path};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode != "write" && mode != "check" {
        eprintln!("usage: icon-raster write|check");
        std::process::exit(2);
    }
    let mut stale = 0;
    for name in NAMES {
        for size in SIZES {
            let svg = std::fs::read(svg_path(name, size)).expect("read svg");
            let blob = rasterize(&svg, size).expect("rasterise");
            let path = blob_path(name, size);
            if mode == "write" {
                std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
                std::fs::write(&path, &blob).expect("write blob");
            } else if std::fs::read(&path).ok().as_deref() != Some(&blob[..]) {
                eprintln!("stale: {}", path.display());
                stale += 1;
            }
            println!("{name}-{size} {}", blake3_hex(&blob));
        }
    }
    if stale != 0 {
        std::process::exit(1);
    }
}
