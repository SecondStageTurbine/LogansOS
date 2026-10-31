# WEDGE-INV-3 baseline findings

Date: 2026-05-08

Dispatch: `Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md`

Substrate: `46984a1 docs: Plans/INDEX.md - post-ADR-015 sweep + open commitments section`

## 1. Summary

Ran 50 uninstrumented boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` on `46984a1`. All 50 runs passed. The observed IN_KERNEL_SMOKE timeout rate was `0/50` (`0.0%`), with no PRE_KERNEL, BOOT_KERNEL_PRE_SMOKE, PANIC, OTHER, or missing-log classifications.

Verdict: Case B. The historical in-kernel wedge did not reproduce on the current uninstrumented post-ADR-015 substrate. ADR-015 plausibly reduced or eliminated the trigger conditions, and the next dispatch should be a regression-guard scenario rather than another kernel patch.

## 2. Methodology

No kernel, tool, or userland code was changed. Existing instrumentation stashes were preserved and not applied:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

Pre-flight confirmed `HEAD=46984a1`, no tracked working-tree modifications, and `cargo k` reproduced with the expected 18-warning baseline. The loop used the committed `smp_per_cpu_scheduler_smoke` scenario with `NEO_SMP=4`. Serial logs were copied to `target/verify/WEDGE-INV-3-baseline-run-N.serial.log`.

Classifier:

- `PASS`: serial log contains `NEO_TEST/1 event=PASS`.
- `TIMEOUT_IN_KERNEL_SMOKE`: no PASS frame, but log reaches `[ kernel ] sched: spawned 'ping'`.
- `TIMEOUT_BOOT_KERNEL_PRE_SMOKE`: no PASS frame and no ping spawn, but log reaches `_kernel_start`, `[ kernel ] mm:`, or `[ kernel ] sched:`.
- `TIMEOUT_PRE_KERNEL`: no kernel marker, but log contains Limine/boot-menu output.
- `PANIC`: log contains `panicked at`, `KERNEL PANIC`, or `BUG:`.
- `TIMEOUT_UNCLASSIFIED`: none of the above markers match.

Run 1 and run 2 were launched manually through the same persistent container. Runs 3-50 were launched by a wrapper around the same command. Before each run, the previous `target/neo-test/smp_per_cpu_scheduler_smoke.serial.log` source log was removed so stale PASS output could not be copied forward after a timeout.

## 3. Quantitative results

| Run | Classification |
|---:|---|
| 1 | PASS |
| 2 | PASS |
| 3 | PASS |
| 4 | PASS |
| 5 | PASS |
| 6 | PASS |
| 7 | PASS |
| 8 | PASS |
| 9 | PASS |
| 10 | PASS |
| 11 | PASS |
| 12 | PASS |
| 13 | PASS |
| 14 | PASS |
| 15 | PASS |
| 16 | PASS |
| 17 | PASS |
| 18 | PASS |
| 19 | PASS |
| 20 | PASS |
| 21 | PASS |
| 22 | PASS |
| 23 | PASS |
| 24 | PASS |
| 25 | PASS |
| 26 | PASS |
| 27 | PASS |
| 28 | PASS |
| 29 | PASS |
| 30 | PASS |
| 31 | PASS |
| 32 | PASS |
| 33 | PASS |
| 34 | PASS |
| 35 | PASS |
| 36 | PASS |
| 37 | PASS |
| 38 | PASS |
| 39 | PASS |
| 40 | PASS |
| 41 | PASS |
| 42 | PASS |
| 43 | PASS |
| 44 | PASS |
| 45 | PASS |
| 46 | PASS |
| 47 | PASS |
| 48 | PASS |
| 49 | PASS |
| 50 | PASS |

Aggregate:

| Classification | Count | Rate |
|---|---:|---:|
| PASS | 50 | 100.0% |
| TIMEOUT_PRE_KERNEL | 0 | 0.0% |
| TIMEOUT_BOOT_KERNEL_PRE_SMOKE | 0 | 0.0% |
| TIMEOUT_IN_KERNEL_SMOKE | 0 | 0.0% |
| PANIC | 0 | 0.0% |
| OTHER / TIMEOUT_UNCLASSIFIED / MISSING | 0 | 0.0% |

Historical comparison:

| Loop | Sample | Timeout / wedge rate | Substrate notes |
|---|---:|---:|---|
| KASLR-Q1B | 50 | 14/50 (28.0%) | Uninstrumented sub-commit 2 substrate |
| ADR-015 PATCH1 | 50 | 11/50 (22.0%) | Uninstrumented post-ADR-015 loop from earlier investigation |
| H1 experiment | 20 | 1/20 (5.0%) | Reaper instrumentation; H1 refuted in narrow form |
| WEDGE-INV-2 | 30 | 1/30 (3.3%) | Smoke/thread/scheduler instrumentation; one PRE_KERNEL timeout |
| WEDGE-INV-3 baseline | 50 | 0/50 (0.0%) | Uninstrumented `46984a1`, this run |

Observed IN_KERNEL_SMOKE rate: `0/50` (`0.0%`). With 50 clean observations, the rough 95% upper bound for the underlying rate is about 6%, so this strongly rejects the old 22-28% historical rate on this substrate while not proving the true rate is exactly zero.

## 4. Qualitative observations

There were no TIMEOUT_IN_KERNEL_SMOKE runs. No partial-reap signature was observed, no scheduler-decision stall signature was observed, and there was no recurrence of the H1 run 4 pattern where the smoke phase had started after ping was reaped.

There were no TIMEOUT_PRE_KERNEL runs. The pre-kernel/Limine bucket seen once in WEDGE-INV-2 did not recur in this 50-run baseline.

There were no TIMEOUT_BOOT_KERNEL_PRE_SMOKE runs. This previously-unseen bucket remains unobserved.

There were no panic, BUG, OTHER, TIMEOUT_UNCLASSIFIED, or missing-log classifications.

All PASS logs reached the terminal frame:

```text
NEO_TEST/1 event=PASS name=smp_per_cpu_scheduler_smoke workers_done=4 scheduler_cpu=0
```

## 5. Verdict and recommendation

Verdict: Case B. The IN_KERNEL_SMOKE wedge rate on uninstrumented `46984a1` is `0/50` (`0.0%`), below the dispatch's low-rate threshold and far below the 22-28% historical baseline.

Recommendation: treat the wedge-fix-before-SC-3B commitment as likely discharged by the current post-ADR-015 substrate plus a regression guard. The next dispatch should add a regression-guard test or loop gate for `smp_per_cpu_scheduler_smoke` rather than changing kernel scheduling code based on a wedge that no longer reproduces on the current uninstrumented baseline.

Do not overclaim mechanism. This experiment shows the current substrate no longer reproduces the in-kernel wedge under the measured conditions; it does not prove which ADR-015 layout change removed the trigger, nor does it prove the underlying race is impossible.

## 6. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md`
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md`
- `Plans/PM-DECISION-2026-05-07-adr-015-exclusive-end-check.md`
- `Plans/CLOSE-REPORT-2026-05-07-adr-015-implementation.md`
- `target/verify/WEDGE-INV-3-baseline-run-*.serial.log`
