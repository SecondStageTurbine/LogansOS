// SPDX-License-Identifier: MPL-2.0
//! neo-text: the Desktop S2 / text-engine T0 library.
//!
//! TrueType parsing (`ttf-parser`) -> exact-area anti-aliased rasterization
//! ([`raster`]) -> per-face glyph cache ([`font`]) -> kerned line layout and
//! word wrap ([`layout`]) -> gamma-correct blending into XRGB8888 buffers
//! ([`canvas`]) -> a streaming, damage-tracking text region ([`flow`]).
//!
//! Pure `no_std` + `alloc`; no syscalls, so every stage is host-testable.
//! Design authority: docs/THINK-2026-07-10-atrium-text-engine-research.md
//! (stage T0) and docs/THINK-2026-07-10-atrium-design-language.md §6.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod canvas;
pub mod flow;
pub mod font;
pub mod fonts;
pub mod layout;
pub mod raster;

#[cfg(test)]
mod tests;

pub use canvas::{Canvas, Rect, blend_pixel};
pub use flow::{FlowStyle, TextFlow};
pub use font::{Font, GlyphBitmap, LineMetrics};
