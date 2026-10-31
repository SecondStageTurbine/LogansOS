# DISPATCH: Phase 2 BUILD subphase 3 - ADR-019 B/C type-level IRQ safety

- From: PM
- To: Jarvis/Codex
- Date: 2026-05-25
- Lane: Kernel implementation + tests
- Status: READY FOR PL ROUTING
- Prior gate: `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-2-close.md`
- PLAN source: `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`)
- Hardware-last constraint: `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`)

---

## 1. Mission

Implement Phase 2 BUILD subphase 3: ADR-019 B/C type-level IRQ safety.

The milestone is narrow:

- Sub-phase B: audit production `SwitchMutex` declaration sites, classify each, and migrate mixed-IRQ-context sites to `IrqSafeMutex` where the type fits.
- Sub-phase C: migrate `Channel.state` to `IrqSafeMutex` and remove the manual `without_interrupts(...)` wrappers whose only purpose was IRQ-off acquisition for that lock.
- Add or confirm exact `INV-IPC-5` coverage via `ipc_rendezvous_translate_fault`.
- Capture the existing `irqsafe_mutex_bench` evidence.
- Pass the SC-3B 200/200 `NEO_SMP=4` `smp_cross_cpu_rendezvous_stress` gate after the Channel.state migration.

This dispatch is implementation work only. It does not reopen ADR-019, THINK v0.2, or the ratified four-subphase PLAN.

## 2. Authoritative Inputs

Read these before editing:

- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-2-close.md` - subphase 2 accepted; preserve its gates.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` - subphase 3 scope, gates, anti-scope.
- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md` - PL ratification of the four-subphase PLAN.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` section 2.2 - downstream BUILD clarifications.
- `docs/ADR-019-switchmutex-irq-off-by-construction.md` - ADR-019 ratified design.
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` - Hpw mechanism and 200/200 stress-gate precedent.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md` - evidence checklist for subphase 3.

Likely source surfaces:

- `kernel/src/sync/irqsafe_mutex.rs`
- `kernel/src/sync/switch_mutex.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/sched/{scheduler,process,kthread}.rs`
- `kernel/src/drivers/block/{mod,ahci,nvme,virtio_blk}.rs`
- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/{inode,cache}.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`

Use live source as truth. The list above is a routing map, not a substitute for reading the implementation.

## 3. Required Implementation

### 3.1 Sub-Phase B - SwitchMutex Production Audit And Migration

Audit all production `SwitchMutex` declaration sites. Exclude `kernel/src/test_harness.rs` fixtures from the production audit, but preserve tests.

For each production site, classify it as one of:

- mixed-IRQ-context: both process-context and IF=0 or interrupt-context acquirers can reach the lock
- exclusively process-context: no interrupt or IF=0 process acquirer
- scheduler/context-switch special case: requires `SwitchMutexGuard` release-token semantics and cannot be migrated mechanically

Required output:

- migrate mixed-IRQ-context sites to `IrqSafeMutex` where the type fits
- keep exclusively process-context sites as `SwitchMutex` only with a short declaration-site comment explaining why no ISR/IF=0 acquirer reaches it
- keep scheduler/context-switch special cases as `SwitchMutex` only with a declaration-site comment naming the release-token requirement
- include an audit table in the close report listing each production site, classification, action, and rationale

Do not change `SwitchMutex` or `IrqSafeMutex` semantics unless a tiny API adaptation is unavoidable and explicitly justified.

### 3.2 Sub-Phase C - Channel.state Structural Migration

Migrate `Channel.state` from `SwitchMutex<[EndpointState; 2]>` to `IrqSafeMutex<[EndpointState; 2]>`.

Then remove the manual `without_interrupts(...)` wrappers whose only purpose was to make `Channel.state.lock()` IRQ-safe. The expected target set includes the SC-3B-wrapped IPC send/recv/grant/close/fail paths and any equivalent debug/cleanup helpers still present in live source.

Do not blindly remove every `without_interrupts(...)` in `kernel/src/ipc/mod.rs`. If a wrapper protects more than Channel.state acquisition, narrow it carefully or leave it with a comment and explain in the close report.

This is the highest-risk portion of the subphase. Treat `Channel.state` migration as the boundary that triggers the SC-3B 200/200 gate.

### 3.3 INV-IPC-5

Add or wire the exact audit (b) scenario:

- `ipc_rendezvous_translate_fault`

Expected property: an IPC rendezvous path that encounters a userspace translation/copy fault unwinds without leaving a peer permanently blocked and without leaking a parked thread or capability state.

If an existing scenario truly covers this property, the close report must name it and explain the fault path. If the exact scenario is absent, add it under the exact name so the audit chain can find `INV-IPC-5`.

### 3.4 Preserve Prior Subphases

Subphases 1 and 2 are accepted and must stay green:

- wait/inspect scenarios must still PASS
- exact `INV-CAP-11` must still PASS
- full `cap_*` regression suite must still PASS
- `sched_ready_queue_capacity_pressure` must still PASS
- no `sys_wait`, `sys_inspect`, `sys_close`, or scheduler-ring ABI/behavior change unless mechanically required and called out

## 4. Required Tests

Run and report:

- `docker compose run --rm neo cargo k`
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test irqsafe_mutex_if_restore irqsafe_mutex_bench --no-meta`
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_rendezvous_translate_fault --no-meta`
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress --no-meta` as a single-run sanity check
- a 200-run loop of `smp_cross_cpu_rendezvous_stress` at `NEO_SMP=4`, stopping on first failure and preserving serial logs
- full non-ignored `cargo neo-test all` at `NEO_SMP=4`, or a PM-justified equivalent if runtime is prohibitive
- subphase 1 wait/inspect + `INV-CAP-11` scenarios at `NEO_SMP=4`
- full `cap_*` regression suite at `NEO_SMP=4`
- `sched_ready_queue_capacity_pressure` at `NEO_SMP=4`
- serial-log scan for `event=FAIL`, `KERNEL PANIC`, `BUG:`, and frame-leak markers
- `git diff --check`

PowerShell loop shape for the 200-run gate, adjustable only if the repo has a better existing helper:

```powershell
1..200 | ForEach-Object {
    docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress --no-meta
    if ($LASTEXITCODE -ne 0) { throw "smp_cross_cpu_rendezvous_stress failed on run $_" }
}
```

Record the exact command used, pass count, runtime summary, and failure classifier if any run fails.

## 5. Acceptance Gates

Minimum gates before close report:

- `cargo k` PASS.
- Production `SwitchMutex` audit table complete.
- Mixed-IRQ-context production sites migrated or explicitly justified as scheduler/context-switch special cases.
- `Channel.state` uses `IrqSafeMutex`.
- Manual Channel.state `without_interrupts(...)` wrappers removed or justified where retained.
- `ipc_rendezvous_translate_fault` (`INV-IPC-5`) PASS at `NEO_SMP=4`.
- `irqsafe_mutex_bench` captured with numbers in the close report.
- SC-3B 200/200 `NEO_SMP=4` `smp_cross_cpu_rendezvous_stress` PASS.
- Prior subphase anti-regression gates remain PASS.
- No `KERNEL PANIC`, no `BUG:`, and no frame-leak marker in exercised serial logs.
- ADR-019 amendment recording B/C completion lands or ready-to-integrate amendment text is included in the close report.

## 6. Anti-Scope

Do not do any of the following in this subphase:

- change `SwitchMutex` release-on-context-switch semantics
- combine or rename `SwitchMutex` and `IrqSafeMutex`
- start perf harness implementation
- execute or prepare hardware evidence as a substitute for QEMU gates
- change scheduler ring behavior from subphase 2
- change accepted `sys_wait`/`sys_inspect` ABI or activate process/channel/capability `sys_close`
- fold in NVMe FLUSH durability diagnosis
- broaden into filesystem, block driver, allocator, or IPC feature work beyond the lock-type migration and required `INV-IPC-5` scenario

## 7. Close Report

File a close report under `Plans/` after implementation.

The close report must include:

- commit id(s)
- files changed
- production `SwitchMutex` audit table
- sites migrated to `IrqSafeMutex`
- sites intentionally kept as `SwitchMutex` and why
- exact `Channel.state` wrapper removals or retained-wrapper justifications
- `INV-IPC-5` implementation/evidence
- `irqsafe_mutex_bench` numbers
- 200/200 stress-gate evidence
- prior subphase anti-regression results
- known blockers or caveats
- ADR-019 B/C close-documentation status

## 8. Tell-Jarvis Text

Jarvis - Phase 2 BUILD subphase 2 is accepted. Please begin subphase 3 per `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md`. Scope is ADR-019 B/C only: audit/migrate production mixed-context `SwitchMutex` sites, migrate `Channel.state` to `IrqSafeMutex`, remove the manual Channel.state IRQ wrappers, add/verify exact `ipc_rendezvous_translate_fault`, capture `irqsafe_mutex_bench`, and close with the SC-3B 200/200 `NEO_SMP=4` `smp_cross_cpu_rendezvous_stress` gate. Preserve subphase 1 and 2 gates. Do not start perf harness or hardware work.
