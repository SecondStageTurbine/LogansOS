# WEDGE-INV-4 findings: Limine timeout fix + extended baseline

Date: 2026-05-08

Dispatch: `Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md`

Substrate: `46984a1` plus `boot/limine.conf` changed from `timeout: 1` to `timeout: 0`

## 1. Summary

M1 verified the Limine `timeout: 0` change with 30/30 `kaslr_smoke` PASS and 0 H7 / `TIMEOUT_PRE_KERNEL` instances. M2 ran 100 uninstrumented `smp_per_cpu_scheduler_smoke` boots at `NEO_SMP=4`; 96 passed and 4 timed out in-kernel. Combined with WEDGE-INV-3's 50/50 PASS baseline, the current uninstrumented in-kernel timeout rate is 4/150 (2.67%).

Verdict: Case III by the dispatch threshold (`4+/150`). The wedge is not at the old 22-28% historical rate, but it is still meaningfully present and should not be discharged as absent. All four M2 timeouts share a scenario-phase signature that the literal dispatch classifier collapses into `SMOKE_COMPLETION`, but qualitative inspection identifies as a fourth signature: `POST_SMOKE_SMP_SCENARIO`.

## 2. Methodology

No kernel code, tool code, test scenario, or userland code was changed. The only tracked source/config change for this dispatch is:

```diff
-timeout: 1
+timeout: 0
```

Pre-flight confirmed `HEAD=46984a1`, no tracked modifications before the Limine edit, and the two prior instrumentation stashes preserved:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

Build sanity was run with `docker compose run --rm neo cargo k`. The command wrapper timed out while Docker continued the build, then `docker wait` returned exit code 0 for the build container.

M1 used `cargo neo-test kaslr_smoke` for 30 boots. M2 used `NEO_SMP=4 cargo neo-test smp_per_cpu_scheduler_smoke` for 100 boots. Both loops ran through a persistent Docker container to avoid per-run container setup noise. Before every run, the prior source serial log in `target/neo-test/` was removed so stale PASS output could not be copied after a timeout.

Canonical classifier source: `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1. The important H7 distinction is the negative check for `limine: Loading executable` before classifying a boot-menu log as `TIMEOUT_PRE_KERNEL`.

Signature classifier source: dispatch §4.2, with qualitative fit checked against `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`.

## 3. Measurement 1 - Limine fix verification

Scenario: `kaslr_smoke`

Logs: `target/verify/WEDGE-INV-4-limine-fix-run-*.serial.log`

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

Aggregate:

| Classification | Count | Rate |
|---|---:|---:|
| PASS | 30 | 100.0% |
| TIMEOUT_PRE_KERNEL / H7 | 0 | 0.0% |
| TIMEOUT_BOOT_KERNEL_PRE_SMOKE | 0 | 0.0% |
| TIMEOUT_IN_KERNEL_SMOKE | 0 | 0.0% |
| FAIL / PANIC / OTHER | 0 | 0.0% |

Result: M1 passes the Limine fix verification gate. The pre-fix H7 rate was about 0.36% across 2,504 historical logs; this 30-run verification observed 0 H7 instances with `timeout: 0`.

## 4. Measurement 2 - Extended baseline

Scenario: `smp_per_cpu_scheduler_smoke`

Environment: `NEO_SMP=4`

Logs: `target/verify/WEDGE-INV-4-extended-run-*.serial.log`

| Run | Classification | Signature |
|---:|---|---|
| 1 | PASS | |
| 2 | PASS | |
| 3 | PASS | |
| 4 | PASS | |
| 5 | PASS | |
| 6 | PASS | |
| 7 | PASS | |
| 8 | PASS | |
| 9 | PASS | |
| 10 | PASS | |
| 11 | PASS | |
| 12 | PASS | |
| 13 | PASS | |
| 14 | PASS | |
| 15 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO |
| 16 | PASS | |
| 17 | PASS | |
| 18 | PASS | |
| 19 | PASS | |
| 20 | PASS | |
| 21 | PASS | |
| 22 | PASS | |
| 23 | PASS | |
| 24 | PASS | |
| 25 | PASS | |
| 26 | PASS | |
| 27 | PASS | |
| 28 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO |
| 29 | PASS | |
| 30 | PASS | |
| 31 | PASS | |
| 32 | PASS | |
| 33 | PASS | |
| 34 | PASS | |
| 35 | PASS | |
| 36 | PASS | |
| 37 | PASS | |
| 38 | PASS | |
| 39 | PASS | |
| 40 | PASS | |
| 41 | PASS | |
| 42 | PASS | |
| 43 | PASS | |
| 44 | PASS | |
| 45 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO |
| 46 | PASS | |
| 47 | PASS | |
| 48 | PASS | |
| 49 | PASS | |
| 50 | PASS | |
| 51 | PASS | |
| 52 | PASS | |
| 53 | PASS | |
| 54 | PASS | |
| 55 | PASS | |
| 56 | PASS | |
| 57 | PASS | |
| 58 | PASS | |
| 59 | PASS | |
| 60 | PASS | |
| 61 | PASS | |
| 62 | PASS | |
| 63 | PASS | |
| 64 | PASS | |
| 65 | PASS | |
| 66 | PASS | |
| 67 | PASS | |
| 68 | PASS | |
| 69 | PASS | |
| 70 | PASS | |
| 71 | PASS | |
| 72 | PASS | |
| 73 | PASS | |
| 74 | PASS | |
| 75 | PASS | |
| 76 | PASS | |
| 77 | PASS | |
| 78 | PASS | |
| 79 | PASS | |
| 80 | PASS | |
| 81 | PASS | |
| 82 | PASS | |
| 83 | PASS | |
| 84 | PASS | |
| 85 | PASS | |
| 86 | PASS | |
| 87 | PASS | |
| 88 | PASS | |
| 89 | PASS | |
| 90 | TIMEOUT_IN_KERNEL_SMOKE | POST_SMOKE_SMP_SCENARIO |
| 91 | PASS | |
| 92 | PASS | |
| 93 | PASS | |
| 94 | PASS | |
| 95 | PASS | |
| 96 | PASS | |
| 97 | PASS | |
| 98 | PASS | |
| 99 | PASS | |
| 100 | PASS | |

Aggregate:

| Classification | Count | Rate |
|---|---:|---:|
| PASS | 96 | 96.0% |
| TIMEOUT_IN_KERNEL_SMOKE | 4 | 4.0% |
| TIMEOUT_PRE_KERNEL / H7 | 0 | 0.0% |
| TIMEOUT_BOOT_KERNEL_PRE_SMOKE | 0 | 0.0% |
| FAIL / PANIC / OTHER | 0 | 0.0% |

Signature breakdown:

| Signature | Count | Runs |
|---|---:|---|
| POST_SMOKE_SMP_SCENARIO | 4 | 15, 28, 45, 90 |
| SMOKE_MID_PHASE | 0 | |
| SMOKE_COMPLETION | 0 | |
| POST_SMOKE_USER_PROCESS | 0 | |
| New/other | 0 | |

### 4.1 Signature note

The literal dispatch §4.2 function returns `SMOKE_COMPLETION` for all four timeouts because:

- Initial ping/pong/hog smoke completed.
- No `neo-init` or user worker process spawned.

That mechanical label loses an important distinction for this scenario. In all four timeout logs, the in-kernel smoke test did not merely complete and then go silent. The scenario dispatch began:

```text
NEO_TEST/1 event=BEGIN name=smp_per_cpu_scheduler_smoke phase=dispatch
```

Then all four scenario workers spawned:

```text
[ kernel ] sched: spawned 'smp-hog-0' (id=7)
[ kernel ] sched: spawned 'smp-hog-1' (id=8)
[ kernel ] sched: spawned 'smp-hog-2' (id=9)
[ kernel ] sched: spawned 'smp-hog-3' (id=10)
```

The wedge occurred after only a subset of those scenario workers were reaped:

| Run | Scenario worker reaps before timeout | Terminal signal line |
|---:|---|---|
| 15 | `smp-hog-0` | `[ kernel ] sched: reaped 'smp-hog-0' (id=7)` |
| 28 | `smp-hog-0` | `[ kernel ] sched: reaped 'smp-hog-0' (id=7)` |
| 45 | `smp-hog-0`, `smp-hog-1` | `[ kernel ] sched: reaped 'smp-hog-1' (id=8)` |
| 90 | `smp-hog-0` | `[ kernel ] sched: reaped 'smp-hog-0' (id=7)` |

I therefore report these as `POST_SMOKE_SMP_SCENARIO`, a fourth signature specific to the SMP scheduler scenario. It is distinct from PATCH1's `SMOKE_COMPLETION` examples, where no scenario/user-process phase had started.

## 5. Combined 150-observation analysis

WEDGE-INV-3 baseline:

- 50 uninstrumented `smp_per_cpu_scheduler_smoke` boots on `46984a1`
- 50 PASS
- 0 `TIMEOUT_IN_KERNEL_SMOKE`

WEDGE-INV-4 M2:

- 100 uninstrumented `smp_per_cpu_scheduler_smoke` boots on functionally equivalent kernel substrate with Limine `timeout: 0`
- 96 PASS
- 4 `TIMEOUT_IN_KERNEL_SMOKE`

Combined:

| Classification | Count | Rate |
|---|---:|---:|
| PASS | 146 | 97.33% |
| TIMEOUT_IN_KERNEL_SMOKE | 4 | 2.67% |
| TIMEOUT_PRE_KERNEL / H7 | 0 | 0.0% |
| TIMEOUT_BOOT_KERNEL_PRE_SMOKE | 0 | 0.0% |
| FAIL / PANIC / OTHER | 0 | 0.0% |

The combined in-kernel timeout rate is `4/150` (`2.67%`). Wilson 95% confidence interval is approximately `1.04%` to `6.66%`. This is far below the old 22-28% historical rate, but it does not support Case I's "effectively absent" conclusion.

The 0/150 Case I upper-bound calculation would have been about 2.5%; the observed result crossed the dispatch's Case III threshold instead.

## 6. Verdict and recommendation

Verdict: Case III. With 4 in-kernel timeouts across 150 combined observations, the wedge is still meaningfully present and the investigation should continue.

Recommendation:

- Land the Limine `timeout: 0` commit. M1 verified 0 H7 instances in 30 simple boots, and M2 also observed 0 H7 instances in 100 SMP boots.
- Do not discharge the wedge-fix-before-SC-3B commitment as "effectively absent." The current substrate still produces a reproducible low-rate in-kernel failure.
- Target the next investigation at `POST_SMOKE_SMP_SCENARIO`: initial smoke passes, `smp_per_cpu_scheduler_smoke` scenario dispatch begins, all four `smp-hog-*` workers spawn, and the run wedges after one or two scenario-worker reaps.
- Treat the literal §4.2 `SMOKE_COMPLETION` classifier as too coarse for `smp_per_cpu_scheduler_smoke`; future signature logic should distinguish "silence before scenario dispatch" from "scenario dispatch started and partially reaped workers."

## 7. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md`
- `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md`
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`
- `target/verify/WEDGE-INV-4-limine-fix-run-*.serial.log`
- `target/verify/WEDGE-INV-4-extended-run-*.serial.log`
