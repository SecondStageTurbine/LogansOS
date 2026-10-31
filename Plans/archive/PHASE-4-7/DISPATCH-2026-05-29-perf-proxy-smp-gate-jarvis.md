# DISPATCH: perf-proxy SMP-gate fix (test fixture; kernel stays untouched)

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Jarvis
**Branch:** `codex/perf-proxy-smp-gate-jarvis` (from origin/main HEAD)
**Type:** Test-fixture / sweep-config fix ONLY. Do NOT modify kernel scheduler code — the kernel is confirmed correct.

---

## Why

Your diagnostic (RETURN 93c5f75) proved verdict (A): phase2_perf_proxy_baseline is SMP-naive. SMP=1 is deterministic 3/3/2; SMP=4 is non-deterministic (3/3/2 or 5/5/4) because AP cores steal the two proxy workers off the global ready queue. No kernel regression. The same class covers irqsafe_mutex_bench (BUILD-1b).

Fix: exact-measurement perf-proxies are deterministic only single-CPU. Enforce them at NEO_SMP=1; do not enforce under NEO_SMP>1. This realizes the PL-ratified BUILD-1b separate-lane principle and gets the SMP=4 full sweep green.

## What to do

### 1. SMP-gate the perf-proxy enforcement (primary, required)

For `phase2_perf_proxy_baseline` and `irqsafe_mutex_bench` (and any sibling exact-count / exact-latency perf-proxy you find in the same family):

- When running under NEO_SMP=1: enforce the exact baseline as today (PASS/FAIL on the counters/threshold).
- When running under NEO_SMP>1: do NOT enforce. Emit an informational line, e.g.:
  `NEO_TEST/1 event=SKIP name=phase2_perf_proxy_baseline reason=smp_sensitive_enforced_only_at_smp1 observed_context_switches=.. observed_cr3_performed=..`
  and exit success (or exclude from the SMP>1 sweep set — whichever fits the harness cleanly).

How you detect CPU count: use the same NEO_SMP plumbing the harness already exposes (env -> kernel cmdline -> per_cpu count). If the kernel can read the active CPU count at runtime, gate on that; otherwise gate via the harness/xtask sweep set. Pick the lower-risk path.

Preserve the regression-detection value: the SMP=1 lane MUST still run and enforce these proxies. If the project's default sweep is SMP=4, ensure there is an SMP=1 invocation path that runs the perf-proxy family (document the exact command in your RETURN).

### 2. Optional enhancement (only if cheap AND safe)

If — and only if — the `phase2_perf_proxy_counters()` are scoped such that AP idle-loop activity does NOT bump them, you MAY additionally pin the two workers to the BSP:
`crate::sched::scheduler::place_thread_on_cpu(worker_a, 0)` / `(worker_b, 0)` instead of `push_ready(...)`, making the measurement deterministic under any NEO_SMP.
Do NOT do this if AP idle scheduling decisions can still bump the global counters — in that case the SMP=1 gate is correct and sufficient. If unsure, SKIP this enhancement; the gate is the floor and fully resolves the blocker.

### 3. Verify green

- `NEO_SMP=4 cargo neo-test all --no-meta` -> full sweep GREEN (perf-proxies skipped/non-enforced under SMP=4).
- `NEO_SMP=1 cargo neo-test phase2_perf_proxy_baseline --no-meta` -> PASS at 3/3/2.
- `NEO_SMP=1 cargo neo-test irqsafe_mutex_bench --no-meta` -> PASS at its threshold.

## Guardrails

- No kernel scheduler logic changes. Test-harness / xtask / sweep-config only. (The optional pin in step 2 is a test-scenario setup change, not scheduler logic — that's allowed; touching the scheduler itself is not.)
- Don't weaken the proxies' SMP=1 assertions — same exact baselines, just gated.
- Leave `jira-dispatcher.py` untouched (unrelated stray file).

## Deliverable

File `Plans/RETURN-2026-05-29-perf-proxy-smp-gate-jarvis.md` with:
- Which scenarios you gated and the gating mechanism (runtime CPU-count check vs sweep-set exclusion).
- Whether you applied the optional BSP-pin (and why / why not).
- The three verification command outcomes above (full SMP=4 sweep green; SMP=1 proxy PASSes).
- The exact SMP=1 command(s) that run the perf-proxy family (so the regression-detection lane is documented).
- Commit hashes.