<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat B, OT-tick-1 — make a ring-3 tick always preemptible (test first, then fix) (2026-11-09)

**Seat:** Seat B (cloud), who owns the reproduction (#98). **Branch:** `claude/tick-preempt-ring3` off `origin/main`.
**Diagnosis:** `Plans/RETURN-2026-11-09-ap-preemption-diagnosis-dexter.md` (relay 381, Dexter).
**Kernel-touching, SMP scheduler core:** the 4-shard gate, the concurrency pitfall-bank table, and a
**cross-family adversarial review** before merge (the CLAUDE.md SMP-core rule).

## Step 1: the falsification test, before any fix

Dexter's hypothesis H1: on the BSP, `sched::on_tick` returns early for a ring-3 spinner because of
`PREEMPT_DISABLE_DEPTH` or a re-armed `preempt_defer_ticks` (an IPC handoff credit), so a preempted client waits at
the back of the ready ring until a spinner exits.

**Caution:** your 22:31 result (the stall survives a yielding WM and Arthur) isn't fully explained by H1. If both
spinners yield, the terminal should get turns. That's why the test comes first.

Build Dexter's diagnostic, behind a test feature only. At the stall and once a second after, print per CPU
`TICKS_SEEN`, `ROTATE`, `READY_LEN`, `CURRENT_TID` and `CURRENT_IS_USER`. On the BSP, also print
`PREEMPT_DISABLE_DEPTH[0]`, the current thread's `preempt_defer_ticks`, and the terminal's tid and state. Run it on
the terminal scenario and on a minimal one: two ring-3 spinners, plus one compute process printing per-ms progress,
on `NEO_SMP=4`. Post the readings on #98 and map them to Dexter's outcome table:
- **H1 confirmed:** ticks advance, rotate is frozen, and depth or defer is nonzero. Go to step 2.
- **The tick is frozen:** the timer path is the bug. Report it, and don't apply H1's fix.
- **User threads are on APs:** H2 is live. Report it; it becomes its own lane.
- **Something else** (for example, the terminal isn't in any ready ring, or its state is wrong): report the
  numbers. This is the case your yield result points to.

## Step 2 (only if H1 is confirmed): the fix

Pass the interrupted ring from `timer_rust_handler` to `on_tick`. When the interrupted context is ring 3, skip the
`PREEMPT_DISABLE_DEPTH` and `consume_preempt_defer()` gates. Ring-0 contexts keep both gates unchanged.
- **Kill-switch:** a feature `tick-ring3-gates-legacy` restores today's behaviour bit for bit, with a test that
  proves it.
- **Regression surface (Dexter §3):** `threads_tls`, `threads_exit_group`, `smp_same_cpu_ipc`,
  `smp_cross_cpu_rendezvous_stress`, `smp_c11_migration_reservation`, `irq_xmm_preserve`, `lapic_timer`,
  `lapic_timer_demote`, plus the full 4-shard gate. If an IPC battery flakes, the fix is to scope the defer to the
  peer's first syscall. Never re-enable the ring-3 gate.
- **The new scenario:** the compute thread's worst gap is at most one tick with the fix, and the gap reproduces with
  the legacy feature (the control).
- **Pitfall-bank table** in the PR, per class: CP-15, CP-21, CP-25 and CP-37. Add a new CP entry for "kernel-only
  preemption gates honoured for ring 3".
- Then #98 rebases onto the fix, and its scenarios should run past the first commit.

## Not in this lane

H2, the AP tick (arm the LAPIC timer on APs and make the reschedule IPI really reschedule), is real but not this
trace. It stays a separate lane, before any user thread is placed on an AP.

## Close

Run the close-gate and post the 4-shard PR run on the new PR. The PM then queues a Codex-family review of the
scheduler change. Commit and push at every checkpoint.
