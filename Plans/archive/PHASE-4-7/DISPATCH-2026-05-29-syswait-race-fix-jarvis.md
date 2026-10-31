# DISPATCH: sys_wait_multiple_waiters SMP race fix (process exit/reap/wait synchronization)

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Jarvis
**Branch:** `codex/syswait-race-fix-jarvis` (from origin/main HEAD)
**Type:** Kernel concurrency fix. Correctness-critical (Bible 1.2, 8.3). Real bug, pre-existing at Phase-3 close (verdict B, RETURN-2026-05-29-syswait-diagnostic-jarvis.md).

---

## What is broken

Two capability handles to the SAME process both call `sys_wait`. Intended contract = single-consumption reap: exactly one waiter consumes the exit and returns Ok(WAIT_MULTIPLE_CODE); every other waiter on any handle to that now-reaped process returns Err(ESRCH). Under NEO_SMP=4 this fails ~55% standalone:
- code=6: second waiter does NOT get ESRCH (double-consume / intermediate-state read).
- timeout: a waiter blocks forever (lost wakeup, or blocked on an already-dead target instead of returning ESRCH).

The test is correct. The kernel fails the invariant under SMP. Fix the kernel, not the test.

## Root cause

The wait path's check-then-decide (consume vs block vs ESRCH) is not serialized against the exit/reap state transition on another CPU. This is a memory-ordering + lock-discipline bug. NOTE (ADR-025): volatile is NOT the tool. Use correct atomic Ordering and the right lock type (ADR-019: IrqSafeMutex if the exit/reap/waitqueue state is mixed-IRQ-context).

## Invariants to establish (all three)

- **I1 single-consume:** exit-completion consumed exactly once. Consumer returns Ok(code); all others on any handle to that process return ESRCH. The unconsumed->consumed transition is atomic (CAS or under the state lock).
- **I2 no-block-on-dead-target:** a wait on an already-exited-and-consumed / gone target returns ESRCH immediately, never blocks. (Fixes the timeouts.)
- **I3 lost-wakeup-free:** for a not-yet-exited target, check-condition-then-enqueue-and-block is atomic w.r.t. exit/wake. The lock protecting exit/reap state must also serialize wait-enqueue against wake.

All three follow from serializing state-read + decision under the process-state lock that the exit path also takes, with Acquire/Release atomics and the correct lock type.

## Steps

### Step 0 - disambiguate code=6 (pre-flight, in this branch)
Before patching, instrument to confirm the sub-mode:
- In `userland/neo-init/src/main.rs` sys_wait_multiple_waiters: log the ACTUAL value returned by the second `sys_wait(second, 0)` (not just pass/fail).
- Add a temporary kernel klog at the exit-consume transition (who consumes, what the second waiter observes).
Run standalone NEO_SMP=4 a few times until you catch failures; record whether code=6 is double-success, wrong-errno, or block-then-late. Report it in the RETURN. Remove temporary instrumentation (or gate behind a debug cfg) before final commit.

### Step 1 - locate the machinery
Process exit/reap/wait lives in `kernel/src/sched/process.rs` (exit + handle/reap state) and the `sys_wait` syscall path, plus the wait-queue/parking primitive backing blocking waits. Identify: where exit-completion is recorded, where a waiter reads it, where it is consumed, where a waiter blocks/enqueues, where the waker wakes. Map every CPU-crossing edge.

### Step 2 - fix the synchronization
- Make unconsumed->consumed atomic (CAS or under the state lock). Exactly one consumer; losers see consumed -> ESRCH.
- Under the SAME lock that the exit path uses to flip state + wake: re-check the condition and enqueue-or-decide. No window between check and enqueue.
- Short-circuit dead/consumed targets to ESRCH before any block (I2).
- Use the correct lock type (ADR-019). If the state is touched from IRQ context or IF=0, it is IrqSafeMutex. Justify with a SAFETY/invariant comment.
- Acquire/Release pairing on any atomic flag carrying the exit state. No volatile, no relaxed-where-ordering-matters.

### Constraints
- Bible 15.2 (SAFETY comments on unsafe), 15.4 (no unlocked global mutable state). No unwrap/expect in steady-state paths.
- Do NOT change sys_wait ABI or the Ok/ESRCH semantics. If you believe a semantic change is required, STOP and surface it (BLOCKER) before implementing.
- Do NOT weaken the test or rely on retries. The scenario must pass on attempt 1.
- Keep the change minimal and local to the wait/reap/exit synchronization. No scheduler-wide refactor.
- Leave jira-dispatcher.py untouched.

## Verification bar (must meet ALL)

1. **Standalone stress:** `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo neo-test sys_wait_multiple_waiters` x50 -> 50/50 PASS, ZERO timeouts. (Bug was ~55% fail; 50 clean is the bar.)
2. **No SMP=1 regression:** `NEO_SMP=1 ... sys_wait_multiple_waiters` -> PASS.
3. **Sweep green honestly:** `NEO_SMP=4 cargo neo-test all` (default attempts) -> green, AND sys_wait_multiple_waiters passes on attempt 1 (grep the log to confirm no retry was used for it).
4. **Sibling check:** also run sys_wait_blocking and any other multi-waiter/wait scenarios x20 under NEO_SMP=4 to confirm the fix doesn't shift the race elsewhere. Report tallies.

## Deliverable

`Plans/RETURN-2026-05-29-syswait-race-fix-jarvis.md` with:
- Step 0 disambiguation finding (what code=6 actually was).
- Files changed + the synchronization approach (which lock, which atomics, how I1/I2/I3 are each guaranteed).
- All four verification results with tallies (the x50 especially).
- Confirmation no ABI/semantic/test changes.
- Commit hashes.