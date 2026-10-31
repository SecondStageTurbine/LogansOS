# PM-NOTE — SC-3C D15 Cerebro consumption; provisional D-revisions finalized

**Date filed:** 2026-05-11
**Author:** PM
**Type:** PM-NOTE. Consumption record for `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`. Status-change artifact for `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (PROVISIONAL → FINALIZED). Doc-only artifact; no code changes; no bible/ANSPEC/ADR mutation. Per dispatch §7.3, PM produces a follow-up PM-NOTE finalizing the D-revisions ratification on Cerebro confirmation, or revising them on Cerebro divergence. Cerebro confirmed all four; this PM-NOTE finalizes.
**Authority:** PM operational ratification, consistent with the original provisional ratification's authority (PL explicit handoff with "Go ahead, your call" before the PM-preliminary research was filed). Cerebro's independent verification closes the discipline loop the role-confusion recovery opened.

---

## 1. Closure summary

SC-3C D15 (TLB shootdown / CR3 cache prior-art deep-research) was dispatched to Cerebro per `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` after the PM-preliminary research (`Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md`) was reclassified during the 2026-05-10 role-confusion recovery.

Cerebro returned in ~2.5 hours wall-clock (right at the 2-3 hour budget specified in the dispatch). Artifact filed at `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` (39.7 KB). All 9 dispatch-required sections present; 64/64 internal-self-check items passed per Cerebro's closure report.

**Headline outcome:** **All four provisional D-revisions are confirmed by independently-verified primary sources.** No D-decision overturns. Seven corrections to the PM-preliminary's evidence base — all factual/citation precision improvements that sharpen rather than contradict the underlying conclusions. One additional finding (Linux lazy-TLB exception for page-table teardown) surfaces as a Phase 2+ forward marker, not a D-decision change.

## 2. D-revisions FINALIZED

The four D-revisions in `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` transition from **PROVISIONAL** to **FINALIZED** status. The ratifications are now Cerebro-independent-verified, not just PM-preliminary-supported.

| D-decision | Revision | Cerebro confirmation |
|---|---|---|
| D3 | N=16 → N=33 (match Linux measured threshold) | **CONFIRMED.** Linux `tlb_single_page_flush_ceiling = 33` verified at primary source. |
| D4 | T1 broadcast stays for SC-3C BUILD; API takes target-set as input parameter; T2 upgraded from indefinite-defer to Phase 2+ forward optimization | **CONFIRMED** with PLAN-stage documentation addendum: the T2 design choice (strictly-accurate vs loosely-accurate per-mm cpumask) must be documented explicitly when the Phase 2+ T2 migration dispatch is drafted. See §3.1 below. |
| D9 | Serial-before-IPI → FreeBSD `curcpu_cb` overlap pattern | **CONFIRMED.** The architectural argument stands; the mechanical sequencing description in the PM-preliminary was off (corrected in §3.3 below) but does not alter D9's conclusion. |
| D14 | Verification gate gains explicit missing-shootdown-detection scenario per FreeBSD-EN-18:07 | **CONFIRMED.** FreeBSD-EN-18:07 verified at primary source as the canonical missing-shootdown silent-data-corruption anchor. |

The eleven D-decisions unchanged from THINK-stage (D1, D2, D5, D6, D7, D8, D10, D11, D12, D13, D15) remain valid. Cerebro's pass did not surface any divergence on those.

**Status of `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`:** the existing "STATUS: PROVISIONAL pending Cerebro confirmation" block in its header is now superseded by this PM-NOTE. That artifact's header is updated immediately following this PM-NOTE's filing to point to this consumption note as the finalization marker.

## 3. Corrections to PM-preliminary (seven factual/citation precision issues)

Cerebro framed these as "corrections, not overturns." Each sharpens the prior-art evidence base without changing what the evidence supports. They are captured here for SC-3C PLAN-stage drafting (the dispatch text that Jarvis consumes must use Cerebro's corrected facts, not PM-preliminary's first-pass facts).

### 3.1 Linux cpumask model: eventually-consistent, not strictly-accurate (most material)

**PM-preliminary said:** Linux T2 is a clean strictly-accurate per-mm CPU mask.

**Source-verified reality:** Linux's T2 is **eventually-consistent.** `switch_mm_irqs_off` adds the incoming CPU's bit to `mm_cpumask(mm)` but **never removes the outgoing CPU's bit**. Stale bits accumulate and are scrubbed lazily by IPI receivers in `flush_tlb_func` (which clears its own bit when invoked on a CPU that's no longer running the relevant mm).

**Implication for NeoPlutarch:** when the Phase 2+ T2 migration is designed, the PLAN-stage dispatch must document the **strictly-accurate vs loosely-accurate T2 design choice** explicitly. Trade-offs:

- **Strictly-accurate** (remove-on-switch-out + add-on-switch-in): cleaner invariant; CPU mask reflects current truth at all times; higher cost on context-switch hot path (extra atomic on every switch-out).
- **Loosely-accurate** (add-only + lazy scrub on IPI receipt, the Linux model): cheaper context-switch path; more IPIs delivered to CPUs that don't actually need them (they self-scrub on receipt); the lazy-scrub semantics complicate reasoning about "is CPU X in mm Y's cpumask?".

Linux chose loosely-accurate because the context-switch path is hotter than the IPI path. NeoPlutarch's choice depends on what other workloads run alongside; if NeoPlutarch's inference scheduler holds threads on accelerators for long stretches with rare context switches, strictly-accurate may be cheaper net. This is a Phase 2+ architectural decision; capturing as a PLAN-stage documentation requirement for whichever dispatch lands the T2 work.

**Action:** when the SC-3C PLAN-stage dispatch is drafted, include an explicit note that says: "The current dispatch implements T1 broadcast per D4. The Phase 2+ T2 migration must include an ADR-tier design-choice decision between strictly-accurate and loosely-accurate per-mm cpumask discipline. Cerebro D15 §<section> documents the trade-off space."

### 3.2 FreeBSD API signature evolved

**PM-preliminary cited:** `smp_targeted_tlb_shootdown(cpuset_t mask, u_int vector, pmap_t pmap, addr1, addr2, curcpu_cb)`.

**Source-verified current signature:** `smp_targeted_tlb_shootdown_native(pmap_t, addr1, addr2, curcpu_cb, enum invl_op_codes op)` — **no `cpuset_t mask`, no `u_int vector`.** Target mask is derived from the `pmap` internally. Function-pointer indirection added for hypervisor override (`smp_targeted_tlb_shootdown_native` vs a hypervisor-replaceable variant).

**Implication for NeoPlutarch:** the architectural posture from PM-preliminary (FreeBSD provides a single API that takes target-set as input and handles the cross-CPU coordination internally) is still correct, but the specific NeoPlutarch API shape decision in D4 (API takes target-set as input) should not be sold as "matching the FreeBSD API exactly" because the FreeBSD API doesn't actually take a target-set explicitly — it derives it from the pmap. The architectural insight (derive target-set from address-space state, don't make every caller compute it) is the right takeaway; the specific signature mimicry framing is wrong.

**Action:** SC-3C PLAN-stage dispatch should describe the API design as "target-set derived from `pmap`-equivalent internal state, with `curcpu_cb` for local-CPU work overlap" rather than "matching FreeBSD's `smp_targeted_tlb_shootdown` parameter list."

### 3.3 curcpu_cb sequencing

**PM-preliminary said:** `curcpu_cb` is "invoked during the wait loop."

**Source-verified reality:** `curcpu_cb` runs **after IPI dispatch and before the scoreboard wait loop.** It is the local-CPU work that happens during the window between "all IPIs sent" and "all remote ACKs received."

**Implication for NeoPlutarch:** D9's conclusion is unchanged (no serial-before-IPI; local work overlaps with remote ACK wait). The mechanical description in PM-preliminary was wrong but the architectural argument is correct. SC-3C PLAN-stage dispatch must use the corrected sequencing description.

### 3.4 broadcast_flush_tlb_all does not exist

**PM-preliminary cited:** `broadcast_flush_tlb_all` as the Linux INVLPGB integration point.

**Source-verified reality:** the function is **`broadcast_tlb_flush`**, not `broadcast_flush_tlb_all`. The INVLPGB gate is at the caller (`mm_global_asid(mm)` check inside `flush_tlb_mm_range`), not inside the function itself.

**Implication:** citation correction; no architectural impact. SC-3C PLAN-stage dispatch (if it cites Linux INVLPGB integration as forward-marker context for D4's T2-Phase-2+-upgrade rationale) must use the correct function name.

### 3.5 consider_global_asid has a 1-in-32 sampling gate

**Not noted in PM-preliminary.** Cerebro surfaced: `consider_global_asid` (Linux's promotion-from-local-ASID-to-global-ASID decision function) has a **1-in-32 sampling gate**. Promotion is **statistical, not deterministic**. The "3 CPU" threshold (which PM-preliminary cited as the deterministic trigger) is actually a **floor** that rises as the global-ASID bitmap fills.

**Implication for NeoPlutarch:** the Linux global-ASID promotion model is more conservative than PM-preliminary implied. The architectural takeaway (promote to broadcast only when the mm is hot enough to justify the global-ASID resource) is still correct; the specific threshold mechanism is more nuanced. If/when NeoPlutarch designs a similar promotion logic (Phase 2+ INVLPGB/RAR work), the sampling-gate-plus-rising-floor pattern is the precedent to study, not a pure 3-CPU-threshold gate.

**Action:** if SC-3C PLAN-stage cites Linux global-ASID as forward marker, use Cerebro's corrected mechanism description. Not a SC-3C BUILD scope item.

### 3.6 seL4 mailing list note date

**PM-preliminary cited:** January 2019.
**Source-verified:** November 2018.

Citation correction; no impact.

### 3.7 FreeBSD-EN-18:07 verbatim wording

**PM-preliminary quoted:** "resulting in."
**Source-verified:** "resulted in."

Minor grammatical correction; substance unchanged.

## 4. Additional finding (Phase 2+ forward marker, not D-decision change)

**Linux's `should_flush_tlb` lazy-TLB filter is bypassed unconditionally when `freed_tables=true`.**

Cerebro surfaced this as an architectural pattern relevant to NeoPlutarch's future lazy-TLB work (deferred, not in SC-3C scope). The pattern: lazy-TLB optimizations (skip TLB invalidation on CPUs that aren't actively running the mm) are **safe for normal flushes** but **catastrophically unsafe for page-table teardown** — a CPU with stale upper-level paging structures cached can fault into freed memory after the teardown completes.

Linux encodes this as an explicit `freed_tables=true` flag on the flush request that bypasses the lazy-TLB filter unconditionally. Any TLB invalidation issued during page-table teardown is delivered to every CPU regardless of lazy state.

**Implication for NeoPlutarch:** if NeoPlutarch ever adds lazy-TLB optimization (Phase 2+, currently no D-decision pending), the **same exception for page-table teardown must be encoded**. This is a hidden invariant that's easy to miss when implementing lazy-TLB from scratch; surfacing it here so it's discoverable in project memory.

**Action:** no current action. Phase 2+ when lazy-TLB work surfaces, the relevant dispatch must cite this PM-NOTE for the page-table-teardown exception.

## 5. SC-3C PLAN-stage status

**Both gates now clear:**

- ✅ Jarvis kernel-rename bundle close (sub-commits 1+2+3 landed on `origin/main`; final commit `27d1b99` for ADR-019 sub-phase A).
- ✅ Cerebro D15 D-revision confirmation (this PM-NOTE).

**SC-3C PLAN-stage Jarvis dispatch is the next critical-path action.** Per the SC-3C THINK artifact §6 D12 row, the PLAN-stage dispatch splits the BUILD work into three sub-commits: 4a (Q7.b CR3 cache) → 4b (Q7.c machinery, no callers) → 4c (Q7.c call-site wiring).

Inputs the PLAN-stage dispatch must consume:

1. `Plans/SC-3C-THINK-2026-05-10.md` — full D-table.
2. `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` — four D-revisions (now FINALIZED via this PM-NOTE).
3. `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — canonical prior-art reference.
4. **This PM-NOTE** for the seven corrections + the strictly-vs-loosely-accurate T2 PLAN-documentation requirement.
5. `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` and `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — Jarvis calibration data; perf-gate threshold environment-anchoring discipline.

PLAN-stage dispatch will be Jarvis calibration dispatch 3 of 3 per agent-roster §3.1.

## 6. Calibration: Cerebro voice profile (second-and-third dispatches under new roster)

**Field survey (2026-05-11 morning):** 2.5 hours, output structure followed dispatch spec exactly, four divergences documented, appropriate disagreement-with-PM-preliminary behavior.

**D15 prior-art (this consumption):** 2.5 hours, output structure followed dispatch spec exactly, 64/64 ISC items passed per Cerebro's self-audit, seven corrections + one forward marker documented, all four D-revisions confirmed.

**Voice profile firming-up signals after two dispatches:**

- Time-budget accuracy: both dispatches landed at ~2.5 hours against 2-3 hour budgets. Cerebro estimates its own work well.
- Output-structure adherence: both dispatches followed the explicit-structure specification PM provided (per agent-roster §3.2 transitional discipline). No drift.
- Divergence-from-PM-preliminary behavior: Cerebro reliably corrects factual errors and surfaces missed nuances. Defers to PM on interpretation calls.
- Citation rigor: both passes spot-checked source-verifiable claims; corrections to PM-preliminary were grounded in primary-source evidence, not "Cerebro thinks differently."
- Internal-self-check discipline: D15 pass reports 64/64 ISC items; field-survey pass had structural completeness check. Cerebro applies its own verification discipline.

**Calibration discipline can transition to "implicit-structure" from the next dispatch onward.** PM no longer needs to specify output structure section-by-section in dispatches; Cerebro can be trusted to apply the Janus-anchored structure on its own. Dispatch §6 specifications can be terser going forward.

**One more substantive Cerebro dispatch (third under new roster) should fully firm up the voice profile.** No immediate dispatch candidate; will surface as research needs arise.

## 7. PM dispatch-shaping discipline (reinforced)

**Three research-shaped errors in PM-preliminary work over 2026-05-10/11 cycle:**

1. Role-confusion (2026-05-10 evening): PM did Cerebro-shape research directly without dispatch.
2. OctantOS factual error (2026-05-10 PM-preliminary field survey): PM concluded a real project was "fabricated or misremembered" by failing to follow an HN link.
3. Seven citation-precision errors (2026-05-10 PM-preliminary TLB shootdown): PM-preliminary's evidence-base citations were correct-in-spirit but sloppy-in-detail.

**The pattern is consistent:** PM-preliminary research produces architecturally-correct conclusions but loose factual/citation precision. Cerebro deep-research produces architecturally-equivalent conclusions with tight factual/citation precision. The two pass types are not equivalent in deliverable quality.

**Internalized discipline:**

- PM-preliminary research should be **rare** and reserved for operationally-urgent decisions that cannot wait 2-3 hours for Cerebro dispatch.
- When PM-preliminary is used, downstream ratifications should be marked **PROVISIONAL** until Cerebro confirms.
- PM-preliminary conclusions should explicitly flag "this is PM-grade, not Cerebro-grade" so consumers calibrate trust accordingly.
- Specific failure mode to watch: PM-preliminary citations are the highest-risk surface for factual error. Conclusions are usually directionally right; the supporting citations are where Cerebro consistently catches issues.

## 8. Action items

**Completed in this PM-NOTE:**
- ✅ Cerebro D15 artifact consumed.
- ✅ D-revisions ratification finalized (PROVISIONAL → FINALIZED).
- ✅ Seven corrections to PM-preliminary documented for SC-3C PLAN-stage consumption.
- ✅ Phase 2+ forward marker on lazy-TLB page-table-teardown exception captured.
- ✅ Cerebro voice profile calibration updated.

**Following this PM-NOTE (PM operational):**
- ⏳ Update `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` header: replace PROVISIONAL status block with FINALIZED status block pointing to this consumption note.
- ⏳ Update `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` header: add supersession marker pointing to Cerebro artifact and this consumption note, parallel to the field-survey supersession pattern.

**Next dispatch:**
- ⏳ Draft SC-3C PLAN-stage Jarvis dispatch (Jarvis calibration 3 of 3). Inputs enumerated in §5 above. PM-tier dispatch shaping, no PL-tier authority required.

**Forward markers (no immediate action):**
- 🔮 Phase 2+ T2 migration dispatch must document strictly-accurate vs loosely-accurate per-mm cpumask design choice explicitly (§3.1).
- 🔮 Phase 2+ lazy-TLB work (if it surfaces) must encode page-table-teardown exception per Linux pattern (§4).
- 🔮 Phase 2+ INVLPGB/RAR work (if it surfaces) must study Linux global-ASID promotion's sampling-gate-plus-rising-floor pattern, not the simplified 3-CPU-threshold model (§3.5).

## 9. Cross-references

**Consumed:**
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` (39.7 KB) — Cerebro deep-research artifact, now canonical for SC-3C TLB-shootdown prior art.

**Finalized by this PM-NOTE:**
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` — PROVISIONAL → FINALIZED. Header update follows immediately.

**Superseded for canonical content (preserved as historical record):**
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` (51 KB) — PM's preliminary first-pass. Header supersession marker follows immediately.

**Inputs for SC-3C PLAN-stage drafting:**
- `Plans/SC-3C-THINK-2026-05-10.md` — full D-table.
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` — D-revisions (FINALIZED via this note).
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — canonical prior art.
- This PM-NOTE — seven corrections + strictly-vs-loosely-accurate T2 PLAN-documentation requirement.
- `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` — Jarvis calibration patterns.
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — perf-gate threshold environment-anchoring discipline.

**Related operational artifacts:**
- `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` — the dispatch.
- `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` — calibration record for the role-confusion error that prompted this dispatch.
- `Plans/PM-NOTE-2026-05-11-ai-native-os-field-survey-consumption.md` — parallel Cerebro consumption pattern.

**Discipline anchors:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3 (Cerebro deep-research lane), §3.2 (Cerebro voice-profile calibration — now firming up after two dispatches).
- PL operational posture clarification (2026-05-10 memory entry).

---

## 10. Closure

SC-3C D15 Cerebro consumption complete. All four provisional D-revisions confirmed by independent primary-source verification; ratification finalizes. Seven PM-preliminary corrections captured for SC-3C PLAN-stage dispatch consumption. One Phase 2+ forward marker (lazy-TLB page-table-teardown exception) recorded. Strictly-vs-loosely-accurate T2 design choice flagged as PLAN-documentation requirement for whichever future dispatch lands Phase 2+ T2 migration.

**Both SC-3C PLAN-stage gates clear.** Next action: PM drafts SC-3C PLAN-stage Jarvis dispatch (Jarvis calibration 3 of 3).

PM dispatch-shaping discipline reinforced: PM-preliminary stays rare and explicitly marks downstream ratifications PROVISIONAL until Cerebro confirms. Cerebro voice profile firming up after two dispatches at ~2.5 hours each with consistent spec adherence and citation rigor; one more dispatch should complete profile.

---

**End of PM-NOTE.**

*All four provisional D-revisions confirmed. Seven citation/factual corrections captured. SC-3C PLAN-stage unblocked. PM-preliminary discipline reinforced. Cerebro voice profile firming up.*
