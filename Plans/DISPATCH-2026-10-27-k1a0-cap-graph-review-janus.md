<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #109): cross-family review of K1a-0, the capability-graph lock and re-parenting (#166) → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. It adds a global interrupts-off ticket lock, outermost in the kernel's lock
order, taken by revoke, every install, every close and process teardown, with cross-process re-parenting. That's SMP
core, Claude-authored (S4), so it needs a cross-family lens (CLAUDE.md, AAG for the concurrency core). Muse reviews
security separately (relay #108). **Do this first in your queue,** ahead of #91: it gates the browser sandbox and
logout.

**Type:** review, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `149d4a19`, #166's squash on main (PR head `7e15450c` plus a merge of
main). Gate: batch 9, run `40484657565`, all jobs green.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/k1a0-cap-graph-review-janus
          git fetch origin && git checkout -B codex/k1a0-cap-graph-review-janus 149d4a19

git show origin/main:Plans/DISPATCH-2026-10-27-k1a0-cap-graph-review-janus.md   # this file
git diff 149d4a19^ 149d4a19
git show origin/main:docs/concurrency-pitfalls.md                                # the new entries, CP-37, CP-56, CP-63
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='CAP_GRAPH revoke_handle install_cap re-parent ticket_mutex'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete interleaving (CPU, lock held,
instruction point).

1. **The lock order.**
   - `CAP_GRAPH` is outermost.
   - Every caller acquires it before the handles lock, PROCESSES and the scheduler lock, never after.
   - Check every caller in the PR's callers table, and grep for any it missed.
   - The reaper takes it only after dropping the scheduler lock (`scheduler.rs:2683-2715`).
2. **The interrupts-off ticket lock.**
   - Every acquisition is inside `without_interrupts`.
   - No IRQ or exception path takes it (CP-37).
   - No holder parks, blocks, sends an IPI and waits, or takes a sleeping lock (CP-56/CP-63).
   - The spin is bounded by the holders queued ahead.
   - The ticket counter can't wrap into a false grant.
3. **Revoke vs install.** With the lock, no install of a child of a doomed cap can complete after the walk. The
   re-check of a cross-process parent (`cap_is_live` walking every registered process) runs with `CAP_GRAPH` held. The
   handles-lock release and re-take around it can't let the recipient exit unnoticed (the `exiting` re-check).
4. **Re-parenting.**
   - On close and on exit, children move to the closing cap's parent before its edge is dropped.
   - Across processes, a concurrent revoke sees the old tree or the new one, never neither.
   - The `HAS_CHILDREN` set stays exact: no stale entry, and no missed one after a re-parent.
5. **Spawn.** The child is registered (visible to the walk) before any inherited handle is usable and before it runs.
   A spawn whose inherited parent died is refused cleanly, with no half-registered child.
6. **The scenario and its control.** All 7 legs (race, close, exit, kill, grant race, spawn race, share-close) would
   fail if their mechanism were removed. The control build leaks all 7 (exit 137). Say whether any leg can pass for
   the wrong reason: timing luck, or a hook that doesn't actually park the revoke.
7. **No regression.** The 45-scenario sweep includes `wait_any_core`, whose fixture has a cross-process parent. Name any
   existing cap path whose behaviour changes (not only its timing).
8. **The `.text` delta** (about 9.5 KB) is only the lock, the re-parent code and the edge checks, with `NEO_BUILD_ID`
   pinned.

## Close condition

- Commit `Plans/RETURN-2026-10-27-k1a0-cap-graph-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
