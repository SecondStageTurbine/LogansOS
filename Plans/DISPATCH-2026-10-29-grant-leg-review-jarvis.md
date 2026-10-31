<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #146) → Jarvis: #193, the CAP_GRAPH two-CPU grant leg

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. SMP-core test code: a narrow check of one new discriminator.

#193 merged after batch 15 (run `40831274927`, green at `5a37066a`) as `fec09323`. Review it on `main`:
`git show fec09323 --stat` and `git show fec09323`. It adds a deterministic two-CPU leg to `cap_graph_revoke`, closing
#183's item 6. A grant on CPU 1 stops at a test barrier inside `complete_grant` while holding `CAP_GRAPH`, a revoke
starts on CPU 2, and the BSP checks the revoke waits. One pushed RETURN, a plain checklist (pass or fail, the enforcing
`file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-grant-leg-review-jarvis.md`.

1. Does the leg fail if `complete_grant`'s source lookup and install are in two `CAP_GRAPH` sections? Trace the
   falsifier (drop, barrier, re-lock → `granted_child_survived_the_revoke`).
2. Can the BSP take `CAP_GRAPH`, send a shootdown, or wait on CPU 1 while CPU 1 holds the barrier?
3. Is every new item behind `neo-test-cap-graph-revoke`, so no product kernel changes? The PR claims `.text` and
   `.rodata` are identical for default, desktop and usb-host.
4. Is the corrected CP-87 text accurate against `install_minted_locked` and `complete_grant`?

```
MODEL: GPT-6.1-Sol. EFFORT: medium. (A narrow check of one SMP-core discriminator.)
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/grant-leg-review-jarvis
          git fetch origin && git checkout -B codex/grant-leg-review-jarvis origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-grant-leg-review-jarvis.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
