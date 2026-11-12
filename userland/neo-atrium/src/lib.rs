// SPDX-License-Identifier: MPL-2.0
//! neo-atrium: the desktop scene — Threshold backdrop, CPU frosted glass,
//! craft chrome and the Reduce-Transparency floor (S3), the Home view and the
//! Lens (DS-1), the frame's place (DI-1b, DL-1) and motion (DM-1) — as pure,
//! host-testable code. The session WM (`neo-init`, feature `desktop-glass` and
//! up) paints its base layer with it; nothing here makes a syscall.
//!
//! Design authority: docs/THINK-2026-07-19-atrium-glass-frost-amendment.md,
//! the ADR-024 Amendment 2026-07-19 (selective glass, trust-as-tint plus
//! hue-independent cues, Reduce-Transparency = opaque neutral + the same cues),
//! docs/THINK-2026-07-10-atrium-design-language.md §1.2/§4/§6, and the S3
//! dispatch (CPU path, blur cached and never per frame).
//!
//! # Scene specification v1.8 (normative — `scripts/desktop/scene_oracle.py`
//! implements this text; the algorithms were written from it independently,
//! while the *design constants* below were fixed first and stated here, so
//! the oracle catches a wrong algorithm, not a wrong design choice — that is
//! what the screenshots and design review are for)
//!
//! **Wide values.** Every channel of the base layer is computed in "wide"
//! units `W = 8-bit value × 256` (8.8 fixed point, `0..=65535`) and dithered
//! to 8 bits once, at the end: `out = min(255, (W + 16·B[y&3][x&3] + 8) >> 8)`
//! with the 4×4 Bayer matrix [`wide::BAYER4`]. An opaque token colour `C`
//! (`W = 256·C`) therefore dithers back to exactly `C`.
//!
//! **Backdrop** ([`backdrop`]). Per channel: the vertical rise from Void
//! `0x0A0E27` to Threshold `0x121C42` in wide units,
//! `(V·(s−t) + T·t + s/2)/s` with `s = max(H−1, 1)`, `t = min(y, s)`; plus two
//! radial lights `q = ((r² − d²)·65536)/r²`,
//! `f = q²>>16`, `add = (256·P·f)>>16`, plus two thin rings whose approximate
//! signed distance is `s = trunc(8·(d² − R²)/R)` in 1/16 px, profile
//! `a = max(0, 24 − |s|)`, `f = (a·65536)/24`, `add = (256·P·f)>>16`; summed and
//! clamped to 65535. `d²` uses integer pixel coordinates. Geometry (integer
//! division of the output size `W×H`): `c = (62W/100, 48H/100)`; light 1 at
//! `c`, `r = 70H/100`, `P = (16,40,86)`; light 2 at `(12W/100, 96H/100)`,
//! `r = 60H/100`, `P = (26,10,44)`; rings at `c`, `R = 58H/100`,
//! `P = (22,48,96)` and `R = 80H/100`, `P = (16,34,70)`.
//!
//! **Blur** ([`blur`]). A glass shape's bounding box `G` is blurred from the
//! backdrop over `S = G` grown by `3·BLUR_RADIUS` and clipped to the screen,
//! with edge-clamp at `S`'s borders. One pass = a horizontal then a vertical
//! box of radius `BLUR_RADIUS`, `out = (Σ in[clamp(i+k)] + R) / (2R+1)` over
//! `k ∈ [−R, R]`; three passes; integer throughout.
//!
//! **Material** ([`material`]). Owned glass: `g = (b·(256−F−V) + 65280·F +
//! 256·EB·V + 128) >> 8` per channel (`F` frost, `V` Electric-Blue vibrancy,
//! weights out of 256). Reduce-Transparency replaces `g` with an opaque
//! neutral token. Cap-Halo cues (solid hairline + Electric-Blue corner ticks +
//! labels) are drawn identically in both modes.
//!
//! Frost/vibrancy: bar, Spine and frame `F = 18`, chip `F = 30`, all with
//! `V = 26` Electric Blue `0x3C8EF4`. RT tokens: panels `0x1A2130`, the chip
//! `0x222A3C`.
//!
//! **Shapes** ([`shape`]). Rounded-rect coverage is 255 inside, 0 outside, and
//! in each `r×r` corner square `round_half_up(255·A)` where `A` is the exact
//! area of the pixel square inside the quarter disc. A panel composites
//! `out = (g·c + below·(255−c) + 127) / 255`; its hairline is the coverage of
//! the shape minus that of the shape inset by one pixel (radius `r−1`),
//! blending white at `a = (36·ring + 127)/255`: `out += ((65280 − out)·a + 127)/255`
//! (saturating). The bar and Spine are square, full-coverage panels whose
//! only hairline is the edge facing the workfield (the bar's last row, the
//! Spine's last column) at `a = 36`; the chip (r = 10) and the window frame
//! (r = 12) are rounded panels with the full ring.
//!
//! **Layout** ([`layout`]) for client `cw×ch` on `W×H` (all integer, floor
//! division): bar `(0,0,W,36)`; Spine `(0,36,84,H−36)`; chip `(14,50,56,56)`;
//! focus marker `(4,62,3,32)` (Electric Blue when focused, `0x244A7A` not);
//! status light: a circle (`r = 4`) in `(16,14,8,8)`, Electric Blue at its
//! coverage; frame `cw+16 × ch+42` centred in the workfield
//! `(84,36,W−84,H−36)`; client `(frame.x+8, frame.y+34)`; the scene needs
//! `W ≥ 84 + frame.w + 60` and `H ≥ 36 + frame.h + 60`. Label boxes: bar
//! `(30,6,170,24)`, title `(frame.x+14, frame.y+4, 240, 26)`, chip
//! `(17,58,50,46)`. Every rounded rect clamps `r` to `min(r, w/2, h/2)`.
//!
//! **Cap-Halo (owned).** On the frame: with `dx, dy` the distances to the
//! nearer vertical / horizontal edge, a pixel is a tick when
//! (`dy < 2` and `12 ≤ dx < 22`) or (`dx < 2` and `12 ≤ dy < 22`); ticks are
//! solid Electric Blue, overwriting panel and hairline.
//!
//! **Focus light** (a separate WM plane, not the base). Straight-alpha
//! Electric Blue, `α = floor(110·t² + 0.5)`, `t = (14 − d)/14`, for
//! `0 < d ≤ 14`, where `d` is the Euclidean distance from the pixel centre
//! `(x+½, y+½)` to the frame's rounded rect (r = 12); composited after the
//! dither with `atrium_wm::blend_over`. Zero when unfocused.
//!
//! **Home view** (DS-1, [`scene::View::Home`]). No window: the frame, its
//! ticks and the client floor are not drawn, the focus marker is inactive
//! (`0x244A7A`) and the bar reads "Atrium". The intent field is a rounded
//! panel (r = 14) of width `min(720, W − 84 − 60)`, height 56, at
//! `x = 84 + (W − 84 − w)/2`, `y = 36 + (H − 36 − 56)·38/100`, frosted like
//! the frame (`F = 18`, `V = 26`; RT `0x222A3C`) from its own cached blur, with
//! the full hairline ring. While the WM holds the keyboard, the focus light
//! law above (around the field, r = 12 → 14) is mixed into the wide pixels
//! *before* the field panel and the dither:
//! `out = (EB·256·α + out·(255 − α) + 127)/255`. Its label boxes are the bar's, the chip's and the field's
//! text box `T = (field.x+22, field.y+8, field.w−44, 40)`, which holds the
//! typed text or the placeholder and the caret, everything clipped to `T`
//! (v1.3.1, exact — Codex DS-1a F2; `scripts/desktop/field_oracle.py`). Both
//! runs sit on the baseline `field.y + 35`. Empty: the placeholder "Ask Arthur
//! anything", 16 px, secondary, its origin at `x0 = T.x`. Otherwise the text,
//! 18 px, primary: with `w` its advance (kerned, unrounded) and `room = T.w −
//! 4`, its origin is `x0 = T.x + room − w` when `w > room` (scrolled left, its
//! end visible), else `T.x`. The caret, only while the field has the
//! keyboard (lit and not chip-selected): an Electric-Blue 2×24 rectangle at
//! `(c, field.y + 16)` with `c = T.x` when empty, else `⌊x0 + w + 1⌋`, drawn
//! over the text.
//!
//! **Home states** (DS-1). *Chip selected* (the keyboard on the Spine chip):
//! the focus light law around the chip (r = 10, reach 14) is mixed into the
//! wide pixels after the Spine and before the chip panel, the field is not
//! lit and has no caret. *Held* (Arthur yielded): an Electric-Blue circle
//! (r = 4) in `(chip.x + 24, chip.y + 62, 8, 8)` is mixed at its exact-area
//! coverage right after the chip panel.
//!
//! **Lens** (v1.4, DS-1c, while [`scene::BaseScene::set_lens`] is on). *Second
//! chip*, in both views: a rounded panel (r = 10) at `(14, 128, 56, 56)` (the
//! first chip's bottom + 22), frosted like the first chip (`F = 30`, `V = 26`)
//! but sampling the **Spine's** cached blur at the same pixel (RT `0x222A3C`),
//! with the full hairline ring, mixed right after the held dot. Its label box
//! `(17, 136, 50, 46)` reads "L" / "Lens" like the first chip's "A" / "Arthur".
//! *Lens frame*, window view only: it exists only when `L.x + L.w + 30 ≤ W`
//! and `L.y + L.h + 14 ≤ H` for `L = (frame.x + frame.w + 32, frame.y, 376,
//! 442)` (v1.6: a fixed height — 34 + 400 + 8; DS-1c took the window's,
//! which was the same 442 until DI-1b could resize it); a rounded
//! panel (r = 12) frosted like the window frame from its own cached blur
//! (RT `0x1A2130`) with the full hairline ring and **no** owned ticks,
//! mixed after the window frame's ticks. *Read-only Cap Halo*: the 1-px
//! border of `(L.x − 4, L.y − 4, L.w + 8, L.h + 8)`: a pixel on its top or
//! bottom row is a dash when `(x − ring.x) mod 10 < 6`, any other border
//! pixel when `(y − ring.y) mod 10 < 6`; dashes are lightened with `a = 110`
//! right after the Lens frame (dashes, not hue: the cue survives greyscale).
//! *Lens client* `(L.x + 8, L.y + 34, 360, 400)`: the content floor, like the
//! window's client. The title label box `(L.x + 14, L.y + 4, 240, 26)` reads
//! "Lens · read-only". The Lens frame's blur is computed with the Home blur
//! at build (glass: six blurs in all; later only as **Docked Lens** below
//! says) — only for a scene built to show a Lens ([`scene::BaseScene::with_lens`]); a Home scene built
//! without it has no Lens layout at all (DS-1c off ≡ DS-1b).
//!
//! **Selection** (v1.5, DS-1c). While the selection is on the Lens
//! ([`scene::BaseScene::set_lens_selected`], only while it is shown) the
//! chip focus light — the Home chip's glow, same formula, around the
//! *second* chip's rect `(14, 128, 56, 56)` — replaces the first chip's: in
//! the window view always (the WM shows Arthur unfocused then), in Home only
//! while the keyboard is on the Spine (`chip_selected`). It is drawn at the
//! chip-light step, before both chip panels and the held dot.
//!
//! **Docked Lens** (v1.6, DL-1). A shown Lens is docked: it is laid out from
//! the frame as above wherever the frame is, so it moves with a move and
//! follows a resize's right and top edges, its own size fixed. While docked,
//! a move or resize keeps the *group* inside the workfield: the group is the
//! frame, the 32-px gap, the Lens frame and the Lens's 16-px right margin
//! (`frame.w + 32 + 376 + 16` wide, `max(frame.h, 442)` tall), clamped like a
//! frame alone (its top-left at least `GLOW` inside the workfield, its
//! bottom-right at least `GLOW` inside); a resize stops the right edge where
//! the group still fits and the top where the Lens's bottom does. A Lens
//! blooming beside a window where it does not fit first moves the window to
//! the group-clamped origin nearest it (none when the workfield cannot hold
//! the group: the bloom is refused). In glass a docked frame change re-blurs
//! the Lens frame too (two blurs); a frame change with no Lens shown leaves
//! the Lens's blur for the next bloom (one).
//!
//! **Motion** (v1.7, DM-1, [`motion`]). Transitions between those states
//! (window open and close, Home ↔ window, the Lens's bloom and close, the
//! focus light) are frame sequences over the state change's own damage
//! rects, ending on the state's ordinary present; the normative text is in
//! the module. A motion-off WM presents the end state directly.
//!
//! **Intro card** (v1.8, `desktop-intro`, while
//! [`scene::BaseScene::set_intro`] is on; Home view only). The *card slot*
//! hangs under the intent field: a card of `n` rows is the rect
//! `(field.x, field.y + 56 + 14 + 18, field.w, 24·n + 20)`, its text box
//! `(card.x + 22, card.y + 10, card.w − 44, 24·n)` (masked like a label box),
//! the slot itself the 8-row card (a view switch repaints it). A shown card
//! (rows from [`intro`]) is a rounded panel (r = 14) filled with the content
//! floor `0x0A0E27` — opaque in both materials, no blur — mixed at its
//! coverage after the intent field, with the full hairline ring; row `i`
//! (Inter 15 px, primary) has its origin at `(text.x, card.y + 30 + 24·i)`.
//! With no card the slot shows the *hint* (no panel): [`intro::HINT`], Inter
//! 14 px, secondary, centred in the 1-row text box (the hint box) on the
//! baseline `card.y + 30`. Off, there is no slot and Home is v1.7's exactly.
//!
//! **Order** ([`scene`]): backdrop → craft bar → Spine → Spine chip → (held
//! dot → second chip) → window frame → accents (ticks) → (Lens frame →
//! halo) → focus marker, status light → client floors → (Home: field light →
//! field → intro card) → dither → Inter labels (8-bit, gamma-correct, inside
//! fixed label boxes).

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod backdrop;
pub mod blur;
pub mod busy;
pub mod clock;
pub mod contrast;
#[cfg(feature = "app-icons")]
pub mod icons;
pub mod intro;
pub mod keys;
pub mod layout;
pub mod material;
pub mod motion;
pub mod pointer_grab;
pub mod power;
pub mod scene;
pub mod shape;
pub mod wide;

#[cfg(all(test, feature = "keyboard-layouts"))]
mod keys_layout_tests;
#[cfg(test)]
mod tests;

pub use layout::{Layout, Rect};
pub use material::{Mode, TrustState};
#[cfg(feature = "app-chips")]
pub use scene::ChipLook;
pub use scene::{BaseScene, View, glow_pixel};
