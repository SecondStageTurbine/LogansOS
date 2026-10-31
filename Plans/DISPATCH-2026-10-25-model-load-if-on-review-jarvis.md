<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #60): narrow post-merge review of sys_model_load IF-on (#123) → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow follow-up to your relay #58 item 4. It covers one syscall's
interrupt window plus a test leg, using #121's helper. It's Claude-authored, so it gets a Codex-family review.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `3fa65eb8` (main; the squash of PR #123, gated as `570946eb`, run `40203140439`,
23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis    (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/model-load-if-on-review-jarvis
          git fetch origin && git checkout -B codex/model-load-if-on-review-jarvis 3fa65eb85522918308f4fc151a500e5a09266cf5

git show origin/main:Plans/DISPATCH-2026-10-25-model-load-if-on-review-jarvis.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-vfs-if-on-review-jarvis.md           # your relay #58 RETURN
git show origin/main:Plans/NOTE-2026-10-25-model-load-if-on-design.md
git diff 9564aa25 3fa65eb8 -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete sequence.

1. **The disk branch.** In `sys_model_load_impl`, both the NeoFS open (`fs::open_root_temporary`) and
   `read_model_file` now run inside `vfs_body_if_on`. User copies stay outside the window, and the caller's IF is
   restored on every return path.
2. **The model build.** The model build (hash, dedup, copy) still runs outside the interrupts-on window, unchanged.
   CP-74 says it must not run on a 32 KiB stack, and that constraint is unchanged.
3. **Preloaded models.** A `/boot/models/...` load with boot-model-preload returns the preloaded model without reaching
   the new window.
4. **The scenario.** The model_load leg runs first in `phase3_neofs_syscall_if_on`, so the IF-off control hangs on it
   (the control proves that leg). Item 5 from your relay #58 (held-at-call-start) is being fixed separately, so note
   whether this leg has the same weakness.
5. **What's left of item 4.** `model.validate_current` (query/infer/generate) reaching `entries` at IF=0 is being
   fixed in S4's lane D, not in this PR. Confirm #123 doesn't change that path or make it worse.

## Close condition

- Commit `Plans/RETURN-2026-10-25-model-load-if-on-review-jarvis.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
