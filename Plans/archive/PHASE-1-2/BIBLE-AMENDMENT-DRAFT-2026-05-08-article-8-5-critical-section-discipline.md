# Bible Amendment Draft: Article 8.5 — Critical-section discipline

| | |
|---|---|
| **Type** | Bible amendment proposal |
| **Status** | DRAFT — PM-authored proposal, awaiting project lead ratification |
| **Date** | 2026-05-08 |
| **Authority** | Project lead (per Bible Articles 24, 27) |
| **Author** | PM |
| **Affects** | Article 8 (Concurrency and Scheduling) — adds new sub-article 8.5 |
| **Triggered by** | WEDGE-INV-J1 finding (`Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md`) and project lead's "Trust but Verify" ratification of A2-direct + A1 defense-in-depth path |

---

## Rationale

WEDGE-INV-J1 identified a heap-lock-re-entry deadlock pattern enabled by:

1. The global heap allocator (`linked_list_allocator::LockedHeap`) using a plain `spinning_top::Spinlock` with no IRQ-safety;
2. A scheduler critical section (`reap()`'s drained-buffer drop) holding the heap lock during a window when timer interrupts were enabled;
3. A timer-mediated context switch into a worker whose `exit()` path triggered a heap allocation under the scheduler lock — which deadlocked against the interrupted reaper.

The proximate fix (Family L) and the architectural fix (Family A) are addressed in `Plans/ADR-016-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md`. This Bible amendment encodes the underlying discipline as a project invariant, so future contributors are guided by written rule and not solely by code review or precedent.

The amendment serves the Bible's prevention-discipline posture per Article 1.3 ("real operating systems engineering... every commit is intended to ship") and Article 14's /simplify gate ("Anything that can be deleted from the plan without breaking the goal milestone is deleted") — by making invalid patterns inexpressible-at-design-time rather than relying on case-by-case code review to catch them.

The amendment also discharges a recurring concern surfaced by `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`: every comparable Rust no_std kernel that's been at this longer (Theseus, Rust-for-Linux, Linux, Redox) has converged on encoded discipline plus structural enforcement. AltimitNeo joins that convergence.

---

## Proposed amendment

Add a new sub-article 8.5 to Article 8 (Concurrency and Scheduling), positioned after the existing 8.4. The new sub-article reads:

---

> ### 8.5 Critical-section discipline
>
> 8.5.1 **Spinlock + interrupt discipline.** Any spinlock that may be acquired from both interrupt context and process context must have IRQ-safe acquisition: the process-context acquirer disables local interrupts before taking the lock and restores the prior interrupt state on release. The kernel provides IRQ-safe lock primitives for this purpose. Plain spinlocks are reserved for locks that are exclusively process-context (no ISR ever takes them) or exclusively interrupt-context (no process ever takes them). The choice is documented at lock declaration via the type used.
>
> 8.5.2 **No heap allocation in interrupt context.** Interrupt handlers and their transitive callees may not allocate from or deallocate to the global heap. This includes timer handlers, IPI handlers, exception handlers, and any function reachable from them. ISR-context code paths use pre-allocated, fixed-size, or stack-bounded data structures. The global heap may not be made structurally safe to use from ISR context as an excuse to violate this rule; the rule stands regardless of allocator implementation.
>
> 8.5.3 **Scheduler heap-independence.** The scheduler's critical sections — any code path that holds a per-CPU scheduler lock — must not depend on the global heap allocator's availability. Scheduler-internal data structures use bounded or pre-allocated storage so that scheduling operations never trigger heap allocation. The scheduler's correctness is independent of whether the heap is currently locked, fragmented, exhausted, or otherwise unavailable.
>
> 8.5.4 **No blocking while holding a spinlock.** A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.
>
> 8.5.5 **Enforcement.** These rules are enforced by code review at PR time, by `unsafe`-block SAFETY comments per Article 15.2 where unsafe code is involved, and structurally by the type system where lock primitives encode the discipline. Future tooling (lockdep-equivalent runtime checks, static analysis) is anticipated but not required to ratify any individual rule.

---

## Consequences

If ratified:

- **Article 8.5 becomes binding project policy.** Future PRs that introduce ISR-context heap allocations, plain-spinlock contention with ISRs, or scheduler-critical-section heap dependencies must be rejected at review or rewritten.
- **Existing code must conform within Phase 1.4's close.** ADR-016's A2 implementation is the conformance work for 8.5.3; ADR-016's A1 implementation is the conformance mechanism for 8.5.1; ADR-016's discipline (no ISR allocations) is the conformance posture for 8.5.2. The bible amendment and the ADR are co-ratified — neither lands without the other for this initial wave.
- **8.5.4 is restated existing practice**, not new policy. The current scheduler honors this rule (per WEDGE-INV-J1's audit, every direct scheduler-lock holder either wraps with `without_interrupts` or is documented as caller's responsibility). 8.5.4 codifies the practice for future contributors.
- **Bible version bumps from 1.0.4 to 1.0.5** per Article 27 (patch bump matching precedent set by Article 6.6's addition: 1.0.3 → 1.0.4). The footer's "Last amended" line updates to 2026-05-08 with the new article reference.
- **No existing article is modified.** This is purely additive.

If rejected:

- The discipline remains implicit / case-by-case, defended by code review without written rule.
- Future contributors who introduce violations rely on reviewers to catch them; the structural enforcement (ADR-016) catches some but not all (Article 8.5.4 has no structural backstop, for instance).
- Project lead's standing decision in this case should be recorded with rationale.

## Phasing

The amendment is binding from the moment of ratification, but its FULL satisfaction depends on ADR-016's implementation:

- **Immediate (at ratification)**: 8.5.1 partial — applies to any new lock; existing locks that violate (the global heap allocator's `LockedHeap` per WEDGE-INV-J1) are subject to ADR-016's A1 implementation. 8.5.2 immediate — no new ISR allocations permitted from this point. 8.5.4 immediate — already practiced.
- **After ADR-016 A1 implementation**: 8.5.1 fully satisfied for the global heap.
- **After ADR-016 A2 implementation**: 8.5.3 fully satisfied for the scheduler.
- **Ongoing**: every new lock and every new ISR is reviewed against 8.5.1–8.5.4.

## Cross-references

- Bible Article 1.3 — every commit is intended to ship; this amendment's prevention discipline supports that posture.
- Bible Article 8.3 — global locks forbidden in hot paths; 8.5.3 extends this from "forbidden" to "structurally impossible at the scheduler layer."
- Bible Article 14 — /simplify gate; 8.5 deletes the implicit-discipline pattern in favor of explicit-rule.
- Bible Article 15.2 — `unsafe` SAFETY comments; 8.5.5 references this as one enforcement mechanism.
- Bible Article 27 — amendment process this draft follows.
- `Plans/ADR-016-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` — the technical decisions implementing 8.5.1 (A1) and 8.5.3 (A2). Co-ratified.
- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — the bug that motivated the amendment.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — the prior-art across Rust no_std kernels showing the convergence on encoded discipline.

---

**End of amendment draft.**

*PM proposes; project lead ratifies. Upon ratification: Bible version bumps to 1.1.0, the new Article 8.5 is appended to Part II of the bible, the amendment date is recorded in the footer, and the existing draft document is marked RATIFIED with the SHA of the bible-update commit.*
