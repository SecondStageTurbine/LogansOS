# H1 experiment findings — BSP reap race validation

**Date:** 2026-05-08  
**Substrate:** Kernel code from `47972c2` (`mm: ADR-015 KASLR-tolerant heap placement (Option E: validate + below-kernel fallback)`)  
**Experiment type:** Throwaway instrumentation, no commit on main  
**Scenario:** `smp_per_cpu_scheduler_smoke`, `NEO_SMP=4`, 20 counted runs  
**Raw logs:** `target/verify/H1-EXPERIMENT-run-*.serial.log`

---

## 1. Summary

H1 is **REFUTED for the measured scenario/substrate**. Across 20 runs, the loop produced 19 PASS, 1 TIMEOUT, 0 PANIC, and 0 OTHER. The instrumented reaper logged 88 active zombie observations; every one had `z_pre == z_post`, and there were **0 H1-signature divergences** (`z_pre=0,z_post>0`). The single timeout occurred even though the experiment removed the lock-free short-circuit and forced lock acquisition on every initialized scheduler.

The major surprise is that `smp_per_cpu_scheduler_smoke` does not currently route its worker threads to AP schedulers. It calls `crate::sched::spawn(...)` from the BSP, which enqueues workers on the current CPU's scheduler; all `[ H1 ]` lines were `cpu=0`.

## 2. Methodology

Instrumentation was added only to `kernel/src/sched/scheduler.rs::reap()`. The original `zombie_count == 0` early-out was removed for the experiment. For each initialized per-CPU scheduler, the instrumented path captured `z_pre = zombie_count.load(Acquire)` before locking, then always acquired the scheduler lock and captured `z_post = s.zombies.len()` before draining.

The instrumentation emitted:

```text
[ H1 ] seq=<N> cpu=<ID> z_pre=<N> z_post=<N> divergence=<YES|no>
```

The line was emitted when `z_pre` or `z_post` was nonzero, or when the two differed. Zero/zero iterations were intentionally omitted per the dispatch skeleton, so the denominator below is "active zombie observations", not every zero-work CPU scan.

Build and run notes:

- `cargo k` passed with 18 warnings.
- Single-run sanity passed and produced `[ H1 ]` lines.
- The preserved raw data is the 20 counted serial logs at `target/verify/H1-EXPERIMENT-run-*.serial.log`.
- During the experiment, `main` advanced from `47972c2` to `46984a1` via a doc-only `Plans/INDEX.md` commit. The diff from `47972c2` to `46984a1` is only `Plans/INDEX.md`; kernel code under test remained the `47972c2` substrate plus throwaway `scheduler.rs` instrumentation.

## 3. Quantitative Results

| Run | Classification | Active H1 observations | Non-zero observations | Divergences | H1 signatures | Max `z_post` | CPUs observed |
|---:|---|---:|---:|---:|---:|---:|---|
| 1 | PASS | 4 | 4 | 0 | 0 | 2 | 0 |
| 2 | PASS | 4 | 4 | 0 | 0 | 3 | 0 |
| 3 | PASS | 5 | 5 | 0 | 0 | 2 | 0 |
| 4 | TIMEOUT | 1 | 1 | 0 | 0 | 1 | 0 |
| 5 | PASS | 4 | 4 | 0 | 0 | 2 | 0 |
| 6 | PASS | 5 | 5 | 0 | 0 | 2 | 0 |
| 7 | PASS | 4 | 4 | 0 | 0 | 2 | 0 |
| 8 | PASS | 4 | 4 | 0 | 0 | 3 | 0 |
| 9 | PASS | 4 | 4 | 0 | 0 | 3 | 0 |
| 10 | PASS | 5 | 5 | 0 | 0 | 3 | 0 |
| 11 | PASS | 4 | 4 | 0 | 0 | 3 | 0 |
| 12 | PASS | 5 | 5 | 0 | 0 | 3 | 0 |
| 13 | PASS | 6 | 6 | 0 | 0 | 2 | 0 |
| 14 | PASS | 4 | 4 | 0 | 0 | 3 | 0 |
| 15 | PASS | 5 | 5 | 0 | 0 | 2 | 0 |
| 16 | PASS | 5 | 5 | 0 | 0 | 2 | 0 |
| 17 | PASS | 5 | 5 | 0 | 0 | 2 | 0 |
| 18 | PASS | 5 | 5 | 0 | 0 | 3 | 0 |
| 19 | PASS | 5 | 5 | 0 | 0 | 2 | 0 |
| 20 | PASS | 4 | 4 | 0 | 0 | 3 | 0 |
| **Total** | **19 PASS / 1 TIMEOUT** | **88** | **88** | **0** | **0** | **3** | **0 only** |

Observed `z_pre/z_post` distribution:

| Pattern | Count |
|---|---:|
| `z_pre=1 z_post=1` | 51 |
| `z_pre=2 z_post=2` | 28 |
| `z_pre=3 z_post=3` | 9 |

Classification summary:

```text
H1 experiment result (20 counted runs, NEO_SMP=4, scenario=smp_per_cpu_scheduler_smoke):
  PANIC:   0
  PASS:    19
  TIMEOUT: 1
  OTHER:   0

Per-run results array (1..20):
PASS, PASS, PASS, TIMEOUT, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS
```

## 4. Qualitative Observations

All active zombie observations were on `cpu=0`. AP schedulers were initialized in the serial logs, but none produced a nonzero `zombie_count` or post-lock zombie queue during the experiment.

The scenario body explains that result: `run_smp_per_cpu_scheduler_smoke()` calls `crate::sched::spawn("smp-hog-N", ...)`, and `sched::spawn()` enqueues on `current_scheduler()`. Because the scenario runs on the BSP, the `smp-hog-*` workers are BSP-local. The scenario name suggests AP scheduler coverage, but the current implementation does not route workers to APs.

The one TIMEOUT was run 4. Its last serial lines show the kernel smoke test reached the normal ping/pong/hog phase, logged one non-divergent active reap, and reaped only `ping` before the QEMU timeout:

```text
[ H1 ] seq=4 cpu=0 z_pre=1 z_post=1 divergence=no
[ pong ] 5
[ kernel ] sched: reaped 'ping' (id=4)
```

This is not the H1 signature. The forced-lock version of `reap()` drained exactly what the pre-lock counter reported, yet the boot still wedged.

There were no `z_pre>z_post` or `z_pre<z_post` cases. The lock-free counter and post-lock queue length agreed in every logged observation.

## 5. Verdict on H1

**REFUTED.**

No direct H1 race was observed. Specifically, there were 0 cases of `z_pre=0,z_post>0`, 0 total divergences, and the timeout still reproduced once while the experiment was already running the Alpha-prime-style forced-lock reaper.

Important caveat: this refutes H1 for the requested `smp_per_cpu_scheduler_smoke` run as currently implemented. It does not prove that an AP-zombie race is impossible in a future test that actually routes threads onto AP scheduler queues. The requested scenario did not exercise that path.

## 6. Implications for Alpha-prime

Alpha-prime remains a correctness tightening, but this experiment does **not** support landing it as the wedge fix. The expected signature did not appear, and the wedge was not eliminated.

The next investigation should not assume the short-circuit is the root cause. Better next targets:

- Inspect the BSP-local ping/pong/hog smoke path, because run 4 wedged there with all measured zombie activity on `cpu=0`.
- Reconcile the `smp_per_cpu_scheduler_smoke` test name with its implementation. If the goal is AP scheduler coverage, the test needs an explicit AP routing primitive or a different scenario.
- If AP-reaper H1 remains a concern, validate it with a scenario that actually creates AP-resident zombies; this experiment did not.

## 7. Cross-References

- `Plans/DISPATCH-2026-05-08-h1-validation-experiment-cerebro.md` — experiment dispatch.
- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — original H1 hypothesis.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — convergence analysis.
- `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` — §Q9 redo design context.
- `target/verify/H1-EXPERIMENT-run-*.serial.log` — raw serial logs.
- `47972c2` — kernel-code substrate commit; instrumentation was not committed.
- `46984a1` — current `main` after the doc-only `Plans/INDEX.md` sweep that landed during the experiment.

---

**End of findings.**
