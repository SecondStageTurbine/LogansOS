// SPDX-License-Identifier: MPL-2.0
//! The app icons' build-time rasteriser (host only). The OS has no SVG
//! renderer, so each `assets/icons/<name>[-<size>].svg` is rasterised here
//! to a raw blob: `size * size * 4` bytes, row-major, RGBA8888, **straight**
//! (not premultiplied) alpha, no header. neo-atrium embeds the 40 px ones
//! (`atrium-app-icons`) and blends them `out = (s·a + d·(255 − a) + 127)/255`.
//!
//! The blobs are committed (`userland/neo-atrium/assets/icons/`), like the
//! busy emblem's layers; `cargo xtask icons` (or `cargo run -p icon-raster
//! -- write`) regenerates them, and this crate's tests fail if a committed
//! blob is not what the current rasteriser makes from the current SVG, or
//! if its BLAKE3 is not the pinned one.

use std::path::{Path, PathBuf};

/// The final set (J.c., 2026-10-29), in Spine order.
pub const NAMES: [&str; 7] = [
    "arthur", "lens", "terminal", "files", "settings", "install", "browser",
];
/// The rasterised sizes: the 40 px master and the hand-hinted 24 and 16.
pub const SIZES: [u32; 3] = [40, 24, 16];

/// The repository root (this crate is `tools/icon-raster`).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `assets/icons/<name>.svg` at 40, else `<name>-<size>.svg`.
pub fn svg_path(name: &str, size: u32) -> PathBuf {
    let file = if size == 40 {
        format!("{name}.svg")
    } else {
        format!("{name}-{size}.svg")
    };
    repo_root().join("assets/icons").join(file)
}

/// `userland/neo-atrium/assets/icons/<name>-<size>.rgba`.
pub fn blob_path(name: &str, size: u32) -> PathBuf {
    repo_root()
        .join("userland/neo-atrium/assets/icons")
        .join(format!("{name}-{size}.rgba"))
}

/// Rasterise `svg` into a `size` x `size` straight-alpha RGBA8888 blob. The
/// SVG's own viewBox is mapped onto the square (every icon's viewBox is
/// `0 0 size size`, so this is 1:1).
pub fn rasterize(svg: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default())
        .map_err(|e| format!("svg: {e}"))?;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size, size).ok_or_else(|| "empty size".to_string())?;
    let sx = size as f32 / tree.size().width();
    let sy = size as f32 / tree.size().height();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied RGBA; the blob is straight alpha.
    Ok(pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect())
}

/// Lowercase hex of the blob's BLAKE3.
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes)
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests;
