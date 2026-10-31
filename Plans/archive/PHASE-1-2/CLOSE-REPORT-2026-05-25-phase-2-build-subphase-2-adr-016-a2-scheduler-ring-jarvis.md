# CLOSE-REPORT: Phase 2 BUILD subphase 2 - ADR-016 A2 scheduler ring

- Author: Jarvis/Codex
- Date: 2026-05-25
- Status: COMPLETE - ready for PM acceptance
- Dispatch: `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md`
- Base commit before implementation: `14b3697b3a7c11ddc4b6a3d6893d52c2274f56ee`
- Implementation commit: `5aa5baaff6029769323a0ced3e4e088ccbbf9d43`
- Close/docs commit: the commit containing this close report and the ADR-016 amendment

## Files changed

Implementation commit:

- `kernel/Cargo.toml`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `userland/neo-init/Cargo.toml`
- `userland/neo-init/src/main.rs`
- `userland/neo-worker/Cargo.toml`
- `userland/neo-worker/src/main.rs`

Close/docs commit:

- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`
- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md`

## Ring design and capacity

`PerCpuSchedulerInner.ready` and `.zombies` now use a local `ThreadRing<const N: usize>` instead of `VecDeque<Box<Thread>>`.

- Capacity: `SCHEDULER_RING_CAPACITY = 64`.
- Scope: both ready and zombie rings are 64 slots per CPU.
- Storage: `[Option<Box<Thread>>; N]` with head and length fields.
- Operations: push-back, push-front, pop-front, PID-position lookup, arbitrary PID removal, and zombie draining.
- Unsafe code: none added for scheduler storage.

The capacity is fixed by the scheduler ring type at construction. There is no adaptive heap growth under scheduler operation.

## Box-pointer stability

The ring stores `Box<Thread>` values, not inline `Thread` values. Moving an entry inside the ring moves only Box ownership. The heap allocation that contains the `Thread` and its `rsp` field remains stable, preserving the saved-stack-pointer invariant used by `context_switch`.

## Spawn-at-capacity behavior

User spawn is now fallible at the ready-ring bound:

- `scheduler::try_push_ready(thread)` returns `Err(Box<Thread>)` when the current CPU ready ring is full.
- `process::spawn_user` unregisters the unstarted process, frees the unqueued thread outside the scheduler lock, and returns `SpawnError::SchedulerFull`.
- `sys_spawn` maps `SpawnError::SchedulerFull` to `-EAGAIN`.

Internal scheduler paths (`wake_thread`, `wake_thread_front`, token wake, kill/zombify, yield/exit rotation) keep the prior invariant model: a full ring on those paths is a kernel invariant failure, not a user ABI result.

## Heap independence under scheduler lock

Code-review note:

- `VecDeque` is removed from `kernel/src/sched/scheduler.rs`; ready/zombie mutations are fixed-array `Option<Box<Thread>>` moves.
- Ring push/pop/remove do not call heap allocation or deallocation.
- `reap()` no longer swaps in a replacement queue. It pops one zombie under `PerCpuScheduler.inner`, drops the lock, then calls `Thread::free` and `process::on_thread_reap`.
- User-spawn capacity failure returns the `Box<Thread>` from `try_push_ready`; `Thread::free` runs after that scheduler lock has been released.

This makes the A2 property structural for scheduler ready/zombie storage.

## INV-SYNC-5 scenario

Scenario added and wired as:

- Kernel feature: `neo-test-sched-ready-queue-capacity-pressure`
- xtask name: `sched_ready_queue_capacity_pressure`
- Userland feature: `sched-ready-queue-capacity-pressure`

Behavior:

- `neo-init` repeatedly spawns minimal `worker` processes.
- Workers spin until killed, keeping the ready ring occupied.
- The scenario requires `sys_spawn("worker")` to return `-EAGAIN` once the ring reaches capacity pressure.
- The scenario requires at least 60 successful worker spawns before `-EAGAIN`, then kills all spawned workers and exits 0.

Evidence from the current successful serial log:

- `target/neo-test/sched_ready_queue_capacity_pressure.serial.log:612`
- `NEO_TEST/1 event=PASS name=sched_ready_queue_capacity_pressure sched_capacity_scenario=ok exit_code=0`

## Verification

All commands were run from `E:\AI Automation\AltimitNeo`.

- `docker compose run --rm neo cargo k` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sched_ready_queue_capacity_pressure --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_per_cpu_scheduler_smoke smp_same_cpu_ipc smp_block_release_smoke smp_send_blocks_first_then_completes smp_recv_blocks_first_then_completes smp_grant_cross_cpu smp_cross_cpu_migration_reap smp_cross_cpu_rendezvous_stress --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sys_wait_blocking sys_wait_wnohang sys_wait_already_exited sys_wait_multiple_waiters sys_inspect_running sys_inspect_exited sys_inspect_rights_gate cap_revoke_spawn_inheritance_descendants --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test cap_mint_and_use cap_derive_subset cap_derive_rights_violation cap_derive_missing_meta cap_grant_basic cap_grant_missing_meta cap_revoke_local cap_revoke_cross_process cap_revoke_descendants_walk cap_revoke_spawn_inheritance_descendants cap_survive_exit --no-meta` - PASS.
- `git diff --check` - PASS. Git emitted only existing line-ending normalization warnings.

Serial-log scan:

- Current successful logs for the exercised scenarios were scanned for `event=FAIL`, `KERNEL PANIC`, `BUG:`, `frame[-_ ]?leak`, `FRAME_LEAK`, `leak_audit=dirty`, and `frame_audit.*leak`.
- Result: no matches in the current successful scenario logs.
- Note: `target/neo-test` contains older retained failure logs from prior attempts and deliberate harness tests, so the acceptance scan was restricted to the current successful `.serial.log` files for this subphase's exercised scenarios.

## Subphase 1 anti-regression

Preserved and verified:

- `sys_wait_blocking` - PASS at `NEO_SMP=4`.
- `sys_wait_wnohang` - PASS at `NEO_SMP=4`.
- `sys_wait_already_exited` - PASS at `NEO_SMP=4`.
- `sys_wait_multiple_waiters` - PASS at `NEO_SMP=4`.
- `sys_inspect_running` - PASS at `NEO_SMP=4`.
- `sys_inspect_exited` - PASS at `NEO_SMP=4`.
- `sys_inspect_rights_gate` - PASS at `NEO_SMP=4`.
- `cap_revoke_spawn_inheritance_descendants` - PASS at `NEO_SMP=4`.
- Full `cap_*` regression list named above - PASS at `NEO_SMP=4`.

No `sys_wait`, `sys_inspect`, or `sys_close` ABI change was made for this subphase.

## Known blockers or caveats

- No hardware evidence was collected; hardware work is explicitly out of scope for this subphase and remains governed by the hardware-last barrier.
- No ADR-019 B/C lock migration, `SwitchMutex` semantic change, `Channel.state` migration, perf harness work, or process/channel/capability `sys_close` activation was started.
- The working tree had pre-existing untracked `ai_digest.py`; it was not touched or committed.

## ADR-016 close-documentation status

Landed in this close/docs commit:

- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`
- Section: `Amendment 2026-05-25 - A2 implementation landed`

That amendment records the implemented ring shape, capacity, Box-pointer-stability invariant, spawn `-EAGAIN` behavior, and verification summary for ADR-016 A2.
