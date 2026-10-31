# Jarvis failure artifact: kernel bundle sub-commit 3

**Dispatch:** `Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md`  
**Sub-commit:** 3 - ADR-019 sub-phase A, introduce `IrqSafeMutex`  
**Branch:** `jarvis/kernel-rename-adr-2026-05-10`  
**Date observed:** 2026-05-11 EDT

## Failure summary

Sub-commit 3 hit the explicit ADR-019 Q4 stop condition: the `IrqSafeMutex` micro-bench exceeded the 50ns acquire/release overhead threshold versus `spin::Mutex`.

No sub-commit 3 commit was created. The working tree is intentionally left with the partial ADR-019 implementation and this failure artifact for PM triage, per dispatch failure handling.

## What was attempted

Implemented a first-pass ADR-019 sub-phase A:

- Added `kernel/src/sync/irqsafe_mutex.rs`.
- Added `IrqSafeMutex<T>` as a sibling type to `SwitchMutex`.
- Disabled local interrupts before acquiring the inner `spin::Mutex`.
- Restored the prior IF state on guard drop.
- Used `ManuallyDrop` so the inner guard is dropped before IF is restored.
- Integrated ADR-020 `LockOwner`.
- Added interrupt-flag read helpers under `kernel/src/arch`.
- Added two kernel scenarios:
  - `irqsafe_mutex_if_restore`
  - `irqsafe_mutex_bench`
- Added xtask scenario entries and kernel feature flags.
- Changed SwitchMutex/IrqSafeMutex owner acquisition sites to use constant `lock_site!()` values to remove per-acquire runtime file-path hashing.
- Added forced inlining on the new lock/owner hot path to check whether debug-build call overhead was the dominant bench factor.

## Expected state

Per dispatch §3.4 and §3.5:

- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- `cargo neo-test all` PASS.
- Micro-bench reports `IrqSafeMutex` overhead versus `spin::Mutex` at or below 50ns per acquire/release cycle.
- If overhead exceeds 50ns/cycle: STOP, do not push, file results, and surface to PM.

## Actual state

Build gates passed before the stop:

```text
cargo k
PASS, 18 kernel warnings

cargo k --no-default-features
PASS, 18 kernel warnings
```

Functional IF restore scenario passed:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test irqsafe_mutex_if_restore
PASS
```

The benchmark scenario failed the threshold.

Initial bench, before constant owner-site optimization:

```text
NEO_TEST/1 event=BENCH name=irqsafe_mutex_bench iters=1000 cycles_per_tick=35851086 spin_mean_cycles=1663 spin_min_cycles=1262 spin_max_cycles=360386 irqsafe_mean_cycles=17354 irqsafe_min_cycles=14864 irqsafe_max_cycles=949774 switch_mean_cycles=8387 switch_min_cycles=7618 switch_max_cycles=541150
NEO_TEST/1 event=FAIL name=irqsafe_mutex_bench reason=overhead_threshold overhead_ns=4377 threshold_ns=50 spin_mean_ns=464 irqsafe_mean_ns=4841 switch_mean_ns=2339
```

After switching owner sites to constant `lock_site!()` values:

```text
NEO_TEST/1 event=BENCH name=irqsafe_mutex_bench iters=1000 cycles_per_tick=35671746 spin_mean_cycles=1733 spin_min_cycles=1264 spin_max_cycles=415666 irqsafe_mean_cycles=12932 irqsafe_min_cycles=11082 irqsafe_max_cycles=851884 switch_mean_cycles=5477 switch_min_cycles=4632 switch_max_cycles=585900
NEO_TEST/1 event=FAIL name=irqsafe_mutex_bench reason=overhead_threshold overhead_ns=3139 threshold_ns=50 spin_mean_ns=486 irqsafe_mean_ns=3625 switch_mean_ns=1535
```

After adding forced inlining on the new lock/owner hot path:

```text
NEO_TEST/1 event=BENCH name=irqsafe_mutex_bench iters=1000 cycles_per_tick=36243986 spin_mean_cycles=1679 spin_min_cycles=1258 spin_max_cycles=379116 irqsafe_mean_cycles=12859 irqsafe_min_cycles=11208 irqsafe_max_cycles=894350 switch_mean_cycles=6657 switch_min_cycles=5224 switch_max_cycles=705764
NEO_TEST/1 event=FAIL name=irqsafe_mutex_bench reason=overhead_threshold overhead_ns=3085 threshold_ns=50 spin_mean_ns=463 irqsafe_mean_ns=3548 switch_mean_ns=1837
```

The QEMU/xtask failure surface for the latest run:

```text
xtask: `irqsafe_mutex_bench` did not exit via debug success; status=exit status: 35 serial=/workspace/target/neo-test/irqsafe_mutex_bench.serial.log
```

## Partial commits already landed

Sub-commit 1:

```text
33152a9 kernel: rename AltimitNeo → NeoPlutarch; A3 boot identity
```

Sub-commit 2:

```text
481217a kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)
```

## Triage notes

The first obvious avoidable cost was runtime acquisition-site hashing from `Location::caller().file()` in the ADR-020 path. Moving the hot path to a constant `lock_site!()` reduced measured `IrqSafeMutex` overhead from 4377ns to 3139ns, but the result remains far above the ADR threshold.

The remaining cost appears dominated by the IRQ save/restore path under the standard QEMU debug-build test configuration. PM should decide whether the 50ns threshold applies to debug QEMU numbers, whether the bench should run under an optimized kernel build, or whether sub-phase A needs a different API/measurement strategy before landing.

## Stop condition

Per dispatch §7.1:

```text
Micro-bench overhead >50ns/cycle threshold after sub-commit 3 -> STOP, do not push. File verification log with results, surface to PM.
```

Status: STOPPED at sub-commit 3. No push performed.
