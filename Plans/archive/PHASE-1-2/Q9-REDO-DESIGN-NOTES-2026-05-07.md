# §Q9 redo design notes

**Date:** 2026-05-07
**Status:** DRAFT research artifact — not normative. Future input to ADR-016 or successor.
**Author:** Jarvis
**Substrate:** ADR-013 §Q9 was deferred at sub-commit 2 closure (see the stashed ADR-013 amendments in `stash@{0}:docs/ADR-013-phase-2-smp-retrofit.md`). These notes capture what a future §Q9 redo must address.

---

## 1. Why §Q9 was deferred

ADR-013 §Q9 specified per-CPU reapers: each CPU's bootstrap thread drains its own `PerCpuScheduler.zombies` queue, with no cross-CPU zombie traffic. The design paired this with a `live_thread_count: AtomicU32` field on `Process`, enabling multi-thread-per-process readiness — when the last thread of a process is reaped, the Process is deregistered.

Sub-commit 2's first closure attempt included both Q9's per-CPU reaper distribution and the `live_thread_count` gate. The `NEO_SMP=4` 100x `ipc_heap_kill_smoke` gate failed at run 81 with a kernel hang in `sched::smoke_test`'s `reap()` path after `[ kernel ] sched: reaped 'pong' (id=5)`. Adjacent runs passed cleanly; the observed failure rate was approximately 1.25%.

A non-mutating bisection narrowed the trigger across eight experiments (A through H):

- **A:** `NEO_SMP=1` 100x `ipc_heap_kill_smoke` — 100/100 PASS. Hang requires SMP.
- **B:** `NEO_SMP=4` bare boot (no test harness) — 100/100 PASS. Hang requires the test-harness kernel binary.
- **D:** Bare-boot kernel under test-harness QEMU flags — 100/100 PASS. QEMU flags alone are not the trigger.
- **E:** `kaslr_smoke` under `NEO_SMP=4` — 100/100 PASS. Generic `mod test_harness` presence is not sufficient.
- **F:** `cap_mint_and_use` under `NEO_SMP=4` — 100/100 PASS. Generic `spawn_neo_init` presence is not sufficient.
- **G:** Instrumented `reap()` on `ipc_heap_kill_smoke` — failed at run 10, amplifying the hang rather than cleanly localizing it. Last `[ DBG ]` line: `post-on_thread_reap id=8`, with no loop-end line following.
- **H:** Sub-commit 2 minus Q9 (BSP scans all CPUs, APs don't reap) — 100/100 PASS under `NEO_SMP=4`. Q9 implicated.

Q9 was deferred. Sub-commit 2 keeps Q3's per-CPU scheduler containers but uses BSP-only reaping: the BSP `reap()` scans all initialized per-CPU schedulers, drains each zombie queue under that scheduler's lock, and processes zombies locally. APs do not participate in reaping.

This interim shape is acceptable because no production thread is routed to APs yet; AP zombie queues are empty in the steady state, so the BSP cross-CPU scan is a bounded no-op beyond checking initialized scheduler pointers. Any later sub-commit that introduces cross-CPU thread movement (sub-commit 3b's wake-on-completer-CPU) must revisit reaper distribution.

---

## 2. The leading hypothesis (informational)

The precise Q9 failure mechanism was not investigated after EXP-H passed. Drilling further would have required additional bisection rounds without changing the closure decision. The leading hypothesis, named in the stashed ADR-013 amendment, is:

Q9's modified `on_thread_reap` decremented `proc.live_thread_count` with `fetch_sub(1, AcqRel)` and returned early when the previous count was not 1. In Phase 1.5b's one-thread-per-process model, that previous count should always be 1. If it is ever 0 or greater than 1, process cleanup is skipped: the process remains in `PROCESSES`, `any_user_process()` keeps returning true, and the kernel can wait forever.

The stashed amendment named four recommendations for a future Q9 replacement:

1. **Initialize counter before threads become runnable.**
2. **Verify exactly-once decrement semantics under SMP.**
3. **Avoid AP-side reaper participation until threads can actually route to APs.**
4. **Use `SeqCst` until weaker ordering is proven.**
5. **Add a regression test that exercises the counter under real multi-thread-per-process load.**

Each recommendation is analyzed below in §3.

---

## 3. The shape of the redo

### 3.1 Initialize counter before threads become runnable

In NEO's spawn flow, a user process is created by `spawn_user()` in `process.rs`. This function:
1. Parses the ELF, builds a Pmap, loads segments.
2. Allocates a user stack at `USER_STACK_TOP`.
3. Constructs a `Process::new_user(name, pmap)`.
4. Registers the Process into `PROCESSES` via `register()`, which wraps it in `Arc<Process>`.
5. Creates a `Thread::new_user(name, pid, entry, initial_user_rsp)`.
6. Pushes the thread onto the scheduler ready queue via `scheduler::push_ready(thread)`.

The counter must be initialized at step 3 (Process construction), before step 6 makes the thread schedulable. Specifically, `Process::new_user()` should set `live_thread_count` to 1 (for the single root thread that will be created immediately after registration). The increment happens atomically:

```
// In Process::new_user():
live_thread_count: AtomicU32::new(1),
```

The spawn flow then creates the thread and pushes it ready. At no point between counter initialization (step 3) and the thread becoming runnable (step 6) can a reaper observe this process, because `without_interrupts()` wraps the entire `spawn_user()` body — interrupts are disabled, so no PIT tick can fire, and no other CPU's scheduler can preempt.

**Failure mode if counter is initialized after the thread is enqueued:** Between enqueue and counter initialization, a concurrent reaper on another CPU could observe `live_thread_count == 0` for a process that has a live (or about-to-be-live) thread. The reaper would attempt premature deregistration, racing with the spawn's subsequent counter increment. In the one-thread-per-process model this is particularly dangerous because the reaper would find count 0 and proceed to remove the Process from `PROCESSES` while `spawn_user()` is still constructing it.

**Alternative — increment at thread creation time:** Instead of initializing to 1 in `Process::new_user()`, initialize to 0 and increment in `Thread::new_user()`. This is structurally cleaner (counter reflects actual thread count) but requires the increment to happen before `push_ready()`. The current code already does this — `Thread::new_user()` is called, then `push_ready()` — so either approach works. The key invariant is: counter must reflect the thread's existence before any other CPU can observe the thread as runnable or zombie.

### 3.2 Verify exactly-once decrement semantics under SMP

The `live_thread_count` decrement happens in `on_thread_reap()`, which is called from `reap()` for each zombie thread. Under per-CPU reapers, multiple CPUs can call `on_thread_reap()` concurrently for threads belonging to the same process. The decrement must fire exactly once per thread that exits.

**What "exactly-once" means here:** Each `Box<Thread>` in a zombie queue represents one thread that has called `exit()`. The reaper iterates the drained zombies and calls `on_thread_reap(pid)` once per zombie. If a Thread can appear in two different CPUs' zombie queues simultaneously, or if a Thread's reap can be triggered twice (once by the local reaper, once by a cross-CPU scan), the counter decrements more than once and underflows.

**Can the same Thread be reaped twice?** Under the current BSP-scans-all-CPUs model, no: the BSP drains each CPU's zombie queue under that CPU's lock, and `mem::take()` empties the queue atomically. A Thread is in exactly one zombie queue at a time. Under per-CPU reapers with AP participation, the same invariant holds — each CPU drains its own queue independently. The risk is not double-reaping of the same Thread; it's concurrent decrements for different Threads of the same process.

**Concurrent decrements on the same process:** Consider a multi-threaded process where thread A exits on CPU0 and thread B exits on CPU1. Both CPUs' reapers call `on_thread_reap(pid)` concurrently. The `fetch_sub(1, AcqRel)` is atomic, so the counter itself doesn't corrupt. But the conditional — "if new_count == 0, deregister" — can fire on both CPUs if the timing aligns:

1. CPU0 decrements from 2 to 1 (new_count = 1, no deregistration).
2. CPU1 decrements from 1 to 0 (new_count = 0, proceeds to deregister).
3. CPU0's `on_thread_reap` has already returned; no race.

This is actually safe because the atomic decrement ensures exactly one CPU sees the transition to 0. The double-check under the write lock in ADR-013 §Q9's pseudocode handles the remaining edge case: between the decrement and the lock acquisition, a new thread could be spawned for this process, incrementing the count back to non-zero.

However, this local safety only holds *given the count is correct on entry*. The leading hypothesis from the ADR-013 amendment is not "two CPUs both saw 0" — it is that the count was already corrupt (0 or greater than expected) by the time the decrement ran. In that case, the decrement's outcome is nonsensical regardless of its atomicity: a `fetch_sub` on a counter that started at 0 wraps to `u32::MAX`, and the early-return guard in the original Q9 code (`return` when previous count != 1) silently skips process cleanup. The bug was upstream of the decrement — in initialization ordering or counting correctness — not in the decrement's atomicity. §3.1's "initialize before threads become runnable" addresses the initialization-order half; §3.2's exactly-once semantics address the counting-correctness half. Neither is sufficient alone: correct initialization ensures the counter starts right, and exactly-once decrement ensures it stays right through exit.

**Invariant on zombie queue contents:** Each zombie queue should contain only Threads that have called `exit()` and been moved to Zombie state by `rotate(ThreadState::Zombie, WhichQueue::Zombie)`. No Thread can be in two zombie queues simultaneously because `rotate()` moves the `Box<Thread>` from `current` into exactly one queue under exactly one lock. The invariant is: each live or zombie Thread belongs to exactly one CPU's scheduler at any given moment.

### 3.3 Avoid AP-side reaper participation until threads can route to APs

Sub-commit 2 ships with BSP-scans-all-CPUs reap and no AP-side reaping. Threads are only placed on the BSP's scheduler (the BSP is the only CPU running user threads). AP zombie queues are empty in the steady state, so the BSP's cross-CPU scan is a bounded no-op beyond checking initialized scheduler pointers.

**When does AP-side reaping become necessary?** Two conditions must both be true:
1. Threads are actually routed to APs (sub-commit 3b's wake-on-completer-CPU under Q4).
2. Those threads can exit on the AP, producing zombies in the AP's local zombie queue.

Under Q4's wake-on-completer-CPU, a thread blocked on IPC on CPU1 can be migrated to CPU0 when the rendezvous completes. If that thread then calls `exit()`, it dies on CPU0 — its zombie goes into CPU0's queue. The AP (CPU1) has no zombies to reap.

However, a thread that was spawned on an AP (via a future multi-thread-per-process spawn that places threads on specific CPUs) and exits without ever migrating will produce a zombie on the AP. At that point, AP-side reaping becomes structurally necessary. This is a Phase 2+ concern; BUILD-3's sub-commit 3b enables cross-CPU IPC but does not enable spawning threads directly onto APs.

**Simplest mechanism for safe AP-side reaping:** Each CPU's bootstrap thread runs a periodic reaper loop (as ADR-013 §Q9 originally specified), but only drains its own local zombie queue. The bootstrap thread is the per-AP equivalent of the BSP's "main" thread — it exists, it has nothing else to do, and the reaper loop is its purpose. The key difference from the original Q9 design: AP-side reaping should not be enabled until a sub-commit that actually routes threads to APs. Until then, AP reapers are compiled but dormant (their queues stay empty).

The reaper follows the thread's final location, not its origin. A thread spawned on CPU0, migrated to CPU1 via §Q4 wake-on-completer-CPU, and exited on CPU1 produces a zombie in CPU1's queue and is reaped by CPU1's reaper. This is implicit in the current `rotate(ThreadState::Zombie, WhichQueue::Zombie)` semantics — the `Box<Thread>` moves into `s.zombies` of the scheduler that owned it at exit time — and the redo preserves it.

This staged enablement matches the discipline rule from ADR-013 §Q2: "APs do not call into any subsystem they haven't been explicitly cleared for." Each capability is enabled in order, so a bug in one doesn't propagate.

### 3.4 Use `SeqCst` until weaker ordering is proven

The original Q9 design used `fetch_sub(1, Ordering::AcqRel)` for the `live_thread_count` decrement. The stashed amendment recommends `SeqCst` instead.

**Why `SeqCst` over `AcqRel`:** The `live_thread_count` operation is not a standalone atomic; it's part of a larger critical section that includes:
1. Taking the zombie out of the queue (under scheduler lock, interrupts disabled).
2. Dropping the `Box<Thread>` via `Thread::free()` (heap allocation + kernel stack deallocation).
3. Decrementing `live_thread_count`.
4. Conditionally acquiring the `PROCESSES` write lock and deregistering.

The hazard is not in the atomic itself — `AcqRel` on a single `fetch_sub` is correct for that operation. The hazard emerges in the multi-thread-per-process case: consider a Process with thread T1 alive. T1 (or another thread of the same process) calls a future `sys_thread_create` to spawn T2, then T1 exits. T2's increment of `live_thread_count` must happen-before T1's reaper observes the count as 0 and proceeds to deregister the Process. Under `AcqRel`, there is no total order guarantee across CPUs; T1's reaper could observe the decrement-to-0 before T2's increment becomes visible, leading to premature deregistration of a process that still has T2 alive.

Phase 1.5b does not have `sys_thread_create`; this hazard becomes real only when multi-thread-per-process spawn lands (post-Phase 1.5). It is one of the reasons §Q9 cannot be fixed in isolation from the multi-thread-per-process roadmap — the test that would catch it requires a primitive that does not exist yet. The §Q9 redo should be co-scheduled with `sys_thread_create` design, not landed standalone before multi-thread support exists.

`SeqCst` provides a single total order on all sequentially-consistent operations. If both the increment (in `sys_thread_create`) and the decrement (in `on_thread_reap`) use `SeqCst`, they appear in a globally consistent order: either the increment happened-before the decrement, or vice versa. The double-check under the write lock then handles whichever ordering occurred.

**Memory-ordering analysis before downgrading:** To safely use `AcqRel` (or weaker), the redo would need to prove that:
1. The increment in `spawn_user()` happens-before any reaper can observe the new thread (guaranteed by `without_interrupts()` wrapping the spawn, plus the scheduler lock acquisition in `push_ready()`).
2. The decrement-to-0 observation and the subsequent `PROCESSES` write lock acquisition form a sufficient happens-before chain to prevent observing stale counter values.

This analysis is non-trivial at Phase 2 scale. `SeqCst` costs ~1 cycle on x86_64 (which has a strong memory model); it's free for correctness. Downgrade only if profiling shows the reaper path is measurably hot, which it won't be at BUILD-3 scale.

### 3.5 Add regression test for multi-thread-per-process load

The original Q9 test was inadequate because it ran in a one-thread-per-process kernel. The `live_thread_count` race only manifests when multiple threads of the same process can exit concurrently on different CPUs. Phase 1.5b's single-thread-per-process model means `live_thread_count` always transitions from 1 to 0, and only one reaper (BSP) ever calls `on_thread_reap()` for a given pid.

**Test shape:** Spawn a user process with N threads (N ≥ 4), each running on a different CPU under `-smp 4`. The threads exit concurrently (e.g., all call `sys_exit(0)` within the same tick window). Verify:
1. Each thread is moved to Zombie state on its respective CPU.
2. Each CPU's reaper drains its local zombie queue.
3. `live_thread_count` decrements from N to 0 exactly once per thread.
4. The Process is deregistered exactly once (one `on_thread_reap` fires the deregistration; the others see non-zero counts and return).
5. No double-deregistration, no premature deregistration, no process remaining in `PROCESSES` after all threads are reaped.

**Why the original Q9 test was inadequate:** The bisection evidence shows that the hang occurred specifically with `ipc_heap_kill_smoke` on `NEO_SMP=4`. This scenario involves a kill message sent via IPC, which triggers the forced-kill path (`terminate()`) rather than the natural-exit path (`on_thread_reap()`). The interaction between `terminate()`'s registry removal and `on_thread_reap()`'s counter-based deregistration is where the race likely lives. A proper regression test must exercise both paths concurrently: natural exit on one CPU, forced kill on another, for threads of the same process.

**Test naming:** `smp_concurrent_process_exit` — this name already appears in ADR-013 §Q9's test scenarios list as a sub-commit 5 item. The redo should implement it exactly as specified there: "multi-threaded user process, threads exit concurrently on different CPUs; verify Process is deregistered exactly once."

---

## 4. Open questions for ADR-016

1. **Should `live_thread_count` be an `AtomicU32` or an `AtomicIsize`?** The unsigned type underflows if decremented below 0 (wrapping to `u32::MAX`). A signed type would produce a negative value, which is easier to detect in debug builds. What's the right failure mode for a counter that goes wrong?

2. **Can `terminate()` and `on_thread_reap()` race on the same process?** `terminate()` removes the Process from `PROCESSES` before calling `close_channel_peers()`. `on_thread_reap()` looks up the Process in `PROCESSES` before decrementing. If a thread is killed via `sys_kill` while its natural-exit reaper is mid-decrement, can both paths attempt deregistration? The current code handles this via the lookup-before-act pattern in `on_thread_reap()`, but does it hold under per-CPU reapers with concurrent access?

3. **What happens when a thread exits on an AP but its Process was spawned on the BSP?** The counter decrement is local to the reaper, but the Process's `Arc` is global. Does the cross-CPU access to `live_thread_count` (stored inside the `Arc<Process>`) introduce false-sharing or cache-line bouncing that makes the reaper path measurably hot?

4. **Should the reaper loop on APs be periodic or event-driven?** ADR-013 §Q9 specified "the bootstrap thread runs its own reaper loop." Is this a tight loop with `while let Some(zombie) = drain_one()`, or a tick-driven callback? The BSP's current reap is called between yields; should APs follow the same pattern?

5. **Should `live_thread_count` be part of `Process` or a separate per-pid structure?** Storing it inside `Process` couples the counter to the Process's memory layout and cache line. A separate structure avoids false-sharing with other Process fields but adds indirection. At Phase 2 scale (small number of processes), does this matter?

6. **What is the interaction between Q9's reaper distribution and sub-commit 3a's ManuallyDrop + asm release?** Partially answered in `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` §3.4–§3.5 for the *current* (BSP-only reap) architecture: the pre-thinking concludes that SC 3a's asm release composes correctly with sub-commit 2's BSP-scans-all-CPUs reaper because (a) the reaper holds no lock while waiting for a target CPU's scheduler lock, (b) the asm release order (Channel-first-then-scheduler) ensures cross-CPU wakers proceed promptly, and (c) `sched.zombies` (what the reaper reads) is independent of `sched.current` (which is what `block_current_into` mutates). Open question for the §Q9 redo: how does this analysis change when AP-side reaping re-enables (per §3.3 above)? In particular, does an AP-side reaper holding its own scheduler lock interact differently with cross-CPU wake-on-completer-CPU rendezvous than the BSP reaper does?

7. **Can the reaper's call to `Thread::free()` interact with the slab allocator's cross-CPU free path?** Sub-commit 3b audits the slab cross-CPU free. A Thread freed by CPU0's reaper was allocated from CPU1's slab magazine (if the thread was migrated). Does `Thread::free()`'s slab return path hold any locks that could deadlock with `on_thread_reap()`'s `PROCESSES` write lock?

---

## 5. Composability concerns

The §Q9 failure was a compositional one: the counter (`live_thread_count`), the reaper (per-CPU zombie drain), and the registry (`PROCESSES`) each worked correctly in isolation, but their interaction produced a deadlock that no individual component had. This is the safety-non-compositionality pattern identified in the ALTIMIT research artifact — capability compositions (here, scheduler + allocator + registry) produce emergent failure modes that are invisible to component-level reasoning.

The redo must treat the reaper as a system-level concern, not a per-component optimization. The counter is not just an `AtomicU32`; it's a coordination primitive between three subsystems that operate on different CPUs with different lock orderings. The future ADR should specify the interaction protocol explicitly: what each subsystem observes, when it acts, and what invariants hold across the boundary.

The staged enablement approach (BSP-only reap first, AP-side reap only after threads route to APs) is a defense against compositional failure: by limiting the number of active reapers, we limit the number of concurrent interactions that can produce emergent races. This is not a fix; it's a risk-reduction measure that buys time for the full analysis.

---

## 6. Cross-references

- **ADR-013 §Q9** — original per-CPU reaper design in `docs/ADR-013-phase-2-smp-retrofit.md` (~line 898).
- **Stashed ADR-013 amendments** — Q9 deferral and hypothesis in `stash@{0}:docs/ADR-013-phase-2-smp-retrofit.md` ("BUILD-3 amendment: sub-commit 2 Q9 deferred with bisection evidence" and "Q9 hypothesis").
- **`kernel/src/sched/scheduler.rs`** — current BSP-scans-all-CPUs `reap()` at HEAD. Stashed version contains the same BSP-only code (post-Q9-deferral).
- **`kernel/src/sched/process.rs`** — current `on_thread_reap()` at HEAD (no `live_thread_count`; single-thread-per-process model). Stashed version is identical for this function.
- **ADR-013 §Q4** — wake-on-completer-CPU migration; determines which CPU a thread exits on, and therefore which CPU's reaper processes it.
- **ADR-013 §Q6** — ManuallyDrop + asm release in `block_current_into`; changes when threads can be blocked and woken, affecting zombie production timing.
- **ADR-012** — slab allocator; `Thread::free()` returns Thread storage to `THREAD_CACHE`, which may involve cross-CPU magazine interaction under sub-commit 3b.
- **`Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`** — sub-commit 3a pre-thinking; §3.4–§3.5 partially answer the ManuallyDrop + asm release interaction with the current BSP-only reaper (§4 Q6).
- **`docs/research/ALTIMIT-real-world-implementation.md` §G** — safety-non-compositionality results (arXiv 2603.15973); the §Q9 failure is an instance of this pattern in kernel scheduling.

---

**End of notes.**

*Draft research artifact. Becomes ADR-016 input when sub-commit 5+ schedules the §Q9 redo.*
