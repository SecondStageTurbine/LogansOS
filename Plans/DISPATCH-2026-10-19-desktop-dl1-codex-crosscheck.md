<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex cross-check of DL-1 (the Lens docked to a moving, resizing window) → a Codex seat (Chad or Janus)

**Type:** review only (no code to merge). **Deliverable:** one pushed RETURN. **Why a Codex seat:** the author is
Claude-family. DL-1 changes the clamp math and the WM's repaint/damage on every frame change, which is exactly where
a same-family review shares blind spots (CLAUDE.md §Cross-model diversity). Either Codex seat qualifies: Chad found
DS-1a's handoff defects, and Janus cross-checked DI-1b.

## What is under review

Branch `claude/desktop-dl1-docked-lens`, one commit on `main` @ `d41286eb` (after #56). The PM names the head SHA when
relaying. Review `git diff origin/main...origin/claude/desktop-dl1-docked-lens`. Design:
`docs/THINK-2026-10-19-dl1-docked-lens.md`. Spec: `userland/neo-atrium/src/lib.rs` "Docked Lens" (v1.6).

**Layout** (`userland/neo-atrium/src/layout.rs`):
- The Lens has a fixed `LENS_H = 400`; DS-1c took Arthur's client height.
- `lens_docked` is set while the Lens is shown.
- `clamp_frame` and `resized` clamp the *group* (frame + 32 + Lens frame 376 + 16 margin; height
  `max(frame.h, 442)`).
- `bloom_origin` is the place the bloom slides the window to.
- `docked_reach` is the Lens halo a frame change repaints.
- `clamp_origin` uses `min().max()` in place of `clamp()`.

**Scene** (`scene.rs`):
- `place_frame` lets a shown Lens follow, re-blurring its glass. A place the Lens could not follow is refused.
- `set_lens` sets and clears the docked flag.
- `with_lens_at` is new.

**WM** (`userland/neo-init/src/desktop_session.rs`):
- The `compile_error!` is removed.
- The title band and resize handles are live while a Lens is shown.
- `move_window`, `apply_configure` and `reset_client_size` repaint through `reach` / `repaint_reframed`, covering the
  focus light and the docked halo, old and new.
- `slide_for_lens` runs before `lens::bloom`, inside `launch::serve`, while Arthur waits on its `OP_LAUNCH` reply.
- `count_move_blurs` counts frame-change blurs, the bloom's stale re-blur included, so none is a "re-blur during
  typing".
- The move witness now logs after the present.
- The Home-return blur total is no longer capped at one digit.

**Arthur:** the Lens boot-test loop polls the configure mailbox.

**Feature:** `desktop-full = ["desktop-lens-pointer", "desktop-resize"]`, scenario `desktop_full_boot`.

**Discriminators:**
- **Host** (`neo-atrium/src/tests.rs`, `fixtures/desktop-dl1/dock_digests.rs` from `scene_oracle.py dock-fixture`):
  - `docked_lens_matches_independent_python_oracle`;
  - `docked_group_clamps_match_the_spec`;
  - `undocked_layout_is_di1b_exactly`;
  - `a_shown_lens_follows_the_window_and_a_moved_lens_reblurs_once`.
- **QEMU** `desktop_full_boot`:
  - a pre-bloom move, then the slide on bloom;
  - a docked move with `lens_mismatch=0`;
  - a docked corner resize with the Lens following the right edge;
  - the capture against the dock oracle, with exact Lens labels (`fixtures/desktop-dl1/dock_labels.rs`) and exact Lens
    content at the docked origin.

**Changed after the Claude AAG** (re-check these):
- **Docking is compiled only with `desktop-full`** (Lens and resize together). `desktop-lens-pointer` alone keeps the
  window fixed while a Lens is up, and does not slide.
- **A stale configure is re-placed.** A configure published before the bloom docked the Lens is re-placed with
  `Layout::clamp_frame_for` inside the docked group, instead of being refused, which used to end Arthur's session.
- **Window-view assert.** `repaint_reframed` asserts it runs in the window view.
- **DS-1c tuple check restored.** The DS-1c capture check again compares the Lens client's full tuple against the
  content oracle.
- **Merged with #56.** `show_lens(bool)` and the lent-buffer `LensStore` now carry the docked Lens.

## Your job: refute it

1. **Clamp math.**
   - Find a frame, size or drag (any edge or corner, any screen size) where a docked move or resize lands somewhere
     the Lens does not fit.
   - Look for a panic: usize underflow in `wf.right() - …`, or i64 ranges with `min > max`.
   - Look for a non-docked result that changed (the DI-1b kill-switch).
   - Is `scene_oracle.py`'s `dock_move`/`dock_resize` independent of the Rust, or a transliteration that would share
     a bug?
2. **Repaint and damage.**
   - Can any frame change leave stale pixels: the old Lens halo or the old Lens client?
   - Does the halo rect cover the Lens client and frame?
   - Can a present miss the moved Lens layer?
   - Can the slide run while the Home view is shown?
   - A move or resize while the Lens is *selected* (Arthur shown unfocused): right focus?
   - `slide_for_lens` succeeds but `bloom` then fails: the window is moved with no Lens. Acceptable?
3. **Ordering.** The slide presents inside `serve`, while Arthur waits for its `OP_LAUNCH` reply. Can that block, or
   reorder anything Arthur sees? It does not see positions.
4. **Kill-switch.** With `desktop-full` off, are `desktop_lens_*`, `desktop_resize_boot` and `desktop_pointer_boot`
   bit-for-bit what they were? Check the Arthur change (resize-only cfg), and the WM changes outside any cfg.
5. **Coverage.** What does the gate not cover? The THINK lists:
   - RT docked only on the host;
   - a QEMU docked move that does not hit the clamp;
   - Lens-selected moves;
   - 1280x800 blooms now sliding.

   Anything else?

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as notes.
"No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    (or E:\AI Automation\workers\janus) — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/desktop-dl1-crosscheck-<seat>
          (git fetch origin && git checkout -B codex/desktop-dl1-crosscheck-<seat> origin/claude/desktop-dl1-docked-lens)
```

```
git fetch origin
git show origin/claude/desktop-dl1-docked-lens:Plans/DISPATCH-2026-10-19-desktop-dl1-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-dl1-docked-lens
```

Optional runs:
- `cargo test -p neo-atrium -p xtask`.
- `python3 scripts/desktop/scene_oracle.py dock-fixture` must reproduce `fixtures/desktop-dl1/dock_digests.rs`.
- `python3 scripts/desktop/lens_content_oracle.py dock-labels` must reproduce `fixtures/desktop-dl1/dock_labels.rs`.
- `cargo xtask neo-test desktop_full_boot desktop_resize_boot desktop_lens_pointer_boot` at `NEO_SMP=4`. Set
  `LIMINE_DIR`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='WM window move resize docked lens repaint clamp'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-desktop-dl1-codex-crosscheck-<seat>.md` to your branch and **push it**. It must
contain:
- a verdict per numbered item;
- each finding with severity, file:line, and a repro or reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never delete the
worktree.
