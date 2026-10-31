<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check of the DL1-X1 fix-forward → Janus

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why Janus:** you found DL1-X1, and the fix's author is Claude-family. You check whether the fix closes your own
finding and opens nothing new.

## What is under review

The branch is `claude/dl1-stale-configure-fix`, cut from `main` @ `02cff42a`. The PM names the head SHA when relaying.
Review this diff:

```
git diff origin/main...origin/claude/dl1-stale-configure-fix
```

Your finding is in `Plans/RETURN-2026-10-19-desktop-dl1-codex-crosscheck-janus.md` on
`origin/codex/desktop-dl1-crosscheck-janus`.

The fix:
- **`Layout::docked_max_client`** (`userland/neo-atrium/src/layout.rs`) gives the largest client a docked group
  holds: frame + `LENS_GAP` + Lens frame + right margin inside the workfield with `GLOW`, within `max_client`.
- **`clamp_origin`** saturates its upper bounds, so a group wider than the workfield lands at the low edge instead
  of underflowing.
- **The WM refit.** In `Scene::refit_pending_for_dock` (`userland/neo-init/src/desktop_session.rs`), right after a
  successful `lens::bloom` inside `OP_LAUNCH`, a pending configure larger than the docked maximum is re-published at
  that maximum. It gets a new serial and the log line `DESKTOP pointer resize refit from=… to=… serial=N docked=lens`.
- **The unchanged ack.** This came from a Claude AAG lens. A refit can equal the current frame (on 1280x800, a
  client already 728 wide at x=98). `apply_configure` now accepts that ack without `place_frame`, which refuses an
  unchanged frame with error 92.
- **The KAT.** `stale_configure_too_wide_for_the_docked_group_refits` is your 1280x800 / x=98 / width-1120 repro.
  It also checks that every refit size is placeable docked on five outputs.
- **The THINK.** `docs/THINK-2026-10-19-dl1-docked-lens.md` records the fix-forward under the stale-configure
  closure.

## Your job: refute it

1. **Closure.** After the refit, can `apply_configure` still reach a size or place the docked group cannot hold?
   Check the height axis too. `docked_max_client` and `resized`'s docked bounds must agree; find an input where they
   do not.
2. **The refit's timing.** The refit assumes Arthur has not read the mailbox since the stale publish, because it is
   parked on its `OP_LAUNCH` reply. Find a path where Arthur reads the old serial, or acks it, after the refit (that
   would be error 91). Also check:
   - a bloom that fails;
   - a second resize released after the bloom;
   - the bloom's retry path.
3. **Saturation.** Does saturating change any result where the old code was well-defined, that is, wherever
   `lo ≤ hi` held?
4. **Kill-switch.** Is the change inert without the Lens + resize composition? Cover `desktop-lens-pointer`,
   `desktop-resize` and `desktop-full`.
5. **Coverage.** The QEMU case (a resize released inside the proof-poll window at 1280x800) is not added. Is there a
   cheap way?

A finding bounces the branch only if it survives your own re-verification. "No findings" must state what you checked
for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\janus    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/dl1-x1-recheck-janus
          (git fetch origin && git checkout -B codex/dl1-x1-recheck-janus origin/claude/dl1-stale-configure-fix)
```

```
git fetch origin
git show origin/claude/dl1-stale-configure-fix:Plans/DISPATCH-2026-10-19-dl1-x1-fix-codex-recheck.md
git diff origin/main...origin/claude/dl1-stale-configure-fix
```

Optional runs:
- `cargo test -p neo-atrium stale_configure`
- `cargo test -p neo-atrium`
- clippy on `neo-init` with `desktop-full`, `desktop-lens-pointer` and `desktop-resize`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='docked Lens configure clamp refit'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-dl1-x1-fix-codex-recheck-janus.md` to your branch and **push it**. It must contain:
- a verdict per item;
- each finding with its severity, file:line, and a repro or the reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never delete the
worktree.
