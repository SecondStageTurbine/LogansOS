<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #64): post-merge review of the Arthur startup fix (#126) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. This is contained code: a build-profile change, an overlay line, and a
per-pid fix in the prefill instrumentation. There's no SMP core and no new syscall. It's Claude-authored, so it gets a
Codex-family review. **Do this after relay #63 (the A7 image build).**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `745dd314` (main; the squash of PR #126, gated as `efa7c2dc`, run `40219358782`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/arthur-startup-review-janus
          git fetch origin && git checkout -B codex/arthur-startup-review-janus 745dd314dd8e74e60839fb2529d3187222a93f5e

git show origin/main:Plans/DISPATCH-2026-10-25-arthur-startup-review-janus.md   # this file
git diff 240a2255 745dd314
```

The PR body (PR #126 on GitHub) has the parse_ms table, the stack high-water figures and the kernel `.text` comparison.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='prefill_perf last commit per pid think presents watchdog timeline vocab'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input.

1. **Profile scope.** The four `[profile.dev.package.*]` entries (`neo-arthur`, `core`, `alloc`, `compiler_builtins`)
   apply to every `build-std` binary, kernel and userland. Check whether anything in the kernel depends on these
   crates being unoptimized: timing loops calibrated by spin count, `#[inline(never)]` assumptions, or stack-depth
   budgets. The PR's stack high-water figures and the timing-scenario list are the evidence; say whether they cover
   the kernel's own stack-tight paths.
2. **Release profile and host builds.** The change is only in `profile.dev`. Host builds (no `build-std`) are
   unaffected apart from cargo's "matches no package" note. The product image (built at opt-level 3 by xtask) is
   unchanged apart from symbol names, per the PR.
3. **Overlay vocab split.** `note_vocab_open` / the close hook record only a `/vocab/` path's open and the close of
   that same fd. Check that a different process closing the same fd number can't record the close, or say why it
   doesn't matter. The `start` line prints `vocab=O..C` correctly when either value is still 0 (shown as `-`).
4. **Think/presents per pid.** `prefill_perf` keeps the last commit per pid (8 slots). Check what happens with more
   than 8 committing pids, and whether a stale slot from an exited pid can be read as Arthur's.
5. **The syscall hook.** In `syscall_dispatch`, `SYS_OPEN` copies the user path a second time
   (`copy_user_bytes`) after a successful open to look for `/vocab/`, and `SYS_CLOSE` calls `note_close` before the
   close. Check that the second copy handles a bad or changed user pointer safely (a changed path can only mislabel the
   timeline), that neither hook takes a lock or changes IF, and that both are compiled out without `freeze-watchdog`.
6. **Host tests.** `tools/fb-progress-logic-tests` covers the new `vocab=O..C` formatting: the empty case
   (`vocab=-..-`), a normal case, and the large-value case. Say whether an open-without-close case (`vocab=O..-`) is
   covered.

## Close condition

- Commit `Plans/RETURN-2026-10-25-arthur-startup-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
