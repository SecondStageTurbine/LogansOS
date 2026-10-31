# PM Disposition - Stale-Fsync Liveness Findings Consumption

Date: 2026-05-18
Authority consumed:

- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-LIVENESS-INSTRUMENTATION-FINDINGS-2026-05-18.md`
- `Plans/DISPATCH-2026-05-18-stale-fsync-liveness-instrumentation-cerebro.md`

## Disposition

PM accepts the liveness instrumentation findings.

The stale-fsync blocker is no longer classified as a timeout-budget problem,
storage-backend problem, or host-freeze problem. The evidence points to a
kernel-side SMP liveness bug in the wake/scheduler handoff family:

```text
convergent multi-CPU spin livelock inside write_fs_block / submit-and-wait
```

The accepted classification is **Case C-or-D family**:

- C: virtio completed but waiter was not woken;
- D: waiter was woken but did not run.

The exact lock/spin site is not yet identified. That is the next lane.

## Evidence Consumed

Accepted findings:

- QEMU/guest remained running on every timeout dump.
- All four vCPUs converged on `core::hint::spin_loop()` / `__mm_pause`.
- CPU0 had IF=0 while CPU1-3 had IF=1, consistent with one CPU in a
  `without_interrupts` critical section and the others spinning/yielding.
- Last diagnostic frame was always `FSYNC_WRITE_BEGIN`, with no matching
  `FSYNC_WRITE_DONE`.
- Pure lost-virtio-request Case B is not sufficient: submission and completion
  counters were matched in most captured stalls.
- No semantic `event=FAIL` occurred.

## Important Correction To The Proposed Next Lane

Do not add generic lock-owner tracking from scratch.

`SwitchMutex` and `IrqSafeMutex` already have ADR-020 owner metadata under the
`kernel-diagnostic` feature. The missing diagnostic is not "who owns locks" in
the abstract. The missing diagnostic is:

```text
which SwitchMutex each CPU is spinning on, and what that lock's owner_snapshot
was when contention happened
```

The next lane should expose contention-target snapshots and relevant lock-owner
snapshots. It should not reinvent the `LockOwner` system.

## New Authorized Path

The next authorized lane is:

```text
Plans/DISPATCH-2026-05-18-stale-fsync-spin-livelock-localization-cerebro.md
```

## Guardrails

Do not implement a mechanism fix yet.

Do not:

- redesign the scheduler;
- redesign fsync;
- add retries or timeouts;
- remove or weaken stale-fsync;
- infer the exact lock from `__mm_pause` alone;
- claim closure before the contended lock and holder are identified.

BUILD-4B remains blocked.
