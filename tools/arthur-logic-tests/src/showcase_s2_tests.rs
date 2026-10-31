// SPDX-License-Identifier: MPL-2.0
//! Desktop S2 host KATs for Arthur's TEXT surface.
//!
//! 1. Kill-switch: `LegacyGrid` (the `arthur-legacy-5x7-font` path) is
//!    bit-for-bit the renderer neo-arthur shipped before S2. The reference
//!    below is the pre-S2 `ArthurSurface` drawing code copied verbatim from
//!    showcase_surface.rs at 82d0f59 (raw-pointer volatile writes and all).
//! 2. The S2 surface's title glyphs match the exact-area oracle shared with the
//!    kernel framebuffer assert (fixtures/arthur-s2/title_window.rs), which is
//!    produced by scripts/fonts/area_oracle.py with no neo-text code.
//! 3. Damage: the text engine commits only the bands it touched.

use super::showcase_logic::{
    GLYPH_HEIGHT, GLYPH_WIDTH, LegacyGrid, SURFACE_BG, SURFACE_COLS, SURFACE_FG, SURFACE_HEIGHT,
    SURFACE_ROWS, SURFACE_WIDTH, glyph_rows,
};
use super::showcase_text::{GREETING, LINE_HEIGHT, TRANSCRIPT, TextSurface};

mod title_window {
    include!("../../../fixtures/arthur-s2/title_window.rs");
}

/// Pre-S2 renderer, verbatim (only the struct wrapper is new).
struct PreS2Surface {
    pixels: *mut u32,
    row: usize,
    col: usize,
}

impl PreS2Surface {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\r' => {}
            b'\n' => self.newline(),
            b'\t' => {
                for _ in 0..4 {
                    self.write_byte(b' ');
                }
            }
            0x08 | 0x7f => self.backspace(),
            0x20..=0x7e => {
                if self.col >= SURFACE_COLS {
                    self.newline();
                }
                self.draw_cell(self.col, self.row, byte);
                self.col += 1;
            }
            _ => self.write_byte(b'?'),
        }
    }

    pub fn write_text(&mut self, text: &[u8]) {
        for &byte in text {
            self.write_byte(byte);
        }
    }

    fn clear(&mut self) {
        // SAFETY: the client role mapping covers exactly SURFACE_WIDTH *
        // SURFACE_HEIGHT u32 pixels, established by the immutable descriptor.
        unsafe {
            for index in 0..SURFACE_WIDTH * SURFACE_HEIGHT {
                core::ptr::write_volatile(self.pixels.add(index), SURFACE_BG);
            }
        }
    }

    fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= SURFACE_ROWS {
            self.scroll();
            self.row = SURFACE_ROWS - 1;
        }
    }

    fn backspace(&mut self) {
        if self.col == 0 {
            return;
        }
        self.col -= 1;
        self.draw_cell(self.col, self.row, b' ');
    }

    fn scroll(&mut self) {
        // SAFETY: source and destination indices are inside the same client RW
        // mapping. Volatile row order handles overlap by copying upward.
        unsafe {
            for y in GLYPH_HEIGHT..SURFACE_HEIGHT {
                for x in 0..SURFACE_WIDTH {
                    let value = core::ptr::read_volatile(self.pixels.add(y * SURFACE_WIDTH + x));
                    core::ptr::write_volatile(
                        self.pixels.add((y - GLYPH_HEIGHT) * SURFACE_WIDTH + x),
                        value,
                    );
                }
            }
            for y in SURFACE_HEIGHT - GLYPH_HEIGHT..SURFACE_HEIGHT {
                for x in 0..SURFACE_WIDTH {
                    core::ptr::write_volatile(self.pixels.add(y * SURFACE_WIDTH + x), SURFACE_BG);
                }
            }
        }
    }

    fn draw_cell(&mut self, cell_x: usize, cell_y: usize, byte: u8) {
        let x0 = cell_x * GLYPH_WIDTH;
        let y0 = cell_y * GLYPH_HEIGHT;
        let rows = glyph_rows(byte);
        // SAFETY: cell_x/cell_y are bounded by SURFACE_COLS/SURFACE_ROWS; each
        // write therefore stays inside the descriptor-sized client mapping.
        unsafe {
            for y in 0..GLYPH_HEIGHT {
                for x in 0..GLYPH_WIDTH {
                    core::ptr::write_volatile(
                        self.pixels.add((y0 + y) * SURFACE_WIDTH + x0 + x),
                        SURFACE_BG,
                    );
                }
            }
            for (glyph_y, bits) in rows.iter().copied().enumerate() {
                for glyph_x in 0..5 {
                    if bits & (1 << (4 - glyph_x)) == 0 {
                        continue;
                    }
                    let x = x0 + 1 + glyph_x;
                    let y = y0 + 1 + glyph_y * 2;
                    core::ptr::write_volatile(self.pixels.add(y * SURFACE_WIDTH + x), SURFACE_FG);
                    core::ptr::write_volatile(
                        self.pixels.add((y + 1) * SURFACE_WIDTH + x),
                        SURFACE_FG,
                    );
                }
            }
        }
    }
}

const PIXELS: usize = SURFACE_WIDTH * SURFACE_HEIGHT;

fn legacy_stream() -> Vec<u8> {
    let mut s = Vec::new();
    s.extend_from_slice(b"ARTHUR / LOCAL\n\nARTHUR IS READY.\nASK ME ANYTHING.\n\nYOU> ");
    s.extend_from_slice(b"hello wrx\x08\x08orld\x7f\x7fld\r");
    s.extend_from_slice(b"\n\nArthur> Thinking...\n\nArthur> ");
    // Longer than 80 columns (forces the grid's own wrap), a tab, bytes the
    // 5x7 table lacks, and non-ASCII / control bytes.
    for i in 0..3 {
        s.extend_from_slice(
            b"The quick brown fox jumps over the lazy dog; 0123456789 {}|~` @#$%^&*\t",
        );
        s.push(0xC3);
        s.push(0x01 + i);
    }
    // Enough newlines to scroll the 25-row grid several times.
    for i in 0..40u8 {
        s.extend_from_slice(b"line ");
        s.push(b'a' + i % 26);
        s.push(b'\n');
    }
    s.extend_from_slice(b"\x08\x08 tail");
    s
}

#[test]
fn legacy_grid_is_bit_for_bit_the_pre_s2_renderer() {
    let mut reference = vec![0xDEAD_BEEFu32; PIXELS];
    let mut ours = vec![0xDEAD_BEEFu32; PIXELS];
    let mut pre = PreS2Surface {
        pixels: reference.as_mut_ptr(),
        row: 0,
        col: 0,
    };
    let mut grid = LegacyGrid::new();
    pre.clear();
    grid.clear(&mut ours);
    assert!(reference == ours, "clear");
    for (i, &byte) in legacy_stream().iter().enumerate() {
        pre.write_byte(byte);
        grid.write_byte(&mut ours, byte);
        assert!(
            reference == ours,
            "diverged at stream byte {i} ({byte:#04x})"
        );
    }
    assert!(ours.contains(&SURFACE_FG), "stream drew ink");
}

#[test]
fn legacy_grid_folds_case_so_mixed_case_strings_are_a_no_op() {
    // main.rs now writes "Arthur> Thinking..." (for the S2 engine); under the
    // kill-switch that must still render exactly the old "ARTHUR> THINKING...".
    let render = |text: &[u8]| {
        let mut px = vec![0u32; PIXELS];
        let mut grid = LegacyGrid::new();
        grid.clear(&mut px);
        for &b in text {
            grid.write_byte(&mut px, b);
        }
        px
    };
    assert!(
        render(b"\n\nArthur> Thinking...\n\nArthur> ")
            == render(b"\n\nARTHUR> THINKING...\n\nARTHUR> ")
    );
}

fn s2_surface() -> (TextSurface<'static>, Vec<u32>) {
    let mut px = vec![0xDEAD_BEEFu32; PIXELS];
    let mut text = TextSurface::new().expect("bundled fonts parse");
    text.paint_chrome(&mut px);
    for &b in GREETING.as_bytes() {
        text.write_byte(&mut px, b);
    }
    (text, px)
}

#[test]
fn s2_title_glyphs_match_exact_area_oracle() {
    let (_, px) = s2_surface();
    let (wx, wy, ww, wh) = title_window::WINDOW;
    let mut ink = 0;
    for y in 0..wh {
        for x in 0..ww {
            let got = px[(wy + y) * SURFACE_WIDTH + wx + x];
            let (lo, hi) = (title_window::LO[y * ww + x], title_window::HI[y * ww + x]);
            for shift in [16, 8, 0] {
                let (g, l, h) = (
                    (got >> shift) & 0xFF,
                    (lo >> shift) & 0xFF,
                    (hi >> shift) & 0xFF,
                );
                assert!(
                    l <= g && g <= h,
                    "pixel ({}, {}) = {got:#08x} outside [{lo:#08x}, {hi:#08x}]",
                    wx + x,
                    wy + y
                );
            }
            ink += usize::from(got & 0xFF > 0x80);
        }
    }
    assert!(
        ink >= title_window::INK_PIXELS / 2,
        "window must hold real ink, got {ink}"
    );
}

#[test]
fn s2_surface_is_mixed_case_proportional_text_not_the_grid() {
    let (_, px) = s2_surface();
    let mut grid_px = vec![0u32; PIXELS];
    LegacyGrid::new().clear(&mut grid_px);
    assert!(px != grid_px);
    // Anti-aliased: intermediate (neither ground nor text-primary) pixels exist.
    let ground = super::showcase_text::GROUND;
    let partial = px
        .iter()
        .filter(|&&p| p != ground && p != 0x00FF_FFFF)
        .count();
    assert!(partial > 200, "AA edges present: {partial}");
    // Case is preserved: "Arthur is ready." and "ARTHUR IS READY." differ.
    let render = |s: &[u8]| {
        let mut px = vec![0u32; PIXELS];
        let mut t = TextSurface::new().unwrap();
        t.paint_chrome(&mut px);
        for &b in s {
            t.write_byte(&mut px, b);
        }
        px
    };
    assert!(render(b"Arthur is ready.") != render(b"ARTHUR IS READY."));
}

#[test]
fn s2_damage_is_the_touched_band_only() {
    let (mut text, mut px) = s2_surface();
    assert_eq!(
        text.take_damage(),
        Some((0, 0, SURFACE_WIDTH as u16, SURFACE_HEIGHT as u16))
    );
    assert_eq!(text.take_damage(), None);
    // The greeting leaves the cursor on transcript row 2 ("You> ").
    text.write_byte(&mut px, b'h');
    let band_y = TRANSCRIPT.y + 2 * LINE_HEIGHT;
    assert_eq!(
        text.take_damage(),
        Some((
            TRANSCRIPT.x as u16,
            band_y as u16,
            TRANSCRIPT.width as u16,
            LINE_HEIGHT as u16
        ))
    );
    // Pixels outside that band are untouched by the keystroke.
    let (_, before) = s2_surface();
    for y in 0..SURFACE_HEIGHT {
        if (band_y as usize..band_y as usize + LINE_HEIGHT as usize).contains(&y) {
            continue;
        }
        assert!(
            px[y * SURFACE_WIDTH..(y + 1) * SURFACE_WIDTH]
                == before[y * SURFACE_WIDTH..(y + 1) * SURFACE_WIDTH],
            "row {y}"
        );
    }
}

#[test]
fn s2_long_answer_wraps_and_scrolls_inside_the_transcript() {
    let (mut text, mut px) = s2_surface();
    let (_, chrome) = s2_surface();
    let _ = text.take_damage();
    for _ in 0..30 {
        for &b in b"Arthur> a fairly long answer line that keeps going past the right margin\n" {
            text.write_byte(&mut px, b);
        }
    }
    // Header and footer never move: everything outside the transcript region
    // is identical to the freshly painted surface.
    let t = TRANSCRIPT;
    for y in 0..SURFACE_HEIGHT {
        for x in 0..SURFACE_WIDTH {
            let inside = (t.x as usize..(t.x + t.width) as usize).contains(&x)
                && (t.y as usize..(t.y + t.height) as usize).contains(&y);
            if !inside {
                assert_eq!(
                    px[y * SURFACE_WIDTH + x],
                    chrome[y * SURFACE_WIDTH + x],
                    "({x},{y})"
                );
            }
        }
    }
    let damage = text.take_damage().unwrap();
    assert_eq!(
        damage,
        (t.x as u16, t.y as u16, t.width as u16, t.height as u16)
    );
}

/// A9-D: the busy emblem's rect starts just right of "Arthur> ", spans that
/// line and the one below, holds only ground (nothing Arthur drew is under
/// the emblem), and stays inside the transcript, scrolling it a line when the
/// caret is on the last row.
#[test]
fn busy_rect_is_empty_ground_right_of_the_caret() {
    use super::showcase_text::{BUSY_GAP, BUSY_WIDTH, GROUND};
    let t = TRANSCRIPT;
    let check = |text: &mut TextSurface<'static>, px: &mut Vec<u32>, row: i32| {
        let (x, y, w, h) = text.reserve_busy(px).expect("on the surface");
        let (x, y, w, h) = (x as i32, y as i32, w as i32, h as i32);
        assert_eq!(
            (y, w, h),
            (t.y + row * LINE_HEIGHT, BUSY_WIDTH, 2 * LINE_HEIGHT)
        );
        // Right of "Arthur> " (ink to its left on the line), in the transcript.
        assert!(x > t.x + BUSY_GAP && x + w <= t.x + t.width);
        assert!(y + h <= t.y + t.height);
        let line = &px[y as usize * SURFACE_WIDTH..(y + LINE_HEIGHT) as usize * SURFACE_WIDTH];
        assert!(
            line.chunks(SURFACE_WIDTH)
                .any(|r| r[t.x as usize..(x - BUSY_GAP) as usize]
                    .iter()
                    .any(|&p| p != GROUND)),
            "\"Arthur> \" is left of the rect"
        );
        for yy in y..y + h {
            for xx in x..x + w {
                assert_eq!(
                    px[yy as usize * SURFACE_WIDTH + xx as usize],
                    GROUND,
                    "({xx},{yy})"
                );
            }
        }
    };
    // Mid-transcript: the greeting, a question, then Arthur's prompt on row 3.
    let (mut text, mut px) = s2_surface();
    typed(&mut text, &mut px, b"hi\nArthur> ");
    let _ = text.take_damage();
    check(&mut text, &mut px, 3);
    assert_eq!(
        text.take_damage(),
        None,
        "room below already: nothing scrolled"
    );
    // On the last row: one line scrolls away to make room below it.
    let (mut text, mut px) = s2_surface();
    for _ in 0..20 {
        typed(&mut text, &mut px, b"a line\n");
    }
    typed(&mut text, &mut px, b"Arthur> ");
    let _ = text.take_damage();
    let last = t.height / LINE_HEIGHT - 1;
    check(&mut text, &mut px, last - 1);
    assert_eq!(
        text.take_damage(),
        Some((t.x as u16, t.y as u16, t.width as u16, t.height as u16))
    );
}

/// A9-C with A9-D: reserving the busy rect while scrolled up brings the
/// newest view back first, so the rect is empty ground beside the caret,
/// exactly as for a surface never scrolled.
#[test]
fn busy_rect_while_scrolled_up_follows_first() {
    use super::showcase_logic::ScrollKey;
    let (mut text, mut px) = long_transcript();
    typed(&mut text, &mut px, b"Arthur> ");
    let (mut never, mut never_px) = long_transcript();
    typed(&mut never, &mut never_px, b"Arthur> ");
    assert!(text.scroll(&mut px, ScrollKey::PageUp));
    let rect = text.reserve_busy(&mut px);
    assert_eq!(text.view_top(), None);
    assert_eq!(rect, never.reserve_busy(&mut never_px));
    assert_eq!(px, never_px);
}

/// A9-C: the scroll keys decode from their HID usages (presses only), and
/// the ASCII decoder still has no byte for them.
#[test]
fn scroll_keys_decode_from_their_usages() {
    use super::showcase_logic::{
        ScrollKey, input_event_to_ascii, input_event_to_scroll, scroll_allowed,
    };
    // kind 1 (key), flags bit 0 (pressed), usage LE, modifiers.
    let key = |usage: u8, pressed: bool| [1, pressed as u8, usage, 0, 0, 0, 0, 0];
    for (usage, want) in [
        (0x4b, ScrollKey::PageUp),
        (0x4e, ScrollKey::PageDown),
        (0x52, ScrollKey::LineUp),
        (0x51, ScrollKey::LineDown),
    ] {
        assert_eq!(input_event_to_scroll(key(usage, true)), Some(want));
        assert_eq!(input_event_to_scroll(key(usage, false)), None, "release");
        assert_eq!(input_event_to_ascii(key(usage, true)), None);
    }
    assert_eq!(input_event_to_scroll(key(0x04, true)), None, "the letter a");
    // A pointer record (kind 3) whose code happens to be 0x4b.
    assert_eq!(input_event_to_scroll([3, 1, 0x4b, 0, 0, 0, 0, 0]), None);
    assert!(scroll_allowed(ScrollKey::PageUp, false));
    assert!(scroll_allowed(ScrollKey::PageDown, false));
    assert!(!scroll_allowed(ScrollKey::LineUp, false));
    assert!(scroll_allowed(ScrollKey::LineUp, true));
    assert!(scroll_allowed(ScrollKey::LineDown, true));
    // A9-C2: kind 5 (a forwarded wheel event), the wheel in `code` (i16 LE).
    assert_eq!(
        input_event_to_scroll([5, 0, 0x01, 0x00, 40, 0, 7, 0]),
        Some(ScrollKey::Wheel(1))
    );
    assert_eq!(
        input_event_to_scroll([5, 1, 0xfe, 0xff, 0, 0, 0, 0]),
        Some(ScrollKey::Wheel(-2))
    );
    assert_eq!(
        input_event_to_scroll([5, 0, 0, 0, 0, 0, 0, 0]),
        None,
        "no wheel"
    );
    assert_eq!(input_event_to_ascii([5, 0, 1, 0, 0, 0, 0, 0]), None);
    assert!(scroll_allowed(ScrollKey::Wheel(1), false));
}

/// A9-C2: a wheel notch away from the user scrolls three lines toward older
/// text; one back toward the user returns, and follows at the newest.
#[test]
fn wheel_notches_scroll_three_lines() {
    use super::showcase_logic::{ScrollKey, WHEEL_LINES};
    let (mut text, mut px) = long_transcript();
    let before = px.clone();
    let bottom = 42 - (TRANSCRIPT.height / LINE_HEIGHT) as u64 + 1;
    assert!(text.scroll(&mut px, ScrollKey::Wheel(1)));
    assert_eq!(text.view_top(), Some(bottom - WHEEL_LINES as u64));
    assert!(text.scroll(&mut px, ScrollKey::Wheel(2)));
    assert_eq!(text.view_top(), Some(bottom - 3 * WHEEL_LINES as u64));
    assert!(text.scroll(&mut px, ScrollKey::Wheel(-3)));
    assert_eq!(text.view_top(), None);
    assert_eq!(px, before);
}

/// A surface with 40 numbered lines: 28 scrolled off the 12-row transcript.
fn long_transcript() -> (TextSurface<'static>, Vec<u32>) {
    let (mut text, mut px) = s2_surface();
    for i in 0..40 {
        typed(&mut text, &mut px, format!("line {i}\n").as_bytes());
    }
    let _ = text.take_damage();
    (text, px)
}

fn transcript_rows(px: &[u32]) -> Vec<u32> {
    let t = TRANSCRIPT;
    let mut out = Vec::new();
    for y in t.y..t.y + t.height {
        let row = y as usize * SURFACE_WIDTH;
        out.extend_from_slice(&px[row + t.x as usize..row + (t.x + t.width) as usize]);
    }
    out
}

/// A9-C: PgUp moves the view a page (rows less one) up and redraws only the
/// transcript; PgDn back to the newest text follows again and is the
/// unscrolled picture bit for bit; the view stops at the oldest kept line.
#[test]
fn page_keys_scroll_the_view_and_come_back_exactly() {
    use super::showcase_logic::ScrollKey;
    let (mut text, mut px) = long_transcript();
    let before = px.clone();
    let rows = (TRANSCRIPT.height / LINE_HEIGHT) as u64;
    // The greeting finishes two lines, the typed ones forty more, and the
    // current line (index 42) is empty: the newest view's top row is line
    // 42 - (rows - 1) = 31.
    let bottom = 42 - (rows - 1);
    assert!(text.scroll(&mut px, ScrollKey::PageUp));
    assert_eq!(text.view_top(), Some(bottom - (rows - 1)));
    assert_eq!(
        text.take_damage(),
        Some((
            TRANSCRIPT.x as u16,
            TRANSCRIPT.y as u16,
            TRANSCRIPT.width as u16,
            TRANSCRIPT.height as u16
        ))
    );
    assert_ne!(transcript_rows(&px), transcript_rows(&before));
    // Outside the transcript nothing moved.
    let t = TRANSCRIPT;
    for (i, (a, b)) in px.iter().zip(&before).enumerate() {
        let (x, y) = ((i % SURFACE_WIDTH) as i32, (i / SURFACE_WIDTH) as i32);
        if !(t.x..t.x + t.width).contains(&x) || !(t.y..t.y + t.height).contains(&y) {
            assert_eq!(a, b, "({x},{y})");
        }
    }
    // Line by line down again, then PgDn to the end: follows, same pixels.
    assert!(text.scroll(&mut px, ScrollKey::LineDown));
    assert_eq!(text.view_top(), Some(bottom - (rows - 1) + 1));
    assert!(text.scroll(&mut px, ScrollKey::PageDown));
    assert_eq!(text.view_top(), None);
    assert_eq!(px, before);
    assert!(
        !text.scroll(&mut px, ScrollKey::PageDown),
        "already at the newest"
    );
    // Up past the start stops at line 0.
    for _ in 0..10 {
        text.scroll(&mut px, ScrollKey::PageUp);
    }
    assert_eq!(text.view_top(), Some(0));
    assert!(!text.scroll(&mut px, ScrollKey::LineUp));
}

/// A9-C: text written while scrolled up (Arthur's answer) leaves the view
/// where the user put it once the frame settles; typing (`follow`) brings
/// the newest text back, the same picture as a surface never scrolled.
#[test]
fn the_view_holds_while_scrolled_and_follows_after() {
    use super::showcase_logic::ScrollKey;
    let (mut text, mut px) = long_transcript();
    text.scroll(&mut px, ScrollKey::PageUp);
    let top = text.view_top();
    let held = px.clone();
    typed(&mut text, &mut px, b"Arthur> a new answer arrives\n");
    text.settle(&mut px);
    assert_eq!(text.view_top(), top);
    assert_eq!(px, held, "the view the user scrolled to stays");
    text.follow(&mut px);
    assert_eq!(text.view_top(), None);
    let (mut never, mut never_px) = long_transcript();
    typed(&mut never, &mut never_px, b"Arthur> a new answer arrives\n");
    assert_eq!(px, never_px);
    // While following, settle changes nothing.
    let now = px.clone();
    text.settle(&mut px);
    assert_eq!(px, now);
}

/// `ARTHUR_S2_PREVIEW=/path/out.ppm cargo test -p arthur-logic-tests preview`
/// writes the host-rendered surface (with a sample exchange) for eyeballing.
#[test]
fn preview_s2_surface() {
    let Ok(path) = std::env::var("ARTHUR_S2_PREVIEW") else {
        return;
    };
    let (mut text, mut px) = s2_surface();
    for &b in b"hello world\n\nArthur> Thinking...\n\nArthur> Hello! I am Arthur, running locally on this machine. How can I help you today?\n" {
        text.write_byte(&mut px, b);
    }
    let mut out = format!("P6\n{SURFACE_WIDTH} {SURFACE_HEIGHT}\n255\n").into_bytes();
    for p in px {
        out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    std::fs::write(path, out).unwrap();
}

// ── DI-1b resize: a sized surface in a buffer built for the largest ─────────

const MAX_W: usize = 1280;
const MAX_H: usize = 800;
const CANARY: u32 = 0xDEAD_BEEF;

/// Chad, #100 RETURN item 1: a view held at the oldest kept line, while
/// appended text evicts lines past the 512-line cap, moves with the oldest
/// line; the next page down counts from the line on screen. 512 finished
/// lines (the greeting's two plus 510), the view paged up to line 0, then 10
/// more lines appended while held: lines 0..=9 are evicted, the screen shows
/// line 10 at its top, and PgDn lands on 10 + 11 = 21, not 0 + 11 = 11.
#[test]
fn held_view_moves_with_evicted_lines() {
    use super::showcase_logic::ScrollKey;
    let (mut text, mut px) = s2_surface();
    for i in 0..510 {
        typed(&mut text, &mut px, format!("line {i}\n").as_bytes());
    }
    while text.scroll(&mut px, ScrollKey::PageUp) {}
    assert_eq!(text.view_top(), Some(0));
    for i in 0..10 {
        typed(&mut text, &mut px, format!("more {i}\n").as_bytes());
    }
    text.settle(&mut px);
    assert_eq!(text.view_top(), Some(10), "the oldest kept line");
    assert!(text.scroll(&mut px, ScrollKey::PageDown));
    assert_eq!(text.view_top(), Some(21));
}

fn typed(text: &mut TextSurface<'static>, px: &mut [u32], bytes: &[u8]) {
    for &b in bytes {
        text.write_byte(px, b);
    }
}

const EXCHANGE: &[u8] =
    b"hello desktop\nArthur> I can hear you, but no model is installed yet.\n\nYou> again";

/// Kill-switch half: at the created size, rendering into a 1280-wide
/// mapping is the 640-wide rendering row for row, and leaves the rest of the
/// mapping untouched.
#[test]
fn a_wide_mapping_renders_the_created_surface_unchanged() {
    let (mut tight, mut tight_px) = s2_surface();
    typed(&mut tight, &mut tight_px, EXCHANGE);
    let mut wide_px = vec![CANARY; MAX_W * MAX_H];
    let mut wide = TextSurface::with_stride(MAX_W).unwrap();
    wide.paint_chrome(&mut wide_px);
    typed(&mut wide, &mut wide_px, GREETING.as_bytes());
    typed(&mut wide, &mut wide_px, EXCHANGE);
    for y in 0..MAX_H {
        let row = &wide_px[y * MAX_W..(y + 1) * MAX_W];
        if y < SURFACE_HEIGHT {
            assert!(row[..SURFACE_WIDTH] == tight_px[y * SURFACE_WIDTH..(y + 1) * SURFACE_WIDTH]);
            assert!(row[SURFACE_WIDTH..].iter().all(|&p| p == CANARY), "row {y}");
        } else {
            assert!(row.iter().all(|&p| p == CANARY), "row {y}");
        }
    }
    assert!(TextSurface::with_stride(SURFACE_WIDTH - 1).is_none());
}

/// A resize re-lays the transcript out as if it had been typed at the new
/// size, and the surface keeps working afterwards.
#[test]
fn a_resize_equals_typing_at_that_size() {
    for (w, h) in [(900, 520), (320, 200), (1280, 800), (640, 400)] {
        let mut px = vec![CANARY; MAX_W * MAX_H];
        let mut resized = TextSurface::with_stride(MAX_W).unwrap();
        resized.paint_chrome(&mut px);
        typed(&mut resized, &mut px, GREETING.as_bytes());
        typed(&mut resized, &mut px, EXCHANGE);
        resized.resize(&mut px, w, h);
        assert_eq!(resized.size(), (w, h));
        assert_eq!(resized.take_damage(), Some((0, 0, w as u16, h as u16)));
        let mut fresh_px = vec![CANARY; MAX_W * MAX_H];
        let mut fresh = TextSurface::with_stride(MAX_W).unwrap();
        fresh.paint_chrome(&mut fresh_px);
        fresh.resize(&mut fresh_px, w, h);
        typed(&mut fresh, &mut fresh_px, GREETING.as_bytes());
        typed(&mut fresh, &mut fresh_px, EXCHANGE);
        // Only the surface (w x h) is the client's picture: a shrink leaves
        // the old layout in the buffer beyond it, which nothing composes.
        let same =
            |a: &[u32], b: &[u32]| (0..h).all(|y| a[y * MAX_W..][..w] == b[y * MAX_W..][..w]);
        assert!(same(&px, &fresh_px), "{w}x{h}");
        typed(&mut resized, &mut px, b"!\n");
        typed(&mut fresh, &mut fresh_px, b"!\n");
        assert!(same(&px, &fresh_px), "{w}x{h} after more typing");
    }
}

/// The resized layout, from the layout rules alone: ground over the
/// whole w x h, the header where it was (the title still matches the
/// exact-area oracle), the footer rule (accent, then the inactive rule) at
/// h - 36 across [24, w - 24), and nothing written outside w x h.
#[test]
fn a_resized_surface_lays_out_by_its_own_size() {
    use super::showcase_text::{
        ACCENT_WIDTH, AUTHORITY_ACTIVE, GROUND, MARGIN_X, STRUCTURE_INACTIVE, footer_rule_y,
        transcript_rect,
    };
    let (w, h) = (900usize, 520usize);
    let mut px = vec![CANARY; MAX_W * MAX_H];
    let mut text = TextSurface::with_stride(MAX_W).unwrap();
    text.paint_chrome(&mut px);
    text.resize(&mut px, w, h);
    for y in 0..MAX_H {
        for x in 0..MAX_W {
            let p = px[y * MAX_W + x];
            assert_eq!(p == CANARY, x >= w || y >= h, "({x},{y})");
        }
    }
    let rule = footer_rule_y(h) as usize;
    assert_eq!(rule, 484);
    let m = MARGIN_X as usize;
    for x in m..w - m {
        assert!(
            px[rule * MAX_W + x] == STRUCTURE_INACTIVE,
            "footer rule ({x},{rule})"
        );
    }
    assert_eq!(px[rule * MAX_W + m - 1], GROUND);
    assert_eq!(px[rule * MAX_W + w - m], GROUND);
    // The header rule: the accent, then the inactive rule, to w - 24.
    assert_eq!(px[52 * MAX_W + m], AUTHORITY_ACTIVE);
    assert_eq!(
        px[52 * MAX_W + m + ACCENT_WIDTH as usize - 1],
        AUTHORITY_ACTIVE
    );
    assert_eq!(
        px[52 * MAX_W + m + ACCENT_WIDTH as usize],
        STRUCTURE_INACTIVE
    );
    assert_eq!(px[52 * MAX_W + w - m - 1], STRUCTURE_INACTIVE);
    // 520 - 36 - 12 - 64 = 408 px of transcript: 17 lines.
    let t = transcript_rect(w, h);
    assert_eq!((t.width, t.height), (852, 17 * LINE_HEIGHT));
    // The title is where it was, still the oracle's glyphs.
    let (wx, wy, ww, wh) = title_window::WINDOW;
    for y in 0..wh {
        for x in 0..ww {
            let got = px[(wy + y) * MAX_W + wx + x];
            let (lo, hi) = (title_window::LO[y * ww + x], title_window::HI[y * ww + x]);
            for shift in [16, 8, 0] {
                let g = (got >> shift) & 0xFF;
                assert!((lo >> shift) & 0xFF <= g && g <= (hi >> shift) & 0xFF);
            }
        }
    }
}

/// The kept transcript is bounded: past `HISTORY` bytes the oldest whole
/// lines go first, and a resize then replays only what is kept — the same
/// as typing just that tail.
#[test]
fn the_kept_transcript_is_bounded_by_whole_lines() {
    use super::showcase_text::HISTORY;
    let line = b"Arthur> the same forty-byte answer line\n";
    assert_eq!(line.len(), 40);
    let lines = HISTORY / line.len() + 10;
    let mut px = vec![CANARY; MAX_W * MAX_H];
    let mut long = TextSurface::with_stride(MAX_W).unwrap();
    long.paint_chrome(&mut px);
    for _ in 0..lines {
        typed(&mut long, &mut px, line);
    }
    long.resize(&mut px, 700, 450);
    let kept = HISTORY / line.len() * line.len();
    let mut tail_px = vec![CANARY; MAX_W * MAX_H];
    let mut tail = TextSurface::with_stride(MAX_W).unwrap();
    tail.paint_chrome(&mut tail_px);
    tail.resize(&mut tail_px, 700, 450);
    let mut stream = Vec::new();
    for _ in 0..lines {
        stream.extend_from_slice(line);
    }
    // Whatever whole lines survive, they are the newest ones.
    let survived = (kept / line.len()).min(lines);
    typed(
        &mut tail,
        &mut tail_px,
        &stream[stream.len() - survived * line.len()..],
    );
    assert!(px == tail_px);
}

// The footer names the model Arthur loaded, from its path; never a default.

#[test]
fn footer_names_the_model_from_its_path() {
    use super::showcase_logic::model_display_name;
    let name = |p: &str| model_display_name(p);
    assert_eq!(
        name("/boot/models/model-qwen2.5-0.5b.neom").as_deref(),
        Some("Qwen2.5 0.5B")
    );
    assert_eq!(
        name("/neofs/model-qwen2.5-0.5b.neom").as_deref(),
        Some("Qwen2.5 0.5B")
    );
    assert_eq!(
        name("/neofs/model-qwen2.5-1.5b.neom").as_deref(),
        Some("Qwen2.5 1.5B")
    );
    assert_eq!(
        name("/neofs/model-qwen2.5-3b.neom").as_deref(),
        Some("Qwen2.5 3B")
    );
    assert_eq!(
        name("/neofs/model-qwen3.5-0.8b.neom").as_deref(),
        Some("Qwen3.5 0.8B")
    );
    // The fixture and KAT models keep their own words.
    assert_eq!(name("/models/a3.neom").as_deref(), Some("A3"));
    assert_eq!(
        name("/models/kv-persist-kat.neom").as_deref(),
        Some("Kv persist kat")
    );
    assert_eq!(
        name("/models/smollm-135m.neom").as_deref(),
        Some("Smollm 135M")
    );
    // A word that only ends in b or m is not a size.
    assert_eq!(name("/m/web-b.neom").as_deref(), Some("Web b"));
    // Nothing to show: no name, so the footer cannot claim one.
    assert_eq!(name(""), None);
    assert_eq!(name("/models/"), None);
    assert_eq!(name("/models/model-.neom"), None);
}

#[test]
fn footer_keeps_the_name_longest() {
    use super::showcase_logic::{FooterModel, footer_text};
    let named = FooterModel::Named("Qwen2.5 0.5B".into());
    let within = |n: usize| move |t: &str| t.chars().count() <= n;
    assert_eq!(
        footer_text(&named, within(80)),
        "Qwen2.5 0.5B \u{b7} runs on this machine \u{b7} Esc to go back"
    );
    // First "runs on this machine" goes.
    assert_eq!(
        footer_text(&named, within(40)),
        "Qwen2.5 0.5B \u{b7} Esc to go back"
    );
    // Then the name is shortened, never dropped while a letter fits.
    assert_eq!(
        footer_text(&named, within(27)),
        "Qwen2.5... \u{b7} Esc to go back"
    );
    assert_eq!(
        footer_text(&named, within(25)),
        "Qwen2... \u{b7} Esc to go back"
    );
    assert_eq!(
        footer_text(&named, within(21)),
        "Q... \u{b7} Esc to go back"
    );
    assert_eq!(footer_text(&named, within(20)), "Esc to go back");
    assert_eq!(
        footer_text(&FooterModel::Loading, within(80)),
        "loading model \u{b7} Esc to go back"
    );
    assert_eq!(
        footer_text(&FooterModel::NoModel, within(80)),
        "no model loaded \u{b7} Esc to go back"
    );
    assert_eq!(
        footer_text(&FooterModel::NoModel, within(20)),
        "Esc to go back"
    );
    assert_eq!(
        footer_text(&FooterModel::Unnamed, within(80)),
        "model loaded \u{b7} runs on this machine \u{b7} Esc to go back"
    );
    // Shortening counts characters, not bytes.
    let wide = FooterModel::Named("\u{e9}".repeat(8));
    assert_eq!(
        footer_text(&wide, within(22)),
        "\u{e9}\u{e9}... \u{b7} Esc to go back"
    );
}

/// The real footer type at the created width and at Arthur's minimum width
/// (DI-1b): whatever is drawn fits between the margins.
#[test]
fn footer_fits_the_surface_in_the_real_type() {
    use super::showcase_logic::{FooterModel, MIN_WIDTH, footer_text};
    use super::showcase_text::{FOOTER_PX, MARGIN_X};
    use neo_text::Font;
    use neo_text::fonts::PLEX_MONO_REGULAR;
    use neo_text::layout::measure;
    let mono = Font::from_bytes(PLEX_MONO_REGULAR).unwrap();
    let fit = |model: &FooterModel, width: usize| {
        let room = (width as i32 - 2 * MARGIN_X) as f32;
        let text = footer_text(model, |t| measure(&mono, FOOTER_PX, t) <= room);
        assert!(
            measure(&mono, FOOTER_PX, &text) <= room,
            "{text:?} at {width}"
        );
        text
    };
    let qwen = FooterModel::Named("Qwen2.5 0.5B".into());
    assert_eq!(
        fit(&qwen, SURFACE_WIDTH),
        "Qwen2.5 0.5B \u{b7} runs on this machine \u{b7} Esc to go back"
    );
    // At the narrowest window the name stays whole.
    assert_eq!(fit(&qwen, MIN_WIDTH), "Qwen2.5 0.5B \u{b7} Esc to go back");
    let long = FooterModel::Named("Qwen2.5 72B instruct q4 k m long".repeat(3));
    let text = fit(&long, SURFACE_WIDTH);
    assert!(text.starts_with("Qwen2.5 72B") && text.ends_with("... \u{b7} Esc to go back"));
    let text = fit(&long, MIN_WIDTH);
    assert!(text.starts_with("Qwen") && text.ends_with("... \u{b7} Esc to go back"));
}

/// Changing the footer redraws only the band below the footer rule.
#[test]
fn set_footer_redraws_only_the_footer_band() {
    use super::showcase_logic::FooterModel;
    use super::showcase_text::footer_rule_y;
    let (mut text, mut px) = s2_surface();
    let _ = text.take_damage();
    let before = px.clone();
    text.set_footer(&mut px, FooterModel::Named("Qwen2.5 0.5B".into()));
    let rule = footer_rule_y(SURFACE_HEIGHT) as usize;
    let changed: Vec<usize> = (0..PIXELS).filter(|&i| px[i] != before[i]).collect();
    assert!(!changed.is_empty(), "the footer text changed");
    assert!(changed.iter().all(|&i| i / SURFACE_WIDTH > rule));
    assert_eq!(
        text.take_damage(),
        Some((
            0,
            rule as u16 + 1,
            SURFACE_WIDTH as u16,
            (SURFACE_HEIGHT - rule - 1) as u16
        ))
    );
    // Back to the created footer: the created pixels.
    text.set_footer(&mut px, FooterModel::Loading);
    assert!(px == before);
}

/// The footer commits on its own: a typed byte's damage stays for the next
/// commit (the Home flow's intent is on screen when the footer changes).
#[test]
fn footer_damage_is_taken_apart_from_the_transcript() {
    use super::showcase_logic::FooterModel;
    use super::showcase_text::footer_rule_y;
    let (mut text, mut px) = s2_surface();
    let _ = text.take_damage();
    text.write_byte(&mut px, b'x');
    text.set_footer(&mut px, FooterModel::NoModel);
    let rule = footer_rule_y(SURFACE_HEIGHT) as u16;
    assert_eq!(
        text.take_footer_damage(),
        Some((
            0,
            rule + 1,
            SURFACE_WIDTH as u16,
            SURFACE_HEIGHT as u16 - rule - 1
        ))
    );
    assert_eq!(text.take_footer_damage(), None);
    let (_, y, _, h) = text.take_damage().expect("the typed byte's band");
    assert!(
        y + h <= rule,
        "the transcript damage stays above the footer"
    );
}
