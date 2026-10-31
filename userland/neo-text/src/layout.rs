// SPDX-License-Identifier: MPL-2.0
//! Single-line layout (advances + GPOS kerning), greedy word wrap, and drawing.
//!
//! Latin only: one char -> one glyph via `cmap`, no shaping (rustybuzz is the
//! later T2 stage). Pen positions are kept in f32 and each glyph origin is
//! quantised to 1/[`SUBPIXEL_BINS`] px when drawn.

use alloc::vec::Vec;
use core::ops::Range;

use crate::canvas::{Canvas, Rect};
use crate::font::{Font, SUBPIXEL_BINS};
use crate::raster::floor;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub glyph: u16,
    /// Pen x of this glyph's origin, relative to the line start.
    pub pen: f32,
}

/// Split a pen position into an integer pixel and a subpixel bin, rounding to
/// the nearest 1/SUBPIXEL_BINS px.
pub fn quantize(pen: f32) -> (i32, u8) {
    let bins = i32::from(SUBPIXEL_BINS);
    let q = floor(pen * bins as f32 + 0.5) as i32;
    (q.div_euclid(bins), q.rem_euclid(bins) as u8)
}

/// Lay out `text` on one line. Returns the glyphs and the final pen (the line's
/// advance width).
pub fn layout_line(font: &Font<'_>, px: f32, text: &str) -> (Vec<PlacedGlyph>, f32) {
    let mut glyphs = Vec::with_capacity(text.len());
    let mut pen = 0.0f32;
    let mut prev: Option<u16> = None;
    for c in text.chars() {
        let glyph = font.glyph_id(c);
        if let Some(prev) = prev {
            pen += font.kerning(prev, glyph, px);
        }
        glyphs.push(PlacedGlyph { glyph, pen });
        pen += font.advance(glyph, px);
        prev = Some(glyph);
    }
    (glyphs, pen)
}

pub fn measure(font: &Font<'_>, px: f32, text: &str) -> f32 {
    layout_line(font, px, text).1
}

/// Greedy word wrap into byte ranges of `text`, one per visual line.
///
/// `\n` forces a break. Lines break at the last space that keeps the ink within
/// `max_width`; the breaking space is dropped and trailing spaces may overhang.
/// A word wider than the line is broken at the character that overflows, and
/// every line holds at least one character, so wrapping always terminates.
pub fn wrap(font: &Font<'_>, px: f32, text: &str, max_width: f32) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut para_start = 0usize;
    for para in text.split('\n') {
        let para_end = para_start + para.len();
        wrap_paragraph(font, px, text, para_start..para_end, max_width, &mut lines);
        para_start = para_end + 1;
    }
    lines
}

fn wrap_paragraph(
    font: &Font<'_>,
    px: f32,
    text: &str,
    para: Range<usize>,
    max_width: f32,
    lines: &mut Vec<Range<usize>>,
) {
    if para.is_empty() {
        lines.push(para);
        return;
    }
    let mut start = para.start;
    while start < para.end {
        let (end, next) = line_break(font, px, text, start..para.end, max_width);
        lines.push(start..end);
        start = next;
    }
}

/// Where the line beginning at `span.start` ends: `(end, next_start)`.
pub fn line_break(
    font: &Font<'_>,
    px: f32,
    text: &str,
    span: Range<usize>,
    max_width: f32,
) -> (usize, usize) {
    let mut pen = 0.0f32;
    let mut prev: Option<u16> = None;
    let mut last_space: Option<usize> = None;
    for (offset, c) in text[span.clone()].char_indices() {
        let index = span.start + offset;
        let glyph = font.glyph_id(c);
        if let Some(prev) = prev {
            pen += font.kerning(prev, glyph, px);
        }
        let right = pen + font.advance(glyph, px);
        if c == ' ' {
            last_space = Some(index);
        } else if right > max_width && index > span.start {
            return match last_space {
                Some(space) => (space, space + 1),
                None => (index, index),
            };
        }
        pen = right;
        prev = Some(glyph);
    }
    (span.end, span.end)
}

/// Draw one line of `text` with its pen origin at (`x`, `baseline`), clipped to
/// `clip`. Returns the bounding box of the pixels it may have touched.
#[allow(clippy::too_many_arguments)]
pub fn draw_line(
    canvas: &mut Canvas<'_>,
    font: &mut Font<'_>,
    px: f32,
    text: &str,
    x: f32,
    baseline: i32,
    color: u32,
    clip: Rect,
) -> Rect {
    let (glyphs, _) = layout_line(font, px, text);
    let mut touched = Rect::new(0, 0, 0, 0);
    for placed in glyphs {
        let (ix, bin) = quantize(x + placed.pen);
        if let Some(bitmap) = font.glyph(placed.glyph, px, bin) {
            canvas.draw_glyph(bitmap, ix, baseline, color, clip);
            let ink = Rect::new(
                ix + bitmap.left,
                baseline + bitmap.top,
                bitmap.width as i32,
                bitmap.height as i32,
            );
            touched = touched.union(&ink.intersect(&clip));
        }
    }
    touched
}
