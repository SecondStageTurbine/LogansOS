<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family review of DI-1b (window move and resize by pointer) and the libneo heap fix → a Codex seat

**Type:** review only. **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the change touches the
WM's present path and the userland allocator that every libneo binary shares. The author is
Claude-family, so a different-family lens is wanted before merge. The PL assigns the seat (Jarvis / Janus /
Chad).

## What is under review

Branch `claude/di1b-pointer-move`; the PM names the head SHA. Diff: `git diff origin/main...origin/claude/di1b-pointer-move`.

1. **Placed frame** (`userland/neo-atrium`):
   - `Layout::placed` / `clamp_frame` / `title_band`. `Layout::new` is unchanged: it is placed at the
     centred origin.
   - `BaseScene::move_frame` moves the frame-derived geometry. In Glass it re-blurs the frame region once
     at its new place. The blur samples the static backdrop, so nothing else needs re-blurring.
   - `with_home_at`.
2. **WM drag** (`userland/neo-init/src/desktop_session.rs`):
   - A press on the title band starts `Grab::Move`. While held, an Electric-Blue outline is drawn as a
     present overlay. On release, `Action::Move` → `Scene::move_window`, which repaints the old and new
     focus-light rects from the base, presents their union, and erases the outline. A held click on an
     unfocused window still focuses it.
   - A move cursor (19×19, centre hotspot) shows over the title band and while dragging.
   - The stale `geo` copies in the Arthur loop now read `scene.geo`.
   - **Claude AAG fixes, included:**
     - Forwarded grab coordinates are clamped to the client surface. Before, a grab dragged to the screen
       corner handed Arthur `(-client.x, -client.y)`.
     - `poll` stops draining after the release that produces `Action::Move`, so later records are
       hit-tested against the moved window.
     - The outline is drawn only once the pointer has moved.
   - **Allocator AAG note:** each growth after another mapping costs an arena slot, so there are 64 arenas
     and growth is geometric.
3. **libneo heap** (`userland/libneo/src/lib.rs`, `LibneoHeap`):
   - The heap used to `extend` its arena on every growth, assuming `sys_map_region` returns the region
     right after the heap. `sys_buffer_map` shares that cursor, so after the WM mapped its planes the
     extension aliased the `out` plane. The first runtime blur wrote box-filter sums over the screen:
     hold view `stale_pixels=207360`.
   - Now a region that does not start at the last arena's top opens a new arena (up to 64; growth is
     geometric, at least a quarter of what is owned, capped at 16 MiB). `dealloc` finds the owning arena
     by address range.
4. **Resize** (R1: a max-capacity buffer; feature `desktop-resize` in neo-init and neo-arthur, libneo
   `atrium-configure`):
   - **Geometry** (`neo_atrium::Layout`): `resize_edges` (side/bottom borders, the top 4 rows of the
     title band, 16 px corners), `resized(edges, dx, dy)` (a dragged edge moves, the opposite edge is
     anchored; client 320x200..min(1280x800, what the output fits); frame + focus light inside the
     workfield). `BaseScene::place_frame` generalises `move_frame`: one blur per completed resize.
   - **No WM-initiated send** (CP-57): IPC is a synchronous rendezvous with no non-blocking send, so the
     WM *publishes* the size in a four-word seqlock mailbox (`libneo::atrium_configure`) that Arthur maps
     read-only. Idle, Arthur reads it, re-lays itself out (its 1280x800 buffer is created 640x400) by
     replaying its kept transcript, and acks with `OP_CONFIGURE` on its surface channel. The WM honours
     the ack only if it equals what it published; then the registry bounds damage by the new size, the
     frame takes it, and the WM copies and presents the new picture. One resize at a time; moves wait.
   - Forwarded coordinates clamp to the client size pinned at press. A new Arthur starts at 640x400.

## Discriminators (claimed)

- **neo-atrium host KATs:**
  - the moved scene equals the independent `scene_oracle.py` digest at (500,200) and at the clamp corner
    (98,50), in both materials and both focus states;
  - moved equals built-there, bit-exact, in both views;
  - exactly one blur per completed move, none for a no-op, none in Reduce-Transparency;
  - clamping;
  - skipping the re-blur fails both scene KATs.
- **QEMU `desktop_pointer_boot`, real usb-mouse via HMP:**
  - the cursor goes to the title band and drags by (−120, +60);
  - `DESKTOP pointer move from=674,337 to=554,397 client_at=562,431 blur_regions_total=6 residue_pixels=0`;
  - the next click reaches Arthur at `local=278,19` (the same x as before the move: the client only ever
    sees window-relative coordinates, and nothing it is sent carries a position);
  - a grab dragged 508 px off the client's left edge reaches Arthur as `local=0,19`, clamped;
  - the DS-1 tail's `stale_pixels=0` and the Home oracle digest still hold after the move.
- **Allocator:** without the arena fix, the same scenario fails with `view switch=hold stale_pixels=207360`.
- **Resize, host:** resized scene == `scene_oracle.py resize-fixture` (three frames, both materials,
  both focus states); resized == built there; one blur per resize (skipping it fails the oracle KATs);
  handle hit-test; bounded geometry at 1920x1080 and 1280x800; mailbox seqlock (no torn read in any
  store interleaving); damage bounded by the acknowledged size; Arthur's resize equals typing at that
  size, a wide mapping renders the created surface unchanged, the kept transcript is bounded by whole
  lines.
- **Resize, QEMU `desktop_resize_boot`** (real usb-mouse): a corner drag of (+120, +60) on the moved
  frame publishes 760x460; Arthur acks; `DESKTOP pointer resize from=640x400 to=760x460 frame_at=554,397
  serial=1 blur_regions_total=7 residue_pixels=0 client_mismatch=0 footer_rule=yes`; a click at local
  700,19 (past the old width) reaches Arthur. `desktop_pointer_boot` (resize compiled out) is the
  kill-switch: unchanged.

## Refute it

1. Can the client ever learn its screen position, from any message, coordinate or damage rect?
2. Does any present after a move leave stale pixels? Check the outline erase, sprite erase, a move during
   a client commit, a Home switch after a move, and Destroy after a move.
3. Is the "one blur per move" claim exact? Can a drag re-blur during motion, or skip the blur on a real
   move?
4. **Allocator:**
   - Can `add_region` misjudge contiguity (alignment of `bottom()`, `top()` with `pending_extend`)?
   - Can `dealloc` pick the wrong arena?
   - What happens when all 16 arenas are used?
   - Is any existing caller (Argon2id's 64 MiB multi-region growth) affected?
5. Does clamping keep the frame and its focus light fully on screen at both supported modes?
6. **Resize:**
   - Can a client size itself, or learn its position, through the mailbox or the ack? Can a malformed
     or stale ack crash or wedge the WM rather than end the session?
   - Is the seqlock sound across processes (store/load ordering, a reader that straddles two publishes,
     serial wrap)?
   - Any deadlock left between the WM and Arthur (the ack is a request; can Arthur be parked where the WM
     needs it to receive)? A configure outstanding across Esc/hold/resume, Launch, or Destroy?
   - Does every composed pixel come from the right size (client layer width/stride, the glow plane's
     stride, the registry's damage bound, the snapshot clip)?
7. Anything else a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/di1b-move-crosscheck-<seat>       (git fetch origin && git checkout -B codex/di1b-move-crosscheck-<seat> origin/claude/di1b-pointer-move)
```

```
git fetch origin
git show origin/claude/di1b-pointer-move:Plans/DISPATCH-2026-10-18-di1b-move-codex-crosscheck.md
git diff origin/main...origin/claude/di1b-pointer-move
```

Optional runs, at `NEO_SMP=4`: `cargo test -p neo-atrium`, `cargo test -p atrium-logic-tests`,
`cargo test -p arthur-logic-tests`, `cargo xtask neo-test desktop_pointer_boot`,
`cargo xtask neo-test desktop_resize_boot`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='WM window move title band drag present'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-di1b-move-codex-crosscheck-<seat>.md` to
`codex/di1b-move-crosscheck-<seat>` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
