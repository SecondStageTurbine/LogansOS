<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #70): narrow re-check of your relay #61 item 5, after #132 → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own earlier finding. Relay #61 item 5
showed that `neo-test-tick-sleep-early-control` could pass if the woken thread was delayed by one tick before
resuming. The selftest judged "early" only by the resume tick. #132 (S2) is the fix. It's test-only.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `e4c4baec` (main; the squash of PR #132, gated as `c7791c35`, run `40258294674`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/tick-sleep-early-recheck-chad
          git fetch origin && git checkout -B codex/tick-sleep-early-recheck-chad e4c4baec6b5a03e54f457418db07778d1cf6a3a7

git show origin/main:Plans/DISPATCH-2026-10-26-tick-sleep-early-recheck-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-discriminators-hwp-recheck-chad.md   # your relay #61
git diff 047f46ee e4c4baec -- kernel
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='tick_sleep on_tick_expire selftest expired_at woken_early DEADLINE'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete schedule (tick numbers) like your #61
counterexample.

1. **Expiry is witnessed.** Under `neo-test-usb-hotplug`, `on_tick_expire` records the real `TICKS` at which it
   expires the selftest's own sleeper. It matches that sleeper by the deadline the worker published. Check that the
   record can't be taken from a different sleeper (the USB port service sleeps in the same table).
2. **Early is judged at expiry.** `woken_early = expired_at < deadline || woke_at < deadline`, and a missing expiry
   (0) also counts as early. Replay your #61 schedule: start 100, deadline 120, the early control expires it at real
   tick 119, and it resumes at 120. It must now FAIL `tick_sleep_not_parked` with `woken_early=true`.
3. **The other controls still fail.** The no-wake control fails within the 200-tick wait, and the never-park control
   still fails.
4. **Production unchanged.** Everything added is under `neo-test-usb-hotplug`. S2 reports identical `.text` for
   desktop+tick-sleep and product-real-lm.

## Close condition

- Commit `Plans/RETURN-2026-10-26-tick-sleep-early-recheck-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
