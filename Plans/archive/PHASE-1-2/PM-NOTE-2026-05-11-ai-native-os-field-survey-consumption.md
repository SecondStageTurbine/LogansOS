# PM-NOTE — AI-native-OS field survey: Cerebro consumption and closure

**Date filed:** 2026-05-11
**Author:** PM
**Type:** PM-NOTE. Consumption record for `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md`. Doc-only artifact. Per dispatch §7.3, no PM-NOTE is required unless Cerebro's findings substantially change NeoPlutarch's strategic posture; posture is unchanged. This note captures four substantive items from Cerebro's closure that warrant project-memory placement even though they don't rise to posture-change level.
**Authority:** None. PM consumed the Cerebro artifact; this note records what landed, what diverged, and what forward markers were surfaced. Strategic posture stands.

---

## 1. Closure summary

The AI-native-OS field survey was dispatched to Cerebro per `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` after PM's role-confusion error caused the original work to be filed under wrong attribution (`Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md`).

Cerebro returned in ~2.5 hours wall-clock (right at the 2-3 hour budget specified in the dispatch). Artifact filed at `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (58.7 KB; comparable to PM-preliminary's 52 KB plus additional taxonomy and per-project verification work).

**Field-shape headline:** Sense-A (kernel-level AI-native) has 1 confirmed peer (Anima OS / ProbeLogits) + 1 claimed-extraordinary candidate (OctantOS) across 16 projects surveyed. Asterinas remains closest kernel-engineering peer. Bible Article 23 reinforced, not challenged.

**Strategic posture:** unchanged. `Plans/PM-NOTE-2026-05-10-ai-aware-not-ai-driven.md` remains the right framing artifact for the recurring critique; Bible Article 23 stance remains the load-bearing constitutional clause; `ALTIMIT-real-world-implementation.md` §C and §J framing remains accurate.

## 2. Divergences from PM-preliminary

Cerebro surfaced four divergences from `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md`. Per dispatch §2 input #2 framing ("disagreement with this artifact is welcome calibration data"), each divergence is calibration on both PM-preliminary discipline and Cerebro deep-research voice.

### 2.1 OctantOS / OctantCore: PM factual error

PM's preliminary audit concluded OctantOS was "fabricated or misremembered" because PM could not find any project by that name. PM cited HN id 47388478 as pointing to a different, unrelated story.

**Cerebro corrected this:** HN id 47388478 actually links to `github.com/MatrixForgeLabs/OctantOS`; OctantCore is the kernel component. **The project is real.** PM failed to follow the HN post link properly during the preliminary audit — exactly the citation-rigor failure mode that distinguishes PM-preliminary from Cerebro-vetted research.

The project's extraordinary scale claims (1.3M LOC, 50 crates, 4 months solo development) remain unverified at source level per Cerebro's audit, but the existence is established.

**Calibration data:** this is the same family of error as the role-confusion error from 2026-05-10 — PM doing research-shaped work without the diligence a Cerebro pass brings. Two errors in two days under research-shaped work is a clear signal that PM-preliminary should remain rare and should explicitly defer to Cerebro for any project-existence-verification work. **Internalized for forward PM dispatch-shaping discipline.**

PM-preliminary header updated with a supersession marker pointing to the Cerebro artifact for canonical project verification.

### 2.2 AGT naming-collision forward marker

Cerebro flagged a positioning concern PM did not surface prominently: **Microsoft AGT (Agent Governance Toolkit, April 2026) ships a package literally named "Agent OS."**

This is a live disambiguation problem for NeoPlutarch's external positioning. When NeoPlutarch is described publicly as an "AI-native operating system," there is now a real risk of being conflated with Microsoft's enterprise-agent-governance product line. The phrase "Agent OS" is no longer available without explicit disambiguation.

**Forward marker for external-communications work:** the next README revision pass, eventual public-launch material, and any conference/paper abstract should explicitly avoid the bare phrase "Agent OS" and should disambiguate from Microsoft AGT where the surrounding context might invite confusion. The "Intelligence as a kernel primitive." tagline (already in the README per commit `3a2cb70`) is well-positioned for this; it stays distinctive even with AGT's product in the field.

Not a Bible amendment. Not an ANSPEC change. Not an ADR. Just a watch-this-for-external-comms note. Filed here for searchability when the external-comms work happens.

### 2.3 Taxonomy extension: Sense-E and Sense-0 added

PM-preliminary proposed a Sense-A/B/C/D taxonomy:
- A: kernel-level AI primitives (true AI-native OS)
- B: agent runtime branded as OS
- C: AI-built OS not AI-aware
- D: agent-isolation OS

Cerebro confirmed A/B/C/D and added two senses:
- **Sense-E:** eBPF agent instrumentation on conventional Linux (e.g., Inner Warden) — agents are first-class observability subjects without the kernel being AI-native.
- **Sense-0:** kernel-engineering peerage without AI claims (e.g., Asterinas, Theseus, Redox) — projects in the same architectural lineage but with no AI framing. These are the projects NeoPlutarch most resembles structurally despite the AI framing distinction.

**Canonical taxonomy is now Cerebro's six-sense version.** PM-preliminary's four-sense version is historically accurate but superseded; PM-preliminary header updated to reference Cerebro for canonical taxonomy.

Sense-0 is the most useful addition for NeoPlutarch self-positioning: structurally, NeoPlutarch is a Sense-0-shaped project (Rust monolith, capability-based, clean-slate) that ALSO carries Sense-A claims (first-class tensor/model/inference primitives). The two are independent axes. Useful framing for future external positioning.

### 2.4 OpenFang Bible-23.1 assessment: disagreement settled in PM's favor

PM-preliminary read OpenFang's named "Hands" agents (the seven AI personas shipped with the project) as evidence of a system AI persona — Bible Article 23.1 violation candidate.

Cerebro read OpenFang's README as "no chatbot persona" and consequently more Bible-23-compatible.

Cerebro **explicitly deferred to PM's reading** ("PM's reading is the more conservative and defensible interpretation; PM's reading stands"). This is the correct behavior for a research agent under PM authority — surface the disagreement, document the alternative reading, let PM resolve. No action required.

**Calibration data on Cerebro voice profile:** Cerebro will surface alternative readings when it disagrees with PM-preliminary, but defers to PM on interpretation calls that rest on judgment rather than fact. This is the right division of authority and is a good calibration data point for future research dispatches.

## 3. Strategic posture: unchanged

Restating for the record:

- **Bible Article 23 stance:** reinforced. Only one confirmed kernel-level AI-native peer (Anima OS) + one unverified-scale claimed candidate (OctantOS) across 16 surveyed projects. The Heimdall / Win11 24H2 / AIOS pattern that the AI-aware-not-AI-driven PM-NOTE refuses remains the dominant Sense-B failure mode in the field.
- **Closest kernel-engineering peer:** Asterinas (ATC 2025), consistent with `ALTIMIT-real-world-implementation.md` framing.
- **NeoPlutarch's distinctive position:** Sense-0 + Sense-A. Structurally similar to Asterinas/Theseus/Redox (Rust monolith, capability-based, clean-slate); architecturally AI-aware via first-class kernel primitives but not AI-driven (no model in scheduler decision path). No other surveyed project occupies exactly this position.
- **The "AI-aware-not-AI-driven" PM-NOTE** (`Plans/PM-NOTE-2026-05-10-ai-aware-not-ai-driven.md`) remains the right consolidated answer to the recurring scheduler-AI critique.

No Bible amendment indicated. No ANSPEC mutation indicated. No ADR drafting indicated. No phase-roadmap implication.

## 4. Action items

**Completed in this PM-NOTE:**
- ✅ Cerebro field-survey artifact consumed.
- ✅ OctantOS PM error logged with calibration framing.
- ✅ AGT naming-collision forward marker recorded for external-comms discoverability.
- ✅ Taxonomy supersession captured (Sense-A/B/C/D → Sense-0/A/B/C/D/E).
- ✅ OpenFang disagreement settled in PM's favor; calibration noted.

**Following this PM-NOTE:**
- ⏳ Update `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` header with supersession marker pointing to Cerebro artifact and this consumption note.

**Forward markers (no immediate action; flagged for discoverability):**
- 🔮 Next README revision / external-comms pass: avoid bare "Agent OS"; disambiguate from Microsoft AGT explicitly where context invites confusion. Cite this PM-NOTE for source rationale.
- 🔮 If/when public-launch material is prepared: position NeoPlutarch as "Sense-0 + Sense-A" using Cerebro's six-sense taxonomy framing; closest peer is Asterinas; distinctive in combining kernel-engineering rigor with first-class AI primitives.

**PM dispatch-shaping discipline updates (internalized):**
- PM-preliminary should remain rare and should explicitly defer to Cerebro for project-existence-verification work. Two errors in two days under PM-preliminary research-shaped work (role-confusion 2026-05-10, OctantOS factual error 2026-05-10) is a clear signal.
- Future PM-preliminary artifacts (when operationally unavoidable) should treat any "I couldn't find this" finding as a high-prior candidate for Cerebro re-verification rather than as a conclusion.

## 5. Cross-references

**Consumed:**
- `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (58.7 KB) — Cerebro deep-research artifact, now canonical for AI-native-OS field landscape.

**Superseded for canonical content (preserved for historical record):**
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` (52 KB) — PM's preliminary audit. Stays in place as historical record; header updated with supersession marker pointing to Cerebro artifact.

**Related artifacts:**
- `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` — the dispatch.
- `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` — calibration record for the role-confusion error that prompted this dispatch.
- `Plans/PM-NOTE-2026-05-10-ai-aware-not-ai-driven.md` — durable answer to recurring AI-in-scheduler critique; unchanged by this consumption.
- `ALTIMIT-real-world-implementation.md` §C, §J — pre-existing landscape framing, consistent with Cerebro's findings.

**Discipline anchors:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3 (Cerebro deep-research lane), §3.2 (Cerebro voice-profile calibration — second dispatch under new roster; explicit-structure dispatch worked cleanly).
- PL operational posture clarification (2026-05-10 memory entry).

**Cerebro voice-profile calibration (second dispatch under new roster):**
- Output structure followed dispatch specification exactly (header, TL;DR, sources, audit per project, broader landscape, taxonomy, synthesis, calibration notes, cross-references, closure).
- Closure-protocol report-back matched dispatch §7.1 specification exactly: (a) artifact filename, (b) field-shape one-line summary, (c) divergences enumerated with reasoning, (d) time-budget actual.
- Demonstrated explicit disagreement-with-PM-preliminary behavior on OctantOS (correcting fact) and OpenFang (deferring on interpretation) — appropriate division of authority.
- After two dispatches (D15 TLB-shootdown still pending Cerebro return; this field survey complete), Cerebro voice profile is calibrating well. One more dispatch should firm up the profile.

---

## 6. Closure

AI-native-OS field survey work closed. Cerebro artifact is canonical; PM-preliminary historically preserved with supersession marker. Strategic posture unchanged (Bible 23 reinforced, Asterinas remains closest peer, no new architectural watch-this projects). Four substantive divergences captured: OctantOS error (PM calibration), AGT naming collision (external-comms forward marker), taxonomy extension to six senses (canonical reference shift to Cerebro), OpenFang disagreement (settled in PM's favor; Cerebro deferred appropriately). PM dispatch-shaping discipline updated: PM-preliminary remains rare, deferring to Cerebro for project-existence verification specifically.

Active queue unchanged on this consumption — SC-3C D15 (Cerebro TLB-shootdown dispatch) remains the critical-path Cerebro work.

---

**End of PM-NOTE.**

*Cerebro field-survey artifact consumed. Strategic posture unchanged. Four divergences captured. PM-preliminary superseded for canonical content; historical record preserved. PM dispatch-shaping discipline updated re: project-existence verification.*
