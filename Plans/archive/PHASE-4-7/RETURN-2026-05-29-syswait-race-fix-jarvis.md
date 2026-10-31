# RETURN: sys_wait_multiple_waiters SMP race fix

**Date:** 2026-05-29  
**From:** Jarvis  
**To:** PM (Cerebro)  
**Branch:** `codex/syswait-race-fix-jarvis`  
**Base:** `origin/main` `383344578a25890b2f0b79e0612b0cd36703857b`  
**Implementation commit:** `c0a3b04` (`fix sys_wait exit completion race`)

## Step 0 disambiguation

Temporary diagnostics were added before the fix and removed before commit.

Baseline failed on the third standalone `NEO_SMP=4`, `NEO_TEST_ATTEMPTS=1` run. The failing serial log showed:

```text
[ kernel ] syswait-debug complete pid=2 exit_code=Some(43) waiters=0
[ kernel ] syswait-debug store-completed pid=2 code=43
[ neo-init ] sys_wait_multiple_waiters second=Err(-11)
NEO_TEST/1 event=FAIL name=sys_wait_multiple_waiters reason=process_cap_scenario_exit_nonzero code=6
```

Finding: code `6` was not duplicate success. It was wrong errno `Err(-11)` / `EAGAIN` from the second derived handle after the first wait consumed the completion.

Root cause confirmed: `ProcessWaitQueue::take_all()` used `core::mem::take(&mut self.waiters)`, which left the live wait queue with capacity `0`. During the natural-exit window after the completion was stored but before process removal, a late second waiter could observe no completed global code, see the per-process queue as full (`0 >= 0`), and return `EAGAIN` instead of `ESRCH`.

## Files changed

- `kernel/src/sched/process.rs`

No userland, syscall ABI, test, or `jira-dispatcher.py` changes remain.

## Synchronization approach

The fix makes the per-process wait queue hold both parked waiters and the one-shot wait completion state under one lock:

- `Process.waiters` is now `IrqSafeMutex<ProcessWaitQueue>`.
- `ProcessWaitQueue` now tracks `ProcessWaitCompletion::{Running, ExitedUnconsumed(code), Consumed}`.
- `wait_for_exit_abi()` no longer removes from `COMPLETED_EXIT_CODES` before checking whether the process still exists. Existing processes decide under `waiters`; absent processes fall back to the global completed-exit map.
- `complete_process_waiters()` drains parked waiters without replacing the queue Vec, preserving reserved capacity.
- Natural exit with no parked waiters records `ExitedUnconsumed(code)` under the process wait lock and mirrors the code into `COMPLETED_EXIT_CODES` for the post-removal path.
- Natural exit with parked waiters marks completion `Consumed` before waking exactly one waiter with `Ok(code)` and all others with `ESRCH`.
- Forced kill marks completion `Consumed` and wakes all waiters with `ESRCH`.

Invariant coverage:

- I1 single-consume: unconsumed-to-consumed is serialized by `Process.waiters`; global completed code is removed while holding that same lock when consumed through the still-live process.
- I2 no-block-on-dead-target: `Consumed` returns `ESRCH` immediately, and non-natural exited states return `ESRCH` before any enqueue.
- I3 lost-wakeup-free: condition check, completion consume/ESRCH decision, and wait enqueue all occur while holding `Process.waiters`; exit completion and waiter drain use the same lock.

Atomic ordering remains Acquire/Release on `exit_state` and `exit_code`; no volatile was added.

## Verification

Host PowerShell does not have `cargo` on PATH in this environment, so verification used the repo's Docker workflow, running the requested `cargo neo-test ...` commands inside the `neo` container.

1. Standalone stress:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test sys_wait_multiple_waiters --no-meta
```

Repeated 50 times. Result: **50/50 PASS, 0 FAIL, 0 timeouts**. Summary saved at `target/neo-test/sys_wait_multiple_waiters.smp4.stress.summary.txt`.

2. SMP=1 regression:

```powershell
docker compose run --rm -e NEO_SMP=1 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test sys_wait_multiple_waiters --no-meta
```

Result: **PASS**. Log saved at `target/neo-test/sys_wait_multiple_waiters.smp1.log`.

3. Full sweep:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all --no-meta
```

Result: **PASS**, exit `0`. Log saved at `target/neo-test/all.smp4.syswait-fix.log`.

Retry scan for `sys_wait_multiple_waiters`:

```text
[xtask] neo-test: launching QEMU for `sys_wait_multiple_waiters` (serial: /workspace/target/neo-test/sys_wait_multiple_waiters.attempt1.serial.log)
[xtask] neo-test: `sys_wait_multiple_waiters` PASS
```

No retry/failure line was present for `sys_wait_multiple_waiters`; it passed on attempt 1.

4. Sibling wait scenarios under `NEO_SMP=4`, `NEO_TEST_ATTEMPTS=1`:

```text
sys_wait_blocking pass=20 fail=0
sys_wait_wnohang pass=20 fail=0
sys_wait_already_exited pass=20 fail=0
```

Summary saved at `target/neo-test/sys_wait_siblings.smp4.summary.txt`.

Additional hygiene:

```powershell
git diff --check
```

Result: **clean**.

`docker compose run --rm neo cargo fmt --check` was attempted, but the repository is not currently rustfmt-clean outside this change and rustfmt reported broad pre-existing formatting drift across unrelated files.

## Confirmation

- No syscall ABI change.
- No `Ok(code)` / `ESRCH` semantic change.
- No test weakening or retry reliance.
- No volatile.
- `jira-dispatcher.py` untouched.
