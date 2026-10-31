<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of Desktop S3 (Atrium glass material + craft chrome) → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the
author and every AAG lens on this branch are Claude-family (memory-safety, oracle independence, completeness,
perf), so their agreement is weak evidence (CLAUDE.md §Cross-model diversity). The single most important
question here is **oracle independence**: the Rust renderer and the Python scene oracle were written by the
same author from the same spec text, which is exactly the shared-blind-spot setup a different family exists to
break. The PL assigns the seat (Jarvis / Janus / Chad).

## What is under review

Branch `claude/desktop-s1-session-boot` (the session's designated branch; the S3 work lives there, restarted
from `main` after S1 #25 merged). The PM names the exact head SHA in the PR comment that relays this dispatch;
review the whole diff at that head (`git diff origin/main...origin/claude/desktop-s1-session-boot`).

Desktop S3, CPU path: the WM (`neo-init`, feature `desktop-glass`) paints an ambient Threshold backdrop and
frosted glass on three surfaces (the craft bar, the Spine, Arthur's window frame/title band), with Arthur's text
area opaque. Reduce Transparency (RT) is both the material kill-switch and the accessibility floor.

- **`userland/neo-atrium/` (new, `no_std` + `alloc`, no syscalls)** — the lib.rs doc comment is the normative
  *Scene specification v1*; the oracle implements that text, not the Rust.
  - `wide.rs`: 8.8 fixed-point colour, 4×4 Bayer dither `min(255,(W+16·B+8)>>8)`, `mix`, `lighten`.
  - `backdrop.rs`: Void→Threshold vertical rise, two radial lights (`f=q²>>16`), two flare-star rings with a
    1.5 px triangular profile (`s=trunc(8(d²−R²)/R)` in 1/16 px). Integer only, i64.
  - `blur.rs`: three passes of a separable integer box (radius 8, H then V per pass), edge-clamped at a source
    region = glass box + 24 px margin clipped to the screen; `out=(Σ+R)/(2R+1)`. Every region blur bumps a
    counter (`blur_calls`).
  - `material.rs`: `Mode {Glass, ReduceTransparency}`; `TrustState` tint (Owned: frost 18, Electric-Blue
    vibrancy 26/256 ≈ 10 %; other states carry structure, not hue); the Cap-Halo cue is hue-independent.
  - `shape.rs`: exact-area corner coverage (closed-form ∫√(r²−t²)), `round_half_up(255·A)`; hairline =
    coverage − inset-1 coverage.
  - `layout.rs`: geometry; `scene.rs`: `BaseScene` (blurs computed once in `new`), `paint_rect` (damage
    repaint), labels via `neo-text` (Plex Sans), focus light as an alpha SDF glow (no drop shadow).
- **`userland/neo-init/src/desktop_session.rs`**: chrome type switches on `desktop-glass`; RT chosen by the
  `reduce-transparency` feature or the NeoFS file `/neofs/atrium-reduce-transparency`; RDTSC frame timing;
  `reblurs_during_typing` logged at surface destroy.
- **Kernel (neo-test only):** `fb::pixel_digest_masked`, scenarios `desktop_glass_boot` /
  `desktop_glass_rt_boot` (`desktop_glass_asserts` in `kernel/src/test_harness.rs`). The digest masks the
  three label boxes; in RT the bar and title labels are then checked exactly against the font-area oracle
  windows (flat panel), in glass only for ink. Kernel concurrency is not touched — the pitfall bank was
  judged N/A; challenge that if you disagree.
- **xtask:** the two scenarios, `run-desktop --reduce-transparency / --classic`, `NEO_DESKTOP_MATERIAL`, and a
  fix to `ppm_bright_pixels` (it skipped *all* whitespace after the PPM header, eating a first raster byte of
  0x0A — the glass backdrop's top-left pixel — so every glass capture read as "not a P6").
- **CI:** clippy `-D warnings` on the shipped `desktop-glass` and `reduce-transparency` neo-init builds.
- **Oracles:** `scripts/desktop/scene_oracle.py` (numpy; cumulative-sum box blur, its own corner-area
  integral) → `fixtures/desktop-s3/scene_digests.rs` (8 FNV-1a digests, label boxes masked);
  `scripts/fonts/area_oracle.py` gained a stem-darkening mode → `fixtures/desktop-s3/*_label_rt.rs`;
  hand-laid KATs in `userland/neo-atrium/src/tests.rs`.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Oracle independence (the core question).** The Claude AAG found the oracle had taken ~9 design
   conventions (geometry, constants, hairline/tick/glow rules) from the code rather than the spec text;
   they are now stated in spec v1.1 (`lib.rs`), and the claim is narrowed: the oracle catches a wrong
   *algorithm*, not a wrong *design constant*. Challenge whether that narrowing is honest. List every convention the Python oracle shares with the Rust
   that is *not* forced by the spec text, and judge whether a wrong renderer could pass because of it.
   Candidates: the blur source margin and clip, the H-then-V pass order, the `+R` rounding, the dither
   threshold, the ring truncation direction (`trunc` vs `floor` for negative `d²−R²`), the coverage
   `round_half_up`, the label-box masks (the digest excludes the text boxes — what wrong pixels can hide
   there, and does the ink test close it?). Re-derive at least one blur output pixel and one corner-mask byte by
   hand. Is the scene spec itself a good design (would you choose these numbers), or only self-consistent?
2. **"Typing never re-blurs."** The witness is a counter read at surface destroy. Can a damage path re-blur
   without bumping it, or can a re-blur occur after the read? Does any path rebuild `BaseScene` (resize, focus,
   RT toggle)? Is per-frame cost actually independent of blur (re-read `paint_rect`)? A focus change repaints
   only `Layout::focus_regions()` (the glow ring around the client, the Spine, the bar label) — is any
   focus-dependent pixel outside them (the host test `damage_repaint_matches_full_paint` claims not)?
3. **Memory safety and panics** (neo-init is the resident WM; a panic kills the desktop). Indexing in
   `Blurred::at` (callers outside the blurred box?), `paint_rect` with rects partly off-screen, `Layout::new` at
   the minimum size (800×538) and odd GOP modes, u32 overflow in the blur sum (`(2R+1)·65535`, three passes),
   `lighten`/`mix` at 65535, i64 in the backdrop at 8K, debug-build overflow checks.
4. **Kill-switch.** Glass compiled out must be bit-for-bit S1: `desktop_session_boot` still asserts S1's exact
   readback. Verify no S1 code path changed under `cfg(not(feature = "desktop-glass"))`. RT must be
   deterministic and pinned (`S3_RT_*` digests); confirm RT does no blur and no backdrop-dependent sampling in
   the chrome.
5. **Accessibility.** RT contrast ≥ 7:1 is asserted for the label colours on `RT_PANEL`/`RT_RAISED`. Is the
   glass (non-RT) title label legible on the brightest backdrop under the frame (compute the worst case)? Is
   the Cap-Halo trust cue truly hue-independent (a deuteranope/greyscale reading)?
6. **Perf numbers.** The PR reports RDTSC cycles for the full 1920×1080 frame and per typing damage frame.
   Are they measured honestly (TSC under TCG is not wall time; what does that make the numbers worth)? Is
   there a cheaper equivalent blur (running sums are already used — what else)?
7. **Visual quality.** Look at `docs/screenshots/desktop-s3-glass.png` and `desktop-s3-rt.png`. Banding,
   halo artefacts at the blur clip edge, corner seams, glow clipping at `GLOW` (14 px), label baseline. Say
   plainly if it looks wrong.
8. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/desktop-s3-crosscheck-<seat>       (git fetch origin && git checkout -B codex/desktop-s3-crosscheck-<seat> origin/claude/desktop-s1-session-boot)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-18-desktop-s3-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Building and running are optional.
- **Host checks:** `cargo test -p neo-atrium` (13 tests). Preview:
  `NEO_ATRIUM_PREVIEW=out.ppm cargo test -p neo-atrium preview -- --ignored`.
- **Oracle:** `python3 scripts/desktop/scene_oracle.py fixture > /tmp/d.rs` must reproduce
  `fixtures/desktop-s3/scene_digests.rs` byte-for-byte (needs numpy).
- **QEMU:** `cargo xtask neo-test desktop_glass_boot desktop_glass_rt_boot desktop_session_boot` at
  `NEO_SMP=4`.
- **Mutation checks:** change the blur rounding (`+ r` → `+ 0`) or the ring `trunc` to `floor` — the
  independent-oracle test must FAIL; call `blur_region` inside `paint_rect` — the blur-once test must FAIL.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='desktop glass blur damage repaint WM scene'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-s3-codex-crosscheck-<seat>.md` to `codex/desktop-s3-crosscheck-<seat>`
and **push it**. A review delivered only in chat does not exist. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line, and a repro or reasoning;
- for each numbered item above, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
