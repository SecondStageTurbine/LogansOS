# ADR-018: Bible Article 8.5.4 clarification — preempt-while-holding for mixed-IRQ-context locks

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Ratified 2026-05-10 by project-lead approval.
**ADR Number:** ADR-018.
**Bible amendment effected:** Article 8.5.4 expanded; bible v1.4.0 → v1.5.0.

---

## Rationale

SC-3B's investigation (closed 2026-05-10 via commit e2b3f4f) named a previously-unnamed failure mode: **Hpw — preempt-while-holding-lock + IRQ-off same-CPU waiter**, producing single-CPU self-deadlock.

The current Bible Article 8.5.4 ("No blocking while holding a spinlock") conceptually covers Hpw because preemption is a form of yielding. But it does not explicitly name the preempt-while-holding case, and the failure shape — same-CPU IRQ-off waiter spinning on a lock held by a preempted thread — was non-obvious enough that the SC-3B investigation required five passes (Phases A through D plus Janus second-pass) over 36 wall-clock hours to localize.

Explicit naming in the bible prevents future reviewers from missing the discipline. The proposed amendment also references the concrete fix pattern (`without_interrupts` wrapping for mixed-IRQ-context process-side acquirers) so the rule is actionable, not just declarative.

## Proposed amendment

### Current Article 8.5.4

> 8.5.4 No blocking while holding a spinlock. A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.

### Proposed new Article 8.5.4

> 8.5.4 No blocking, yielding, or being preempted while holding a spinlock.
>
> A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.
>
> Additionally — and this is the discipline that SC-3B's Hpw mechanism (commit e2b3f4f, 2026-05-10) was caused by violating: a spinlock acquired in preemptible context, where the same lock may also be acquired in IF=0 (interrupts-disabled) context on the same CPU, MUST be acquired with local interrupts disabled by the preemptible-context acquirer. The kernel-provided `without_interrupts(...)` wrapper is the canonical pattern; type-level enforcement is the subject of ADR-019 (see `docs/ADR-019-switchmutex-irq-off-by-construction.md`).
>
> Failing this discipline produces the **Hpw failure mode**: a preemptible-context holder is preempted by an interrupt; the same CPU then enters an IF=0 acquirer that spins on the lock; the preempted holder cannot resume because the IF=0 spinner monopolizes the CPU; the system is in a single-CPU self-deadlock that an SMP scheduler cannot route around. See `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 for the canonical analysis and the discipline's first instance.

### Footer change on ratification

Bumps bible to v1.5.0 (or whichever version is current at ratification time). The footer's `Last amended:` line records the 8.5.4 expansion.

## Compatibility with existing bible articles

- **Article 8.5 (parent)** — preserved unchanged. 8.5.4 is the specific sub-clause being amended.
- **Article 8.5.1 (Spinlock and interrupt discipline)** — preserved unchanged. 8.5.1 already addresses interrupt-context-vs-process-context locks; the new 8.5.4 wording addresses preempt-while-holding which is a different failure shape (same-CPU self-deadlock rather than two-context mutual exclusion).
- **Article 8.5.5 (Enforcement)** — preserved unchanged. Adding "structural enforcement via type system" is the subject of the separate ADR-019 proposal.
- **Article 15.2 (SAFETY comments)** — adjacent. The new discipline implies that any spinlock acquisition in preemptible context where the lock may be ISR-acquired or IF=0-acquired requires a SAFETY-comment-style justification, or use of the IrqSafeMutex type from the ADR-019 proposal.

## Implementation discipline

This amendment is descriptive (it names the discipline) rather than prescriptive new code. SC-3B Sub-step 5c (commit e2b3f4f) already implemented the discipline locally for the five affected `Channel.state` call sites. The amendment generalizes the rule kernel-wide; future code review enforces it.

The companion ADR-019 proposal proposes structural (type-system) enforcement, eliminating the need for case-by-case review. Without ADR-019 ratification, the discipline depends on manual review.

## Ratification decisions

- **Q1 (audit trigger or rely on 019):** Rely on ADR-019. Structural enforcement via `IrqSafeMutex` type makes manual audit unnecessary for the dual-context case. Plain `spin::Mutex` retention still requires SAFETY-comment-style justification at declaration site for non-dual-context uses (per ADR-019 §"Migration" guidance).
- **Q2 (inline Plans/ citation or only commit message):** Inline. The bible's new Article 8.5.4 cites `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 by path. The cross-reference is durable; if the cited file is ever superseded, the supersession marker in the target file (per Bible 24.2 reversibility discipline) handles the redirection.
- **Q3 (combine with ADR-019):** Yes — ratified together as linked decision. ADR-018 names the discipline; ADR-019 makes it un-bypassable. Both land in this dispatch's commits 1 and 2; ratification dates match.

## Cross-references

- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — canonical SC-3B closure record
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — the Phase D capture that localized Hpw
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus's prior-art research that confirmed Hpw as a known bug class
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — verification log for the per-call-site fix
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` — companion proposal for type-system enforcement
- `docs/bible.md` Article 8.5 (current state) — the article being amended

## Project-lead decision (2026-05-10)

Ratified as drafted; PM leans applied for all "Open questions for ratification" without explicit override. Three-decision package (ADR-018 + ADR-019 + ADR-020) approved as one ratification cycle; 018+019 linked, 020 independent.

---

**End of ADR.**
