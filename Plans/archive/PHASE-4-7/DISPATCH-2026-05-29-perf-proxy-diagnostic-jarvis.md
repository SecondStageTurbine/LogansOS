# DISPATCH: phase2_perf_proxy_baseline drift — root-cause diagnostic

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Jarvis
**Branch:** `codex/perf-proxy-diagnostic-jarvis` (create from origin/main at current HEAD)
**Type:** DIAGNOSTIC ONLY — no kernel/test code changes without a follow-up dispatch

---

## Why

This is the 2nd consecutive full-sweep block by a Phase-2 perf-proxy:
- BUILD-1b: irqsafe_mutex_bench (overhead_ns)
- BUILD-2: phase2_perf_proxy_baseline (context_switches 3->5, scheduling_decisions 3->5, cr3_performed 2->4; cr3_skipped/alloc/dealloc unchanged)

We do NOT ship broken things, so before recalibrating any baseline we must decisively answer: **real kernel regression, or a test that was never valid under NEO_SMP=4?** Recalibrating over a real bug would mask it; "fixing" a kernel that's fine would waste effort. This diagnostic settles it.

## PM analysis already done (so you don't redo it)

- `drivers::net::init()` (kernel/src/drivers/net/mod.rs) spawns NO thread and enqueues NO task — synchronous PCI scan + logging only. So "persistent net poll task" is ruled out.
- `run_phase2_perf_proxy_baseline()` (kernel/src/test_harness.rs ~L1877) RESETS the scheduler + heap counters immediately before a controlled 2-worker section, so counts are scenario-local, not cumulative-since-boot.
- Each scenario builds with its own cargo feature and boots fresh (suite includes reboot-persistence scenarios), so cross-scenario state bleed within one boot is ruled out.
- The drift is a near-uniform +2 (switches/decisions/cr3-writes) while cr3_skipped/alloc/dealloc match exactly. That is the signature of the two worker threads being picked up by AP cores under SMP, NOT a leak or a busy loop.
- The expected constants (PERF_PROXY_EXPECTED_* in test_harness.rs) were authored in Phase 2. BUILD-1b's sweep died earlier at irqsafe_mutex_bench, so BUILD-2 is plausibly the FIRST time an NEO_SMP=4 sweep ever reached this proxy.

**Leading hypothesis:** the proxy is SMP-naive. Baseline 3/3/2 is a single-CPU figure; under NEO_SMP=4 the workers schedule across APs and legitimately produce 5/5/4. If so, this is a test-fixture problem, not a kernel regression — and BUILD-1b/BUILD-2 introduced no functional defect.

## Your job: run the decisive experiments and REPORT (do not fix yet)

### Experiment A — SMP sensitivity (the decisive one)

Run the proxy standalone at two CPU counts on current origin/main:

```
docker compose run --rm -e NEO_SMP=1 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase2_perf_proxy_baseline --no-meta
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase2_perf_proxy_baseline --no-meta
```

Record the full NEO_TEST/1 line (PASS or FAIL with the counter values) for each.

Interpretation:
- SMP=1 -> 3/3/2 PASS and SMP=4 -> 5/5/4 FAIL => confirms SMP sensitivity. Hypothesis confirmed. Root cause = test fixture, not kernel.
- SMP=1 also FAILS (drifts from 3/3/2) => NOT pure SMP sensitivity; a kernel boot-path change is implicated. Proceed to Experiment B.

### Experiment B — commit bisect (ONLY if SMP=1 also drifts)

Run the proxy standalone at NEO_SMP=1 at each commit, oldest to newest, to find where 3/3/2 first breaks:

```
# 43fc2da  (pre-net baseline: phase-1-3 archive refactor, before BUILD-1a net code)
# c07a4fb  (BUILD-1b socket syscalls)
# 05cff8d  (BUILD-2 merge)
```

For each: `git checkout <sha>`, then the SMP=1 standalone command above. Report the counter line per commit. The first commit that drifts from 3/3/2 is the culprit; name it.

Return to origin/main HEAD when done (`git checkout main` is held by the PM worktree — use `git checkout origin/main` detached or `git switch -` back to your branch).

### Experiment C — confirm SMP=4 reaches the proxy only now (context, cheap)

`git log -p -- kernel/src/test_harness.rs` around the PERF_PROXY_EXPECTED_* constants: report the commit/date they were introduced and whether they were ever accompanied by an SMP>1 expectation. Also note (from xtask) whether the full-sweep run order places irqsafe_mutex_bench before phase2_perf_proxy_baseline (which would explain why BUILD-1b never reached this proxy).

## Deliverable

File `Plans/RETURN-2026-05-29-perf-proxy-diagnostic-jarvis.md` with:
- Experiment A: the two counter lines (SMP=1, SMP=4)
- Experiment B (if run): per-commit counter lines + the first drifting commit
- Experiment C: when the constants were authored + sweep order fact
- Your one-line verdict: (A) SMP-naive test fixture / (B) real kernel regression at commit X
- NO code changes. The fix dispatch follows PM review of your findings.

## Guardrails

- Diagnostic only. Do not edit kernel or test_harness code, do not recalibrate constants, do not touch the sweep config.
- If a run hangs or Docker misbehaves, capture the symptom and report; do not chase infra fixes.
- Keep the branch clean; the only artifact you add is the RETURN.