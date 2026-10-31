# PM decision: take Q1 to N=50 on main HEAD before closing sub-commit 2

**Date:** 2026-05-07
**Author:** PM (incoming, post-handoff)
**Status:** Decision recorded; higher-N Q1 dispatch issued
**Substrate:** Cerebro completed the original 10-run Q1 + Q2a; results are ambiguous by the diagnostic dispatch's own decision rule. PM-DECISION-2026-05-06-kaslr-overflow-blocks-closure.md and DISPATCH-2026-05-06-kaslr-overflow-diagnostic-cerebro.md are the parents. Project lead selected this path.

---

## Cerebro's diagnostic results (verbatim summary)

- **Q1, clean main HEAD `9505619`, NEO_SMP=4 default suite, 10 runs:** PANIC=0, PASS=7, OTHER=3, OVERFLOW_MATCHES=0. The three OTHER runs were 180s timeouts on `smap_catch` (run 1), `slab_local_to_global_promotion` (run 8), and `buddy_max_order` (run 10). Each timeout happened at the QEMU launch / scenario-start phase with no scenario-body output. Different scenario each time.
- **Q2a, sub-commit 2 working tree, KASLR disabled via `boot/limine.conf` `kaslr: no`:** PASS, deterministic kernel base `0xffffffff80000000`, `ipc_heap_kill_smoke` clean.
- **Q2b skipped:** repo only exposes Limine `kaslr: yes/no`, no seed override.

PM verified: timeout logs grep clean for `attempt to add with overflow`, `panicked`, and `virt=0xffffff7…` (the high-end-of-canonical-space placement signature). Cerebro's PANIC regex was reliable; the OTHERs are the harness wedge from the open triage queue (handoff §7), not the KASLR overflow.

## What this means by the diagnostic's own rule

The diagnostic dispatch's §6 decision rule says, verbatim:

> Q1 PANIC does NOT reproduce in 10 runs on main HEAD AND Q2a/Q2b show KASLR is the trigger: ambiguous. Could mean main HEAD got lucky 10 times in a row (~99% probability if base rate is even ~1/30), or could mean sub-commit 2 amplified the bug. PM dispatches a higher-N Q1 (50 or 100 runs).

We are squarely in that bucket. The dispatch wrote down what to do next; do it.

## Why not "accept Q1 as sufficient" given the napkin-stats argument

Napkin stats — 1 panic / ~125 boots on sub-commit 2 vs 0 panics / ~250 boots on main, statistically indistinguishable — are real but not load-bearing for the decision. Two reasons:

1. **The diagnostic dispatch's decision rule was written into the parent dispatch deliberately, with the closure pressure of the moment factored in. We're bound by the rule, not by a re-litigation of the rule.** Lowering the bar after the fact, when the bar was specifically chosen to be conservative, is an instance of the same shape of mistake the §Q5 lesson and the closure-gate-overspecification lesson were meant to inoculate against — pressure to commit driving an evidence-quality argument that wouldn't survive a calmer reading.
2. **The cost is small relative to the value.** ~3–8h of Cerebro time, almost certainly producing 50 more clean runs that tighten the upper bound on main's overflow rate from ~26% (10 runs, 95% CI by rule of three: 3/N) to ~6% (50 runs, 95% CI). At the tighter bound the "main got lucky 10 times in a row" alternative becomes implausible enough that the decision rule resolves cleanly in favor of pre-existence.

## Why not "go to N=100" instead of N=50

The dispatch wrote "50 or 100." 50 is the right pick:

- Each suite run is ~3–6 minutes (build cache hot) up to ~30 minutes when one of the harness-wedge timeouts fires (each timeout costs 180s + boot overhead + remaining-suite cost). 50 runs is a workday; 100 runs is a workday plus overflow into tomorrow.
- 50 runs gives ~6% upper bound at 95% CI (rule of three: 3/50). That's well below the ~0.8% observed rate that prompted the diagnostic. Enough resolution to call it.
- If a panic *does* surface in the first 50, we have our answer immediately and didn't waste the back half of the day.
- If 50 are clean, we accept and move forward. We don't ladder up to 100 just for a tighter bound that wouldn't change the call.

## Working tree state at dispatch time (2026-05-07 09:07)

- `git diff HEAD` shows 11 modified tracked files matching post-EXP-H state (per stash@{0} stat).
- `Plans/` has the recovered session artifacts plus this decision and the dispatch.
- `git stash list` shows `stash@{0}: On main: sub-commit-2-close-attempt-blocked-on-kaslr` — Cerebro kept it as a safety net per his close report. We don't drop it until sub-commit 2 commits.
- `live_thread_count` and `[ DBG ]` greps return zero matches in code (matches in ADR-013 are inside the amendment narrative, expected).

This state is correct for the dispatch to operate on. The dispatch instructs Cerebro to stash again (creating a second stash for the pre-Q1B state), run the higher-N on main, and `git stash apply` (not `pop`) to keep the safety net intact through the second pass.

## What changes after Cerebro's higher-N report lands

Decision tree (committed in advance, so post-result decision isn't made under closure pressure):

- **Q1 panic reproduces ≥ 1 time in 50 runs on main HEAD:** pre-existence confirmed. Issue corrected closure dispatch (gate downgraded to the original bisection gate of 100x ipc_heap_kill_smoke at NEO_SMP=4, which EXP-H already met). KASLR overflow becomes a separate triage item.

- **Q1 panic does NOT reproduce in 50 runs on main HEAD:** the 95% upper bound on main's overflow rate drops to ~6%, well below the observed sub-commit 2 rate. Combined with Q2a's confirmation that KASLR is the trigger and the unchanged-`mm/heap.rs` argument, this is sufficient to accept "pre-existing latent" with reasonable confidence. Issue corrected closure dispatch on the same shape as above. KASLR overflow becomes a separate triage item with the higher-N evidence cited.

- **Q1 produces 1+ panics AND timeout count is materially different from the original 10-run rate (3/10 = 30%):** flag. The harness-wedge rate is one of the open triage items; if N=50 dramatically changes that rate signal, that's its own signal worth recording before closure.

- **Anything weirder:** stop, surface, re-think.

In all branches, the corrected closure dispatch uses **the original bisection gate** (100x ipc_heap_kill_smoke at NEO_SMP=4) and treats the full default suite at NEO_SMP=4 as informational, not gating. This is the closure-gate lesson from yesterday's PM self-criticism, applied.

## Pacing

Cerebro starts the dispatch at ~09:10. The loop is single-task, low-cognitive-load (no fix, no commit, no design work — just stash, run, capture). Cerebro can kick off the loop and let it run mostly hands-off, periodically checking progress. Jarvis stays idle.

PM-side parallel work runs while Cerebro's loop runs. First parallel item: KASLR overflow ADR draft (decision per project lead this morning). The ADR draft proceeds in parallel because it doesn't block on the higher-N result — pre-existence is the leading hypothesis, the ADR's value holds whether main reproduces or not, and the ADR can be sharpened once Cerebro's results land.

## Time budget projection

Cerebro starts ~09:10:
- ~5 min stash + clean checkout verify
- ~3 min main HEAD `cargo k` + binary-size capture
- ~150–500 min for 50 NEO_SMP=4 default-suite runs (each 3–10 min depending on harness-wedge timeouts; 3/10 yesterday is the planning assumption — that's a 12–30% timeout rate, expected to push toward the high end of the budget)
- ~3 min sub-commit-2 `cargo k` + binary-size capture
- ~10 min stash apply + verify restore + close report

Best case Cerebro's report lands ~12:00. Worst case ~17:00–18:00. If the worst case hits, sub-commit 2 closure slides into tomorrow morning regardless — at that point Cerebro is at the end of his second consecutive day on this and accepting the result with limited PM-side review headroom is the wrong call.

If at hour 6 Cerebro isn't done, surface — partial-N (e.g., 30 runs gets a ~10% upper bound, weaker but still meaningful) is the fallback.

## Lessons compounding

This decision continues the §Q5 / closure-gate-overspecification lesson chain:

- §Q5: bets that look defensible can fail; verify before committing.
- Closure-gate overspecification: don't add stricter gates than what was specified at design time.
- This one: when a written decision rule says "ambiguous → escalate," don't relitigate the rule under closure pressure to short-circuit the escalation. The rule earned its conservatism honestly.

## Plans/INDEX.md staleness

INDEX.md (the only committed file in Plans/) is yesterday's snapshot. It's missing entries for: EXP-F, EXP-G, the bisect-step1 dispatch, the close attempt, the kaslr-overflow PM decision, the kaslr diagnostic dispatch, the kaslr-overflow-blocks-closure PM decision, and this dispatch + decision. It also still lists older items as `active` that have since either been superseded or closed.

**Decision: defer the INDEX.md update.** Per yesterday's handoff §10 step 6, the INDEX.md sweep is one of Jarvis's first tasks *after* sub-commit 2 closes. We're not ready to dispatch that yet — sub-commit 2 hasn't closed. Updating INDEX before then risks:
- A doc-only commit that introduces work-tree noise during the higher-N diagnostic.
- Re-categorizing entries (e.g., flipping `active` to `closed`) before the underlying work is actually closed, which is the kind of premature-commit-hygiene the project has been disciplined about avoiding.

**Once sub-commit 2 closes:** dispatch Jarvis with a single INDEX.md sweep covering everything from 2026-05-06 14:00 through 2026-05-07's closure. The sweep is single-shot, scoped, and respects the rule that index updates trail the underlying work.

---

**End of decision note.**