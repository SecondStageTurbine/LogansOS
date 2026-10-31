// SPDX-License-Identifier: MPL-2.0
//! The Spine chips' app icons (`app-icons`; neo-init's `atrium-app-icons`).
//!
//! The 40 px masters (`assets/icons/<name>.svg`) are rasterised on the host
//! by `tools/icon-raster` (`cargo xtask icons`) to straight-alpha RGBA8888
//! blobs, committed under `assets/icons/` beside this crate and pinned there
//! by BLAKE3. Here they are only blended: the OS has no SVG renderer.
//!
//! **Placement.** The icon replaces the chip's letter. Its 40×40 box sits at
//! `(chip.x + 8, chip.y + 6)`: centred across the 56 px chip and 2 px above
//! centre, so the chip's name fits below it (its baseline moves from
//! `chip.y + 47` to `chip.y + 53`). The box's top 2 rows lie above the chip's
//! label box and are clipped away; every master keeps its 2 px margin fully
//! transparent (icon-raster's tests), so no ink is lost.
//!
//! **Blend.** Per channel, in sRGB as the rasteriser's own coverage was
//! computed: `out = (s·a + d·(255 − a) + 127) / 255`, where `s` and `a` are the
//! blob's straight colour and alpha and `d` is the chip pixel below. The
//! DS-1 scene oracle (`scripts/desktop/scene_oracle.py icon-fixture`) applies
//! the same formula over its own chip pixels for the scenario's readback.

use crate::layout::Rect;

/// The icon box's side.
pub const SIZE: usize = 40;
/// The chip name's baseline below the chip's top edge, with an icon.
pub const NAME_BASELINE: usize = 53;

pub static ARTHUR: &[u8; SIZE * SIZE * 4] = include_bytes!("../assets/icons/arthur-40.rgba");
pub static LENS: &[u8; SIZE * SIZE * 4] = include_bytes!("../assets/icons/lens-40.rgba");
/// OT-87 R4.4b: the Install chip (#185's blob).
#[cfg(feature = "install-chip")]
pub static INSTALL: &[u8; SIZE * SIZE * 4] = include_bytes!("../assets/icons/install-40.rgba");

/// The icon's 40×40 box on `chip`.
pub const fn rect(chip: Rect) -> Rect {
    Rect::new(chip.x + 8, chip.y + 6, SIZE, SIZE)
}

/// One channel of the blend.
pub const fn blend_channel(d: u32, s: u32, a: u32) -> u32 {
    (s * a + d * (255 - a) + 127) / 255
}

/// The straight-alpha pixel `rgba` over the XRGB8888 pixel `d`.
pub fn blend_pixel(d: u32, rgba: [u8; 4]) -> u32 {
    let a = rgba[3] as u32;
    let ch = |shift: u32, s: u8| blend_channel((d >> shift) & 0xFF, s as u32, a) << shift;
    ch(16, rgba[0]) | ch(8, rgba[1]) | ch(0, rgba[2])
}

/// Blend `blob` into `dst` (XRGB8888, `stride` pixels per row) at `chip`'s
/// icon box, touching only pixels inside `clip`.
pub fn paint(dst: &mut [u32], stride: usize, chip: Rect, clip: Rect, blob: &[u8; SIZE * SIZE * 4]) {
    let r = rect(chip).intersect(&clip);
    let at = rect(chip);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let i = ((y - at.y) * SIZE + (x - at.x)) * 4;
            let px = [blob[i], blob[i + 1], blob[i + 2], blob[i + 3]];
            if px[3] != 0 {
                let d = &mut dst[y * stride + x];
                *d = blend_pixel(*d, px);
            }
        }
    }
}
