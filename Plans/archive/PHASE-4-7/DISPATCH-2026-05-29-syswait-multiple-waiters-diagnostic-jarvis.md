# DISPATCH: sys_wait_multiple_waiters sweep-vs-standalone — root-cause diagnostic

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Jarvis
**Branch:** `codex/syswait-diagnostic-jarvis` (from origin/main HEAD a4d37f2)
**Type:** DIAGNOSTIC ONLY — no kernel/test/harness code changes without a follow-up dispatch.

---

## Why

The perf-proxy SMP-gate fix landed (a4d37f2), so the SMP=4 full sweep now runs further than ever and exposes a downstream failure the proxies had masked:

- `sys_wait_multiple_waiters` -> FAIL `reason=process_cap_scenario_exit_nonzero code=6`, both sweep attempts, same point.
- PASSES standalone under NEO_SMP=4.

This is a FUNCTIONAL Phase-2 wait/wake test. It cannot be gated to SMP=1 (SMP correctness is core, Bible 8.3). So we must determine: real SMP race, or sweep-context flakiness? We do NOT fix or retry-mask until that is settled.

## PM analysis already done (don't redo)

- The scenario predates Phase 4; it was likely never reached in an SMP=4 sweep before because the perf proxies failed first under ATTEMPTS=1. So this is latent, not a BUILD-1/2 regression (confirm in Exp C).
- Jarvis ran the sweep at NEO_TEST_ATTEMPTS=1 (no retries); harness default is 4. The retry setting is a confound to control (Exp B).
- "Passes standalone, fails in sweep" points to either host/QEMU resource+timing pressure across back-to-back SMP=4 boots, or a real race that scheduling pressure triggers.

## Experiments (run and REPORT; do not fix)

### Exp A — standalone repeat-stress (intrinsic race detector)
Run `sys_wait_multiple_waiters` standalone under NEO_SMP=4, many iterations:
```
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test sys_wait_multiple_waiters --no-meta
```
Repeat ~20 times (script a loop). Record pass/fail tally.
- Any standalone failure => intrinsic SMP race (not sweep-specific). 
- 20/20 pass => failure is sweep-context-specific (resource/timing or ordering).

### Exp B — sweep at default attempts (retry-maskable?)
Run the full sweep WITHOUT forcing attempts=1 (use harness default 4):
```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all --no-meta
```
- Sweep goes green => failure is retry-maskable flakiness. (We still report it; we do not declare victory on retries alone if Exp A shows a race.)
- Still fails sys_wait_multiple_waiters at 4 attempts => hard failure in sweep context.

### Exp C — decode code=6 + locate the assertion
In the scenario source (search test_harness.rs / neo-init for sys_wait_multiple_waiters and exit code 6), report what condition makes the user process exit 6 — i.e., WHICH waiter gets WHICH wrong result. This tells us the failure semantics (e.g., a waiter returns the wrong pid/status, a wake is lost, a double-wake, etc.).

### Exp D — pre-existing vs introduced (only if Exp A shows a race)
Run Exp A (standalone repeat-stress, ~20x) at a pre-Phase-4 commit (Phase 3 close, e.g. the commit tagged at Phase 3 closure / before 43fc2da). 
- Fails there too => pre-existing Phase 2 latent race.
- Passes there, fails on current main => introduced between Phase 3 close and now; bisect the window.

## Deliverable

File `Plans/RETURN-2026-05-29-syswait-diagnostic-jarvis.md` with:
- Exp A tally (X/20 pass standalone SMP=4)
- Exp B outcome (sweep green at default attempts? where does it fail if not?)
- Exp C: the exact assertion behind code=6 and the failing waiter semantics
- Exp D (if run): pre-existing vs introduced, with the culprit commit/window if introduced
- One-line verdict: (A) sweep-context flakiness / (B) intrinsic SMP race [pre-existing | introduced-at-X]
- NO code changes. Fix dispatch follows PM review.

## Guardrails
- Diagnostic only. No kernel, test, or harness edits. No gating. No retry-count changes as a "fix."
- Leave `jira-dispatcher.py` untouched.
- Return the worktree to origin/main HEAD when done.