# Dispatch - Wake-Token-Abandon Scheduler Timing Root Cause

Date: 2026-05-18
Agent: Cerebro
Status: AUTHORIZED INVESTIGATION AND NARROW FIX LANE

Authority:

- `Plans/PM-DISPOSITION-2026-05-18-stale-fsync-virtio-irqfix-close-consumption.md`
- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-VIRTIO-IRQFIX-CLOSE-2026-05-18.md`
- `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`

## 1. Purpose

Root-cause the remaining `phase3_sched_wake_token_abandon` timing tail under
`NEO_SMP=4`.

The stale-fsync virtio deadlock is closed. This is a separate scheduler lane.
Do not treat this as storage work, fsync work, readdir work, or BUILD-4B feature
work.

Known current evidence:

- stale-fsync IRQ fix passed 50/50 with 0 retries and no virtio signature;
- mixed wake-token-abandon loop hit a final timeout at run 19;
- isolated wake-token-abandon rerun passed 30/30 but had 5 retry-pass attempts;
- retry/fail attempts truncate after `event=BEGIN` and the scheduler enqueue of
  user thread `id=7`;
- retry/fail attempts have no stale-fsync or virtio lock signature;
- older CI triage already identified wake-token-abandon as a nondeterministic SMP
  scheduler scenario.

This lane exists because "pre-existing" is not the same thing as "safe to ship."

## 2. First Move: Read-Only Audit

Before writing instrumentation, perform a read-only substrate audit and file a
short note if the code shape differs from the assumptions below.

Audit:

- the `phase3_sched_wake_token_abandon` scenario in `kernel/src/test_harness.rs`;
- wake-token installation, wake, abandon, and zombify paths in the scheduler;
- `block_current_with_token_and_guard` and any release-on-context-switch helper it uses;
- process termination and reap paths relevant to wake-token parked threads;
- current `kill_thread_by_pid` behavior, including any existing multi-CPU scan commit;
- current timeout evidence support in xtask (`NEO_TEST_RUN_ID`, per-attempt logs, `NEO_QEMU_TIMEOUT_DUMP`).

Do not assume the 2026-05-16 CPU-local kill-path finding is still unfixed.
Audit the current HEAD.

## 3. Classification Targets

Classify the remaining tail into one of these buckets before proposing a fix:

| Case | Meaning |
|---|---|
| A | Test ordering race: the harness observes before the worker has installed the wake token or reached the intended state. |
| B | Ready-queue ownership race: the target thread is Ready or Running on a remote CPU when the kill or abandon path expects parked state. |
| C | Wake-token abandon bug: token exists, but termination/reap does not convert it to the expected zombie/reaped state. |
| D | Wake-token wake bug: token is consumed or woken but the target never becomes runnable or never exits. |
| E | Scheduler lock/livelock: CPUs converge in spin or lock wait before the PASS/FAIL frame. |
| F | Panic/debug-exit path broken: the test reaches a fail condition but does not reach the debug-exit sentinel. |
| G | No local reproduction with current durable evidence: loop passes cleanly enough that CI-side artifact data is required. |

If evidence points elsewhere, name a new case explicitly. Do not squeeze a new
mechanism into the wrong bucket.

## 4. Allowed Instrumentation

Temporary instrumentation is allowed if the read-only audit cannot classify the
tail.

Allowed:

- NEO_TEST progress frames around scenario-local phase boundaries;
- scheduler trace frames for wake-token install, wake, abandon, zombify, reap, and ready-queue enqueue/dequeue;
- per-thread lifecycle markers for the worker and test harness thread;
- lock/site snapshots if a timeout dump suggests spin convergence;
- use of existing `NEO_TEST_RUN_ID` and `NEO_QEMU_TIMEOUT_DUMP`.

Rules:

- keep instrumentation gated or temporary;
- preserve serial logs for every timed-out or retry-pass attempt;
- do not commit instrumentation until the simplify gate decides it is durable;
- do not add retries, timeout bumps, scenario removals, or "just run it again" masking.

## 5. Initial Measurement

After the read-only audit, run a reproduction loop with durable evidence:

```powershell
for ($i = 1; $i -le 50; $i++) {
  $tag = "wake-token-rootcause-{0:D2}" -f $i
  docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=$tag -e NEO_QEMU_TIMEOUT_DUMP=1 neo cargo neo-test phase3_sched_wake_token_abandon
}
```

Equivalent scripting is fine. The important part is:

- `NEO_SMP=4`;
- unique `NEO_TEST_RUN_ID` per run;
- timeout dump enabled;
- per-attempt serial logs preserved.

Stop early if any of these fire:

- first final timeout or semantic FAIL with serial and timeout dump captured;
- three retry-pass attempts with the same last-frame signature captured;
- a convergent spin/lock signature is captured;
- the loop reaches 50/50 with 0 retry-pass.

## 6. Fix Policy

A fix is authorized only after classification.

Allowed fixes:

- narrow scheduler or test-harness mechanism changes tied directly to the classified failure;
- small evidence-substrate additions if they are independently useful and default-off or behavior-neutral.

Forbidden fixes:

- timeout increases;
- retry-count changes;
- scenario removal or weakening;
- broad scheduler redesign;
- unrelated storage, readdir, fsync, or BUILD-4B feature work;
- reverting unrelated commits.

If the fix requires a broad scheduler redesign, stop and file findings instead
of starting a large refactor.

## 7. Verification After Fix

Before commit:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
```

Then run targeted loops:

```text
50x NEO_SMP=4 phase3_sched_wake_token_abandon
30x NEO_SMP=4 ipc_heap_kill_smoke
30x NEO_SMP=4 cap_revoke_local
20x NEO_SMP=4 phase3_neofs_fsync_filecap_stale_regression
```

Acceptance:

- wake-token-abandon must have 50/50 final PASS;
- the previous truncation signature after `event=BEGIN` must not recur;
- any retry-pass must be explained by captured evidence, not hand-waved;
- any semantic FAIL or final timeout is a blocker;
- stale-fsync must remain clean of the prior virtio deadlock signature.

Final gate:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

If this exits nonzero, file a blocker with exact scenario, wrapper status, serial
evidence, and timeout dump if present.

## 8. Simplify Gate

Before commit, run a simplify gate over the lane:

- keep the root fix;
- keep only durable diagnostic substrate that has value beyond this investigation;
- remove temporary trace frames and lock snapshots unless PM explicitly accepts them;
- boot or scenario-test after each removal group;
- do not ship failed hypotheses by inertia.

## 9. Required Close Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-WAKE-TOKEN-ABANDON-ROOT-CAUSE-CLOSE-2026-05-18.md
```

Include:

- exact HEAD and worktree state;
- classification case and evidence;
- serial last-frame evidence for retry/fail attempts;
- timeout dump summary if any timeout occurred;
- code diff summary;
- simplify gate table;
- targeted loop results;
- final full-suite result;
- explicit statement whether BUILD-4B is unblocked.

## 10. Commit Policy

Commit only after:

- the tail is classified;
- any fix is narrow and tied to that classification;
- simplify gate is complete;
- targeted loops pass;
- final `NEO_SMP=4 cargo neo-test --no-meta` exits 0;
- close artifact is written.

No push. No merge. No BUILD-4B until PM consumes the close artifact.
