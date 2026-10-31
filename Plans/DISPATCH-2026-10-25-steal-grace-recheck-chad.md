<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #53): narrow re-check of your relay #52 items 3, 4 and 6 → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This re-checks only the three items you failed in relay #52
(RETURN `22f99efc`). The fix-forward is PR #117.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `ae6231a0` (main; the squash of #117,
gated as `d0265cde`, run `40163944426`, 23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/steal-grace-recheck-chad
          git fetch origin && git checkout -B codex/steal-grace-recheck-chad ae6231a086047b883b8db88471fd2aee3451ea02

git show origin/main:Plans/DISPATCH-2026-10-25-steal-grace-recheck-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-steal-grace-review-chad.md      # your round-1 RETURN
git diff 0df36d09 ae6231a0 -- kernel tools .github docs/concurrency-pitfalls.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input or sequence.

- **R3 (item 3).** `steal_grace_cycles` (par_matmul_logic.rs) returns a non-zero threshold for every non-zero
  `tsc_hz` with `grace_us = 50`, including your 500 kHz input. It rounds up, can't overflow, and keeps the spin
  fallback at `tsc_hz == 0`. The host KATs cover 500 kHz, a non-integral MHz rate, 3 GHz and `u64::MAX`.
- **R4 (item 4).** The `published` stamp is taken with the store-visibility ordering you cited (MFENCE then LFENCE
  before RDTSC), after every slot's `pending` Release store, and only once per job.
- **R6 (item 6).** The gate now runs a scenario with `matmul-steal-grace-time` ON: `threaded_matmul_parity_grace_time`.
  It forces a BSP steal, asserts the steal happened at or after the grace, and asserts output identical to the
  oracle. Cite where CI selects it, and its PASS line in run `40163944426`. Its test-only rate/grace override is
  compiled only under the scenario feature, so no product build can set it.

## Close condition

- Commit `Plans/RETURN-2026-10-25-steal-grace-recheck-chad.md` on your branch and **push it**, with R3/R4/R6 verdicts
  (CLOSED or STILL OPEN). Paste the SHA to the PL.
- No build is needed. Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
