# PM decision: instrument-amplified hang confirms second latent SMP bug; partial rollback bisection is next

**Date:** 2026-05-06
**Author:** PM
**Status:** Decision recorded; partial-rollback bisection dispatched to Cerebro
**Substrate:** Experiments A, B, D, E, F, G; sub-commit 2 working tree; ADR-013

---

## What G told us

Experiment G instrumented `reap()`'s for-loop body with four `[ DBG ]` `serial_println!` calls bracketing each operation. Result: **9 consecutive PASS, then run 10 FAIL.** The instrumentation did NOT mask the bug; it AMPLIFIED it. Original config: ~80 consecutive PASS before first failure (~1.25% rate). With instrumentation: ~10 consecutive PASS (~10% rate or higher; we don't have full distribution data because Cerebro stopped on first failure as instructed).

The failing run-10 produced a different hang locus than the original run-81:

- **Run-81 (no instrumentation):** hung after `[ kernel ] sched: reaped 'pong'` — early in the in-kernel scheduler smoke test.
- **Run-G-10 (with instrumentation):** hung after `[ kernel ] proc: user thread exited with code 0` — late in the IPC scenario, after the entire IPC test has completed correctly and after worker has been reaped.

The shifted locus tells us the bug is not localized to a specific operation. It's a more general scheduler-state corruption that manifests at different points depending on timing.

## What the run-G-10 trace tells us

Reconstructed sequence from the trace:

1. neo-init completed all kill assertions correctly (`sys_kill on Blocked → EINVAL`, `sys_kill on Ready → 0`, `sys_kill on stale → ESRCH`).
2. Worker had been killed earlier; worker's thread sat in BSP's zombies queue.
3. BSP main's wait-loop in `spawn_neo_init` ran `reap()` and drained worker.
4. Inside `reap()`'s for-loop: `pre-Thread::free id=8` printed, `Thread::free(worker)` ran (which dropped the kernel stack via the global heap, releasing IF=1 between operations).
5. **Timer preempted BSP main during `Thread::free`'s heap-dealloc path.** Switch to neo-init.
6. neo-init's userspace continued, printed the ESRCH success line, returned from `run_ipc_heap_kill_smoke`, fell out of `_start`, called `sys_exit(0)`.
7. **Wait — that's wrong.** If neo-init reached sys_exit, the next print would be `[ kernel ] proc: user thread exited with code 0`. But that print appears AFTER `post-Thread::free` and `post-on_thread_reap`. So neo-init was preempted again before reaching sys_exit. Actual sequence:
   - Switch to neo-init. neo-init prints `sys_kill on stale returned ESRCH as expected`. Continues userspace.
   - **Timer preempts neo-init.** Switch back to BSP main.
   - BSP main prints `post-Thread::free id=8`. Calls `on_thread_reap(pid=2)` (worker pid). Returns.
   - BSP main prints `post-on_thread_reap id=8`. About to exit for-loop and print `loop end`.
   - **Timer preempts BSP main.** Switch to neo-init.
   - neo-init runs to sys_exit. `sys_exit_impl` prints `proc: user thread exited`, calls `sched::exit()`.
   - sched::exit moves neo-init to zombies, switches.
   - **HANG.**

The hang's locus: after `sched::exit()` from neo-init switches to whichever thread is at front of BSP's ready queue (presumably BSP main, which was preempted while inside `reap()`'s for-loop). That thread should resume, print `loop end`, return from reap(), continue the wait loop, eventually drain neo-init's zombie. **None of that happens.**

## What the bug isn't

- **Not the §Q5 deferred-quantum issue.** That's resolved; the IPC kill assertions all pass with mechanism restored.
- **Not a `cap_*` scenario issue.** Experiment F passed 100/100 with cap_mint_and_use, which exercises the same `spawn_neo_init` path.
- **Not an instrumentation artifact.** Sub-commit 2 hangs without instrumentation at run 81; instrumentation amplifies but does not create the bug.
- **Not in `Thread::free` itself, deterministically.** The `post-Thread::free` print appears in run-G-10's trace, so the slab path completed. The hang is later.
- **Not in `on_thread_reap` itself, deterministically.** The `post-on_thread_reap` print appears.
- **Not in IPC syscalls.** All IPC operations completed correctly.

## What the bug probably IS

A scheduler-state corruption introduced somewhere in sub-commit 2's per-CPU scheduler refactor, exposed by the interaction of:
- BSP main's wait-loop yielding repeatedly while user threads run.
- User-process exit path (`sched::exit()` from sys_exit).
- Per-CPU scheduler's `current` / `ready` / `zombies` field access via `current_scheduler().inner.lock()`.

Possible specific failure modes (none confirmed, all hypotheses):

- **Per-CPU scheduler pointer corruption.** If `PER_CPU[0].scheduler_ptr` is somehow read as null or wrong-CPU after a context switch, `current_scheduler()` would panic-return or return another CPU's state.
- **`live_thread_count` racing with thread-spawn paths.** Phase 1.5b assumed one thread per process; sub-commit 2 added `live_thread_count` for forward-compat. If the count gets out of sync (e.g., decremented twice for a single thread, or decremented before a corresponding spawn was registered), `on_thread_reap` could remove a process while threads still exist, OR fail to remove a process that should be cleaned up.
- **Per-CPU reaper queue corruption.** If a thread that should be on BSP's zombies queue ends up on an AP's queue (or vice-versa), it never gets reaped.
- **Some interaction between `block_current_into`'s lock-order workaround and the per-CPU shape.** ADR-013 §Q6 anticipated this needs the ManuallyDrop+asm release pattern in sub-commit 3a; sub-commit 2 deferred it. It's possible the deferral allowed a race we hadn't anticipated.

The instrument-amplification result is consistent with all of these — extra serial calls increase total time spent in `reap()` and surrounding paths, increasing the timing window for whatever race exists.

## Decision

**Partial rollback bisection.** Sub-commit 2 has three independent components:

1. **AP GS-base rewire after GDT load** (BUILD-3 amendment). Small, isolated, fixes a real AP boot issue.
2. **§Q3 — per-CPU schedulers.** The big refactor. Replaces global `Scheduler` with `PerCpuScheduler` per CPU.
3. **§Q9 — per-CPU reapers + `live_thread_count`.** Distributes reaper work to per-CPU bootstrap threads; adds atomic counter to Process.

Plus the §Q5 mechanism restoration on top of all of these.

Bisection plan:

- **Step 1: Revert §Q9 only** (keep §Q3 and §Q5-restore and AP-GS-base). Run the 100x gate. If hang persists, §Q9 is not the cause; restore it and proceed to step 2. If hang disappears, §Q9 is the (or a) cause; investigate live_thread_count and per-CPU reaper distribution.
- **Step 2: Revert §Q3 too** (keep only §Q5-restore and AP-GS-base, return to single global `SCHED` Mutex). Run the 100x gate. If hang persists, the cause is in §Q5-restore or AP-GS-base — much smaller surface. If hang disappears, §Q3 is the cause.

This is more disruptive than instrumentation but produces a different kind of answer: **which subsystem's change is causing the hang**. With that answer, fixing or redesigning becomes tractable.

The §Q5 lesson keeps applying: when in doubt, narrow before fixing.

## Why not other options

- **Continue instrumented debugging.** G has already amplified the bug 8x. More instrumentation could amplify further or shift the locus again. Pinning a specific operation requires instrumenting half the kernel; we'd be playing whack-a-mole with the bug's manifestation rather than its cause.
- **Full rollback of sub-commit 2; redo with smaller scope.** Throws away substantial work that might be salvageable. We don't yet know whether ALL of sub-commit 2 is broken or just one piece.
- **Constrain to SMP=1 only.** Defers Phase 2's "scheduler is per-CPU" goal entirely. Sub-commit 3+ depends on per-CPU; deferring that to "later" is a euphemism for "never." Discipline violation.
- **Lower the closure gate threshold and commit anyway.** 9-out-of-10 PASS rate is much worse than the "intermittent at 1.25%" we previously rejected. Committing a known kernel hang into Phase 2 corrupts test signal for every subsequent BUILD.

## Closure gate stays the same

100x ipc_heap_kill_smoke at NEO_SMP=4 with zero instrumentation, 100/100 PASS. Lowering the gate is not on the table. Either we find the bug and fix it, or we partial-rollback enough to remove it.

## What sub-commit 2 closes once this is resolved

Same as the previous decision note. The shape is intact:

- §Q3 (or its replacement) — per-CPU scheduler container.
- §Q5 (mechanism retained, in per-CPU form) — deferred-quantum.
- §Q9 (or its replacement) — per-CPU reapers, optional `live_thread_count`.
- AP GS-base rewire amendment.

If the bisection identifies §Q9 as the culprit, we keep §Q3+Q5-restore+AP-GS-base and defer §Q9 to a future sub-commit with cleaner design. If §Q3 is the culprit, harder — we need to redesign the per-CPU container shape. We'll cross that bridge when we know.

## Time budget

Expect 1–2 days of partial-rollback work plus 100x gate runs per bisection step. Slower than continuing instrumentation, but the answer is more decisive.

---

**End of decision note.**
