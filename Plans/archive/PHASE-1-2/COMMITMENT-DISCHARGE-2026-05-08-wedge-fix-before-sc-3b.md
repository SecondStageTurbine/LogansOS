# Commitment Discharge: wedge-fix-before-SC-3B

| | |
|---|---|
| **Type** | Project commitment discharge record |
| **Status** | Met 2026-05-08 |
| **Authority** | Project Lead |
| **Author** | PM |
| **Affects** | `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` (the original commitment) |

---

## Commitment

Per `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`: "Test-runner intermittent wedge fixed before sub-commit 3b ships." Authority structure: PM cannot re-defer; project lead can re-defer only via ADR or amendment.

## Discharge

The commitment is **discharged 2026-05-08** by the combination of:

- **`4c6abf4`** — `sched: prevent heap-lock re-entry in reap zombie drain` (Family L, "localized fix"). Closes the proven WEDGE-INV-J1 mechanism (`POST_SMOKE_SMP_SCENARIO`). Verified 300/300 across `smp_per_cpu_scheduler_smoke` (200/200), `cap_revoke_local` (50/50), and `cap_survive_exit` (50/50) at NEO_SMP=4. Zero historical-signature timeouts. Verification report at `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md`.

- **`aac0c11`** — `mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)`. Wraps the global heap with an `IrqSafeHeap` shim using `spin::Mutex` + `without_interrupts(...)` around init/alloc/dealloc. Closes the broader heap-lock-re-entry class structurally at the heap layer; same-CPU deadlock from any process/ISR contention on the global heap is impossible by construction.

## Statistical context

At the historical pre-fix rate of 2.67% for `POST_SMOKE_SMP_SCENARIO` (4/150 in WEDGE-INV-3+4 baselines), P[0 timeouts in 200 boots | rate unchanged] ≈ 0.45%. A clean 200/200 is therefore strong evidence the proven mechanism is closed. The supplementary 100/100 across two additional scenarios broadens coverage to other historical signatures. WEDGE-INV-4 had identified four historical signatures total; the supplementary scenarios exercise paths beyond just `smp_per_cpu_scheduler_smoke`, providing some coverage of the other three signatures' likely loci.

The 300/300 figure is an upper-bound point estimate, not a proof of zero. A latent very-low-rate remnant of the broader class is not statistically excluded. A1's structural backstop addresses this: even if a residual same-CPU heap-lock-re-entry path exists on a code path the audit didn't enumerate, A1 prevents the deadlock from materializing.

## What this discharge does NOT cover

The commitment named "wedge fix before SC-3B." It does NOT cover:

- **Bible 8.5.3 structural conformance.** Article 8.5.3 ("Scheduler heap-independence") stands as binding policy with A2 as the conformance milestone. Conformance lands at Phase 2 alongside the buddy+slab allocator work, per the 2026-05-08 amendment to ADR-016. Bible has well-established phased-conformance precedent (Articles 7.5, 8.1).
- **A2 implementation.** Reframed to Phase 2 work per the same ADR-016 amendment.
- **Future heap-lock-re-entry classes from new ISRs.** Bible 8.5.2 forbids new ISR allocations as project policy. A1 provides defense-in-depth even if 8.5.2 is later violated.

## Forward path

SC-3B work is unblocked. Project lead can begin SC-3B planning whenever ready. The wedge-fix-before-SC-3B row migrates from "Open Project Commitments" to a new "Met Commitments" subsection in `Plans/INDEX.md` per the existing maintenance pattern (the section header explicitly contemplates this transition).

## Cross-references

- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — original commitment text.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — empirical evidence (300/300).
- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — bug analysis identifying the proven mechanism.
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` (Amendment 2026-05-08) — A2 reframed to Phase 2.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — project-state record of the β execution this discharge is part of.
- Bible Article 8.5 — the discipline that A2 will conform to at Phase 2.
- Commits `4c6abf4` (L) and `aac0c11` (A1) — the discharge basis.

---

**End of discharge record.**

*Wedge fix met before SC-3B. SC-3B unblocked. A2 scheduled for Phase 2 conformance work.*
