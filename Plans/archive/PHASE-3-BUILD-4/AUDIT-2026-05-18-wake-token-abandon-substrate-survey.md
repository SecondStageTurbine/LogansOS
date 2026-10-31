# Substrate Audit - Wake-Token-Abandon Lane

Date: 2026-05-18
Agent: Cerebro
Status: READ-ONLY AUDIT (no code changes)
Authority: `Plans/DISPATCH-2026-05-18-wake-token-abandon-root-cause-cerebro.md`

Tree state: main, HEAD = `27d8c5a` (post stale-fsync IRQ-fix close). No uncommitted changes besides the audit/loop substrate kept from the prior lane (per-attempt logs, `NEO_TEST_RUN_ID`, `NEO_QEMU_TIMEOUT_DUMP`).

## 1. The dispatch's hypothesis-test, answered

> Do not assume the 2026-05-16 CPU-local kill-path finding is still unfixed. Audit the current HEAD.

**Audited.** The older CPU-local finding has been fixed:

| Path | File:line | Behavior at current HEAD |
|---|---|---|
| `scheduler::kill_thread_by_pid` | `kernel/src/sched/scheduler.rs:1024-1058` | Iterates `PER_CPU.iter()` — full multi-CPU ready-queue scan. Returns on first match. |
| `scheduler::zombify_parked_thread_by_pid` | `kernel/src/sched/scheduler.rs:889-919` | Iterates `0..MAX_CPUS` — full multi-CPU parked-token-table scan. |
| `process::terminate` | `kernel/src/sched/process.rs:909-994` | After `kill_thread_by_pid` returns `false`, tries IPC-channel parked-side extraction, then `zombify_parked_thread_by_pid`. Three-step fallback covers Ready / IPC-Blocked / wake-token-parked states. |

The wake-token-abandon test scenario exercises the third path (wake-token-parked, not IPC-channel). All three multi-CPU scans are present and look correct. The 2026-05-16 finding is not the cause of the remaining tail.

## 2. Test scenario shape (`kernel/src/test_harness.rs:8453-8517`)

```
reset_wake_token_state()
register Process("wake-token-abandon", pmap) -> pid
Thread::new_kernel_for_process_test("wake-token-abandon", pid, wake_token_abandon_blocker)
scheduler::push_ready(worker)             <-- emits `[ kernel ] sched: enqueued user thread (id=N)`
wait_for_primary_token()                  <-- yield+reap up to 100K iters waiting for state.token
debug_token_occupied(token) -> must be true
process::terminate(pid)                   <-- expected to take the wake-token-parked fallback
debug_total_zombies() must increase
debug_token_occupied(token) -> must be false (slot freed by zombify)
wake_thread_token(token, 0xdead) -> must be Gone or Stale
reap loop up to 100K iters until zombies == 0
process::get(pid).is_none() -> must hold
event=PASS
```

Worker entry (`wake_token_abandon_blocker`, line 8442):

```rust
let ret = without_interrupts(|| {
    let guard = WAKE_TOKEN_STATE.lock();
    sched::scheduler::block_current_with_token_and_guard(guard, |state, token| {
        state.token = Some(token);
    })
});
fail_with_hex("reason=abandoned_wake_token_thread_resumed ret", ret as u64);
```

Worker is `extern "C" fn() -> !` — if `block_current_with_token_and_guard` returns, the test FAILs (because abandon should mean the thread is never resumed).

## 3. Failure-pattern correlation

Across loop-2 mixed-run, isolated 30x rerun, and earlier stale-fsync loops, every retry-pass or final-fail attempt-1 log shows the identical truncation:

```
NEO_TEST/1 event=BEGIN name=phase3_sched_wake_token_abandon phase=dispatch
[ kernel ] sched: enqueued user thread (id=7)
<truncated by 360s timeout>
```

Implications:

- The `event=BEGIN` is from `frame("BEGIN", ...)` in `run_selected_scenario` — fires before `run_phase3_sched_wake_token_abandon` does anything scenario-local.
- The `enqueued user thread (id=7)` is emitted at the END of `push_ready` (`scheduler.rs:267`), AFTER the lock is released, AFTER `without_interrupts` exits. Lock is freed; IF=1 when this prints.
- Nothing after this point ever reaches serial. No `wake_token_primary_not_installed` fail message (which would fire if the 100K-yield loop in `wait_for_primary_token` exhausted). No PASS frame. No FAIL frame.

This is **either**:
- the test-main thread is yielding repeatedly without making progress and the 100K bound has not been hit at the moment QEMU times out; OR
- the test-main thread has been switched off, never gets re-scheduled, and silently waits forever while QEMU sees no further serial output.

100K yields, even with timer preemption at 100 Hz, completes in well under 360s of wall time on this host — so a clean yield loop that doesn't make progress would have exhausted and printed the fail message by 360s. **The truncation pattern means the test-main thread is genuinely not running.**

## 4. Classification candidates (pre-reproduction)

Mapping the truncation pattern to dispatch §3's A-G table:

| Case | Plausibility |
|---|---|
| A (test ordering race: harness observes too early) | Low — the harness has not yet observed anything past `enqueued user thread`. |
| **B (ready-queue ownership race: target not parked when expected)** | **High** — the worker was just enqueued on BSP. If the BSP scheduler picks the idle thread instead of the worker, OR work-stealing migrates it to an AP whose scheduler is occupied, the worker never runs and the test-main thread (also blocked-on-yield in BSP queue) stays parked. |
| C (wake-token abandon bug) | Cannot fire — neither `process::terminate` nor `wake_thread_token` is reached on the failing path; both happen AFTER `wait_for_primary_token` returns. |
| D (wake-token wake bug) | Same — not reached. |
| **E (scheduler lock/livelock)** | **Possible** — if 4 CPUs converge on a `SwitchMutex` spin somewhere in the scheduler path, the pattern would look identical from serial. The stale-fsync lane's `__mm_pause` convergent-RIP signature lived in the same family. |
| F (panic/debug-exit path broken) | Low — kernel panics print frames before halt; we see no panic. |
| G (no local reproduction) | Counter-evidence: 5/30 retry-pass + 1/19 final-fail across 49 total local runs. Reproduces locally. |

Primary suspicion: **Case B or E**. They produce identical serial signatures; the discriminator is the per-CPU register dump on timeout — exactly what `NEO_QEMU_TIMEOUT_DUMP=1` will give us.

## 5. Durable substrate confirmed

Inheriting from the stale-fsync close, the xtask side already has:

- per-attempt serial logs (`<scenario>.<run-id>.attempt{1,2}.serial.log`)
- `NEO_TEST_RUN_ID` suffix for outer-loop archives
- `NEO_QEMU_TIMEOUT_DUMP=1` + `NEO_QEMU_MONITOR_PORT=<port>` → `info status` / `info cpus` / `info registers -a` on timeout

These are sufficient to distinguish Case B from Case E without any new instrumentation. If the per-CPU dump shows all 4 CPUs converging on `__mm_pause`, that's Case E. If only one CPU is in user/idle and three are in known idle paths, that's Case B (or further-refined sub-case).

## 6. Next move

Per dispatch §5: run a 50× reproduction loop with `NEO_QEMU_TIMEOUT_DUMP=1` + unique `NEO_TEST_RUN_ID`. Stop early on the first final timeout or three same-signature retry-passes. Use the resulting dump(s) to discriminate B vs E.

No instrumentation lands until the reproduction either fails to discriminate (then add scheduler trace frames per dispatch §4) or succeeds (then go to fix-or-substrate).

## 7. What this audit explicitly rules out

- Causes inside `process::terminate`, `wake_thread_token`, or `zombify_parked_thread_by_pid` — none are reached on the failing path.
- The 2026-05-16 CPU-local kill-path finding — already fixed at HEAD.
- The stale-fsync virtio deadlock — that lane is closed; current HEAD includes the IRQ-mask fix.
- Test-harness logic errors after `wait_for_primary_token` returns.

The bug, if a bug, lives between **`push_ready` returning** and **`wait_for_primary_token` returning** under SMP=4.
