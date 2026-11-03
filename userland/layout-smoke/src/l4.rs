// SPDX-License-Identifier: MPL-2.0
//! OT-61 L4: a line of text, laid out by Servo's layout with the bundled
//! Roboto Slab (L1) and drawn by WebRender on SWGL (L3's path), its fonts
//! passed from layout to WebRender by the paint host.
//!
//! A glyph's exact pixels depend on the rasterizer's anti-aliasing, so the
//! frame isn't compared pixel by pixel. What is hand-derived is where ink may
//! and must be, from the font's `hmtx` advances (`GLYPHS`, read from the
//! font by `oracle/font_tables.py`) and the page's CSS:
//!
//! - `#t` is a block at x 8, y 8 (body's 8 px margin), 40 px tall, with
//!   `line-height: 40px`; the text starts at its left edge. So every glyph
//!   is inside y 8..48.
//! - Roboto Slab has 2048 units per em, so at 16 px a glyph advances
//!   `units * 16 / 2048` px. The run "Hello, LogansOS 0" is the sum of its
//!   characters' advances wide.
//! - Each non-space character's advance cell (its pen position to the next
//!   one, widened by 1 px either side for kerning and rounding) holds ink.
//!
//! The checks: one reflow (70); some ink exists (71); no ink lies outside
//! the run's box (72); every non-space character's cell holds ink (73); and
//! the text is grey (black text on white, anti-aliased in greyscale:
//! B = G = R) (74).

use crate::l3::{self, H, RenderError, W, WHITE};
use crate::{GLYPHS, TAG, check};

/// The page: one block of text, in the bundled family at 16 px.
pub const PAGE: &str = "<!DOCTYPE html><html><head><style>\
html, body, div { display: block; margin: 0; padding: 0; border: 0 }\
body { margin: 8px }\
#t { height: 40px; font-family: 'Roboto Slab'; font-size: 16px; line-height: 40px; color: black }\
</style></head><body><div id=t>Hello, LogansOS 0</div></body></html>";

const TEXT: &str = "Hello, LogansOS 0";
const UNITS_PER_EM: f32 = 2048.0;
const FONT_PX: f32 = 16.0;
/// `#t`'s content box: x 8.., y 8..48.
const LEFT: f32 = 8.0;
const TOP: usize = 8;
const BOTTOM: usize = 48;

fn advance_px(c: char) -> f32 {
    let (_, _, units) = GLYPHS
        .iter()
        .find(|g| g.0 == c)
        .expect("every character of TEXT is in GLYPHS");
    *units as f32 * FONT_PX / UNITS_PER_EM
}

/// Each character, with its advance cell's start and end x (CSS px).
fn cells() -> Vec<(char, f32, f32)> {
    let mut x = LEFT;
    TEXT.chars()
        .map(|c| {
            let a = advance_px(c);
            let cell = (c, x, x + a);
            x += a;
            cell
        })
        .collect()
}

fn inked(p: [u8; 4]) -> bool {
    p != WHITE
}

/// Render the text page and check where its ink is (checks 71-74; the caller checks 70).
pub fn render_and_check(page: &neo_static_page::StaticPage) {
    let frame = l3::render_frame(page);
    match &frame {
        Err(RenderError::NoDisplayList) => println!("{TAG} L4: layout sent no display list"),
        Err(RenderError::WebRender(e)) => println!("{TAG} L4: {e}"),
        Err(RenderError::NotOurBuffer) => println!("{TAG} L4: the frame isn't in our buffer"),
        Ok(_) => {}
    }
    let Ok(fb) = frame else {
        check(71, "the text page renders, with ink", false);
        unreachable!()
    };

    let cells = cells();
    let right = cells.last().map_or(LEFT, |c| c.2);
    // The run's box, widened by 1 px either side (kerning, rounding).
    let (x0, x1) = ((LEFT - 1.0).floor() as usize, (right + 1.0).ceil() as usize);

    let mut ink = 0usize;
    let mut outside = None;
    let mut not_grey = None;
    for y in 0..H {
        for x in 0..W {
            let p = l3::pixel(&fb, x, y, true);
            if !inked(p) {
                continue;
            }
            ink += 1;
            if !(x >= x0 && x < x1 && y >= TOP && y < BOTTOM) {
                outside.get_or_insert((x, y, p));
            }
            if !(p[0] == p[1] && p[1] == p[2]) {
                not_grey.get_or_insert((x, y, p));
            }
        }
    }
    println!("{TAG} L4: {ink} inked pixels; run box x {x0}..{x1}, y {TOP}..{BOTTOM}");
    check(71, "the text page renders, with ink", ink > 0);
    if let Some((x, y, p)) = outside {
        println!("{TAG} L4: first ink outside the run's box at ({x},{y}): {p:02x?}");
    }
    check(
        72,
        "no ink outside the run's hand-derived box",
        outside.is_none(),
    );

    let mut empty = Vec::new();
    for &(c, a, b) in cells.iter().filter(|c| c.0 != ' ') {
        let (a, b) = ((a - 1.0).floor() as usize, (b + 1.0).ceil() as usize);
        let has = (TOP..BOTTOM).any(|y| (a..b).any(|x| inked(l3::pixel(&fb, x, y, true))));
        if !has {
            empty.push((c, a, b));
        }
    }
    if !empty.is_empty() {
        println!("{TAG} L4: cells with no ink: {empty:?}");
    }
    check(
        73,
        "every character of \"Hello, LogansOS 0\" has ink in its advance cell",
        empty.is_empty(),
    );
    if let Some((x, y, p)) = not_grey {
        println!("{TAG} L4: first non-grey ink at ({x},{y}): {p:02x?}");
    }
    check(74, "the text's ink is grey (B = G = R)", not_grey.is_none());
}
