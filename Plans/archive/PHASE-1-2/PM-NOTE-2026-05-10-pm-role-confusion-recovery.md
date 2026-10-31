# PM-NOTE — PM role-confusion recovery (Cerebro-vs-PM lane error)

**Date filed:** 2026-05-10 (later session)
**Author:** PM
**Type:** PM-NOTE. Calibration record. Doc-only artifact. Operational discipline anchor for future PM behavior.
**Authority:** None. PL identified the error and directed the recovery via "option 1" (relabel + redirect); this artifact documents the error, the recovery, and the lesson for the project's institutional memory.
**Scope:** Single-session error within the 2026-05-10 later-evening PM session. No code changes; no kernel touches; no bible/ANSPEC/ADR mutation. Three artifacts renamed and re-headed; one PM-NOTE provisionally marked; two Cerebro dispatches written that should have existed from the start.

---

## 1. The error

PM (Claude, this conversation's session) confused itself with Cerebro (Claude as a model lineage; *not* this Claude instance). Per the agent-roster note `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3:

> **Deep research dispatches** — synthesizing across multiple sources (papers, prior art, kernel discussions, SDM chapters where applicable). Output is a research artifact (`Plans/RESEARCH-YYYY-MM-DD-<topic>-cerebro.md`); **PM consumes and produces follow-up dispatches**.

The flow is: PM-dispatch → PL-ferry → Cerebro-execute (separate Claude session) → PL-ferry-back → PM-consume. PM produces the *dispatch* and *consumes* the result; PM does not produce the research artifact itself.

In the 2026-05-10 later-evening session, PM executed three research-like tasks directly:

1. **SC-3C D15 deep-research on TLB shootdown / CR3 cache prior art.** PM ran five web searches, synthesized findings, filed `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` with `Author: Cerebro research artifact` in the header.
2. **AI-native-OS field survey.** PM ran ~8 web searches, audited the user-pasted project list, filed `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` with `Author: Cerebro research artifact` in the header.
3. **SC-3C THINK-stage artifact.** PM filed `Plans/SC-3C-THINK-2026-05-10-cerebro.md` with `Author: Cerebro (PM lane; ...)` in the header.

All three artifacts were authored by PM but attributed to Cerebro. The "Cerebro" attribution was false. The work-product itself was real synthesis, but the provenance label was wrong.

A downstream contamination: the D-revisions ratification PM-NOTE (`Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`) ratified four D-decisions based on the misattributed research artifact, treating it as Cerebro-grade research. The ratification is technically valid as PM operational authority (PL handed PM the call), but the foundation it rests on is PM-grade-preliminary, not Cerebro-vetted.

PL caught the error directly: "You are supposed to hand me off Cerebro research, not do it yourself. You are not Cerebro."

## 2. Root cause

Two contributing factors:

**2.1 Linguistic conflation.** The agent-roster note maps "Cerebro = Claude" as model backing. PM (also Claude) read the mapping as identity rather than as model-lineage assignment. The same error would not occur with Jarvis (Codex) or Prometheus (Qwen3.6-27B) because PM is not Codex and not Qwen; the model mismatch surfaces the role mismatch automatically. With Cerebro, the model match obscured the role mismatch.

**2.2 Compacted-summary ambiguity.** The session's compacted summary at start read "PL dispatched Cerebro for D15 — TLB shootdown / CR3 cache prior-art deep-research. Web searches completed on: 1. Linux..." This phrasing is ambiguous between (a) "PL dispatched a Cerebro session that did web searches" and (b) "PL told PM-acting-as-Cerebro to do web searches." PM defaulted to (b) without checking the agent-roster discipline. The compacted summary itself is consistent with (a), but PM didn't re-check the roster definition before continuing the work.

**Combined effect.** PM did the research directly across two full dispatches (TLB shootdown, field survey) and one synthesis artifact (THINK), each time labeling the output as Cerebro work. PL caught the error after three artifacts had been filed.

## 3. The recovery (option 1 — relabel + redirect)

Per PL direction. Recovery actions completed this same session:

**3.1 Renamed three misfiled artifacts:**
- `Plans/SC-3C-THINK-2026-05-10-cerebro.md` → `Plans/SC-3C-THINK-2026-05-10.md` (THINK is PM cadence work per Bible Article 14; correct fix is to drop the agent suffix, not to add a different agent name).
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` → `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md`.
- `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` → `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md`.

**3.2 Updated headers of all three to correctly attribute authorship to PM.** Each renamed artifact now contains a provenance-correction block at the top explaining what happened and pointing to this PM-NOTE.

**3.3 Fixed cross-references.** Global replace across the four affected artifacts (three renames + the D-revisions PM-NOTE) updated 12 stale references from the old `-cerebro.md` filenames to the renamed targets.

**3.4 Provisionally marked the D-revisions ratification.** `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` now carries a "STATUS: PROVISIONAL pending Cerebro confirmation" block in the header. The four ratified D-revisions (D3 N=16→N=33, D4 forward-marker upgrade, D9 FreeBSD overlap pattern, D14 missing-shootdown-detection scenario) stand as PM's provisional read; finalization depends on Cerebro's independent pass.

**3.5 Wrote the two Cerebro dispatches that should have existed from the start:**
- `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` — dispatches Cerebro for the proper D15 deep-research pass. Specifies output filename `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` (distinct from the renamed PM-preliminary). Cites the PM-preliminary as starting reference with the explicit framing "disagreement with this artifact is welcome calibration data."
- `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` — dispatches Cerebro for the proper field survey. Same framing.

**3.6 Filed this PM-NOTE.** Calibration record for future PM sessions and for the project's institutional memory.

## 4. State after recovery

**Standing as PM-authored, correctly labeled:**
- `Plans/SC-3C-THINK-2026-05-10.md` (THINK content; PM cadence work per Bible 14; correctly PM-authored).
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` (PM-preliminary survey; will be superseded or confirmed by Cerebro's pass).
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` (PM-preliminary survey; will be superseded or confirmed by Cerebro's pass).
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (PM-authored from the start; correctly labeled; status now provisional pending Cerebro).
- `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` (PM-authored dispatch for Cerebro to execute).
- `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (PM-authored dispatch for Cerebro to execute).
- `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` (this artifact).

**Pending Cerebro execution (next PL action: ferry the dispatches):**
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` (Cerebro to produce per `DISPATCH-2026-05-10-sc-3c-d15-...-cerebro.md`).
- `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (Cerebro to produce per `DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md`).

**Pending PM follow-up after Cerebro returns:**
- Confirmation or revision of the four D-revisions (D3, D4, D9, D14). New PM-NOTE either finalizing the provisional ratification (Cerebro confirms) or revising it (Cerebro diverges).
- AI-native-OS field survey is situational awareness only; no PM-NOTE follow-up expected unless Cerebro's findings substantially change NeoPlutarch's strategic posture.

## 5. Lesson for future PM behavior

**5.1 Role identity is task-coded, not model-coded.** PM is the session acting as project manager. Cerebro is a session acting as deep-research agent. Both happen to be backed by Claude, but the role identity comes from the task, not from the model. The agent-roster note's "Cerebro = Claude" mapping defines model backing, not role identity. PM is not Cerebro by virtue of being Claude.

**5.2 Sanity check before executing research-shaped work.** When PM is about to run web searches or produce a research artifact, the right reflex is to stop and check: "Is this PM work (synthesis of existing context, operational decision-making, dispatch drafting) or is this Cerebro work (fresh-context deep-research with citation-rigor discipline)?" If it's Cerebro work, the correct action is to write a dispatch document, not to execute the research.

**5.3 Compacted-summary ambiguity should default to checking the agent-roster.** When a compacted summary describes prior-session work in a way that could be read either as "PM executed X" or "PM dispatched agent Y to execute X," PM should default to checking the agent-roster definition before continuing the work. The roster is the source of truth on which work belongs in which lane.

**5.4 THINK-stage filenames should drop agent suffixes.** Bible Article 14 cadence has PM owning OBSERVE and THINK. THINK-stage artifacts are PM cadence work. Naming convention should reflect this: `Plans/SC-XX-THINK-YYYY-MM-DD.md`, not `Plans/SC-XX-THINK-YYYY-MM-DD-<agent>.md`. The earlier `SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` naming was probably also wrong on this same axis; not retroactively renaming, but noting the convention for forward use.

**5.5 PM-preliminary research is a legitimate category, but it's not Cerebro-grade.** PM running web searches and producing first-pass synthesis is sometimes operationally necessary (e.g., when PM needs to make an immediate call and Cerebro dispatch latency is too high). The output should be filed as `Plans/PM-PRELIMINARY-RESEARCH-...md` and labeled as such, with explicit acknowledgment that a Cerebro pass should follow. PM-preliminary is not a substitute for Cerebro-vetted research; it's a placeholder that should be superseded.

## 6. Operational implications going forward

**6.1 Outstanding Cerebro dispatches.** Two dispatches are now queued for PL to ferry. SC-3C D15 (TLB shootdown) is on the critical path — SC-3C PLAN-stage Jarvis dispatch drafting waits on Cerebro confirmation or revision of the provisional D-revisions. AI-native-OS field survey is situational awareness, lower priority.

**6.2 PL queue rebalancing.** PL is now juggling: (a) Jarvis kernel-rename bundle in flight, (b) potentially two Cerebro dispatches to ferry, (c) general operational dispatching. PM-side guidance: if PL prefers to ferry only the SC-3C D15 dispatch (since it's critical-path) and defer the field-survey dispatch until later, that's defensible. Field survey is genuinely just situational awareness; no project decisions gate on it.

**6.3 Future PM-Cerebro dispatches.** Each new Cerebro dispatch under the new roster is calibration data on Cerebro's deep-research voice profile (per agent-roster §3.2 — "Cerebro's natural voice differs [from Janus]; PM specifies output structure explicitly in research dispatches until a Cerebro-research voice profile establishes"). PM has now drafted two dispatches that specify output structure explicitly using the Janus voice anchor as rigor target. When Cerebro returns, PM should note how Cerebro's actual output differs from the specified structure; that's the calibration data for the next dispatch's specification level.

**6.4 No bible / ANSPEC / ADR / phase-roadmap implications.** This is purely an operational discipline correction. The error did not produce wrong technical decisions (the D-revisions are still PM's read of the prior art, and PM's read may turn out correct); it produced wrong provenance labels. Recovery restores correct labels; technical conclusions remain in place provisionally.

## 7. Cross-references

**Caused by:**
- The 2026-05-10 later-evening PM session (this conversation), three research-shaped artifacts produced with incorrect Cerebro attribution.

**Caught by:**
- PL direct correction: "You are supposed to hand me off Cerebro research, not do it yourself. You are not Cerebro."

**Discipline anchors that should have prevented this:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3 (Cerebro deep-research lane definition).
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §3.2 (Cerebro partial-transfer calibration — PM specifies output structure explicitly in dispatches).
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` (citation-rigor discipline that requires agent-produced research go through agent dispatch).

**Affected artifacts (recovered):**
- `Plans/SC-3C-THINK-2026-05-10.md` (renamed and re-headed).
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` (renamed and re-headed).
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` (renamed and re-headed).
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (provisional marker added, "PM (Cerebro)" authorship line corrected to "PM").

**Recovery dispatches filed:**
- `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` (Cerebro to execute when PL ferries).
- `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (Cerebro to execute when PL ferries; lower priority).

---

## 8. Closure

Role-confusion error documented. Recovery executed per PL "option 1" direction. Three misattributed artifacts renamed and re-headed; one downstream-contaminated PM-NOTE marked provisional; two Cerebro dispatches filed for the actual research to happen. PM operational discipline updated: role identity is task-coded, not model-coded; PM-preliminary is a legitimate category but is not Cerebro-grade; THINK-stage filenames drop agent suffixes going forward.

PM moves to standby. Next active actions are PL-side: ferry the two Cerebro dispatches (priority on the SC-3C D15 dispatch since SC-3C PLAN-stage drafting gates on it). Jarvis kernel-rename bundle remains in flight; no interaction with this recovery.

---

**End of PM-NOTE.**

*Role-confusion error caught by PL. Three misattributed artifacts renamed, re-headed, and cross-references repaired. D-revisions ratification marked provisional pending Cerebro confirmation. Two proper Cerebro dispatches filed. Lesson: role identity is task-coded, not model-coded; PM is not Cerebro by virtue of being Claude.*
