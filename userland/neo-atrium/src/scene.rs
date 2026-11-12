// SPDX-License-Identifier: MPL-2.0
//! The WM's base layer: backdrop + selective glass chrome + Cap-Halo cues, the
//! Home view, the Lens and the frame's place. Region blurs are computed at
//! construction and cached; only a move or resize of the frame (DI-1b, DL-1)
//! blurs again, and never per frame.

use alloc::string::String;
use alloc::vec::Vec;
use neo_text::fonts::INTER_REGULAR;
use neo_text::layout::{draw_line, measure};
use neo_text::{Canvas, Font};

use crate::backdrop::Backdrop;
use crate::blur::{Blurred, blur_region};
use crate::layout::{
    CARD_LINE_H, CARD_MAX_ROWS, CHIP_RADIUS, FIELD_RADIUS, GLOW, HALO_ON, HALO_PERIOD, Layout,
    Rect, WINDOW_RADIUS,
};
use crate::material::{ELECTRIC_BLUE, INACTIVE_BLUE, Mode, RT_PANEL, RT_RAISED, TrustState, glass};
use crate::shape::RoundRect;
use crate::wide::{Wide, dither, lighten, mix, widen};

/// Hairline: white at this 8-bit alpha (≈14%).
pub const HAIRLINE_ALPHA: u32 = 36;
/// Extra frost on the Spine chip (a raised tile reads lighter).
pub const CHIP_FROST: u32 = 30;
/// The floor under the client surface (Arthur's own ground, opaque).
pub const CONTENT_FLOOR: u32 = 0x000A_0E27;
/// Length of a Cap-Halo corner tick along each edge, and its thickness.
pub const TICK_LEN: usize = 10;
pub const TICK_W: usize = 2;
/// Peak alpha of the focus light at the frame edge.
pub const GLOW_PEAK: f64 = 110.0;
/// DS-1c: the read-only Cap Halo's dashes lighten toward white by this.
pub const HALO_ALPHA: u32 = 110;

/// Label colours (both modes; contrast-tested in RT, see tests).
pub const LABEL_PRIMARY: u32 = 0x00FF_FFFF;
pub const LABEL_SECONDARY: u32 = 0x00C8_CDD2;

/// DS-1 Home: the intent field's placeholder, sizes and caret.
pub const FIELD_PLACEHOLDER: &str = "Ask Arthur anything";
pub const FIELD_TEXT_PX: f32 = 18.0;
pub const FIELD_PLACEHOLDER_PX: f32 = 16.0;
/// Baseline offset from the field's top edge.
pub const FIELD_BASELINE: usize = 35;
pub const CARET_W: usize = 2;
pub const CARET_H: usize = 24;
/// The caret's top edge, from the field's top edge.
pub const CARET_TOP: usize = 16;

/// `desktop-intro`: a card's rows (primary, this size, on the baseline
/// `card.y + CARD_BASELINE + i·CARD_LINE_H`) and the hint (secondary, this
/// size, centred on the first row's baseline).
pub const CARD_TEXT_PX: f32 = 15.0;
pub const HINT_PX: f32 = 14.0;
pub const CARD_BASELINE: usize = 30;
/// `desktop-clock` (OT-85): the Home readout, both lines centred in
/// `Layout::clock_box`: the time (primary, this size, on `box.y +
/// CLOCK_TIME_BASELINE`) over the date (secondary, on `box.y +
/// CLOCK_DATE_BASELINE`). Text on the backdrop, like the hint: no panel.
pub const CLOCK_TIME_PX: f32 = 44.0;
pub const CLOCK_DATE_PX: f32 = 17.0;
pub const CLOCK_TIME_BASELINE: usize = 46;
pub const CLOCK_DATE_BASELINE: usize = 76;
/// What the base layer shows. `Window` is the S3 scene (Arthur's frame and
/// client floor); `Home` (DS-1) has no window and shows the intent field
/// instead, lit by the focus light while the WM itself holds the keyboard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Window {
        focused: bool,
    },
    /// `focused`: the WM holds the keyboard. `chip_selected`: the keyboard
    /// is on the Spine chip (Tab) rather than the field — the focus light
    /// follows it. `held`: Arthur yielded and is held (the dot under its chip).
    Home {
        focused: bool,
        held: bool,
        chip_selected: bool,
    },
}

impl View {
    /// The plain Home view: field focused, nothing held.
    pub const fn home(focused: bool) -> Self {
        View::Home {
            focused,
            held: false,
            chip_selected: false,
        }
    }
}

/// A2 (`app-chips`): an app chip's look. Not running: the chip alone. In
/// front: the accent dot under it. In the background: a grey dot.
#[cfg(feature = "app-chips")]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChipLook {
    #[default]
    NotRunning,
    Front,
    Background,
}

/// A2: the background dot's grey.
#[cfg(feature = "app-chips")]
const BACKGROUND_DOT: u32 = 0x0080_868C;

pub struct BaseScene {
    pub layout: Layout,
    pub mode: Mode,
    pub backdrop: Backdrop,
    chip_shape: RoundRect,
    frame_shape: RoundRect,
    /// The shapes inset by one pixel: the hairline is the coverage difference.
    chip_inner: RoundRect,
    frame_inner: RoundRect,
    status_shape: RoundRect,
    /// DS-1: the "held" dot under Arthur's chip (a circle).
    held_shape: RoundRect,
    /// Cached blurs, `None` in Reduce-Transparency (nothing is ever blurred).
    blurs: Option<[Blurred; 4]>,
    /// DS-1 Home: the intent field's shape and its cached blur (`None`
    /// unless built [`BaseScene::with_home`] in Glass mode).
    field_shape: RoundRect,
    field_inner: RoundRect,
    field_blur: Option<Blurred>,
    home: bool,
    /// DS-1c: the second chip and the Lens frame, the Lens frame's cached
    /// blur (`None` unless built with Home in Glass mode with room for a
    /// Lens) and whether the Lens is shown.
    chip2_shape: RoundRect,
    chip2_inner: RoundRect,
    lens_shape: RoundRect,
    lens_inner: RoundRect,
    lens_blur: Option<Blurred>,
    lens: bool,
    /// DS-1c: the selection is on the Lens (its chip wears the focus light;
    /// only while the Lens is shown).
    lens_selected: bool,
    /// DS-1c: built able to show a Lens; DI-1b: the window moved since the
    /// Lens frame was last blurred (the next `set_lens(true)` re-blurs it).
    lens_capable: bool,
    lens_blur_stale: bool,
    /// OT-87 R4.4b: the third chip (Install LogansOS), whether it shows, and
    /// whether the selection is on it (its chip wears the focus light while
    /// the keyboard is on the Spine; Home only).
    #[cfg(feature = "install-chip")]
    chip3_shape: RoundRect,
    #[cfg(feature = "install-chip")]
    chip3_inner: RoundRect,
    #[cfg(feature = "install-chip")]
    install: bool,
    /// A2 (`app-chips`): the app chips' shapes, their dots' shapes, each
    /// app's look and whether its name reads "Not yet" (a click on an app
    /// with no module); and whether Arthur runs in the background (another
    /// client in front: his chip gets the grey dot).
    #[cfg(feature = "app-chips")]
    app_shape: [RoundRect; crate::layout::APP_CHIPS],
    #[cfg(feature = "app-chips")]
    app_inner: [RoundRect; crate::layout::APP_CHIPS],
    #[cfg(feature = "app-chips")]
    app_dot_shape: [RoundRect; crate::layout::APP_CHIPS],
    #[cfg(feature = "app-chips")]
    app_look: [ChipLook; crate::layout::APP_CHIPS],
    #[cfg(feature = "app-chips")]
    app_note: [bool; crate::layout::APP_CHIPS],
    #[cfg(feature = "app-chips")]
    arthur_background: bool,
    /// A2: the app chips are on the Spine (off by default, which is today's
    /// Spine bit for bit with the feature compiled in).
    #[cfg(feature = "app-chips")]
    apps_shown: bool,
    #[cfg(feature = "install-chip")]
    install_selected: bool,
    /// `desktop-intro`: the card's rows (empty: the hint shows instead,
    /// while `layout.intro`), its shape and its hairline's inner shape.
    card: Vec<String>,
    card_shape: RoundRect,
    card_inner: RoundRect,
    /// `desktop-clock`: the readout's time and date lines (shown while
    /// `layout.clock`).
    clock_time: String,
    clock_date: String,
}

impl BaseScene {
    /// Build the scene for an output; in [`Mode::Glass`] this runs four region
    /// blurs (bar, Spine, chip, window frame). Later blurs come only from
    /// [`Self::move_frame`] / a resize, or a Lens shown after one.
    pub fn new(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        mode: Mode,
    ) -> Option<Self> {
        Self::build(
            screen_w, screen_h, client_w, client_h, mode, false, false, None,
        )
    }

    /// As [`Self::new`], plus the DS-1 Home view: in Glass mode one more
    /// blur (the intent field), also computed here and never again. No Lens
    /// can be shown (its layout is dropped).
    pub fn with_home(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        mode: Mode,
    ) -> Option<Self> {
        Self::build(
            screen_w, screen_h, client_w, client_h, mode, true, false, None,
        )
    }

    /// DS-1c: as [`Self::with_home`], able to show the Lens: with room for
    /// one, in Glass mode one more blur (the Lens frame), at build.
    pub fn with_lens(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        mode: Mode,
    ) -> Option<Self> {
        Self::build(
            screen_w, screen_h, client_w, client_h, mode, true, true, None,
        )
    }

    /// As [`Self::with_home`] with the window frame placed at `(x, y)`
    /// ([`Layout::placed`]): what a scene moved there must equal.
    pub fn with_home_at(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        mode: Mode,
        frame_at: (usize, usize),
    ) -> Option<Self> {
        Self::build(
            screen_w,
            screen_h,
            client_w,
            client_h,
            mode,
            true,
            false,
            Some(frame_at),
        )
    }

    /// DL-1: as [`Self::with_lens`] with the frame placed at `(x, y)`: what
    /// a scene with a docked Lens moved (or resized) there must equal once
    /// its Lens is shown.
    pub fn with_lens_at(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        mode: Mode,
        frame_at: (usize, usize),
    ) -> Option<Self> {
        Self::build(
            screen_w,
            screen_h,
            client_w,
            client_h,
            mode,
            true,
            true,
            Some(frame_at),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        mode: Mode,
        home: bool,
        lens_capable: bool,
        frame_at: Option<(usize, usize)>,
    ) -> Option<Self> {
        let mut layout = match frame_at {
            None => Layout::new(screen_w, screen_h, client_w, client_h)?,
            Some((x, y)) => Layout::placed(screen_w, screen_h, client_w, client_h, x, y)?,
        };
        if !(home && lens_capable) {
            layout.lens = None;
        }
        let backdrop = Backdrop::new(screen_w, screen_h);
        let shape = round_rect;
        let chip_shape = shape(layout.chip, CHIP_RADIUS);
        let frame_shape = shape(layout.frame, WINDOW_RADIUS);
        let status_shape = shape(layout.status, layout.status.w / 2);
        let held_shape = shape(layout.held_dot, layout.held_dot.w / 2);
        let field_shape = shape(layout.field, FIELD_RADIUS);
        let chip_inner = chip_shape.inset1();
        let frame_inner = frame_shape.inset1();
        let field_inner = field_shape.inset1();
        let chip2_shape = shape(layout.chip2, CHIP_RADIUS);
        let chip2_inner = chip2_shape.inset1();
        #[cfg(feature = "install-chip")]
        let chip3_shape = shape(layout.chip3, CHIP_RADIUS);
        #[cfg(feature = "install-chip")]
        let chip3_inner = chip3_shape.inset1();
        #[cfg(feature = "app-chips")]
        let app_shape = layout.app_chip.map(|r| shape(r, CHIP_RADIUS));
        #[cfg(feature = "app-chips")]
        let app_inner = app_shape.clone().map(|s| s.inset1());
        #[cfg(feature = "app-chips")]
        let app_dot_shape = layout.app_dot.map(|r| shape(r, r.w / 2));
        let lens_frame = layout.lens.map(|l| l.frame).unwrap_or_default();
        let lens_shape = shape(lens_frame, WINDOW_RADIUS);
        let lens_inner = lens_shape.inset1();
        let b = |r: Rect| blur_rect(&backdrop, screen_w, screen_h, r);
        let lens_blur = match (mode, layout.lens) {
            (Mode::Glass, Some(lens)) => Some(b(lens.frame)),
            _ => None,
        };
        let field_blur = match mode {
            Mode::Glass if home => Some(b(layout.field)),
            _ => None,
        };
        let blurs = match mode {
            Mode::ReduceTransparency => None,
            Mode::Glass => Some([
                b(layout.bar),
                b(layout.spine),
                b(layout.chip),
                b(layout.frame),
            ]),
        };
        Some(Self {
            layout,
            mode,
            backdrop,
            chip_shape,
            frame_shape,
            chip_inner,
            frame_inner,
            status_shape,
            held_shape,
            blurs,
            field_shape,
            field_inner,
            field_blur,
            home,
            chip2_shape,
            chip2_inner,
            lens_shape,
            lens_inner,
            lens_blur,
            lens: false,
            lens_selected: false,
            lens_capable: home && lens_capable,
            lens_blur_stale: false,
            #[cfg(feature = "install-chip")]
            chip3_shape,
            #[cfg(feature = "install-chip")]
            chip3_inner,
            #[cfg(feature = "install-chip")]
            install: false,
            #[cfg(feature = "install-chip")]
            install_selected: false,
            #[cfg(feature = "app-chips")]
            app_shape,
            #[cfg(feature = "app-chips")]
            app_inner,
            #[cfg(feature = "app-chips")]
            app_dot_shape,
            #[cfg(feature = "app-chips")]
            app_look: [ChipLook::NotRunning; crate::layout::APP_CHIPS],
            #[cfg(feature = "app-chips")]
            app_note: [false; crate::layout::APP_CHIPS],
            #[cfg(feature = "app-chips")]
            arthur_background: false,
            #[cfg(feature = "app-chips")]
            apps_shown: false,
            card: Vec::new(),
            card_shape: round_rect(Rect::default(), FIELD_RADIUS),
            card_inner: round_rect(Rect::default(), FIELD_RADIUS),
            clock_time: String::new(),
            clock_date: String::new(),
        })
    }

    /// DI-1b: move the window frame to `(x, y)` (clamped, see
    /// [`Layout::clamp_frame`]). Only the frame-derived geometry changes; in
    /// Glass the frame's glass is re-blurred at its new place, the one
    /// region blur a completed move costs (the backdrop it samples does not
    /// move, so nothing else is re-blurred). Returns whether the frame moved.
    pub fn move_frame(&mut self, x: usize, y: usize) -> bool {
        let c = self.layout.client;
        self.place_frame(x, y, c.w, c.h)
    }

    /// DI-1b: place the window frame at `(x, y)` around a `client_w` x
    /// `client_h` client (a move keeps the size; a resize changes it, and may
    /// move the origin when a left or top edge is dragged). As
    /// [`Self::move_frame`]: one region blur in Glass, none in
    /// Reduce-Transparency, none when nothing changes. Returns whether the
    /// frame changed (`false` too when the output cannot hold that client).
    pub fn place_frame(&mut self, x: usize, y: usize, client_w: usize, client_h: usize) -> bool {
        let l = self.layout;
        let Some(mut placed) = Layout::placed(l.screen.w, l.screen.h, client_w, client_h, x, y)
        else {
            return false;
        };
        if placed.frame == l.frame {
            return false;
        }
        // DL-1: a shown Lens stays docked and follows the frame; a place it
        // would not fit beside is refused (the WM clamps the group first, see
        // `Layout::clamp_frame` / `Layout::resized`, so it never asks).
        if self.lens {
            if placed.lens.is_none() {
                return false;
            }
            placed.lens_docked = true;
        }
        // The card slot hangs from the field, which a frame change never
        // moves.
        placed.intro = l.intro;
        self.layout = placed;
        self.frame_shape = round_rect(placed.frame, WINDOW_RADIUS);
        self.frame_inner = self.frame_shape.inset1();
        if let Some(blurs) = self.blurs.as_mut() {
            blurs[FRAME_PANEL] = blur_rect(&self.backdrop, l.screen.w, l.screen.h, placed.frame);
        }
        // DS-1c: the Lens sits beside the frame, so its geometry follows
        // (in a scene that can show one); its glass is re-blurred only when
        // a Lens is next shown (`set_lens`), so a move still costs one blur.
        // DL-1: a shown Lens moves with the frame, so its glass is re-blurred
        // now: a docked move costs two.
        if !self.lens_capable {
            self.layout.lens = None;
        }
        let lens_frame = self.layout.lens.map(|l| l.frame).unwrap_or_default();
        self.lens_shape = round_rect(lens_frame, WINDOW_RADIUS);
        self.lens_inner = self.lens_shape.inset1();
        self.lens_blur_stale = self.lens_capable && self.mode == Mode::Glass;
        if self.lens {
            self.reblur_lens();
        }
        true
    }

    /// The Lens frame's glass, blurred at its current place (Glass only).
    fn reblur_lens(&mut self) {
        self.lens_blur_stale = false;
        if let (Mode::Glass, Some(lens)) = (self.mode, self.layout.lens) {
            let (w, h) = (self.layout.screen.w, self.layout.screen.h);
            self.lens_blur = Some(blur_rect(&self.backdrop, w, h, lens.frame));
        }
    }

    /// DS-1c: show (or end) the Lens: its frame and halo in the window view
    /// and the second Spine chip in both views. Only a scene built with Home
    /// and room for a Lens can show one; `false` otherwise. (DI-1b: a Lens
    /// shown after the window moved re-blurs its glass at its new place.)
    pub fn set_lens(&mut self, on: bool) -> bool {
        if on && (!self.home || self.layout.lens.is_none()) {
            return false;
        }
        if on && self.lens_blur_stale {
            self.reblur_lens();
        }
        self.lens = on;
        self.layout.lens_docked = on;
        if !on {
            self.lens_selected = false;
        }
        true
    }

    pub fn lens_shown(&self) -> bool {
        self.lens
    }

    /// DS-1c: put the selection on the Lens (`true`) or back on Arthur. The
    /// Lens's chip then wears the focus light: in the window view always, in
    /// Home while the keyboard is on the Spine. Only a shown Lens can be
    /// selected; `false` otherwise.
    pub fn set_lens_selected(&mut self, on: bool) -> bool {
        if on && !self.lens {
            return false;
        }
        self.lens_selected = on;
        true
    }

    pub fn lens_selected(&self) -> bool {
        self.lens_selected
    }

    /// A2: put the app chips on the Spine (`true`), or take them off. Off
    /// (the default) is today's Spine, bit for bit.
    #[cfg(feature = "app-chips")]
    pub fn set_app_chips(&mut self, on: bool) {
        self.apps_shown = on;
    }

    /// A2: app chip `app`'s look (`layout::APP_CHIPS` order: Terminal,
    /// Files, Settings). The caller repaints the Spine.
    #[cfg(feature = "app-chips")]
    pub fn set_app_look(&mut self, app: usize, look: ChipLook) {
        if let Some(slot) = self.app_look.get_mut(app) {
            *slot = look;
        }
    }

    #[cfg(feature = "app-chips")]
    pub fn app_look(&self, app: usize) -> ChipLook {
        self.app_look.get(app).copied().unwrap_or_default()
    }

    /// A2: app chip `app`'s name reads "Not yet" (`true`: its app has no
    /// module and was clicked) or its name.
    #[cfg(feature = "app-chips")]
    pub fn set_app_note(&mut self, app: usize, on: bool) {
        if let Some(slot) = self.app_note.get_mut(app) {
            *slot = on;
        }
    }

    /// A2: Arthur runs with another client in front (his chip's grey dot).
    #[cfg(feature = "app-chips")]
    pub fn set_arthur_background(&mut self, on: bool) {
        self.arthur_background = on;
    }

    #[cfg(feature = "app-chips")]
    pub fn arthur_background(&self) -> bool {
        self.arthur_background
    }

    /// A2: the session's client table is gone (back to Home): every app not
    /// running, no note, Arthur not in the background. The caller repaints.
    #[cfg(feature = "app-chips")]
    pub fn reset_app_chips(&mut self) {
        self.app_look = [ChipLook::NotRunning; crate::layout::APP_CHIPS];
        self.app_note = [false; crate::layout::APP_CHIPS];
        self.arthur_background = false;
    }

    /// OT-87 R4.4b: show (or end) the Install chip, the third on the Spine.
    /// Only a scene built with Home has one; `false` otherwise. Off (the
    /// default) leaves every view as without it, bit for bit.
    #[cfg(feature = "install-chip")]
    pub fn set_install(&mut self, on: bool) -> bool {
        if on && !self.home {
            return false;
        }
        self.install = on;
        if !on {
            self.install_selected = false;
        }
        true
    }

    #[cfg(feature = "install-chip")]
    pub fn install_shown(&self) -> bool {
        self.install
    }

    /// OT-87 R4.4b: put the selection on the Install chip (`true`) or off
    /// it. Only a shown chip can be selected; `false` otherwise. The caller
    /// keeps it and the Lens's selection exclusive.
    #[cfg(feature = "install-chip")]
    pub fn set_install_selected(&mut self, on: bool) -> bool {
        if on && !self.install {
            return false;
        }
        self.install_selected = on;
        true
    }

    #[cfg(feature = "install-chip")]
    pub fn install_selected(&self) -> bool {
        self.install_selected
    }

    /// `desktop-intro`: give Home its card slot under the field: the hint
    /// while no card shows. Only a scene built with Home has one; `false`
    /// otherwise. Off (the default) leaves the Home view DS-1's exactly.
    pub fn set_intro(&mut self, on: bool) -> bool {
        if on && !self.home {
            return false;
        }
        self.layout.intro = on;
        if !on {
            self.set_card(Vec::new());
        }
        true
    }

    /// `desktop-intro`: show a card of `rows` (at most `CARD_MAX_ROWS`, the
    /// rest dropped) under the field in the Home view; empty rows end it
    /// (the hint shows again). Nothing shows without [`Self::set_intro`].
    pub fn set_card(&mut self, mut rows: Vec<String>) {
        rows.truncate(CARD_MAX_ROWS);
        let card = self.layout.card(rows.len());
        self.card_shape = round_rect(card, FIELD_RADIUS);
        self.card_inner = self.card_shape.inset1();
        self.card = rows;
    }

    /// `desktop-clock` (OT-85): show the Home readout with `time` over
    /// `date` (e.g. "5:48 PM", "Wednesday, October 21, 2026"), or end it
    /// (`None`). The words are the caller's (libneo::civil); this only draws
    /// them. Only a scene built with Home has a readout; `false` otherwise.
    /// Off (the default) leaves the Home view as without it, bit for bit.
    pub fn set_clock(&mut self, text: Option<(&str, &str)>) -> bool {
        if text.is_some() && !self.home {
            return false;
        }
        self.clock_time.clear();
        self.clock_date.clear();
        if let Some((time, date)) = text {
            self.clock_time.push_str(time);
            self.clock_date.push_str(date);
        }
        self.layout.clock = text.is_some();
        true
    }

    /// The readout's lines (`("", "")` with none).
    pub fn clock_text(&self) -> (&str, &str) {
        (&self.clock_time, &self.clock_date)
    }

    /// The box the readout sits in (masked like the label boxes), empty
    /// without one.
    pub fn clock_text_box(&self) -> Rect {
        self.layout.clock_slot()
    }

    /// How many rows the card shown has (0: none, the hint shows).
    pub fn card_rows(&self) -> usize {
        self.card.len()
    }

    /// The card's rows (empty: none). OT-66 L2: the lock screen borrows the
    /// card and puts these back.
    pub fn card(&self) -> &[String] {
        &self.card
    }

    /// `desktop-intro`: the boxes of a button row's two labels, drawn as one
    /// line (`[ Go ]` then spaces then `[ Back ]`): row `row`'s band, each
    /// from the pen at its label's `[` to the pen after its `]`, measured
    /// with the font and size the row is painted in. A press hit-tests
    /// against these, so it lands on the label drawn; the spaces between
    /// them are in neither. `None` without the row or its two labels.
    pub fn card_buttons(&self, row: usize) -> Option<[Rect; 2]> {
        let text = self.card.get(row)?;
        let go = 0..text.find(']')? + 1;
        let back = text.rfind('[')?..text.len();
        if !text.starts_with('[') || !text.ends_with(']') || back.start < go.end {
            return None;
        }
        let font = Font::from_bytes(INTER_REGULAR)?;
        let card = self.layout.card_text(self.card.len());
        let pen = |at: usize| measure(&font, CARD_TEXT_PX, &text[..at]);
        let span = |r: core::ops::Range<usize>| {
            let (x0, x1) = (libm::floorf(pen(r.start)), libm::ceilf(pen(r.end)));
            Rect::new(
                card.x + x0 as usize,
                card.y + row * CARD_LINE_H,
                (x1 - x0) as usize,
                CARD_LINE_H,
            )
        };
        Some([span(go), span(back)])
    }

    /// The box the intro's text sits in (masked like the label boxes): the
    /// card's text box, the hint's while no card shows, empty without
    /// `desktop-intro`.
    pub fn intro_text_box(&self) -> Rect {
        match (self.layout.intro, self.card.len()) {
            (false, _) => Rect::default(),
            (true, 0) => self.layout.hint(),
            (true, n) => self.layout.card_text(n),
        }
    }

    fn panel(&self, which: usize, x: usize, y: usize, frost: u32) -> Wide {
        let blurred = match (which, &self.blurs) {
            (FIELD, _) => self.field_blur.as_ref(),
            (LENS, _) => self.lens_blur.as_ref(),
            // The second chip is frosted from the Spine's blur.
            (CHIP2, Some(blurs)) => Some(&blurs[1]),
            // OT-87 R4.4b: and the third.
            #[cfg(feature = "install-chip")]
            (CHIP3, Some(blurs)) => Some(&blurs[1]),
            (_, Some(blurs)) => Some(&blurs[which]),
            (_, None) => None,
        };
        match blurred {
            Some(b) => {
                let t = TrustState::Owned.tint();
                glass(b.at(x, y), frost, t.vibrancy, t.vibrancy_color)
            }
            #[cfg(not(feature = "install-chip"))]
            None => widen(if which == CHIP_PANEL || which == FIELD || which == CHIP2 {
                RT_RAISED
            } else {
                RT_PANEL
            }),
            #[cfg(feature = "install-chip")]
            None => widen(if matches!(which, CHIP_PANEL | FIELD | CHIP2 | CHIP3) {
                RT_RAISED
            } else {
                RT_PANEL
            }),
        }
    }

    /// Composite one rounded panel (+ its hairline ring) over `out`.
    #[allow(clippy::too_many_arguments)]
    fn apply_panel(
        &self,
        out: Wide,
        x: usize,
        y: usize,
        which: usize,
        shape: &RoundRect,
        inner: &RoundRect,
        frost: u32,
    ) -> Wide {
        let c = shape.coverage(x, y);
        if c == 0 {
            return out;
        }
        let mut px = mix(self.panel(which, x, y, frost), out, c);
        let ring = c.saturating_sub(inner.coverage(x, y));
        if ring > 0 {
            px = lighten(px, (HAIRLINE_ALPHA * ring + 127) / 255);
        }
        px
    }

    /// The pre-dither (wide) base pixel at `(x, y)` of the S3 window view.
    pub fn wide_pixel(&self, x: usize, y: usize, focused: bool) -> Wide {
        self.view_pixel(x, y, View::Window { focused })
    }

    /// The pre-dither (wide) base pixel at `(x, y)` of `view`.
    pub fn view_pixel(&self, x: usize, y: usize, view: View) -> Wide {
        let l = &self.layout;
        let (window, focused, field_lit, chip_lit, held) = match view {
            View::Window { focused } => (true, focused, false, false, false),
            View::Home {
                focused,
                held,
                chip_selected,
            } => (
                false,
                false,
                focused && !chip_selected,
                focused && chip_selected,
                held,
            ),
        };
        let accent = widen(if focused {
            ELECTRIC_BLUE
        } else {
            INACTIVE_BLUE
        });
        let mut out = self.backdrop.pixel(x, y);
        let owned_frost = TrustState::Owned.tint().frost;

        if l.bar.contains(x, y) {
            out = mix(self.panel(BAR_PANEL, x, y, owned_frost), out, 255);
            if y + 1 == l.bar.bottom() {
                out = lighten(out, HAIRLINE_ALPHA);
            }
        }
        if l.spine.contains(x, y) {
            out = mix(self.panel(SPINE_PANEL, x, y, owned_frost), out, 255);
            if x + 1 == l.spine.right() {
                out = lighten(out, HAIRLINE_ALPHA);
            }
        }
        // DS-1c: the selection on the Lens moves the chip light to its chip
        // (in the window view as well); both glows go under both panels.
        let lens_lit = self.lens && self.lens_selected && (window || chip_lit);
        let (lit, lit_glow) = if lens_lit {
            (l.chip2, l.chip2_glow)
        } else {
            (l.chip, l.chip_glow)
        };
        // OT-87 R4.4b: the Install chip takes the light in Home only.
        #[cfg(feature = "install-chip")]
        let (lit, lit_glow) = if self.install && self.install_selected && chip_lit {
            (l.chip3, l.chip3_glow)
        } else {
            (lit, lit_glow)
        };
        if (chip_lit || lens_lit) && lit_glow.contains(x, y) {
            out = glow_mix(out, &lit, CHIP_RADIUS, x, y);
        }
        if l.chip.contains(x, y) {
            out = self.apply_panel(
                out,
                x,
                y,
                CHIP_PANEL,
                &self.chip_shape,
                &self.chip_inner,
                CHIP_FROST,
            );
        }
        if held && l.held_dot.contains(x, y) {
            let c = self.held_shape.coverage(x, y);
            out = mix(widen(ELECTRIC_BLUE), out, c);
        }
        #[cfg(feature = "app-chips")]
        if self.apps_shown && !held && self.arthur_background && l.held_dot.contains(x, y) {
            let c = self.held_shape.coverage(x, y);
            out = mix(widen(BACKGROUND_DOT), out, c);
        }
        #[cfg(feature = "app-chips")]
        for i in 0..crate::layout::APP_CHIPS {
            if !self.apps_shown {
                break;
            }
            if l.app_chip[i].contains(x, y) {
                out = self.apply_panel(
                    out,
                    x,
                    y,
                    CHIP2,
                    &self.app_shape[i],
                    &self.app_inner[i],
                    CHIP_FROST,
                );
            }
            let dot = match self.app_look[i] {
                ChipLook::NotRunning => None,
                ChipLook::Front => Some(ELECTRIC_BLUE),
                ChipLook::Background => Some(BACKGROUND_DOT),
            };
            if let Some(colour) = dot
                && l.app_dot[i].contains(x, y)
            {
                let c = self.app_dot_shape[i].coverage(x, y);
                out = mix(widen(colour), out, c);
            }
        }
        if self.lens && l.chip2.contains(x, y) {
            out = self.apply_panel(
                out,
                x,
                y,
                CHIP2,
                &self.chip2_shape,
                &self.chip2_inner,
                CHIP_FROST,
            );
        }
        #[cfg(feature = "install-chip")]
        if self.install && l.chip3.contains(x, y) {
            out = self.apply_panel(
                out,
                x,
                y,
                CHIP3,
                &self.chip3_shape,
                &self.chip3_inner,
                CHIP_FROST,
            );
        }
        if window && l.frame.contains(x, y) {
            out = self.apply_panel(
                out,
                x,
                y,
                FRAME_PANEL,
                &self.frame_shape,
                &self.frame_inner,
                owned_frost,
            );
            if self.on_tick(x, y) {
                out = widen(ELECTRIC_BLUE);
            }
        }
        let lens = if window && self.lens { l.lens } else { None };
        if let Some(lens) = lens {
            if lens.frame.contains(x, y) {
                out = self.apply_panel(
                    out,
                    x,
                    y,
                    LENS,
                    &self.lens_shape,
                    &self.lens_inner,
                    owned_frost,
                );
            }
            if on_halo(&lens.halo, x, y) {
                out = lighten(out, HALO_ALPHA);
            }
        }
        if l.marker.contains(x, y) {
            out = accent;
        }
        if l.status.contains(x, y) {
            let c = self.status_shape.coverage(x, y);
            out = mix(widen(ELECTRIC_BLUE), out, c);
        }
        if window && l.client.contains(x, y) {
            out = widen(CONTENT_FLOOR);
        }
        if lens.is_some_and(|lens| lens.client.contains(x, y)) {
            out = widen(CONTENT_FLOOR);
        }
        if !window && l.field_glow.contains(x, y) {
            if field_lit {
                out = glow_mix(out, &l.field, FIELD_RADIUS, x, y);
            }
            if l.field.contains(x, y) {
                out = self.apply_panel(
                    out,
                    x,
                    y,
                    FIELD,
                    &self.field_shape,
                    &self.field_inner,
                    owned_frost,
                );
            }
        }
        // `desktop-intro`: the card, an opaque panel on Arthur's own ground
        // (no glass, so no blur), with the full hairline ring.
        if !window && !self.card.is_empty() && self.layout.intro {
            let c = self.card_shape.coverage(x, y);
            if c > 0 {
                out = mix(widen(CONTENT_FLOOR), out, c);
                let ring = c.saturating_sub(self.card_inner.coverage(x, y));
                if ring > 0 {
                    out = lighten(out, (HAIRLINE_ALPHA * ring + 127) / 255);
                }
            }
        }
        out
    }

    /// Cap-Halo "owned" geometry: an L of Electric Blue hugging each rounded
    /// corner of the window frame (on the edge rows/columns, after the arc).
    fn on_tick(&self, x: usize, y: usize) -> bool {
        let f = &self.layout.frame;
        let r = WINDOW_RADIUS;
        let (dx, dy) = (x - f.x, y - f.y);
        let (rx, ry) = (f.right() - 1 - x, f.bottom() - 1 - y);
        let near_x = dx.min(rx);
        let near_y = dy.min(ry);
        let along = |d: usize| d >= r && d < r + TICK_LEN;
        (near_y < TICK_W && along(near_x)) || (near_x < TICK_W && along(near_y))
    }

    /// Paint `rect` of the base layer (8-bit) into `dst` (`stride` pixels),
    /// including any label clipped to it. A full repaint passes the screen.
    pub fn paint_rect(&self, dst: &mut [u32], stride: usize, focused: bool, rect: Rect) {
        let r = rect.intersect(&self.layout.screen);
        for y in r.y..r.bottom() {
            let row = y * stride;
            for x in r.x..r.right() {
                dst[row + x] = dither(self.wide_pixel(x, y, focused), x, y);
            }
        }
        self.paint_labels(dst, stride, focused, r, None);
    }

    pub fn paint(&self, dst: &mut [u32], stride: usize, focused: bool) {
        self.paint_rect(dst, stride, focused, self.layout.screen);
    }

    /// Paint `rect` of the Home view (DS-1): the base without a window, the
    /// intent field holding `text` (its placeholder when empty) and, while
    /// the WM holds the keyboard (`focused`), the focus light and caret.
    pub fn paint_home_rect(
        &self,
        dst: &mut [u32],
        stride: usize,
        focused: bool,
        text: &str,
        rect: Rect,
    ) {
        self.paint_home_view(dst, stride, View::home(focused), text, rect);
    }

    /// As [`Self::paint_home_rect`] for any Home `view` (held / chip
    /// selected). The caret shows only while the field has the keyboard.
    pub fn paint_home_view(
        &self,
        dst: &mut [u32],
        stride: usize,
        view: View,
        text: &str,
        rect: Rect,
    ) {
        let View::Home {
            focused,
            chip_selected,
            ..
        } = view
        else {
            return;
        };
        let focused = focused && !chip_selected;
        let r = rect.intersect(&self.layout.screen);
        for y in r.y..r.bottom() {
            let row = y * stride;
            for x in r.x..r.right() {
                dst[row + x] = dither(self.view_pixel(x, y, view), x, y);
            }
        }
        self.paint_labels(dst, stride, false, r, Some((text, focused)));
    }

    /// `home`: `None` for the window view, else the intent field's text and
    /// whether the WM holds the keyboard.
    fn paint_labels(
        &self,
        dst: &mut [u32],
        stride: usize,
        focused: bool,
        clip: Rect,
        home: Option<(&str, bool)>,
    ) {
        let l = &self.layout;
        let boxes = if home.is_some() {
            l.home_label_boxes()
        } else {
            l.label_boxes()
        };
        let lens_title = match (self.lens, home, l.lens) {
            (true, None, Some(lens)) => lens.title_label,
            _ => Rect::default(),
        };
        let chip2_label = if self.lens {
            l.chip2_label
        } else {
            Rect::default()
        };
        #[cfg(feature = "install-chip")]
        let chip3_label = if self.install {
            l.chip3_label
        } else {
            Rect::default()
        };
        let intro_box = if home.is_some() {
            self.intro_text_box()
        } else {
            Rect::default()
        };
        let clock_box = if home.is_some() {
            self.clock_text_box()
        } else {
            Rect::default()
        };
        #[cfg(feature = "app-chips")]
        let app_labels = if self.apps_shown {
            l.app_label
        } else {
            [Rect::default(); crate::layout::APP_CHIPS]
        };
        // (Without `app-chips`, these two checks are exactly today's: the
        // feature's boxes are chained on only when it is compiled in.)
        #[cfg(all(not(feature = "install-chip"), not(feature = "app-chips")))]
        if boxes
            .iter()
            .chain([lens_title, chip2_label, intro_box, clock_box].iter())
            .all(|b| b.intersect(&clip).is_empty())
        {
            return;
        }
        #[cfg(all(feature = "install-chip", not(feature = "app-chips")))]
        if boxes
            .iter()
            .chain([lens_title, chip2_label, chip3_label, intro_box, clock_box].iter())
            .all(|b| b.intersect(&clip).is_empty())
        {
            return;
        }
        #[cfg(all(not(feature = "install-chip"), feature = "app-chips"))]
        if boxes
            .iter()
            .chain([lens_title, chip2_label, intro_box, clock_box].iter())
            .chain(app_labels.iter())
            .all(|b| b.intersect(&clip).is_empty())
        {
            return;
        }
        #[cfg(all(feature = "install-chip", feature = "app-chips"))]
        if boxes
            .iter()
            .chain([lens_title, chip2_label, chip3_label, intro_box, clock_box].iter())
            .chain(app_labels.iter())
            .all(|b| b.intersect(&clip).is_empty())
        {
            return;
        }
        // `app-icons`: each chip's icon, blended under its name (below), in
        // place of the letter; clipped to the chip's label box.
        #[cfg(feature = "app-icons")]
        {
            crate::icons::paint(
                dst,
                stride,
                l.chip,
                l.chip_label.intersect(&clip),
                crate::icons::ARTHUR,
            );
            if self.lens {
                crate::icons::paint(
                    dst,
                    stride,
                    l.chip2,
                    chip2_label.intersect(&clip),
                    crate::icons::LENS,
                );
            }
            #[cfg(feature = "install-chip")]
            if self.install {
                crate::icons::paint(
                    dst,
                    stride,
                    l.chip3,
                    chip3_label.intersect(&clip),
                    crate::icons::INSTALL,
                );
            }
            #[cfg(feature = "app-chips")]
            for (i, blob) in crate::icons::APPS.iter().enumerate() {
                crate::icons::paint(
                    dst,
                    stride,
                    l.app_chip[i],
                    app_labels[i].intersect(&clip),
                    blob,
                );
            }
        }
        let Some(mut font) = Font::from_bytes(INTER_REGULAR) else {
            return;
        };
        let (sw, sh) = (l.screen.w, l.screen.h);
        let Some(mut canvas) = Canvas::new(dst, sw, sh, stride) else {
            return;
        };
        let to = |b: Rect| {
            let c = b.intersect(&clip);
            neo_text::Rect::new(c.x as i32, c.y as i32, c.w as i32, c.h as i32)
        };
        let bar_text = if focused { "Arthur" } else { "Atrium" };
        let bar_clip = to(l.bar_label);
        if !bar_clip.is_empty() {
            draw_line(
                &mut canvas,
                &mut font,
                13.0,
                bar_text,
                32.0,
                23,
                LABEL_PRIMARY,
                bar_clip,
            );
        }
        if let Some((text, home_focused)) = home {
            self.paint_field_text(&mut canvas, &mut font, text, home_focused, to(l.field_text));
        }
        let title_clip = if home.is_some() {
            neo_text::Rect::new(0, 0, 0, 0)
        } else {
            to(l.title_label)
        };
        if !title_clip.is_empty() {
            let owned = "\u{b7} owned";
            title_text(
                &mut canvas,
                &mut font,
                l.title_label,
                "Arthur",
                owned,
                title_clip,
            );
        }
        let chip_clip = to(l.chip_label);
        if !chip_clip.is_empty() {
            chip_text(&mut canvas, &mut font, l.chip, "A", "Arthur", chip_clip);
        }
        let chip2_clip = to(chip2_label);
        if !chip2_clip.is_empty() {
            chip_text(&mut canvas, &mut font, l.chip2, "L", "Lens", chip2_clip);
        }
        #[cfg(feature = "install-chip")]
        {
            let chip3_clip = to(chip3_label);
            if !chip3_clip.is_empty() {
                chip_text(&mut canvas, &mut font, l.chip3, "I", "Install", chip3_clip);
            }
        }
        #[cfg(feature = "app-chips")]
        for (i, (letter, name)) in [("T", "Terminal"), ("F", "Files"), ("S", "Settings")]
            .into_iter()
            .enumerate()
        {
            let clip_i = to(app_labels[i]);
            if !clip_i.is_empty() {
                let name = if self.app_note[i] { "Not yet" } else { name };
                chip_text(&mut canvas, &mut font, l.app_chip[i], letter, name, clip_i);
            }
        }
        let intro_clip = to(intro_box);
        if !intro_clip.is_empty() {
            self.paint_intro_text(&mut canvas, &mut font, intro_clip);
        }
        let clock_clip = to(clock_box);
        if !clock_clip.is_empty() {
            self.paint_clock_text(&mut canvas, &mut font, clock_clip);
        }
        let lens_clip = to(lens_title);
        if !lens_clip.is_empty() {
            let read_only = "\u{b7} read-only";
            title_text(
                &mut canvas,
                &mut font,
                lens_title,
                "Lens",
                read_only,
                lens_clip,
            );
        }
    }

    /// The intent field's text (or placeholder) and caret, clipped to `clip`
    /// (already inside the field's text box). Text that overflows the box
    /// scrolls left so its end and the caret stay visible.
    fn paint_field_text(
        &self,
        canvas: &mut Canvas<'_>,
        font: &mut Font<'_>,
        text: &str,
        focused: bool,
        clip: neo_text::Rect,
    ) {
        if clip.is_empty() {
            return;
        }
        let l = &self.layout;
        let x0 = l.field_text.x as f32;
        let base = (l.field.y + FIELD_BASELINE) as i32;
        let room = l.field_text.w.saturating_sub(CARET_W + 2) as f32;
        let mut caret_x = l.field_text.x;
        if text.is_empty() {
            draw_line(
                canvas,
                font,
                FIELD_PLACEHOLDER_PX,
                FIELD_PLACEHOLDER,
                x0,
                base,
                LABEL_SECONDARY,
                clip,
            );
        } else {
            let w = measure(font, FIELD_TEXT_PX, text);
            let start = if w > room { x0 + room - w } else { x0 };
            draw_line(
                canvas,
                font,
                FIELD_TEXT_PX,
                text,
                start,
                base,
                LABEL_PRIMARY,
                clip,
            );
            caret_x = (start + w + 1.0) as usize;
        }
        if focused {
            let caret = Rect::new(caret_x, l.field.y + CARET_TOP, CARET_W, CARET_H);
            let caret = neo_text::Rect::new(
                caret.x as i32,
                caret.y as i32,
                caret.w as i32,
                caret.h as i32,
            );
            canvas.fill(caret.intersect(&clip), ELECTRIC_BLUE);
        }
    }

    /// `desktop-intro`: the card's rows, or the hint centred in its box,
    /// clipped to `clip` (already inside the text box).
    fn paint_intro_text(&self, canvas: &mut Canvas<'_>, font: &mut Font<'_>, clip: neo_text::Rect) {
        let l = &self.layout;
        let card = l.card(self.card.len());
        let base = |i: usize| (card.y + CARD_BASELINE + i * CARD_LINE_H) as i32;
        if self.card.is_empty() {
            let hint = l.hint();
            let w = measure(font, HINT_PX, crate::intro::HINT);
            let x = hint.x as f32 + (hint.w as f32 - w) / 2.0;
            draw_line(
                canvas,
                font,
                HINT_PX,
                crate::intro::HINT,
                x,
                base(0),
                LABEL_SECONDARY,
                clip,
            );
            return;
        }
        let x = l.card_text(self.card.len()).x as f32;
        for (i, row) in self.card.iter().enumerate() {
            draw_line(
                canvas,
                font,
                CARD_TEXT_PX,
                row,
                x,
                base(i),
                LABEL_PRIMARY,
                clip,
            );
        }
    }

    /// `desktop-clock`: the readout's two lines, each centred in its box,
    /// clipped to `clip` (already inside the box).
    fn paint_clock_text(&self, canvas: &mut Canvas<'_>, font: &mut Font<'_>, clip: neo_text::Rect) {
        let b = self.layout.clock_box();
        for (text, px, baseline, color) in [
            (
                &self.clock_time,
                CLOCK_TIME_PX,
                CLOCK_TIME_BASELINE,
                LABEL_PRIMARY,
            ),
            (
                &self.clock_date,
                CLOCK_DATE_PX,
                CLOCK_DATE_BASELINE,
                LABEL_SECONDARY,
            ),
        ] {
            let w = measure(font, px, text);
            let x = b.x as f32 + (b.w as f32 - w) / 2.0;
            draw_line(
                canvas,
                font,
                px,
                text,
                x,
                (b.y + baseline) as i32,
                color,
                clip,
            );
        }
    }

    /// Fill the focus-light plane (`layout.glow`, tightly packed).
    pub fn fill_glow(&self, dst: &mut [u32], focused: bool) {
        self.fill_glow_strided(dst, self.layout.glow.w, focused);
    }

    /// As [`Self::fill_glow`] into a plane `stride` pixels wide (DI-1b: the
    /// WM keeps one plane for the largest window a resize can make).
    pub fn fill_glow_strided(&self, dst: &mut [u32], stride: usize, focused: bool) {
        let g = self.layout.glow;
        for y in 0..g.h {
            for x in 0..g.w {
                dst[y * stride + x] = glow_pixel(&self.layout, focused, g.x + x, g.y + y);
            }
        }
    }
}

/// The focus light at scene pixel `(x, y)`: Electric Blue, straight alpha
/// `floor(GLOW_PEAK·t² + 0.5)` with `t = (GLOW − d)/GLOW`, `d` the Euclidean
/// distance from the pixel centre to the rounded window frame (outside only).
pub fn glow_pixel(layout: &Layout, focused: bool, x: usize, y: usize) -> u32 {
    if !focused {
        return 0;
    }
    let a = rounded_glow_alpha(&layout.frame, WINDOW_RADIUS, x, y);
    if a == 0 { 0 } else { (a << 24) | ELECTRIC_BLUE }
}

/// The panels' indices (into the cached blurs, for the first four).
const BAR_PANEL: usize = 0;
const SPINE_PANEL: usize = 1;
const CHIP_PANEL: usize = 2;
const FRAME_PANEL: usize = 3;
/// Which cached blur is the intent field's.
const FIELD: usize = 4;
/// DS-1c: the Lens frame's cached blur, and the second chip (frosted from
/// the Spine's blur).
const LENS: usize = 5;
const CHIP2: usize = 6;
/// OT-87 R4.4b: the third chip (Install), frosted from the Spine's blur.
#[cfg(feature = "install-chip")]
const CHIP3: usize = 7;

/// A frame's title in its label box `title` (the window's "Arthur · owned",
/// the Lens's "Lens · read-only"): the name at 14 px, the cue at 11 px
/// 8 px after it, both on the baseline `title.y + 18`.
fn title_text(
    canvas: &mut Canvas<'_>,
    font: &mut Font<'_>,
    title: Rect,
    name: &str,
    cue: &str,
    clip: neo_text::Rect,
) {
    let x = (title.x + 4) as f32;
    let base = (title.y + 18) as i32;
    draw_line(canvas, font, 14.0, name, x, base, LABEL_PRIMARY, clip);
    let w = measure(font, 14.0, name);
    draw_line(
        canvas,
        font,
        11.0,
        cue,
        x + w + 8.0,
        base,
        LABEL_SECONDARY,
        clip,
    );
}

/// DS-1c: a Spine chip's monogram and name, centred on `chip`.
#[cfg(not(feature = "app-icons"))]
fn chip_text(
    canvas: &mut Canvas<'_>,
    font: &mut Font<'_>,
    chip: Rect,
    mono: &str,
    name: &str,
    clip: neo_text::Rect,
) {
    let cx = chip.x as f32 + chip.w as f32 / 2.0;
    let mono_w = measure(font, 20.0, mono);
    draw_line(
        canvas,
        font,
        20.0,
        mono,
        cx - mono_w / 2.0,
        (chip.y + 30) as i32,
        LABEL_PRIMARY,
        clip,
    );
    let name_w = measure(font, 10.0, name);
    draw_line(
        canvas,
        font,
        10.0,
        name,
        cx - name_w / 2.0,
        (chip.y + 47) as i32,
        LABEL_SECONDARY,
        clip,
    );
}

/// `app-icons`: the chip's name alone, below its icon (the icon itself is
/// blended before the label canvas is made: `crate::icons::paint`).
#[cfg(feature = "app-icons")]
fn chip_text(
    canvas: &mut Canvas<'_>,
    font: &mut Font<'_>,
    chip: Rect,
    _mono: &str,
    name: &str,
    clip: neo_text::Rect,
) {
    let cx = chip.x as f32 + chip.w as f32 / 2.0;
    let name_w = measure(font, 10.0, name);
    draw_line(
        canvas,
        font,
        10.0,
        name,
        cx - name_w / 2.0,
        (chip.y + crate::icons::NAME_BASELINE) as i32,
        LABEL_SECONDARY,
        clip,
    );
}

/// DS-1c: whether `(x, y)` is a dash of the read-only Cap Halo: on the
/// ring's top or bottom row when `(x − h.x) mod 10 < 6`, else on its left
/// or right column when `(y − h.y) mod 10 < 6`.
fn on_halo(h: &Rect, x: usize, y: usize) -> bool {
    if !h.contains(x, y) {
        return false;
    }
    if y == h.y || y + 1 == h.bottom() {
        (x - h.x) % HALO_PERIOD < HALO_ON
    } else if x == h.x || x + 1 == h.right() {
        (y - h.y) % HALO_PERIOD < HALO_ON
    } else {
        false
    }
}

/// The focus light law around `f` (radius `radius`) mixed into the wide
/// pixel `out` (DS-1: before the panel it lights).
fn glow_mix(out: Wide, f: &Rect, radius: usize, x: usize, y: usize) -> Wide {
    match rounded_glow_alpha(f, radius, x, y) {
        0 => out,
        a => mix(widen(ELECTRIC_BLUE), out, a),
    }
}

/// The focus-light alpha at `(x, y)` around the rounded rect `f` (radius
/// `radius`): `floor(GLOW_PEAK·t² + 0.5)`, `t = (GLOW − d)/GLOW`, `d` the
/// Euclidean distance from the pixel centre to the shape (outside only).
fn rounded_glow_alpha(f: &Rect, radius: usize, x: usize, y: usize) -> u32 {
    let r = radius as f64;
    let hw = f.w as f64 / 2.0;
    let hh = f.h as f64 / 2.0;
    let px = x as f64 + 0.5 - (f.x as f64 + hw);
    let py = y as f64 + 0.5 - (f.y as f64 + hh);
    let qx = (libm::fabs(px) - (hw - r)).max(0.0);
    let qy = (libm::fabs(py) - (hh - r)).max(0.0);
    let d = libm::sqrt(qx * qx + qy * qy) - r;
    let g = GLOW as f64;
    if d <= 0.0 || d > g {
        return 0;
    }
    let t = (g - d) / g;
    libm::floor(GLOW_PEAK * t * t + 0.5) as u32
}

/// The glass blur of `r`, sampled from the backdrop (spec "Blur").
fn blur_rect(backdrop: &Backdrop, screen_w: usize, screen_h: usize, r: Rect) -> Blurred {
    blur_region(
        |x: usize, y: usize| backdrop.pixel(x, y),
        screen_w,
        screen_h,
        (r.x, r.y, r.w, r.h),
    )
}

fn round_rect(r: Rect, radius: usize) -> RoundRect {
    RoundRect::new(r.x, r.y, r.w, r.h, radius)
}

/// OT-88 S1 (neo-init's `atrium-settings-store`): Reduce Transparency, set
/// from the settings store, applied live.
impl BaseScene {
    /// Switch the material. To Glass, every glass region is blurred where it
    /// is now: the four panels, the intent field in a Home scene, and the
    /// Lens frame in a scene that can show one (what a fresh build makes, for
    /// the current layout). To Reduce Transparency, the blurs are dropped.
    /// What the scene paints afterwards equals a scene built in `mode` with
    /// the same layout and state. Returns whether the mode changed.
    pub fn set_mode(&mut self, mode: Mode) -> bool {
        if self.mode == mode {
            return false;
        }
        let l = self.layout;
        let (w, h) = (l.screen.w, l.screen.h);
        let b = |r: Rect| blur_rect(&self.backdrop, w, h, r);
        let (blurs, field_blur, lens_blur) = match mode {
            Mode::ReduceTransparency => (None, None, None),
            Mode::Glass => (
                Some([b(l.bar), b(l.spine), b(l.chip), b(l.frame)]),
                self.home.then(|| b(l.field)),
                l.lens.map(|lens| b(lens.frame)),
            ),
        };
        self.mode = mode;
        self.blurs = blurs;
        self.field_blur = field_blur;
        self.lens_blur = lens_blur;
        self.lens_blur_stale = false;
        true
    }
}
