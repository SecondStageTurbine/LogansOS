# Dispatch - Phase 3 BUILD-4B Force-Kill FD-Teardown Interleaving Audit

Date: 2026-05-20
Agent: Jarvis
Status: AUTHORIZED OBSERVE (AUDIT-ONLY, NO IMPLEMENTATION)

Authority:

- `Plans/PM-DISPOSITION-2026-05-20-wake-token-state-if-invariant-correction-consumption.md`
- `Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md`
- `Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md` (Q9: unmount/open-handle rule)
- `docs/ADR-021-phase-3-design-substrate.md`

Substrate:

```text
0682d24 test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts
```

## 1. Mission

OBSERVE-only audit of the force-kill / FD-teardown interleaving against
BUILD-4B's proposed mount-lifetime token model, BEFORE Cerebro lands the
live-handle accounting code.

Cerebro's dispatch authorizes:

- `OpenFile` retains a mount-lifetime token or counted reference;
- `sys_umount` returns `EBUSY` while `live_handles > 0`;
- mount entry removed only after busy check passes.

PM concern: the increment/decrement of `live_handles` must be ordered against
`sys_umount`'s busy check AND against the FD-table-teardown paths in
force-kill, process-exit, and OOM-kill such that no observable window exists
in which `sys_umount` sees `live_handles == 0` while `OpenFile` references
still exist anywhere in the kernel.

The specific race shape PM is worried about:

```text
T0: Process A holds OpenFile F referencing mount M. live_handles(M) = 1.
T1: Process B calls sys_umount(M). Busy check observes 1, returns EBUSY.
T2: Process A is force-killed. Its FD-table teardown begins.
T3: Process A's drop path decrements live_handles(M) to 0.
T4: Process B retries sys_umount(M). Busy check observes 0.
T5: Process B detaches the mount entry.
T6: Process A's remaining cleanup (queued I/O completions, deferred
    Arc<OpenFile> drops, writeback worker references) still reaches
    into M's now-detached state.
```

This is a classic ABA-adjacent race: the count returns to zero before the
references it represents are fully drained.

## 2. Audit Questions

Q1. Where exactly in the current force-kill path does `Handle::File` get
    dropped? Is the drop synchronous with the kill syscall return, or
    deferred to a reaper / zombie reap path?

Q2. Are there any non-FD-table paths that can hold an `Arc<OpenFile>` alive
    past the owning process's FD-table teardown? Candidates:

    - in-flight syscalls on auxiliary threads (single-thread-per-process is
      the current kernel invariant per ADR-021 Q1; verify no exceptions);
    - IPC channel state holding fds in transit (ADR-004);
    - capability derivation tree DERIVE/GRANT chains (ADR-010);
    - kernel-internal scheduler bookkeeping holding the file ref;
    - I/O completion paths holding `DmaBuffer` Arcs that transitively
      reference `OpenFile` (likely NOT — `DmaBuffer` is separate Arc per
      ADR-021 Q3, but confirm);
    - NeoFS writeback worker holding refs into mount state (BUILD-3D).

Q3. Does the proposed decrement-on-`Drop`-of-`OpenFile` model close the
    race, or does `sys_umount` need to participate in a sequence-number /
    epoch scheme so that "live_handles == 0 AND no umount-blocking work in
    flight" is the actual busy predicate?

Q4. Specifically for the BUILD-3D writeback cache: can dirty writeback
    work be queued against M after the last user-visible FD has been
    closed but before drain completes? If yes, that work is a hidden
    live reference that must block umount.

Q5. Force-kill currently runs how? Is it synchronous in the killer's
    context, or does it post to a per-target reaper? The interleaving
    risk is highest if force-kill is asynchronous, because then the
    killer's syscall returns before the killed process's FD references
    drop.

Q6. Is there an existing kernel-wide pattern (RCU-like quiescence,
    epoch-based reclamation, an existing "drain before detach"
    primitive) that BUILD-4B should reuse rather than invent? Check
    SC-3C TLB shootdown machinery and ADR-016 IRQ-safe scheduler
    decoupling for precedent.

Q7. Does `sys_umount` need to be a two-phase operation in the kernel
    (mark-detaching → drain → detach) even if it's a single syscall
    from userspace? Two-phase makes `EBUSY` answerable precisely;
    single-phase requires the predicate to be conservative.

## 3. Audit Outputs

Required:

- Direct source reads of:

  - `kernel/src/sched/process.rs` (force-kill, exit paths);
  - `kernel/src/fs/mod.rs` (`Handle::File` drop path, `OpenFile` drop);
  - `kernel/src/cap/` (capability table teardown on process exit);
  - `kernel/src/ipc/` (channel state on process exit, fds-in-transit);
  - `kernel/src/sched/kthread.rs` (BUILD-3A writeback worker lifecycle);
  - `kernel/src/fs/neofs/` (writeback cache, BUILD-3D).

- `rg` sweeps for all sites that construct, clone, or drop `OpenFile`
  or `Handle::File`.

- A written finding answering each of Q1–Q7 with code references.

- A verdict: does the dispatched live-handle accounting model close the
  race cleanly, OR does Cerebro need a sequence-number / two-phase
  umount / drain-before-detach refinement before implementation?

## 4. Anti-Scope

NOT in this audit:

- Any code changes. This is OBSERVE-only.
- Designing the fix if a fix is needed. Surface the gap; PM decides the
  shape and re-dispatches Cerebro.
- Broader mixed-IF `SwitchMutex` audit (separate follow-up lane, not
  blocked by BUILD-4B closure per PM-DISPOSITION-2026-05-20).
- FAT32/ext4 work.
- Anything in BUILD-4C/4D scope.

## 5. Stop Conditions

Stop and surface to PM before continuing if:

- The audit reveals that the proposed mount-lifetime token model is
  structurally incompatible with current force-kill semantics in a way
  that requires changing force-kill rather than mount accounting.
- The audit finds that an `Arc<OpenFile>` can be held by kernel-internal
  state that has no defined drop trigger (an actual leak path).
- The audit finds that the BUILD-3D writeback cache holds references
  to NeoFS mount state in a way that `sys_umount` cannot observe.
- The audit timeline exceeds same-day turnaround. PM expects this to
  fit in a few hours of focused source-reading; if it expands, surface
  why.

## 6. Required Close Artifact

Create:

```text
Plans/OBSERVE-2026-05-20-phase-3-build-4B-fd-teardown-interleaving-audit-jarvis.md
```

Required content:

- substrate commit hash;
- direct answers to Q1–Q7 with code line references;
- verdict: `PROCEED-AS-DISPATCHED`, `REFINE-DISPATCH`, or `BLOCK-AND-REDESIGN`;
- if `REFINE` or `BLOCK`: specific recommended shape of the refinement;
- any incidental findings (mixed-IF lock sites encountered, other races
  noticed) noted but not chased.

## 7. PM Posture

This audit gates Cerebro's deep implementation. Cerebro may continue
exploratory work on the mount-table refactor (Step 2 of his dispatch)
in parallel, but should not commit to the `OpenFile` mount-lifetime token
shape until Jarvis's audit is consumed.

Verdict handling:

- `PROCEED-AS-DISPATCHED`: Cerebro continues unchanged.
- `REFINE-DISPATCH`: PM consumes the finding and issues a revised Cerebro
  dispatch addendum before live-handle code lands.
- `BLOCK-AND-REDESIGN`: BUILD-4B pauses and the live-handle accounting
  model goes back to PM/PL design discussion.
