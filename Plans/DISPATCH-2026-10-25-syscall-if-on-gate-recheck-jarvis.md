<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #67): narrow re-check of your relay #58 item 5, after #129 → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own earlier finding: relay #58 item 5 said
`phase3_neofs_syscall_if_on` could pass without contending on `entries`, if the holder released between the scenario
reading `HELD` and the call. #129 (S2) is the fix. It's test-only. **Do this after relay #65.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `16fc5e2d` (main; the squash of PR #129, gated as `14e68e5f`, run `40235587254`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/syscall-if-on-gate-recheck-jarvis
          git fetch origin && git checkout -B codex/syscall-if-on-gate-recheck-jarvis 16fc5e2d77af349c970c95dcdbe6027484a36f40

git show origin/main:Plans/DISPATCH-2026-10-25-syscall-if-on-gate-recheck-jarvis.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-vfs-if-on-review-jarvis.md               # your #58, item 5
git diff d47e94c5 16fc5e2d -- kernel
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='ENTRIES_GATE_HELD ENTRIES_GATE_CALLING debug_hold_entries_yielding contend'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete interleaving.

1. **A won claim proves the lock was held.** The holder stores `HELD` only after taking `entries`. It swaps in
   `RELEASED` before dropping it (`kernel/src/fs/neofs/inode.rs`). The caller's `HELD → CALLING` CAS runs with
   interrupts off, immediately before the call. Check that no interleaving lets the CAS win after the lock was
   dropped.
2. **The holder's witness.** The scenario also requires the holder to report that it saw `CALLING` at its swap, and
   then `done ≥ released`. Check that both are asserted for every leg.
3. **A lost claim makes no call.** It retries with a new holder (3 attempts), then FAILs
   `entries_released_before_call`. Check that a lost claim can't fall through to an uncontended call that counts as
   a pass.
4. **The late-call control.** `neo-test-vfs-syscall-if-on-late-call-control` claims only after the release. It must
   FAIL `entries_released_before_call`, not pass or time out. Check that it isn't in the gate list.
5. **Test-only.** Every change in `kernel/src/fs/mod.rs`, `kernel/src/fs/neofs/inode.rs` and
   `kernel/src/fs/neofs/mount.rs` is behind `neo-test-phase3-neofs-syscall-if-on`, so the product kernel is
   unchanged. S2 reports identical `.text` for the default, desktop and product-real-lm builds.
6. **The IF-off control** still hangs at the first leg (`model_load`), so the fix itself is still proven.

## Close condition

- Commit `Plans/RETURN-2026-10-25-syscall-if-on-gate-recheck-jarvis.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
