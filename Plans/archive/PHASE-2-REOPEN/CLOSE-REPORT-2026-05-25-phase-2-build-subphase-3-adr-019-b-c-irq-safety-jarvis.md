# CLOSE-REPORT: Phase 2 BUILD subphase 3 - ADR-019 B/C IRQ safety

- Author: Jarvis/Codex
- Date: 2026-05-25
- Status: COMPLETE - ready for PM acceptance
- Dispatch: `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md`
- Base commit before implementation: `8359eb234049fae0254dccffebcf930eafe96627`
- Implementation commit: `e7094a860d0b928a5bb08046009357fd8eb2f6d7`
- Close/docs commit: the commit containing this close report

## Files changed

Implementation commit:

- `docs/ADR-019-switchmutex-irq-off-by-construction.md`
- `kernel/Cargo.toml`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/drivers/block/ahci.rs`
- `kernel/src/drivers/block/mod.rs`
- `kernel/src/drivers/block/nvme.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/cache.rs`
- `kernel/src/fs/neofs/inode.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/sched/kthread.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/sync/irqsafe_mutex.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `userland/libneo/src/lib.rs`
- `userland/neo-init/Cargo.toml`
- `userland/neo-init/src/main.rs`
- `userland/neo-worker/Cargo.toml`
- `userland/neo-worker/src/main.rs`

Close/docs commit:

- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md`

## Implementation summary

ADR-019 subphases B/C are complete for the production sites in scope.

- `Channel.state` is now `IrqSafeMutex<[EndpointState; 2]>`.
- `IrqSafeMutex` now delegates to `SwitchMutex<T>` internally and exposes `release_token_for_context_switch()`, preserving the IPC block path's release-token handoff while making IF-off acquisition structural.
- Scheduler block helpers now accept guards implementing the new local `ContextSwitchRelease` trait, so `SwitchMutexGuard` and `IrqSafeGuard` use the same release-on-switch path.
- Mixed process/interrupt device-list locks that fit the type moved to `IrqSafeMutex`: `AHCI_DEVICES` and `VIRTIO_DEVICES`.
- Retained `SwitchMutex` declarations now carry declaration-site comments naming either process-context-only reachability or scheduler/context-switch release-token requirements.

## Production SwitchMutex audit

Test-harness fixtures are excluded from this production audit per dispatch.

| Site | Classification | Action | Rationale |
| --- | --- | --- | --- |
| `ipc::Channel.state` | mixed IF-on syscall / IF-off teardown and test-helper paths; context-switch handoff | migrated to `IrqSafeMutex` | Type now enforces IF-off acquisition; inner `SwitchMutex` release token preserves parked-thread publication. |
| `drivers::block::ahci::AHCI_DEVICES` | mixed process/interrupt | migrated to `IrqSafeMutex` | PCI probe mutates from process context; MSI handler reads with IF=0. |
| `drivers::block::virtio_blk::VIRTIO_DEVICES` | mixed process/interrupt | migrated to `IrqSafeMutex` | PCI probe mutates from process context; completion worker/IRQ hook poll from IF=0 paths. |
| `drivers::block::ahci::AhciDevice.inner` | process-context only | kept as `SwitchMutex` | Submitters take it; MSI handler only acknowledges register status and does not take this lock. |
| `drivers::block::nvme::NVME_DEVICES` | process-context only | kept as `SwitchMutex` | Probe/lookup/debug paths use it; MSI/MSI-X handler only counts interrupts. |
| `drivers::block::nvme::NvmeNamespaceDevice.inner` | process-context only | kept as `SwitchMutex` | Synchronous submitters take it; MSI/MSI-X handler does not take queue ownership. |
| `drivers::block::REGISTRY` | process-context only | kept as `SwitchMutex` | Block registry mutation/lookup happens from probe and process/test setup, not interrupt handlers. |
| `drivers::block::virtio_blk::PRIMARY_DEVICE` | process-context only | kept as `SwitchMutex` | Probe/debug paths use it; completion path uses `VIRTIO_DEVICES`. |
| `drivers::block::virtio_blk::VirtioBlkDevice.inner` | scheduler/context-switch special | kept as `SwitchMutex` | Request submission parks through `block_current_with_token_and_guard`, requiring release-token semantics. |
| `sched::scheduler::PerCpuScheduler.inner` | scheduler/context-switch special | kept as `SwitchMutex` | Context-switch shim releases the scheduler guard on the new stack. |
| `sched::process::Process.handles` | process-context only | kept as `SwitchMutex` | Syscall/process cleanup may run under explicit IF-off discipline; no ISR acquires the handle table. |
| `sched::process::Process.derivations` | process-context only | kept as `SwitchMutex` | Capability derivation walks are process-context; no interrupt handler mutates derivations. |
| `sched::process::Process.waiters` | scheduler/context-switch special | kept as `SwitchMutex` | `sys_wait` parks through `block_current_into_with_guard`, requiring release-token semantics. |
| `sched::kthread::REGISTRY` | process/kthread context only | kept as `SwitchMutex` | Spawn/trampoline/detach/unregister are not reached from interrupt handlers. |
| `sched::kthread::KthreadControl.state` | scheduler/context-switch special | kept as `SwitchMutex` | Stop/join waiters park through `block_current_with_token_and_guard`. |
| `fs::OpenFile.offset` | process-context only | kept as `SwitchMutex` | VFS syscall cursor state only; no ISR path reaches it. |
| `fs::OpenFile.generation` | process-context only | kept as `SwitchMutex` | Stale-handle generation snapshot is VFS/process state only. |
| `fs::MOUNTS` | process-context only | kept as `SwitchMutex` | Mount namespace mutation/lookup is process-context only. |
| `fs::TestInode.data` | process-context only | kept as `SwitchMutex` | Testfs payload is touched by VFS/test setup paths, never interrupt handlers. |
| `fs::neofs::NeoFsBlockCache.inner` | scheduler/context-switch special | kept as `SwitchMutex` | Writeback worker waiters park through `block_current_with_token_and_guard`. |
| `fs::neofs::NeoFsState.metadata` | process/kthread context only | kept as `SwitchMutex` | NeoFS metadata is not inspected directly by IRQ handlers. |
| `fs::neofs::NeoFsState.root` | process/kthread context only | kept as `SwitchMutex` | Root state has no interrupt-context acquirer. |
| `fs::neofs::NeoFsState.entries` | process/kthread context only | kept as `SwitchMutex` | Directory entry state has no interrupt-context acquirer. |
| `fs::neofs::NeoFsState.writeback_worker` | process/kthread context only | kept as `SwitchMutex` | Lifecycle handle is managed by process/kthread paths. |
| `fs::neofs::NeoInode.inode_block` | process/kthread context only | kept as `SwitchMutex` | Filesystem IRQ work is delegated below NeoFS to block drivers. |
| `fs::neofs::NeoInode.record` | process/kthread context only | kept as `SwitchMutex` | No ISR path acquires inode records. |

## Channel.state wrapper disposition

Removed or narrowed state-only `without_interrupts(...)` wrappers:

- `Channel::fail_recv_grant`: state extraction now locks directly; scheduler handoff remains wrapped after the guard is dropped.
- Debug helpers: `debug_endpoint_is_grant_blocked`, `debug_endpoint_is_send_blocked`, `debug_endpoint_is_recv_blocked`, and `debug_endpoints_are_closed` now lock `Channel.state` directly.
- `Channel::extract_my_side_if_blocked`: forced-kill extraction now locks directly.
- `Channel::close_peer`: channel-state extraction now locks directly; wake/zombify remains wrapped because it touches scheduler queues.
- Grant failure path in `sys_grant_impl`: the caller no longer wraps `channel.fail_recv_grant(side)` solely for `Channel.state` acquisition.

Retained wrappers:

- `sys_send`, `sys_recv`, `sys_grant`, and `sys_recv_grant` still wrap the complete IPC handoff because the block/handoff path can touch scheduler queues after `Channel.state` acquisition.
- `close_peer` and `fail_recv_grant` retain narrowed scheduler wake/zombify wrappers after state extraction.

## INV-IPC-5

Exact scenario added and wired as `ipc_rendezvous_translate_fault`.

- Kernel feature: `neo-test-ipc-rendezvous-translate-fault`
- Userland features: `ipc-rendezvous-translate-fault`
- xtask scenario: `ipc_rendezvous_translate_fault`
- Harness evidence: `target/neo-test/ipc_rendezvous_translate_fault.serial.log`
- PASS line: `NEO_TEST/1 event=PASS name=ipc_rendezvous_translate_fault inv=INV-IPC-5 translate_fault=clean_unwind exit_code=0`

Scenario shape: the parent parks in raw `sys_recv` with an unmapped userspace receive pointer, the worker attempts the peer `send`, both sides observe `EFAULT`, the worker exits 0, and the parent reaps it. This exercises rendezvous unwind after a userspace translation/copy fault without leaving the peer permanently blocked.

## Verification

All commands were run from `E:\AI Automation\AltimitNeo`.

- `docker compose run --rm neo cargo k` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test irqsafe_mutex_if_restore irqsafe_mutex_bench --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_rendezvous_translate_fault --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 -e CARGO_BUILD_JOBS=1 neo cargo neo-test sys_wait_blocking sys_wait_wnohang sys_wait_already_exited sys_wait_multiple_waiters sys_inspect_running sys_inspect_exited sys_inspect_rights_gate cap_mint_and_use cap_derive_subset cap_derive_rights_violation cap_derive_missing_meta cap_grant_basic cap_grant_missing_meta cap_revoke_local cap_revoke_cross_process cap_revoke_descendants_walk cap_revoke_spawn_inheritance_descendants cap_survive_exit sched_ready_queue_capacity_pressure ipc_rendezvous_translate_fault --no-meta` - PASS.
- `docker compose run --rm -e NEO_SMP=4 -e CARGO_BUILD_JOBS=1 neo cargo neo-test all --no-meta` - PASS, wall time 4287.5s.
- `git diff --check` - PASS.

`irqsafe_mutex_bench` evidence:

- Serial log: `target/neo-test/irqsafe_mutex_bench.serial.log`
- PASS line: `NEO_TEST/1 event=PASS name=irqsafe_mutex_bench spin_mean_ns=864 irqsafe_mean_ns=3834 switch_mean_ns=1473 overhead_ns=2970 threshold_ns=6000`

## 200/200 stress gate

Final successful 200-run gate:

- Container name: `altimitneo-smp-stress-200-final-20260525-094951`
- Container ID: `551a8704c66279924bac732617d02ae17aef041cc865f279b7099d9c2c4200da`
- Environment: `NEO_SMP=4`, `CARGO_BUILD_JOBS=1`
- Saved output: `target/neo-test/smp_cross_cpu_rendezvous_stress_200run_20260525-094951.docker.log`
- `docker wait 551a8704c66279924bac732617d02ae17aef041cc865f279b7099d9c2c4200da` - `0`
- Completion line: `[loop] COMPLETE 200/200 2026-05-25T14:19:44+00:00`

Decoded loop body:

```sh
set -euo pipefail
for i in $(seq 1 200); do
  printf '[loop] smp_cross_cpu_rendezvous_stress %03d/200 start %s\n' "$i" "$(date -Iseconds)"
  cargo neo-test smp_cross_cpu_rendezvous_stress --no-meta
  printf '[loop] smp_cross_cpu_rendezvous_stress %03d/200 PASS %s\n' "$i" "$(date -Iseconds)"
done
printf '[loop] COMPLETE 200/200 %s\n' "$(date -Iseconds)"
```

An earlier named rerun, `altimitneo-smp-stress-200-final-20260525-093727`, failed before QEMU while the container was compiling dependencies, with Docker/host filesystem I/O symptoms (`failed to write ...`, `Bad address`, `os error 5`). It was not a kernel scenario failure. The final rerun above used `CARGO_BUILD_JOBS=1` and completed 200/200.

## Serial-log scan

Relevant successful scenario logs for this subphase were scanned for:

- `event=FAIL`
- `KERNEL PANIC`
- `BUG:`
- `frame leak detected`
- `frame_leak`
- `leak_audit=dirty`
- `frame_audit.*leak`

Result:

- `CLEAN relevant-success-log scan files=23`

Scope note: `target/neo-test` retains older attempt logs and deliberate/expected failure scenario logs such as the meta-test expected failure and expected shootdown panic. The acceptance scan was therefore restricted to current successful `.serial.log` files for this subphase's exercised scenarios.

## Prior subphase anti-regression

Preserved and verified at `NEO_SMP=4`:

- `sys_wait_blocking` - PASS.
- `sys_wait_wnohang` - PASS.
- `sys_wait_already_exited` - PASS.
- `sys_wait_multiple_waiters` - PASS.
- `sys_inspect_running` - PASS.
- `sys_inspect_exited` - PASS.
- `sys_inspect_rights_gate` - PASS.
- `cap_mint_and_use` - PASS.
- `cap_derive_subset` - PASS.
- `cap_derive_rights_violation` - PASS.
- `cap_derive_missing_meta` - PASS.
- `cap_grant_basic` - PASS.
- `cap_grant_missing_meta` - PASS.
- `cap_revoke_local` - PASS.
- `cap_revoke_cross_process` - PASS.
- `cap_revoke_descendants_walk` - PASS, exact `INV-CAP-11` gate.
- `cap_revoke_spawn_inheritance_descendants` - PASS.
- `cap_survive_exit` - PASS.
- `sched_ready_queue_capacity_pressure` - PASS.

No `sys_wait`, `sys_inspect`, `sys_close`, or scheduler-ring ABI/behavior change was made for this subphase.

## Known blockers or caveats

- No hardware evidence was collected; hardware work is explicitly out of scope for this dispatch.
- No perf harness was implemented; only the existing `irqsafe_mutex_bench` evidence was captured.
- The broad retained-log directory contains stale or expected failure logs. Current exercised-success logs scanned clean as recorded above.
- The working tree had pre-existing untracked `ai_digest.py`; it was not touched or committed.

## ADR-019 close-documentation status

Landed in implementation commit `e7094a860d0b928a5bb08046009357fd8eb2f6d7`:

- `docs/ADR-019-switchmutex-irq-off-by-construction.md`
- Section: `Amendment 2026-05-25 - Sub-phases B/C landed`

That amendment records that `Channel.state` is now structurally IRQ-safe, the IPC release-token path is preserved through `IrqSafeMutex`, state-only manual wrappers were removed or narrowed, production `SwitchMutex` declarations were audited, and the subphase 3 gates passed.
