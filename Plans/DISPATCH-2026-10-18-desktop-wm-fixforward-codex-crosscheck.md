<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check of the desktop WM fix-forward for S1-F2, S2-F2 and the S3 label-mask finding → Chad

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why Chad:** these are fixes for
three of Chad's own findings (`Plans/RETURN-2026-10-18-desktop-s1-codex-crosscheck-chad.md` finding 2,
`Plans/RETURN-2026-10-18-desktop-s2-codex-crosscheck-chad.md` finding 2, and the MEDIUM in
`Plans/RETURN-2026-10-18-desktop-s3-codex-crosscheck-chad.md`). The author is Claude-family, so the seat
that found the defects is the right cross-family lens to say whether they are closed.

## What is under review

Branch `claude/desktop-s1-session-boot` (restarted from `main` after DS-1b merged). The PM names the head SHA when
relaying. Review `git diff origin/main...origin/claude/desktop-s1-session-boot`.

**S1-F2 — no ordinary slice over client-writable pages.**
- `libneo::atrium_wm::SharedPixels` (`userland/libneo/src/atrium_wm.rs`): a raw view over a client's mapping. Every
  read is a relaxed `AtomicU32` load. `snapshot(rect, dst)` copies a clipped rect into WM-owned storage.
- Desktop WM (`userland/neo-init/src/desktop_session.rs`): `Scene::client_copy` is a WM-owned plane. `commit()` copies
  exactly the committed rect into it and presents it. Layers point only at WM-owned planes. The `from_raw_parts` over
  Arthur's mapping is gone.
- DW-1/DW-2 test-harness WM (`userland/neo-init/src/dw1_desktop_wm.rs`): copies each 32×32 client to the stack before
  composing or hashing.
- Host tests: `tools/atrium-logic-tests/src/shared_pixels_kats.rs`. The second test runs a concurrently-mutating
  writer thread: every copied pixel must be a value the writer stored, and the WM's copy must stay stable after it
  is taken.

**S2-F2 — Arthur's band damage reaches the desktop WM.**
- New ADR-073 verb `COMMIT_DAMAGE` (opcode 9, feature `atrium-damage`; the table is in ADR-073 §"Desktop extension
  verbs"). Payload: `client_serial:u64@48`, then `x, y, w, h` as `u16` at 56/58/60/62.
- `SurfaceRegistry::commit_damage_on_channel` binds the commit to the caller's own channel. The rect must lie inside
  the surface as created, or the request gets `BadField` and nothing commits.
- Arthur (`showcase_surface.rs`, feature `desktop-session`) sends it with the rect it already passes to
  `sys_buffer_commit_damage`. The standalone showcase keeps `OP_COMMIT`.
- The WM translates the rect to output coordinates and presents only that rect.
- Byte oracle: `commit_damage_byte_oracle`. Registry KAT: `registry_commit_damage_is_channel_bound_and_bounded`.
- QEMU witness: every desktop scenario logs `DESKTOP client damage first=WxH at=X,Y band=…`. xtask requires
  `band=yes`, meaning inside the client rect and shorter than it. A local run logged
  `first=592x24 at=700,482 band=yes`.

**S3 MEDIUM — exact labels instead of broad masks.**
- `scripts/desktop/label_oracle.py` produces `fixtures/desktop-s3/labels.rs`. For each of the five label runs (bar
  "Atrium", title "Arthur", **"· owned"**, chip "A", chip "Arthur") it derives:
  - a tight glyph window;
  - per-pixel [lo, hi] bounds from exact-area coverage (`scripts/fonts/area_oracle.py`, ±2/255, stem darkening),
    blended over the independent scene oracle's own background, so Glass gets its real non-flat frost and RT its
    flat panels;
  - the unfocused 1920×1080 scene digest with **only** those windows masked.
- The kernel readback in `desktop_glass_boot` and `desktop_glass_rt_boot` asserts both: the digest, and every pixel of
  every window against its bounds. `kernel/src/test_harness.rs`, `desktop_glass_asserts`.
- Host test `labels_match_exact_oracle_and_deletions_fail` (`userland/neo-atrium/src/tests.rs`) adds negative
  controls: erasing "· owned" or the chip's "Arthur" must fail. Two code-level mutations were also run and both
  failed the test: deleting the "· owned" `draw_line`, and changing the chip name to "Arthor".

## Your job: refute it

1. **S1-F2 closed?** Is any Rust reference over client-writable memory left anywhere in the WM or `libneo`? Grep for
   `from_raw_parts` and for `Layer`/`SceneSurface` `pixels:` sources. Is `SharedPixels::new`'s safety contract met at
   each construction site, given that construction happens after descriptor validation? Is a relaxed atomic load the
   right primitive here? Compare volatile reads and the cross-process data-race model. Can a hostile writer still
   affect anything but its own pixels: indices, lengths, control flow?
2. **S2-F2 closed?** Is the damage rect validated before it is used for indexing? Look at u16 overflow in
   `x + width`, clipping in `snapshot`, and the translation into output coordinates. Can a client name damage that
   makes the WM present pixels outside its client rect, or skip presenting pixels it changed? Note that
   under-reporting only stales the client's own surface; confirm that is the only effect.
3. **Protocol.** Check the byte layout against the hand-laid oracle independently. Is `OP_COMMIT` unchanged for every
   other client (DW-1/DW-2 workers, the Lens later, the standalone showcase)? Check INV-WM-1: can a client damage-commit
   a peer?
4. **Witness strength.** Does `band=yes` actually prove a keystroke presented only a band, or could it pass for the
   wrong reason? It checks only the first damage commit. Would a WM that ignored the rect and presented the whole
   client be caught?
5. **Kill-switch / regressions.** S1's exact full-screen readback and S3's oracle digests must be unchanged, since the
   pixels are the same and only the copied and presented area shrinks. DW-1/DW-2 hashes must be unchanged too.
6. **S3 labels closed?** Is the label oracle independent of neo-text and neo-atrium? It imports `area_oracle` and
   `scene_oracle`, not the Rust. Re-derive one window's placement, including the `"· owned"` x =
   title x + measure("Arthur", 14) + 8. Can a wrong label still pass: different text with similar ink, a wrong
   colour within ±2/255, a shifted run inside its 1-px margin? Is anything outside the five windows still unchecked?
   Home's labels and the typed field text remain masked by design; say whether that is acceptable.
7. **Anything else**, including whether the snapshot copy cost moved somewhere worse.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/desktop-wm-fixforward-crosscheck-chad
          (git fetch origin && git checkout -B codex/desktop-wm-fixforward-crosscheck-chad origin/claude/desktop-s1-session-boot)
```

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-18-desktop-wm-fixforward-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Optional runs: `cargo test -p atrium-logic-tests -p neo-atrium`;
`python3 scripts/desktop/label_oracle.py fixture` must reproduce `fixtures/desktop-s3/labels.rs` byte for byte;
`cargo xtask neo-test desktop_session_boot desktop_glass_boot desktop_home_boot dw1_desktop_wm dw2_scene_policy` at
`NEO_SMP=4`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='compositor reads client shared pixel buffer'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-wm-fixforward-codex-crosscheck-chad.md` to
`codex/desktop-wm-fixforward-crosscheck-chad` and **push it**. Contents: a verdict for each of S1-F2, S2-F2 and the S3
label finding (CLOSED / PARTIAL / OPEN); each finding with severity, file:line and a repro or reasoning; for each numbered item,
what you checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** a review lane. If you built, `cargo clean` your worktree afterwards; never delete the worktree.
