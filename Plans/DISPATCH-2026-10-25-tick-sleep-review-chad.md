<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #56): post-merge review of `tick-sleep` (#119) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. It's a new scheduler-wake primitive driven from the timer tick (core
SMP/timer code). It's Claude-authored, so it gets a Codex-family review. **Do this after relay #53.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `bbca1c43` (main; the squash of PR #119, gated as `eecde97f`, run `40174433840`,
23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/tick-sleep-review-chad
          git fetch origin && git checkout -B codex/tick-sleep-review-chad bbca1c439c3a869d4be2fddcde4c163db8def999

git show origin/main:Plans/DISPATCH-2026-10-25-tick-sleep-review-chad.md   # this file
git diff 24ab67a0 bbca1c43 -- kernel tools .github docs/concurrency-pitfalls.md
```

Read `docs/concurrency-pitfalls.md` CP-8, CP-14, CP-73 and CP-81 first.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='tick_sleep park_until on_tick_expire wake_thread_token'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with the enforcing `file:line`. For a FAIL, give a concrete sequence (which CPU does
what, in what order) or input.

1. **No lost wake.** A sleeper whose deadline passes between its check and its park is always woken (`tick_sleep.rs`
   `park_until`, `on_tick_expire`). Check both the tick on the sleeper's own CPU and the tick on another CPU.
2. **Lock order.** `TABLE` is never held while a scheduler lock is taken, except by the park's own
   `block_current_with_token_and_guard`. The tick drops `TABLE` before `wake_thread_token`. No governance lock is taken.
3. **The fast path.** The `SLEEPING` / `EARLIEST` checks can't strand an installed sleeper: each install raises both
   under the lock, and every expiry recomputes `EARLIEST`. Check the memory orderings on those atomics.
4. **A full table.** `Park::Full` falls back to the yield loop and installs nothing. A slot freed by expiry can be
   reused safely while a wake for its previous owner is still in flight.
5. **Feature off.** Every new item is behind `tick-sleep` (or a scenario feature), and both `nap_ticks` keep main's loop
   without it. Name the check that proves it.
6. **The scenario.** `run_usb_hotplug` fails if the test sleep doesn't park, runs early, is never woken, sleeps fewer
   than 20 ticks, parks on a past deadline, or if any nap fell back to spinning. The control
   `neo-test-tick-sleep-control` fails.
7. **A stop request.** A kthread stop request doesn't cut a sleep short, and the sleep ends at its deadline. Check
   that no caller depends on a prompt stop (`usb-ports` is detached and never stopped).

## Close condition

- Commit `Plans/RETURN-2026-10-25-tick-sleep-review-chad.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
