# WEDGE-FIX-L Verification - 2026-05-08

## Verdict

Outcome: CLEAN DISCHARGE.

The WEDGE-FIX-L scheduler change landed as:

`4c6abf487cf94991912cf5a25c0e7fc21723f912 sched: prevent heap-lock re-entry in reap zombie drain`

Primary verification produced 200/200 PASS for `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`, with zero timeouts and zero historical wedge signatures. Supplemental historical-signature coverage produced 50/50 PASS for `cap_revoke_local` and 50/50 PASS for `cap_survive_exit`.

At the prior 2.67% historical wedge rate, observing 0 failures in 200 primary runs has probability 0.004460100810574388 if the bug is unchanged. The verification data supports marking the wedge-fix-before-SC-3B commitment discharged.

## Implementation

Changed file: `kernel/src/sched/scheduler.rs`

Mechanism:

- `Scheduler::reap()` now allocates a replacement zombie queue before taking the scheduler lock.
- The scheduler lock section uses `mem::replace(&mut s.zombies, replacement)` instead of `mem::take`, avoiding heap allocation while the scheduler lock is held.
- The drained queue is dropped inside `without_interrupts`, preventing destructor-time allocator re-entry with interrupts enabled.
- Debug assertions were added at scheduler-owned ready/zombie queue push sites to catch future zero-capacity queue regressions.

No ADR-016 architectural rewrite was included in this fix.

## Build And Backup

`docker compose run --rm neo cargo k` passed.

Warning count:

- Baseline from dispatch: 18 warnings.
- Observed WEDGE-FIX-L `cargo k`: 18 warnings.
- Diff: 0 new warnings.

Observed warning summary from `target/verify/WEDGE-FIX-L-cargo-k.log`:

```text
warning: `altimit-neo-kernel` (bin "kernel") generated 18 warnings (run `cargo fix --bin "kernel" -p altimit-neo-kernel` to apply 1 suggestion)
```

Single-run sanity passed:

```text
[xtask] neo-test: `smp_per_cpu_scheduler_smoke` PASS
```

Single-run log: `target/verify/WEDGE-FIX-L-single-smoke.log`

Backup captured after the WEDGE-FIX-L commit:

- Backup log: `target/verify/WEDGE-FIX-L-backup.log`
- Backup folder: `Z:\AltimitNeo\2026-05-08-232414`
- Source HEAD at backup time: `4c6abf487cf94991912cf5a25c0e7fc21723f912 (main)`
- Stashes to preserve: 3
- Verification: `OK (HEAD matches, commit count >= source, all stashes preserved)`

Backup log also observed `Working tree: 1 tracked mods, 94 untracked entries` at backup time. The tracked modification was later committed separately as `8ef8cac docs: bible.md - Article 8.5 critical-section discipline (v1.0.5)`. The WEDGE-FIX-L code commit remains directly below that doc commit.

## Primary Loop

Scenario: `smp_per_cpu_scheduler_smoke`

Environment: `NEO_SMP=4`

Artifacts:

- Results: `target/verify/WEDGE-FIX-L-results.jsonl`
- Runner log: `target/verify/WEDGE-FIX-L-runner.log`
- Serial logs: `target/verify/WEDGE-FIX-L-run-N.serial.log`

Classification:

- PASS: 200
- TIMEOUT_PRE_KERNEL: 0
- TIMEOUT_BOOT_KERNEL_PRE_SMOKE: 0
- TIMEOUT_IN_KERNEL_SMOKE: 0
- FAIL/PANIC/EMPTY/OTHER: 0

Timing:

- Rows: 200
- Min: 6.537s
- Max: 105.942s
- Mean: 10.207s
- Median: 8.940s

The first run includes build/startup cost from the fresh loop; later runs are the meaningful steady-state measurements.

Sample recorded rows:

```json
{"classification": "PASS", "elapsed_s": 105.942, "exit_code": 0, "run": 1, "serial_log": "target/verify/WEDGE-FIX-L-run-1.serial.log", "signature": null}
{"classification": "PASS", "elapsed_s": 8.068, "exit_code": 0, "run": 200, "serial_log": "target/verify/WEDGE-FIX-L-run-200.serial.log", "signature": null}
```

## Supplemental Historical Signatures

Supplemental results file: `target/verify/WEDGE-FIX-L-supplemental-results.jsonl`

Supplemental runner log: `target/verify/WEDGE-FIX-L-supplemental-runner.log`

SMOKE_MID_PHASE historical scenario:

- Scenario selected: `cap_revoke_local`
- Reason: historical PATCH1 run 15 classified as SMOKE_MID_PHASE on this scenario.
- Result: 50/50 PASS.
- Serial logs: `target/verify/WEDGE-FIX-L-cap_revoke_local-run-N.serial.log`

POST_SMOKE_USER_PROCESS historical scenario:

- Scenario selected: `cap_survive_exit`
- Reason: historical PATCH1 runs 11, 17, and 42 classified as POST_SMOKE_USER_PROCESS on this scenario.
- Result: 50/50 PASS.
- Serial logs: `target/verify/WEDGE-FIX-L-cap_survive_exit-run-N.serial.log`

Supplemental aggregate timing:

- Rows: 100
- Min: 6.533s
- Max: 73.413s
- Mean: 8.684s
- Median: 7.089s

Sample recorded rows:

```json
{"classification": "PASS", "elapsed_s": 73.413, "exit_code": 0, "label": "SMOKE_MID_PHASE historical scenario", "run": 1, "scenario": "cap_revoke_local", "serial_log": "target/verify/WEDGE-FIX-L-cap_revoke_local-run-1.serial.log", "signature": null}
{"classification": "PASS", "elapsed_s": 11.287, "exit_code": 0, "label": "POST_SMOKE_USER_PROCESS historical scenario", "run": 50, "scenario": "cap_survive_exit", "serial_log": "target/verify/WEDGE-FIX-L-cap_survive_exit-run-50.serial.log", "signature": null}
```

## Recommendation

Mark the WEDGE-FIX-L commitment discharged and allow SC-3B planning to proceed.

ADR-016 remains valid as an architectural follow-up for IRQ-safe heap / scheduler decoupling, but the localized Family L fix removed the observed wedge signature under the required 200-run primary loop and both supplemental historical scenario loops.
