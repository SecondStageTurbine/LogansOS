# [SUPERSEDED — DO NOT RATIFY]

> **Status:** SUPERSEDED 2026-05-08, same day, by an expanded ADR-016 draft.
> **Replaced by:** `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md`. Same ADR number; broader scope.
> **Reason:** This earlier-same-day draft recommended A1-only (IRQ-safe heap wrapper) as the architectural response to WEDGE-INV-J1. Project lead's "Trust but Verify" decision in this conversation (2026-05-08) selected the broader posture: A2-direct (scheduler decoupling from heap) + A1 defense-in-depth + co-ratified Bible amendment 8.5 (`Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md`). The replacement ADR covers all three.
> **Disposition:** This draft is preserved verbatim below for project history. **Do not ratify.** It is superseded by the broader-scope draft at the path above.
> **What carries forward:** The A1 implementation specifics (in-house wrapper around `linked_list_allocator::Heap`, ~30-50 lines, `pushfq`/`cli`/`popfq` pattern, `kernel/src/mm/heap.rs` location, SAFETY comments per Bible 15.2) are unchanged in the superseding draft. The recommendation rationale shifts from "A1 alone is sufficient" to "A1 is defense-in-depth on top of A2"; the implementation work is the same.

---

(Original ADR-016 draft preserved verbatim below.)

---

# ADR-016 (DRAFT): IRQ-safe global heap allocator

| | |
|---|---|
| **ADR Number** | ADR-016 |
| **Title** | IRQ-safe global heap allocator |
| **Status** | DRAFT — PM-authored proposal, awaiting project lead ratification |
| **Date** | 2026-05-08 |
| **Authority** | Project lead (per Bible Article 24) |
| **Author** | PM |
| **Supersedes** | None |
| **Superseded by** | `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` |

---

## Context

WEDGE-INV-J1 (`Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md`) identified the proximate cause of the SMP `POST_SMOKE_SMP_SCENARIO` wedge as timer-mediated heap-allocator-lock re-entry: BSP holds the global heap lock during a `VecDeque` drop in `reap()`'s drained-buffer cleanup, timer ISR fires, ISR yields to a worker, worker exits and calls `s.zombies.push_back(prev)` which triggers a `VecDeque` reallocation (because `mem::take` left zero capacity), reallocation tries to take the heap lock, heap lock still held by interrupted reaper, deadlock.

`Plans/DISPATCH-2026-05-08-wedge-fix-L-cerebro.md` (Family L) addresses the proven mechanism by preserving zombie-queue capacity and dropping drained buffers under `without_interrupts`. L closes the wedge as observed, but the audit's per-call-site table shows multiple other scheduler-held `VecDeque` push paths (`spawn`, `push_ready`, `yield_now → rotate → ready.push_back`, `wake_thread`, `wake_thread_front`, `kill_thread_by_pid`) that share the same vulnerability class. Each is theoretically vulnerable to the same heap-lock-re-entry pattern under capacity pressure.

The class is not unique to AltimitNeo. Per `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`:

- Linux's spinlock convention requires `spin_lock_irqsave` for any lock that may be acquired in both process and interrupt contexts; `lockdep` actively checks this invariant.
- Theseus OS built `irq_safety` (`github.com/theseus-os/irq_safety`) as a dedicated crate after hitting this pattern — an `IrqSafeMutex` wrapper that disables interrupts for the lock guard's lifetime.
- Rust-for-Linux's `kernel::sync::SpinLock` exposes `lock()` vs. `lock_irqdisable()` as distinct APIs; choosing wrong is a known bug class.
- Redox's design baseline is "interrupts disabled almost everywhere in kernel mode" (`redox-os.org/news/kernel-10/`).

AltimitNeo currently uses `linked_list_allocator::LockedHeap` (`kernel/src/mm/heap.rs:32-33`) as the global heap allocator. `LockedHeap` is `spinning_top::Spinlock<Heap>` per `linked_list_allocator-0.10.6/src/lib.rs:285-343`. The lock has no IRQ-safety wrapper. Process-context heap operations run with interrupts enabled by default; interrupt-context handlers that allocate (e.g., timer-tick paths reachable from `consume_preempt_defer`, `yield_now`, `wake_thread`, etc.) can therefore deadlock against an interrupted process-context heap operation.

This ADR addresses that architectural concern. Family L closes the proven path; Family A as ratified by this ADR closes the underlying class.

## Decision (proposed; pending PL ratification)

Adopt **Option A1: wrap the global heap allocator with an IRQ-safe shim that disables local interrupts during acquire and restores prior interrupt state on release.**

Rationale: A1 is the smallest delta that closes the entire heap-lock-re-entry class. It applies the convention every comparable Rust no_std kernel has converged on. It does not require restructuring scheduler internals or replacing `linked_list_allocator`. The performance cost (one `cli` and one `pushf`/`popf`-equivalent per heap op) is small compared to the cost of the heap operation itself, and is what every other modern OS pays for the same correctness guarantee.

Defer to a future ADR the question of whether scheduler-critical-section data structures should be restructured to avoid heap dependency entirely (Option A2 below). That's a longer-horizon design question best deferred until Phase 2's capability system and Phase 3's filesystem work surface additional scheduler-internals concerns.

[remaining options, consequences, implementation plan, risks, alternatives, and references sections preserved verbatim in original draft, omitted here for brevity. The superseding draft at `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` covers the same ground in expanded scope.]

---

**End of superseded draft.**

*Originally drafted as A1-only architectural follow-up to Family L. Superseded same-day by the broader A2+A1+Bible-amendment posture per project lead's "Trust but Verify" decision. Preserved for project history; not actionable.*
