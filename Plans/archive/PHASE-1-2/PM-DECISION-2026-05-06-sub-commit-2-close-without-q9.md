# PM decision: close sub-commit 2 with §Q9 deferred; bisection complete

**Date:** 2026-05-06
**Author:** PM
**Status:** Decision recorded; closure dispatch issued
**Substrate:** EXP-H (sub-commit 2 minus §Q9) passes 100/100 at NEO_SMP=4. §Q9 implicated.

---

## Bisection result

EXP-H ran the closure gate (100x ipc_heap_kill_smoke at NEO_SMP=4) on sub-commit 2 with §Q9 reverted:

```
EXP-H-PASS-COUNT: 100/100
EXP-H-FAIL-COUNT: 0/100
```

Specifically, the post-revert working tree contains:

- §Q3 (per-CPU schedulers, `PerCpuScheduler` per CPU, `current`/`ready`/`zombies` queues per-CPU): **retained, working.**
- §Q5 mechanism restoration (deferred-quantum in per-CPU form): **retained, working.**
- AP GS-base rewire after GDT load (BUILD-3 amendment to ADR-013): **retained, working.**
- §Q9 (per-CPU reapers + `live_thread_count` + AP-side reaper participation): **reverted.**
- BSP-scans-all-CPUs zombie reap (option (b) per the bisection dispatch): **applied as the §Q9 replacement.**

The §Q9 rollback was substantive: ~30 lines deleted from `process.rs` (live_thread_count field, initializers, proc_on_thread_spawn, modified on_thread_reap body), ~10 lines changed in scheduler.rs (BSP reap loops over all CPUs' zombie queues instead of running per-CPU). Cerebro's diff is in his uncommitted working tree.

## What we know about §Q9's failure

The failure is in the §Q9 implementation specifically — not in §Q3 or §Q5 or AP-GS-base. We do NOT know precisely WHICH part of §Q9 is at fault:

- The `live_thread_count: AtomicU32` field on Process.
- The `fetch_sub(1, AcqRel)` decrement in the modified `on_thread_reap`.
- The "prev_count != 1 → return early" branch.
- AP-side reaper participation calling `reap()` on AP's per-CPU zombie queue.
- Some interaction among these.

We did not bisect within §Q9 because:

1. The closure gate passes without §Q9. That's a clean stop point.
2. §Q9 was forward-compat for multi-thread-per-process. Phase 1.5b/2 has one thread per user process. §Q9 isn't load-bearing for current functionality; deferring it doesn't break anything.
3. Drilling down within §Q9 would cost 2-3 more bisection rounds without changing the closure decision.

## Leading hypothesis for §Q9's failure (informational, not investigated)

Worth recording so future-us has a head start. The `on_thread_reap` modification:

```rust
let prev_count = proc_arc.live_thread_count.fetch_sub(1, Ordering::AcqRel);
if prev_count != 1 {
    return; // not the last thread; another reaper or this CPU later finishes
}
```

This returns early when prev_count != 1. In Phase 1.5b's one-thread-per-process world, prev_count should ALWAYS be 1. If it isn't, cleanup is skipped — the Process stays in PROCESSES, the wait loop's `any_user_process()` returns true forever, kernel hangs.

Possible races that would produce prev_count != 1:

- Counter initialization race: `live_thread_count: AtomicU32::new(1)` runs after the Process is registered AND after the first thread runs. If the thread can exit before the counter is fully observable to other CPUs, fetch_sub sees 0 and wraps to u32::MAX. Subsequent comparison `prev_count != 1` fires the early-return.
- Double-reap from AP-side reaper: if the same zombie thread somehow ends up on both an AP's zombie queue AND BSP's, fetch_sub fires twice. First call sees 1, returns and cleans up. Second call sees 0, wraps to u32::MAX, returns early without cleanup. But the Process is already cleaned by the first call — so the second-call early-return is benign. Unless ordering matters.
- Memory-ordering subtlety: AcqRel on fetch_sub may not synchronize with the registration's Release-store in the way the code assumes.

None of these is proven. The leading theory is that the counter races with initialization OR with the BSP-vs-AP reaper distribution. The instrumentation amplification observed in EXP-G is consistent with timing-window-sensitivity, which fits the racing-counter shape.

A future sub-commit that reintroduces `live_thread_count` should:

- Initialize the counter BEFORE the thread becomes runnable (eliminate init-race window).
- Verify exactly-once decrement semantics under SMP.
- Not rely on AP-side reaper participation while the per-CPU scheduler shape doesn't yet route threads to APs.
- Use SeqCst ordering until proven AcqRel is sufficient.

These are recommendations, not normative requirements. The amendment is informational.

## Decision

**Close sub-commit 2 with §Q9 deferred.** Cerebro commits the current uncommitted working tree (sub-commit 2 minus §Q9 plus option-(b) BSP-scans-all-CPUs reap). The closure gate passes. ADR-013 amendments record:

1. §Q9 deferred with bisection evidence.
2. The leading hypothesis for §Q9's failure mode.
3. Option (b) — BSP-scans-all-CPUs zombie reap — applied as the §Q9 replacement.
4. Recommendations for future re-introduction.

Sub-commit 2's actual scope ends up:

- §Q3 per-CPU schedulers ✓
- §Q5 deferred-quantum mechanism (per-CPU form, restored) ✓
- AP GS-base rewire after GDT load (BUILD-3 amendment) ✓
- BSP cross-CPU zombie reap (option b, replaces §Q9's per-CPU reaper distribution) ✓
- §Q9 distributed reapers + live_thread_count → DEFERRED to a future sub-commit

The closure gate (100x ipc_heap_kill_smoke at NEO_SMP=4) passes 100/100. Default test suite passes 25/25 + meta-FAIL on both NEO_SMP=1 and NEO_SMP=4. Build green at 18 warnings.

## Why not Option B (drill down within §Q9)

We've already spent a long day on sub-commit 2. The §Q5 lesson, applied honestly: bisect to the level where the closure gate passes, document what we know and don't know, commit. Drilling deeper into §Q9 to identify the exact race would cost 2-3 more rounds without changing the decision (defer §Q9). The diminishing-returns boundary is here.

Future sub-commit work that reintroduces multi-thread-per-process will need to design `live_thread_count` (or equivalent) carefully. The ADR amendment captures what we learned; future work will build on it.

## Why not Option C (commit sub-commit 2 minus §Q9, but still call §Q9 "in scope")

Honest scope is non-negotiable. §Q9 is not in sub-commit 2's scope at close. ADR-013 §Q9 reflects that. Calling it "in scope but deferred" is a discipline violation.

## ADR-013 amendments needed

Cerebro's working tree already has 30 lines added to ADR-013 (the BUILD-3 amendment for AP GS-base + the §Q5 amendment). The closure dispatch adds two more amendment subsections:

- **§Q9 amendment: deferred with bisection evidence.** Records the bisection chain (A → B → D → E → F → G → H), the decision to close without §Q9, and the option-(b) replacement.
- **§Q9 hypothesis subsection:** records the leading theory of failure mode (counter init race / double-reap / memory ordering) for future-us. Marked clearly as hypothesis-not-investigated.

## Closure gate

100x ipc_heap_kill_smoke at NEO_SMP=4: passed 100/100 in EXP-H. The gate is met. Sub-commit 2 closes.

## What sub-commit 3a inherits

Sub-commit 3a (ManuallyDrop + asm release in `block_current_into`) inherits this scheduler shape:

- Per-CPU schedulers (`PerCpuScheduler`).
- Per-CPU `current`/`ready`/`zombies` queues (zombies drained by BSP scanning all CPUs).
- Deferred-quantum mechanism in per-CPU form.
- No AP-side reaper.
- No `live_thread_count` on Process.

3a's design conversation should account for: BSP cross-CPU zombie scan is a "current-CPU lock then loop over peers' locks" pattern that may interact with 3a's lock-order resolution. Worth flagging when 3a's checkpoint conversation happens.

---

**End of decision note.**
