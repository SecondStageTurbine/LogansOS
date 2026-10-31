# ADR Proposal: SwitchMutex IRQ-off by construction (type-level enforcement)

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.
**Tentative ADR number on ratification:** ADR-019 (next available after ADR-018; verify at ratification time).
**Related:** ADR-018 proposal at `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` (companion declarative discipline; this proposal is the structural enforcement).
**Bible articles potentially affected:** Article 15 (Code Standards) may gain a clause about lock-type discipline; Article 8.5.5 (Enforcement) updated to reference type-system enforcement.

---

## Rationale

SC-3B Sub-step 5c (commit e2b3f4f, 2026-05-10) implemented IRQ-safe acquisition as a per-call-site discipline for `Channel.state`. The discipline works — gate scores went from intermittent MISSING_TERMINAL_FRAME to 200/200 PASS — but it is enforced only by code review.

Any future code path that acquires a mixed-IRQ-context lock from preemptible context without `without_interrupts(...)` re-introduces the Hpw bug class. The ADR-018-proposed bible amendment names the discipline; this proposal makes it un-bypassable in safe Rust.

The proposal: introduce (or rename to) an `IrqSafeMutex<T>` type that disables local interrupts on acquire and restores prior interrupt state on drop. Plain `spin::Mutex<T>` remains available for locks that are exclusively process-context or exclusively ISR-context. The type system distinguishes the two cases, and a mixed-IRQ-context lock that uses plain `spin::Mutex` is a type error (or a documented exception requiring SAFETY-comment-style justification).

## Decision context

### Current state

NeoPlutarch uses `SwitchMutex` for scheduler-protecting locks and `spin::Mutex` (from the `spin` crate) for general-purpose locks. Some `spin::Mutex` instances protect data that is touched in mixed-IRQ contexts (like `Channel.state` was before Sub-step 5c).

There is no current type-level distinction between:
- (a) Locks that are exclusively process-context (no ISR touches them; no IF=0 acquirer)
- (b) Locks that are exclusively ISR-context (no process-context acquirer)
- (c) Locks that are mixed-IRQ-context (both process-context and ISR or IF=0 process-context acquirers; the dangerous case)

Hpw applies only to category (c). Category (a) and (b) locks are safe with plain `spin::Mutex`.

### Proposed state

Three distinct lock types:

- **`spin::Mutex<T>`** (unchanged) — for locks that the discipline declares to be exclusively-process-context. Use requires a documented SAFETY-comment-style justification at the declaration site naming why no ISR or IF=0-process can acquire this lock.
- **`IrqLocalMutex<T>`** (new or renamed) — for locks that are exclusively ISR-context. Acquisition assumes IF is already off (per IRQ context). Same primitive as `spin::Mutex` in mechanics, distinct in type to make the invariant visible.
- **`IrqSafeMutex<T>`** (new or renamed `SwitchMutex`) — for locks that may be acquired from BOTH process and IF=0 contexts. Acquisition disables local interrupts and saves prior IF state in the guard; guard drop restores prior IF state. Cannot be misused: the type guarantees IRQ-off-on-acquire.

### Migration

Existing `SwitchMutex` callers are audited. Each is classified into one of the three categories. The bulk are expected to become `IrqSafeMutex` (the canonical mixed-context case). Any that turn out to be exclusively-process-context move to `spin::Mutex` with SAFETY-comment justification.

`Channel.state` and similar `spin::Mutex` instances that turn out to be category (c) are converted to `IrqSafeMutex`. This removes the per-call-site `without_interrupts(...)` wrapping that Sub-step 5c added — the discipline becomes structural.

## Concrete proposal

### API sketch

```rust
// kernel/src/sync/irqsafe_mutex.rs (new file or rename of existing switch_mutex.rs)

pub struct IrqSafeMutex<T> {
    inner: spin::Mutex<T>,
    // existing ownership-metadata fields (if ADR-020 ratifies, see Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md)
}

pub struct IrqSafeGuard<'a, T> {
    guard: spin::MutexGuard<'a, T>,
    prior_if_state: bool,
}

impl<T> IrqSafeMutex<T> {
    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        let prior_if_state = read_interrupt_flag();
        disable_local_interrupts();
        let guard = self.inner.lock();
        IrqSafeGuard { guard, prior_if_state }
    }
}

impl<T> Drop for IrqSafeGuard<'_, T> {
    fn drop(&mut self) {
        // guard.drop() releases inner spinlock first, then we restore IF
        if self.prior_if_state {
            enable_local_interrupts();
        }
    }
}
```

The above is sketch-level. Real implementation needs:
- Correct ordering of `enable_local_interrupts` and `inner` guard drop (the asm sequence matters; this is exactly the territory where SC-3A and SC-3B's release-on-switch concerns lived).
- IPI-vs-local-interrupt distinction if relevant.
- Re-entrancy semantics (does `IrqSafeMutex::lock()` work if interrupts are already disabled by an outer acquirer? Probably yes, and the prior-IF tracking handles it automatically).
- Interaction with the panic path (if a panic happens with the guard held, prior IF state should be restored).

### What this removes

Once `IrqSafeMutex` lands and `Channel.state` is migrated:
- The five `without_interrupts(...)` wrappers Sub-step 5c added become redundant. They can be removed in the migration commit.
- Future code touching `Channel.state` (or any other migrated lock) cannot reintroduce Hpw — the type system enforces IRQ-off-on-acquire.

### What this leaves open

- Locks that are exclusively-process-context still need discipline (SAFETY-comment-style justification at declaration) to prevent accidental ISR acquisition that would re-introduce the dual-context case.
- The discipline still applies kernel-wide; this proposal addresses the structural enforcement for the mixed-context case specifically.

## Implementation phases

If ratified, implementation could land in three sub-phases:

1. **Sub-phase A:** Introduce `IrqSafeMutex` type alongside `SwitchMutex` (no migration yet). Verify build, basic tests.
2. **Sub-phase B:** Migrate `SwitchMutex` callers that are mixed-IRQ-context to `IrqSafeMutex`. Audit each.
3. **Sub-phase C:** Migrate Sub-step 5c's `without_interrupts(...)`-wrapped `spin::Mutex` callers (Channel.state and similar) to `IrqSafeMutex`. Remove the wrappers.

Each sub-phase is a separate commit with its own gate (200/200 NEO_SMP=4 closure-gate equivalent). Total cost is moderate: maybe 1-2 weeks of focused work depending on the migration audit's scope.

## Open questions for ratification

- **Q1:** Naming. Is `IrqSafeMutex` the right name, or should `SwitchMutex` be renamed to it? `SwitchMutex` is established convention; renaming has churn. PM lean: keep `SwitchMutex` and create `IrqSafeMutex` as a sibling; consolidate later if reasonable.
- **Q2:** Should plain `spin::Mutex` be banned from kernel code (replaced everywhere by one of the three typed mutexes), or remain available with a justification requirement?
- **Q3:** Re-entrancy semantics — should `IrqSafeMutex` be re-entrant from the same thread? PM lean: NO, re-entrancy hides bugs. Throw the entire critical section into the inner spinlock; the outer caller is responsible for not nesting.
- **Q4:** Performance — IRQ-off-on-every-lock has a small cost (one read + two writes to RFLAGS, plus possible cache effects). Quantify before committing? PM lean: probably negligible in practice; verify with a micro-bench during sub-phase A.
- **Q5:** Sequencing — should this ratify alongside ADR-018, or land separately? PM lean: ratify together since they're conceptually linked.

## Cross-references

- `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` — companion declarative discipline proposal
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record establishing the Hpw mechanism
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — Sub-step 5c verification (the per-call-site fix that this proposal would make structural)
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — prior art on release-on-switch lock primitives (Asterinas, Redox `switch_finish_hook`)
- `docs/bible.md` Article 8.5 — critical-section discipline

## Project-lead decision required

Three options:
- **Ratify as drafted.** ADR-019 file lands in docs/; implementation work begins per the three-sub-phase plan.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Proposal stays in Plans/ as historical record; the per-call-site discipline remains the only enforcement mechanism.

Decision goes in the ratification commit message + the ADR-019 status field on ratification.

---

**End of proposal.**
