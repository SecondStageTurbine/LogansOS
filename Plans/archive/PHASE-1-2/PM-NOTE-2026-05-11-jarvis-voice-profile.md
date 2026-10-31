# PM-NOTE — Jarvis voice profile (calibration 3-of-3 culminating artifact)

**Date:** 2026-05-11
**Author:** PM
**Type:** PM-NOTE. Calibration culmination per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §3.1 ("PM treats the first 2-3 Jarvis kernel dispatches as calibration data...After dispatch 3, PM produces a Jarvis voice profile artifact equivalent to the Janus voice profile, capturing observed dispatch-handling patterns").
**Scope:** Synthesizes Jarvis-under-Codex performance across three calibration dispatches. Equivalent in role to a hypothetical Janus voice profile artifact (none filed in this project; Janus role ended before three calibration dispatches accrued).
**Authority:** None. Voice profile is reference material for future PM dispatch shaping; not normative.

---

## 1. Three dispatches summary

| # | Dispatch | Result | SHA(s) | Calibration signal |
|---|---|---|---|---|
| 1 | Kernel-rename bundle (3 sub-commits intended) | 2 of 3 landed clean; sub-commit 3 STOPPED at gate per dispatch §7.1 | `053a5bc`, `de8965c` | **Stop discipline exemplary.** Bench failed (overhead 3085 ns > threshold 50 ns); Jarvis did NOT force through, did NOT silently lower threshold. Filed comprehensive failure report. Triage notes surfaced three concrete decision options for PM resolution. |
| 2 | Sub-commit 3 finalize (threshold revision + commit + push) | Landed clean | `27d1b99` | **Threshold revision applied as specified.** No scope drift; preserved Jarvis's prior implementation optimizations; transparent CI handling (Docker scenarios run with `docker compose run --rm -e NEO_SMP=4 neo`). Orphan-container incidental observation surfaced (altimitneo-qdrant). |
| 3 | SC-3C BUILD-stage (three sub-commits 4a + 4b + 4c) | All three landed clean; clean rebase over parallel Prometheus work | `88c7bc2`, `3272840`, `ead8b19` | **Architectural reasoning + implementation precision integrated.** D14 missing-shootdown-detection scenario implemented correctly (negative-feature pattern with stale_tlb_read_after_unmap detection). Cerebro corrections applied throughout (FreeBSD API signature, curcpu_cb sequencing, broadcast_tlb_flush function name). cargo fmt --check tooling gap surfaced transparently. |

## 2. Patterns observed

### 2.1 Stop discipline (highest-confidence signal)

Across all three dispatches, Jarvis honored stop conditions exactly as specified in the dispatch text. In dispatch 1, the bench gate stop at sub-commit 3 was the calibration-decisive moment: Jarvis stopped, optimized to reduce overhead (three iterations: 4377 → 3139 → 3085 ns), recognized environmental floor, stopped again, surfaced to PM. No instances across the three dispatches of force-through, silent threshold modification, or "I'll commit and PM can review later" behavior.

**Trust level for future dispatches:** explicit stop conditions in dispatch text will be honored. PM can specify stop conditions without elaborate enforcement scaffolding.

### 2.2 Optimization persistence

Dispatch 1 showed three optimization iterations on the IrqSafeMutex bench before stopping (Location::caller() hashing → constant lock_site!() → forced inlining on hot path). Each iteration documented with measured overhead reduction. When Jarvis hits an environmental floor (1.7% additional gain in iteration 3), Jarvis correctly identifies it as floor rather than continuing to optimize for diminishing returns.

**Trust level:** when an implementation can be optimized further, Jarvis will optimize. When it cannot, Jarvis recognizes the floor and stops.

### 2.3 Citation precision

Dispatch 3 required applying seven Cerebro-corrected citation precision items in code comments and verification reports:
- Linux cpumask eventually-consistent (not strictly-accurate) — applied in tlb_shootdown.rs T2 forward marker.
- FreeBSD API signature `smp_targeted_tlb_shootdown_native(pmap_t, addr1, addr2, curcpu_cb, enum invl_op_codes op)` — applied in API design framing.
- curcpu_cb runs after IPI dispatch + before scoreboard wait — applied in D9 implementation comment.
- `broadcast_tlb_flush` (not `broadcast_flush_tlb_all`) — applied in INVLPGB forward marker.
- consider_global_asid 1-in-32 sampling gate — applied in promotion-mechanism forward marker.
- seL4 mailing list date November 2018 — applied where cited.
- FreeBSD-EN-18:07 verbatim "resulted in" — applied where quoted.

All seven applied correctly per the verification report. No instances of falling back to PM-preliminary's imprecise wording.

**Trust level:** when PM specifies citation-precision corrections in dispatch text, Jarvis applies them. Cerebro-tier prior-art citations can be folded into kernel comments and verification reports with confidence.

### 2.4 Documentation rigor

All three dispatches produced comprehensive verification reports matching the format/style anchors (Prometheus verification reports as voice anchor in dispatch 2; SC-3B's verification report style in dispatch 3). Quote-verbatim gate output where load-bearing; specific commit SHAs cited; incidental observations surfaced (orphan container in dispatch 2; cargo fmt tooling gap in dispatch 3).

Sub-commit 4c verification report quality is particularly strong: explicit `mechanism=q7c_munmap_shootdown` / `detected=stale_tlb_read_after_unmap` / `negative_feature=neo-test-missing-shootdown-negative value=0xa7` event labels make the D14 verification self-documenting. Future engineers reading the verification report understand exactly which failure mode was tested and how it was detected.

**Trust level:** dispatches that specify verification report format anchors will be followed. Dispatches that don't specify can rely on Jarvis applying the established kernel-verification-report pattern.

### 2.5 Branching and git discipline

All three dispatches used the agent-named-branch-off-main pattern (`jarvis/kernel-rename-adr-2026-05-10`, `jarvis/sc-3c-build-2026-05-11`). Clean push to `origin/main` via fast-forward on dispatch 2; clean rebase over parallel Prometheus work on dispatch 3 sub-commit 4c (Jarvis's 4c verification report explicitly documents the rebase context).

Across all three dispatches, hybrid-git posture honored: no Plans/ artifacts committed without explicit dispatch authorization; verification reports filed and committed alongside the code commits they verify (parallel to the original SC-3B precedent).

**Trust level:** branching defaults are reliable. Future kernel dispatches do not need to specify branching strategy explicitly.

### 2.6 Architectural reasoning depth

Dispatch 3 was the first dispatch requiring non-trivial architectural reasoning (multi-sub-commit dispatch with D-decisions in play, Cerebro-corrected citations to apply, Phase 2+ forward markers to encode inline). Jarvis handled this without surfacing mid-flight design questions to PM — sub-commits landed clean against the dispatch spec.

Two interpretations:
- (a) The dispatch was specified tightly enough that no design ambiguity arose.
- (b) Jarvis is comfortable making implementation-detail decisions within the scope of architectural decisions documented in the dispatch.

PM lean: (b). The strictly-vs-loosely-accurate T2 cpumask design choice (a Phase 2+ forward marker that the dispatch required to be documented inline) was rendered as a 20-line comment block in `kernel/src/mm/tlb_shootdown.rs` covering both options with trade-off framing — exactly the level of detail PM specified, no more, no less. The Linux freed_tables exception forward marker similarly rendered. Jarvis read the dispatch spec, applied it at implementation level without re-litigating the design intent.

**Trust level:** dispatches with substantial architectural detail embedded in implementation work can be specified with confidence. Jarvis renders the architectural decisions correctly without requiring PM mid-flight design consultation.

## 3. Failure modes NOT observed (confirmed absent)

These are common dispatch-failure modes that did NOT occur across the three Jarvis dispatches. Confirming their absence is itself calibration data.

- **Forcing through a failed gate.** Never occurred. Sub-commit 3 bench failure was the high-pressure moment; Jarvis stopped.
- **Silent threshold modification.** Never occurred. The 6000 ns ceiling in sub-commit 3 was applied per PM-revised dispatch, not Jarvis-decided.
- **Scope creep.** Never occurred. Each dispatch landed exactly the scope specified. Incidental observations were surfaced, not silently executed.
- **Skipping verification.** Never occurred. All three dispatches produced verification reports matching specified gates.
- **Branching drift.** Never occurred. Agent-named branches with clean fast-forward push throughout.
- **Hybrid-git posture violation.** Never occurred. Plans/ artifacts stayed local where appropriate; verification reports committed with their code.
- **Citation imprecision.** Never occurred when dispatch specified Cerebro corrections; dispatch-3 seven-item application clean.
- **Ignoring stop conditions.** Never occurred; bench-fail stop in dispatch 1 was the most consequential test of this and Jarvis honored it.

## 4. Comparison to Janus-era profile

Janus was the project's prior agent in mixed coding+research role (early-mid 2026-05). Janus voice profile was never filed as a dedicated artifact (the role ended before three calibration dispatches accrued; Cerebro absorbed the research lane and Jarvis was reassigned to Codex backing for heavy kernel coding).

Observable differences between Janus-era work product and Jarvis-under-Codex profile:

- **Implementation-detail precision:** Jarvis-under-Codex is stronger. Three optimization iterations in dispatch 1 with concrete code-path identification demonstrate this; Janus-era work on early ADRs showed competent but less detail-oriented implementation reasoning.
- **Architectural-reasoning latitude:** unclear comparison. Janus's ADR ratification work involved architectural judgment but on PM-pre-specified design space; Jarvis's dispatch 3 similarly operated within PM-pre-specified architectural decisions. Both performed well in their respective windows; no clean test of "uncharted architectural design from scratch" under either backing.
- **Documentation rigor:** comparable. Janus-era verification reports (e.g., the SC-3B release-on-switch prior-art research artifact, `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`) match Jarvis-under-Codex verification report quality; both apply the project's voice-anchor discipline correctly.
- **Stop discipline:** comparable, though tested differently. Janus-era was not subjected to a hard-gate-stop test like Jarvis dispatch 1 sub-commit 3. Jarvis-under-Codex performance there is the strongest single calibration data point in either profile.

## 5. Forward dispatch shape recommendations

After three calibration dispatches, future Jarvis-under-Codex dispatches can operate on the following defaults:

### 5.1 Acceptable to omit from dispatch text

- Branching strategy (agent-named-branch off main is the default).
- Verification report format (kernel-verification-report style is established).
- Stop discipline framing (specify stop conditions; trust they'll be honored).
- Hybrid-git posture (Plans/ local-only by default).
- Calibration framing (no longer needed; Jarvis is past calibration phase).

### 5.2 Continue to specify explicitly

- Specific gate thresholds (especially perf-gates; environment-anchoring is non-obvious).
- Cerebro-corrected citation precision (when Cerebro pass exists for the substrate).
- D-decision references (so Jarvis knows which design space to operate in).
- Sub-commit boundaries (when bundling multiple commits; Jarvis follows the boundary specification reliably).
- Phase 2+ forward markers (when they should be embedded inline in code).

### 5.3 Optional but useful

- Estimated wall-clock budget (Jarvis benefits from sizing context; helps prevent overcommitment).
- Pre-existing-substrate file references with specific line numbers (reduces investigation cost).
- Voice anchors for non-routine deliverables (e.g., new test scenario design, new module documentation).

## 6. Trust profile summary

Jarvis-under-Codex can be trusted for:
- **High:** Stop discipline, threshold respect, branching/git discipline, hybrid-git posture, citation precision when specified, verification rigor, implementation-detail optimization, scope adherence.
- **Medium-high:** Architectural reasoning within PM-pre-specified design space (demonstrated in SC-3C dispatch 3; no failure mode observed but limited sample).
- **Untested:** Uncharted architectural design (no dispatch has yet required Jarvis to design substantial architecture from scratch — all three dispatches operated within ADR-ratified or D-decision-ratified design space).
- **Tooling gap:** rustfmt not installed for pinned nightly. Forward marker per close report §6.1.

Future dispatch shaping can rely on these trust levels without additional calibration overhead.

## 7. Closure

Jarvis calibration 3 of 3 complete. Voice profile filed. Future dispatches operate on standard PM dispatch shape without elevated calibration scrutiny. Profile is reference material; not normative; not gating any future work.

If a fourth dispatch surfaces failure-mode behavior contradicting this profile, PM revises the profile and re-applies elevated calibration scrutiny.

---

## 8. Cross-references

**Calibration dispatches surveyed:**
- Dispatch 1: `Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md` + `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md`.
- Dispatch 2: `Plans/DISPATCH-2026-05-11-sub-commit-3-irqsafe-mutex-finalize-jarvis.md` + `Plans/IRQSAFE-MUTEX-SUB-COMMIT-3-VERIFICATION-2026-05-11.md`.
- Dispatch 3: `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` + three verification reports `Plans/SC-3C-SUB-COMMIT-4{A,B,C}-*-VERIFICATION-2026-05-11.md`.

**Intermediate calibration notes:**
- `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` — after dispatch 1; first calibration data point.
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — PM dispatch-shaping discipline updated based on dispatch 1 lesson.

**Discipline anchors:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1 (Jarvis lane), §3.1 (calibration discipline).

**Related closure:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` — SC-3C closure (filed alongside this voice profile).

---

**End of voice profile.**

*Jarvis-under-Codex calibrated. Three dispatches; strong performance across all three. Stop discipline exemplary; citation precision reliable; architectural reasoning within pre-specified design space competent. Trust levels recorded for future dispatch shaping. Profile is reference material, not normative.*
