# RETURN: perf-proxy SMP-gate fix

**Date:** 2026-05-29
**From:** Jarvis
**To:** PM
**Branch:** `codex/perf-proxy-smp-gate-jarvis`
**Status:** FIXTURE IMPLEMENTED; BLOCKED ON UNRELATED FULL-SWEEP FAILURE

## Summary

Implemented runtime SMP gating for the two SMP-sensitive perf fixtures:

- `phase2_perf_proxy_baseline`
- `irqsafe_mutex_bench`

Under `NEO_SMP=1`, the existing assertions remain identical. Under `NEO_SMP>1`, each scenario emits an informational `NEO_TEST/1 event=SKIP ... reason=smp_sensitive_enforced_only_at_smp1 ...` frame, then emits a terminal `NEO_TEST/1 event=PASS ... skipped=true ...` frame so `xtask` records the scenario as successful.

## Gating Mechanism

The gate uses a runtime CPU-count check via:

- `kernel/src/arch/x86_64/smp.rs`: `reported_cpu_count()`

The helper reads Limine's MP response and defaults to `1` if no response is available. It is compiled only for:

- `neo-test-phase2-perf-proxy-baseline`
- `neo-test-irqsafe-mutex-bench`

No scheduler logic was changed.

## Optional BSP Pin

Not applied. The runtime SMP gate is sufficient for the fixture problem, and pinning worker placement would change the scenario setup without being needed to preserve the SMP=1 regression lane.

## Verification

Host feature checks:

- `cargo check -p neoplutarch-kernel --features neo-test-phase2-perf-proxy-baseline` -> PASS
- `cargo check -p neoplutarch-kernel --features neo-test-irqsafe-mutex-bench` -> PASS

SMP=1 regression lane:

- `docker compose run --rm -e NEO_SMP=1 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase2_perf_proxy_baseline --no-meta` -> PASS
  - Serial: `context_switches=3 scheduling_decisions=3 cr3_performed=2 cr3_skipped=1 alloc_calls=1 dealloc_calls=1 baseline=enforced`
- `docker compose run --rm -e NEO_SMP=1 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test irqsafe_mutex_bench --no-meta` -> PASS
  - Serial: `spin_mean_ns=420 irqsafe_mean_ns=4434 switch_mean_ns=1491 overhead_ns=4014 threshold_ns=6000`

SMP=4 targeted fixture behavior:

- `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase2_perf_proxy_baseline --no-meta` -> PASS via SKIP/PASS
- `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test irqsafe_mutex_bench --no-meta` -> PASS via SKIP/PASS

Full SMP=4 sweep:

- `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta` -> NOT GREEN
- Attempt 1 log: `target/neo-test/perf-proxy-smp-gate-smp4-all-20260529.host.log`
- Attempt 2 log: `target/neo-test/perf-proxy-smp-gate-smp4-all-rerun-20260529.host.log`
- Both attempts passed `irqsafe_mutex_bench` and `phase2_perf_proxy_baseline`, then failed later at `sys_wait_multiple_waiters`:
  - `xtask: sys_wait_multiple_waiters did not exit via debug success; status=exit status: 35`
  - Serial: `NEO_TEST/1 event=FAIL name=sys_wait_multiple_waiters reason=process_cap_scenario_exit_nonzero code=6`
- Standalone rerun of `sys_wait_multiple_waiters` under `NEO_SMP=4` passed.

This leaves the requested full-sweep green gate blocked by an unrelated later scenario, not by the perf proxy fixtures.

## Commits

- Fixture implementation: `1ff59ca`
- Return artifact: carried by this follow-up commit
