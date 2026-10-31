<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-74 — turn "flaky" into a number (log-scrape only) → Oracle

**Date:** 2026-08-02 · **From:** PM (Claude) · **To:** Oracle (Claude)
**Status:** DISPATCHED — **measurement only. No branch, no build, no gate, no runner.** Pure CI-log scraping.

**Why this is not new scope under the break gate:** the **integration gate blocking three merge-ready lanes** (WiFi + Slice-4 + rung-5) died on **`sys_generate_kv_persist_equivalence`** on 2026-08-02. Quantifying these two scenarios is diagnosing the *current* blocker, not opening a lane.

## The ask (yours, accepted as proposed)
Per-attempt outcomes are already in CI logs. Count **attempt-1 failures** for **`arthur_reversibility`** and **`sys_generate_kv_persist_equivalence`** across recent runs and report:
1. **Attempt-1 failure rate** per scenario — the headline number. This is the thing the retry-4 wrapper currently hides: a scenario failing attempt 1 half the time is **indistinguishable in the summary** from one that never fails.
2. **Runner correlation** — does the rate differ across `ryzen-1..4` (the 128 GB Ryzen 9 box) vs `ryzen-5/6` (the M75q, `<runner-host>`)? You proved the two hosts are independent, so this separates a code race from a host-capacity effect. **Report it even if it's flat** — flat is the more interesting answer, because it means the race is ours everywhere.
3. **Duration histogram, and confirm or kill the bimodality.** Your clue is the sharpest thing we have: `arthur_reversibility` at **7.2 s / 7.3 s or the full 420 s**, four hangs running, **nothing in between**; `sys_generate_kv_persist_equivalence` at **84.5 s or hung to 180 s**. If that holds across a larger sample it is decisive — a slowness problem produces a *distribution*, a bimodal split produces a *switch*. **State the sample size**; if the tail is thin, say so rather than asserting the shape.
4. **Any other retry-dependent scenario.** The general hazard is bigger than these two: **any** scenario relying on retry to stay green is a race wearing a pass. If the scrape surfaces others, name them — that list is the real deliverable.

## What NOT to do
- **Do not raise any timeout.** A budget increase on a bimodal distribution buys a longer wait for the same hang. Recorded in OT-74 so nobody reaches for it later.
- **Do not fix the race.** That is a separate kernel-touching lane, main-tier, after the break. This dispatch produces a *number*, not a patch.
- **No branch, no gate.** If you find yourself needing either, stop and tell me — the scope drifted.

## Deliverable
A short RETURN with the four items above. If the numbers say "nuisance," we deprioritise honestly. If they say a scenario fails attempt 1 often, then **every lane has been paying a hidden tax all along** and OT-74 jumps the post-break queue. Either answer is worth having; today we have neither.

## Relay — Oracle
- **WORKTREE:** `E:\AI Automation\workers\oracle` (yours alone) · **BRANCH:** none — measurement only.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-02-ot74-flake-rate-measurement-oracle.md`, and OT-74 in `git show origin/main:docs/OPEN-THREADS.md`.
- **One-line:** **Word said — pull the numbers.** Exactly as you proposed: attempt-1 failure rates for `arthur_reversibility` and `sys_generate_kv_persist_equivalence` from existing CI logs. Four things: **(1)** the attempt-1 failure rate per scenario — the number the retry-4 wrapper hides, since fails-half-the-time and never-fails look identical in the summary; **(2)** runner correlation, `ryzen-1..4` vs `ryzen-5/6` — you proved the hosts are independent, so this separates a code race from host capacity, and **report it even if flat**, because flat means the race is ours everywhere; **(3)** a duration histogram to **confirm or kill your bimodality** — `7.2/7.3 s or 420 s` with nothing between is the sharpest clue we have, and if it holds at larger n it's decisive, so **state the sample size** and say if the tail is thin; **(4)** any *other* retry-dependent scenario — that list is the real deliverable, because any scenario relying on retry to stay green is a race wearing a pass. **Do not raise a timeout** (a budget bump on a bimodal split buys a longer wait for the same hang — recorded in OT-74 so nobody reaches for it later) and **do not fix the race** (separate kernel lane, post-break). **This isn't new scope:** the integration gate holding three merge-ready lanes died on `sys_generate_kv_persist_equivalence` today, so this is diagnosing the current blocker. No branch, no gate, no runner.
- **RAG** not needed for this one (it's CI-log scraping, not code search) — use it if you end up looking for the scenario definitions.
