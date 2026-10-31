# PM Disposition - Wake-Token-Abandon Close Consumption

Date: 2026-05-19
Authority consumed:

- `Plans/PHASE-3-BUILD-4-WAKE-TOKEN-ABANDON-ROOT-CAUSE-CLOSE-2026-05-18.md`
- `Plans/DISPATCH-2026-05-18-wake-token-abandon-root-cause-cerebro.md`
- Commit `f63232d` (`test/wake-token: hold WAKE_TOKEN_STATE with IF=0 in wait helpers`)

## Disposition

PM accepts the root-cause classification and the targeted mechanism direction,
but does **not** accept BUILD-4 wake-token closure as-is.

The close artifact proves the observed `phase3_sched_wake_token_abandon` failure:

```text
same-CPU SwitchMutex deadlock on WAKE_TOKEN_STATE
mixed IF=0 and IF=1 holders
all CPUs converge on __mm_pause
```

The targeted loops are strong evidence that the specific wake-token-abandon tail
was fixed:

- 50/50 `phase3_sched_wake_token_abandon` PASS;
- 30/30 `ipc_heap_kill_smoke` PASS;
- 30/30 `cap_revoke_local` PASS;
- 20/20 `phase3_neofs_fsync_filecap_stale_regression` PASS;
- 0 retry-pass and 0 final-fail across the 130 targeted runs;
- final `NEO_SMP=4 cargo neo-test --no-meta` exited 0.

However, the committed code still has IF=1 holders of the same
`WAKE_TOKEN_STATE` lock.

## Blocking Finding

The new comment in `reset_wake_token_state` correctly states the invariant:

```text
Every other holder must too.
```

But current HEAD still contains these unwrapped holders:

```text
kernel/src/test_harness.rs:8425  let mut state = WAKE_TOKEN_STATE.lock();
kernel/src/test_harness.rs:8542  let mut state = WAKE_TOKEN_STATE.lock();
kernel/src/test_harness.rs:8558  let mut state = WAKE_TOKEN_STATE.lock();
```

These are the post-wake `ret/done` writes in the wake-token round-trip and
stale-generation blocker paths. They are not the exact failing path from
`phase3_sched_wake_token_abandon`, but they violate the same lock-level invariant
that the close artifact names as the root cause.

Given the project rule that we do not ship broken code, this is a closure
blocker. Passing the full suite once is not enough to accept a lock invariant
that is still only partially enforced.

## Accepted Parts

Accepted:

- the Case E classification;
- the same-CPU mixed-IF `SwitchMutex` mechanism;
- the decision to fix at the test-harness lock sites rather than weakening the scenario;
- the durable evidence substrate from the prior xtask lane;
- the fact that the older 2026-05-16 CPU-local `kill_thread_by_pid` finding was audited and is fixed at current HEAD.

Not accepted yet:

- BUILD-4B unblocking;
- wake-token closure as final;
- the statement that all `WAKE_TOKEN_STATE` holders obey the IF=0 invariant.

## Required Correction

Next authorized lane:

```text
Plans/DISPATCH-2026-05-19-wake-token-state-if-invariant-correction-cerebro.md
```

Scope is intentionally tiny: make every `WAKE_TOKEN_STATE.lock()` acquisition
run under `without_interrupts`, then prove all three wake-token scenarios under
`NEO_SMP=4`.

No push. No merge. No BUILD-4B until this correction is consumed.
