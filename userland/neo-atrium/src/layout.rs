// SPDX-License-Identifier: MPL-2.0
//! The desktop layout: craft bar, Spine (with Arthur's chip and, DS-1c, the
//! Lens chip), one WM-placed window around a client surface, and the Lens
//! docked to its right. Integer geometry from the output size and the frame's
//! place only; the client never learns any of it.

/// A half-open pixel rectangle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(&self) -> usize {
        self.x + self.w
    }

    pub const fn bottom(&self) -> usize {
        self.y + self.h
    }

    pub const fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        if x1 <= x0 || y1 <= y0 {
            Rect::default()
        } else {
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
    }

    /// `self` grown by `n` on every side (the caller keeps it on screen).
    pub const fn grow(&self, n: usize) -> Rect {
        Rect::new(self.x - n, self.y - n, self.w + 2 * n, self.h + 2 * n)
    }

    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        Rect::new(
            x0,
            y0,
            self.right().max(o.right()) - x0,
            self.bottom().max(o.bottom()) - y0,
        )
    }
}

pub const BAR_H: usize = 36;
pub const SPINE_W: usize = 84;
pub const PAD: usize = 8;
pub const TITLE_H: usize = 34;
pub const WINDOW_RADIUS: usize = 12;
pub const CHIP_INSET: usize = 14;
pub const CHIP: usize = 56;
pub const CHIP_RADIUS: usize = 10;
/// The focus light reaches this far outside the window frame.
pub const GLOW: usize = 14;
const MIN_MARGIN: usize = 16;
/// DS-1 Home: the intent field (a glass pill in the workfield).
pub const FIELD_MAX_W: usize = 720;
pub const FIELD_H: usize = 56;
pub const FIELD_RADIUS: usize = 14;
/// The typed text's inset inside the field.
pub const FIELD_TEXT_INSET: usize = 22;
/// `desktop-intro`: the card slot under the field (the intro, the help
/// card, or the one-row hint), this far below the field's focus light.
pub const CARD_GAP: usize = 18;
/// A card row's height, and the card's padding above and below its rows.
pub const CARD_LINE_H: usize = 24;
pub const CARD_PAD_Y: usize = 10;
/// The most rows a card holds (the slot a view switch repaints).
pub const CARD_MAX_ROWS: usize = 8;
/// `desktop-clock` (OT-85): the Home readout's box, centred over the field,
/// `CLOCK_GAP` above its focus light: the time on one line and the date
/// under it (`CLOCK_H` tall in all).
pub const CLOCK_GAP: usize = 28;
pub const CLOCK_H: usize = 84;
/// DI-1b resize: the client size bounds (the WM also keeps the frame and
/// its focus light inside the workfield, which can bound it further).
pub const MIN_CLIENT_W: usize = 320;
pub const MIN_CLIENT_H: usize = 200;
pub const MAX_CLIENT_W: usize = 1280;
pub const MAX_CLIENT_H: usize = 800;
/// The top edge's resize band: the top four rows of the title band (the
/// rest of the band moves the window).
pub const RESIZE_TOP: usize = 4;
/// How far from a corner, along either edge, a press takes the corner.
pub const RESIZE_CORNER: usize = 16;

/// DI-1b: the frame edges a resize drag moves (bit set).
pub mod edge {
    pub const LEFT: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const TOP: u8 = 4;
    pub const BOTTOM: u8 = 8;
}

/// DS-1c: the Lens instrument's client size. DL-1: fixed (DS-1c took
/// Arthur's height, the same 400 until a resize could change it); the Lens's
/// surface never resizes.
pub const LENS_W: usize = 360;
pub const LENS_H: usize = 400;
/// The gap between Arthur's frame and the Lens frame.
pub const LENS_GAP: usize = 32;
/// The read-only Cap Halo: a dashed ring this far outside the Lens frame,
/// dashes `HALO_ON` on / `HALO_PERIOD − HALO_ON` off.
pub const HALO_GAP: usize = 4;
pub const HALO_ON: usize = 6;
pub const HALO_PERIOD: usize = 10;
/// The second Spine chip sits this far below the first.
pub const CHIP2_GAP: usize = 22;

/// DS-1c: the Lens instrument beside Arthur's window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LensLayout {
    pub frame: Rect,
    pub client: Rect,
    /// The read-only Cap Halo's ring (its 1-px border is the dashed line).
    pub halo: Rect,
    pub title_label: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub screen: Rect,
    pub bar: Rect,
    pub spine: Rect,
    pub chip: Rect,
    /// The focus marker beside the chip.
    pub marker: Rect,
    /// The status light in the craft bar (a circle).
    pub status: Rect,
    pub workfield: Rect,
    pub frame: Rect,
    pub client: Rect,
    pub glow: Rect,
    /// Fixed boxes the Inter labels are drawn inside (and masked from the
    /// digest oracle).
    pub bar_label: Rect,
    pub title_label: Rect,
    pub chip_label: Rect,
    /// DS-1 Home: the intent field, its text box (labels + caret, masked
    /// from the digest oracle like the other label boxes) and its focus
    /// light's reach.
    pub field: Rect,
    pub field_text: Rect,
    pub field_glow: Rect,
    /// DS-1: the chip's focus light's reach (when the Spine has the
    /// keyboard) and the "held" dot under the chip.
    pub chip_glow: Rect,
    pub held_dot: Rect,
    /// DS-1c: the second Spine chip (the Lens), its label box and its
    /// focus light's reach (as `chip_glow`).
    pub chip2: Rect,
    pub chip2_label: Rect,
    pub chip2_glow: Rect,
    /// OT-87 R4.4b: the third Spine chip (Install LogansOS), below the
    /// Lens's at the same gap; its label box and focus light's reach.
    #[cfg(feature = "install-chip")]
    pub chip3: Rect,
    #[cfg(feature = "install-chip")]
    pub chip3_label: Rect,
    #[cfg(feature = "install-chip")]
    pub chip3_glow: Rect,
    /// DS-1c: `None` when the output has no room for the Lens beside the
    /// window where it is (DL-1: a bloom first slides the window to
    /// [`Self::bloom_origin`]).
    pub lens: Option<LensLayout>,
    /// DL-1: the Lens is shown, docked to the frame's right: a move or a
    /// resize keeps the whole group (frame, gap, Lens) inside the workfield,
    /// and the Lens follows the frame's top-right corner.
    pub lens_docked: bool,
    /// `desktop-intro`: Home has the card slot under the field (the hint or
    /// a card). Off: no slot, and the Home view is DS-1's exactly.
    pub intro: bool,
    /// `desktop-clock` (OT-85): Home shows the date-and-time readout above
    /// the field. Off: no readout, and the Home view is as without it.
    pub clock: bool,
}

impl Layout {
    /// `None` when the output cannot hold the S3 chrome around the client;
    /// the glass WM then refuses the session ("output too small", exit 3).
    /// The window frame is centred in the workfield.
    pub fn new(screen_w: usize, screen_h: usize, client_w: usize, client_h: usize) -> Option<Self> {
        let (frame_w, frame_h) = frame_size(client_w, client_h);
        if !fits(screen_w, screen_h, frame_w, frame_h) {
            return None;
        }
        let (wx, wy) = (SPINE_W, BAR_H);
        let (ww, wh) = (screen_w - SPINE_W, screen_h - BAR_H);
        let x = wx + (ww - frame_w) / 2;
        let y = wy + (wh - frame_h) / 2;
        Self::placed(screen_w, screen_h, client_w, client_h, x, y)
    }

    /// DI-1b: as [`Self::new`] with the window frame's top-left at `(x, y)`,
    /// clamped by [`Self::clamp_frame`] so the frame and its focus light stay
    /// inside the workfield. Everything else is independent of the frame.
    pub fn placed(
        screen_w: usize,
        screen_h: usize,
        client_w: usize,
        client_h: usize,
        x: usize,
        y: usize,
    ) -> Option<Self> {
        let (frame_w, frame_h) = frame_size(client_w, client_h);
        if !fits(screen_w, screen_h, frame_w, frame_h) {
            return None;
        }
        let screen = Rect::new(0, 0, screen_w, screen_h);
        let bar = Rect::new(0, 0, screen_w, BAR_H);
        let spine = Rect::new(0, BAR_H, SPINE_W, screen_h - BAR_H);
        let chip = Rect::new(CHIP_INSET, BAR_H + CHIP_INSET, CHIP, CHIP);
        let marker = Rect::new(4, chip.y + 12, 3, 32);
        let status = Rect::new(16, 14, 8, 8);
        let workfield = Rect::new(SPINE_W, BAR_H, screen_w - SPINE_W, screen_h - BAR_H);
        let (x, y) = clamp_origin(&workfield, frame_w, frame_h, x as i64, y as i64);
        let frame = Rect::new(x, y, frame_w, frame_h);
        let client = Rect::new(frame.x + PAD, frame.y + TITLE_H, client_w, client_h);
        let glow = frame.grow(GLOW);
        let field_w = FIELD_MAX_W.min(workfield.w - 2 * (GLOW + MIN_MARGIN));
        let field = Rect::new(
            workfield.x + (workfield.w - field_w) / 2,
            workfield.y + (workfield.h - FIELD_H) * 38 / 100,
            field_w,
            FIELD_H,
        );
        let field_text = Rect::new(
            field.x + FIELD_TEXT_INSET,
            field.y + 8,
            field.w - 2 * FIELD_TEXT_INSET,
            field.h - 16,
        );
        let field_glow = field.grow(GLOW);
        let chip2 = Rect::new(chip.x, chip.bottom() + CHIP2_GAP, CHIP, CHIP);
        #[cfg(feature = "install-chip")]
        let chip3 = Rect::new(chip.x, chip2.bottom() + CHIP2_GAP, CHIP, CHIP);
        Some(Self {
            screen,
            bar,
            spine,
            chip,
            marker,
            status,
            workfield,
            frame,
            client,
            glow,
            bar_label: Rect::new(30, 6, 170, 24),
            title_label: Rect::new(frame.x + 14, frame.y + 4, 240, 26),
            chip_label: chip_label_of(chip),
            field,
            field_text,
            field_glow,
            chip_glow: chip.grow(GLOW),
            held_dot: Rect::new(chip.x + (CHIP - 8) / 2, chip.bottom() + 6, 8, 8),
            chip2,
            chip2_label: chip_label_of(chip2),
            chip2_glow: chip2.grow(GLOW),
            #[cfg(feature = "install-chip")]
            chip3,
            #[cfg(feature = "install-chip")]
            chip3_label: chip_label_of(chip3),
            #[cfg(feature = "install-chip")]
            chip3_glow: chip3.grow(GLOW),
            lens: Self::lens(screen_w, screen_h, frame),
            lens_docked: false,
            intro: false,
            clock: false,
        })
    }

    /// DI-1b: where a frame dragged to `(x, y)` lands: kept whole inside the
    /// workfield with its focus light (`GLOW`) on screen.
    /// DL-1: with the Lens docked, the group (frame, `LENS_GAP`, Lens frame)
    /// is what stays inside, with the Lens's right margin.
    pub fn clamp_frame(&self, x: i64, y: i64) -> (usize, usize) {
        let (gw, gh) = self.group_size(self.frame.w, self.frame.h);
        clamp_origin(&self.workfield, gw, gh, x, y)
    }

    /// DL-1: as [`Self::clamp_frame`] for a frame around a `client_w` x
    /// `client_h` client (a configure acked at a new size).
    pub fn clamp_frame_for(
        &self,
        client_w: usize,
        client_h: usize,
        x: i64,
        y: i64,
    ) -> (usize, usize) {
        let (fw, fh) = frame_size(client_w, client_h);
        let (gw, gh) = self.group_size(fw, fh);
        clamp_origin(&self.workfield, gw, gh, x, y)
    }

    /// DL-1: the outer size a frame of `frame_w` x `frame_h` occupies for
    /// clamping: itself, or with the Lens docked the whole group (the Lens's
    /// right margin, `MIN_MARGIN`, included in the width).
    fn group_size(&self, frame_w: usize, frame_h: usize) -> (usize, usize) {
        if self.lens_docked && self.lens.is_some() {
            docked_group(frame_w, frame_h)
        } else {
            (frame_w, frame_h)
        }
    }

    /// DL-1: where the frame must be for a Lens to bloom beside it: where it
    /// is when there is room, else the nearest origin (the group clamped as
    /// in [`Self::clamp_frame`]). `None` when the output cannot hold the group
    /// at all (the bloom is refused, as DS-1c).
    pub fn bloom_origin(&self) -> Option<(usize, usize)> {
        let (gw, gh) = docked_group(self.frame.w, self.frame.h);
        let wf = self.workfield;
        if wf.w < gw + 2 * GLOW || wf.h < gh + 2 * GLOW {
            return None;
        }
        Some(clamp_origin(
            &wf,
            gw,
            gh,
            self.frame.x as i64,
            self.frame.y as i64,
        ))
    }

    /// DI-1b: the title band, the part of the frame above the client that
    /// a press drags.
    pub fn title_band(&self) -> Rect {
        Rect::new(self.frame.x, self.frame.y, self.frame.w, TITLE_H)
    }

    /// DI-1b: the resize edges ([`edge`] bits) under `(x, y)`: the frame's
    /// side and bottom borders (`PAD` wide, outside the client), the top
    /// `RESIZE_TOP` rows of the title band, and within `RESIZE_CORNER` of a
    /// corner along either edge, both edges. `0` off the handles (the client,
    /// the rest of the title band, or outside the frame).
    pub fn resize_edges(&self, x: usize, y: usize) -> u8 {
        let f = self.frame;
        if !f.contains(x, y) {
            return 0;
        }
        let near_left = x < f.x + RESIZE_CORNER;
        let near_right = x >= f.right() - RESIZE_CORNER;
        let near_top = y < f.y + RESIZE_CORNER;
        let near_bottom = y >= f.bottom() - RESIZE_CORNER;
        let mut edges = 0;
        if x < f.x + PAD {
            edges |= edge::LEFT;
        }
        if x >= f.right() - PAD {
            edges |= edge::RIGHT;
        }
        if y < f.y + RESIZE_TOP {
            edges |= edge::TOP;
        }
        if y >= f.bottom() - PAD {
            edges |= edge::BOTTOM;
        }
        if edges & (edge::TOP | edge::BOTTOM) != 0 {
            if near_left {
                edges |= edge::LEFT;
            } else if near_right {
                edges |= edge::RIGHT;
            }
        }
        if edges & (edge::LEFT | edge::RIGHT) != 0 {
            if near_top {
                edges |= edge::TOP;
            } else if near_bottom {
                edges |= edge::BOTTOM;
            }
        }
        edges
    }

    /// DI-1b: the largest client this output can hold, within
    /// `MAX_CLIENT_W` x `MAX_CLIENT_H` (the frame must still [`fits`]).
    pub fn max_client(&self) -> (usize, usize) {
        let pad = FIT_PAD;
        let w = (self.screen.w - SPINE_W - pad).saturating_sub(2 * PAD);
        let h = (self.screen.h - BAR_H - pad).saturating_sub(TITLE_H + PAD);
        (w.min(MAX_CLIENT_W), h.min(MAX_CLIENT_H))
    }

    /// DL-1 fix-forward (Janus DL1-X1): the largest client a docked group
    /// holds on this output: the frame, `LENS_GAP`, the Lens frame and its
    /// right margin inside the workfield with the focus light (as
    /// [`Self::bloom_origin`] and a docked [`Self::resized`] require), within
    /// [`Self::max_client`].
    pub fn docked_max_client(&self) -> (usize, usize) {
        let (w, h) = self.max_client();
        let wf = self.workfield;
        let frame_w =
            wf.w.saturating_sub(2 * GLOW + LENS_GAP + lens_frame_w() + MIN_MARGIN);
        let frame_h = wf.h.saturating_sub(2 * GLOW);
        (
            w.min(frame_w.saturating_sub(2 * PAD)),
            h.min(frame_h.saturating_sub(TITLE_H + PAD)),
        )
    }

    /// DI-1b: where a resize drag of `edges` by `(dx, dy)` leaves the
    /// frame: `(x, y, client_w, client_h)`. Each dragged edge moves by its
    /// delta, stopped at the workfield (frame plus focus light stay inside)
    /// and by the client bounds (`MIN_CLIENT_*` up to [`Self::max_client`]);
    /// the opposite edge never moves. `edges == 0` is the frame as it is.
    pub fn resized(&self, edges: u8, dx: i64, dy: i64) -> (usize, usize, usize, usize) {
        let f = self.frame;
        let wf = self.workfield;
        let (max_w, max_h) = self.max_client();
        let (min_fw, max_fw) = (
            (MIN_CLIENT_W.min(max_w) + 2 * PAD) as i64,
            (max_w + 2 * PAD) as i64,
        );
        let (min_fh, max_fh) = (
            (MIN_CLIENT_H.min(max_h) + TITLE_H + PAD) as i64,
            (max_h + TITLE_H + PAD) as i64,
        );
        let (lo_x, mut hi_x) = ((wf.x + GLOW) as i64, (wf.right() - GLOW) as i64);
        let (lo_y, hi_y) = ((wf.y + GLOW) as i64, (wf.bottom() - GLOW) as i64);
        // DL-1: a docked Lens follows the right edge and the top: the right
        // edge stops where the Lens still fits, the top where the Lens's
        // bottom does (the current frame satisfies both, so the ranges below
        // stay non-empty).
        let mut hi_t = i64::MAX;
        if self.lens_docked && self.lens.is_some() {
            hi_x = hi_x
                .min(wf.right() as i64 - (GLOW + MIN_MARGIN + LENS_GAP + lens_frame_w()) as i64);
            hi_t = (wf.bottom() - GLOW - lens_frame_h()) as i64;
        }
        let (mut l, mut r) = (f.x as i64, f.right() as i64);
        let (mut t, mut b) = (f.y as i64, f.bottom() as i64);
        if edges & edge::LEFT != 0 {
            l = (l + dx).clamp((r - max_fw).max(lo_x), r - min_fw);
        } else if edges & edge::RIGHT != 0 {
            r = (r + dx).clamp(l + min_fw, (l + max_fw).min(hi_x));
        }
        if edges & edge::TOP != 0 {
            t = (t + dy).clamp((b - max_fh).max(lo_y), (b - min_fh).min(hi_t));
        } else if edges & edge::BOTTOM != 0 {
            b = (b + dy).clamp(t + min_fh, (t + max_fh).min(hi_y));
        }
        (
            l as usize,
            t as usize,
            (r - l) as usize - 2 * PAD,
            (b - t) as usize - TITLE_H - PAD,
        )
    }

    /// The Lens frame: top-aligned with the window, `LENS_GAP` to its right,
    /// around a fixed `LENS_W` x `LENS_H` client (DS-1c: as tall as the
    /// window's 400-row client; DL-1: a resized window leaves it so).
    fn lens(screen_w: usize, screen_h: usize, frame: Rect) -> Option<LensLayout> {
        let lf = Rect::new(
            frame.right() + LENS_GAP,
            frame.y,
            lens_frame_w(),
            lens_frame_h(),
        );
        if lf.right() + GLOW + MIN_MARGIN > screen_w || lf.bottom() + GLOW > screen_h {
            return None;
        }
        Some(LensLayout {
            frame: lf,
            client: Rect::new(lf.x + PAD, lf.y + TITLE_H, LENS_W, LENS_H),
            halo: lf.grow(HALO_GAP),
            title_label: Rect::new(lf.x + 14, lf.y + 4, 240, 26),
        })
    }

    /// DL-1: what a move or resize of the frame repaints for a docked Lens
    /// (the halo's reach, where it was and where it is); empty otherwise.
    pub fn docked_reach(&self) -> Rect {
        match self.lens {
            Some(lens) if self.lens_docked => lens.halo,
            _ => Rect::default(),
        }
    }

    /// DS-1c: what blooming (or ending) the Lens repaints: its halo's reach
    /// (frame and ring) and the second chip with its focus light's reach (a
    /// Lens may end selected). Empty without room for a Lens.
    pub fn lens_regions(&self) -> [Rect; 2] {
        match self.lens {
            Some(lens) => [lens.halo, self.chip2_glow],
            None => [Rect::default(); 2],
        }
    }

    /// `desktop-intro`: a card of `rows` rows under the field: the field's
    /// width, `CARD_GAP` below its focus light.
    pub fn card(&self, rows: usize) -> Rect {
        let f = self.field;
        Rect::new(
            f.x,
            f.bottom() + GLOW + CARD_GAP,
            f.w,
            rows * CARD_LINE_H + 2 * CARD_PAD_Y,
        )
    }

    /// The text box of a `rows`-row card (masked from the digest oracle
    /// like the label boxes): inset like the field's text.
    pub fn card_text(&self, rows: usize) -> Rect {
        let c = self.card(rows);
        Rect::new(
            c.x + FIELD_TEXT_INSET,
            c.y + CARD_PAD_Y,
            c.w - 2 * FIELD_TEXT_INSET,
            rows * CARD_LINE_H,
        )
    }

    /// The hint's box: a card's first text row (no panel under it). A click
    /// in it opens the help card.
    pub fn hint(&self) -> Rect {
        self.card_text(1)
    }

    /// The whole slot a card can cover (empty without `intro`).
    pub fn card_slot(&self) -> Rect {
        if self.intro {
            self.card(CARD_MAX_ROWS)
        } else {
            Rect::default()
        }
    }

    pub fn label_boxes(&self) -> [Rect; 3] {
        [self.bar_label, self.title_label, self.chip_label]
    }

    /// The label boxes of the Home view (DS-1): no window title; the intent
    /// field's text box instead.
    pub fn home_label_boxes(&self) -> [Rect; 3] {
        [self.bar_label, self.field_text, self.chip_label]
    }

    /// What switching between the Home view and the window view repaints:
    /// both views' differing areas (the window's glow and the field's glow
    /// overlap; the overlap is simply painted twice). DS-1c: the Lens frame
    /// shows only in the window view, so its halo's reach is one more (empty
    /// without room for a Lens; painting an empty rect is a no-op).
    /// `desktop-intro`: the card slot, shown only in Home (empty without it).
    /// `desktop-clock`: the readout's box, shown only in Home (empty without
    /// it).
    pub fn view_regions(&self) -> [Rect; 7] {
        let lens = self.lens.map(|l| l.halo).unwrap_or_default();
        [
            self.glow,
            self.field_glow,
            self.spine,
            self.bar_label,
            lens,
            self.card_slot(),
            self.clock_slot(),
        ]
    }

    /// `desktop-clock`: the readout's box (time above date), the field's
    /// width, `CLOCK_GAP` above the field's focus light; kept below the craft
    /// bar on a short output. Masked from the digest oracle like the labels.
    pub fn clock_box(&self) -> Rect {
        let top = self.field_glow.y.saturating_sub(CLOCK_GAP + CLOCK_H);
        Rect::new(
            self.field.x,
            top.max(self.workfield.y),
            self.field.w,
            CLOCK_H,
        )
    }

    /// The readout's box when Home shows one (empty without `clock`).
    pub fn clock_slot(&self) -> Rect {
        if self.clock {
            self.clock_box()
        } else {
            Rect::default()
        }
    }

    /// The area a focus change repaints: the focus light, the window, the
    /// Spine (marker) and the craft-bar label.
    pub fn focus_damage(&self) -> Rect {
        self.glow.union(&self.spine).union(&self.bar_label)
    }

    /// The disjoint pieces of [`Self::focus_damage`] whose base pixels can
    /// change with focus. The union's bounding box spans most of the screen,
    /// so repaint (the backdrop is ~60 cycles/px) walks these instead; only
    /// the cheap composite copies the union. The client interior is
    /// focus-independent (opaque content floor), so the glow rect is walked
    /// as the four strips around it.
    pub fn focus_regions(&self) -> [Rect; 6] {
        let (g, c) = (self.glow, self.client);
        [
            Rect::new(g.x, g.y, g.w, c.y - g.y),
            Rect::new(g.x, c.bottom(), g.w, g.bottom() - c.bottom()),
            Rect::new(g.x, c.y, c.x - g.x, c.h),
            Rect::new(c.right(), c.y, g.right() - c.right(), c.h),
            self.spine,
            self.bar_label,
        ]
    }
}

fn lens_frame_w() -> usize {
    LENS_W + 2 * PAD
}

fn lens_frame_h() -> usize {
    TITLE_H + LENS_H + PAD
}

fn frame_size(client_w: usize, client_h: usize) -> (usize, usize) {
    (client_w + 2 * PAD, TITLE_H + client_h + PAD)
}

/// The room a frame needs around it on each axis: its focus light and the
/// workfield margin, both sides.
const FIT_PAD: usize = 2 * (GLOW + MIN_MARGIN);

fn fits(screen_w: usize, screen_h: usize, frame_w: usize, frame_h: usize) -> bool {
    screen_w >= SPINE_W + frame_w + FIT_PAD && screen_h >= BAR_H + frame_h + FIT_PAD
}

/// DL-1: the docked group's outer size for a frame of `frame_w` x `frame_h`
/// (the Lens's right margin, `MIN_MARGIN`, in the width).
fn docked_group(frame_w: usize, frame_h: usize) -> (usize, usize) {
    (
        frame_w + LENS_GAP + lens_frame_w() + MIN_MARGIN,
        frame_h.max(lens_frame_h()),
    )
}

/// A Spine chip's label box ("A" / "Arthur", "L" / "Lens", "I" / "Install").
const fn chip_label_of(chip: Rect) -> Rect {
    Rect::new(chip.x + 3, chip.y + 8, CHIP - 6, CHIP - 10)
}

/// The frame origin nearest `(x, y)` that keeps the frame plus its focus
/// light inside `workfield` (callers have checked [`fits`]).
fn clamp_origin(
    workfield: &Rect,
    frame_w: usize,
    frame_h: usize,
    x: i64,
    y: i64,
) -> (usize, usize) {
    let lo_x = (workfield.x + GLOW) as i64;
    let hi_x = (workfield.right() - GLOW).saturating_sub(frame_w) as i64;
    let lo_y = (workfield.y + GLOW) as i64;
    let hi_y = (workfield.bottom() - GLOW).saturating_sub(frame_h) as i64;
    // Callers check the frame (or group) fits (`fits`, a docked Lens, the
    // `bloom_origin` precheck, [`Layout::docked_max_client`]), so
    // `lo <= hi`. A mistake lands at the low edge, never a panic or a wrap:
    // the subtractions saturate (Janus DL1-X1) and `max` comes last.
    (
        x.min(hi_x).max(lo_x) as usize,
        y.min(hi_y).max(lo_y) as usize,
    )
}
