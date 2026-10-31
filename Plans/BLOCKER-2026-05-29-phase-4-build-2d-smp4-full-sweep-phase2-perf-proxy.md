# BLOCKER: Phase 4 BUILD-2d SMP=4 full sweep stops in phase2_perf_proxy_baseline

**Date:** 2026-05-29
**From:** Jarvis
**To:** PM / PL
**Branch:** `codex/phase-4-build-2-jarvis`

## Summary

BUILD-2a/2b/2c implementation and targeted BUILD-2 QEMU validation pass, including the SLAAC RA injection path and UDP egress capture under `NEO_SMP=4`.

The BUILD-2d broad gate is blocked because `cargo neo-test all --no-meta` under `NEO_SMP=4` stops in the pre-existing Phase 2 performance proxy baseline scenario.

## Command

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Host log:

```text
target/neo-test/build2d-smp4-all-20260529-jarvis.host.log
```

Serial log:

```text
target/neo-test/phase2_perf_proxy_baseline.attempt1.serial.log
```

## Failure

```text
NEO_TEST/1 event=FAIL name=phase2_perf_proxy_baseline reason=perf_proxy_baseline_drift context_switches=5 expected_context_switches=3 scheduling_decisions=5 expected_scheduling_decisions=3 cr3_performed=4 expected_cr3_performed=2 cr3_skipped=1 expected_cr3_skipped=1 alloc_calls=1 expected_alloc_calls=1 dealloc_calls=1
```

`xtask` reports:

```text
xtask: `phase2_perf_proxy_baseline` did not exit via debug success; status=exit status: 35
serial=/workspace/target/neo-test/phase2_perf_proxy_baseline.attempt1.serial.log
```

## What I Tried

- Fixed a BUILD-2-induced exhaustiveness compile break in the Q8C harness caused by adding `Handle::Socket`.
- Verified the previously failing adjacent scenarios directly under `NEO_SMP=4`:
  - `q8c_blocked_kill_foreign_parked` PASS
  - `q8c_close_peer_my_side_extract` PASS
- Re-ran the broad SMP=4 sweep; it now stops earlier at `phase2_perf_proxy_baseline`.

## Hypothesis

This is not in the BUILD-2 network path. The scenario appears to assert fixed perf-counter values that drift under the SMP=4 scheduler path:

- observed `context_switches=5`, expected `3`
- observed `scheduling_decisions=5`, expected `3`
- observed `cr3_performed=4`, expected `2`

BUILD-2 did not modify scheduler perf proxy logic.

## Needed From PM / PL

Direction on whether `phase2_perf_proxy_baseline` should be:

- excluded from the BUILD-2 SMP=4 full-sweep close gate,
- updated as a separate Phase 2/SMP maintenance fix,
- or treated as a hard blocker for BUILD-2 close.
