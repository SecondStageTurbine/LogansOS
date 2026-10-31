<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #75): narrow re-check of your relay #70 item 1, after #138 → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own finding. Relay #70 item 1 showed
that the selftest's expiry witness matched its sleeper by deadline, so a USB port-service sleeper with the same
deadline could overwrite the early-expiry record. #138 (S2) keys the witness to the worker's thread id instead. It's
test-only (`neo-test-usb-hotplug`).

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `c3aae11c` (main; the squash of PR #138, merged from the batch-1 gate
`2da79dd3`, run `40363142380`, green).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/tick-sleep-witness-recheck-chad
          git fetch origin && git checkout -B codex/tick-sleep-witness-recheck-chad c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde

git show origin/main:Plans/DISPATCH-2026-10-26-tick-sleep-witness-recheck-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-26-tick-sleep-early-recheck-chad.md        # your relay #70
git diff dde0d988 c3aae11c -- kernel/src/sched/tick_sleep.rs
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='tick_sleep SELFTEST_TID SELFTEST_EXPIRED_AT on_tick_expire selftest'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete schedule (tick numbers) like your #70 one.

1. **Witness keyed to the worker.** `on_tick_expire` records `SELFTEST_EXPIRED_AT` only for a slot whose
   `token.tid` equals `SELFTEST_TID`, and only the first time (`compare_exchange` from 0, stored value at least 1).
   Replay your #70 overwrite schedule: the port service's same-deadline slot must not change the record.
2. **The tid is published before the sleeper is visible.** The worker stores `SELFTEST_TID` before it publishes
   `DEADLINE` and calls `sleep_until`, so no expiry of the worker's slot can happen while `SELFTEST_TID` is still
   `u64::MAX`. `current_thread_id()` is the id the tick-sleep token carries.
3. **`installed` matches the worker's slot.** The parked check requires both the tid and the deadline.
4. **Your #61 schedule still fails.** Start 100, deadline 120, the early control expires the worker at real tick 119,
   it resumes at 120: `woken_early=true`, FAIL `tick_sleep_not_parked`. The no-wake and never-park controls still fail.
5. **Reset between runs.** `SELFTEST_TID` and `SELFTEST_EXPIRED_AT` are reset so a second selftest run can't read the
   first run's record.
6. **Production unchanged.** Everything added is under `neo-test-usb-hotplug`.

## Close condition

- Commit `Plans/RETURN-2026-10-26-tick-sleep-witness-recheck-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
