# PM-NOTE — SC-3C D-decision revisions ratified post-prior-art

**Date:** 2026-05-10 (later session, post-D15 research close)
**Type:** PM-NOTE. Operational ratification record. Doc-only artifact.
**Authority:** PM, per the PM operational posture clarification recorded 2026-05-10 in memory (PL delegated operational/technical D-decisions to PM; PL-tier reserved for ratifications of legitimately PL-tier calls — bible amendments, scope, major architectural pivots). PL explicitly handed this call to PM with "Go ahead, your call" after reviewing the prior-art research summary.
**Scope:** Closes the D-decision loop for SC-3C THINK-stage. Consumes `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` and revises the D-table in `Plans/SC-3C-THINK-2026-05-10.md` §6 accordingly. No code changes; no kernel touches; no bible/ANSPEC/ADR mutation.

**STATUS: FINALIZED 2026-05-11 by Cerebro independent verification.** Cerebro's deep-research pass (`Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`, 39.7 KB, filed 2026-05-11) confirmed all four provisional D-revisions: D3 (N=16→N=33), D4 (T1 broadcast stays for SC-3C BUILD; API takes target-set; T2 upgraded to Phase 2+ forward optimization), D9 (FreeBSD `curcpu_cb` overlap pattern), and D14 (missing-shootdown-detection scenario). No D-decision overturns. Seven citation/factual corrections to the PM-preliminary evidence base were captured for SC-3C PLAN-stage consumption in `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`. See that consumption note for the full finalization record; see `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` for the original role-confusion calibration that prompted this dispatch.

---

## 1. Context

SC-3C THINK artifact (filed earlier this session) surfaced fifteen D-decisions (D1–D15) with PM leans documented. D15 was a deep-research dispatch on TLB shootdown / CR3 cache prior art; it closed with `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` (50.8 KB, ~75 min wall-clock).

The research artifact's §4 synthesis surfaced four changes against the PM leans:

1. D3 (INVLPG-vs-full-flush threshold): N=16 → N=33 to match Linux's measured `tlb_single_page_flush_ceiling`.
2. D9 (self-invalidation ordering): serial-before-IPI → FreeBSD `curcpu_cb` overlap pattern.
3. D14 (verification gate scenarios): add explicit missing-shootdown-detection scenario inspired by FreeBSD-EN-18:07 corruption-class evidence.
4. D4 (target selection): T1 broadcast remains the SC-3C BUILD-stage lean, but T2 (per-Pmap CPU mask) upgrades from "deferred indefinitely" to "Phase 2+ documented forward optimization." Implementation note: SC-3C API takes the target CPU set as an input parameter so a future T2 migration is a caller-side change, not an API break.

PL response: "Go ahead, your call." PM-tier ratification authority engaged.

## 2. Ratified

All four changes ratified by PM. Reasoning:

**Why ratify rather than defer:**
- All four are empirically grounded in production prior art (Linux measured threshold, FreeBSD measured API pattern, FreeBSD-EN-18:07 silent-data-corruption evidence), not speculative.
- None are correctness-blocking against the original PM leans; they refine toward measured optima.
- None change SC-3C BUILD-stage scope, sub-commit boundaries, or dependency surface.
- D14 in particular is correctness-adjacent: a verification gate capable of catching the FreeBSD-EN-18:07-class missing-shootdown bug is meaningfully better than one that exercises only the happy path. The cost is one extra test scenario; the benefit is catching a class of silent-corruption bugs the original D14 spec would not have caught.
- D4's forward-marker upgrade is pure documentation — the SC-3C API was always going to take a target set internally; making that explicit at the API surface costs nothing and saves API churn during a future T2 migration.
- D3's N=16 → N=33 reduces calibration churn: less likely we revisit the threshold post-BUILD just because measurement converges toward Linux's empirically grounded number.
- D9's `curcpu_cb` pattern is a small performance win at NEO_SMP=4 (overlap window is narrow) but has zero implementation-complexity cost relative to the serial-before-IPI alternative — both shapes are similarly sized; the overlap shape is just structured differently.

**Why this is PM-tier and not PL-tier:**
- No bible/ANSPEC/ADR mutation.
- No phase-roadmap change.
- No major architectural pivot.
- No new dependency or scope expansion.
- Operational refinement of D-decisions surfaced by Cerebro deep research — exactly the territory the 2026-05-10 PM operational posture clarification specified as PM-owned.

## 3. Resulting D-state for PLAN-stage drafting

Authoritative state of D1–D15 post-ratification. The SC-3C THINK artifact's §6 D-table is **not edited in place** (history preservation); this PM-NOTE is the new authoritative record for D-state going into PLAN-stage dispatch drafting.

| # | Decision | Ratified state | Source of decision |
|---|---|---|---|
| D1 | Q7.b CR3 cache scope | B2.a (compare-and-skip, CR3-write-then-field-store ordering) | THINK PM lean; corroborated by Asterinas #799 |
| D2 | PCID enablement | Defer to future ADR-driven sub-commit | THINK PM lean; corroborated by Linux global-ASID complexity scan |
| **D3** | **Q7.c shootdown granularity** | **G3 hybrid INVLPG-with-threshold, N=33** (revised from N=16) | **Research §3.1: Linux's `tlb_single_page_flush_ceiling = 33` is the measured production threshold** |
| **D4** | **Q7.c target selection** | **T1 broadcast for BUILD; API takes target-set as input parameter; T2 (per-Pmap cpumask) documented as Phase 2+ forward optimization** | **Research §3.2: FreeBSD `smp_targeted_tlb_shootdown(cpuset_t mask, ...)` API shape; Linux's measured T2 dominance at higher CPU counts** |
| D5 | Q7.c synchrony | S3 synchronous with 1ms bounded wait + panic-on-timeout | THINK PM lean; no prior-art comparator for the 1ms specifically — treat as Phase 2 calibration parameter |
| D6 | Lazy TLB | L1 none; receiver-side short-circuit when added later (no API impact) | THINK PM lean; corroborated by Linux's receiver-side `should_flush_tlb` design |
| D7 | Shootdown batching | B1 none; deferred | THINK PM lean; Linux `mmu_gather` is significant added complexity |
| D8 | IPI vector for shootdown | 0xfc | THINK PM lean; no prior-art constraint |
| **D9** | **Self-invalidation ordering** | **FreeBSD `curcpu_cb` overlap pattern — local invalidation runs as a callback during the remote-wait loop, not serially before IPI dispatch** | **Research §3.2: FreeBSD's measured production design** |
| D10 | ACK mechanism | Per-shootdown `AtomicU32` counter | THINK PM lean; FreeBSD's generation-counter alternative is functionally equivalent but adds wraparound complexity |
| D11 | Per-CPU shootdown slot | α single slot (AtomicPtr) | THINK PM lean; no directly comparable prior-art design |
| D12 | Sub-commit boundaries | 4a (Q7.b) → 4b (Q7.c machinery, no callers) → 4c (Q7.c wiring into Pmap mutation sites) | THINK PM lean; Bible-15 small-commit discipline |
| D13 | IrqSafeMutex dependency | Bridge via `without_interrupts(...)` if not landed at BUILD time | THINK PM lean |
| **D14** | **Verification gate scenarios** | **200/200 NEO_SMP=4 + concurrent munmap + process-exit-during-thread-run + same-process context-switch micro-bench + missing-shootdown-detection scenario** (CPU A unmaps page; CPU B reads same virtual address; without correct shootdown B sees stale physical page; gate asserts B faults) | **Research §3.2: FreeBSD-EN-18:07 silent-data-corruption anchor** |
| D15 | Deep-research dispatch | CLOSED. Research artifact filed at `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md`. | THINK PM lean; executed this session |

**Summary of post-ratification state:**
- D3 revised (N=16 → N=33).
- D4 unchanged in BUILD-stage shape; API surface clarified to take target-set as input parameter; T2 upgraded from indefinite-defer to Phase 2+ documented forward optimization.
- D9 revised (serial-before-IPI → FreeBSD overlap).
- D14 revised (verification gate gains missing-shootdown-detection scenario).
- D15 closed.
- D1, D2, D5, D6, D7, D8, D10, D11, D12, D13 stand unchanged.

## 4. Forward markers (for Phase 2+ tracking)

Items the research artifact §5 surfaced for later tracking. None blocks SC-3C; all become interesting at later inflection points. Recommend adding to PHASE-2.md or a dedicated forward-markers document during the next PM doc sweep:

1. **PCID / ASID enablement** — interesting when TLB-miss cost dominates measurement or when many concurrent processes exceed TLB natural capacity.
2. **T2 targeted shootdown (per-Pmap CPU mask)** — interesting when NEO_SMP scales beyond 4 or when broadcast IPI cost dominates.
3. **Lazy TLB (receiver-side short-circuit)** — interesting once NeoPlutarch has many idle CPUs or many kernel threads.
4. **Shootdown batching (mmu_gather analogue)** — interesting if profiling shows shootdown-per-unmap is a workload bottleneck.
5. **Early-ACK pattern (USPTO 11321242)** — micro-optimization; file-for-later.
6. **AMD INVLPGB / Intel RAR hardware-broadcast** — Phase 3+ forward marker; gate is NEO_SMP > 4 or measurement-driven.
7. **Spectre-v2 BTB flush on context switch (IBPB analogue)** — Phase 2+ CPU-vulnerability mitigation cluster; deserves its own ADR; not SC-3C territory.
8. **ARM port intermediate-page-table flush hook (`flush_tlb_pgtable` pattern)** — Phase 5+ marker for aarch64 port.
9. **Asterinas SMP TLB design tracking** — closest architectural reference; mid-flight on the same problem set through 2025–2026; watch their repo for design choices as cross-validation source.

## 5. Resulting state for SC-3C PLAN-stage drafting

PLAN-stage Jarvis dispatch is now unblocked from a D-decision perspective. The remaining gate before PLAN-stage dispatch:

- **Jarvis kernel-rename bundle close.** Bundle is in flight. SC-3C PLAN-stage waits for that bundle to either land cleanly or stall — whichever; the SC-3C dispatch will be drafted with full knowledge of post-rename kernel state, including (importantly) whether `IrqSafeMutex` landed in sub-commit 3 of the bundle (which determines whether D13's `without_interrupts` bridge applies or whether SC-3C handler code can use `IrqSafeMutex` directly).

When Jarvis bundle closes:

1. PM drafts `Plans/DISPATCH-2026-05-MM-sc-3c-q7-tlb-shootdown-jarvis.md` per Bible-14 PLAN-stage convention.
2. Three sub-commits per D12: 4a (Q7.b) → 4b (Q7.c machinery) → 4c (Q7.c wiring).
3. Each sub-commit with a verification gate matching D14 (with the missing-shootdown-detection scenario added per ratification).
4. PM negative-list discipline applied: dispatch enumerates the file regions Jarvis can touch and explicitly out-of-scope items (no PCID changes, no T2 cpumask infrastructure, no lazy TLB, no batching).
5. PM review intensity calibrated per the second-Jarvis-dispatch-under-Codex calibration (first was the kernel-rename bundle, in flight; data point not yet in hand).

PM wall-clock estimate for PLAN-stage drafting: ~2-3 hr. Jarvis wall-clock estimate for SC-3C BUILD: 6-10 hr total across the three sub-commits per THINK §7.

## 6. Cross-references

**This PM-NOTE consumes:**
- `Plans/SC-3C-THINK-2026-05-10.md` — original THINK artifact with PM leans D1–D14.
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` — D15 research artifact surfacing the four changes.

**This PM-NOTE will be consumed by:**
- The forthcoming `Plans/DISPATCH-2026-05-MM-sc-3c-q7-tlb-shootdown-jarvis.md` (drafted after Jarvis kernel-rename bundle closes).

**Operational discipline anchors applied:**
- PM operational posture clarification 2026-05-10 (memory): PM-owned operational decisions; don't over-surface micro-decisions; ferry on.
- Bible Article 1.2 (correctness > coherence > performance): ratified changes are correctness-adjacent (D14) or correctness-neutral performance refinements (D3, D9).
- Bible Article 4.3 (every dependency a liability): D4 forward-marker upgrade is documentation-only and reduces future API churn; no new dependency introduced.

**Voice anchors:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` — current PM-NOTE format and authority discipline.
- `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — local-only by default; this PM-NOTE lands local-only, no push event triggered.

---

## 7. Closure

SC-3C D-decisions are ratified. SC-3C PLAN-stage drafting is gated only on Jarvis kernel-rename bundle close. No further THINK or research work needed on SC-3C before PLAN-stage.

PM moves to standby. Next active action is one of:
- Jarvis bundle reports back → PM reviews verification logs → if clean, push the bundle and immediately draft SC-3C PLAN-stage dispatch from clean state.
- Adjacent work (HANDOFF audit, AI-native-OS field survey, etc.) if Jarvis is still in flight.
- End-of-session checkpoint if no adjacent work is queued.

---

**End of PM-NOTE.**

*Four D-revisions ratified by PM operational authority. D3 N=16→N=33. D4 API takes target-set as input parameter; T2 upgraded to Phase 2+ forward optimization. D9 serial-before-IPI → FreeBSD `curcpu_cb` overlap pattern. D14 verification gate gains missing-shootdown-detection scenario per FreeBSD-EN-18:07 anchor. Eleven D-decisions stand. SC-3C PLAN-stage drafting unblocked from D-decision perspective; gated only on Jarvis kernel-rename bundle close.*
