<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check of the DS-1a / DS-1b fix-forward → Chad (DS-1a F1, F2) + Janus (DS-1b)

**Type:** review only (no code to merge). **Deliverable:** one pushed RETURN per seat. **Why these seats:** the fixes
close the seats' own findings:
- `Plans/RETURN-2026-10-18-desktop-ds1a-codex-crosscheck-chad.md` F1 (HIGH) and F2 (MEDIUM);
- `Plans/RETURN-2026-10-18-desktop-ds1b-codex-crosscheck-janus.md` (HIGH).

The author is Claude-family, so the seat that found each defect is the right cross-family lens to say whether it is
closed. The handoff order touches the focus/route seam (CLAUDE.md §Cross-model diversity).

## What is under review

Branch `claude/desktop-s1-session-boot`, restarted from `main` at `cfafff09`. The PM names the head SHA when relaying.
Review `git diff origin/main...origin/claude/desktop-s1-session-boot`. Design text:
`docs/THINK-2026-10-18-ds1-home-spine-shell.md` §"Codex fix-forward".

**F1 — keyboard handoff (Chad).**
- `userland/libneo/src/atrium_handoff.rs` fixes the order `HANDOFF = [Swap, Discard, Present]`, and
  `hand_over` runs it.
- The WM (`userland/neo-init/src/desktop_session.rs`) uses it for every transition:
  - the launch (Arthur's first activation, then `K` and the intent, then the focus light);
  - `home::hold`, `home::resume` and `home::return_home`;
  - `pointer::apply`, covering the escape and the refocus.
- Home's discard is `home::discard_queued`, run after the swap.
- Arthur's discard is `ArthurSurface::discard_queued`. `recv_intent` calls it on every intent, after the WM
  has made Arthur's focus the route, then acknowledges with `D`. The WM presents only after the `D`.
- The refocus discards nothing (see the THINK).
- Discriminators:
  - **Host:** `tools/atrium-logic-tests/src/handoff_kats.rs` models routing with a key injected at every step
    boundary of a round trip. DS-1a's orders must fail it.
  - **QEMU:** `desktop_home_boot` types `zz⌫⌫` immediately after Esc. xtask requires that no `z` or backspace
    reaches Arthur after it yields, and that each handoff to Home logs its discard.

**F2 — exact field (Chad).**
- Spec v1.3.1 states the field rules (`userland/neo-atrium/src/lib.rs`, "Home view").
- `scripts/desktop/field_oracle.py` → `fixtures/desktop-ds1/field_text.rs` covers four states: empty+caret,
  typed+caret, typed with no caret, and long scrolled. Each state has a tight window and per-pixel bounds over the
  scene oracle's glass background, plus a digest masking only that window and the bar/chip label boxes.
- The host KAT is `field_matches_exact_oracle_and_mutations_fail`.
- The QEMU readback (`kernel/src/test_harness.rs` `desktop_glass_asserts`, Home) checks the empty focused field.

**DS-1b — withheld proof (Janus).**
- `launch::serve` polls the proof with `SYS_TRY_RECV`, calling `idle` (the WM's `pointer_tick`: the SAK, the
  pointer, escaped keys) between polls.
- `PROOF_DEADLINE_NS` = 3 s. On timeout, or when the cap closes, the request is refused with 7.
- A late proof also revokes the cap for the session. The broker's end is closed, so a late proof send gets EPIPE
  rather than blocking the client short of the reply, and `atrium_client::launch` always receives the reply.
- Kernel: `SYS_CLOCK_GETTIME` and `SYS_SCHED_YIELD` (with their constants) are now compiled under
  `any(browser-runtime, atrium-pointer-forward)` (`kernel/src/arch/x86_64/syscall.rs`). Check that this widens
  nothing else.
- This is the pointer build only (`SYS_TRY_RECV` is `atrium-pointer-forward`). Without the pointer the proof is
  awaited, the same as the surface channel.
- Arthur's probes add `proof=withheld`. `desktop_pointer_boot` sends the SAK on `awaiting-proof`, and xtask
  requires `pointer escape … via=sak` before `proof-timeout`.
- `LaunchBroker::judge` refuses token 0 (KAT line added).

## Your job: refute it

1. **Chad, F1.**
   - Is any route swap left outside `HANDOFF`? Grep `sys_input_focus_activate`.
   - Is the discard really after the swap and before the view in each path?
   - Can a key still be replayed into a later turn? Consider:
     - Arthur's discard happening only on an intent;
     - the refocus that discards nothing;
     - the launch, where Home's leftovers wait for Home's next turn.
   - Does the model in `handoff_kats` capture ADR-074 faithfully, for example "only the route can be received
     from"? Could it pass for the wrong reason?
   - Is the QEMU injection a real discriminator? Would the DS-1a order fail it?
2. **Chad, F2.**
   - Is `field_oracle.py` independent of neo-text and neo-atrium?
   - Re-derive the scrolled state's origin and caret x.
   - Can a wrong field still pass? Consider a similar glyph within ±2/255, a caret one pixel off, or the
     unfocused state showing a caret.
   - Is the Home digest's remaining masking (bar and chip label boxes) acceptable?
3. **Janus, DS-1b.**
   - Is there any blocking receive left on the launch path?
   - Can `idle` re-enter the broker or change the route mid-judge unsafely? Consider a SAK escape during the wait,
     followed by the refusal reply to a now-unfocused Arthur.
   - Is the revocation sound: a closed end, `Option<Handle>`, close-once at session end? Are there other
     reply-rendezvous traps?
   - Is the deadline sound? `CLOCK_MONOTONIC` has 10 ms resolution, and a failing clock falls back to `u64::MAX`,
     which is an immediate timeout.
   - What does a client gain by sending its proof just after the deadline?
   - Does the scenario prove the SAK was honoured *inside* `serve` rather than in the main loop?
4. **Both.**
   - Kill-switch: `desktop-home` off must be the S3 session bit-for-bit. Check that the S1/S3 launch path keeps its
     old order.
   - What does the gate not cover?

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    (Chad)   — yours alone, never another coder's folder, never the repo root
WORKTREE: E:\AI Automation\workers\janus   (Janus)
BRANCH:   codex/desktop-ds1ab-fixforward-crosscheck-<seat>
          (git fetch origin && git checkout -B codex/desktop-ds1ab-fixforward-crosscheck-<seat> origin/claude/desktop-s1-session-boot)
```

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-18-desktop-ds1ab-fixforward-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Optional runs:
- `cargo test -p atrium-logic-tests -p neo-atrium`;
- `python3 scripts/desktop/field_oracle.py fixture` must reproduce `fixtures/desktop-ds1/field_text.rs`;
- `cargo xtask neo-test desktop_home_boot desktop_pointer_boot` at `NEO_SMP=4`. Set `LIMINE_DIR`: both previous
  RETURNs stopped at the missing `/home/neo/limine`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='WM keyboard focus handoff discard queue'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-ds1ab-fixforward-codex-crosscheck-<seat>.md` to your branch and
**push it**. Contents:
- a verdict per finding of yours: CLOSED / PARTIAL / OPEN;
- each new finding with severity, file:line and a repro or reasoning;
- for each numbered item, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** a review lane. If you built, `cargo clean` your worktree afterwards; never delete the worktree.
