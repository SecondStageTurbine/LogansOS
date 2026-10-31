# PM Disposition - BUILD-4B FD-Teardown Interleaving Audit Consumption

Date: 2026-05-20
From: PM
Status: ACCEPTED / VERDICT PROCEED-AS-DISPATCHED
Consumed artifact:

```text
Plans/OBSERVE-2026-05-20-phase-3-build-4B-fd-teardown-interleaving-audit-jarvis.md
```

## 1. Disposition

PM accepts the Jarvis audit and its verdict: PROCEED-AS-DISPATCHED.

The live-handle accounting model is sound under current Phase 3 BUILD-4B
constraints. The workspace at audit HEAD (`d1ec982`) already implements the
counter tied to `OpenFile` object lifetime rather than fd-table slot lifetime,
which is the load-bearing distinction that closes the PM-named race shape:

- `OpenFile::new_with_mount` increments at `kernel/src/fs/mod.rs:146`/`:153`;
- `OpenFile::Drop` decrements at `kernel/src/fs/mod.rs:297`;
- `MountTable::remove_child` checks `live_handles > 0` under the mount-table
  lock before removal at `kernel/src/fs/mod.rs:465`/`:469`;
- syscall-local `Arc<OpenFile>` clones from `file_lookup` keep the mount busy
  until their Drop runs, closing the T3–T6 window PM was worried about.

No addendum to Cerebro's BUILD-4B dispatch is required. No sequence number,
epoch scheme, or two-phase umount refinement is needed for current constraints.

## 2. Process Observation

The audit HEAD (`d1ec982`) was ahead of the dispatch substrate (`0682d24`) and
already contained BUILD-4B implementation. Jarvis effectively audited Cerebro's
in-progress code rather than only the dispatched plan. This is acceptable and
arguably stronger evidence than a pure pre-implementation audit; the verdict
applies to the actual code, not an abstract design.

PM notes for future reference: when a substrate gap opens between dispatch and
audit (because parallel agents are advancing the tree), the auditing agent
should be explicit about which HEAD it audited. Jarvis was explicit; this
behavior should be the standard.

## 3. Future-Trigger Conditions To Record In BUILD-4B Close Evidence

Jarvis named three forward conditions under which the current single-phase
umount model would break and a two-phase mark-detaching/drain/detach model
would become necessary. None apply today; all are forward markers. PM directs
that Cerebro's BUILD-4B close evidence include these explicitly so the next
PM seat does not have to re-derive them:

1. Multi-thread-per-process is enabled (currently forbidden by Phase 1.5b
   single-thread-per-process invariant per ADR-013).
2. File caps become grantable/transferable via IPC and channel state or
   pending grants can carry an actual `Arc<OpenFile>` (currently
   `Handle::File::with_cap` maps generic `CapRights` to empty rights at
   `kernel/src/sched/process.rs:284`, so file caps are not grantable).
3. A mounted filesystem owns async work that is not synchronously drained by
   filesystem drop (currently NeoFS state drop synchronously stops and joins
   the writeback worker at `kernel/src/fs/neofs/inode.rs:650`).

These are written into the dispatch addendum below.

## 4. Incidental Findings

Two items noted in the audit, neither blocking:

- `kernel/src/cap/` does not exist as a separate module; capability code is
  in `kernel/src/sched/process.rs`. PM's dispatch path list was wrong on this
  point. Not a substantive error; Jarvis adapted and covered the right code.
- Mixed-IF lock sites were visible around mount table and NeoFS cache paths
  but were not chased in this audit's scope. Folds into the existing broader
  mixed-IF SwitchMutex audit follow-up lane noted in
  `Plans/PM-DISPOSITION-2026-05-20-wake-token-state-if-invariant-correction-consumption.md`
  §"Carry-Forward".

## 5. Cerebro Coordination

PM will pass the following to Cerebro via PL:

a. Jarvis verdict: PROCEED-AS-DISPATCHED. No course change.

b. Required close-evidence addition: the three future-trigger conditions in §3.

c. Status request: current implementation progress, scenario completion,
   stop-condition status, ETA on close artifact.

Cerebro's dispatch is otherwise unchanged. All seven required scenarios, all
anti-regressions, all stop conditions, and the close-artifact requirements in
`Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md`
remain in force.
