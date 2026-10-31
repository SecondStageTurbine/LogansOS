# DISPATCH: Phase 2 THINK v0.1 polish (Prometheus)

- Author: PM
- Date: 2026-05-24
- Agent: Prometheus (Qwen3.6-27B — documenting, amending, document cleanup lane)
- PL ratification: not required for the polish work itself (purely formatting/voice normalization of PM-authored substance). The polished output goes to PL for ratification of the question batch.
- Scope class: small-scope documentation polish.
- Relationship to other workstreams: produces the PL-ratification-ready THINK doc that gates Phase 2 PLAN's PLAN sub-step. Runs sequentially after PM substance is locked (DRAFT filed this commit). Parallel with the in-flight `Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md` (the three-amendments dispatch) — both are Prometheus-lane work; Prometheus prioritizes per its own load.

---

## 1. The task

Polish PM-authored substance into a PL-ratification-ready THINK question batch.

**Input:** `Plans/PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md` (this commit, `255372a`+1).

**Output:** `Plans/PHASE-2-THINK-v0.1-2026-05-24.md`.

The DRAFT contains the binding substance — seven design questions Q1–Q7, each one paragraph, derived from Cerebro's OBSERVE §5 against the four PL scope-class ratifications recorded in `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md`.

The polish job is voice, format, and trim — NOT substance. The DRAFT's §"Note for Prometheus" section enumerates the specific polish targets verbatim.

## 2. Deliverable

Single artifact at `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` containing:

- Frontmatter: title, author=PM (substance) + Prometheus (polish), date, status "Awaiting PL ratification," scope binding to PL-RATIFICATION, predecessor link to DRAFT.
- Preamble (the close-first framing context from DRAFT — load-bearing for PL).
- Q1–Q7 active questions, polished prose, normalized voice, each in single-paragraph addressable form.
- Either: Q11 (Spectre-v2 / IBPB) inclusion decision per PL (if PL rules before Prometheus polish lands, drop or include the section accordingly); or: preserve the DRAFT's "Tell-PL note: Q11" verbatim so PL ratifies its disposition along with the batch.
- Reference section to predecessor DRAFT, PL-RATIFICATION, and Cerebro OBSERVE.

Trim from DRAFT (NOT present in polished output):
- "Deferred-as-measurement-gated" traceability section (PM tracking, not PL-facing).
- "Note for Prometheus (polish target)" section.
- "Predecessor:" and "Format reference:" frontmatter lines (PM-tracking).
- "Status: DRAFT" — replaced with "Awaiting PL ratification."

Title pattern: model on `Plans/PHASE-4-THINK-v0.2-2026-05-21.md` for naming + framing consistency.

## 3. Acceptance criteria

- **C1.** Output exists at `Plans/PHASE-2-THINK-v0.1-2026-05-24.md`.
- **C2.** All seven DRAFT question paragraphs (Q1–Q7) present as separately addressable units. No question merged, no question split, no substance change. PL ratifies per-question.
- **C3.** Preamble preserved (close-first framing context). DRAFT's first preamble paragraph and the seven-question framing of "PL ratifies as a batch; questions resolving to 'do X' become BUILD subphase scope" survives.
- **C4.** Voice normalized — sequential prose written by PM may carry inconsistencies (question-density, framing-verb choice, paragraph-length variance). Polish for one consistent voice across Q1–Q7. The Phase 4 THINK v0.2 Q1–Q12 batch is the reference voice/density.
- **C5.** Q11 disposition explicit — either inclusion (if PL has ratified inclusion before polish) or preservation of the DRAFT's "Tell-PL note: Q11" as a PL-decision-pending block. Default: preserve the note for PL to decide alongside Q1–Q7 ratification.
- **C6.** Sections trimmed from DRAFT (per §2 above) NOT present in polished output. PM-tracking content does not leak into the PL-facing doc.
- **C7.** Push to `origin/main` (`paths-ignore` should skip CI for this docs-only change).
- **C8.** Close report filed at `Plans/CLOSE-REPORT-2026-05-24-phase-2-think-v0.1-polish.md` with: commit hash, list of polish edits (one bullet per question or section touched), explicit confirmation that no substance changed, any open questions for PM.

## 4. Out of scope

- **Substance changes.** Adding or removing questions, changing what a question asks, redirecting a question's framing. The DRAFT's seven questions plus the Q11-tell-PL note are the substance lock. If polish surfaces an ambiguity that requires substance work, file as a NOTES open-question item, do not amend; PM addresses before polish ships.
- **PL ratification of the questions themselves.** That comes after polish lands.
- **The PHASE-2.md / ADR-019 / ADR-013 header amendments** from the parallel dispatch (`Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md`). Those are a separate Prometheus deliverable; this dispatch does not consume or block them.
- **Drafting THINK answers.** Answering Q1–Q7 is PM + PL work after PL ratifies the batch.

## 5. Notes for Prometheus

- This is the third Prometheus dispatch in the project ledger (CI caching `a434992`, doc-reconciliation in flight `255372a`, this one). Voice-normalization polish is squarely in your documenting / amending / document-cleanup lane.
- The DRAFT is verbose by design (it carries PM-tracking content); the polished output is meant to be terser PL-facing. Cutting is part of polish.
- Phase 4 THINK v0.2 Q1–Q12 is the voice/density reference. If you read it and observe a stylistic convention not yet present in the DRAFT, applying it consistently across Q1–Q7 is in scope.
- If you find Q11's "Tell-PL note" awkward to position structurally (it sits between active questions and references), proposing a different placement is in scope — but the substance of the note must survive verbatim. PL ratifies its content along with the Q1–Q7 batch.
- `git push` returning status 1 is cosmetic stderr decoration, not an error.

## 6. References

- `Plans/PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md` (this commit) — input substance.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` (this commit) — scope binding.
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) §5 — origin of the question seeds (14 → 7 after ratification).
- `Plans/PHASE-4-THINK-v0.2-2026-05-21.md` — voice/density format reference.
- `Plans/PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md` (`f0785c8`) — pattern reference for PL's eventual ratification of the polished output.
