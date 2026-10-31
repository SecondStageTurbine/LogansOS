# PM-DISPOSITION: perf-proxy diagnostic consumption — verdict (A), fix direction

**Date:** 2026-05-29
**From:** PM (Cerebro)
**Reference:** Plans/RETURN-2026-05-29-perf-proxy-diagnostic-jarvis.md (commit 93c5f75)

---

## Verdict consumed: (A) SMP-naive test fixture. Kernel is CLEAN.

Jarvis's experiments settle it:
- **NEO_SMP=1:** deterministic PASS at 3/3/2.
- **NEO_SMP=4:** non-deterministic — PASS at 3/3/2 on one run, reproduced FAIL at 5/5/4 on repeat.
- Experiment B (bisect) correctly NOT run, since SMP=1 never drifts.

The drift is SMP interleaving sensitivity: under multiple CPUs, AP cores opportunistically pick up the two proxy worker threads (pushed to the global ready queue via `push_ready`), adding non-deterministic context switches and CR3 writes. There is **no deterministic kernel regression**. BUILD-1a/1b/2 introduced no scheduler defect.

"We don't ship broken things" is satisfied: nothing in the kernel is broken. The TEST is mis-scoped — it asserts exact scheduler counter values (3/3/2) that only hold deterministically on a single CPU, but the full sweep runs it under NEO_SMP=4.

This also retroactively explains the BUILD-1b blocker: `irqsafe_mutex_bench` (an exact-latency-threshold benchmark) is the same class — deterministic micro-measurement, noisy under SMP=4. Both recurrences share one root cause: **exact-measurement perf-proxies enforced under multi-CPU conditions where their assertions are non-deterministic.**

---

## Fix direction (one principled fix for the whole class)

Perf-proxies / exact-measurement benchmarks are deterministic ONLY under single-CPU execution. Their regression-detection value is fully realized at NEO_SMP=1. The functional sweep runs NEO_SMP=4. Therefore:

**Gate exact-measurement perf-proxies to enforce only under NEO_SMP=1.** Under NEO_SMP>1 they either skip with a clear "informational: SMP-sensitive, not enforced" line or are excluded from the SMP>1 sweep. The SMP=1 stability lane must actually run them so the regression-detection value is preserved.

This is the concrete realization of the PL-ratified BUILD-1b separate-lane principle (Option 2). It closes BOTH recurrences (phase2_perf_proxy_baseline AND irqsafe_mutex_bench) and lets us re-enable a GREEN full sweep under SMP=4.

**Primary mechanism (Jarvis to implement):** SMP-gate the enforcement of phase2_perf_proxy_baseline and irqsafe_mutex_bench (and any sibling exact-count/exact-latency perf-proxies) to NEO_SMP=1.

**Optional enhancement (only if cheap + counters are BSP-scoped):** pin the proxy worker threads to the BSP via `place_thread_on_cpu(worker, 0)` instead of `push_ready`, making the measurement deterministic under any NEO_SMP. Do NOT pursue if the global proxy counters can still be bumped by AP idle-loop scheduling decisions — in that case the SMP=1 gate is the correct and sufficient fix. Jarvis decides feasibility during implementation; the SMP=1 gate is the floor.

---

## Scope guard

- Test-fixture / sweep-config change only. NO kernel scheduler code changes (the kernel is correct).
- After the fix: full sweep under NEO_SMP=4 should be GREEN (perf-proxies not enforced there); SMP=1 stability lane enforces the proxies at their deterministic baselines.

---

## PL flags

- This formalizes a lane structure (SMP=1 perf-proxy/benchmark stability lane vs SMP=4 functional sweep). It is the natural realization of the already-ratified separate-lane principle; PM is proceeding. Flagged for PL ratification at fix close.
- Carried: sendto/recvfrom ABI (0x186/0x187) still awaiting PL stamp.

## Worktree hygiene

Jarvis flagged an untracked `jira-dispatcher.py` in the worker tree, left untouched — correct (unrelated to NeoPlutarch, not ours to commit/delete in a feature branch). Noting for awareness; no action.

---

**Verdict: kernel clean. Fix dispatched as a test-fixture SMP-gate. BUILD-3 unblocks once the green SMP=4 sweep is confirmed.**