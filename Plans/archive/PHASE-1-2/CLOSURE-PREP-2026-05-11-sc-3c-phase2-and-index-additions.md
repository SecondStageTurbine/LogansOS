# SC-3C Closure Prep — PHASE-2.md and INDEX.md Additions (DRAFT)

**Date:** 2026-05-11
**Author:** Prometheus
**Type:** Closure-prep draft, doc-only. Awaits Jarvis SC-3C close before integration into live `docs/phases/PHASE-2.md` and `Plans/INDEX.md`.
**Status:** DRAFT pending PM review.

---

## A — Draft PHASE-2.md SC-3C Section

Insert immediately after the existing "## BUILD-3 sub-commit 3 (SC-3B)" section in `docs/phases/PHASE-2.md` (after line 431, before "## Transition from BUILD-1").

```markdown
## BUILD-3 sub-commit 4 (SC-3C) — Q7.b CR3 cache + Q7.c IPI TLB shootdowns

**Closed <PLACEHOLDER: YYYY-MM-DD> via commits `<PLACEHOLDER: 4a-sha>`, `<PLACEHOLDER: 4b-sha>`, `<PLACEHOLDER: 4c-sha>`.** Three sub-commits per SC-3C D12 ratification: 4a (Q7.b CR3 cache), 4b (Q7.c TLB shootdown machinery), 4c (Q7.c call-site wiring). Verification logs: `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-<PLACEHOLDER: date>.md`, `Plans/SC-3C-SUB-COMMIT-4B-Q7C-MACHINERY-VERIFICATION-<PLACEHOLDER: date>.md`, `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-<PLACEHOLDER: date>.md`. Closure report: `<PLACEHOLDER: filename>`. Gate scores: <PLACEHOLDER>. Mechanism named: <PLACEHOLDER or "N/A — no new wedge mechanism">.

### D-decisions ratified

Fifteen D-decisions (D1–D15) surfaced in THINK-stage (`Plans/SC-3C-THINK-2026-05-10.md` §6). Four revised post-prior-art research (D3 N=16 → N=33, D4 API takes target-set as input parameter with T2 forward optimization, D9 FreeBSD curcpu_cb overlap pattern, D14 missing-shootdown-detection scenario). All finalized by `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` and independently verified by Cerebro deep-research (`Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`).

### What landed

- Sub-commit 4a: Q7.b CR3 compare-and-skip. Wire `per_cpu.active_pmap_cr3` field in scheduler context-switch path. B2.a design: compare target CR3 against cached value; skip write on match. CR3-write-then-field-store ordering ensures stale-low is the only failure mode (one redundant write, no correctness issue).
- Sub-commit 4b: Q7.c TLB shootdown machinery. New IPI vector `0xfc`, IDT handler, `ShootdownPayload` struct with per-shootdown `AtomicU32` ACK counter, INVLPG helper, per-CPU single-slot shootdown payload via `AtomicPtr`. G3 hybrid granularity (INVLPG for ≤33 pages; full flush above threshold). T1 broadcast target selection. S3 synchronous with 1ms bounded sender wait + panic-on-timeout.
- Sub-commit 4c: Q7.c call-site wiring. Modify `pmap.rs` `unmap_page` and related mutation sites to invoke shootdown on cross-CPU-visible page-table changes.

### Course corrections

<PLACEHOLDER: "No course corrections" or actual corrections from Jarvis BUILD-stage, e.g., environment-anchored threshold revisions parallel to the ADR-019 bench lesson>

### Verification Log

- `cargo k` — <PLACEHOLDER: PASS at commit sha>.
- `cargo neo-test all` — <PLACEHOLDER: result>.
- 200/200 NEO_SMP=4 — <PLACEHOLDER: PASS or specific failures>.
- D14 missing-shootdown-detection scenario — <PLACEHOLDER: CPU A unmaps page; CPU B reads same virtual address; without correct shootdown B sees stale physical page; gate asserts B faults>.
- Frame leak audit clean — <PLACEHOLDER: PASS>.
- No `BUG:`, no `KERNEL PANIC` — <PLACEHOLDER: PASS>.
```

---

## B — Draft ADR Ledger Updates

No new ADRs land with SC-3C. The work is gated by already-ratified machinery (ADR-019 for `IrqSafeMutex`, ADR-020 for lock owner metadata, ADR-018 for Hpw discipline). The existing ADR ledger in PHASE-2.md requires no SC-3C-specific rows.

**Ledger unchanged.** If Jarvis surfaces a course correction requiring a new ADR during BUILD-stage, the post-Jarvis integration dispatch will add the row then.

---

## C — Draft Phase 2+ Forward Markers

The following forward markers derive from SC-3C THINK §8, PM-NOTE D-revisions ratification §4, and Cerebro consumption note §§3.1/4/3.5. Format ready to insert into PHASE-2.md after the existing "Forward reference" section, or as a new subsection.

```markdown
## Forward reference: Phase 2+ TLB management optimizations

SC-3C (sub-commit 4) landed the correctness baseline for SMP TLB management: CR3 compare-and-skip cache (Q7.b), broadcast IPI shootdowns with INVLPG hybrid granularity (Q7.c). Several performance and safety optimizations are deferred to future sub-commits. Recording them here so the next BUILD author finds them when they search for "TLB", "PCID", or "shootdown" in the docs tree.

### PCID / ASID enablement

Deferred per SC-3C D2. Current kernel has no PCID; every CR3 write triggers a full local TLB flush. Interesting when TLB-miss cost dominates measurement or when many concurrent processes exceed TLB natural capacity. Linux global-ASID promotion mechanism uses a sampling-gate-plus-rising-floor pattern (1-in-32 statistical gate, not deterministic 3-CPU threshold) — see `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §3.5. Future ADR-driven sub-commit when measurement justifies.

Source: `Plans/SC-3C-THINK-2026-05-10.md` §3.1 B3/B4 alternatives; `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 1.

### T2 targeted shootdown (per-Pmap CPU mask)

Deferred per SC-3C D4. Current broadcast sends IPI to all CPUs except self. T2 would track which CPUs have loaded each Pmap's CR3 and target only those. Critical design choice for the future dispatch: **strictly-accurate vs loosely-accurate per-mm cpumask**. Linux chose loosely-accurate (add-only on switch-in, lazy scrub on IPI receipt) because the context-switch path is hotter than the IPI path. NeoPlutarch's inference scheduler may hold threads on accelerators for long stretches with rare context switches — strictly-accurate could be cheaper net. This trade-off must be documented explicitly in whichever dispatch lands T2.

Source: `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §3.1; `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 2.

### Lazy TLB (receiver-side short-circuit)

Deferred per SC-3C D6. Linux's `should_flush_tlb` pattern: skip TLB invalidation on CPUs that aren't actively running the affected mm. **Critical exception:** lazy-TLB optimization is bypassed unconditionally during page-table teardown (`freed_tables=true`). A CPU with stale upper-level paging structures cached can fault into freed memory after teardown completes — Linux encodes this as an explicit flag that forces delivery to every CPU regardless of lazy state. Any NeoPlutarch lazy-TLB implementation must encode the same exception.

Source: `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §4; `Plans/SC-3C-THINK-2026-05-10.md` §4.1 L2/L3.

### Shootdown batching (mmu_gather analogue)

Deferred per SC-3C D7. Linux's `mmu_gather` batches page-table modifications and sends one IPI for many pages. Interesting if profiling shows shootdown-per-unmap is a workload bottleneck. Adds significant complexity; defer until measurement justifies.

Source: `Plans/SC-3C-THINK-2026-05-10.md` §4.1 B2; `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 4.

### AMD INVLPGB / Intel RAR hardware-broadcast

Phase 3+ forward marker. INVLPGB (AMD Zen 4+) and RAR (Intel 4th-gen Xeon+) can invalidate TLB entries across all CPUs via a single hardware broadcast, eliminating IPI-based shootdown entirely. Gate: NEO_SMP > 4 or measurement-driven. Linux's `broadcast_tlb_flush` integration point uses `mm_global_asid(mm)` check inside `flush_tlb_mm_range` — not inside the function itself.

Source: `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 6; `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §3.4, §3.5.

### Spectre-v2 BTB flush on context switch (IBPB analogue)

Phase 2+ CPU-vulnerability mitigation cluster. Every CR3 switch is a potential BTI attack surface. Deserves its own ADR; not SC-3C territory. Related to Q7.b's CR3 cache: once CR3 writes are conditional, the IBPB call sites must be placed correctly relative to the skip path.

Source: `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` §4 item 7.
```

---

## D — Draft INDEX.md Catalog Entries

New entries for the SC-3C artifact section in `Plans/INDEX.md`. Format matches existing INDEX.md entry style (filename | status | description).

### SC-3C artifacts (2026-05-10 to 2026-05-11)

| File | Status | What it is |
|------|--------|------------|
| `SC-3C-THINK-2026-05-10.md` | active | THINK-stage planning artifact for SC-3C (Q7.b CR3 cache + Q7.c IPI TLB shootdowns). Fifteen D-decisions surfaced with PM leans. Substrate at HEAD `fa8ec7c`. Input for PLAN-stage dispatch. |
| `PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` | closed | SC-3C D-decision revisions ratified post-prior-art research. Four changes: D3 (N=16→N=33), D4 (T1 broadcast stays, API takes target-set; T2 forward optimization), D9 (FreeBSD curcpu_cb overlap pattern), D14 (missing-shootdown-detection scenario). FINALIZED 2026-05-11 by Cerebro independent verification. |
| `DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` | closed | D15 deep-research dispatch to Cerebro: Linux/seL4/Asterinas/FreeBSD TLB shootdown prior art. First Cerebro dispatch under new agent roster. |
| `PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` | superseded | PM's preliminary first-pass TLB shootdown research (50.8 KB). Superseded by Cerebro independent verification (`RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`). Seven citation/factual corrections captured in consumption note. |
| `RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` | closed | Cerebro deep-research artifact: TLB shootdown / CR3 cache prior art (39.7 KB). 64/64 internal-self-check items passed. Confirmed all four provisional D-revisions; surfaced seven corrections to PM-preliminary evidence base plus one Phase 2+ forward marker (lazy-TLB page-table-teardown exception). |
| `PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` | closed | Consumption record for Cerebro D15 artifact. Finalizes four provisional D-revisions (PROVISIONAL → FINALIZED). Documents seven corrections to PM-preliminary. Both SC-3C PLAN-stage gates now clear. |
| `DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` | active | SC-3C PLAN-stage dispatch to Jarvis. Three sub-commits per D12: 4a (Q7.b CR3 cache), 4b (Q7.c TLB shootdown machinery), 4c (Q7.c call-site wiring). Verification gates per finalized D14. Jarvis calibration dispatch 3 of 3. |
| `SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-2026-05-11.md` | closed | Sub-commit 4a verification log: Q7.b CR3 compare-and-skip cache. <PLACEHOLDER: add gate result, e.g., "cargo k PASS, 200/200 NEO_SMP=4, micro-bench shows CR3 writes/sec dropped on same-process context switch">. |

**Post-Jarvis entries to be added by integration dispatch:**
- `SC-3C-SUB-COMMIT-4B-Q7C-MACHINERY-VERIFICATION-<date>.md` — sub-commit 4b verification log.
- `SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-<date>.md` — sub-commit 4c verification log.
- `CLOSE-REPORT-2026-05-MM-sc-3c.md` — SC-3C closure report.
- Any PM-NOTE filed during/after Jarvis BUILD-stage (e.g., course corrections, calibration notes).

---

## E — Integration Plan

After Jarvis closes SC-3C (all three sub-commits landed on `origin/main`):

1. **Insert §A into PHASE-2.md.** Paste the SC-3C section after the existing SC-3B section (after line 431). Fill in `<PLACEHOLDER: ...>` fields from Jarvis's verification reports and commit SHAs.
2. **Apply §C forward markers to PHASE-2.md.** Insert the "Forward reference: Phase 2+ TLB management optimizations" section after the existing async-discipline forward reference section at end of file.
3. **Apply §D entries to INDEX.md.** Add the "SC-3C artifacts" subsection to INDEX.md's artifact table, after the existing "Post-Sub-step-6 artifacts" section. Fill in any post-Jarvis entries (verification logs for 4b/4c, closure report).
4. **Update PHASE-2.md header.** Change status line from "sub-commit 1b in flight" to reflect SC-3C closure. Update "ADRs landed" row if applicable (unlikely; §B says ledger unchanged). Update "Commits" row with SC-3C commit SHAs.
5. **One docs-only commit.** Message: `docs(phases): add SC-3C closure section and Phase 2+ TLB forward markers`. Push to `origin/main` per hybrid git posture.

**Estimated integration scope:** 1-2 small edits + 1 commit. Same pattern as the post-SC-3B INDEX sweep (Commit 6 of the ADR ratification dispatch).

---

**End of closure-prep draft.**

*Deliverable A: PHASE-2.md SC-3C section draft with placeholders, forward markers for Phase 2+ TLB optimizations (PCID, T2 targeted shootdown, lazy TLB, batching, INVLPGB/RAR, Spectre-v2 BTB), INDEX.md catalog entries for 8 SC-3C artifacts (1 post-Jarvis placeholder group), and integration plan. Cross-references verified: all cited Plans/ artifacts exist at specified paths.*
