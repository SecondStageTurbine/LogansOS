# Dispatch - WAKE_TOKEN_STATE IF-Invariant Correction

Date: 2026-05-19
Agent: Cerebro
Status: AUTHORIZED CORRECTION BEFORE BUILD-4B

Authority:

- `Plans/PM-DISPOSITION-2026-05-19-wake-token-abandon-close-consumption.md`
- `Plans/PHASE-3-BUILD-4-WAKE-TOKEN-ABANDON-ROOT-CAUSE-CLOSE-2026-05-18.md`
- Commit `f63232d` (`test/wake-token: hold WAKE_TOKEN_STATE with IF=0 in wait helpers`)

## 1. Purpose

Close the residual `WAKE_TOKEN_STATE` mixed-IF hole left after `f63232d`.

The root cause was correct: same-CPU `SwitchMutex` deadlock caused by mixed IF=0
and IF=1 holders of the same lock. The fix wrapped the wait helpers, but three
post-wake blocker writes still acquire `WAKE_TOKEN_STATE` at IF=1.

This lane completes that invariant. It is not BUILD-4B and not a broad scheduler
redesign.

## 2. Required Code Change

Single file:

```text
kernel/src/test_harness.rs
```

Wrap the remaining `WAKE_TOKEN_STATE.lock()` acquisitions in
`crate::arch::x86_64::without_interrupts`.

At current HEAD, the unwrapped sites are:

```text
kernel/src/test_harness.rs:8425
kernel/src/test_harness.rs:8542
kernel/src/test_harness.rs:8558
```

Expected shape:

```rust
crate::arch::x86_64::without_interrupts(|| {
    let mut state = WAKE_TOKEN_STATE.lock();
    state.ret = ret;
    state.done = true;
});
```

and the equivalent shape for `second_ret` / `second_done`.

After the edit, `rg -n "WAKE_TOKEN_STATE\\.lock" kernel/src/test_harness.rs`
must show every acquisition either directly inside a `without_interrupts` call or
inside an immediately enclosing `without_interrupts` block.

## 3. Anti-Scope

Do not:

- touch scheduler internals;
- touch storage, fsync, NeoFS, readdir, or BUILD-4B code;
- add retries or timeout changes;
- weaken or remove wake-token scenarios;
- perform a broad `SwitchMutex` audit in this commit.

The broader mixed-IF `SwitchMutex` audit is real follow-up work, but this lane is
only the residual `WAKE_TOKEN_STATE` invariant correction.

## 4. Verification

Run build gates:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
```

Run targeted wake-token loops under `NEO_SMP=4`:

```text
50x phase3_sched_wake_token_abandon
50x phase3_sched_wake_token_round_trip
50x phase3_sched_wake_token_stale_generation
```

Run guardrails under `NEO_SMP=4`:

```text
30x ipc_heap_kill_smoke
30x cap_revoke_local
20x phase3_neofs_fsync_filecap_stale_regression
```

Final gate:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Acceptance:

- all three wake-token scenarios have 50/50 final PASS;
- 0 final-fail across all targeted and guardrail loops;
- any retry-pass must be explained with preserved per-attempt logs;
- the previous `event=BEGIN ... enqueued user thread (id=7)` truncation does not recur;
- stale-fsync remains clean of the prior virtio deadlock signature;
- warning baseline does not increase without explanation.

## 5. Close Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-WAKE-TOKEN-STATE-IF-INVARIANT-CORRECTION-CLOSE-2026-05-19.md
```

Include:

- exact HEAD and worktree state;
- code diff summary;
- `rg -n "WAKE_TOKEN_STATE\\.lock" kernel/src/test_harness.rs` output after the fix;
- build gate results;
- targeted loop counts;
- guardrail loop counts;
- final full-suite result;
- whether BUILD-4B is unblocked or whether PM must still consume a broader mixed-IF audit.

## 6. Commit Policy

Commit only after:

- every `WAKE_TOKEN_STATE` holder obeys the IF=0 invariant;
- targeted wake-token loops pass;
- guardrail loops pass;
- final `NEO_SMP=4 cargo neo-test --no-meta` exits 0;
- close artifact is written.

No push. No merge. No BUILD-4B until PM consumes the correction close.
