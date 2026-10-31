// SPDX-License-Identifier: MPL-2.0
//! DM-1 motion (spec v1.7 "Motion", normative; `scripts/desktop/scene_oracle.py
//! motion-fixture` implements this text independently).
//!
//! A transition replaces the WM's one present of a state change with `N`
//! frames over the same rects `R` (its damage — never the whole screen).
//! `F` is the picture on screen when the transition starts, `T` the picture
//! the state change presents. Frame `k = 1..=N`:
//!
//! * **Weight.** `a(k) = 256 − ⌈256·(N − k)³ / N³⌉` (an ease-out cubic,
//!   integer; `a(N) = 256` and only there — no frame repeats the end).
//! * **Blend.** Every byte of an XRGB pixel: `(f·(256 − a) + t·a + 128) >> 8`.
//!   At `a = 256` this is `t` exactly.
//! * **Shape.** `Dissolve`: every pixel of `R` blends. `Grow` / `Shrink`
//!   reveal through an iris centred on an anchor rect `A`:
//!   `c = (A.x + ⌊A.w/2⌋, A.y + ⌊A.h/2⌋)`; over the bounding box `B` of `R`,
//!   `hw = max(c.x − B.x, B.right − c.x)` and `hh` likewise; with
//!   `s = a(k)` (Grow) or `256 − a(k)` (Shrink) the iris is
//!   `[c.x − ⌈hw·s/256⌉, c.x + ⌈hw·s/256⌉) × [c.y − ⌈hh·s/256⌉, c.y + ⌈hh·s/256⌉)`
//!   (clamped at 0). A pixel inside the iris blends; outside it is `F`
//!   (Grow) or `T` (Shrink).
//! * **End frame.** Frame `N` is the WM's ordinary present of the new state —
//!   the present motion-off makes — so the end frame is `T` by construction
//!   (and, by the rules above, so would a blended frame `N` be: Grow's iris
//!   covers `B`, Shrink's is empty, `a = 256`).
//!
//! **Plans.** Full: open 12 frames Grow (anchor: the window's focus-light
//! rect), close 10 Shrink (same anchor), Home ↔ window 10 Dissolve, Lens
//! bloom 12 Grow (anchor: the Lens halo), Lens close 10 Shrink (same), focus
//! light 8 Dissolve. Reduced motion: every kind is a 6-frame Dissolve — no
//! spatial movement, only a short cross-fade. Off: one frame (the present
//! itself).
//!
//! **Pacing.** Frame `k` is shown no earlier than `t0 + (k − 1)·P`,
//! `P = 16 666 667 ns` (60 Hz), `t0` the first frame's start: a frame is
//! never skipped (the count is exact), a slow frame only delays the rest.

use crate::layout::Rect;

/// The frame period (60 Hz), nanoseconds.
pub const FRAME_NS: u64 = 16_666_667;

/// The motion profile: the user's choice, read once at boot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Full,
    /// Reduce Motion: cross-fades only, shorter.
    Reduced,
    /// No transition: the state change's present is the only frame.
    Off,
}

/// What changes on screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Home → the window view, Arthur launched.
    Open,
    /// The window view → Home, Arthur gone.
    Close,
    /// Home ↔ the window view, Arthur alive (hold, resume, the Lens view).
    View,
    /// The Lens shown.
    Bloom,
    /// The Lens hidden.
    Unbloom,
    /// The focus light on or off.
    Focus,
}

impl Kind {
    pub const fn name(self) -> &'static [u8] {
        match self {
            Kind::Open => b"open",
            Kind::Close => b"close",
            Kind::View => b"view",
            Kind::Bloom => b"bloom",
            Kind::Unbloom => b"unbloom",
            Kind::Focus => b"focus",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    Dissolve,
    Grow,
    Shrink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plan {
    pub frames: u32,
    pub shape: Shape,
}

pub const fn plan(profile: Profile, kind: Kind) -> Plan {
    let (frames, shape) = match profile {
        Profile::Off => (1, Shape::Dissolve),
        Profile::Reduced => (6, Shape::Dissolve),
        Profile::Full => match kind {
            Kind::Open => (12, Shape::Grow),
            Kind::Close => (10, Shape::Shrink),
            Kind::View => (10, Shape::Dissolve),
            Kind::Bloom => (12, Shape::Grow),
            Kind::Unbloom => (10, Shape::Shrink),
            Kind::Focus => (8, Shape::Dissolve),
        },
    };
    Plan { frames, shape }
}

/// `a(k)` out of 256 for frame `k` of `n` (`1 ≤ k ≤ n`).
pub const fn weight(k: u32, n: u32) -> u32 {
    let (k, n) = (k as u64, n as u64);
    let rest = n - k;
    (256 - (256 * rest * rest * rest).div_ceil(n * n * n)) as u32
}

/// One XRGB pixel blended at weight `a` (`0..=256`), every byte
/// `(f·(256 − a) + t·a + 128) >> 8`, two bytes at a time (each lane's sum is
/// at most `255·256 + 128 < 2^16`).
#[inline]
pub const fn blend(f: u32, t: u32, a: u32) -> u32 {
    let b = 256 - a;
    let even = ((f & 0x00FF_00FF) * b + (t & 0x00FF_00FF) * a + 0x0080_0080) >> 8;
    let odd = (((f >> 8) & 0x00FF_00FF) * b + ((t >> 8) & 0x00FF_00FF) * a + 0x0080_0080) >> 8;
    (even & 0x00FF_00FF) | ((odd & 0x00FF_00FF) << 8)
}

/// The iris of frame `k` of `n` (empty for `Dissolve`, which has none).
pub fn iris(shape: Shape, anchor: Rect, bounds: Rect, k: u32, n: u32) -> Rect {
    let a = weight(k, n) as usize;
    let s = match shape {
        Shape::Dissolve => return Rect::default(),
        Shape::Grow => a,
        Shape::Shrink => 256 - a,
    };
    let (cx, cy) = (anchor.x + anchor.w / 2, anchor.y + anchor.h / 2);
    let hw = cx
        .saturating_sub(bounds.x)
        .max(bounds.right().saturating_sub(cx));
    let hh = cy
        .saturating_sub(bounds.y)
        .max(bounds.bottom().saturating_sub(cy));
    let (rx, ry) = ((hw * s).div_ceil(256), (hh * s).div_ceil(256));
    let (x0, y0) = (cx.saturating_sub(rx), cy.saturating_sub(ry));
    Rect::new(x0, y0, cx + rx - x0, cy + ry - y0)
}

/// Write frame `k` of `n` over `rect` of `dst` from `from` (`F`) and `to`
/// (`T`), all three `stride`-pixel planes of the same size; `iris` from
/// [`iris`] for the same frame. Touches nothing outside `rect`.
#[allow(clippy::too_many_arguments)]
pub fn frame(
    dst: &mut [u32],
    from: &[u32],
    to: &[u32],
    stride: usize,
    rect: Rect,
    shape: Shape,
    iris: Rect,
    k: u32,
    n: u32,
) {
    let a = weight(k, n);
    for y in rect.y..rect.bottom() {
        let row = y * stride;
        for x in rect.x..rect.right() {
            let i = row + x;
            dst[i] = match shape {
                Shape::Dissolve => blend(from[i], to[i], a),
                _ if iris.contains(x, y) => blend(from[i], to[i], a),
                Shape::Grow => from[i],
                Shape::Shrink => to[i],
            };
        }
    }
}
