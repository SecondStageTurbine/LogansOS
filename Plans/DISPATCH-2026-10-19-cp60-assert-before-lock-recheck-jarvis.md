<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: re-check of the CP-60 assertion placement after your F1 bounce → Jarvis (Codex)

**Type:** review only, short. **Deliverable:** a pushed RETURN file.

**Why you.** You bounced the merged CP-60 fix (`d62a494b`) on F1: `Pmap::drop`'s assertions panic inside
`with_frame_alloc`, and the bare-metal test panic path re-enters it. This checks the repair.

## What is under review

Branch `claude/cp60-assert-before-lock`, cut from `origin/main` 02cff42a; the PM names the head SHA in the
relay.

```
git fetch origin
git show origin/claude/cp60-assert-before-lock:Plans/DISPATCH-2026-10-19-cp60-assert-before-lock-recheck-jarvis.md
git diff origin/main...origin/claude/cp60-assert-before-lock
```

1. **The fix.** `assert_pml4_not_live(self.pml4_frame)` is the first statement of `Pmap::drop`, before the
   collection walk, the shootdown and every free, and outside `with_frame_alloc`. The assertions are
   unchanged and stay hard.
2. **The witnesses, all neo-test only.**
   - `mm::FRAME_ALLOC_HELD`, a per-CPU flag set for exactly the `with_frame_alloc` closure
     (`run_locked`).
   - `pmap::LIVE_CHECKS` and `LIVE_CHECKS_UNDER_FRAME_ALLOC`.
   - `token_park_teardown_churn` requires at least 60 checks and 0 under the lock.
   - `panic_failed` prints `frame_alloc_held=`.
3. **Claimed evidence:**
   - The fix passes 2/2.
   - The lazy-CR3 control fails with `frame_alloc_held=false`.
   - With the check moved back inside the closure, the scenario fails
     `pmap_live_check_under_frame_alloc` (`under_frame_alloc=60`), and the control shows
     `frame_alloc_held=true`.

## Refute it

1. Is there any path to `assert_pml4_not_live` that still holds `FRAME_ALLOC` or another lock the panic path
   takes? Consider a `Pmap` dropped from inside a `with_frame_alloc` closure, the slab allocator, the serial
   lock, or the log sink's lock.
2. Between the check and the free, can any CPU load this PML4? You argued no: `&mut self`, plus every
   schedulable thread owns an `Arc<Pmap>`. Confirm, now that the window is longer: the walk and the
   shootdown run in between.
3. Is the witness sound? Does `run_locked` mark exactly the closure's lifetime on both the debug and
   release branches? Can the flag stay stuck `true` after a panic inside the closure, and does it matter?
4. Anything else.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must
say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp60-assert-before-lock-recheck-jarvis   (git fetch origin && git checkout -B codex/cp60-assert-before-lock-recheck-jarvis origin/claude/cp60-assert-before-lock)
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test token_park_teardown_churn` should PASS.
- With `NEO_TEST_EXTRA_FEATURES=neo-test-kernel-cr3-lazy-control` it should FAIL with
  `frame_alloc_held=false`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions;
grep still wins for exact symbols. Health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`,
expecting `{"status":"healthy",...}`. If it is down or degraded, the PL manages it: note that in your RETURN
and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='panic handler log flush block sink allocation'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-cp60-assert-before-lock-recheck-jarvis.md` to
`codex/cp60-assert-before-lock-recheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
