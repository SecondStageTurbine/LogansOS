# PM-DISPOSITION: sys_wait_multiple_waiters verdict (B) consumed — fix direction

**Date:** 2026-05-29
**From:** PM (Cerebro)
**Reference:** Plans/RETURN-2026-05-29-syswait-diagnostic-jarvis.md

---

## Verdict consumed: (B) intrinsic SMP race, PRE-EXISTING at Phase-3 close. Real bug.

- Exp A (current main, standalone NEO_SMP=4 x20): 9/20 PASS, 11/20 FAIL (9 code=35/code=6, 2 timeouts). Reproduces standalone -> not sweep-only.
- Exp B (sweep, default 4 attempts): green, retry-maskable. We do NOT rely on retries to hide a real race.
- Exp C: code=6 = second derived waiter did not get ESRCH after the first waiter consumed the exit. Scenario asserts single-consumption reap (first waiter Ok(WAIT_MULTIPLE_CODE); any second waiter on a handle to the same now-reaped process -> Err(ESRCH)).
- Exp D (8b5e7f6, Phase-3 close, standalone x20): 13/20 PASS, 7/20 FAIL, same signature. PRE-EXISTING. NOT introduced by Phase 4 or the perf-proxy fix.

The test is correct; the contract (single-consumption reap) is a legitimate kernel invariant. The kernel fails to uphold it under SMP. Fix goes in the kernel.

## Root cause (PM analysis)

Two observed failure shapes, one root:
- code=6 (wrong result): the exit-completion "exited-unconsumed -> consumed" transition is not atomic w.r.t. a concurrent waiter -> double-consume (both get a success-ish result) or an intermediate-state read.
- timeout (hang): a waiter blocks forever -> lost wakeup (commits to block in the window between condition-check and wait-queue enqueue; the waker flips state + wakes before the waiter is enqueued), OR it blocked on an already-reaped target that should have short-circuited to ESRCH.

Both collapse to: the wait path's check-then-decide (consume vs block vs ESRCH) is NOT serialized against the exit/reap state transition happening on another CPU. Classic condition-variable discipline violated under SMP.

This is an atomics + lock-ordering problem. Per ADR-025 (ratified 2026-05-29), volatile is explicitly NOT the tool; the tools are correct atomic Ordering and the right lock type (ADR-019 IrqSafeMutex if the exit/reap/waitqueue state is mixed-IRQ-context).

## Invariants the fix must establish

- **I1 single-consume:** the process exit-completion is consumed exactly once. The consuming waiter returns Ok(code); all other concurrent/subsequent waiters on any handle to that process return ESRCH. The unconsumed->consumed transition is atomic (CAS or under the state lock).
- **I2 no-block-on-dead-target:** a wait whose target is already exited-and-consumed (or otherwise gone) returns ESRCH immediately and never blocks. (Directly addresses the timeouts.)
- **I3 lost-wakeup-free:** for a not-yet-exited target, check-condition-then-enqueue-and-block is atomic w.r.t. the exit/wake. The lock that protects the exit/reap state also serializes wait-enqueue against wake, so a waiter that decides to block is guaranteed to be woken.

All three follow from one disciplined change: serialize state-read + decision under the process-state lock that the exit path also takes, with correct lock type (ADR-019) and Acquire/Release atomics. No volatile.

## Decision

- Fix dispatched to Jarvis (codex/syswait-race-fix-jarvis). Real kernel concurrency fix in the process exit/reap/wait path.
- Pre-flight disambiguation folded into the fix branch: instrument the code=6 sub-mode (log actual sys_wait return + a klog at the consume transition) to confirm double-consume vs wrong-errno vs block-then-late before patching. Cheap; sharpens the patch. Not a separate dispatch.
- Verification bar is the stress that exposed it: standalone NEO_SMP=4 x50 must be 50/50 PASS with ZERO timeouts (bug was ~55% fail, so 50 clean runs is strong evidence); NEO_SMP=1 still PASS; full sweep green at default attempts with sys_wait_multiple_waiters passing on attempt 1 (no retry reliance).
- BUILD-3 remains held until this lands and the SMP=4 sweep is green WITHOUT retry-masking of this scenario.

## Notes

- This is a correctness fix to a pre-existing latent bug -> operational/technical, PM owns it. PL informed; no scope/ABI/bible change. If disambiguation reveals a needed sys_wait semantic/ABI change (not expected; Ok-vs-ESRCH is already defined), Jarvis surfaces before implementing.
- Good news from Exp D: this is decoupled from all Phase 4 work. It is a Phase 2/3 wait-machinery hardening item that the SMP=4 sweep simply never reached before the perf-proxy gate let it through.