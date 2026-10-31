# Dispatch - Phase 3 BUILD-4 Stale-Fsync Liveness Root Cause

Date: 2026-05-18
Agent: Cerebro
Status: AUTHORIZED INVESTIGATION + TARGETED FIX

Authority:

- `Plans/PM-DISPOSITION-2026-05-18-phase-3-build-4-close-with-caveat-superseded.md`
- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-6-BLOCKER-2026-05-17.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`

## 1. PM Decision

The residual stale-fsync double-timeout is not accepted for BUILD-4 closure.

The previous close-with-caveat posture is superseded. BUILD-4 remains blocked
until the timeout mechanism is root-caused and fixed, or until PM accepts a
blocker proving the current local QEMU/host substrate cannot reliably execute
the gate and a replacement verification substrate is required.

This lane is not test-harness tuning. It is a storage/block/scheduler liveness
root-cause lane.

## 2. Scope

Target only:

```text
phase3_neofs_fsync_filecap_stale_regression
```

under:

```text
NEO_SMP=4
```

Current known symptoms:

- Fix 9: failures clustered after `mount_done`, during `seed_fsync`.
- Fix 14: larger timeout moved failures to pre-mount.
- Fix 15: smaller valid disk moved mount inside budget, but `seed_fsync`
  double-timeouts returned.

The goal is to classify exactly where forward progress stops.

## 3. Required Classification

A timeout must be classified into exactly one of these cases:

- **A. Guest stopped executing:** QEMU/host does not deliver NMI/monitor capture
  and serial output remains frozen. This is substrate failure, not kernel logic.
- **B. Virtio request submitted but never completed:** block device/queue/kick
  path is stuck.
- **C. Virtio completed but waiter was not woken:** completion-to-scheduler
  handoff bug.
- **D. Waiter was woken but did not run:** scheduler/runqueue/reschedule bug.
- **E. Waiter ran but fsync made no progress:** NeoFS drain/cache state bug.
- **F. Fsync is progressing but too slowly:** serialized one-block-at-a-time
  architecture cannot satisfy the gate under host variance.
- **G. Semantic failure:** stale cap, durability, flush, or refreshed-cap
  invariant fails. This is a kernel correctness bug, not a timeout tail.
- **H. No convergent evidence:** instrumentation insufficient; file a blocker
  with the missing observation.

Do not proceed to a fix until the failure is classified.

## 4. Instrumentation Requirements

Add the smallest instrumentation needed to classify the timeout. Prefer
counters and bounded summary frames over per-block log spam.

Required evidence points:

- fsync phase: mount, seed_fsync, hold, mutation, stale_fsync, refreshed_fsync;
- NeoFS drain state: dirty block count, drained block count, flush-required,
  flush count, current drain phase;
- block request state: last submitted request id/op/block, last completed
  request id/op/block/status, outstanding request count;
- wake-token state: wake result, target CPU, current CPU, stale/gone/woken;
- scheduler state on timeout: current thread per CPU, ready counts, blocked
  counts if obtainable;
- out-of-band liveness: on timeout, attempt to capture guest state using the
  existing QEMU monitor/NMI-style tooling or an equivalent one-shot dump.

Instrumentation may be temporary or committed if it becomes generally useful.
If committed, it must be narrow and disabled/quiet outside targeted diagnostic
paths.

## 5. Fix Policy

Allowed fixes after classification:

- Case B: fix virtio queue/kick/completion liveness, or add bounded request
  timeout/error semantics if the device can fail without completion.
- Case C: fix completion-to-waiter wake path.
- Case D: fix scheduler/reschedule/runqueue handoff.
- Case E: fix NeoFS drain/cache state machine.
- Case F: stop before implementation and file a design checkpoint for pipelined
  fsync/multi-in-flight drain. Do not casually rewrite fsync ordering inside
  this lane.
- Case A: file substrate blocker with proof. Propose a replacement verification
  substrate, such as a cleaner Linux host/runner, non-WSL runner, or dedicated
  QEMU/KVM environment.
- Case G: fix the semantic bug directly; no caveat path.

Forbidden fixes:

- larger timeout;
- extra retries;
- scenario removal/ignore;
- weakening assertions;
- changing the disk size again;
- changing storage backend mode as the primary fix;
- claiming "environmental" without out-of-band evidence.

## 6. Verification Gate

After a targeted fix lands, run:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Then run a loop:

```powershell
50x NEO_SMP=4 phase3_neofs_fsync_filecap_stale_regression
```

Acceptance threshold:

- 50/50 final PASS;
- 0 final double-timeouts;
- retry-pass count must be reported, not hidden;
- every retry-pass must have preserved attempt logs;
- any semantic `event=FAIL` is a blocker.

If the fix targets scheduler or block wakeups, also run:

```powershell
30x NEO_SMP=4 phase3_sched_wake_token_abandon
20x NEO_SMP=4 ipc_heap_kill_smoke
```

After targeted loops pass, run:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Final full-suite gate must exit 0 before BUILD-4B is unblocked.

## 7. Required Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-STALE-FSYNC-LIVENESS-FINDINGS-2026-05-18.md
```

The artifact must include:

- exact HEAD and worktree state;
- instrumentation summary and whether it was committed or temporary;
- classification case A-H with evidence;
- serial tails and attempt-log paths for every timeout/failure observed;
- literal PASS frames for passing targeted runs;
- retry-pass table;
- fix description, if any;
- verification results;
- explicit statement whether BUILD-4B is still blocked.

## 8. Commit Policy

No push. No merge. No BUILD-4B.

Commit only after:

- classification is complete;
- any mechanism fix is implemented;
- build gates pass;
- targeted 50x stale-fsync loop passes with 0 final double-timeouts;
- final full-suite gate exits 0.

If classification proves Case A substrate failure, do not commit a fake kernel
fix. File the blocker and wait for PM disposition.
