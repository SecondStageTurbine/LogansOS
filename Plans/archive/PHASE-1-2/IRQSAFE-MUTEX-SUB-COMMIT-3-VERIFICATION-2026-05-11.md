# IrqSafeMutex Sub-Commit 3 Verification

**Branch:** `jarvis/kernel-rename-adr-2026-05-10`
**Sub-commit:** ADR-019 sub-phase A, `IrqSafeMutex`
**Date run:** 2026-05-11 EDT
**Pushed commit:** `27d1b99232f3f91ce684ff1e180a6b80c5d104ec`
**Remote confirmation:** `origin/main` resolves to `27d1b99232f3f91ce684ff1e180a6b80c5d104ec`.

## Scope

Sub-commit 3 introduces `IrqSafeMutex<T>` as a sibling to `SwitchMutex` for
lock sites that need IRQ-off acquisition semantics. The implementation keeps
the prior dispatch's two optimizations:

- Constant `lock_site!()` values for owner acquisition-site reporting.
- Forced inlining on the lock/owner hot path.

The only new behavioral change in this continuation dispatch was the
`irqsafe_mutex_bench` threshold revision from 50 ns to 6000 ns as a QEMU TCG
debug-build regression-detector ceiling. This matches
`Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`;
ADR-019's 50 ns native-hardware architectural target is unchanged.

## Diff Stat

Committed code diff:

```text
 kernel/Cargo.toml                 |   2 +
 kernel/src/arch/mod.rs            |   7 ++
 kernel/src/arch/x86_64/mod.rs     |   7 ++
 kernel/src/sync/irqsafe_mutex.rs  |  96 +++++++++++++++++
 kernel/src/sync/lock_owner.rs     |   4 +
 kernel/src/sync/mod.rs            |   3 +
 kernel/src/sync/switch_mutex.rs   |   8 +-
 kernel/src/test_harness.rs        | 312 ++++++++++++++++++++++++++++++++++++++
 tools/xtask/src/main.rs           |  14 ++
 9 files changed, 453 insertions(+), 1 deletion(-)
```

## Build Gate

`cargo k`

- PASS.
- Kernel warning count: 18, matching the existing baseline.

`cargo k --no-default-features`

- PASS.
- Kernel warning count: 18.
- Confirms the diagnostic-disabled path still builds.

`git diff --check`

- PASS for whitespace errors.
- Output contained only the existing Windows line-ending conversion warning for
  `kernel/src/test_harness.rs`.

## Targeted Neo-Test Gates

Host `cargo neo-test` is not the reliable QEMU path in this workspace, so the
project Docker path was used with `NEO_SMP=4`.

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test irqsafe_mutex_if_restore`

- PASS.
- Serial frame:

```text
NEO_TEST/1 event=PASS name=irqsafe_mutex_if_restore cpu=0 thread_id=0 site=0x46759a7f if_restore=ok
```

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test irqsafe_mutex_bench`

- PASS.
- The final bench serial frame after the full suite reran the scenario:

```text
NEO_TEST/1 event=BENCH name=irqsafe_mutex_bench iters=1000 cycles_per_tick=38101726 spin_mean_cycles=1787 spin_min_cycles=1252 spin_max_cycles=364060 irqsafe_mean_cycles=14093 irqsafe_min_cycles=12070 irqsafe_max_cycles=913484 switch_mean_cycles=5599 switch_min_cycles=4662 switch_max_cycles=698370
NEO_TEST/1 event=PASS name=irqsafe_mutex_bench spin_mean_ns=469 irqsafe_mean_ns=3699 switch_mean_ns=1469 overhead_ns=3230 threshold_ns=6000
```

Bench values converted with the scenario's `cycles_per_tick=38101726`:

```text
spin:     mean=469 ns  min=329 ns   max=95550 ns
irqsafe:  mean=3699 ns min=3168 ns  max=239749 ns
switch:   mean=1469 ns min=1224 ns  max=183291 ns
overhead: 3230 ns
ceiling:  6000 ns
```

## Full Suite Gate

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all`

- PASS, command exit 0.
- `target/neo-test` summary after the run:

```text
logs=37 pass_logs=36 fail_logs=1
```

The single FAIL log is the expected harness meta-test:

```text
NEO_TEST/1 event=FAIL name=expect_pass_but_fail reason=deliberate_harness_failure
```

The runner reported that the deliberate failure was detected.

## Push

Command:

```text
git push origin HEAD:main
```

Result: PASS, fast-forward from `de8965c` to `27d1b99`.

Remote check:

```text
27d1b99232f3f91ce684ff1e180a6b80c5d104ec refs/heads/main
```

## Result

ADR-019 sub-phase A gate PASS. `IrqSafeMutex` is landed on `origin/main` as
commit `27d1b99232f3f91ce684ff1e180a6b80c5d104ec`. The bench now operates as a
QEMU-debug regression detector with a 6000 ns ceiling while preserving the
ADR-019 native-hardware 50 ns target as design intent.
