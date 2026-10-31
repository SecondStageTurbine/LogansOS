# PL-RATIFICATION: Phase 2 OBSERVE scope-class items

- Recorded by: PM
- Date: 2026-05-24
- Ratifying authority: Project Lead
- Ratification form: "Yes to all" (this session, in response to four PM-recommended dispositions framed in `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` §4)
- Source artifact: `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` (`255372a`)
- Downstream consumers: Phase 2 PLAN THINK substrate; `Plans/PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md` (this commit); the eventual polished `PHASE-2-THINK-v0.1-2026-05-24.md` (Prometheus polish dispatch, this commit)

---

## 1. Ratified dispositions

The four scope-class items surfaced by Cerebro's Phase 2 OBSERVE (`97972e1`) and dispositioned with PM recommendation in the OBSERVE-close PM-DISPOSITION are ratified per PM's recommendation. Each is now binding for THINK and downstream PLAN/BUILD work.

### 1.1 §7.1 — Phase 2 PLAN shape

**Ratified: (c) both, with formal-close (a) first.**

Phase 2 PLAN produces a formal-completion BUILD set first — bare-metal multi-CPU verification, capability reserved-bit activation (sys_close / sys_wait / sys_inspect), audit (b)-disposition test absorption, Bible 16.4 perf-regression harness stand-up, and any defense-in-depth items (ADR-019 B/C, ADR-016 A2) that classify as Phase-2-conformance. Optimization items (Arc-aware slab, PCID/lazy-TLB) are explicitly NOT in Phase 2 close scope; they defer to (b) follow-on or fold into Phase 3 substrate where workload justification surfaces.

### 1.2 §7.4 — "Advanced TLB management" interpretation

**Ratified: correctness baseline satisfies Bible Article 13's "advanced TLB management."**

The shipped baseline (Q7.a GLOBAL bit + Q7.b CR3 cache + Q7.c IPI shootdown with INVLPG/full-flush hybrid) discharges Article 13's Phase-2 TLB workstream. Forward-marked optimizations (PCID, T2 targeted shootdown, lazy-TLB, batching, INVLPGB/RAR) stay project-self-classified as Phase-2+/Phase-3+ and are NOT pulled into Phase 2 close. If a specific optimization is later pulled forward, the right shape is a new ADR per item with measurement-trigger criteria — not a Phase 2 PLAN scope expansion.

### 1.3 §7.6 — PID recycling in Phase 2 scope

**Ratified: NOT in Phase 2 scope.**

`NEXT_PID: AtomicU64` remains monotonic. PID recycling and the associated stale-handle-revocation machinery (cap-id-based handle identity, revoke-on-death) are deferred to the phase where pid exhaustion becomes a real workload concern. ADR-007 Implementation note 6 stays open as a forward marker, not a Phase 2 deliverable.

### 1.4 §3.2 — ADR-016 A2 phase classification

**Ratified: A2 is Phase-2 conformance work.**

ADR-016's 2026-05-08 amendment (`1dbbfa7`) reframed A2 (scheduler decoupling from global heap — replace `VecDeque<Box<Thread>>` with bounded/pre-allocated storage) as Phase 2 conformance. The HANDOFF-2026-05-11 §10.2 "Phase 3 implementation" reading is superseded by the later ADR amendment. A2 implementation folds into Phase 2 close's defense-in-depth subset.

The HANDOFF doc carries stale phase-classification text; folding a correction into the existing Prometheus doc-reconciliation dispatch (`Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md`) is an option for the close report, but is not blocking and PM does not amend the dispatch unilaterally — flagged here for future visibility.

### 1.5 Q11 (Spectre-v2 / IBPB on context switch) — Phase 2 inclusion

**Ratified: NOT in the Phase 2 close THINK batch. Deferred to a dedicated ADR at a future trigger.**

Ratified per PM recommendation in `Plans/PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md` ("Tell-PL note: Q11" section, since removed by this DRAFT's amendment). Spectre-v2 mitigation is a security-deliverable cluster with its own threat-model considerations; Q7.b's conditional CR3 writes complicate IBPB placement enough that the work deserves its own design discussion rather than absorption into Phase 2 close.

Future trigger for revisit: a CPU-vulnerability event that elevates BTI urgency, a security ADR that takes IBPB-on-context-switch as part of broader CPU-vuln mitigation, or a measurement showing the unmitigated BTI surface is a near-term workload concern. Until one of those triggers, Q11 sits as a Phase-2.5-or-Phase-3 security-hardening ADR placeholder, not a Phase-2-close BUILD item.

---

## 2. Downstream effects on Cerebro's §5 question batch

Of Cerebro's 14 OBSERVE design-question seeds, the ratifications resolve or remove the following:

- **Q1 (close shape):** closed by §1.1 ratification. Removed from THINK batch.
- **Q5 (PID recycling stale-handle):** closed by §1.3 ratification. Removed from THINK batch.
- **Q7 (Arc-aware slab integration):** out of Phase-2 close scope per §1.1. Removed from THINK batch; preserved as substrate for any future (b) follow-on THINK.
- **Q10 (advanced TLB satisfaction bar):** closed by §1.2 ratification. Removed from THINK batch.
- **Q11 (Spectre-v2 / IBPB):** closed by §1.5 ratification (deferred to dedicated ADR). Removed from THINK batch.

Of the remaining 9, three are deferred as measurement-gated (Q6 CapId-SMP, Q8 buddy SMP contention, Q9 DerivationEntry slab) per the operational-posture principle that contention/fragmentation work follows measurement evidence, not preemptive design. These are noted in the DRAFT's traceability section but not present as active THINK questions.

**Surviving as THINK questions:** Q2 (sys_close), Q3 (sys_wait), Q4 (sys_inspect), Q12 (perf harness), Q13 (ADR-019 B/C), Q14 (bare-metal verification protocol), plus new Q-ADR016-A2 (A2 implementation shape per §1.4 ratification). Seven questions, renumbered Q1–Q7 in `PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md`.

---

## 3. References

- `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` (`255372a`) — source of the four scope-class items.
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) — origin of the 14 question seeds.
- `Plans/PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md` (this commit) — refined question substance per §2.
- `Plans/DISPATCH-2026-05-24-phase-2-think-polish-prometheus.md` (this commit) — Prometheus polish dispatch.
- Past pattern reference: `Plans/PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md` (`f0785c8`).
