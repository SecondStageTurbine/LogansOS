<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #69): narrow re-check of your relay #64 items 4–6, after #131 → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own earlier findings on #126. #131 (S1) is
the fix-forward:
- **item 4:** the `prefill_perf` commit slots could mix two pids' stores;
- **item 5:** the vocab hooks took the scheduler lock with IF off;
- **item 6:** there was no open-without-close test.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `047f46ee` (main; the squash of PR #131, gated as `348e399e`, run `40253348390`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/arthur-startup-fix-recheck-janus
          git fetch origin && git checkout -B codex/arthur-startup-fix-recheck-janus 047f46ee4a3d6a1dcc1459af00c0e1a7a8ad25bd

git show origin/main:Plans/DISPATCH-2026-10-26-arthur-startup-fix-recheck-janus.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-arthur-startup-review-janus.md          # your relay #64
git diff 993b0aeb 047f46ee -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='commit_slot_logic PID_MASK note_syscall note_vocab_open note_close'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete interleaving or input.

1. **Item 4, one store per commit.** Each slot is one `AtomicU64` holding `tsc & !PID_MASK | pid & PID_MASK`
   (`kernel/src/ai/commit_slot_logic.rs`), published by a single store and read with a single load. Check that no reader
   can see one pid's tsc with another pid's id. More than 8 committing pids evict (the later commit wins, and the
   evicted pid reads `think=-`). Say what precision the truncated tsc leaves, and whether the overlay's think= is still
   correct at that precision.
2. **False match.** A wrong match needs two pids equal mod 8 and in their low 20 bits. With the monotonic pid
   allocator, confirm that's out of reach in practice. Also check that the `#130` merge kept #130's
   `write_perf_line` reading the new slot word correctly.
3. **Item 5, no lock.** `note_syscall` returns the pid it already computes at dispatch, and the vocab hooks take it as
   a parameter. Check that `note_vocab_open` and `note_close` now take no lock and change no IF state. Check that the pid
   they use is the caller's (not stale from an earlier syscall).
4. **Item 6, test.** `fb-progress-logic-tests` covers `vocab=O..-` (open without close). The host KATs in
   `ai-ops-tests` cover the slot word's packing and the eviction rule.
5. **Compiled out.** All of it stays under `freeze-watchdog` / `prefill-perf`; nothing is added to a kernel without
   those features.

## Close condition

- Commit `Plans/RETURN-2026-10-26-arthur-startup-fix-recheck-janus.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
