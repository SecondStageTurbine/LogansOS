# PM decision: sub-commit 2 has a separate latent SMP hang; isolate before restoring

**Date:** 2026-05-06
**Author:** PM
**Status:** Decision recorded; investigate-and-isolate dispatched to Cerebro
**Substrate:** Sub-commit 2 restore failed at run 81 of 100x ipc_heap_kill_smoke gate. Failure is **not** the §Q5 race. Working tree remains uncommitted.

---

## What happened (round 2)

Cerebro applied the §Q5 mechanism restoration per `Plans/DISPATCH-2026-05-06-build3-sub-commit-2-restore-cerebro.md`. The restoration is correct: `Thread.preempt_defer_ticks` is back, `wake_thread_front` re-sets the credit, `continue_current_after_receive` re-calls `defer_current_preempt_for_handoff`, the per-CPU forms of `consume_preempt_defer` / `clear_current_preempt_defer` / `defer_current_preempt_for_handoff` route through `current_scheduler().inner.lock()` correctly. The `cargo k` build is clean (18 warnings, baseline). Default suite passes 25/25 + meta-FAIL on both NEO_SMP=1 and NEO_SMP=4.

Then 100x `ipc_heap_kill_smoke` on NEO_SMP=4: 80/100 PASS, run 81 FAIL (xtask 180s timeout). Run 80 passed cleanly with the full IPC scenario (`sys_kill on Blocked returned EINVAL as expected`, `sys_kill on Ready returned 0`, `sys_kill on stale returned ESRCH`, leak audit clean, exit 0). Run 81 hung BEFORE reaching the IPC scenario, mid-stream during the in-kernel scheduler smoke test:

```
[ kernel ] sched: spawned 'ping' (id=4)
[ kernel ] sched: spawned 'pong' (id=5)
[ kernel ] sched: spawned 'hog' (id=6)
[ ping ] 1
[ pong ] 1
[ pong ] 2
[ ping ] 2
[ pong ] 3
[ ping ] 3
[ pong ] 4
[ ping ] 4
[ pong ] 5
[ ping ] 5
[ kernel ] sched: reaped 'pong' (id=5)
<HANG — no further serial output for 180s>
```

No panic. No fault. No leak audit (which only emits on clean exit). xtask QEMU timed out.

## Why this is NOT the §Q5 race

The §Q5 race had a clean signature: `[ neo-init ] BUG: sys_kill on Blocked did NOT return EINVAL`. It happened deterministically at the userland sys_kill assertion in `ipc_heap_kill_smoke`. It was an IPC-scenario semantic violation.

This hang has none of those properties:

- **It's in the in-kernel scheduler smoke test, not IPC.** ping/pong/hog/main are all kernel threads. The `[ ai ] sched: smoke test passed` line never appears, meaning we never even dispatched the IPC scenario.
- **All 80 prior runs passed the IPC sys_kill assertions.** The deferred-quantum mechanism is doing its job. The IPC semantic is resolved.
- **It's intermittent at ~1.25%, not deterministic.** A semantic violation would either always-fire or never-fire under fixed conditions. This is a race or scheduling-jitter sensitivity.
- **The serial output stops mid-stream with no error indication.** The kernel is hung in some way that produces no further log output for 180 seconds.

This is a separate bug. The §Q5 mechanism restoration is correct; this is not a "the per-CPU form of the mechanism has a subtle break" case (which the dispatch §6 explicitly anticipated).

## What we know about the new failure

After `[ kernel ] sched: reaped 'pong' (id=5)` is emitted, the next expected line in run 80's log is `[ kernel ] sched: reaped 'ping' (id=4)`. In run 81, that line never appears. The hang is somewhere between the println of "reaped pong" and "reaped ping".

Given the structure of `sched::reap()`'s for-loop (`println; Thread::free; on_thread_reap; loop iterates`), the hang is in one of:

1. **`Thread::free(pong_box)`** — calls `THREAD_CACHE.free()` which touches per-CPU magazine + (on overflow) global Mutex.
2. **The implicit `Box::drop` ordering** — `Thread::free` consumes the Box; the next iteration moves the next Box out of the local `VecDeque`. No locking required for that.
3. **A timer interrupt firing on main between iterations** — main's reap loop runs with interrupts ENABLED outside of `Thread::free`'s `without_interrupts` scope. If a timer fires, `on_tick` → `yield_now` → switches to whoever's at front of BSP ready queue. That thread (likely `hog`, possibly `ping`) runs.
4. **`process::on_thread_reap(KERNEL_PID)`** — early-returns for kernel pid; no work. Not a candidate.

The most likely shape (informed guess, not proven): **a timer fires on main during the for-loop. yield_now switches to ping (which has printed all 5 lines but not yet stored PING_DONE / called exit). Ping runs, but something about the per-CPU scheduler interaction causes ping to never reach `exit()`**. Or a different scheduling path: hog runs forever (HOG_TRIGGER not set yet), gets preempted, on_tick yields to ping, ping yields again somehow, hog runs again, etc. — in some configuration ping never reaches its exit path. Main spins on `while !(PING_DONE && PONG_DONE)` because PING_DONE remains false.

But this is a guess; the evidence does not pin which scheduler-state path is wedged.

## What is changed by sub-commit 2 that could produce this

Sub-commit 2 changes scheduler internals across the SMP boundary. Pre-sub-commit-2 (single-CPU global SCHED Mutex) demonstrably had no such hang — the same in-kernel smoke test ran in every test scenario for many BUILDs and was never observed to wedge. The SMP-specific changes that could plausibly introduce a 1-in-80 hang:

- **Per-CPU scheduler state accessed via GS-base.** `current_scheduler()` reads `per_cpu::current_cpu_id()` (a GS-relative load) then loads `PER_CPU[idx].scheduler_ptr`. If GS-base is ever stale or transiently wrong on the BSP path, BSP could mistakenly access an AP's scheduler. The BUILD-3 amendment fixed AP GS-base, but does not address whether some BSP path could clobber GS-base.
- **APs running busy-spin loops** that hammer their own per-CPU scheduler lock. Each AP loops `reap; yield_now; pause`. AP scheduler locks are uncontended (only that AP touches them) but represent ongoing memory traffic that pre-sub-commit-2 didn't have.
- **The `live_thread_count` AtomicU32 added to Process.** Touched on every thread exit / spawn. For kernel threads (KERNEL_PID), `on_thread_reap` early-returns BEFORE touching `live_thread_count`. So this is not a candidate for the in-kernel smoke test hang.
- **AP startup ordering** — APs come up before BSP scheduler init. APs immediately enter their bootstrap loop. By the time main runs, three APs are spinning on their schedulers. None of them holds a lock the BSP would block on.

The strongest a-priori candidate is "GS-base race that lets BSP transiently access an AP's scheduler." But timer ISR ring-0 path does not touch GS; nothing in the smoke test path takes a syscall (which would `swapgs`); SYSRET back from a syscall is also not in the path (no userspace runs in the smoke test). I cannot identify a concrete path where BSP's GS-base would shift, but the symptom is consistent with such a race.

## Decision

**Do not commit yet. Do not retry-the-100x-and-hope. Run a focused isolation experiment.**

Three things must be answered before we know what to do:

1. **Does the hang reproduce on NEO_SMP=1?** If yes, it's not SMP-specific — it's a sub-commit 2 scheduler-internals bug that happens to surface under load. If no, it's SMP-specific and we narrow further.
2. **Does it reproduce in the default test suite (which exercises every scenario, not just `ipc_heap_kill_smoke`)?** If yes, the in-kernel smoke test is the actual locus. If no, something about repeated `ipc_heap_kill_smoke` runs (e.g., kernel re-init under repeated boots) is implicated.
3. **What does adding minimal instrumentation reveal?** Specifically, an extra serial-print between every `Thread::free` and the next iteration's println would localize the hang to "before/after Thread::free returns".

A separate isolation dispatch will run these experiments. Sub-commit 2 stays uncommitted until we understand the failure.

## Why not just reduce the closure gate threshold

Tempting: 80/100 is "good enough", commit and move on. Don't.

- An intermittent 1-in-80 kernel hang is a kernel bug. We may understand its root cause and decide it's acceptable, but we don't currently understand it. Committing without understanding bakes the bug into main and makes every subsequent test run a roll of the dice.
- Phase 1.5b (and earlier) testing relied on the property that the in-kernel smoke test is deterministic. It runs ahead of every test scenario. If 1-in-80 boots hang there, every test scenario inherits a 1.25% flake rate — and the test runner has no way to distinguish "real test failure" from "kernel hung in smoke test". That corrupts test signal across the entire phase.
- Sub-commit 3a (block_current_into ManuallyDrop+asm release) is the next substantial work. It depends on a stable scheduler. Building 3a on a scheduler with a 1.25% mystery hang is asking for compounding pain.

## What sub-commit 2 closes once this is resolved

- **§Q3 (per-CPU schedulers)** — implemented and works for the IPC scenario at 80/80 plus the default suite at 25/25.
- **§Q5 (deferred-quantum)** — the bet failed; mechanism restored in per-CPU form; ADR-013 amendment captures the conclusion.
- **§Q9 (per-CPU reapers + live_thread_count)** — implemented.
- **AP GS-base rewire after GDT load** — implemented; ADR-013 amendment captures.

The sub-commit 2 close is delayed but not at risk. The work is largely correct; we have one residual hang to localize.

## What if isolation can't pin the bug

Two fallback paths:

- **Add the smoke-test hang to anti-scope and gate the commit on a documented "we know it happens, here's how to recognize it, here's what we know" disclosure in ADR-013.** Defensible only if we can characterize the hang well enough that future work can recognize when it bites. Risky.
- **Roll back sub-commit 2 partially** — keep the AP GS-base fix and the Q5 restoration as a tiny standalone commit, defer the per-CPU scheduler refactor to a redo with different shape. Heavy, but cleanly separates the AP-bringup-amendment work from the per-CPU-state work.

Default to neither yet. Run isolation first.

---

**End of decision note.**
