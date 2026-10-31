# PM-NOTE: WEDGE-INV-J1 Verdict A — bug identified as heap-lock re-entry during reap-induced zero-capacity zombie queue

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures the WEDGE-INV-J1 audit's Verdict A finding: the wedge is timer-mediated heap-allocator-lock re-entry, enabled by `reap()`'s `mem::take` leaving `s.zombies` at zero capacity, triggering allocation in a subsequent `exit() → rotate → push_back` path while the interrupted reaper still holds the heap lock during its drained-buffer drop. Also captures the disposition of J2 (not needed), the strengthening of the unifying-bug interpretation (now plausible across all four historical signatures), and the fix-family options for the next dispatch.
**Authority:** PM. Records investigation results. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` remains active and is on track to be discharged by the upcoming fix dispatch + verification cycle.

---

## 1. Verdict A summary

WEDGE-INV-J1's static audit (`Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md`) names the bug. The mechanism is structurally interrupt-context same-CPU lock re-entrance, but **the contended lock is the global heap allocator's `spinning_top::Spinlock`, not `PerCpuScheduler.inner`**. The full chain:

1. BSP enters `reap()` (`kernel/src/sched/scheduler.rs:541-546`). Under `without_interrupts`, takes `PerCpuScheduler.inner` (a `spin::Mutex`), does `mem::take(&mut s.zombies)`, decrements `zombie_count`, releases scheduler inner. The `mem::take` replaces `s.zombies` with `VecDeque::default()` — **zero capacity** (the original was created with `ZOMBIE_INITIAL_CAPACITY = 4`).
2. The drained `VecDeque<Box<Thread>>` is processed in a per-zombie loop *outside* the `without_interrupts` region (`kernel/src/sched/scheduler.rs:547-557`). When the loop ends, the drained `VecDeque` falls out of scope and is dropped. The drop frees the underlying buffer via the global heap allocator (`linked_list_allocator::LockedHeap`, which is `spinning_top::Spinlock`-backed per `Cargo.lock:56-63` and `kernel/src/mm/heap.rs:32-33`). The heap lock is held during this free.
3. While BSP is inside the heap lock during the drained-buffer free, the timer interrupt fires (IF=1 outside `without_interrupts`).
4. `timer_isr_entry → timer_rust_handler → sched::on_tick → yield_now → rotate` runs (`kernel/src/arch/x86_64/timer.rs:181-189`, `kernel/src/sched/scheduler.rs:306-310`). Rotate takes scheduler inner (FREE — reaper released it in step 1), context-switches to a ready `smp-hog-N` worker.
5. The worker reaches its loop's `crate::sched::exit()` call. Under `without_interrupts`, exit calls `rotate(ThreadState::Zombie, WhichQueue::Zombie)` (`kernel/src/sched/scheduler.rs:354-364`, `kernel/src/sched/scheduler.rs:241-247`). Rotate takes scheduler inner (still free from step 4's release).
6. `s.zombies.push_back(prev)`: pushes the exiting thread's `Box<Thread>` into `s.zombies`. Since `mem::take` left the queue with zero capacity in step 1, `push_back` triggers `VecDeque` reallocation.
7. The reallocation enters `<LockedHeap as GlobalAlloc>::alloc`, which calls `.lock()` on the `spinning_top::Spinlock`.
8. The heap lock is **still held by the interrupted reaper at step 2**. Spinlock not re-entrant. CPU 0 spins in `spinning_top::RawSpinlock::lock`'s `_mm_pause` loop.
9. The captured RIP `0xffffffff8007bc22` (across all 4 WEDGE-INV-8 captures) is the heap allocator's spinlock acquire path. CPU 0's scheduler inner is observed locked because step 5's worker exit took it before the deadlock fired.

The two-fact reading of WEDGE-INV-8 ("CPU 0 spinning in `spinning_top` AND scheduler inner is locked") is consistent with this geometry: both observations are downstream of the same one wedge.

## 2. The unifying-bug interpretation strengthens

`Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` §3 noted the possibility that the four historical signatures (`POST_SMOKE_SMP_SCENARIO`, `SMOKE_MID_PHASE`, `SMOKE_COMPLETION` literal, `POST_SMOKE_USER_PROCESS`) all share one underlying bug. The audit's per-call-site table identifies multiple scheduler-held `VecDeque` push paths that share the same vulnerability class: **any heap allocation triggered while `PerCpuScheduler.inner` is held can deadlock if the heap lock is contended with an interrupted heap-lock holder**.

Specifically, the audit names these scheduler-held allocation-risk sites beyond the reap-induced zombie capacity loss:

- `spawn` and `push_ready` — ready queue `push_back`, can allocate if capacity grows.
- `rotate` via `yield_now` — ready queue `push_back`.
- `rotate` via `exit` — zombie `push_back` (the proven mechanism).
- `wake_thread` and `wake_thread_front` — ready queue `push_back` / `push_front`.
- `kill_thread_by_pid` — zombie `push_back` (same-class as `exit`).

The four historical signatures plausibly map to allocations in different scheduler-held paths exercised by different scenario phases. POST_SMOKE_SMP_SCENARIO is the reap-induced zombie path (proven). SMOKE_MID_PHASE could be spawn/yield ready-queue growth. SMOKE_COMPLETION could be wake_thread at smoke completion. POST_SMOKE_USER_PROCESS could be user spawn allocations. **All would close in one shot if the scheduler-critical-section-no-heap invariant is enforced architecturally.** Localized fix to reap() alone would close the proven path and possibly leave the others latent.

This is the unifying interpretation. It is now strongly plausible. It is not yet proved — verification of the interpretation requires running the post-fix kernel through the historical-signature-producing scenarios and confirming all four are gone, not just the proven one.

## 3. Fix-family options

The audit's §"Proposed Fix" lists four localized options plus an architectural follow-up. Restating with my framing:

### Family L (Localized, narrow)
**L1.** Replace `reap()`'s `mem::take` with a capacity-preserving drain (e.g., `drain(..)` then collect, or swap with a pre-allocated empty VecDeque that has reserved capacity). Live scheduler zombie queue never falls below `ZOMBIE_INITIAL_CAPACITY`.
**L2.** Drop the drained `VecDeque` under `without_interrupts` so timer can't fire mid-drop.
**L3.** Add `debug_assert!(s.zombies.capacity() > s.zombies.len())` before the `push_back` (and same for ready-queue pushes), so future violations fail loudly in test.

L1 + L2 + L3 is the minimum coherent fix for the proven mechanism. Closes `POST_SMOKE_SMP_SCENARIO` definitively. Does NOT address other scheduler-held allocation paths (ready queue growth in spawn/yield/wake/etc.) which remain theoretically vulnerable to the same heap-lock-re-entry pattern.

### Family A (Architectural, broad)
**A1.** Make the global heap allocator IRQ-safe (wrap acquire with `interrupts::disable()`/restore). Closes the heap-lock-re-entry pattern for ALL heap users, not just the scheduler. Touches `kernel/src/mm/heap.rs`.
**A2.** Restructure scheduler critical sections to never depend on the general heap. Bounded queues, pre-allocated capacity, or no-alloc data structures for `ready` and `zombies`. Larger surgery; touches scheduler structures across multiple files.

A1 alone closes ALL four historical signatures if the unifying interpretation holds. A2 is a long-term goal that gradient-improves over time; not required to close the wedge.

### Family L+A (recommended)
Apply L1+L2+L3 immediately to close the proven path and add the debug assertions for fail-loud regression catching. In parallel, draft an ADR for A1 (IRQ-safe heap allocator) as architectural follow-up to be ratified separately. Verify the L fix at sample size sufficient to test all four historical signatures (not just POST_SMOKE_SMP_SCENARIO); if any of the other three signatures resurface post-L-fix, A1 becomes urgent rather than follow-up.

This is the recommended path because (a) it closes the proven wedge fastest, (b) it doesn't pre-commit to architectural changes before evidence justifies them, (c) it preserves the discipline of "fix the proven thing, treat the latent class as a separate normative decision."

## 4. Disposition of WEDGE-INV-J2

**No longer needed.** Cerebro's audit note's final paragraph says it directly: "removing heap allocation/deallocation from scheduler critical sections is independently correct and directly addresses the observed wedge geometry." J2's saved-GPR lock-pointer capture would be confirmation, not necessity. The audit's static-analysis evidence is sufficient: the captured `spinning_top` RIP exists in exactly one place in the codebase (the heap allocator), the captured scheduler-inner-locked observation is explained by the worker exit path, and the partial-reap phase precondition matches the `mem::take` zero-capacity setup.

If the L+A fix verifies clean across all four historical signatures, J2 is permanently retired. If it doesn't (e.g., one of SMOKE_MID_PHASE / SMOKE_COMPLETION / POST_SMOKE_USER_PROCESS resurfaces post-fix), J2 may come back as a focused follow-up to identify whether the non-reap-path bugs share the same lock-target.

## 5. Wedge-fix-before-SC-3B commitment status

**Active and on track.** Per `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`: discharge requires (1) identify the lock-holder and call path, (2) determine the mechanism, (3) design and commit a fix, (4) verify at sufficient sample size, (5) close report.

Status:
- (1) Identified. The lock-holder is the BSP itself, in `reap()`'s drained-buffer drop path.
- (2) Determined. Timer-mediated heap-allocator re-entry, enabled by `mem::take` zero-capacity left-behind.
- (3) Pending. Fix dispatch design to follow per §3.
- (4) Pending. Verification methodology to be specified in the fix dispatch.
- (5) Pending. After verification.

Estimated path to discharge: one fix dispatch (small, focused — Cerebro's option family L immediate work), one verification cycle (200+ boots covering all four historical signatures), one close report. If the unifying interpretation holds, this is days of work, not weeks.

## 6. Project-artifact dispositions

- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` (lock-free `zombie_count == 0` short-circuit composition-safety analysis): not contradicted; the bug is upstream of the short-circuit's correctness. The fastpath remains a candidate for de-emphasis per `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` §6.4 but that's forward-looking.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` (H1 narrow refutation): unchanged; H1 was about lock-free `zombie_count` divergence, which is not the actual bug.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md`: superseded in detail by this note's §1 mechanism description, but remains canonical for the WEDGE-INV-8 timeframe. Cross-referenced rather than rewritten.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`: directionally vindicated. The pattern (interrupt-context same-CPU re-entrance, `spinning_top`-backed lock) was correctly identified; the lock-target was located on the wrong object (scheduler inner vs heap allocator). The §5 fix-family options remain applicable to the architectural Family A; Family L is purely AltimitNeo-specific to the reap zero-capacity issue.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md`: framework intact. McKenney's "fastpaths hide bugs" applies (the lock-free `zombie_count == 0` short-circuit's specific correctness wasn't the bug, but the broader observation that scheduler-held heap allocations are a fastpath that hide slowpath issues stands).
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md`: substrate-shift damping interpretation strongly supported. ADR-015's heap shift changed the timing window for the heap-lock-re-entry race, which is consistent with the rate dropping from PATCH1's ~14% to post-ADR-015's 2.67%.

## 7. Stash topology

Unchanged from `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` §5. The J1 audit was static analysis only; no stashes touched. Three stashes remain:

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

Most-recent backup (`Z:\AltimitNeo\2026-05-08-212442\`) preserves all three.

## 8. Cross-references

- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — Cerebro's audit note. Canonical source for the mechanism, per-call-site table, and fix-family proposals.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-J1-audit-cerebro.md` — the dispatch that produced the audit.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` — the prior project-state note; this note refines the §2 "what's not known" portion of that note into specifics.
- `Plans/WEDGE-INV-8-FINDINGS-2026-05-08.md` — the captures whose RIP is now identified as the heap allocator's spinlock.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — the directional research that vindicated; lock-target was wrong, pattern was right.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` — the McKenney framework; "de-emphasize fastpaths" applies but in a refined form.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment; status in §5 above.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 — the substrate-shift damping interpretation; strongly supported.

---

**End of note.**

*Mechanism named. Fix-family options laid out. J2 retired. Wedge-fix-before-SC-3B commitment on track for discharge via L+A path, with L immediate and A as ADR-class follow-up. Project lead's call on fix dispatch direction is the gating decision; PM is ready to draft on signal.*
