# ADR Proposal: Bible Article 8.5.4 clarification — preempt-while-holding for mixed-IRQ-context locks

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.
**Tentative ADR number on ratification:** ADR-018 (next available; verify at ratification time).
**Bible article affected on ratification:** Article 8.5.4 (amended to explicitly name the preempt-while-holding case).

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
> Additionally — and this is the discipline that SC-3B's Hpw mechanism (commit e2b3f4f, 2026-05-10) was caused by violating: a spinlock acquired in preemptible context, where the same lock may also be acquired in IF=0 (interrupts-disabled) context on the same CPU, MUST be acquired with local interrupts disabled by the preemptible-context acquirer. The kernel-provided `without_interrupts(...)` wrapper is the canonical pattern; type-level enforcement is the subject of a separate proposal (see ADR-019 proposal at `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`).
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

## Open questions for ratification

- **Q1:** Should the amendment also name an audit trigger — e.g., "any new `spin::Mutex` declaration in a kernel module that has both ISR and process-context callers requires a SAFETY-style comment justifying why plain `spin::Mutex` is correct"? Or is type-system enforcement (ADR-019) the right answer instead of comment-based audit?
- **Q2:** Should `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 be cited inline in the bible (durable cross-reference) or only in the amendment commit message? Inline citation makes the bible self-contained but adds a Plans/ reference that may age.
- **Q3:** Should this amendment be combined with ADR-019 ratification into a single commit (since they're related), or land separately?

## Cross-references

- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — canonical SC-3B closure record
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — the Phase D capture that localized Hpw
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus's prior-art research that confirmed Hpw as a known bug class
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — verification log for the per-call-site fix
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` — companion proposal for type-system enforcement
- `docs/bible.md` Article 8.5 (current state) — the article being amended

## Project-lead decision required

Three options:
- **Ratify as drafted.** Bible amendment lands at next post-ratification commit; ADR-018 file lands in docs/.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Proposal stays in Plans/ as historical record; no bible amendment lands.

Decision goes in the amendment commit message + the ADR-018 status field on ratification.

---

**End of proposal.**
