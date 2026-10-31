// SPDX-License-Identifier: MPL-2.0
//! Preprocessed boot splash asset.
//!
//! The kernel does not decode PNG during early boot. The canonical crest lives
//! in `assets/branding/LogansOS.png`; this module embeds a 160x160 RGBA8888
//! derivative generated for the framebuffer splash by `tools/branding/generate.py`.
//! The background is kept opaque black on purpose — the framebuffer is cleared
//! to black before the blit, so black-on-black is seamless.

pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 160;

pub static RGBA: &[u8] = include_bytes!("../../assets/logansos_boot_logo_160.rgba");

// Byte oracle: a wrong-sized derivative must fail to compile, not silently
// render garbage. 160 * 160 * 4 (RGBA8888) == 102400 bytes.
const _: () = assert!(WIDTH * HEIGHT * 4 == RGBA.len());
