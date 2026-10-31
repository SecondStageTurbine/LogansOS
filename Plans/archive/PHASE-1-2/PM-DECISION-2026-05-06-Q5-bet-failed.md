# PM decision: Q5 bet failed — restore the deferred-quantum mechanism

**Date:** 2026-05-06
**Author:** PM
**Status:** Decision recorded; restoration dispatched to Cerebro
**Substrate:** ADR-013 §Q5, sub-commit 2 100x closure-gate failure

---

## What happened

Sub-commit 2 implemented ADR-013 §Q5 deletions exactly as specified: removed `Thread.preempt_defer_ticks`, `HANDOFF_PREEMPT_DEFER_TICKS`, `consume_preempt_defer`, `clear_current_preempt_defer`, `defer_current_preempt_for_handoff`. Cerebro's working tree builds clean (18 warnings unchanged), passes 25/25 default-suite scenarios on both `NEO_SMP=1` and `NEO_SMP=4`. The closure gate is the 100x consecutive `ipc_heap_kill_smoke` run on `NEO_SMP=4`. Run 24 failed:

```
[ neo-init ] sent go-block; worker should now be RecvBlocked
[ neo-init ] attempting sys_kill on Blocked worker (expect EINVAL)
[ kernel ] proc: terminated 'worker' (pid=2) via sys_kill
[ neo-init ] BUG: sys_kill on Blocked did NOT return EINVAL
NEO_TEST/1 event=FAIL name=ipc_heap_kill_smoke reason=neo_init_exit_nonzero code=5
```

Cerebro stopped at the gate, did not commit. Discipline working as designed.

## Why the bet failed

The §Q5 argument confused two distinct atomicity claims:

- **Kernel-side critical section atomicity** — `Channel::send`/`Channel::recv` run entirely inside `without_interrupts`. While the syscall body executes, IF=0; the timer cannot fire; the front-queue + yield window is closed by construction. **This holds.**
- **End-to-end "receiver reaches its next syscall before any other thread runs"** — once the asm `context_switch` lands in the resumed thread and `block_current_into`'s `without_interrupts` scope exits, IF gets re-enabled. The PIT then fires whenever it would have fired anyway, on a clock independent of "the resumed thread just got the CPU." On bare hardware the next tick is far enough away that user code reaches its next syscall first; under QEMU SMP, the host's scheduling jitter makes the inter-tick gap unpredictable. **This does not hold.**

The §Q5 argument relied on the second claim. It does not follow from the first. The deferred-quantum mechanism existed precisely because the second claim does not hold.

## What the mechanism actually did

The mechanism gave the just-resumed thread (and, separately, the current thread that completed a rendezvous as the receiver) a quantum-bounded credit of preemption immunity:

- `wake_thread_front` set `thread.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS` before push_front. When the front-queued thread became current, `on_tick` would `consume_preempt_defer()` and no-op for up to 32 ticks (~320ms at 100Hz).
- `continue_current_after_receive` called `defer_current_preempt_for_handoff()` to set the **current** thread's credit, so a receiver that completed a parked sender could reach its next blocking syscall before the timer ever ran.
- `clear_current_preempt_defer` was called at every syscall entry, bounding the protection to one quantum.

The mechanism is not load-bearing for *correctness in the kernel* — it is load-bearing for the **userland-observable semantic** that "after `sys_send` to a `RecvBlocked` peer returns to the sender, the peer has reached its next blocking syscall." Several Phase 1.5b smoke tests depend on that semantic. `ipc_heap_kill_smoke` is the canonical example: it asserts that `sys_kill` on a peer that *should* be blocked returns `EINVAL`, which can only be true if the peer has actually reached its next blocking recv.

## Decision

**Restore the deferred-quantum mechanism in per-CPU form.** Sub-commit 2 closes with the mechanism intact, not with it deleted. ADR-013 §Q5 is amended to record the failed bet and the restoration.

The mechanism is forward-compatible with everything sub-commit 3+ does:

- Sub-commit 3a's `block_current_into` ManuallyDrop+asm release pattern doesn't interact with `preempt_defer_ticks` (which lives on the Thread, read by `on_tick`).
- Sub-commit 3b's cross-CPU IPC needs *some* analog — when a sender on CPU0 wakes a receiver parked on CPU1, the cross-CPU wake will need to set the receiver's defer credit before the receiver becomes current on CPU1. The per-CPU mechanism handles this naturally: defer credit is a per-thread field; whoever sets it observes per-thread state, no cross-CPU lock needed.
- Sub-commit 4's TLB shootdown work is orthogonal.

The §Q5 deletion was a cleanup attempt that should not have been bundled with the per-CPU scheduler refactor. Cleaner separation in retrospect: per-CPU schedulers (Q3 + Q9) is one change; deferred-quantum reconsideration is another, and probably never happens — the mechanism is small and correct and deleting it bought us nothing.

## Why not a stronger handoff mechanism

Cerebro's close report mentioned "replace it with a stronger explicit handoff mechanism" as the alternative. I considered it; it's the wrong direction:

- "Stronger handoff" would mean making the receiver yield to the woken sender (i.e., `continue_current_after_receive` does `wake_thread_front` + `yield_now`, mirroring `handoff_to_rendezvous_peer`). But the receiver is the **consumer** of the payload — yielding before the receiver acts on it inverts the producer/consumer relationship and breaks all the tests that depend on receiver-runs-before-sender-observes.
- A symmetric direct-handoff primitive (e.g., a `swap_to(thread)` that atomically demotes current to ready-back and promotes thread to current without going through a queue) is plausible but far larger than restoring `preempt_defer_ticks`. It also doesn't subsume `preempt_defer_ticks` — even with a swap-to primitive, the resumed thread still needs preemption immunity until it reaches its next syscall.

The minimum correct restoration is the original mechanism, per-CPU.

## Sub-commit 2 anti-scope clarification

The §Q5 deletion is no longer part of sub-commit 2. Sub-commit 2's actual scope is:

- ADR-013 §Q3 (per-CPU schedulers) ✓
- ADR-013 §Q9 (per-CPU reapers + `live_thread_count`) ✓
- AP GS-base rewire after GDT load (BUILD-3 amendment) ✓
- Two new test scenarios ✓
- §Q5 deletion ✗ — reverted; mechanism stays in per-CPU form

ADR-013 §Q5 itself gets the amendment: the bet failed, the mechanism is retained, the deletion will not be retried.

## Closure gate

The 100x `ipc_heap_kill_smoke` gate stays as the closure gate. With the mechanism restored, it should pass.

If it doesn't — i.e., the mechanism in per-CPU form has some subtle break — STOP again and surface. But I expect this to pass. The mechanism worked for Phase 1.5b; per-CPU form preserves the per-thread field semantics; nothing about SMP changes the fundamental "credit consumed on every tick of the CPU the thread runs on" pattern.

---

**End of decision note.**
