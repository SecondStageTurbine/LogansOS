# WEDGE-INV-7 Findings: Cargo-Driven GDB-Stub Preflight

**Date:** 2026-05-08  
**Substrate:** `6edb012 xtask: optional QEMU GDB stub for investigation use`  
**Parent substrate:** `708223b docs: Plans/INDEX.md - sweep through WEDGE-INV-6 launch`  
**Scenario:** `smp_per_cpu_scheduler_smoke`  
**CPU count:** `NEO_SMP=4`  
**GDB stub:** `NEO_QEMU_GDB_PORT=1234`, no `-S`  
**Driver path:** standard `cargo neo-test smp_per_cpu_scheduler_smoke`

---

## Summary

WEDGE-INV-7 hit the dispatch's Case G stop condition.

The clean preflight produced `60/60 PASS`, `0/60 TIMEOUT_IN_KERNEL_SMOKE`, and no `POST_SMOKE_SMP_SCENARIO` reproductions. Per dispatch §4.1 and §9, the 200-run GDB-capture main loop was not run.

Verdict: **Case G - pre-flight stop condition.** With the QEMU GDB stub enabled under the original cargo-driven path, the historical cargo/no-stub wedge did not reproduce within the required 60-boot preflight window. This indicates the GDB stub itself is a masking factor under cargo as well as under direct QEMU. It is not proof the kernel race is fixed; it means this observer configuration is not suitable for mechanism capture.

---

## Methodology

One tracked change was made before measurement:

- `6edb012 xtask: optional QEMU GDB stub for investigation use`
- Adds `NEO_QEMU_GDB_PORT`; default is off.
- No kernel, scenario, userland, or additional xtask changes were made.

Backup was run after the commit, and both investigation stashes remained reachable:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

Clean measurement run:

- Fresh persistent dev container: `wedgeinv7`
- Artifact cleanup before measurement: `target/verify/WEDGE-INV-7-*`
- Preflight run 1-30: GDB stub enabled, no GDB attached
- Preflight run 31-60: extension required because 0/30 in-kernel timeouts
- Stop at 60 per dispatch; no main loop

Tooling note: an initial attempt in a previous container was discarded before this clean run because concurrent/partial rustup recovery produced `EMPTY` classifications with cargo logs showing toolchain component installation conflicts and zero-byte serial logs. Those were tooling failures, not kernel boots. The clean run began after recreating the container, clearing `target/verify/WEDGE-INV-7-*`, and confirming the toolchain was stable.

---

## M1 Preflight Results

Summary:

- Total boots: `60`
- PASS: `60`
- FAIL/PANIC: `0`
- `TIMEOUT_IN_KERNEL_SMOKE`: `0`
- `POST_SMOKE_SMP_SCENARIO`: `0`
- State dumps: `0`
- Elapsed min: `6.757s`
- Elapsed max: `55.062s`
- Elapsed mean: `8.965s`

Artifacts:

- Results JSONL: `target/verify/WEDGE-INV-7-results.jsonl`
- Runner logs:
- `target/verify/WEDGE-INV-7-preflight-1-30.runner.log`
- `target/verify/WEDGE-INV-7-preflight-31-60.runner.log`
- Per-run cargo logs: `target/verify/WEDGE-INV-7-run-N.cargo.log`
- Per-run serial logs: `target/verify/WEDGE-INV-7-run-N.serial.log`

Per-run classification:

| run | classification | signature | dump-file |
|---:|---|---|---|
| 1 | PASS |  |  |
| 2 | PASS |  |  |
| 3 | PASS |  |  |
| 4 | PASS |  |  |
| 5 | PASS |  |  |
| 6 | PASS |  |  |
| 7 | PASS |  |  |
| 8 | PASS |  |  |
| 9 | PASS |  |  |
| 10 | PASS |  |  |
| 11 | PASS |  |  |
| 12 | PASS |  |  |
| 13 | PASS |  |  |
| 14 | PASS |  |  |
| 15 | PASS |  |  |
| 16 | PASS |  |  |
| 17 | PASS |  |  |
| 18 | PASS |  |  |
| 19 | PASS |  |  |
| 20 | PASS |  |  |
| 21 | PASS |  |  |
| 22 | PASS |  |  |
| 23 | PASS |  |  |
| 24 | PASS |  |  |
| 25 | PASS |  |  |
| 26 | PASS |  |  |
| 27 | PASS |  |  |
| 28 | PASS |  |  |
| 29 | PASS |  |  |
| 30 | PASS |  |  |
| 31 | PASS |  |  |
| 32 | PASS |  |  |
| 33 | PASS |  |  |
| 34 | PASS |  |  |
| 35 | PASS |  |  |
| 36 | PASS |  |  |
| 37 | PASS |  |  |
| 38 | PASS |  |  |
| 39 | PASS |  |  |
| 40 | PASS |  |  |
| 41 | PASS |  |  |
| 42 | PASS |  |  |
| 43 | PASS |  |  |
| 44 | PASS |  |  |
| 45 | PASS |  |  |
| 46 | PASS |  |  |
| 47 | PASS |  |  |
| 48 | PASS |  |  |
| 49 | PASS |  |  |
| 50 | PASS |  |  |
| 51 | PASS |  |  |
| 52 | PASS |  |  |
| 53 | PASS |  |  |
| 54 | PASS |  |  |
| 55 | PASS |  |  |
| 56 | PASS |  |  |
| 57 | PASS |  |  |
| 58 | PASS |  |  |
| 59 | PASS |  |  |
| 60 | PASS |  |  |

---

## M2 Main Loop

Not run.

Reason: dispatch §4.1 and §9 explicitly stop after `0/60` in-kernel timeouts in preflight. Running the 200-boot main loop would violate the stop condition because there was no preflight evidence that cargo + GDB stub still reproduces the target wedge.

---

## Cross-Dump Synthesis

No state dumps were produced because no timeout occurred. There is no GDB corpus to synthesize.

This is itself the result: the observer configuration needed for capture appears to suppress the target failure mode before capture can begin.

---

## Case Verdict

**Case G - Pre-flight stop condition.**

Observed:

- Cargo, no stub: `4/150` `POST_SMOKE_SMP_SCENARIO` timeouts in WEDGE-INV-3+4
- Direct QEMU, stub: `0/60` timeouts in WEDGE-INV-5
- Direct QEMU, no stub: `1/200` timeout in WEDGE-INV-6, different `SMOKE_MID_PHASE` signature
- Cargo, stub: `0/60` timeouts in WEDGE-INV-7

Interpretation:

The cargo-vs-direct driver path remains relevant, but WEDGE-INV-7 isolates the GDB stub as an additional masking variable under the cargo path. The target `POST_SMOKE_SMP_SCENARIO` wedge is not observable with QEMU's GDB stub enabled in the 60-boot preflight window.

The result should not be used to discharge the wedge-fix commitment. It only rejects this capture strategy for the target signature.

---

## Recommendation

Pivot to a non-GDB observation strategy for the cargo-driven path.

The next dispatch should avoid QEMU's GDB stub and use an observer that does not materially alter the guest timing profile. Candidate directions:

- Kernel-side scenario watchdog that dumps per-CPU scheduler state to serial after a bounded smoke-test stall.
- Cargo-path external timeout using serial-only observation plus higher-frequency structured kernel progress frames.
- A minimal in-kernel ring buffer flushed only on timeout/panic, if serial volume is a concern.

The target remains the same: capture what every CPU is doing and whether runnable work is stranded during `POST_SMOKE_SMP_SCENARIO`.

---

## Cross-References

- `Plans/DISPATCH-2026-05-08-wedge-investigation-7-cargo-stub-cerebro.md`
- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-6-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md`
