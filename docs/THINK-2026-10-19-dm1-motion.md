<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — DM-1: motion for the desktop's state changes

**Lane:** DM-1 (PM, 2026-10-19). **Branch:** `claude/desktop-s1-session-boot`. **Base:** `main` @ `35930c52` (DL-1 #57).

## What moves

Every state change the WM shows now plays as a short transition, ending on the frame it showed before:

| Change | Where in the WM | Kind | Full | Reduce Motion |
|---|---|---|---|---|
| Arthur launched from Home | launch `HANDOFF` present | `open` | 12 frames, iris grows from the window | 6-frame cross-fade |
| Arthur exits → Home | `return_home` | `close` | 10 frames, iris shrinks into the window | 6-frame cross-fade |
| Hold / resume / the Lens view | `hold`, `resume`, `lens::view` | `view` | 10-frame cross-fade | 6-frame cross-fade |
| The Lens shown | `show_lens(true)` | `bloom` | 12 frames, iris grows from the Lens halo | 6-frame cross-fade |
| The Lens hidden | `show_lens(false)` | `unbloom` | 10 frames, iris shrinks into the halo | 6-frame cross-fade |
| Focus light on/off or moved | launch focus, pointer/keyboard focus switch, Arthur exit, Home's chip selection | `focus` | 8-frame cross-fade | 6-frame cross-fade |

Window moves and resizes are direct manipulation. They follow the pointer and never animate. Typing never animates.

## The mechanism: one primitive, the old present as its last frame

Every hook site already did the same two things: repaint the base for the new state, then present some rects. DM-1 replaces only
that final present, with `Scene::transition(kind, anchor, rects, client, finals)`:

1. **`F`** is what is on screen now: the WM's output plane over `rects`, copied once.
2. **`T`** is the change composed, not presented: `compose(client, r)` for each rect, copied once.
3. **Frames `1..N−1`.** `neo_atrium::motion::frame` blends `F → T` over each rect, then commits and presents that rect alone. The kernel
   keeps one pending damage rect per present, so a disjoint set (the focus light's six strips) is presented rect by rect. Neither the
   WM nor the kernel touches a pixel outside `rects`.
4. **Frame `N`** is the site's own `present(client, final)` calls, unchanged. **The end frame is therefore the motion-off frame by
   construction,** provided every animated rect lies inside one of those presents. The boot-test witness `uncovered` checks that
   geometrically at every transition. `end_mismatch` compares each rect pixel after the final present with `T`. Since both come
   from the same `compose`, it only catches state drift during the frames; it is not the left-nothing-behind proof
   (AAG lens 1, F2).

Motion off (feature `desktop-motion` off) leaves only step 4. `transition` is then exactly the presents it replaced: the kill-switch.
Two sites gather their repaints before presenting: `move_selection` and `repaint_lens` used to interleave repaint and present per
region. Their regions are disjoint, so every present shows the same pixels. Only the order of base writes between the two presents
changed.

### Spec v1.7 "Motion" (`neo_atrium::motion`, normative)

- **Weight:** `a(k) = 256 − ⌈256·(N−k)³/N³⌉`, an ease-out cubic. It is integer and reaches 256 only at `k = N`; an earlier floor draft hit 256
  one frame early, wasting a frame on a repeat.
- **Blend:** each byte becomes `(f·(256−a) + t·a + 128) >> 8`. At `a = 256` this is `t` exactly, and where `F = T` the pixel never moves.
- **Iris:** centred on an anchor (the window's focus-light rect, or the Lens halo). Its half-extent is `⌈h·s/256⌉` of the reach to
  the far side of the rects' bounding box. Grow uses `s = a`, with pixels outside the iris showing `F`. Shrink uses `s = 256 − a`, with pixels outside
  showing `T`.
- **Pacing:** frame `k` is shown no earlier than `t0 + (k−1)·16.67 ms`. A slow frame delays the rest, and no frame is ever
  dropped, so the frame count is exact and oracle-checkable.

**Reduce Motion:** no spatial movement, only a short cross-fade (6 frames, about 83 ms paced). It comes from the persistent
preference file `/neofs/atrium-reduce-motion` (read at boot, like Reduce Transparency's) or the build feature `reduce-motion`.

## The kernel change (small, gated)

The desktop kernel had no clock and no yield: `SYS_CLOCK_GETTIME` and `SYS_SCHED_YIELD` were compiled only with
`browser-runtime` or DI-1a's `atrium-pointer-forward`. The first QEMU run hung in the launch transition: the clock returned
`ENOSYS`, so a deadline loop never ended. DM-1 also compiles both syscalls under `desktop-session`; it is the same precedent that
gave them to DI-1a.

- **Bounded:** the clock is a relaxed load of the 100 Hz tick counter, and the yield is the scheduler's existing `yield_now`. No new
  cross-CPU code, lock or wake path. Pitfall bank: no CP class is introduced or touched.
- **Ambient:** in a desktop kernel both are ambient syscalls for *every* process (Arthur, the Lens, spawned modules), not only
  the WM: no capability gates them. Like DI-1a's identical exposure, the risk is low: a 10 ms tick clock, and a yield that is a
  no-op on an empty ready queue.
- **Kill-switch:** with `desktop-session` off (and `browser-runtime` / `atrium-pointer-forward` off, as before), both return
  `ENOSYS` exactly as before.
- **The WM never hangs on its clock:**
  - With no clock it skips pacing, and the witness (`elapsed_ms < paced_ms`) fails the scenario loudly.
  - A clock that stops advancing bounds each frame's wait at 100 000 yields (AAG).

## Discriminators

**Host** (`neo-atrium/src/tests.rs`, against `scene_oracle.py motion-fixture` → `fixtures/desktop-dm1/motion_digests.rs`, written
from the spec text with numpy masks and per-byte arithmetic):

- **Plans and weights:** every plan (profile × kind) and the weight table for each frame count equal the oracle's. Weights strictly
  rise and reach 256 once.
- **Blend:** a hand-laid byte oracle; the two-lane form equals the per-byte formula; `a = 256` gives `t` and `a = 0` gives `f`.
- **Iris:** a hand-derived byte oracle; Grow ends covering the bounds and Shrink ends empty.
- **Synthetic frames:** every shape at frames 1/4/6/11/12 over two rects equals the oracle's digest. The destination starts as a fill,
  so a write outside the rects changes the digest. Frame 12 is the target.
- **Mid-hold:** the real scenes, window view focused → Home held, at frame 3 equal the oracle for both profiles. The halfway frame
  could not separate the profiles, because every `n` gives `a = 224` there.
- **Mutation check:** offsetting the blend's rounding by one fails the blend, synthetic and mid-hold KATs.

**QEMU** (`desktop_motion_boot`, `desktop_motion_rm_boot`: `desktop_lens_boot`'s session with motion):

- **Mid frame:** the first Esc's hold stops at frame 3 (`DESKTOP motion hold kind=view profile=… frame=3/N`). The WM has the
  keyboard there, so it waits for one key. The harness screendumps first, then sends `m`, which the WM consumes and Home never
  sees. The capture must equal the oracle's mid-hold digest.
- **End frame:** the capture after the Lens bloom is held to the same oracle as `desktop_lens_boot` (scene digest, exact labels, all
  144,000 Lens pixels).
- **Every transition:** each logs
  `DESKTOP motion kind profile frames rects presented frame_px screen_px elapsed_ms paced_ms early uncovered end_mismatch`.
  xtask holds every line to the following:
  - `frames` equals the oracle's plan.
  - `presented = (frames − 1)·rects`: rect presents actually made, so no frame is skipped (AAG lens 3, M-1).
  - `0 < frame_px < screen_px`: bounded damage, never a full-screen redraw.
  - `early = 0` (no frame shown before its own deadline), and therefore `elapsed_ms ≥ paced_ms`. Every frame is paced, not only the
    total (AAG lens 3, M-2).
  - `uncovered = 0`: every animated rect is inside the end frame's presents.
  - `end_mismatch = 0`.

  All six kinds must have run. The client-damage band check (S2-F2) also covers the motion scenarios, which exercise the split
  `present`.
- **`desktop_full_boot`:** `desktop-full` carries motion. The same per-transition check runs over every transition of the DL-1
  session, and its dock capture is then an end frame.

## Budget (measured)

From the local QEMU runs (`NEO_SMP=4`, pure TCG, 1920×1080 = 2,073,600 px):

| Kind | Rects | `frame_px` (per frame) | Share of screen | Full: frames, paced → measured | Reduced: frames, paced → measured |
|---|---|---|---|---|---|
| open | 5 | 648,888 | 31 % | 12, 183 ms → 1030 ms | 6, 83 ms → 520 ms |
| view | 5 | 648,888 | 31 % | 10, 150 ms → 780–1410 ms | 6, 83 ms → 550–1270 ms |
| close | 5 | 648,888 | 31 % | 10, 150 ms → 770 ms | 6, 83 ms → 560 ms |
| bloom | 2 | 179,856 | 9 % | 12, 183 ms → 350 ms | 6, 83 ms → 120 ms |
| unbloom | 2 | 179,856 | 9 % | 10, 150 ms → 200 ms | 6, 83 ms → 140 ms |
| focus | 1–7 | 87,696–413,256 | 4–20 % | 8, 116 ms → 130–730 ms | 6, 83 ms → 100–590 ms |

- **No transition recomposes more than a third of the screen per frame.** The largest is the view switch's five regions. The focus
  light's six disjoint strips are 157k px, where their bounding box, the old single present, spans most of the screen.
- **Under TCG** a 31 % frame (blend, compose and present of each rect) costs about 80–140 ms, so the full-screen-share transitions
  run 5–9× their paced length. The gate does not bound this; see the limitations.
- **On silicon,** the same work is a few milliseconds per frame. S3 measured the full-screen compose at `full_frame_cycles`
  ≈ 4.5e9 TCG cycles in this same run. The paced length is the expected duration there; that measurement is an acceptance item.
- **Scenario time:**
  - `desktop_motion_boot`: 147 s.
  - `desktop_motion_rm_boot`: also under budget.
  - `desktop_full_boot` with motion: 169 s.

  The budget for these three is 300 s (the old 180 s left too little headroom).
- **Every transition** across the three scenarios logged `end_mismatch=0`.

## Limitations

- **Waiting is a yield loop.** The WM holds no timer object; each frame waits in a `sched_yield` loop on the 10 ms clock (the
  desktop's existing idle style, which OT-22's readiness multiplex will replace). The 10 ms resolution quantizes the 16.67 ms
  period, so frames land on tick boundaries.
- **The time budget is a lower bound in the gate.** Under TCG a frame can take longer than its period. The gate asserts pacing
  (`elapsed ≥ (N−1)·P`) and the exact frame count, not an upper wall-clock bound, which would make the gate depend on the runner
  (hermeticity). The real-silicon number is an acceptance measurement.
- **Input waits during a transition.** Transitions are synchronous. Keys queue on the new owner after its discard (the route
  already moved under `HANDOFF`; the queue holds 64) and are read when the transition ends. Pointer records follow the queue's
  existing policy: motion-only reports coalesce, while button transitions are kept.
- **The client does not animate.** Arthur's and the Lens's surfaces appear with the view (open, bloom); the motion is the chrome's.
  At Arthur's exit, the client's last picture fades with the focus light.
- **Reduce-Transparency scenes with motion** are not run in QEMU; the blend is material-independent and proven on the host.
- **Mid frames in QEMU:** only the hold, a View dissolve at frame 3, is captured against the oracle, and that capture masks Arthur's
  client (about 13 % of the screen, where Arthur fades into Home), because the oracle has no model of Arthur's pixels.
  - **Grow and Shrink** (open, close, bloom, unbloom) are proven on the host: the synthetic oracle cases plus the iris byte
    oracle.
  - **In QEMU** they are held only to their present count, pacing, bounds and end frame.
  - **Still unverified on hardware:** a wrong anchor rect at a call site would pass both.
- **The oracle's iris** (`m_iris`) is close to a transliteration of the Rust, whereas its blend and frame are genuinely
  independent. The spec text was re-derived by hand (AAG lens 3); the Codex cross-check is the cross-family remedy.
- **The kill-switch run of `desktop-full`:** `desktop-full` now carries motion, so no QEMU scenario runs the docked-Lens-plus-resize
  combination with motion off (AAG lens 1, F1).
  - The cfg is the switch, and motion off is gated for home, lens, lens-pointer and resize separately.
  - A `desktop-full`-without-motion scenario was not added; the gate's runners are the binding constraint.
- **`motion_off_is_the_present_alone`** checks `Profile::Off`, which the WM never selects (the real switch is the cfg). It pins the
  plan table's one-frame entry, not the kill-switch.
- **The `/neofs/atrium-reduce-motion` preference path** is not run in QEMU; only the build feature is (Reduce Transparency's
  preference has the same gap).
