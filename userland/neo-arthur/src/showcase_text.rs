// SPDX-License-Identifier: MPL-2.0
//! Arthur's TEXT surface rendered with the neo-text engine (Desktop S2):
//! proportional, anti-aliased, mixed-case Inter for Arthur's voice and
//! IBM Plex Mono for the `neo>` rail, on the design-language tokens
//! (docs/THINK-2026-07-10-atrium-design-language.md §6).
//!
//! Pure: renders into a borrowed XRGB8888 slice so the whole surface is
//! host-testable (tools/arthur-logic-tests). The syscall side lives in
//! showcase_surface.rs.

#![cfg_attr(
    any(not(feature = "arthur-compositor"), feature = "arthur-legacy-5x7-font"),
    allow(dead_code)
)]

extern crate alloc;

use alloc::vec::Vec;
use neo_text::fonts::{INTER_REGULAR, PLEX_MONO_REGULAR};
use neo_text::layout::{draw_line, measure};
use neo_text::{Canvas, FlowStyle, Font, Rect, TextFlow};

use crate::showcase_logic::{FooterModel, SURFACE_HEIGHT, SURFACE_WIDTH, footer_text};

// Semantic colour tokens (design language §6.2).
/// `ground` (Void).
pub const GROUND: u32 = 0x000A_0E27;
/// `text-primary` (White): 14.9:1 on ground, AAA for body copy.
pub const TEXT_PRIMARY: u32 = 0x00FF_FFFF;
/// `structure` (Silver): secondary text and chrome, 8.4:1 on ground.
pub const STRUCTURE: u32 = 0x00A8_ADB2;
/// `structure-inactive`: a low-value derivative of authority blue.
pub const STRUCTURE_INACTIVE: u32 = 0x001E_3558;
/// `authority-active` (Electric Blue): used as light (an accent), never body copy.
pub const AUTHORITY_ACTIVE: u32 = 0x003C_8EF4;

pub const MARGIN_X: i32 = 24;
pub const TITLE: &str = "Arthur";
pub const TITLE_PX: f32 = 22.0;
pub const TITLE_BASELINE: i32 = 36;
pub const SUBTITLE: &str = "on-device assistant";
pub const SUBTITLE_PX: f32 = 14.0;
pub const SUBTITLE_GAP: f32 = 10.0;
pub const HEADER_RULE_Y: i32 = 52;
pub const ACCENT_WIDTH: i32 = 40;

pub const BODY_PX: f32 = 16.0;
pub const LINE_HEIGHT: i32 = 24;
pub const BODY_BASELINE: i32 = 18;
pub const TRANSCRIPT_TOP: i32 = 64;
/// A9-D: the busy emblem's rect: this far right of the caret, this wide,
/// two lines high (the WM centres its 44 px emblem in it).
pub const BUSY_GAP: i32 = 4;
pub const BUSY_WIDTH: i32 = 48;
/// The transcript at the created 640x400 size: twelve lines.
pub const TRANSCRIPT: Rect = transcript_rect(SURFACE_WIDTH, SURFACE_HEIGHT);

pub const FOOTER_RULE_Y: i32 = footer_rule_y(SURFACE_HEIGHT);
pub const FOOTER_PX: f32 = 12.0;
pub const FOOTER_BASELINE: i32 = footer_baseline(SURFACE_HEIGHT);
/// DI-1b resize: the footer sits this far above the bottom edge, its rule
/// and baseline below the transcript's last line.
const FOOTER_RULE_FROM_BOTTOM: i32 = 36;
const FOOTER_BASELINE_FROM_BOTTOM: i32 = 14;
/// The gap between the transcript's last line and the footer rule.
const TRANSCRIPT_FOOTER_GAP: i32 = 12;

// The created size lays out exactly as before resize existed (S2).
const _: () = assert!(TRANSCRIPT.height == 12 * LINE_HEIGHT && TRANSCRIPT.width == 592);
const _: () = assert!(FOOTER_RULE_Y == 364 && FOOTER_BASELINE == 386);

/// DI-1b resize: the transcript for a `width` x `height` surface: as many
/// whole lines as fit between the header and the footer.
pub const fn transcript_rect(width: usize, height: usize) -> Rect {
    let room = footer_rule_y(height) - TRANSCRIPT_FOOTER_GAP - TRANSCRIPT_TOP;
    let lines = if room > 0 { room / LINE_HEIGHT } else { 0 };
    Rect::new(
        MARGIN_X,
        TRANSCRIPT_TOP,
        width as i32 - 2 * MARGIN_X,
        lines * LINE_HEIGHT,
    )
}

pub const fn footer_rule_y(height: usize) -> i32 {
    height as i32 - FOOTER_RULE_FROM_BOTTOM
}

pub const fn footer_baseline(height: usize) -> i32 {
    height as i32 - FOOTER_BASELINE_FROM_BOTTOM
}

/// DI-1b resize: the transcript bytes kept for a re-layout (the newest; the
/// oldest whole lines are dropped first).
pub const HISTORY: usize = 4096;

pub const GREETING: &str = "Arthur is ready. Ask me anything.\n\nYou> ";

pub struct TextSurface<'f> {
    sans: Font<'f>,
    mono: Font<'f>,
    flow: TextFlow,
    damage: Rect,
    /// The surface's size, and the row stride of the pixels it renders into
    /// (DI-1b: a resizable surface lives in a buffer sized for the largest).
    width: usize,
    height: usize,
    stride: usize,
    /// The transcript since the chrome, replayed by [`Self::resize`]. On
    /// the heap: Arthur's surface is built on a small user stack.
    history: Vec<u8>,
    /// What the footer says about the model ([`Self::set_footer`]), and the
    /// band its last change damaged, kept apart from the transcript's so it
    /// can be committed on its own ([`Self::take_footer_damage`]).
    footer: FooterModel,
    footer_damage: Rect,
    /// A9-C: the transcript line the view's top row shows while the user has
    /// scrolled up; `None` while it follows the newest text.
    #[cfg(feature = "arthur-scrollback")]
    view_top: Option<u64>,
    /// Text arrived while scrolled up; [`Self::settle`] redraws the view.
    #[cfg(feature = "arthur-scrollback")]
    view_stale: bool,
}

/// A9-C: how many finished transcript lines Arthur keeps to scroll back
/// through.
#[cfg(feature = "arthur-scrollback")]
pub const SCROLLBACK_LINES: usize = 512;

/// The transcript's flow: with its scrollback under `arthur-scrollback`,
/// exactly today's flow without it.
fn new_flow(region: Rect) -> TextFlow {
    #[cfg(feature = "arthur-scrollback")]
    return TextFlow::with_scrollback(region, STYLE, SCROLLBACK_LINES);
    #[cfg(not(feature = "arthur-scrollback"))]
    TextFlow::new(region, STYLE)
}

const STYLE: FlowStyle = FlowStyle {
    px: BODY_PX,
    line_height: LINE_HEIGHT,
    baseline: BODY_BASELINE,
    fg: TEXT_PRIMARY,
    bg: GROUND,
};

impl TextSurface<'static> {
    /// The created 640x400 surface in a tightly packed 640x400 mapping.
    #[allow(dead_code)] // the host KATs' reference surface
    pub fn new() -> Option<Self> {
        Self::with_stride(SURFACE_WIDTH)
    }

    /// The created 640x400 surface in a mapping `stride` pixels wide.
    pub fn with_stride(stride: usize) -> Option<Self> {
        if stride < SURFACE_WIDTH {
            return None;
        }
        Some(Self {
            sans: Font::from_bytes(INTER_REGULAR)?,
            mono: Font::from_bytes(PLEX_MONO_REGULAR)?,
            flow: new_flow(TRANSCRIPT),
            damage: Rect::new(0, 0, 0, 0),
            width: SURFACE_WIDTH,
            height: SURFACE_HEIGHT,
            stride,
            history: Vec::with_capacity(HISTORY),
            footer: FooterModel::Loading,
            footer_damage: Rect::new(0, 0, 0, 0),
            #[cfg(feature = "arthur-scrollback")]
            view_top: None,
            #[cfg(feature = "arthur-scrollback")]
            view_stale: false,
        })
    }
}

impl<'f> TextSurface<'f> {
    #[allow(dead_code)] // host KATs
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Ground, header (title, subtitle, accented rule), footer rail, empty
    /// transcript. Damages the whole surface; forgets the transcript.
    pub fn paint_chrome(&mut self, pixels: &mut [u32]) {
        self.history.clear();
        #[cfg(feature = "arthur-scrollback")]
        {
            self.view_top = None;
        }
        self.paint_frame(pixels);
    }

    /// DI-1b resize: lay the surface out again at `width` x `height` (within
    /// the mapping: the caller bounds it) and replay the kept transcript into
    /// it, as if it had been typed at that size. Damages the whole surface.
    #[cfg_attr(not(feature = "desktop-resize"), allow(dead_code))]
    pub fn resize(&mut self, pixels: &mut [u32], width: usize, height: usize) {
        self.width = width.min(self.stride);
        self.height = height;
        self.flow = new_flow(transcript_rect(self.width, height));
        #[cfg(feature = "arthur-scrollback")]
        {
            self.view_top = None;
        }
        self.paint_frame(pixels);
        let mut canvas = canvas(pixels, self.width, self.height, self.stride);
        for &byte in &self.history {
            self.flow.push(&mut self.sans, &mut canvas, as_char(byte));
        }
        let _ = self.flow.take_damage();
    }

    fn paint_frame(&mut self, pixels: &mut [u32]) {
        let (width, height) = (self.width, self.height);
        let mut canvas = canvas(pixels, width, height, self.stride);
        let full = canvas.bounds();
        canvas.fill(full, GROUND);
        draw_line(
            &mut canvas,
            &mut self.sans,
            TITLE_PX,
            TITLE,
            MARGIN_X as f32,
            TITLE_BASELINE,
            TEXT_PRIMARY,
            full,
        );
        let subtitle_x = MARGIN_X as f32 + measure(&self.sans, TITLE_PX, TITLE) + SUBTITLE_GAP;
        draw_line(
            &mut canvas,
            &mut self.sans,
            SUBTITLE_PX,
            SUBTITLE,
            subtitle_x,
            TITLE_BASELINE,
            STRUCTURE,
            full,
        );
        let rule_width = width as i32 - 2 * MARGIN_X;
        canvas.fill(
            Rect::new(MARGIN_X, HEADER_RULE_Y, rule_width, 1),
            STRUCTURE_INACTIVE,
        );
        canvas.fill(
            Rect::new(MARGIN_X, HEADER_RULE_Y, ACCENT_WIDTH, 1),
            AUTHORITY_ACTIVE,
        );
        canvas.fill(
            Rect::new(MARGIN_X, footer_rule_y(height), rule_width, 1),
            STRUCTURE_INACTIVE,
        );
        self.draw_footer(&mut canvas);
        self.flow.clear(&mut canvas);
        let _ = self.flow.take_damage();
        self.damage = full;
    }

    /// Say `model` in the footer: the band below the footer rule is redrawn
    /// and damaged; the transcript is untouched.
    pub fn set_footer(&mut self, pixels: &mut [u32], model: FooterModel) {
        self.footer = model;
        let mut canvas = canvas(pixels, self.width, self.height, self.stride);
        let rule = footer_rule_y(self.height) + 1;
        let band = Rect::new(0, rule, self.width as i32, self.height as i32 - rule);
        canvas.fill(band, GROUND);
        self.draw_footer(&mut canvas);
        self.footer_damage = self.footer_damage.union(&band);
    }

    /// The footer's damage alone, as (x, y, w, h); the transcript's stays
    /// for the next [`Self::take_damage`].
    #[cfg_attr(not(feature = "desktop-session"), allow(dead_code))]
    pub fn take_footer_damage(&mut self) -> Option<(u16, u16, u16, u16)> {
        let damage = core::mem::replace(&mut self.footer_damage, Rect::new(0, 0, 0, 0));
        as_tuple(damage.intersect(&Rect::new(0, 0, self.width as i32, self.height as i32)))
    }

    /// The footer line, as long as fits between the margins.
    fn draw_footer(&mut self, canvas: &mut Canvas<'_>) {
        let room = (self.width as i32 - 2 * MARGIN_X) as f32;
        let mono = &self.mono;
        let text = footer_text(&self.footer, |t| measure(mono, FOOTER_PX, t) <= room);
        let full = canvas.bounds();
        draw_line(
            canvas,
            &mut self.mono,
            FOOTER_PX,
            &text,
            MARGIN_X as f32,
            footer_baseline(self.height),
            STRUCTURE,
            full,
        );
    }

    /// One byte of transcript, with the legacy grid's byte semantics: `\r`
    /// ignored, `\t` = four spaces, `\x08`/`\x7f` erase, printable ASCII drawn,
    /// anything else drawn as `?`.
    pub fn write_byte(&mut self, pixels: &mut [u32], byte: u8) {
        self.remember(byte);
        let mut canvas = canvas(pixels, self.width, self.height, self.stride);
        self.flow.push(&mut self.sans, &mut canvas, as_char(byte));
        // A9-C: the flow drew at the bottom; the view it covered is redrawn
        // before the next commit. A held view whose top line the 512-line
        // cap just let go moves to the oldest kept line, so the stored top
        // is always a line that exists and the next scroll counts from the
        // line on screen (Chad, #100 RETURN item 1).
        #[cfg(feature = "arthur-scrollback")]
        {
            self.view_stale |= self.view_top.is_some();
            let first = self.flow.first_kept();
            if let Some(top) = self.view_top.as_mut() {
                *top = (*top).max(first);
            }
        }
    }

    /// A9-C: scroll the view by `key` (a page is the transcript's rows less
    /// one, so a line stays in sight). It stops at the oldest kept line and,
    /// at the newest, follows new text again. Returns whether the view moved
    /// (and so was redrawn and damaged).
    #[cfg(feature = "arthur-scrollback")]
    pub fn scroll(&mut self, pixels: &mut [u32], key: crate::showcase_logic::ScrollKey) -> bool {
        use crate::showcase_logic::ScrollKey;
        let page = (self.flow.rows() - 1).max(1) as i64;
        let delta = match key {
            ScrollKey::PageUp => -page,
            ScrollKey::PageDown => page,
            ScrollKey::LineUp => -1,
            ScrollKey::LineDown => 1,
            ScrollKey::Wheel(notches) => -i64::from(notches) * crate::showcase_logic::WHEEL_LINES,
        };
        let bottom = self.flow.bottom_top();
        let from = self.view_top.unwrap_or(bottom);
        let to = (from as i64 + delta).clamp(self.flow.first_kept() as i64, bottom as i64) as u64;
        if to == from {
            return false;
        }
        self.view_top = (to < bottom).then_some(to);
        self.view_stale = false;
        let mut canvas = canvas(pixels, self.width, self.height, self.stride);
        self.flow.render_view(&mut self.sans, &mut canvas, to);
        true
    }

    /// A9-C: the line the view's top row shows while scrolled up.
    #[cfg(feature = "arthur-scrollback")]
    #[allow(dead_code)] // the boot-test witness and the host KATs
    pub fn view_top(&self) -> Option<u64> {
        self.view_top
    }

    /// A9-C: follow new text again (the user typed). Redraws the newest view
    /// if the user had scrolled up.
    #[cfg(feature = "arthur-scrollback")]
    pub fn follow(&mut self, pixels: &mut [u32]) {
        if self.view_top.take().is_some() {
            self.view_stale = false;
            let bottom = self.flow.bottom_top();
            let mut canvas = canvas(pixels, self.width, self.height, self.stride);
            self.flow.render_view(&mut self.sans, &mut canvas, bottom);
        }
    }

    /// A9-C: before a commit, put back the view the user scrolled to if text
    /// written since drew over it (the flow always writes at the bottom).
    #[cfg(feature = "arthur-scrollback")]
    pub fn settle(&mut self, pixels: &mut [u32]) {
        if let (true, Some(top)) = (self.view_stale, self.view_top) {
            self.view_stale = false;
            let mut canvas = canvas(pixels, self.width, self.height, self.stride);
            self.flow.render_view(&mut self.sans, &mut canvas, top);
        }
    }

    /// Keep `byte` for a re-layout; when full, drop the oldest whole line
    /// (or, with no line break kept, the oldest half).
    fn remember(&mut self, byte: u8) {
        if self.history.len() == HISTORY {
            let cut = self
                .history
                .iter()
                .position(|&b| b == b'\n')
                .map_or(HISTORY / 2, |i| i + 1);
            self.history.drain(..cut);
        }
        self.history.push(byte);
    }

    /// A9-D: the rect for the WM's busy emblem, as (x, y, w, h): from just
    /// right of the caret, `BUSY_WIDTH` wide, over the current line and the
    /// one below it (made empty first, scrolling the transcript up a line if
    /// the caret is on the last row). `None` if nothing of it is on the
    /// surface. With the scrollback, the newest view comes back first: the
    /// rect is beside the caret, and only that view has the caret on it.
    #[cfg_attr(not(feature = "arthur-busy-emblem"), allow(dead_code))]
    pub fn reserve_busy(&mut self, pixels: &mut [u32]) -> Option<(u16, u16, u16, u16)> {
        #[cfg(feature = "arthur-scrollback")]
        self.follow(pixels);
        let mut canvas = canvas(pixels, self.width, self.height, self.stride);
        self.flow.make_room(&mut canvas, 1);
        let (x, y) = self.flow.caret(&self.sans);
        let rect = Rect::new(x + BUSY_GAP, y, BUSY_WIDTH, 2 * LINE_HEIGHT);
        as_tuple(rect.intersect(&Rect::new(0, 0, self.width as i32, self.height as i32)))
    }

    /// Union of everything drawn since the last call, as (x, y, w, h).
    pub fn take_damage(&mut self) -> Option<(u16, u16, u16, u16)> {
        let mut damage = core::mem::replace(&mut self.damage, Rect::new(0, 0, 0, 0));
        if let Some(flow) = self.flow.take_damage() {
            damage = damage.union(&flow);
        }
        let footer = core::mem::replace(&mut self.footer_damage, Rect::new(0, 0, 0, 0));
        damage = damage.union(&footer);
        as_tuple(damage.intersect(&Rect::new(0, 0, self.width as i32, self.height as i32)))
    }
}

/// The legacy grid's byte semantics: `\r` ignored, `\t` = four spaces,
/// `\x08`/`\x7f` erase, printable ASCII drawn, anything else drawn as `?`.
fn as_char(byte: u8) -> char {
    match byte {
        b'\r' | b'\n' | b'\t' | 0x08 | 0x7f | 0x20..=0x7e => char::from(byte),
        _ => '?',
    }
}

/// A damage rect as (x, y, w, h); `None` when empty.
fn as_tuple(damage: Rect) -> Option<(u16, u16, u16, u16)> {
    (!damage.is_empty()).then_some((
        damage.x as u16,
        damage.y as u16,
        damage.width as u16,
        damage.height as u16,
    ))
}

fn canvas(pixels: &mut [u32], width: usize, height: usize, stride: usize) -> Canvas<'_> {
    Canvas::new(pixels, width, height, stride)
        .expect("Arthur's surface mapping holds width x height pixels at this stride")
}
