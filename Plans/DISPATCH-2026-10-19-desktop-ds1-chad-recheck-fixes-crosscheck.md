<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check of the fixes for Chad's DS-1ab F1 (re-check) and DS-1c F1/F2 → Chad

**Type:** review only (no code to merge). **Deliverable:** one pushed RETURN. **Why Chad:** these fixes close Chad's
own findings:
- `Plans/RETURN-2026-10-18-desktop-ds1ab-fixforward-codex-crosscheck-chad.md` (on
  `codex/desktop-ds1ab-fixforward-crosscheck-chad` @ `cc1f4aa9`): F1 STILL OPEN.
- `Plans/RETURN-2026-10-18-desktop-ds1c-codex-crosscheck-chad.md` (on `codex/desktop-ds1c-crosscheck-chad` @
  `bf6434f5`): F1 Medium, F2 Low.

The author is Claude-family; Chad is the cross-family lens.

## What is under review

Branch `claude/desktop-s1-session-boot`, restarted from `main` at `2acfa308`. The PM names the head SHA when relaying.
Review `git diff origin/main...origin/claude/desktop-s1-session-boot`.

**DS-1ab F1: the initial launch.**
- `run()` no longer shows or presents the window view before `arthur_session`. Home stays on screen, with its route
  live, while Arthur is spawned and handshaken.
- `commit()` copies, but does not present, a commit that arrives while Home is shown.
- The launch's `hand_over` present step now:
  - presents the window view, unfocused, and logs the stale/residue witnesses;
  - then sets the focus light and presents it.

  It runs after the swap, and after Arthur's discard ack.
- Arthur logs its intent line *before* sending `D`, so its line and the WM's view-switch lines cannot interleave.
- **Discriminator.** `desktop_home_boot` types `x` and Backspace the moment `DESKTOP view switch=window
  residue_pixels=0` appears. xtask requires `DESKTOP arthur keystroke ascii=120` and `ascii=8` between that line and
  Arthur's yield. With the old order, both keys reached Home and were discarded.

**DS-1c F1: the failed-bloom leak.**
- The WM now creates *one* Lens buffer, maps it read-only once, and keeps one copy plane (`lens::LensStore`, made at
  the first bloom).
- It *lends* that buffer to each Lens over the surface channel: O / R / grant / A, with `USE` meta and
  `MAP_WRITE|QUERY|COMMIT`.
- The Lens (`userland/neo-worker/src/lens.rs`) validates the descriptor, maps it RW, draws, commits, then sends
  ImportAttach and Commit. The old "R / grant the Lens's own buffer" step is gone.
- A bloom, failed or not, now maps nothing new in the WM.
- `LensSession::release` revokes the process handle with `sys_revoke`. `SYS_CLOSE` refuses Process handles.
- `libneo::atrium_launch::LensAttempts` caps blooms per Arthur session at `MAX_LENS_ATTEMPTS = 3`. A failed bloom
  spends its attempt; past the cap the reply is `STATUS_ATTEMPTS_SPENT` (12). Host KAT:
  `lens_attempts_are_bounded_and_failures_spend_them`.
- **Discriminator.** `desktop_lens_fault_boot` (WM `desktop-lens-fault-test`, Arthur `desktop-lens-fault-test`):
  - the first two blooms fail as late as possible, after the Lens committed;
  - Arthur asks again, and the third shows the Lens, captured and checked exactly as in `desktop_lens_boot`;
  - before the third, `DESKTOP lens leak-check failed_blooms=2 handles_leaked=0 maps_leaked=0`:
    - the first 32 free handle slots must hash the same as before attempt 1;
    - a probe mapping must land exactly one probe stride past the baseline's, because the mmap cursor never reuses
      an address.

**DS-1c F2: strict parse.**
- `wants_root_lens` accepts exactly `ls /`, `list /`, `list directory /` and `list the directory /` (whitespace
  separated, any case).
- Negative KATs: `ls / tmp`, `list / ../scratch`, `ls / please`, `ls tmp /`, `ls //`, among others.

## Your job: refute it

1. **DS-1ab F1.**
   - Is any view now shown before its owner has the route?
   - Is the first commit, made while Home shows, always presented once the window shows, or could a commit be lost?
   - Does the discriminator fail on the old order?
   - Is any key typed during launch preparation still mis-delivered? It reaches Home, which is on screen; Home's next
     turn discards it.
2. **DS-1c F1.**
   - Is anything still allocated per bloom in the WM and never released? Check every `?` / `Err` path in `bloom` and
     `serve`.
   - **Lending.** Can a Lens keep writing after it is killed? Can two Lenses ever hold the buffer at once? Can it
     grant the buffer onward? Can a stale picture from an earlier Lens be shown?
   - Is `sys_revoke` on the process handle correct and sufficient?
   - Does the leak-check witness actually discriminate? Re-derive the slot and cursor arguments.
3. **DS-1c F2.** Can a wrong command still pass, or a supported one now fail?
4. **Kill-switch.**
   - With `desktop-lens` off, nothing changes.
   - With `desktop-home` off, the S1/S3 launch is unchanged.
   - The non-fault Lens builds carry no fault code.
5. **What the gate does not cover.**

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as notes.
"No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/desktop-ds1-recheck-fixes-crosscheck-chad
          (git fetch origin && git checkout -B codex/desktop-ds1-recheck-fixes-crosscheck-chad origin/claude/desktop-s1-session-boot)
```

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-19-desktop-ds1-chad-recheck-fixes-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Optional runs:
- `cargo test -p atrium-logic-tests -p arthur-logic-tests -p neo-atrium`.
- `cargo xtask neo-test desktop_home_boot desktop_lens_fault_boot desktop_lens_boot` at `NEO_SMP=4`. Set
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
Query e.g.: `$body=@{query='WM launch handoff Lens buffer lend revoke'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-desktop-ds1-recheck-fixes-crosscheck-chad.md` to your branch and **push it**. It must
contain:
- a verdict per finding: CLOSED / PARTIAL / OPEN;
- each new finding with severity, file:line, and a repro or reasoning;
- for each numbered item, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never delete the
worktree.
