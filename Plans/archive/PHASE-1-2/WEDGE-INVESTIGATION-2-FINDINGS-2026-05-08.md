# WEDGE investigation 2 findings — smoke/scheduler lifecycle instrumentation

**Date:** 2026-05-08  
**Substrate:** `46984a1` (`docs: Plans/INDEX.md — post-ADR-015 sweep + open commitments section`)  
**Experiment type:** Throwaway instrumentation, no commit on main  
**Scenario:** `smp_per_cpu_scheduler_smoke`, `NEO_SMP=4`, 30 counted runs  
**Raw logs:** `target/verify/WEDGE-INV-2-run-*.serial.log`

---

## 1. Summary

The target in-kernel smoke-test wedge did **not** reproduce in this 30-run experiment. Results were 29 PASS, 1 TIMEOUT, 0 PANIC, and 0 OTHER. The single TIMEOUT was not a scheduler/smoke-test wedge: its serial log is only 318 bytes and stops at the Limine boot menu countdown, before `limine: Loading executable`, `_kernel_start`, `[ SMOKE ]`, `[ THR ]`, or `[ SCHED ]`.

The observed timeout therefore establishes a new signature, **H7: pre-kernel boot/menu stall**, but does not explain the H1 run-4 in-kernel wedge. In all 29 kernel-entering runs, the smoke test reached `phase=smoke_test_call done`, all `ping`/`pong`/`hog` threads reached `Zombie->Freed`, the scenario began, all four `smp-hog-*` workers reached `Zombie->Freed`, and the terminal PASS frame was emitted.

## 2. Methodology

Instrumentation was added as throwaway code in four files:

- `kernel/src/sched/mod.rs`: `[ SMOKE ]` phase logs around `smoke_test()` reset, spawn, `wait_pingpong`, `trigger_hog`, `wait_hog`, and final reap.
- `kernel/src/sched/thread.rs`: `[ THR ]` creation and `Zombie->Freed` lifecycle logs.
- `kernel/src/sched/scheduler.rs`: `[ THR ]` significant state transitions and `[ SCHED ]` queue/scheduling samples.
- `kernel/src/main.rs`: coarse `[ SMOKE ] phase=smoke_test_call start/done` bracket around `sched::smoke_test()`.

One dispatch assumption was stale: the smoke-test orchestration is not in `main.rs` on this HEAD. `main.rs` only calls `sched::smoke_test()`. The actual state machine lives in `kernel/src/sched/mod.rs`, so that file was instrumented narrowly to satisfy the required `[ SMOKE ]` observations.

The instrumentation deliberately did not modify `ping`, `pong`, or `hog` bodies, did not re-instrument `reap()`, did not change scheduling logic, and did not add assertions. High-volume Ready/Running transitions and per-idle scheduler decisions were throttled after early sanity runs showed that logging every rotation turns the experiment into a serial benchmark and can itself cause timeouts.

Build/sanity notes:

- `cargo k` passed with the existing 18-warning baseline.
- Initial high-volume instrumentation produced sanity timeouts and was reduced before the 30-run loop.
- Final sanity run with reduced instrumentation passed and showed `[ SMOKE ]`, `[ THR ]`, and `[ SCHED ]` streams.

## 3. Quantitative Results

| Run | Classification | Last `[ SMOKE ]` phase | Threads non-freed at timeout | Last `[ SCHED ]` state |
|---:|---|---|---|---|
| 1 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 2 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 3 | TIMEOUT | none | n/a — kernel never entered | none |
| 4 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 5 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 6 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 7 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 8 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 9 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 10 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 11 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 12 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 13 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 14 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 15 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 16 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 17 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 18 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 19 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 20 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 21 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 22 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 23 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 24 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 25 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 26 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 27 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 28 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 29 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |
| 30 | PASS | `phase=smoke_test_call done` | none | scenario workers drained; terminal PASS |

Aggregate:

```text
WEDGE-INV-2 result (30 counted runs, NEO_SMP=4, scenario=smp_per_cpu_scheduler_smoke):
  PANIC:   0
  PASS:    29
  TIMEOUT: 1
  OTHER:   0

Per-run results array (1..30):
PASS, PASS, TIMEOUT, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS
```

Thread lifecycle aggregate for the 29 PASS runs:

```text
passLogs=29 missingSmokeDone=0 missingPass=0 missingThreadFree=0
```

Run 3 timeout serial contents:

```text
ALTIMIT NEO boot menu rendered
Booting automatically in 1, press any key to stop the countdown...
```

No kernel lines appear in run 3.

## 4. Qualitative Observations

The passing smoke-test flow is stable. A representative PASS run shows:

```text
[ SMOKE ] phase=spawn_ping done
[ SMOKE ] phase=spawn_pong done
[ SMOKE ] phase=spawn_hog done
[ THR ] id=4 name=ping pid=0 Running->Zombie reason=rotate_out
[ THR ] id=4 name=ping pid=0 Zombie->Freed reason=reap_free
[ THR ] id=5 name=pong pid=0 Running->Zombie reason=rotate_out
[ SMOKE ] phase=wait_pingpong done iters=2 ping_done=true pong_done=true hog_done=false trigger=false
[ SMOKE ] phase=trigger_hog done hog_done=false trigger=true
[ THR ] id=6 name=hog pid=0 Running->Zombie reason=rotate_out
[ THR ] id=6 name=hog pid=0 Zombie->Freed reason=reap_free
[ SMOKE ] phase=smoke_test_call done
```

Then the `smp_per_cpu_scheduler_smoke` scenario begins and all four scenario workers are created, run on `cpu=0`, become zombies, are freed, and produce:

```text
NEO_TEST/1 event=PASS name=smp_per_cpu_scheduler_smoke workers_done=4 scheduler_cpu=0
```

This confirms the H1 experiment's earlier observation: despite its name, the scenario's workers are BSP-local on this substrate. `sched::spawn()` enqueues into `current_scheduler()`, and the scenario runs on the BSP.

Run 3 is qualitatively different from all prior wedge signatures. It never reaches Limine's module-loading lines, so it provides no evidence about `ping`, `pong`, `hog`, thread lifecycle, reaping, scheduler choices, or AP behavior. It should be classified separately from the in-kernel smoke-test wedge.

## 5. Verdict On H2-H6

**H2: pong/hog are stuck in an unreachable state.**  
Not supported by this experiment. No in-kernel timeout was captured. In all 29 kernel-entering runs, both `pong` and `hog` reached `Running->Zombie` and `Zombie->Freed`.

**H3: pong/hog are running but their output is suppressed.**  
Not supported. The only timeout produced no kernel output at all, not selective suppression after `ping`/`pong`/`hog` had started. In PASS runs, serial output remains coherent through smoke-test completion.

**H4: pong/hog completed but `on_thread_reap` failed silently.**  
Not supported. In all kernel-entering runs, `pong` and `hog` were visibly freed. The one timeout never reached kernel thread creation.

**H5: a kernel thread that should have been spawned was not spawned.**  
Not supported. In all kernel-entering runs, `ping`, `pong`, and `hog` were created and enqueued. The timeout did not reach the spawn phase.

**H6: cpu=0 scheduler idle/yield path wedged.**  
Not supported by captured kernel data. PASS runs show normal scheduling to smoke threads, then a final `chose=idle` only after all relevant workers had become zombies/freed. The timeout contains no scheduler data.

**H7: pre-kernel boot/menu stall.**  
Confirmed as a separate timeout class. Run 3 timed out while the serial log still contained only the Limine boot menu countdown. This is outside the scheduler and outside H2-H6. It does not explain the earlier H1 run-4 in-kernel smoke-test wedge.

Overall: **the target in-kernel wedge remains unconfirmed in this experiment.** H2-H6 are not evidenced by these 30 boots; H7 explains the single observed timeout.

## 6. Implications For The Next Investigation

Do not write a scheduler fix from this data. The experiment did not capture the target in-kernel failure mode.

Recommended next step: split future timeout classification into at least three buckets before drawing scheduler conclusions:

- `PRE_KERNEL`: serial never reaches `limine: Loading executable` or `_kernel_start`.
- `BOOT_KERNEL_PRE_SMOKE`: kernel starts but never reaches `[ SMOKE ] phase=smoke_test_call start`.
- `IN_KERNEL_SMOKE`: kernel reaches smoke-test instrumentation and later times out.

If the project still wants the H2-H6 answer, rerun with this same low-volume instrumentation for a higher N, or bias toward reproducing the prior H1 run-4 signature specifically. A 100-run loop with early classification may be more useful than repeated 30-run loops, because the only relevant samples are `IN_KERNEL_SMOKE` timeouts.

If pre-kernel stalls are recurring, they should be investigated as a harness/QEMU/Limine boot issue separately from the scheduler wedge.

## 7. Cross-References

- `Plans/DISPATCH-2026-05-08-wedge-investigation-2-cerebro.md` — this dispatch.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 narrow-form refutation and prior run-4 in-kernel wedge.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — wedge/Q9 convergence context.
- `Plans/PM-DECISION-2026-05-08-wedge-investigation-continues.md` — continued-investigation decision.
- `target/verify/WEDGE-INV-2-run-*.serial.log` — raw logs.
- `46984a1` — substrate commit; instrumentation was not committed.

---

**End of findings.**
