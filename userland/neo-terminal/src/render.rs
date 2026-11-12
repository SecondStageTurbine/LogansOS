// SPDX-License-Identifier: MPL-2.0
//! The screen's cells drawn into the window's pixels.
//!
//! A fixed cell grid in IBM Plex Mono (the desktop's monospace face, already
//! bundled with neo-text, SIL OFL-1.1): the cell is the face's advance wide
//! and its line height tall. Each row keeps a hash of what it last drew (its
//! cells, their colours and attributes, and the cursor if it is on that row),
//! so a frame redraws only the rows that changed and the WM is sent only
//! their band as damage.

use alloc::vec::Vec;
use neo_text::fonts::PLEX_MONO_REGULAR;
use neo_text::{Canvas, Font, Rect};
use vt100::Color;

/// The face size.
pub const FONT_PX: f32 = 14.0;
/// The window's floor and the default ink (neo-atrium spec v1.4's content
/// floor and secondary label colour).
pub const FLOOR: u32 = 0x000A_0E27;
pub const INK: u32 = 0x00C8_CDD2;
/// The margin around the grid.
pub const PAD: usize = 4;

/// The grid's place in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub cell_w: usize,
    pub cell_h: usize,
    /// The baseline's offset from a cell's top.
    pub ascent: i32,
    pub cols: u16,
    pub rows: u16,
}

impl Geometry {
    /// The pixel rectangle of a band of rows.
    pub fn band(&self, first: u16, last: u16) -> (u16, u16) {
        let y = PAD + usize::from(first) * self.cell_h;
        let h = usize::from(last - first + 1) * self.cell_h;
        (y as u16, h as u16)
    }
}

/// The grid that fits `width` × `height` with the face at [`FONT_PX`].
pub fn geometry(font: &Font<'_>, width: usize, height: usize) -> Option<Geometry> {
    let advance = font.advance(font.glyph_id('M'), FONT_PX);
    let m = font.line_metrics(FONT_PX);
    let cell_w = libm_ceil(advance) as usize;
    let cell_h = libm_ceil(m.ascent + m.descent + m.line_gap) as usize;
    if cell_w == 0 || cell_h == 0 {
        return None;
    }
    let cols = width.checked_sub(2 * PAD)? / cell_w;
    let rows = height.checked_sub(2 * PAD)? / cell_h;
    if cols == 0 || rows == 0 {
        return None;
    }
    Some(Geometry {
        cell_w,
        cell_h,
        ascent: libm_ceil(m.ascent) as i32,
        cols: cols.min(usize::from(u16::MAX)) as u16,
        rows: rows.min(usize::from(u16::MAX)) as u16,
    })
}

fn libm_ceil(v: f32) -> f32 {
    let t = v as i32 as f32;
    if t < v { t + 1.0 } else { t }
}

pub struct Renderer {
    font: Font<'static>,
    geo: Geometry,
    width: usize,
    height: usize,
    /// Pixels from one row to the next in the buffer (`>= width`): a WM with
    /// `desktop-resize` imports a buffer sized for the largest client, and
    /// the window is its top-left `width` x `height`.
    stride: usize,
    /// What each row drew last (`None`: never drawn).
    drawn: Vec<Option<u64>>,
}

impl Renderer {
    pub fn new(width: usize, height: usize) -> Option<Self> {
        Self::with_stride(width, height, width)
    }

    /// A window of `width` x `height` in a buffer whose rows are `stride`
    /// pixels apart (`None` when `stride < width`).
    pub fn with_stride(width: usize, height: usize, stride: usize) -> Option<Self> {
        if stride < width {
            return None;
        }
        let font = Font::from_bytes(PLEX_MONO_REGULAR)?;
        let geo = geometry(&font, width, height)?;
        Some(Self {
            font,
            geo,
            width,
            height,
            stride,
            drawn: alloc::vec![None; usize::from(geo.rows)],
        })
    }

    pub fn geometry(&self) -> Geometry {
        self.geo
    }

    /// Paint the whole window: the floor, then every row.
    pub fn draw_all(&mut self, screen: &vt100::Screen, pixels: &mut [u32]) -> Option<(u16, u16)> {
        if self.height == 0 || pixels.len() < self.stride * (self.height - 1) + self.width {
            return None;
        }
        for y in 0..self.height {
            let row = y * self.stride;
            pixels[row..row + self.width].fill(FLOOR);
        }
        self.drawn.iter_mut().for_each(|d| *d = None);
        self.draw(screen, pixels)?;
        Some((0, self.height as u16))
    }

    /// Redraw the rows that changed. Returns the damaged band `(y, height)`
    /// in pixels, or `None` when nothing changed.
    pub fn draw(&mut self, screen: &vt100::Screen, pixels: &mut [u32]) -> Option<(u16, u16)> {
        self.draw_with(screen, pixels, || {})
    }

    /// [`Renderer::draw`], calling `between` after each row it redraws (the
    /// app yields there, so a long redraw is many short bursts).
    pub fn draw_with(
        &mut self,
        screen: &vt100::Screen,
        pixels: &mut [u32],
        mut between: impl FnMut(),
    ) -> Option<(u16, u16)> {
        let mut canvas = Canvas::new(pixels, self.width, self.height, self.stride)?;
        let (rows, cols) = screen.size();
        let rows = rows.get().min(self.geo.rows);
        let cols = cols.get().min(self.geo.cols);
        let (cursor_row, cursor_col) = screen.cursor_position();
        let cursor_shown = !screen.hide_cursor();
        let mut band: Option<(u16, u16)> = None;
        for row in 0..rows {
            let cursor = (cursor_shown && cursor_row == row).then_some(cursor_col.min(cols - 1));
            let hash = row_hash(screen, row, cols, cursor);
            if self.drawn[usize::from(row)] == Some(hash) {
                continue;
            }
            self.drawn[usize::from(row)] = Some(hash);
            for col in 0..cols {
                draw_cell(
                    &mut canvas,
                    &mut self.font,
                    &self.geo,
                    screen,
                    row,
                    col,
                    cursor == Some(col),
                );
            }
            band = Some(match band {
                None => (row, row),
                Some((first, _)) => (first, row),
            });
            between();
        }
        band.map(|(first, last)| self.geo.band(first, last))
    }
}

fn draw_cell(
    canvas: &mut Canvas<'_>,
    font: &mut Font<'_>,
    geo: &Geometry,
    screen: &vt100::Screen,
    row: u16,
    col: u16,
    cursor: bool,
) {
    let x = (PAD + usize::from(col) * geo.cell_w) as i32;
    let y = (PAD + usize::from(row) * geo.cell_h) as i32;
    let rect = Rect::new(x, y, geo.cell_w as i32, geo.cell_h as i32);
    let Some(cell) = screen.cell(row, col) else {
        canvas.fill(rect, FLOOR);
        return;
    };
    let mut fg = colour(cell.fgcolor(), INK, cell.bold());
    let mut bg = colour(cell.bgcolor(), FLOOR, false);
    if cell.inverse() != cursor {
        core::mem::swap(&mut fg, &mut bg);
    }
    canvas.fill(rect, bg);
    if cell.is_wide_continuation() {
        return;
    }
    let baseline = y + geo.ascent;
    // A wide character's glyph may run into the next cell; the clip is the
    // two cells.
    let clip_w = if cell.is_wide() {
        2 * geo.cell_w
    } else {
        geo.cell_w
    } as i32;
    let clip = Rect::new(x, y, clip_w, geo.cell_h as i32);
    for ch in cell.contents().chars() {
        let id = font.glyph_id(ch);
        if let Some(glyph) = font.glyph(id, FONT_PX, 0) {
            canvas.draw_glyph(glyph, x, baseline, fg, clip);
        }
    }
    if cell.underline() {
        canvas.fill(Rect::new(x, baseline + 2, geo.cell_w as i32, 1), fg);
    }
}

/// A cell colour as XRGB8888: the default, xterm's 16 colours (bold
/// brightens the first eight), the 6×6×6 cube, the grey ramp, or RGB.
pub fn colour(c: Color, default: u32, bold: bool) -> u32 {
    const BASE: [u32; 16] = [
        0x000000, 0xCD0000, 0x00CD00, 0xCDCD00, 0x0000EE, 0xCD00CD, 0x00CDCD, 0xE5E5E5, 0x7F7F7F,
        0xFF0000, 0x00FF00, 0xFFFF00, 0x5C5CFF, 0xFF00FF, 0x00FFFF, 0xFFFFFF,
    ];
    match c {
        Color::Default => default,
        Color::Idx(i) if i < 8 && bold => BASE[usize::from(i) + 8],
        Color::Idx(i) if i < 16 => BASE[usize::from(i)],
        Color::Idx(i) if i < 232 => {
            let i = u32::from(i - 16);
            let level = |v: u32| if v == 0 { 0 } else { 55 + 40 * v };
            level(i / 36) << 16 | level(i / 6 % 6) << 8 | level(i % 6)
        }
        Color::Idx(i) => {
            let v = 8 + 10 * u32::from(i - 232);
            v << 16 | v << 8 | v
        }
        Color::Rgb(r, g, b) => u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
    }
}

/// FNV-1a over what a row draws.
fn row_hash(screen: &vt100::Screen, row: u16, cols: u16, cursor: Option<u16>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            break;
        };
        eat(cell.contents().as_bytes());
        eat(&[0xff]);
        eat(&colour(cell.fgcolor(), INK, cell.bold()).to_le_bytes());
        eat(&colour(cell.bgcolor(), FLOOR, false).to_le_bytes());
        eat(&[u8::from(cell.inverse())
            | u8::from(cell.underline()) << 1
            | u8::from(cell.is_wide()) << 2
            | u8::from(cell.is_wide_continuation()) << 3]);
    }
    eat(&cursor.map_or(u16::MAX, |c| c).to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU16;
    use neo_term_core::term::Term;

    const W: usize = 640;
    const H: usize = 400;

    fn setup() -> (Renderer, Term, Vec<u32>) {
        let r = Renderer::new(W, H).expect("renderer");
        let g = r.geometry();
        let t = Term::new(
            NonZeroU16::new(g.rows).unwrap(),
            NonZeroU16::new(g.cols).unwrap(),
        );
        (r, t, alloc::vec![0u32; W * H])
    }

    fn inked(pixels: &[u32], g: &Geometry, row: u16, col: u16) -> usize {
        let x0 = PAD + usize::from(col) * g.cell_w;
        let y0 = PAD + usize::from(row) * g.cell_h;
        let mut n = 0;
        for y in y0..y0 + g.cell_h {
            for x in x0..x0 + g.cell_w {
                if pixels[y * W + x] != FLOOR {
                    n += 1;
                }
            }
        }
        n
    }

    /// The window's grid: Plex Mono at 14 px in 640 × 400 is at least
    /// 70 × 20 cells (neo-shell's 80-column lines wrap; THINK §3.2's size
    /// is what the child reads, not a fixed 80 × 25).
    #[test]
    fn geometry_fits_the_window() {
        let (r, _, _) = setup();
        let g = r.geometry();
        assert!(g.cols >= 70, "{g:?}");
        assert!(g.rows >= 20, "{g:?}");
        assert!(PAD * 2 + usize::from(g.cols) * g.cell_w <= W);
        assert!(PAD * 2 + usize::from(g.rows) * g.cell_h <= H);
    }

    /// A glyph inks its own cell only; the cursor cell is drawn inverse.
    #[test]
    fn cells_ink_where_the_characters_are() {
        let (mut r, mut t, mut px) = setup();
        let _ = t.process(b"\x1b[?25lA");
        let band = r.draw_all(t.screen(), &mut px).expect("drawn");
        assert_eq!(band, (0, H as u16));
        let g = r.geometry();
        assert!(inked(&px, &g, 0, 0) > 10);
        assert_eq!(inked(&px, &g, 0, 1), 0);
        assert_eq!(inked(&px, &g, 1, 0), 0);
        // Cursor shown: its (blank) cell becomes a solid block.
        let _ = t.process(b"\x1b[?25h");
        let band = r.draw(t.screen(), &mut px).expect("cursor row");
        assert_eq!(band, g.band(0, 0));
        assert_eq!(inked(&px, &g, 0, 1), g.cell_w * g.cell_h);
    }

    /// `draw_with` calls back once per redrawn row, and damages the same.
    #[test]
    fn draw_with_calls_back_per_row() {
        let (mut r, mut t, mut px) = setup();
        let _ = t.process(b"\x1b[?25l");
        r.draw_all(t.screen(), &mut px).expect("first");
        let _ = t.process(b"a\r\nb\r\nc");
        let mut calls = 0;
        let band = r.draw_with(t.screen(), &mut px, || calls += 1);
        assert_eq!(calls, 3);
        assert_eq!(band, Some(r.geometry().band(0, 2)));
    }

    /// Only the changed rows are redrawn and damaged.
    #[test]
    fn unchanged_rows_are_not_redrawn() {
        let (mut r, mut t, mut px) = setup();
        let _ = t.process(b"\x1b[?25lone\r\ntwo");
        r.draw_all(t.screen(), &mut px).expect("first");
        assert_eq!(r.draw(t.screen(), &mut px), None);
        let _ = t.process(b"\x1b[3;1Hthree");
        let g = r.geometry();
        assert_eq!(r.draw(t.screen(), &mut px), Some(g.band(2, 2)));
        let _ = t.process(b"\x1b[1;1Hx\x1b[3;1Hy");
        assert_eq!(r.draw(t.screen(), &mut px), Some(g.band(0, 2)));
    }

    /// In a wider buffer (a `desktop-resize` WM's, 1280 x 800) the window is
    /// its top-left 640 x 400: the same pixels as a tight buffer, and nothing
    /// written to the right of the window or below it.
    #[test]
    fn a_stride_draws_the_top_left_window_only() {
        const SW: usize = 1280;
        const SH: usize = 800;
        const UNTOUCHED: u32 = 0x00AB_CDEF;
        let (mut tight, mut t, mut tight_px) = setup();
        let mut wide = Renderer::with_stride(W, H, SW).expect("renderer");
        assert_eq!(wide.geometry(), tight.geometry());
        let mut wide_px = alloc::vec![UNTOUCHED; SW * SH];
        let _ = t.process(b"hello\r\nworld");
        assert_eq!(
            wide.draw_all(t.screen(), &mut wide_px),
            tight.draw_all(t.screen(), &mut tight_px)
        );
        let _ = t.process(b"\r\nagain");
        assert_eq!(
            wide.draw(t.screen(), &mut wide_px),
            tight.draw(t.screen(), &mut tight_px)
        );
        for y in 0..SH {
            for x in 0..SW {
                let got = wide_px[y * SW + x];
                if x < W && y < H {
                    assert_eq!(got, tight_px[y * W + x], "({x}, {y})");
                } else {
                    assert_eq!(got, UNTOUCHED, "({x}, {y})");
                }
            }
        }
        // A stride narrower than the window, or a buffer too short for it,
        // is refused.
        assert!(Renderer::with_stride(W, H, W - 1).is_none());
        assert_eq!(
            wide.draw_all(t.screen(), &mut wide_px[..SW * (H - 1)]),
            None
        );
    }

    #[test]
    fn colours_follow_xterm() {
        assert_eq!(colour(Color::Default, INK, false), INK);
        assert_eq!(colour(Color::Idx(1), INK, false), 0xCD0000);
        assert_eq!(colour(Color::Idx(1), INK, true), 0xFF0000);
        assert_eq!(colour(Color::Idx(16), INK, false), 0x000000);
        assert_eq!(colour(Color::Idx(231), INK, false), 0xFFFFFF);
        assert_eq!(colour(Color::Idx(240), INK, false), 0x585858);
        assert_eq!(colour(Color::Rgb(1, 2, 3), INK, false), 0x010203);
    }
}
