# CLOSE REPORT: Phase 2 doc reconciliation (Prometheus)

- Author: Prometheus
- Date: 2026-05-24
- Dispatch: `Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md`
- Commit: `a796303`

---

## Deliverable

Three surgical edits across three files. Single commit.

## Amendment 1 — `docs/phases/PHASE-2.md` status header

**Before:**
```
| **Status** | In progress (BUILD-3 active; sub-commits 0, 1a, 3 (SC-3B), 4 (SC-3C), 5+6 (SC-4) closed) |
```

**After:**
```
| **Status** | BUILD-1/2/3 substantially complete (2026-05-11). Phase 2 formal close pending — see `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` for remaining work and scope-class items currently with PL. |
```

**Rationale:** BUILD-3 was declared substantially complete on 2026-05-11 (SC-4 close at `57f3cb1`). The header no longer misleads readers into thinking BUILD-3 is active. Does not declare "Phase 2 closed" — PL hasn't ratified that yet. Points at the live disposition artifact.

## Amendment 2 — `docs/ADR-019-switchmutex-irq-off-by-construction.md` implementation status

**Before:**
```
**Implementation status:** PENDING — sub-phases A, B, C scheduled in the forthcoming Jarvis kernel-rename bundle dispatch (sub-phase A) and follow-up Jarvis dispatches (sub-phases B and C).
```

**After:**
```
**Implementation status:** Sub-phase A landed at `27d1b99` (`IrqSafeMutex` type-enforcement at the wrapper layer). Sub-phases B (mixed-context `SwitchMutex` caller migration) and C (`Channel.state` `without_interrupts` site migration, manual-wrapper removal) pending — queued for Phase 2 close work per `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` §4.1 / §6.
```

**Rationale:** Sub-phase A landed at `27d1b99`. The "PENDING" status was stale. B and C are queued for Phase 2 close, not for a future Jarvis dispatch.

## Amendment 3 — `docs/ADR-013-phase-2-smp-retrofit.md` Q8.a supersession note

**Added** (new subsection between "Choice" and "Rationale" in Q8.a):

```
#### Superseded at SC-4 (commit `57f3cb1`)

Q8.a's `RwLock` selection was superseded by FM-D5.4's single-`Mutex` approach. Implementation at SC-4 D5 refactored `Thread` to carry `Arc<Pmap>` directly, removing PROCESSES from the SMP hot path entirely — the read-heavy contention that motivated `RwLock` no longer exists. PROCESSES remains correct with a single `Mutex` at NeoPlutarch's scale. See FM-D5.4 in `docs/phases/PHASE-2.md` and SC-4 close commit `57f3cb1` for the implementation truth.
```

**Rationale:** A reader of Q8.a in isolation would believe `RwLock` was the implemented choice. The supersession happened at SC-4 (FM-D5.4) but the Q8.a section carried no marker. The note redirects to the implementation truth without re-deriving FM-D5.4's rationale.

## Acceptance Criteria Verification

| Criterion | Status |
|-----------|--------|
| C1: PHASE-2.md status header updated, references PM-DISPOSITION | PASS |
| C2: ADR-019 status reflects sub-phase A landed at `27d1b99`, B/C pending | PASS |
| C3: ADR-013 Q8.a carries supersession marker → FM-D5.4, commit `57f3cb1` | PASS |
| C4: No semantic content beyond three amendments | PASS — surgical edits only |
| C5: Push to origin/main | PASS (`a796303`) |
| C6: Close report filed | This document |

## Open Questions

None. No additional staleness discovered in the three files beyond the three specified targets.
