<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of Desktop DS-1c (the Lens) → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the
author and the AAG lens are Claude-family. DS-1c makes the WM serve a **second client process** for the first time.
The Lens is spawned by the session broker inside Arthur's `OP_LAUNCH` and lent one directory capability. It adds a
new scene element and a selection state, each with an independent-oracle fixture, and it switches between two
surfaces by keyboard and by DI-1a's pointer. A same-family panel agreeing on either is weak
evidence (CLAUDE.md §Cross-model diversity). The PL assigns the seat (Jarvis / Janus / Chad).

## What is under review

Branch `claude/desktop-ds1c-lens` (PR #50). It is rebased onto `main` after the DS-1a/1b fix-forward (#47) merged.
SHA map: the parked head `263e88f6` became `d3d28dfe` (a squash) plus follow-ups. Note that this lane's branch name
changed; read it from the `claude/desktop-ds1c-lens` ref. The PM names
the head SHA when relaying. Review `git diff origin/main...origin/claude/desktop-ds1c-lens`. Design text:
`docs/THINK-2026-10-18-ds1-home-spine-shell.md`, §"DS-1c design — the Lens" (incl. Switching, Gate, Kill-switch) and
§"DS-1c known limitations".

- **The action.** Arthur (`userland/neo-arthur`, `desktop-home`) recognises a listing of `/`
  (`showcase_logic::wants_root_lens`: "list" / "ls" / "directory", and `/` named with no other path). It asks for
  `OP_LAUNCH { MODULE_LENS = 3 }` through its LaunchCap, which DS-1c widens to `{probe, lens}`.
- **The broker** (`userland/neo-init/src/desktop_session.rs`, `mod launch` + `mod lens`) serves the Lens *inside*
  Arthur's request, while Arthur is blocked on the reply. Steps:
  - spawn `worker` with handles: session channel (index 0), a closed placeholder channel (index 1, because file I/O
    reserves fd 1), and `/` opened `LOOKUP | READDIR` (index 2);
  - create → grant → import/attach → one commit;
  - snapshot the Lens's pixels through `SharedPixels` into a WM-owned plane;
  - show the Lens;
  - reply 0.

  The Lens ends with Arthur's session: the WM closes its session channel, the Lens sees `EPIPE`, and it is reaped.
  A failed bloom logs its stage and the Lens's own exit code, then rolls the scene back. One bloom per session.
- **The Lens** (`userland/neo-worker/src/lens.rs`, feature `desktop-lens`) is the multi-role worker binary. With no
  handles it is still DS-1b's probe (exit 1). It lists the directory with `sys_readdir`, draws the names with
  neo-text, commits once and waits. It holds no input focus.
- **Scene spec v1.4** (`userland/neo-atrium/src/lib.rs`, `scene.rs`, `layout.rs`) adds:
  - the Lens frame, top-aligned 32 px to the right of Arthur's window and frosted from its own cached blur
    (six blurs total);
  - a **dashed read-only Cap Halo** (dashes, not hue);
  - the Lens client floor;
  - a second Spine chip, "L / Lens", frosted from the Spine's blur.

  The independent oracle is `scripts/desktop/scene_oracle.py lens-fixture` → `fixtures/desktop-ds1/lens_digests.rs`.
- **Switching** (spec v1.5, `BaseScene::set_lens_selected`): the selection on the Lens lights its chip and the WM
  shows Arthur unfocused, the keyboard on the WM's own focus. Keyboard: Home's Tab cycle field → Arthur's chip →
  the Lens's chip, Enter there → `lens::view` (Tab/Enter back to Arthur, Esc to Home). Pointer (`desktop-pointer`):
  `pointer::chip_action` — a press on the Lens's chip → `Action::SelectLens`, on Arthur's chip (unfocused) →
  `FocusWindow`.
- **Kill-switch:** neo-init / neo-arthur feature `desktop-lens`. Off: `BaseScene::with_home` drops the Lens layout,
  LaunchCap `allow=probe`, no Lens module, Arthur unchanged. `desktop_home_boot` and `desktop_pointer_boot` are
  main's scripts and expectations, untouched.
- **Gate.** `desktop_lens_boot` (keyboard) and `desktop_lens_pointer_boot` (click). xtask compares each QEMU
  screendump against the scene oracle outside the masks. The keyboard capture is Arthur-focused. The click capture
  is Lens-selected, with the sprite masked. xtask also checks every Lens client pixel against
  `scripts/desktop/lens_content_oracle.py` (`fixtures/desktop-ds1/lens_content.rs`): glyph-window bounds, floor
  elsewhere. The xtask check also holds the Lens's labels — "Lens", "· read-only", and the chip's "L" / "Lens" —
  to `lens_content_oracle.py labels` (`fixtures/desktop-ds1/lens_labels.rs`).
- **On #47's handoff:**
  - `serve` takes the scene, so `idle(scene)` and the Lens bloom share one borrow.
  - `SelectLens` is an escape through `HANDOFF`.
  - `show_home` clears the selection.
  - `lens::choose` enters the Lens view only on the empty intent.

## Your job: refute it

1. **WM liveness.** The WM blocks serving the Lens inside Arthur's request, with no timeout. Beyond the recorded
   hung-Lens limitation, is there a schedule that deadlocks the WM? Consider:
   - Arthur's own messages while the WM serves the Lens;
   - keys typed during a bloom;
   - the Lens exiting at each step;
   - `EPIPE` versus a clean wait.

   Is teardown ordering safe? The orderings to check: `show_lens(None)`, close session, wait, release; and on
   failure: kill, wait, close, release. Can a handle be closed twice, or closed while still needed? Remember
   that closing any handle closes that endpoint side for every holder.
2. **Authority.** What can the Lens do beyond reading `/`? Look at the handle inventory and the ambient `sys_open`
   (recorded as OT-66). Can Arthur make the broker spawn anything other than the probe or the Lens, bloom twice,
   or bloom with a different path? Is `wants_root_lens` sound?
3. **Compositing.** Check every present path in the window and Home views:
   - the Lens layer only in the window view;
   - per-region presents in `show_lens`;
   - resume/hold;
   - Destroy.

   Can Arthur's pixels be overwritten by base, or can a stale Lens frame survive the Lens's end? Does the WM read
   the Lens's pages only through `SharedPixels` (S1-F2)?
4. **Oracle independence.** Does `scene_oracle.py` implement the v1.4 Lens and v1.5 selection text, or does it copy the Rust? Check:
   - the halo dash rule;
   - the chip-2 blur source;
   - the drawing order;
   - the fit condition;
   - the RT colours.

   Is the xtask mask set right? Re-derive one halo pixel and one chip-2 pixel.
5. **Kill-switch.** With `desktop-lens` off, is the session DS-1b / DI-1a exactly? Look for any Lens code, layout,
   blur, repaint region or log line still reachable. Is `neo-worker` without `desktop-lens` unchanged for its many
   other scenarios? With `desktop-home` off, is the build S3 bit-for-bit? Are the DS-1a/DS-1b Home digests still
   pinned?
6. **Switching and the content oracle.** Check these:
   - Can the selection outlive the Lens and leave a lit chip or glow residue? (`lens_regions` now includes the
     chip's glow reach.)
   - Can a click on a chip be forwarded to Arthur, or reach Arthur while the Lens is selected?
   - Does every exit from `lens::view` leave a consistent focus route plus `escaped` state?
   - Is `lens_content_oracle.py` independent of neo-text and neo-worker?
   - Re-derive one glyph window, and say whether a wrong but similar name ("scratch" → "scrateh") would pass the
     ±2/255 bounds.

7. **What the gate does not cover:**
   - a failed bloom;
   - a second bloom;
   - no room for a Lens (1280×800);
   - RT with a Lens;
   - pointer clicks in Home.

   Which of these matter next?
8. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/desktop-ds1c-crosscheck-<seat>     (git fetch origin && git checkout -B codex/desktop-ds1c-crosscheck-<seat> origin/claude/desktop-ds1c-lens)
```

```
git fetch origin
git show origin/claude/desktop-ds1c-lens:Plans/DISPATCH-2026-10-18-desktop-ds1c-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-ds1c-lens
```

Optional runs:
- `cargo test -p neo-atrium -p atrium-logic-tests -p arthur-logic-tests`;
- `python3 scripts/desktop/scene_oracle.py lens-fixture` must reproduce `fixtures/desktop-ds1/lens_digests.rs`;
- `python3 scripts/desktop/lens_content_oracle.py fixture` must reproduce `fixtures/desktop-ds1/lens_content.rs`;
- `cargo xtask neo-test desktop_lens_boot desktop_lens_pointer_boot desktop_home_boot desktop_pointer_boot` at
  `NEO_SMP=4`;
- `cargo xtask run-desktop --lens --pointer`, then type `list /` and click the Lens's chip.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='session broker spawns lens instrument directory capability'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-ds1c-codex-crosscheck-<seat>.md` to `codex/desktop-ds1c-crosscheck-<seat>`
and **push it**. Contents: a verdict (MERGE / MERGE-WITH-NITS / BOUNCE); each finding with severity, file:line and
a repro or reasoning; for each numbered item, what you checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** a review lane. If you built, `cargo clean` your worktree afterwards; never delete the worktree.
