# ADR-019: SwitchMutex IRQ-off by construction (type-level enforcement)

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Ratified 2026-05-10 by project-lead approval.
**ADR Number:** ADR-019.
**Linked to:** ADR-018 (declarative discipline; this ADR is the structural enforcement).
**Implementation status:** Sub-phase A landed at `27d1b99` (`IrqSafeMutex` type-enforcement at the wrapper layer). Sub-phases B/C landed in the 2026-05-25 Phase 2 BUILD subphase 3 dispatch: production `SwitchMutex` sites were audited, mixed-context list locks were migrated where the type fit, `Channel.state` moved to `IrqSafeMutex`, and state-only manual `without_interrupts(...)` wrappers were removed or narrowed.
**Related:** ADR-018 (companion declarative discipline; this ADR is the structural enforcement). Both ratified 2026-05-10 as a linked decision.
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

## Ratification decisions

- **Q1 (naming — IrqSafeMutex vs rename SwitchMutex):** Keep `SwitchMutex` as established convention. Create `IrqSafeMutex` as a sibling type alongside it. Consolidation deferred — if at sub-phase C the migration audit shows `SwitchMutex` and `IrqSafeMutex` would have identical semantics for all remaining callers, a follow-up ADR proposes the merge. Until then both names coexist with distinct roles.
- **Q2 (ban plain `spin::Mutex`):** No ban. Plain `spin::Mutex` remains available for locks that are declared to be exclusively-process-context. Declaration requires a SAFETY-comment-style justification at the lock's declaration site naming why no ISR or IF=0-process can acquire the lock. Compile-time enforcement is not added in this ADR; code-review enforcement remains the discipline.
- **Q3 (re-entrancy):** No re-entrancy. `IrqSafeMutex::lock()` from the same thread that already holds the lock is undefined behavior (typically: spin forever, since `spin::Mutex` underlying primitive is non-re-entrant). Re-entrancy hides bugs; callers structure their critical sections to not nest.
- **Q4 (perf bench):** Yes. Sub-phase A includes a micro-bench measuring `IrqSafeMutex::lock()` + `drop` cycle cost vs. plain `spin::Mutex::lock()` + drop. The bench is run on the project's standard QEMU configuration; numbers go into the sub-phase A verification log. If the overhead exceeds 50ns per acquire/release pair on the standard config, escalate to PM before sub-phase B.
- **Q5 (combine with ADR-018):** Yes — ratified together as linked decision. ADR-018 names the discipline; this ADR is the un-bypassable structural enforcement. See ADR-018 §"Ratification decisions" Q3.

## Cross-references

- `docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md` — companion declarative discipline proposal
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record establishing the Hpw mechanism
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — Sub-step 5c verification (the per-call-site fix that this proposal would make structural)
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — prior art on release-on-switch lock primitives (Asterinas, Redox `switch_finish_hook`)
- `docs/bible.md` Article 8.5 — critical-section discipline

## Project-lead decision (2026-05-10)

Ratified as drafted; PM leans applied for all "Open questions for ratification" without explicit override. Linked-ratification with ADR-018 (both land in this dispatch's commits 1 and 2). Implementation scheduled for the forthcoming Jarvis kernel-rename bundle: sub-phase A (introduce IrqSafeMutex type) lands alongside the kernel rename and ADR-020 LockOwner implementation; sub-phases B (migrate SwitchMutex mixed-context callers) and C (migrate Sub-step-5c-wrapped sites) follow as separate Jarvis dispatches with their own gates.

## Amendment 2026-05-25 - Sub-phases B/C landed

Phase 2 BUILD subphase 3 completed the ADR-019 B/C migration scope:

- `Channel.state` is now `IrqSafeMutex<[EndpointState; 2]>`.
- `IrqSafeMutex` delegates to `SwitchMutex` internally and exposes a release token for the IPC block path, preserving the context-switch handoff invariant while making IF-off acquisition structural.
- State-only IPC `without_interrupts(...)` wrappers were removed. Wrappers retained around syscall IPC bodies and scheduler wake/zombify phases cover broader handoff or scheduler-queue critical sections, not `Channel.state.lock()` itself.
- Production `SwitchMutex` declarations were audited. Mixed process/interrupt list locks for AHCI and virtio block devices moved to `IrqSafeMutex`; remaining `SwitchMutex` declarations are either process-context only or scheduler/context-switch release-token special cases and now carry declaration-site comments.
- Verification included `cargo k`, `irqsafe_mutex_if_restore`, `irqsafe_mutex_bench`, exact `INV-IPC-5` scenario `ipc_rendezvous_translate_fault`, prior subphase wait/inspect/cap/scheduler gates, a 200/200 `NEO_SMP=4` `smp_cross_cpu_rendezvous_stress` loop, and full `cargo neo-test all` at `NEO_SMP=4`.

Close evidence is recorded in `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md`.

---

**End of ADR.**
