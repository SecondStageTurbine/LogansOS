<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Codex cross-check of DL-1 (Janus)

**Branch reviewed:** `origin/claude/desktop-dl1-docked-lens` @ `80305241`

**Merge base:** `d41286eb`

**Verdict:** **BOUNCE — one blocking High finding.** A stale configure that was legal while the Lens was undocked can be wider than the docked group can fit. On a 1280x800 output the ack reaches an unchecked `usize` subtraction in debug builds; in release it wraps, the re-place is refused, and Arthur's session ends with error 92. This contradicts DL-1's stated stale-configure closure.

## Finding

### DL1-X1 — High (blocking): an undocked pending configure can become impossible after bloom

**Locations:** `userland/neo-init/src/desktop_session.rs:608`, `userland/neo-init/src/desktop_session.rs:517`, `userland/neo-atrium/src/layout.rs:298`, `userland/neo-atrium/src/layout.rs:562`, `userland/neo-init/src/desktop_session.rs:527`.

`slide_for_lens` decides whether the *current* 640x400 frame can take a Lens. It does not consider a pending configure published while `launch::serve` was polling the Lens proof. After the Lens is shown, `apply_configure` passes that pending size to `Layout::clamp_frame_for`. The latter assumes the newly sized docked group fits, but that invariant does not follow from either the current Lens fitting or the undocked resize being valid.

Reproduction/reasoning:

1. Use a 1280x800 output and an undocked 640x400 Arthur frame at the left clamp, x=98. The current docked group fits, so a bloom is possible.
2. While Arthur's `list /` request waits inside `launch::serve`, release an undocked right-edge resize. The undocked maximum client width is 1120; any pending width above the docked maximum 728 demonstrates the defect.
3. The bloom succeeds for the still-current 640x400 frame and sets `lens_docked`.
4. Arthur receives the launch reply, polls the mailbox and acks (for example) 1120x400.
5. `clamp_frame_for(1120, 400, ...)` constructs a group width of `1136 + 32 + 376 + 16 = 1560`, but the workfield is only 1196 pixels wide. `clamp_origin` evaluates `1280 - 14 - 1560` as `usize`.

I added a temporary host test that performed exactly that call. `cargo test -p neo-atrium janus_repro_stale_configure_too_wide_for_docked_group -- --nocapture` failed at `layout.rs:562` with `attempt to subtract with overflow`; the temporary test was then removed. With release overflow checks off, the subtraction wraps before the `i64` cast, the origin falls back to the low edge, `BaseScene::place_frame` finds that the Lens no longer fits and returns `false`, and `apply_configure` returns fatal error 92 after already clearing the pending configure. Thus the shipped path still ends Arthur's session even when it does not trap.

The existing `stale_configure_is_replaced_inside_the_docked_group` test uses 1920x1080 and client width 1064. That group still fits, so it does not exercise the violated precondition.

## Numbered review verdicts

### 1. Clamp math — FAIL (DL1-X1)

- Normal docked moves and resizes preserve the group: the current docked frame witnesses non-empty resize clamp ranges, including left/right/top/bottom and corners.
- `lens()` and the group clamps leave at least `GLOW` around the group; the Lens halo is smaller than that margin.
- Undocked `lens_docked = false` behavior remains DI-1b's frame-only clamp, and a set docked flag is inert when `lens` is `None`.
- `clamp_origin` itself is not safe for an oversized group. The stale-configure path supplies such a group on narrower supported outputs; see DL1-X1.
- The Python renderer is independent code, but `dock_move` and `dock_resize` are close formula-by-formula translations of the same spec. They catch Rust/Python divergence, not a shared missing precondition. All generated cases are 1920x1080, so they cannot expose DL1-X1.

### 2. Repaint and damage — PASS, with one non-blocking behavior note

- `docked_reach` is the Lens halo. The halo is the Lens frame expanded by four pixels, so it contains the complete Lens frame and client.
- Frame changes repaint and present the union of old/new Arthur glow and old/new Lens halo. The old halo erases the old Lens frame/client from the base; `present(true, ...)` composites the moved Lens layer at its new geometry over the same damage. I found no missed moved layer or stale-pixel region.
- A move retains the same-size focus plane at the new geometry; a resize refills the focus plane for the new size. With the Lens selected, `focused == false` is passed through repaint and the selection remains in the scene's Spine state, so Arthur stays unfocused.
- The slide is reached from Arthur's live window-session `OP_LAUNCH`; Home cannot be the active view there. The window-view assertion is consistent with the call graph.
- **Note:** a late bloom failure after `slide_for_lens` leaves Arthur moved without a Lens. The spec says the bloom first moves the window and does not promise rollback, so I do not classify this as a branch-bouncing defect, but `desktop-full` has no fault-path test for it.

### 3. Ordering — PASS

- The slide's present and fence wait occur while Arthur is synchronously waiting for its `OP_LAUNCH` reply. Arthur observes neither global position nor the intermediate present.
- The wait is the ordinary bounded present path (1000 ms); failure is fatal as elsewhere. I found no IPC dependency from the output fence back to Arthur or the not-yet-spawned Lens, so no deadlock or client-visible reordering follows.
- A resize published during proof polling remains pending until Arthur gets the launch reply and polls its mailbox. That ordering is coherent, but the later size/group compatibility check is the defect in DL1-X1.

### 4. Kill-switch — PASS by code audit, host KATs and feature builds

- `desktop-lens-pointer` without resize still makes the title band/handles inert while the Lens is shown; `slide_for_lens` is not compiled.
- `desktop-resize` without the Lens keeps `lens_docked == false`/`lens == None`, so clamps and repaint reach reduce to DI-1b. The Arthur polling addition exists only inside the Lens boot-test loop and is itself gated by `desktop-resize`.
- The unconditional layout changes are dormant off-composition: fixed `LENS_H` equals DS-1c's 400-row client, and `min().max()` equals the former `clamp()` for valid frame-only callers.
- Clippy passed for `neo-init` with `desktop-lens-pointer`, `desktop-resize`, and `desktop-full`, and for `neo-arthur` with `desktop-resize` and `desktop-lens,desktop-resize`.
- I did not run the optional QEMU scenarios on this Windows seat because no `LIMINE_DIR` is configured (the local default `/home/neo/limine` is absent).

### 5. Coverage — FAIL for the blocking gap; additional gaps noted

The gate covers the advertised 1920x1080 happy path, ordinary host clamp cases, Glass and RT host renders, and exact Lens labels/content. In addition to the THINK's listed gaps, it does not cover:

- a pre-bloom pending configure whose size is legal undocked but cannot fit once docked (DL1-X1);
- supported outputs near the minimum dockable width, including the stated 1280x800 slide composition;
- the real `launch::serve` race in which pointer polling publishes a configure before bloom and Arthur acks only after the launch reply;
- a `desktop-full` late bloom failure after the window has slid;
- WM-level lens-selected move/resize damage and focus (the host scene oracle does not exercise compositor layers or focus handoff).

## Verification performed

- RAG health: healthy (`ollama=true`, `qdrant=true`); semantic discovery used before exact-symbol searches.
- `cargo test -p neo-atrium -p xtask`: PASS — 42 active `neo-atrium` tests passed (2 ignored), 32 `xtask` tests passed.
- `scene_oracle.py dock-fixture`: reproduced `fixtures/desktop-dl1/dock_digests.rs` text-exactly (normalizing Windows stdout encoding/newlines only).
- `lens_content_oracle.py dock-labels`: reproduced `fixtures/desktop-dl1/dock_labels.rs` text-exactly (same normalization).
- Feature clippy matrix described under item 4: PASS with `-D warnings`.
- Targeted temporary 1280x800 stale-configure repro: FAIL as expected at `layout.rs:562`; test removed after confirmation.
- Optional QEMU scenarios: not run; `LIMINE_DIR` is unset on this seat.
