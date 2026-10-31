<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of Desktop DS-1a (Home intent field) → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the
author and both AAG lenses are Claude-family. DS-1a is the first time the window manager holds an input focus
of its **own** — the property under review is ADR-074's "the WM is keyboard-blind to its clients", and a
same-family panel agreeing that it still holds is weak evidence (CLAUDE.md §Cross-model diversity). The PL
assigns the seat (Jarvis / Janus / Chad).

## What is under review

Branch `claude/desktop-s1-session-boot` (the session's designated branch, restarted from `main` after S3 #36
merged). The PM names the head SHA when relaying; review `git diff origin/main...origin/claude/desktop-s1-session-boot`.

DS-1a, keyboard only (synthesis `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` §ladder DS-1):
- **Home view** (`userland/neo-atrium`): `View::Home`, `BaseScene::with_home` (one more blur — the intent
  field — at build), `paint_home_rect`, the field text/placeholder/caret, `keys.rs` (HID decode + a bounded
  line editor). Normative text: spec v1.2 "Home view" in `lib.rs`; independent oracle
  `scripts/desktop/scene_oracle.py home-fixture` → `fixtures/desktop-ds1/home_digests.rs`.
- **WM** (`userland/neo-init/src/desktop_session.rs`, feature `desktop-home`): `home::claim_focus` creates the
  WM's own `InputFocus` and activates it; `read_intent` edits the field from that focus only; Enter spawns
  Arthur (`arthur_session`, extracted unchanged from S1/S3's `run()`), hands it the intent as `b"I" + bytes`
  on the session channel after the `K` handshake, and on Arthur's exit `return_home` re-activates the WM's
  focus, drains type-ahead and repaints Home. Per-round handles are closed.
- **Arthur** (`userland/neo-arthur/src/main.rs`, feature `desktop-home`): reads the intent and answers it as its
  first line (injected Enter, not logged as a keystroke). **Esc → `OP_YIELD_FOCUS`** (new ADR-073 verb 7,
  `libneo::atrium_client::yield_focus`, `SurfaceRegistry::yield_focus_on_channel`): Arthur is *held* (blocked
  on its session channel) while Home has the keyboard; Home's next intent, or Enter on the Arthur Spine chip
  (Tab), resumes the same Arthur. Client-cooperative by PM decision: a wedged client can trap focus until DI-1's
  SAK (recorded in `docs/THINK-2026-10-18-ds1-home-spine-shell.md`).
- **Gate:** `desktop_home_boot` — WM-own keystrokes, intent → Arthur, return to Home, `blur_regions_total=5`,
  two view-switch witnesses and the kernel's Home readback against the oracle. The witnesses:
  `stale_pixels` (presented ≠ base; proven: removing the switch's present gives 35,437) and `residue_pixels`
  (base ≠ the target view recomputed per pixel; proven: routing the window repaint through the focus-region
  clip gives 314,183 — a bug the first witness and the host tests both missed, found on a screenshot).

## Your job: refute it

1. **WM-blind (the core question).** Can the WM ever receive a key meant for Arthur? Trace
   `kernel/src/atrium/focus.rs` route swaps against the WM's sequence (own focus active → Arthur's activator
   activated → Arthur exits → WM re-activates). What happens to keys typed in each gap? What does the WM's
   permanently-held RECEIVE handle on its own focus allow?
2. **Return path.** Is the route left on a dead client's focus object between Arthur's exit and `return_home`?
   Is the drain correct (it must run after re-activation — before it, `recv` is `EPERM`)? Bounded?
3. **The `I` message.** Ad-hoc control framing on the session channel: framing, length, truncation, non-printable
   rejection. Is this an INV-WM-1 concern (it names no peer)? Should it be an ADR-073 verb with a byte oracle?
4. **`OP_YIELD_FOCUS` + hold/resume.** Byte layout vs the hand-laid oracle; channel binding (INV-WM-1); can a
   client yield on behalf of a peer, yield twice, yield before attach, or commit while held? On resume the WM
   re-activates the client's focus and sends the intent — any window where keys reach the wrong object?
5. **Relaunch loop.** Production Arthur never exits (Esc holds it instead), so the exit → Home → relaunch loop
   runs only in the boot test. Would a second launch work (registry, channels, the RO buffer mapping that has no
   unmap, scene focus state)?
6. **Oracle independence** for the Home view, as for S3: what did the oracle take from code rather than the
   v1.2 text? The typed text and caret are masked — is anything important hiding there?
7. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your re-verification; uncertain items go in the RETURN as notes.
"No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/desktop-ds1a-crosscheck-<seat>     (git fetch origin && git checkout -B codex/desktop-ds1a-crosscheck-<seat> origin/claude/desktop-s1-session-boot)
```

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-18-desktop-ds1a-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Optional runs: `cargo test -p neo-atrium`; `python3 scripts/desktop/scene_oracle.py home-fixture`;
`cargo xtask neo-test desktop_home_boot` at `NEO_SMP=4`; `cargo xtask run-desktop --home` to try it.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='input focus activate route swap'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-ds1a-codex-crosscheck-<seat>.md` to
`codex/desktop-ds1a-crosscheck-<seat>` and **push it**. Contents: a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
each finding with severity, file:line and a repro or reasoning; for each numbered item, what you checked.
Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** a review lane. If you built, `cargo clean` your worktree afterwards; never delete the worktree.
