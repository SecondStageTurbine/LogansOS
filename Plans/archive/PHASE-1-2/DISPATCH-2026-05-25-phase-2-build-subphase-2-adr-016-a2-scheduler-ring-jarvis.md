# DISPATCH: Phase 2 BUILD subphase 2 - ADR-016 A2 scheduler heap-decoupling

- From: PM
- To: Jarvis/Codex
- Date: 2026-05-25
- Lane: Kernel implementation + tests
- Status: READY FOR PL ROUTING
- Prior gate: `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-1-close.md`
- PLAN source: `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`)
- Hardware-last constraint: `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`)

---

## 1. Mission

Implement Phase 2 BUILD subphase 2: ADR-016 A2 scheduler heap-decoupling.

The milestone is narrow:

- Replace scheduler `ready` and `zombies` `VecDeque<Box<Thread>>` storage with fixed-capacity per-CPU rings of `Box<Thread>`.
- Preserve Box-pointer-stability for saved thread stack pointers.
- Make spawn fail gracefully with `-EAGAIN` at the per-CPU capacity bound.
- Add and verify the exact `INV-SYNC-5` scenario: `sched_ready_queue_capacity_pressure`.
- Keep all subphase 1 process-capability behavior working.

This dispatch is implementation work only. It does not reopen ADR-016, THINK v0.2, or the ratified four-subphase PLAN.

## 2. Authoritative Inputs

Read these before editing:

- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-1-close.md` - subphase 1 accepted; preserve its gates.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` - subphase 2 scope, gates, anti-scope.
- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md` - PL ratification of the four-subphase PLAN.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` section 2.2 - downstream BUILD clarifications.
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` - ADR-016 A2 architectural constraint.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md` - evidence checklist for subphase 2.

Likely source surfaces:

- `kernel/src/sched/scheduler.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/sched/thread.rs`
- `kernel/src/errno.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `userland/neo-init/src/main.rs`

Use live source as truth. The list above is a routing map, not a substitute for reading the implementation.

## 3. Required Implementation

### 3.1 Fixed-Capacity Scheduler Rings

Replace `PerCpuSchedulerInner.ready` and `PerCpuSchedulerInner.zombies` from `VecDeque<Box<Thread>>` to fixed-capacity ring storage.

Required properties:

- Stores owned `Box<Thread>` values, not `Thread` values.
- Moving entries inside the ring moves only the `Box` pointer; the `Thread` allocation and captured `rsp` location remain stable.
- Supports the operations currently used by scheduler paths:
  - push back
  - push front where wake-front behavior needs it
  - pop front
  - arbitrary removal by PID for `kill_thread_by_pid`
  - draining zombies for `reap`
  - debug length/count helpers
- Uses a default capacity of 64 slots per CPU for both ready and zombie rings unless the implementation proves a different split is required.
- Capacity is configured at scheduler construction/init time, not by adaptive heap growth under scheduler operation.

The implementation may choose a local `ThreadRing` type or an equivalent local structure, but it must be simple enough to audit and must not introduce unsafe code for intrusive lists.

### 3.2 Heap Independence Under `PerCpuScheduler.inner`

The core invariant is structural:

> no allocation or deallocation is reachable while holding `PerCpuScheduler.inner`.

In particular:

- remove the current `VecDeque` capacity assertions as the safety mechanism
- do not replace them with another growable collection
- rework `reap()` so it does not allocate a replacement queue while holding the scheduler lock
- ensure ring push/pop/drain operations do not allocate

The close report must include a short code-review note explaining why the scheduler-lock critical sections are heap-independent after the change.

### 3.3 Fallible Spawn At Capacity

Make ready-queue capacity pressure visible and graceful:

- `push_ready` or its replacement should be fallible where user-spawn can observe failure.
- `process::spawn_user` must propagate capacity pressure as `-EAGAIN`.
- `sys_spawn` behavior and libneo-facing behavior must remain coherent with the repo's existing error conventions.
- Existing wake/exit/internal scheduler paths must either handle ring-full explicitly or state the invariant that prevents overflow.

Do not panic for the `INV-SYNC-5` capacity-pressure path. The expected result is graceful `-EAGAIN`.

### 3.4 INV-SYNC-5 Scenario

Add or wire the exact scenario:

- `sched_ready_queue_capacity_pressure`

Expected property:

- Drive the per-CPU ready ring to its configured capacity.
- Assert spawn returns `-EAGAIN` once the capacity bound is reached.
- Verify no realloc/alloc-under-lock/panic behavior is required to reach the bound.
- Run at NEO_SMP=4.

If a helper worker process is needed, keep it minimal and local to the scenario.

### 3.5 Preserve Subphase 1

Subphase 1 is accepted and must stay green:

- `sys_wait_*` scenarios must still PASS.
- `sys_inspect_*` scenarios must still PASS.
- `cap_revoke_spawn_inheritance_descendants` must still PASS.
- full `cap_*` regression suite must still PASS.

Do not change the `sys_wait` ABI, `sys_inspect` ABI, or `sys_close` deferral except for mechanical updates required by scheduler API changes.

## 4. Required Tests

Add or wire:

- `sched_ready_queue_capacity_pressure`

Run and report:

- `docker compose run --rm neo cargo k`
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sched_ready_queue_capacity_pressure --no-meta`
- existing SMP regression scenarios at NEO_SMP=4
- subphase 1 wait/inspect scenarios at NEO_SMP=4
- full `cap_*` regression suite at NEO_SMP=4
- serial-log scan for `event=FAIL`, `KERNEL PANIC`, `BUG:`, and frame-leak markers
- `git diff --check`

If a command must be adjusted to match the repo's current test runner, record the exact command and rationale in the close report.

## 5. Acceptance Gates

Minimum gates before close report:

- `cargo k` PASS.
- `sched_ready_queue_capacity_pressure` (`INV-SYNC-5`) PASS at NEO_SMP=4.
- Existing SMP regression scenarios PASS at NEO_SMP=4.
- Subphase 1 wait/inspect + `INV-CAP-11` scenarios remain PASS.
- Full `cap_*` regression suite remains PASS at NEO_SMP=4.
- No allocation/deallocation reachable while holding `PerCpuScheduler.inner`, supported by code-review notes in the close report.
- No `KERNEL PANIC`, no `BUG:`, and no frame-leak marker in exercised serial logs.

## 6. Anti-Scope

Do not do any of the following in this subphase:

- start ADR-019 B/C lock migration
- migrate `Channel.state` or remove its `without_interrupts(...)` wrappers
- change `SwitchMutex` or `IrqSafeMutex` semantics
- activate process/channel/capability `sys_close`
- change the accepted `sys_wait`/`sys_inspect` ABI except for mechanical scheduler API adaptation
- add the tiered perf harness
- execute or prepare hardware evidence as a substitute for QEMU gates
- fold in NVMe FLUSH durability diagnosis
- introduce intrusive linked lists or new unsafe scheduler storage
- broaden into allocator, IPC, filesystem, or device-driver refactors not needed for A2

## 7. Close Report

File a close report under `Plans/` after implementation.

The close report must include:

- commit id(s)
- files changed
- ring design and capacity
- how Box-pointer-stability is preserved
- exact spawn-at-capacity error behavior
- how `reap()` avoids allocation under scheduler lock
- `INV-SYNC-5` scenario behavior and evidence
- verification commands and PASS/FAIL results
- subphase 1 anti-regression results
- known blockers or caveats
- ADR-016 A2 close-documentation status: either the docs amendment landed, or the close report includes ready-to-integrate amendment text for PM/Prometheus to file immediately after acceptance

## 8. Tell-Jarvis Text

Jarvis - Phase 2 BUILD subphase 1 is accepted. Please begin subphase 2 per `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md`. Scope is ADR-016 A2 only: replace scheduler ready/zombie `VecDeque<Box<Thread>>` with fixed-capacity per-CPU rings, make spawn fail with `-EAGAIN` at capacity, add `sched_ready_queue_capacity_pressure`, and preserve all subphase 1 wait/inspect/cap gates. Do not start ADR-019, perf harness, or hardware work. Close with a Plans report including ring design, allocation-under-lock review, verification evidence, and ADR-016 close-doc status.
