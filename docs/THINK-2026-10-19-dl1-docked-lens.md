<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — DL-1: the Lens with move and resize (docked)

**Lane:** DL-1 (PM, 2026-10-19). **Branch:** `claude/desktop-s1-session-boot`. **Base:** `main` @ `2acfa308` (DI-1b #53).

## The gap DI-1b left

DI-1b made Arthur's window movable and resizable, and DS-1c had already put the Lens beside it. Merging both left two
problems:

- `desktop-lens` + `desktop-resize` was a `compile_error!`. DS-1c laid the Lens out at Arthur's *client height*, so a
  resized Arthur would have a Lens taller or shorter than its fixed 360x400 surface.
- With a Lens shown, the title band and the resize handles went dead. `place_frame` refused every move, because the Lens
  sat beside the frame and a move could push it off-screen.

## The choice: dock, don't float

There were two ways to compose them:

1. **Docked.** The Lens follows Arthur's frame. It stays top-aligned, `LENS_GAP` to the right, moves with a move, and
   follows a resize's right and top edges. Its own size is fixed.
2. **Floating.** The Lens becomes its own movable window.

**Docked is chosen.**

- **The Lens is Arthur's instrument, not an app.** Arthur's `list /` blooms it, its rights are lent inside Arthur's
  `OP_LAUNCH`, and it ends with Arthur's session. Keeping it attached to Arthur says so on screen, where the read-only
  Cap Halo already says "lent, read-only".
- **Floating would open problems this lane has no reason to own:**
  - z-order, and occlusion between two client surfaces (today's compositor never overlaps clients);
  - a second set of grabs, and drag-outline state for a second frame;
  - per-window focus light;
  - a pointer hit-test that decides which window a press means.

  Those belong to a real multi-window lane, not to this composition fix.
- **Docked keeps every DS-1c pixel.** At Arthur's default 640x400 the docked Lens is exactly DS-1c's. The new fixed Lens
  height (`LENS_H = 400`, frame 442) equals the window frame's height then, so all DS-1c fixtures regenerate
  byte-identically. This was checked: `scene_oracle.py lens-fixture` and `lens_content_oracle.py labels`/`fixture` diff
  clean.

## The rules (spec v1.6, `neo-atrium/src/lib.rs` "Docked Lens")

- **Fixed Lens.** The client is `360x400` and the frame `376x442`, wherever it is. A resized Arthur never resizes the
  Lens surface, so no second configure protocol is needed.
- **Group clamp.** While docked, a move or resize keeps the *group* inside the workfield:
  - the group is the frame, the 32-px gap, the Lens frame and the Lens's 16-px right margin;
  - its height is `max(frame.h, 442)`;
  - a move clamps the group's origin as DI-1b clamped the frame alone;
  - a resize stops the right edge where the group still fits, and the top where the Lens's bottom does.
- **Bloom slides.** If a window was moved where a Lens would not fit beside it, the bloom first moves it, in one ordinary
  move, to the group-clamped origin nearest it (`Layout::bloom_origin`). DS-1c refused the bloom instead. Only when the
  workfield cannot hold the group at all is the bloom refused.
- **Blurs.**
  - In glass, a docked frame change re-blurs the frame and the Lens: two blurs.
  - A frame change with no Lens shown re-blurs only the frame. The Lens's blur goes stale and is redone once at its next
    bloom.
  - All of these count as *move* blurs (`count_move_blurs`), never as re-blurs during typing.

## Kill-switch

- `desktop-full = ["desktop-lens-pointer", "desktop-resize"]` is the composed build.
- **Docking is compiled only with the Lens and resize together.** That covers the live title band and handles while
  a Lens is shown, and the bloom's slide.
- **`desktop-lens-pointer` alone** is DS-1c x DI-1b: the window stays put while a Lens is up, and a bloom with no room
  is refused. An AAG lens caught an earlier draft that docked there too.
- **`desktop-resize` alone** shows no Lens, so `lens_docked` is never set and every clamp is DI-1b's own. The
  `undocked_layout_is_di1b_exactly` KAT covers this.
- **Witness-only change.** The Lens's stale re-blur at a bloom after a move now counts as a move blur, not as a
  re-blur during typing. That is a boot-test witness correction; it changes no pixel.
- **`clamp_origin`** now uses `min().max()` instead of `clamp()`. This is the same result whenever `lo ≤ hi`, which
  every caller guarantees.

**A stale configure (AAG).** A resize released while Arthur's `list /` waits on its launch proof publishes a size
computed with no Lens docked. The bloom may then slide the window. When Arthur acks, the WM re-places the frame where
the docked group fits at that size (`Layout::clamp_frame_for`), instead of refusing it, which would have ended the
session. The KAT is `stale_configure_is_replaced_inside_the_docked_group`.

**Fix-forward, Janus DL1-X1 (High).** The closure above assumed the stale size fits a docked group somewhere. It
need not. On 1280x800, an undocked 640x400 frame at x=98 can be resized to client width 1120 during `list /`. The
docked group at that size is 1560 wide, but the workfield is 1196. `clamp_origin` then underflowed: a debug panic, or
in release a wrap, a refused re-place and fatal error 92.

The closure now holds by construction:
- **The refit.** Right after a successful bloom (still inside `OP_LAUNCH`, Arthur parked on the reply), the WM
  re-publishes any pending configure the docked group cannot hold at `Layout::docked_max_client`, with a new serial
  (`DESKTOP pointer resize refit …`). Arthur only ever reads the refit, and its ack is always placeable.
- **The unchanged ack (AAG).** A refit can equal the frame as it is (Arthur already at the docked maximum, at the
  clamp). `apply_configure` takes that ack as a no-op place instead of the refusal `place_frame` gives an unchanged
  frame.
- **The clamp.** `clamp_origin` saturates, so a violated precondition lands at the low edge and cannot underflow.
- **The KAT.** `stale_configure_too_wide_for_the_docked_group_refits` is Janus's repro (it fails with
  `attempt to subtract with overflow` without the fix). It also proves every refit size placeable docked on five
  outputs.
- **Not in QEMU.** Driving a resize release into the proof-poll window at 1280x800 needs a new scenario. This
  fix-forward does not add one.

## Discriminators

- **Host.** In `neo-atrium/src/tests.rs`:
  - `docked_lens_matches_independent_python_oracle`: three docked cases, glass and RT, against `scene_oracle.py
    dock-fixture`.
  - `docked_group_clamps_match_the_spec`: moves, bloom origins and resizes against the oracle's clamps, written from
    the spec. Undocked controls land elsewhere.
  - `a_shown_lens_follows_the_window_and_a_moved_lens_reblurs_once`: a moved docked scene equals one built there; a
    place the Lens cannot follow is refused; blur counts.
  - `undocked_layout_is_di1b_exactly`.
- **QEMU.** `desktop_full_boot`, with every desktop feature on and a real usb-mouse:
  - **Pre-bloom move.** The title band is dragged +300,+60 with no Lens: 674,337 → 974,397, one blur.
  - **Slide.** `list /` blooms the Lens. It does not fit there, so the window slides to 826,397 (the oracle's bloom
    origin) and the Lens docks at 1514,397.
  - **Docked move.** −200,−40 moves the group to 626,357. The Lens follows to 1314,357 with
    `lens_mismatch=0` (presented Lens pixels against the WM's copy) and `residue_pixels=0` over both halos.
  - **Docked corner resize.** +120,+60: Arthur acks 760x460 and the Lens follows the right edge to 1434,357, still
    376x442.
  - **Capture.** A click inside the resized client reaches Arthur at `local=564,264`. The capture is then held to:
    - `scene_oracle.py dock-fixture` case 0, the whole screen with labels and clients masked;
    - the docked Lens's title and the second chip's text, exact (`lens_content_oracle.py dock-labels`);
    - all 144,000 Lens content pixels, against the DS-1c content oracle at the docked origin.
  - **Blur total.** 6 at build, +1 move, +1 slide, +1 bloom re-blur, +2 docked move, +2 docked resize: 13.
- **Fixed in passing.** `home return … blur_regions_total` was printed as one digit capped at 9, which only worked while
  totals stayed below 10. It is now the full number.

## Limitations (not covered by the gate)

- **RT.** Reduce-Transparency docked scenes are held to the oracle on the host only; no QEMU run uses RT with a Lens.
- **Group clamp.** A docked move that hits the clamp is proven on the host (`DOCK_MOVES`). The QEMU docked move stays
  inside it; only the slide exercises the clamp on hardware.
- **Lens selected.** Moving or resizing while the Lens is selected (Arthur shown unfocused) is the same code path with
  `focused = false`. It is not scripted.
- **1280x800.** With the slide, a Lens can now bloom at 1280x800: the window slides left. DS-1c refused it there. No
  scenario runs that resolution with a Lens.
