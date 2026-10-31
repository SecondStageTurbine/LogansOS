# Dispatch - Stale-Fsync Spin-Livelock Localization

Date: 2026-05-18
Agent: Cerebro
Status: AUTHORIZED LOCALIZATION / NO MECHANISM FIX YET

Authority:

- `Plans/PM-DISPOSITION-2026-05-18-stale-fsync-liveness-findings-consumption.md`
- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-LIVENESS-INSTRUMENTATION-FINDINGS-2026-05-18.md`
- `Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md`

## 1. Purpose

Localize the exact lock/spin site behind the stale-fsync SMP livelock.

The prior instrumentation classified the blocker into the Case C/D family:

```text
write_fs_block enters submit-and-wait
no WRITE_DONE
VM still running
all CPUs converge on __mm_pause
one CPU has IF=0
```

`__mm_pause` is a shared debug-build symbol, so RIP alone does not identify the
spin site. This lane must identify which lock each CPU is spinning on and who
owns that lock.

## 2. Scope

Target only:

```text
NEO_SMP=4 cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Use the current WIP diagnostic substrate. Keep the existing stale-fsync
`NEO_DIAG/1` write/flush frames and timeout monitor dump.

No mechanism fix is authorized until this lane produces a lock/site
localization finding.

## 3. Existing Asset To Use

Do not add a new generic owner system.

`SwitchMutex` already carries `LockOwner` metadata under `kernel-diagnostic`:

- owner CPU;
- owner thread id;
- acquisition site hash;
- seqlock-protected `owner_snapshot()`.

The missing piece is contention-target visibility: when a CPU is spinning in
`SwitchMutex::lock()`, we need to know the mutex address, request site, owner
snapshot, current CPU/thread, and spin duration/sample count.

## 4. Authorized Instrumentation

### 4.1 SwitchMutex contention snapshot

Add `kernel-diagnostic` / `neo-test` gated contention tracking to
`SwitchMutex::lock()`.

For each CPU, record a compact "currently spinning on" snapshot:

- spinner CPU id;
- spinner thread id;
- requested lock address;
- requester acquisition site hash;
- owner snapshot at time of contention, if consistent;
- spin sample counter or approximate loop count;
- whether the lock was later acquired.

Clear the per-CPU snapshot after acquisition succeeds.

Do not serial-print from every spin iteration. The snapshot should be readable
by timeout/NMI diagnostic paths; continuous serial output from a spin loop risks
perturbing or worsening the bug.

### 4.2 Relevant lock snapshots

Expose narrow debug snapshots for the locks on the stale-fsync path:

- per-CPU scheduler `inner` locks;
- virtio-blk device `inner`;
- NeoFS cache `inner`;
- NeoFS writeback worker / mount locks only if they are on the observed path.

If a lock is private and exposing it broadly would bloat API surface, use a
`neo-test`-gated debug function that emits only the fields needed for this
investigation.

### 4.3 Timeout diagnostic output

On timeout, capture both:

- QEMU monitor register dump, as already implemented;
- kernel-side spin/lock snapshot.

Preferred shape:

- add an opt-in timeout NMI path such as `NEO_QEMU_TIMEOUT_NMI=1`, or reuse an
  existing monitor/NMI mechanism if already present;
- have the NMI handler print a bounded `NEO_DIAG/1 event=SPIN_SNAPSHOT` report
  for all CPU slots using atomics only;
- then halt/allow xtask to kill QEMU as before.

If NMI delivery only reaches one CPU, that is acceptable as long as the handler
prints the global per-CPU spin snapshots captured by all CPUs.

### 4.4 Optional source audit

Run a targeted audit for scheduler lock misuse:

- `scheduler.inner.lock()` held across yield, block, context switch, or
  cross-CPU wake;
- lock acquisition order involving virtio `inner` and scheduler `inner`;
- same CPU attempting to reacquire a scheduler lock while already holding it;
- wake-token path spinning on a target CPU scheduler lock while the target CPU
  is unable to release it.

The audit should support the lock-snapshot evidence. It is not a substitute for
capturing the contended lock.

## 5. Stop Conditions

Stop and file findings when one of these occurs:

- first retry-pass or final-fail sample identifies a contended lock and owner;
- three samples converge on the same contended lock/site;
- NMI/timeout snapshot path fails to capture useful state;
- instrumentation masks the failure for 30 attempts;
- any semantic `event=FAIL` appears.

## 6. Verification / Measurement

After instrumentation builds:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
```

Then run:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=spinloc-smoke -e NEO_QEMU_TIMEOUT_DUMP=1 -e NEO_QEMU_TIMEOUT_NMI=1 -e NEO_QEMU_MONITOR_PORT=45555 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

If smoke passes, run up to 30 sequential attempts with unique
`NEO_TEST_RUN_ID=spinloc-NN`.

Do not run parallel loops on the same host.

## 7. Required Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-STALE-FSYNC-SPIN-LOCALIZATION-FINDINGS-2026-05-18.md
```

Include:

- exact HEAD and worktree state;
- instrumentation diff summary;
- whether existing ADR-020 owner metadata was reused;
- timeout/retry table;
- last fsync diagnostic frame for each captured timeout;
- monitor register summary;
- spin snapshot summary per CPU;
- contended lock address/type if identified;
- owner CPU/thread/site if identified;
- C vs D leaning after localization;
- recommended mechanism fix lane;
- whether BUILD-4B remains blocked.

## 8. Commit Policy

No commit for localization-only instrumentation until PM consumes findings.

No push. No merge. No BUILD-4B. No scheduler redesign. No fsync redesign.
