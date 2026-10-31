# DISPATCH — SC-3C D15 deep-research: TLB shootdown / CR3 cache prior art (Cerebro)

**Date filed:** 2026-05-10 (later session)
**From:** PM
**To:** Cerebro (Claude, deep-research lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3)
**Ferry:** PL ferries this dispatch from PM session to a fresh Cerebro session.
**Status:** First Cerebro deep-research dispatch under the new agent roster. No Cerebro-research voice profile exists yet; per agent-roster §3.2, PM specifies output structure explicitly in this dispatch.

---

## 1. Mandate

Conduct a deep-research prior-art survey on cross-CPU TLB invalidation and per-CPU CR3 caching for SMP x86_64 monolithic kernels. This is the D15 row of the SC-3C THINK artifact's D-decision table (`Plans/SC-3C-THINK-2026-05-10.md` §6); D15 was originally written as "Cerebro deep-research dispatch" but was executed by PM directly (a role-confusion error). This dispatch re-executes the D15 work in the correct lane.

The deliverable is a research artifact at `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` that PM consumes to confirm, extend, or correct the four D-revisions ratified in `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`.

## 2. Context for Cerebro (read these first, in order)

**Authoritative inputs:**

1. **`Plans/SC-3C-THINK-2026-05-10.md`** — the SC-3C THINK-stage artifact with fifteen D-decisions (D1–D15) and PM leans documented. §6 contains the D-table; §3 and §4 contain Q7.b and Q7.c design-space enumeration; §5 contains cross-cutting risks. This is PM's situational read of the problem; it is *not* the prior-art research itself.

2. **`Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md`** — PM's first-pass prior-art synthesis, executed before this dispatch existed and acknowledged as preliminary. Treat as a *starting reference*, not as authority. Cerebro's job is to confirm what's correct, extend what's incomplete, and correct what's wrong. **Disagreement with this artifact is fully welcome and is valuable calibration data.**

3. **`Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`** — PM's provisional ratification of four D-revisions (D3 N=16→N=33, D4 forward-marker upgrade, D9 FreeBSD overlap pattern, D14 missing-shootdown-detection scenario). Currently marked PROVISIONAL pending Cerebro's pass. Cerebro should treat these as PM's read; final ratification depends on whether Cerebro's findings support them.

**Voice anchor for output style and rigor:**

4. **`Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`** — the Janus voice anchor. Cerebro's output artifact should match this format: explicit "Sources surveyed" section with primary/secondary distinction, per-system findings sections with bold lead-in claims and quoted source text, calibration notes acknowledging unverified claims, explicit cross-references. The Janus artifact is the rigor target; Cerebro is free to develop its own voice within that frame, but the citation-discipline anchor and section structure should match.

**Calibration discipline anchor:**

5. **`Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`** — the citation-rigor calibration discipline. Every cited URL spot-checked; specific path/line anchors where possible; future-dated arXiv IDs flagged honestly; speculative claims clearly marked. Cerebro should self-audit per this discipline before declaring the artifact final.

## 3. Research scope

**Required coverage — five reference systems:**

- **Linux kernel** — `arch/x86/mm/tlb.c` evolution. Specifically: `flush_tlb_mm_range`, `flush_tlb_kernel_range`, `should_flush_tlb`, `switch_mm_irqs_off`, `tlb_single_page_flush_ceiling`, `mm_cpumask`, `cpu_tlbstate.is_lazy`, `consider_global_asid`, `broadcast_flush_tlb_all`, the AMD INVLPGB mainline integration (Linux 6.15, Feb 2025, Rik van Riel patch series), and the Spectre-v2 `cond_ibpb` BTB-flush interaction.
- **FreeBSD** — `smp_targeted_tlb_shootdown` API, the `cpuset_t mask` / `curcpu_cb` callback pattern, `smp_tlb_generation` counter, and the FreeBSD-EN-18:07 (2018) silent-data-corruption advisory as the canonical "missing TLB shootdown = silent corruption" anchor.
- **seL4** — TLB invalidation through capability operations vs. direct syscalls; the verification-first design's avoidance of multi-CPU concurrency; the 2019 sel4-devel mailing list note on x86_64 SMP TLB limitations; X86 ASID pool design.
- **Asterinas** — issue #799 (April 2024) framing of the same Q7.b problem NeoPlutarch is solving; 2024 retrospective committing to 2025 SMP optimization; ATC25 paper evaluation at SMP=1; ongoing 2025–2026 SMP TLB work in the repository.
- **AMD INVLPGB hardware-broadcast** — Zen 3+ ISA, Linux 6.15 mainline integration, the >3-CPU-active threshold for global-ASID assignment, Intel RAR follow-up plan. Treat as Phase 3+ forward marker for NeoPlutarch, not as SC-3C scope.

**Secondary / informational coverage (optional but encouraged):**

- Redox OS Rust microkernel TLB handling (relevant Rust prior art).
- Theseus OS cell-swapping interaction with TLB invalidation.
- USPTO patents 11321242 (early-ACK shootdown) and 12253956 (hybrid IPI/TLBI dispatch) as design-space markers, not authority.
- Recent CPU-vulnerability-driven TLB work (PTI / Meltdown response, KAISER / KPTI legacy effects).

**Out of scope:**

- AI-native OS field-landscape work (that's a separate dispatch: `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md`).
- PCID enablement (Bible 1.2 + project decision; deferred from SC-3C per D2; survey may mention but should not recommend).
- aarch64 / ARM64 TLB invalidation patterns (Phase 5+ forward marker; survey may mention but should not deep-dive).

## 4. Required output structure

Output file: **`Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`**.

Note: this filename does NOT conflict with the renamed PM-preliminary artifact (`Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md`); the two are distinct artifacts with distinct provenance.

Sections (in order):

1. **Header** — date, author (Cerebro), type (research artifact, informational, not normative), authority (none), voice anchor (Janus artifact at §2 input #4).
2. **TL;DR** — 3–6 paragraphs synthesizing the headline findings against the five reference systems. Bold lead-in claims; concrete numbers where supported by source.
3. **Sources surveyed** — primary sources consulted directly (with URLs, specific paths, and what was read from each); secondary / community / patent references separated; future-marker references separated.
4. **Findings by reference system** — one subsection per system in §3 above. Each subsection: bold lead-in claim, quoted source text where load-bearing, specific path/line anchors where verifiable, calibration notes when claims rest on indirect evidence.
5. **Synthesis against SC-3C D-decisions** — table or per-D-decision treatment. For each D-decision (D1–D14), state whether Cerebro's findings *confirm*, *extend*, or *contradict* the PM-lean / PM-revision. Where Cerebro diverges from PM, document the divergence explicitly with citation support.
6. **Forward markers** — items the survey surfaced that NeoPlutarch should track but that are out of scope for SC-3C (PCID, T2 cpumask, lazy TLB, batching, INVLPGB/RAR, Spectre-v2 mitigations, ARM port).
7. **Calibration notes** — explicit acknowledgment of verified vs. unverified vs. speculative claims; future-dated arXiv IDs flagged; honest acknowledgment of any single-source claims; research-pass time budget actual.
8. **Cross-references** — what this artifact consumes (the §2 inputs), what consumes this artifact (PM's follow-up confirmation or divergence-handling).
9. **Closure paragraph** — single paragraph stating whether Cerebro's findings support PM's provisional D-revisions (D3, D4, D9, D14) or diverge, with one-line summary per divergence.

## 5. Citation rigor expectations

Per the Janus calibration discipline:

- Every URL cited must be reachable at research time and the cited content actually read.
- Quote source text verbatim where load-bearing, with `>` blockquote framing where appropriate.
- Specific path/line anchors where verifiable (e.g., `arch/x86/mm/tlb.c:N` referencing a specific function).
- Future-dated arXiv IDs flagged: any arXiv ID with a year prefix >= 2604 (April 2026) should be acknowledged as recent and the publication metadata cross-checked.
- Single-source claims flagged: if Cerebro finds only one source for a specific claim, mark it as single-source rather than asserting it as well-supported.
- No hallucinated citations: if Cerebro cannot find primary-source evidence for a claim, say so. The PM-preliminary artifact contains specific claims (e.g., the FreeBSD `smp_targeted_tlb_shootdown` API signature from FreeBSD review D25510, the Linux `tlb_single_page_flush_ceiling = 33` constant, the Asterinas issue #799 verbatim quote, the FreeBSD-EN-18:07 verbatim advisory text) — Cerebro should independently verify these claims against original sources, not transcribe them from the PM-preliminary artifact.

## 6. Time budget

PM's read on appropriate budget for this dispatch: **2–3 hours** wall-clock. Five reference systems is substantial; the Janus voice anchor was ~75 min for one system; doing five systems carefully takes proportionally longer.

If Cerebro encounters scope creep (e.g., wanting to deep-dive AMD INVLPGB implementation details, or surveying ARM TLB semantics), exercise stop-discipline: those are Phase 3+ forward markers, not SC-3C scope. Surface them in §6 (Forward markers) and move on.

If Cerebro encounters citation-verification failures (e.g., a URL no longer reachable, an arXiv ID returning unexpected content), document the failure honestly in §7 (Calibration notes) and proceed with what's verifiable. Partial coverage of a reference system with honest acknowledgment is better than fabricated coverage.

## 7. Closure protocol

When the research artifact is complete:

1. Cerebro reports back to PL with: (a) confirmation the artifact is filed at the specified path, (b) one-line summary of whether findings support the provisional D-revisions or diverge, (c) any divergences enumerated, (d) time-budget actual.
2. PL ferries the report and artifact reference back to PM.
3. PM consumes the artifact and produces a follow-up PM-NOTE either finalizing the D-revisions ratification (if Cerebro confirms) or revising them (if Cerebro diverges).
4. SC-3C PLAN-stage Jarvis dispatch drafting proceeds from the finalized D-decisions.

## 8. Cross-references

- `Plans/SC-3C-THINK-2026-05-10.md` — THINK artifact this dispatch services.
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` — PM-preliminary starting reference (treat as input, not authority).
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` — provisional D-revisions ratification awaiting Cerebro confirmation.
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — voice anchor and rigor target.
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3 — Cerebro deep-research lane definition.
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — citation-rigor calibration discipline.
- `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` — calibration record for the role-confusion error this dispatch corrects.

---

**End of dispatch.**

*PM dispatches Cerebro for the proper D15 deep-research pass. PM-preliminary artifact is provided as starting reference; disagreement with it is welcome calibration data. Output filename `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`. Voice anchor: Janus release-on-switch artifact. Time budget 2–3 hours. Closure: Cerebro reports back via PL; PM consumes and finalizes or revises the provisional D-revisions ratification.*
