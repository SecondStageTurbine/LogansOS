# PM-DISPOSITION: perf-proxy SMP-gate fix ACCEPTED; full-sweep-green now blocked by sys_wait_multiple_waiters

**Date:** 2026-05-29
**From:** PM (Cerebro)
**Reference:** Plans/RETURN-2026-05-29-perf-proxy-smp-gate-jarvis.md

---

## Perf-proxy SMP-gate fix: ACCEPTED and MERGED (a4d37f2)

The fix does exactly what it was dispatched to do, and is fixture/harness-only (smp.rs +11 cfg-gated helper, test_harness.rs +52), no scheduler changes:

- **SMP=1:** both proxies enforce, identical baselines, PASS
  - phase2_perf_proxy_baseline: 3/3/2, baseline=enforced
  - irqsafe_mutex_bench: overhead_ns=4014 < threshold 6000
- **SMP=4:** both emit SKIP then terminal PASS via runtime `reported_cpu_count()` gate
- BSP-pin enhancement correctly NOT applied (gate is sufficient; pinning would alter scenario setup for no benefit)

Both original recurrence blockers (phase2_perf_proxy_baseline, irqsafe_mutex_bench) are RESOLVED. This realizes the PL-ratified separate-lane principle: deterministic perf measurements enforce at SMP=1; the SMP=4 functional sweep no longer trips on them.

The merge is pure, independent progress and does not depend on the new issue below.

---

## New, SEPARATE blocker surfaced: sys_wait_multiple_waiters (SMP=4 full sweep)

With the perf proxies no longer failing first, the SMP=4 full sweep now runs FURTHER than it ever has, and fails at a downstream scenario the proxies had been masking:

- `sys_wait_multiple_waiters` -> FAIL, `reason=process_cap_scenario_exit_nonzero code=6`
- Failed in BOTH full-sweep attempts at the same point
- **PASSES standalone under NEO_SMP=4**

### Why this is a different animal from the perf proxy

- Perf proxy: failed STANDALONE under SMP=4 (intrinsic, non-deterministic) -> was a mis-scoped test, gateable.
- sys_wait_multiple_waiters: PASSES standalone, FAILS in sweep -> signature of either (a) sweep-context flakiness (host/QEMU resource+timing pressure running many SMP=4 boots back-to-back), or (b) a real SMP race in the multi-waiter wait/wake path that the sweep's scheduling pressure triggers.

### Critical: this is NOT gateable, and it is NOT a Phase 4 regression

- `sys_wait_multiple_waiters` is a Phase 2 capability/wait FUNCTIONAL test. Functional correctness under SMP is a CORE requirement (Bible Article 8.3, SMP from Phase 2). Unlike a perf proxy, we CANNOT gate it to SMP=1 to make the sweep green — that would hide a possible real concurrency bug. If it's a race, it needs a real fix.
- It predates Phase 4 entirely. It was likely never exercised in an SMP=4 full sweep before, because the perf proxies (earlier in run order) always failed first under NEO_TEST_ATTEMPTS=1. So this is a LATENT issue newly exposed, not something BUILD-1/2 introduced.

### Confound to control: NEO_TEST_ATTEMPTS=1

Jarvis ran the sweep with NEO_TEST_ATTEMPTS=1. The harness default is 4 (TEST_SCENARIO_ATTEMPTS_DEFAULT). With 1 attempt, a mildly-flaky scenario gets no retry buffer. This must be controlled in the diagnostic: is the failure hard (fails at default 4 attempts too) or retry-maskable (passes at 4)? Either way, "don't ship broken things" means we determine whether there's a genuine race before relying on retries to paper over it.

---

## Decision

1. Perf-proxy fix: CLOSED, merged at a4d37f2.
2. Full-sweep-green milestone: REMAINS OPEN, now blocked solely by sys_wait_multiple_waiters.
3. Open a diagnostic for sys_wait_multiple_waiters (dispatch issued) using the same diagnose-before-fixing discipline that worked for the perf proxy. No fix, no gating, until the real-race-vs-flakiness question is settled.
4. BUILD-3 (IPv4+DHCPv4) stays held until the SMP=4 sweep is genuinely green (i.e., sys_wait is understood and resolved).

## PL flags (carried)

- sendto/recvfrom ABI (0x186/0x187) — still awaiting PL ratification stamp.
- SMP-lane formalization (perf proxies enforce at SMP=1) — flagged for PL ratification; PM proceeded as it realizes the prior ratified principle.

---

**Perf-proxy fix accepted/merged. sys_wait_multiple_waiters diagnostic dispatched. Sweep-green and BUILD-3 held pending verdict.**