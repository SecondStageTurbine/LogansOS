// SPDX-License-Identifier: MPL-2.0
//! Host discriminators for neo-text.
//!
//! Byte oracles come from paths that share no code with the rasterizer:
//! - `H` at 20 px: exact rational areas of the glyph's three rectangles,
//!   computed by hand (python `fractions`) from the outline points;
//! - curved glyphs: exact per-pixel areas from scripts/fonts/area_oracle.py
//!   (fontTools outlines, f64 polygon clipping), tight tolerance; plus a
//!   looser sanity cross-check against FreeType 2.13 unhinted coverage
//!   (scripts/fonts/freetype_oracle.py);
//! - advances and kerning: values read with fontTools (hmtx, GPOS PairPos).

mod exact_a_13;
mod exact_g_20;
mod exact_o_24;
mod exact_s_32;
mod font_integrity;
mod ft_a_13;
mod ft_g_20;
mod ft_o_24;
mod ft_s_32;
mod hb_kern_sans;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::canvas::{Canvas, Rect, blend_channel, blend_pixel};
use crate::flow::{FlowStyle, TextFlow};
use crate::font::{Font, darken_stems};
use crate::fonts::{INTER_REGULAR, PLEX_MONO_REGULAR};
use crate::layout::{layout_line, measure, quantize, wrap};
use crate::raster::{Point, Rasterizer};

fn sans() -> Font<'static> {
    Font::from_bytes(INTER_REGULAR).expect("bundled Inter parses")
}

fn mono() -> Font<'static> {
    Font::from_bytes(PLEX_MONO_REGULAR).expect("bundled Plex Mono parses")
}

// ---------------------------------------------------------------- rasterizer

#[test]
fn axis_aligned_square_has_exact_fractional_coverage() {
    // Square [0.5, 2.5] x [0.25, 1.75] in a 3x2 bitmap, clockwise in y-down.
    let mut r = Rasterizer::new(3, 2);
    let (a, b, c, d) = (
        Point::new(0.5, 0.25),
        Point::new(2.5, 0.25),
        Point::new(2.5, 1.75),
        Point::new(0.5, 1.75),
    );
    r.line(a, b);
    r.line(b, c);
    r.line(c, d);
    r.line(d, a);
    // Row 0 covers y 0.25..1 (0.75), row 1 covers 1..1.75 (0.75);
    // columns cover x 0.5, 1.0, 0.5. 0.375*255 = 95.6 -> 96, 0.75*255 = 191.25 -> 191.
    assert_eq!(r.coverage(), vec![96, 191, 96, 96, 191, 96]);
}

#[test]
fn winding_direction_does_not_change_coverage() {
    let tri = [
        Point::new(0.3, 0.2),
        Point::new(3.7, 1.1),
        Point::new(1.4, 3.9),
    ];
    let mut cw = Rasterizer::new(4, 4);
    let mut ccw = Rasterizer::new(4, 4);
    for i in 0..3 {
        cw.line(tri[i], tri[(i + 1) % 3]);
        ccw.line(tri[(i + 1) % 3], tri[i]);
    }
    assert_eq!(cw.coverage(), ccw.coverage());
}

#[test]
fn triangle_total_coverage_matches_its_area() {
    let tri = [
        Point::new(0.3, 0.2),
        Point::new(7.7, 1.1),
        Point::new(2.4, 6.9),
    ];
    let mut r = Rasterizer::new(8, 7);
    for i in 0..3 {
        r.line(tri[i], tri[(i + 1) % 3]);
    }
    // Shoelace area = 23.845. Per-pixel rounding is at most 0.5/255 each.
    let total: f64 = r.coverage().iter().map(|&c| f64::from(c) / 255.0).sum();
    assert!((total - 23.845).abs() < 56.0 * 0.5 / 255.0, "total {total}");
}

// ---------------------------------------------------------- byte oracle: 'H'

/// Inter `H` (glyph 161) is one 12-point contour that is exactly the union
/// of three rectangles (font units, upem 2048, y up):
///   left stem  x 180..370,  y 0..1490
///   crossbar   x 370..1152, y 678..846
///   right stem x 1152..1342, y 0..1490
/// At 20 px (scale 20/2048) with the origin shifted right 0.5 px the pixel
/// edges are: stems x 2.258..4.113 and 11.75..13.605; crossbar x
/// 4.113..11.75; stems y -14.551..0 and crossbar y -8.262..-6.621 (y down
/// from the baseline). Bitmap = floor/ceil of the bbox: left 2, top -15,
/// 12 x 15. Each byte is round_half_up(255 * exact overlap area), computed
/// with exact rational arithmetic (python `fractions.Fraction`) from the
/// fontTools `glyf` points; no value lies within 0.05 of a rounding boundary,
/// so exact equality is a fair demand. (The 0.25 px origin used for Plex puts
/// the right stem's edge at exactly x = 11.5, a byte on the boundary.)
#[rustfmt::skip]
const H_20PX_HALF: [u8; 12 * 15] = [
    104, 140,  16,   0,   0,   0,   0,   0,   0,  35, 140,  85,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  88,  67,  67,  67,  67,  67,  67, 114, 255, 154,
    189, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 154,
    189, 255, 115,  97,  97,  97,  97,  97,  97, 136, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
    189, 255,  29,   0,   0,   0,   0,   0,   0,  64, 255, 154,
];

#[test]
fn h_oracle_transcription_matches_rectangle_overlap() {
    // Guards the literal table above against transcription slips: recompute
    // each byte from the three rectangles by plain overlap arithmetic.
    let rects: [(f64, f64, f64, f64); 3] = [
        (180.0, 370.0, 0.0, 1490.0),
        (370.0, 1152.0, 678.0, 846.0),
        (1152.0, 1342.0, 0.0, 1490.0),
    ];
    let scale = 20.0 / 2048.0;
    for row in 0..15 {
        for col in 0..12 {
            let (px, py) = (f64::from(2 + col), f64::from(-15 + row));
            let mut area = 0.0;
            for (x0, x1, y0, y1) in rects {
                let (x0, x1) = (x0 * scale + 0.5, x1 * scale + 0.5);
                let (y0, y1) = (-y1 * scale, -y0 * scale);
                let w = (x1.min(px + 1.0) - x0.max(px)).max(0.0);
                let h = (y1.min(py + 1.0) - y0.max(py)).max(0.0);
                area += w * h;
            }
            let expected = (area * 255.0 + 0.5) as u8;
            assert_eq!(H_20PX_HALF[(row * 12 + col) as usize], expected);
        }
    }
}

#[test]
fn h_coverage_equals_hand_computed_byte_oracle() {
    let font = sans();
    let h = font.glyph_id('H');
    assert_eq!(h, 161);
    let bitmap = font.rasterize(h, 20.0, 0.5).expect("H has ink");
    assert_eq!((bitmap.left, bitmap.top), (2, -15));
    assert_eq!((bitmap.width, bitmap.height), (12, 15));
    assert_eq!(bitmap.coverage, H_20PX_HALF.to_vec());
}

// --------------------------------------------- curved glyphs: exact + FreeType

/// Max |ours - reference| per pixel and mean |diff| over the union of both
/// bitmaps, plus the reference's inked-pixel count.
fn compare(
    c: char,
    px: f32,
    x_offset: f32,
    reference: (i32, i32, usize, usize, &[u8]),
) -> (u8, f64, usize) {
    let font = sans();
    let ours = font.rasterize(font.glyph_id(c), px, x_offset).expect("ink");
    let (rl, rt, rw, rh, rdata) = reference;
    let x0 = ours.left.min(rl);
    let y0 = ours.top.min(rt);
    let x1 = (ours.left + ours.width as i32).max(rl + rw as i32);
    let y1 = (ours.top + ours.height as i32).max(rt + rh as i32);
    let sample = |l: i32, t: i32, w: usize, h: usize, d: &[u8], x: i32, y: i32| -> i32 {
        if x < l || y < t || x >= l + w as i32 || y >= t + h as i32 {
            0
        } else {
            i32::from(d[(y - t) as usize * w + (x - l) as usize])
        }
    };
    let (mut max, mut sum, mut n, mut inked) = (0u8, 0u64, 0u64, 0usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let a = sample(
                ours.left,
                ours.top,
                ours.width,
                ours.height,
                &ours.coverage,
                x,
                y,
            );
            let b = sample(rl, rt, rw, rh, rdata, x, y);
            let d = (a - b).unsigned_abs() as u8;
            max = max.max(d);
            sum += u64::from(d);
            n += 1;
            inked += usize::from(b > 0);
        }
    }
    (max, sum as f64 / n as f64, inked)
}

/// Exact-area oracle: the only expected error is our curve flattening
/// (FLATTEN_TOL = 0.005 px => at most ~1.3 LSB on an edge) plus rounding.
/// The mean bound is 0.4: Inter's `a` at 13 px puts 23 of its 63 pixels on
/// curved edges, each off the exact area by under 1 LSB, for a mean of 0.365
/// (Plex's `a` scored under 0.35); the max-2 bound is unchanged.
macro_rules! exact_case {
    ($name:ident, $c:expr, $px:expr, $off:expr, $m:ident) => {
        #[test]
        fn $name() {
            let (max, mean, inked) = compare(
                $c,
                $px,
                $off,
                ($m::LEFT, $m::TOP, $m::WIDTH, $m::HEIGHT, &$m::COVERAGE),
            );
            assert!(inked > 20, "oracle must have real ink");
            assert!(max <= 2, "max per-pixel diff {max} (mean {mean})");
            assert!(mean <= 0.4, "mean per-pixel diff {mean} (max {max})");
        }
    };
}

exact_case!(o_24px_matches_exact_area, 'o', 24.0, 0.0, exact_o_24);
exact_case!(
    g_20px_half_offset_matches_exact_area,
    'g',
    20.0,
    0.5,
    exact_g_20
);
exact_case!(
    s_32px_quarter_offset_matches_exact_area,
    'S',
    32.0,
    0.25,
    exact_s_32
);
exact_case!(
    a_13px_three_quarter_offset_matches_exact_area,
    'a',
    13.0,
    0.75,
    exact_a_13
);

/// Sanity cross-check against an established renderer. FreeType's gray
/// rasterizer flattens conics coarsely (chords sit a few hundredths of a pixel
/// inside the curve) and scales outlines to 26.6 fixed point, so curve edges
/// legitimately differ by up to ~1/16 px; straight edges and overall ink agree.
/// This is a sanity bound only (the exact-area oracle itself scores a mean of
/// ~3.1 against FreeType at 13 px); the discriminator is `exact_case!`.
macro_rules! freetype_case {
    ($name:ident, $c:expr, $px:expr, $m:ident) => {
        #[test]
        fn $name() {
            let (max, mean, inked) = compare(
                $c,
                $px,
                0.0,
                ($m::LEFT, $m::TOP, $m::WIDTH, $m::HEIGHT, &$m::COVERAGE),
            );
            assert!(inked > 20, "oracle must have real ink");
            assert!(max <= 24, "max per-pixel diff {max} (mean {mean})");
            assert!(mean <= 4.0, "mean per-pixel diff {mean} (max {max})");
        }
    };
}

freetype_case!(o_24px_agrees_with_freetype, 'o', 24.0, ft_o_24);
freetype_case!(g_20px_agrees_with_freetype, 'g', 20.0, ft_g_20);
freetype_case!(s_32px_agrees_with_freetype, 'S', 32.0, ft_s_32);
freetype_case!(a_13px_agrees_with_freetype, 'a', 13.0, ft_a_13);

// ------------------------------------------------------------------- layout

#[test]
fn advances_match_hmtx() {
    // hmtx advance widths (font units, upem 2048) read with fontTools.
    let font = sans();
    for (c, gid, adv) in [
        ('A', 2, 1413),
        ('V', 456, 1413),
        ('o', 790, 1228),
        (' ', 1777, 576),
        ('w', 984, 1676),
    ] {
        assert_eq!(font.glyph_id(c), gid, "{c:?}");
        assert_eq!(font.advance_units(gid), adv, "{c:?}");
    }
    // 1413 * 16 / 2048.
    assert!((font.advance(2, 16.0) - 11.039_062_5).abs() < 1e-5);
}

#[test]
fn gpos_kerning_pairs_match_fonttools() {
    let font = sans();
    let g = |c| font.glyph_id(c);
    // PairPos values read with fontTools from the same GPOS `kern` lookups,
    // each cross-checked against HarfBuzz. Inter's `kern` feature has two
    // lookups: lookup 1 is an Extension (type 9) lookup wrapping a Format-1
    // subtable then a Format-2 class subtable; lookup 2 is a plain PairPos
    // lookup, Format 1 then Format 2.
    // Extension lookup, Format-2 class subtable (most Latin pairs):
    assert_eq!(font.kerning_units(g('A'), g('V')), -140);
    assert_eq!(font.kerning_units(g('L'), g('T')), -197);
    assert_eq!(font.kerning_units(g('T'), g('o')), -160);
    assert_eq!(font.kerning_units(g('F'), g('A')), -186);
    assert_eq!(font.kerning_units(g('r'), g('.')), -128);
    assert_eq!(font.kerning_units(g('A'), g('r')), 0);
    // Extension lookup, Format-1 subtable (a listed pair):
    assert_eq!(font.kerning_units(g('&'), g('T')), -116);
    // `+` is in that Format-1 subtable's coverage but `+T` is not listed:
    // the lookup falls through to its Format-2 class subtable.
    assert_eq!(font.kerning_units(g('+'), g('T')), -151);
    // Plain lookup, Format-1 subtable:
    assert_eq!(font.kerning_units(g('&'), g('"')), -69);
    // Every lookup applies: lookup 1's class subtable matches `"` with a
    // zero record, and lookup 2's class subtable supplies the kern.
    assert_eq!(font.kerning_units(g('"'), g(',')), -168);
    // Monospace: no kerning, uniform 600-unit advances.
    let m = mono();
    assert_eq!(m.kerning_units(m.glyph_id('A'), m.glyph_id('V')), 0);
    assert_eq!(measure(&m, 10.0, "Hello, neo>"), 11.0 * 6.0);
}

/// Every printable-ASCII pair against HarfBuzz's shaped advances: an
/// independent, full-semantics GPOS implementation sharing no code with us.
#[test]
fn every_ascii_pair_kerns_like_harfbuzz() {
    let font = sans();
    let mut expected = std::collections::BTreeMap::new();
    for &(a, b, k) in hb_kern_sans::PAIRS {
        expected.insert((a, b), k);
    }
    let mut checked = 0;
    for a in ' '..='~' {
        for b in ' '..='~' {
            let pair: String = [a, b].iter().collect();
            if hb_kern_sans::SKIPPED.contains(&pair.as_str()) {
                continue;
            }
            let want = expected.get(&(a, b)).copied().unwrap_or(0);
            let got = font.kerning_units(font.glyph_id(a), font.glyph_id(b));
            assert_eq!(got, want, "pair {pair:?}");
            checked += 1;
        }
    }
    assert_eq!(checked, 95 * 95 - hb_kern_sans::SKIPPED.len());
    assert!(hb_kern_sans::PAIRS.len() > 1000);
}

#[test]
fn kerned_line_width_is_advances_plus_kern() {
    let font = sans();
    let w = measure(&font, 16.0, "AV");
    // Scale 16 / 2048; advances 1413 + 1413, kern -140.
    assert!(
        (w - (1413.0 + 1413.0 - 140.0) * 0.007_812_5).abs() < 1e-4,
        "{w}"
    );
    let (glyphs, _) = layout_line(&font, 16.0, "AV");
    assert!((glyphs[1].pen - (1413.0 - 140.0) * 0.007_812_5).abs() < 1e-4);
}

#[test]
fn quantize_rounds_to_quarter_pixels() {
    assert_eq!(quantize(10.0), (10, 0));
    assert_eq!(quantize(10.12), (10, 0));
    assert_eq!(quantize(10.13), (10, 1));
    assert_eq!(quantize(10.5), (10, 2));
    assert_eq!(quantize(10.88), (11, 0));
    assert_eq!(quantize(-0.25), (-1, 3));
}

fn lines<'t>(text: &'t str, ranges: &[core::ops::Range<usize>]) -> Vec<&'t str> {
    ranges.iter().map(|r| &text[r.clone()]).collect()
}

#[test]
fn wrap_breaks_at_last_fitting_space() {
    let font = sans();
    let text = "hello world from you";
    let width = measure(&font, 16.0, "hello world") + 0.5;
    assert!(measure(&font, 16.0, "from you") < width);
    assert!(measure(&font, 16.0, "hello world from") > width);
    assert_eq!(
        lines(text, &wrap(&font, 16.0, text, width)),
        ["hello world", "from you"]
    );
    // Exactly-fitting text never wraps; trailing spaces may overhang.
    let exact = measure(&font, 16.0, "hello");
    assert_eq!(
        lines("hello   ", &wrap(&font, 16.0, "hello   ", exact)),
        ["hello   "]
    );
}

#[test]
fn wrap_honours_newlines_and_breaks_long_words() {
    let font = sans();
    let text = "ab\n\nabcdefghij";
    let width = measure(&font, 16.0, "abcd") + 0.1;
    assert_eq!(
        lines(text, &wrap(&font, 16.0, text, width)),
        ["ab", "", "abcd", "efgh", "ij"]
    );
    // A width narrower than one glyph still makes progress, one char a line.
    assert_eq!(
        lines("abc", &wrap(&font, 16.0, "abc", 1.0)),
        ["a", "b", "c"]
    );
}

// ----------------------------------------------------------------- blending

#[test]
fn blending_happens_in_linear_light() {
    // ~50% white over black is sRGB(0.502 linear) -> 188, not the naive 128;
    // ~50% black over white is sRGB(0.498 linear) -> 187.
    assert_eq!(blend_channel(0, 255, 128), 188);
    assert_eq!(blend_channel(255, 0, 128), 187);
    assert_eq!(blend_channel(17, 200, 0), 17);
    assert_eq!(blend_channel(17, 200, 255), 200);
    assert_eq!(blend_pixel(0x0A0E27, 0xFFFFFF, 0), 0x0A0E27);
    assert_eq!(blend_pixel(0x0A0E27, 0xFFFFFF, 255), 0xFFFFFF);
    // Monotone in coverage.
    let mut prev = 0;
    for a in 0..=255u8 {
        let v = blend_channel(10, 250, a);
        assert!(v >= prev);
        prev = v;
    }
}

#[test]
fn stem_darkening_only_below_16px() {
    let ramp: Vec<u8> = (0..=255).collect();
    let mut at16 = ramp.clone();
    darken_stems(&mut at16, 16.0);
    assert_eq!(at16, ramp, "16 px and up is untouched");
    let mut at12 = ramp.clone();
    darken_stems(&mut at12, 12.0);
    assert_eq!((at12[0], at12[255]), (0, 255));
    assert!(at12.windows(2).all(|w| w[0] <= w[1]), "monotone");
    assert!(
        at12[128] >= 140,
        "mid coverage pushed toward ink: {}",
        at12[128]
    );
}

// --------------------------------------------------------------------- flow

const BG: u32 = 0x000A_0E27;
const FG: u32 = 0x00FF_FFFF;

fn style() -> FlowStyle {
    FlowStyle {
        px: 16.0,
        line_height: 24,
        baseline: 18,
        fg: FG,
        bg: BG,
    }
}

fn row_has_ink(pixels: &[u32], width: usize, y0: usize, y1: usize) -> bool {
    pixels[y0 * width..y1 * width].iter().any(|&p| p != BG)
}

#[test]
fn flow_wraps_words_and_reports_damage() {
    let (w, h) = (200usize, 72usize);
    let mut pixels = vec![BG; w * h];
    let mut canvas = Canvas::new(&mut pixels, w, h, w).unwrap();
    let mut font = sans();
    let mut flow = TextFlow::new(Rect::new(0, 0, w as i32, h as i32), style());
    assert_eq!(flow.rows(), 3);
    flow.push_str(&mut font, &mut canvas, "Arthur is ready.");
    let damage = flow.take_damage().expect("drawing damages");
    assert_eq!(damage, Rect::new(0, 0, 200, 24), "only the first band");
    assert_eq!(flow.take_damage(), None);
    flow.push_str(&mut font, &mut canvas, " Ask me anything");
    // "Arthur is ready. Ask me" overflows 200 px: the word in flight moves down.
    assert_eq!(flow.row(), 1);
    assert!(!flow.current_line().starts_with(' '));
    let mut expected = String::new();
    let full = "Arthur is ready. Ask me anything";
    for r in wrap(&font, 16.0, full, 200.0).iter().skip(1) {
        expected.push_str(&full[r.clone()]);
    }
    assert_eq!(flow.current_line(), expected);
    assert!(row_has_ink(&pixels, w, 0, 24) && row_has_ink(&pixels, w, 24, 48));
    assert!(!row_has_ink(&pixels, w, 48, 72));
}

#[test]
fn flow_backspace_restores_the_line_bit_for_bit() {
    let (w, h) = (240usize, 24usize);
    let mut before = vec![BG; w * h];
    let mut after = vec![BG; w * h];
    let mut font = sans();
    {
        let mut c = Canvas::new(&mut before, w, h, w).unwrap();
        TextFlow::new(c.bounds(), style()).push_str(&mut font, &mut c, "you> hello");
    }
    {
        let mut c = Canvas::new(&mut after, w, h, w).unwrap();
        let mut flow = TextFlow::new(c.bounds(), style());
        flow.push_str(&mut font, &mut c, "you> hellox\x08");
        assert_eq!(flow.current_line(), "you> hello");
    }
    assert_eq!(before, after);
}

#[test]
fn flow_scrolls_when_full() {
    let (w, h) = (200usize, 48usize);
    let mut pixels = vec![BG; w * h];
    let mut font = sans();
    let mut canvas = Canvas::new(&mut pixels, w, h, w).unwrap();
    let mut flow = TextFlow::new(canvas.bounds(), style());
    flow.push_str(&mut font, &mut canvas, "one\ntwo\n");
    assert_eq!(flow.row(), 1, "stays on the last row");
    assert_eq!(flow.take_damage(), Some(Rect::new(0, 0, 200, 48)));
    // "two" scrolled into band 0; band 1 is the fresh empty line.
    assert!(row_has_ink(&pixels, w, 0, 24));
    assert!(!row_has_ink(&pixels, w, 24, 48));
}

/// A9-D: the caret sits after the line's measured width, at the top of the
/// current band; `make_room` scrolls only as far as it must, keeping the
/// current line's text and leaving empty rows below it.
#[test]
fn flow_caret_and_make_room() {
    let (w, h) = (200usize, 72usize);
    let mut pixels = vec![BG; w * h];
    let mut font = sans();
    let mut flow = TextFlow::new(Rect::new(0, 0, w as i32, h as i32), style());
    assert_eq!(flow.caret(&font), (0, 0));
    let x = measure(&font, 16.0, "Arthur> ").ceil() as i32;
    {
        let mut canvas = Canvas::new(&mut pixels, w, h, w).unwrap();
        flow.push_str(&mut font, &mut canvas, "one\ntwo\nArthur> ");
        assert_eq!(flow.row(), 2);
        assert_eq!(flow.caret(&font), (x, 48));
        let _ = flow.take_damage();
        // Room below the first row already: nothing moves.
        let mut quiet = TextFlow::new(canvas.bounds(), style());
        quiet.make_room(&mut canvas, 1);
        assert_eq!((quiet.row(), quiet.take_damage()), (0, None));
        // On the last row: one scroll; the line keeps its text and moves up.
        flow.make_room(&mut canvas, 1);
    }
    assert_eq!(flow.row(), 1);
    assert_eq!(flow.current_line(), "Arthur> ");
    assert_eq!(flow.caret(&font), (x, 24));
    assert_eq!(flow.take_damage(), Some(Rect::new(0, 0, 200, 72)));
    assert!(
        row_has_ink(&pixels, w, 24, 48),
        "the current line moved up a band"
    );
    assert!(!row_has_ink(&pixels, w, 48, 72), "an empty row below it");
    // Never past the top: asking for more rows than exist stops there.
    {
        let mut canvas = Canvas::new(&mut pixels, w, h, w).unwrap();
        flow.make_room(&mut canvas, 5);
    }
    assert_eq!(flow.row(), 0);
    assert_eq!(flow.current_line(), "Arthur> ");
    assert!(row_has_ink(&pixels, w, 0, 24) && !row_has_ink(&pixels, w, 24, 72));
}

/// A9-C: a scrollback flow draws exactly as a plain one; its log holds the
/// finished lines (wraps included), bounded with stable absolute indices; and
/// the bottom view it renders again is pixel-for-pixel what it drew itself,
/// after wraps and scrolls.
#[test]
fn scrollback_flow_logs_lines_and_redraws_its_own_view() {
    let (w, h) = (200usize, 72usize);
    let text = "one\ntwo is a line long enough to wrap around the edge\nthree\nfour\nfive";
    let mut font = sans();
    let mut plain = vec![BG; w * h];
    let mut kept = vec![BG; w * h];
    {
        let mut c = Canvas::new(&mut plain, w, h, w).unwrap();
        TextFlow::new(c.bounds(), style()).push_str(&mut font, &mut c, text);
    }
    let mut flow;
    {
        let mut c = Canvas::new(&mut kept, w, h, w).unwrap();
        flow = TextFlow::with_scrollback(c.bounds(), style(), 4);
        flow.push_str(&mut font, &mut c, text);
    }
    assert_eq!(plain, kept, "keeping lines changes no pixel");
    // "two ..." wrapped once: one, two-a, two-b, three, four finished, five
    // current. Cap 4: "one" was let go.
    assert_eq!(flow.current_index(), 5);
    assert_eq!(flow.first_kept(), 1);
    assert_eq!(flow.current_line(), "five");
    assert_eq!(flow.bottom_top(), 3);
    let _ = flow.take_damage();
    let mut again = vec![0x1234_5678u32; w * h];
    {
        let mut c = Canvas::new(&mut again, w, h, w).unwrap();
        let top = flow.bottom_top();
        flow.render_view(&mut font, &mut c, top);
    }
    assert_eq!(again, kept, "the bottom view redraws the flow's own pixels");
    assert_eq!(flow.take_damage(), Some(Rect::new(0, 0, 200, 72)));
    // A view two lines up starts at the second half of "two ..." (index 2):
    // the same pixels as a flow that only ever held those three lines.
    let mut up = vec![BG; w * h];
    {
        let mut c = Canvas::new(&mut up, w, h, w).unwrap();
        flow.render_view(&mut font, &mut c, 1);
    }
    let mut want = vec![BG; w * h];
    {
        let mut c = Canvas::new(&mut want, w, h, w).unwrap();
        let mut fresh = TextFlow::new(c.bounds(), style());
        fresh.push_str(&mut font, &mut c, text);
    }
    // Rows 0..3 of the view at top=1 hold lines 1, 2, 3; the plain flow's
    // screen at its bottom holds 3, 4, 5. Compare row 2 of the view (line 3,
    // "three") with row 0 of the plain screen (also line 3).
    let row = |px: &[u32], r: usize| px[r * 24 * w..(r + 1) * 24 * w].to_vec();
    assert_eq!(row(&up, 2), row(&want, 0));
    // A top before the oldest kept line is clamped to it.
    let mut clamped = vec![BG; w * h];
    {
        let mut c = Canvas::new(&mut clamped, w, h, w).unwrap();
        flow.render_view(&mut font, &mut c, 0);
    }
    assert_eq!(clamped, up);
}

/// A9-C with A9-D: after `make_room` scrolls the current line up, the
/// bottom view moves with it and still redraws the flow's own pixels.
#[test]
fn scrollback_bottom_view_follows_make_room() {
    let (w, h) = (200usize, 72usize);
    let mut font = sans();
    let mut kept = vec![BG; w * h];
    let mut flow;
    {
        let mut c = Canvas::new(&mut kept, w, h, w).unwrap();
        flow = TextFlow::with_scrollback(c.bounds(), style(), 8);
        flow.push_str(&mut font, &mut c, "one\ntwo\nArthur> ");
        assert_eq!(flow.bottom_top(), 0);
        flow.make_room(&mut c, 1);
    }
    assert_eq!((flow.row(), flow.bottom_top()), (1, 1));
    let _ = flow.take_damage();
    let mut again = vec![0x1234_5678u32; w * h];
    {
        let mut c = Canvas::new(&mut again, w, h, w).unwrap();
        let top = flow.bottom_top();
        flow.render_view(&mut font, &mut c, top);
    }
    assert_eq!(
        again, kept,
        "the moved-up bottom view is the flow's own pixels"
    );
}

// ------------------------------------------- rasterizer vs exact clipping

/// Exact area of polygon `poly` inside pixel square (px, py): Sutherland-Hodgman
/// clip in f64, then the shoelace formula. Shares nothing with `Rasterizer`.
fn clip_area(poly: &[(f64, f64)], px: f64, py: f64) -> f64 {
    let mut pts: Vec<(f64, f64)> = poly.to_vec();
    for (bound, is_x, keep_ge) in [
        (px, true, true),
        (px + 1.0, true, false),
        (py, false, true),
        (py + 1.0, false, false),
    ] {
        let coord = |p: (f64, f64)| if is_x { p.0 } else { p.1 };
        let inside = |p: (f64, f64)| {
            if keep_ge {
                coord(p) >= bound
            } else {
                coord(p) <= bound
            }
        };
        let mut out = Vec::new();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if inside(a) {
                out.push(a);
            }
            if inside(a) != inside(b) {
                let t = (bound - coord(a)) / (coord(b) - coord(a));
                out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
        }
        pts = out;
        if pts.is_empty() {
            return 0.0;
        }
    }
    let mut twice = 0.0;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        twice += a.0 * b.1 - b.0 * a.1;
    }
    (twice / 2.0).abs()
}

/// Many short edges (a 2000-gon ellipse) and a coarse 12-gon: every pixel must
/// equal the exactly clipped area to within one rounding step.
#[test]
fn polygon_coverage_matches_exact_clipping() {
    for n in [12usize, 2000] {
        let poly: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64 * core::f64::consts::TAU;
                (5.0 + 4.3 * libm::cos(t), 5.0 + 3.7 * libm::sin(t))
            })
            .collect();
        let mut r = Rasterizer::new(10, 10);
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            r.line(
                Point::new(a.0 as f32, a.1 as f32),
                Point::new(b.0 as f32, b.1 as f32),
            );
        }
        let cov = r.coverage();
        for y in 0..10 {
            for x in 0..10 {
                let exact = (clip_area(&poly, x as f64, y as f64) * 255.0 + 0.5) as i32;
                let got = i32::from(cov[y * 10 + x]);
                assert!(
                    (got - exact).abs() <= 1,
                    "n={n} ({x},{y}) got {got} exact {exact}"
                );
            }
        }
    }
}
