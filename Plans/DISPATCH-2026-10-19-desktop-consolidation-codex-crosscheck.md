<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex cross-check of the desktop consolidation build → a Codex seat (Chad or Janus)

**Type:** review only (no code to merge). **Deliverable:** one pushed RETURN.

**Why a Codex seat:** the author and its AAG panel are Claude-family. A refactor review is exactly
where a same-family reader shares the author's blind spots: "this inline block equals that helper"
is the judgement both make the same way (CLAUDE.md §Cross-model diversity).

## What is under review

The branch is `claude/desktop-consolidation`, cut from `main` @ `9ee63db7` (DM-1, #61). The PM names
the head SHA when relaying. Review this diff:

```
git diff origin/main...origin/claude/desktop-consolidation
```

This is the desktop's complexity-governance Rule-4 consolidation. **It ships zero features.** Its one
claim is **exact equivalence**: every desktop scenario's digest fixtures, oracle outputs, screenshots
and WM serial log lines are unchanged, byte for byte, under every feature combination. Any
divergence is a defect.

What moved:
- **WM** (`userland/neo-init/src/desktop_session.rs`):
  - `wm_rect` / `atrium_rect` replace about 11 hand conversions;
  - `Scene::view_transition` / `focus_transition` replace the transition tails at launch, resume,
    hold, return Home, the pointer focus switch and `lens::view`;
  - `grant_over` replaces three inline O / `'R'` / `sys_grant` / `'A'` handshakes (the surface
    grant, `resize::mint`, and the Lens lent buffer);
  - about 13 log writers now use `Line` (with a new `Line::i`) instead of `put_decimal` / `put_i64`;
  - redundant cfg gates are removed;
  - `handle_commit` now returns `Result`;
  - the module doc is rewritten.
- **neo-atrium** (`scene.rs`, `layout.rs`, `tests.rs`):
  - new helpers `blur_rect`, `round_rect`, `glow_mix`, `title_text`, `Rect::grow`,
    `chip_label_of`, `docked_group` and `FIT_PAD`;
  - in the tests: `write_ppm`, `window_violation`, and `tuple` / `rect`;
  - doc fixes.
- **Kernel** (`kernel/Cargo.toml`, `kernel/src/test_harness.rs`; test-only):
  - two non-scenario base features, `neo-test-desktop-home` and `neo-test-desktop-pointer`;
  - the scenario features are rebased onto them;
  - the `SCENARIO` `not()` ladders are removed;
  - a `DESKTOP_PASS` const per scenario.
- **xtask** (`tools/xtask/src/main.rs`):
  - one `DESKTOP_SCENARIOS` table replaces the per-scenario arms for timeout, memory, qemu-max,
    mid capture, serial script and band check;
  - HID scripts are built from named shared `step` consts;
  - `load_capture` / `masked_fnv` / `rgb` / `read_serial` / `require_single_arthur` helpers;
  - a unit test holds the table equal to `TEST_SCENARIOS`.
- **New files:**
  - `scripts/desktop/check_fixtures.sh` regenerates all 14 desktop fixtures from their oracles
    and `cmp`s them;
  - `docs/desktop-layering.md` is the fresh-reader layering note plus the I-14 / CP-55 / CP-56
    re-verification.

## Your job: refute it

1. **WM equivalence.** For each `view_transition` / `focus_transition` site, compare against the
   inline code on `main`:
   - the rect list;
   - the `finals` presents and their order;
   - the client argument;
   - motion on and off.

   For `grant_over`:
   - the order of the control messages;
   - the rights passed;
   - the error code returned on each failure (94, 99 and the surface grant's).

   For each `Line` conversion, check the byte text: separators, sign, padding and newline. Did a
   `put_decimal` (unsigned?) become a signed `i`, or the reverse?
2. **Removed cfg gates.** For every gate removed or narrowed, name a feature set in which the
   compiled code differs from `main`. Check `userland/neo-init/Cargo.toml` to see which features
   imply which. CI's desktop lint block (`.github/workflows`) names the sets CI builds.
3. **Kernel features.** Compare old against new: the transitive feature closure of every
   `neo-test-desktop-*-boot` feature. Then answer:
   - Did any scenario gain or lose a feature?
   - Can a CI-built set now define two `SCENARIO`s, or none?
   - Is every pass string byte-identical?
   - Does a cfg that used to mean "the home-boot scenario" now fire elsewhere?
4. **xtask table.** For each of the 12 desktop scenarios and for the non-desktop ones:
   - Are timeout, memory, CPU model, mid capture and serial script (the exact prompt/command
     sequence) unchanged?
   - Does reordering of guard arms shadow anything?
   - Is the masked digest the same (box order, pixel order, FNV constants)?
5. **neo-atrium helpers.** Is each helper pixel-identical to the inline code it replaced (arithmetic,
   saturation, blend order)? Do the tests still assert what they asserted?
6. **The proof itself.** Does `check_fixtures.sh` actually catch a changed fixture? Try it: edit one
   byte of one fixture, then run the script. Does it cover every fixture directory, and would it
   miss a fixture that an oracle no longer writes? Is anything the desktop scenarios check *not*
   covered by fixtures, screenshots or log text?
7. **Dead scaffolding and discriminators.** Was any gate discriminator, witness line or control
   scenario removed or weakened? Is anything dead that the build left behind?

A finding bounces the branch only if it survives your own re-verification. Uncertain items go in the
RETURN as notes. A "no findings" verdict must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    (or E:\AI Automation\workers\janus) — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/desktop-consolidation-crosscheck-<seat>
          (git fetch origin && git checkout -B codex/desktop-consolidation-crosscheck-<seat> origin/claude/desktop-consolidation)
```

```
git fetch origin
git show origin/claude/desktop-consolidation:Plans/DISPATCH-2026-10-19-desktop-consolidation-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-consolidation
```

Optional runs:
- `scripts/desktop/check_fixtures.sh` (needs `python3` with numpy, and the fonts the oracles read).
- `cargo test -p neo-atrium`
- `cargo test -p xtask`
- `cargo xtask neo-test --no-meta desktop_session_boot desktop_glass_boot desktop_glass_rt_boot desktop_home_boot desktop_pointer_boot desktop_lens_boot desktop_lens_pointer_boot desktop_lens_fault_boot desktop_resize_boot desktop_full_boot desktop_motion_boot desktop_motion_rm_boot`
  at `NEO_SMP=4`, with `LIMINE_DIR` set. Compare the screenshots under `target/` with the same run
  on `origin/main`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='desktop scenario serial script table xtask'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-desktop-consolidation-codex-crosscheck-<seat>.md` to your branch and
**push it**. It must contain:
- a verdict per numbered item;
- each finding with its severity, file:line, and a repro or the reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built anything, run `cargo clean` in your worktree
afterwards; never delete the worktree.
