// SPDX-License-Identifier: MPL-2.0
//! S1 desktop session: the WM-owned full-screen scene, as pure logic.
//!
//! Everything the trusted userland WM paints itself lives here — layout of the
//! craft bar, the Spine, and the client window frame for any output size, the
//! base painter, and the focus light. It is deliberately free of syscalls so
//! that the same file is mounted three times:
//!
//! * by `neo-init`'s desktop session (the WM paints with [`paint_base`] and
//!   [`fill_glow`]),
//! * by the host KATs (`tools/atrium-logic-tests`), which assert the rect
//!   painter against the independent per-pixel definition [`base_pixel`], and
//! * by the kernel `desktop_session_boot` scenario, which reads every pixel of
//!   the real framebuffer back and checks it against [`scene_pixel`] — the
//!   full-screen, no-stale-pixels, no-phase-indicator oracle.
//!
//! Visual grammar (docs/THINK-2026-07-10-atrium-design-language.md §1.2, §4,
//! §6.2 and the glass amendment §3): the Reduce-Transparency floor is solid —
//! Void ground with a quiet Threshold rise toward the bottom, calm chrome in
//! the Dark tones, Silver structure, and Electric Blue used only as *light*
//! (focus glow, owned-authority corner ticks, the Spine's active marker).
//! There are no drop shadows: the Undo Shadow is the only one in the system.

use super::atrium_wm::{Rect, blend_over};

/// Void — the resting ground (design-system §1).
pub const VOID: u32 = 0x000A_0E27;
/// The Threshold end of the ground's vertical rise (Void lifted toward
/// Electric Blue); the bottom row of the screen reaches exactly this colour.
pub const THRESHOLD: u32 = 0x0012_1C42;
/// Dark 2 — the craft bar and Spine panels.
pub const PANEL: u32 = 0x000F_1219;
/// Gray 2 — hairline separators.
pub const HAIRLINE: u32 = 0x002D_3748;
/// Dark 1 — Spine chips and the title band.
pub const RAISED: u32 = 0x001A_2332;
/// Silver — owned-surface border (cap-halo "owned" material).
pub const SILVER: u32 = 0x00A8_ADB2;
/// Electric Blue — light, never fill.
pub const ELECTRIC_BLUE: u32 = 0x003C_8EF4;
/// Muted structure-inactive blue (unfocused Spine marker / pips).
pub const INACTIVE_BLUE: u32 = 0x0024_4A7A;

pub const CRAFT_BAR_H: usize = 32;
pub const SPINE_W: usize = 72;
pub const TITLE_H: usize = 28;
pub const BORDER: usize = 1;
/// Width of the focus light outside the frame.
pub const GLOW: usize = 12;
/// Peak alpha of the focus light, at the frame edge.
pub const GLOW_ALPHA: u32 = 96;
pub const TICK_LEN: usize = 10;
pub const TICK_W: usize = 2;
pub const CHIP: usize = 40;
pub const CHIP_INSET: usize = 16;
pub const MARKER_W: usize = 3;
pub const PIP: usize = 8;
const MIN_MARGIN: usize = 16;

/// How much WM chrome fits the output. Chosen per output size so any GOP mode
/// that can hold the client at all gets a scene that covers every pixel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    /// Craft bar + Spine + framed, focus-lit window.
    Full,
    /// No craft bar / Spine; framed window without the focus light.
    Framed,
    /// Client only (the output is barely larger than the client).
    Bare,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopLayout {
    pub tier: Tier,
    pub screen: Rect,
    pub craft_bar: Rect,
    pub spine: Rect,
    pub workfield: Rect,
    /// Border + title band + client, the WM-owned window chrome.
    pub frame: Rect,
    pub title: Rect,
    /// Where the client's pixels are composited. The client never learns it.
    pub client: Rect,
    /// The focus light's extent (empty unless `Tier::Full`).
    pub glow: Rect,
    /// The Spine chip for the (single, in S1) client surface.
    pub chip: Rect,
}

/// Lay the desktop out on a `screen_w` x `screen_h` output for one client
/// surface of `client_w` x `client_h`. `None` when the client cannot fit.
pub fn layout(
    screen_w: usize,
    screen_h: usize,
    client_w: usize,
    client_h: usize,
) -> Option<DesktopLayout> {
    if screen_w == 0 || screen_h == 0 || client_w == 0 || client_h == 0 {
        return None;
    }
    let screen = Rect::new(0, 0, screen_w, screen_h);
    let frame_w = client_w + 2 * BORDER;
    let frame_h = client_h + TITLE_H + 2 * BORDER;
    let pad = 2 * (GLOW + MIN_MARGIN);

    let tier = if screen_w >= SPINE_W + frame_w + pad && screen_h >= CRAFT_BAR_H + frame_h + pad {
        Tier::Full
    } else if screen_w >= frame_w && screen_h >= frame_h {
        Tier::Framed
    } else if screen_w >= client_w && screen_h >= client_h {
        Tier::Bare
    } else {
        return None;
    };

    let (craft_bar, spine) = match tier {
        Tier::Full => (
            Rect::new(0, 0, screen_w, CRAFT_BAR_H),
            Rect::new(0, CRAFT_BAR_H, SPINE_W, screen_h - CRAFT_BAR_H),
        ),
        _ => (Rect::default(), Rect::default()),
    };
    let workfield = Rect::new(
        spine.right(),
        craft_bar.bottom(),
        screen_w - spine.right(),
        screen_h - craft_bar.bottom(),
    );
    let (fw, fh) = match tier {
        Tier::Bare => (client_w, client_h),
        _ => (frame_w, frame_h),
    };
    let frame = Rect::new(
        workfield.x + (workfield.w - fw) / 2,
        workfield.y + (workfield.h - fh) / 2,
        fw,
        fh,
    );
    let (title, client) = match tier {
        Tier::Bare => (Rect::default(), frame),
        _ => (
            Rect::new(frame.x + BORDER, frame.y + BORDER, client_w, TITLE_H),
            Rect::new(
                frame.x + BORDER,
                frame.y + BORDER + TITLE_H,
                client_w,
                client_h,
            ),
        ),
    };
    let (glow, chip) = match tier {
        Tier::Full => (
            Rect::new(
                frame.x - GLOW,
                frame.y - GLOW,
                frame.w + 2 * GLOW,
                frame.h + 2 * GLOW,
            ),
            Rect::new(spine.x + CHIP_INSET, spine.y + CHIP_INSET, CHIP, CHIP),
        ),
        _ => (Rect::default(), Rect::default()),
    };
    Some(DesktopLayout {
        tier,
        screen,
        craft_bar,
        spine,
        workfield,
        frame,
        title,
        client,
        glow,
        chip,
    })
}

/// The ground colour of row `y` on an output `height` rows tall: a linear,
/// per-channel rise from [`VOID`] (row 0) to [`THRESHOLD`] (the last row).
pub fn ground(y: usize, height: usize) -> u32 {
    let span = height.saturating_sub(1).max(1) as u64;
    let t = (y as u64).min(span);
    let mut out = 0u32;
    let mut shift = 0;
    while shift < 24 {
        let a = ((VOID >> shift) & 0xFF) as u64;
        let b = ((THRESHOLD >> shift) & 0xFF) as u64;
        let c = (a * (span - t) + b * t + span / 2) / span;
        out |= (c as u32) << shift;
        shift += 8;
    }
    out
}

/// The independent per-pixel definition of the WM base layer (everything the
/// WM paints opaquely). Inside `layout.client` it returns the ground colour —
/// the client layer covers that area; this is only what shows if it did not.
pub fn base_pixel(layout: &DesktopLayout, focused: bool, x: usize, y: usize) -> u32 {
    let accent = if focused {
        ELECTRIC_BLUE
    } else {
        INACTIVE_BLUE
    };
    if layout.craft_bar.contains(x, y) {
        if y + 1 == layout.craft_bar.bottom() {
            return HAIRLINE;
        }
        let light = Rect::new(CHIP_INSET, (CRAFT_BAR_H - PIP) / 2, PIP, PIP);
        return if light.contains(x, y) {
            ELECTRIC_BLUE
        } else {
            PANEL
        };
    }
    if layout.spine.contains(x, y) {
        if x + 1 == layout.spine.right() {
            return HAIRLINE;
        }
        if layout.chip.contains(x, y) {
            let marker = Rect::new(layout.chip.x, layout.chip.y, MARKER_W, CHIP);
            return if marker.contains(x, y) {
                accent
            } else {
                RAISED
            };
        }
        return PANEL;
    }
    if layout.tier != Tier::Bare && layout.frame.contains(x, y) && !layout.client.contains(x, y) {
        let f = layout.frame;
        let (dx, dy) = (x - f.x, y - f.y);
        let (rx, ry) = (f.right() - 1 - x, f.bottom() - 1 - y);
        let near_x = dx.min(rx);
        let near_y = dy.min(ry);
        // Owned-authority corner ticks: an L of Electric Blue in each corner.
        if (near_x < TICK_W && near_y < TICK_LEN) || (near_y < TICK_W && near_x < TICK_LEN) {
            return ELECTRIC_BLUE;
        }
        if near_x < BORDER || near_y < BORDER {
            return SILVER;
        }
        let pip = Rect::new(
            layout.title.x + CHIP_INSET,
            layout.title.y + (TITLE_H - PIP) / 2,
            PIP,
            PIP,
        );
        return if pip.contains(x, y) { accent } else { RAISED };
    }
    ground(y, layout.screen.h)
}

/// The focus light as a straight-alpha XRGB pixel at scene `(x, y)`: Electric
/// Blue whose alpha falls off linearly over [`GLOW`] pixels outside the frame
/// (Chebyshev distance). Zero alpha on and inside the frame and outside the
/// glow rect, and always zero unless focused in [`Tier::Full`].
pub fn glow_pixel(layout: &DesktopLayout, focused: bool, x: usize, y: usize) -> u32 {
    if !focused || layout.glow.is_empty() || !layout.glow.contains(x, y) {
        return 0;
    }
    let f = layout.frame;
    if f.contains(x, y) {
        return 0;
    }
    let dx = if x < f.x {
        f.x - x
    } else if x >= f.right() {
        x - f.right() + 1
    } else {
        0
    };
    let dy = if y < f.y {
        f.y - y
    } else if y >= f.bottom() {
        y - f.bottom() + 1
    } else {
        0
    };
    let d = dx.max(dy) as u32;
    let alpha = GLOW_ALPHA * (GLOW as u32 + 1 - d) / GLOW as u32;
    (alpha << 24) | ELECTRIC_BLUE
}

/// The final composed colour of every scene pixel the WM owns (all pixels
/// outside `layout.client`): the base with the focus light blended over it.
pub fn scene_pixel(layout: &DesktopLayout, focused: bool, x: usize, y: usize) -> u32 {
    let base = base_pixel(layout, focused, x, y);
    let glow = glow_pixel(layout, focused, x, y);
    if glow >> 24 == 0 {
        base
    } else {
        blend_over(base, glow)
    }
}

/// Paint the base layer into `dst` (`stride` in pixels) with rect fills. Must
/// equal [`base_pixel`] at every pixel — the host KAT holds them together.
pub fn paint_base(dst: &mut [u32], stride: usize, layout: &DesktopLayout, focused: bool) {
    let screen = layout.screen;
    for y in 0..screen.h {
        let row = y * stride;
        dst[row..row + screen.w].fill(ground(y, screen.h));
    }
    let accent = if focused {
        ELECTRIC_BLUE
    } else {
        INACTIVE_BLUE
    };
    if !layout.craft_bar.is_empty() {
        let bar = layout.craft_bar;
        fill(dst, stride, bar, PANEL);
        fill(
            dst,
            stride,
            Rect::new(bar.x, bar.bottom() - 1, bar.w, 1),
            HAIRLINE,
        );
        fill(
            dst,
            stride,
            Rect::new(CHIP_INSET, (CRAFT_BAR_H - PIP) / 2, PIP, PIP),
            ELECTRIC_BLUE,
        );
    }
    if !layout.spine.is_empty() {
        let spine = layout.spine;
        fill(dst, stride, spine, PANEL);
        fill(
            dst,
            stride,
            Rect::new(spine.right() - 1, spine.y, 1, spine.h),
            HAIRLINE,
        );
        fill(dst, stride, layout.chip, RAISED);
        fill(
            dst,
            stride,
            Rect::new(layout.chip.x, layout.chip.y, MARKER_W, CHIP),
            accent,
        );
    }
    if layout.tier != Tier::Bare {
        let f = layout.frame;
        fill(dst, stride, f, SILVER);
        fill(
            dst,
            stride,
            Rect::new(
                f.x + BORDER,
                f.y + BORDER,
                f.w - 2 * BORDER,
                f.h - 2 * BORDER,
            ),
            RAISED,
        );
        fill(
            dst,
            stride,
            Rect::new(
                layout.title.x + CHIP_INSET,
                layout.title.y + (TITLE_H - PIP) / 2,
                PIP,
                PIP,
            ),
            accent,
        );
        for (cx, cy) in [
            (f.x, f.y),
            (f.right() - TICK_LEN, f.y),
            (f.x, f.bottom() - TICK_W),
            (f.right() - TICK_LEN, f.bottom() - TICK_W),
        ] {
            fill(
                dst,
                stride,
                Rect::new(cx, cy, TICK_LEN, TICK_W),
                ELECTRIC_BLUE,
            );
        }
        for (cx, cy) in [
            (f.x, f.y),
            (f.right() - TICK_W, f.y),
            (f.x, f.bottom() - TICK_LEN),
            (f.right() - TICK_W, f.bottom() - TICK_LEN),
        ] {
            fill(
                dst,
                stride,
                Rect::new(cx, cy, TICK_W, TICK_LEN),
                ELECTRIC_BLUE,
            );
        }
        // The client area keeps the ground colour underneath (see base_pixel).
        let c = layout.client;
        for y in c.y..c.bottom() {
            let row = y * stride;
            dst[row + c.x..row + c.right()].fill(ground(y, screen.h));
        }
    }
}

/// Fill the focus-light layer (`layout.glow.w` x `layout.glow.h`, tightly
/// packed) from [`glow_pixel`].
pub fn fill_glow(dst: &mut [u32], layout: &DesktopLayout, focused: bool) {
    let g = layout.glow;
    for y in 0..g.h {
        for x in 0..g.w {
            dst[y * g.w + x] = glow_pixel(layout, focused, g.x + x, g.y + y);
        }
    }
}

fn fill(dst: &mut [u32], stride: usize, rect: Rect, colour: u32) {
    for y in rect.y..rect.bottom() {
        let row = y * stride;
        dst[row + rect.x..row + rect.right()].fill(colour);
    }
}
