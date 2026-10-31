# DISPATCH: Phase 2 THINK v0.2 substance — recommended dispositions for Q1–Q7 (Cerebro)

- Author: PM
- Date: 2026-05-24
- Agent: Cerebro (Claude — deep research + synthesis lane)
- PL ratification: implicit. The PL-RATIFICATION of the THINK v0.1 question batch (`Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md`, this commit) authorizes proceeding to THINK v0.2 substance work per Bible Article 14 cadence.
- Scope class: worker-tier dispatch. Substantial — comparable in scope to the Phase 2 OBSERVE dispatch, possibly larger given the design-density of seven multi-part questions.
- Bible amendment: none anticipated. THINK is "enumerate alternatives, choose" within established Bible framework.
- Inputs available:
  - `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) — the seven ratified questions
  - `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md` (this commit) — batch ratification
  - `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) — OBSERVE substrate (Cerebro's own prior pass; rich context for design alternatives)
  - `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` (`ee5055e`+`7040a49`) — scope ratifications that constrain answer space (close-first, baseline-satisfies-TLB, no PID recycling, A2 is Phase-2 conformance)
  - Audit chain (`5da364d` scope → `c2a4411` Day 1 → `bb1b679` Day 2 → `199fb51` close → `5884ffd` F-005 close → audit `(b)` test items, `(d)` deferrals, foundation invariants)
  - All ADRs (007, 010, 012, 013, 016, 019, 022); Bible (Articles 8, 12, 13, 14, 16.4 especially)
  - The RAG at `http://127.0.0.1:8404` (7,500+ points indexed, verified healthy; semantic discovery preferred over brute-force file enumeration)

## 1. The task

For each of Q1–Q7 in `Plans/PHASE-2-THINK-v0.1-2026-05-24.md`, produce a **recommended disposition** — the answer Cerebro thinks is right, with the reasoning that supports it, plus the alternatives considered and why-not. This is Bible Article 14's "enumerate alternatives, **choose**" step.

Cerebro does NOT make the final ratified call (that's PL after PM refinement). Cerebro produces the recommendation that PM consumes and refines, the final form going to PL for THINK v0.2 ratification.

The seven questions vary in design density. Some have clear paths from existing ADR + audit substrate (Q5 ADR-019 B/C ordering is relatively narrow); some are genuine design exploration (Q4 perf harness measurement methodology, Q7 A2 implementation structure). Cerebro applies depth proportional to design density, not uniform per-question word count.

## 2. Per-question scope guidance

These are scope notes — they sketch where Cerebro's design exploration is expected to spend more vs less effort. They are NOT pre-emption of Cerebro's recommendation; Cerebro may diverge from the sketches with reasoning.

**Q1 — `sys_close` use case + semantics.** The question carries a built-in fallback ("if no use case surfaces, defer"). Cerebro's first work is the use-case search: scan the project (RAG-first) for evidence of long-lived processes holding many channels, application-level "I'm done with this channel" patterns, or anti-patterns the current implicit-close-at-exit produces. If genuine use cases surface, design the semantics (peer-wake with -EPIPE, cap-survive-exit interaction, CLOSE-bit enforcement). If they don't, recommend deferral and note what would trigger revisit.

**Q2 — `sys_wait` semantics + FM-D14.** Five sub-questions; each needs a chosen design. Anchors: ADR-007 (process termination), D8 `exit_state` CAS, D13 single-thread invariant. Look at how Linux `wait()`, Plan 9 `await`, seL4 `Notification`, etc. handle multi-waiter and already-exited cases — RAG may surface project notes; outside research is fine where the design space justifies. FM-D14 retirement check is mechanical (does landing `sys_wait` make `COMPLETED_EXIT_CODES` bounded by handle-presence, or only by wait-arrival).

**Q3 — `sys_inspect` surface.** Two design questions plus one meta question. Design: pick the inspectable-state set (justify each inclusion against Bible 12.2 no-ambient-authority and ADR-010 cap-leak discipline). Meta: is INSPECT useful enough in Phase 2 without Phase 4 cap-passing-over-IPC to justify shipping in close, or does it await Phase 4? Recommendation must address both.

**Q4 — Bible 16.4 perf-regression harness.** Five sub-questions; this is the most design-dense in the batch. The reference-environment choice (sub-question a) is operationally significant — bare-metal OptiPlex needs PL hardware sessions for every measurement; VMware-VM is reproducible but project-self-classified as non-bare-metal; QEMU TCG is reproducible but performance numbers are essentially fiction. Cerebro recommends; PL ratifies. Sub-questions b-e flow from the environment choice. Note the ADR-019 50ns native bench composition (sub-question d) — does the harness subsume or stay orthogonal.

**Q5 — ADR-019 B/C scope + pairing.** Relatively narrow — three sub-questions. Anchors: ADR-019 itself, the audit (b) tests INV-IPC-5 + INV-SYNC-5 (both touch Channel.state lock surface). PM read (not binding for Cerebro): pair B/C with the audit (b) Channel.state tests as one BUILD subphase; B-before-C is correct because it lets C remove the manual-wrapper sites with type-enforcement already in place. Cerebro confirms or diverges with reasoning.

**Q6 — Bare-metal multi-CPU verification protocol.** Five sub-questions, with significant PL-operational input on sub-questions (d) hardware target and (e) procedure-doc shape. Sub-questions (a) acceptance bar, (b) core count, (c) evidence shape are design questions Cerebro can recommend on. Anchor: `PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` is the prior-art procedure template; the Phase-3 OptiPlex evidence doc is the current bare-metal substrate (single-CPU only).

**Q7 — ADR-016 A2 implementation shape.** Four sub-questions. ADR-016 §Decision 1 explicitly defers implementation structure to "Cerebro in a follow-up A2 design dispatch" — this IS that dispatch's substrate. Anchors: per-CPU scheduler topology (ADR-013), D13 single-thread invariant, INV-SYNC-5 ready-queue-capacity audit finding. Cerebro recommends a specific structure (intrusive list / fixed ring / other), bounded-capacity source, INV-SYNC-5 composition, sequencing relative to ADR-019 B/C.

## 3. Deliverable

**Path:** `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md`

Contents:

1. **Frontmatter** — author, date, scope binding (this dispatch + PL-RATIFICATION of v0.1 batch), inputs consumed with commit hashes.
2. **Per-question section (Q1–Q7).** Each section:
   - The ratified question paragraph from THINK v0.1, verbatim (for reference; PL ratifies against this).
   - **Recommended disposition** — Cerebro's chosen answer, sub-question by sub-question.
   - **Reasoning** — why this answer, anchored in ADRs / audit / Bible / RAG findings.
   - **Alternatives considered and why-not** — at least one real alternative per question; the reasoning that ruled it out.
   - **Open dependencies (if any)** — items where Cerebro can recommend but PL/PM resolution is required (e.g. Q6 (d) hardware target — Cerebro recommends a target shape, PL ratifies the specific hardware).
3. **§8 Cross-question synthesis** — any patterns or interactions that emerged across questions (e.g., Q5 + Q7 may have ordering interaction; Q4 perf-harness environment choice may interact with Q6 bare-metal protocol environment).
4. **§9 Open questions for PM** — anything Cerebro hit that needs PM judgment to decide before THINK v0.2 polish.
5. **References** as usual.

## 4. Acceptance criteria

- **C1.** Each of Q1–Q7 receives a recommended disposition with reasoning, alternatives, and (where applicable) explicit open dependencies for PL.
- **C2.** All sub-questions called out in §2 of this dispatch are addressed within each question's section.
- **C3.** Cross-question synthesis section (§8) names interactions or notes their absence.
- **C4.** Open-questions-for-PM section (§9) catalogs items needing PM/PL resolution (likely few; recommendations may be made even where dependencies exist).
- **C5.** Deliverable filed at the §3 path and pushed to `origin/main`.
- **C6.** Cerebro does NOT make final ratified calls (those are PL's). Does NOT amend audit artifacts. Does NOT draft ADRs (recommendations may reference proposed-ADR shapes; actual drafting is PM-dispatched separately if PL ratifies). Does NOT propose bible amendments (flag in §9 if one seems warranted).

## 5. Out of scope

- THINK v0.2 polishing (post-Cerebro Prometheus pass, or PM-direct depending on PL preference).
- THINK v0.2 final ratification.
- PLAN sub-step BUILD-subphase enumeration.
- The audit (b)-disposition test items, beyond their composition with Q-question dispositions.
- Code changes, ADR drafting, bible amendments.
- The parallel doc-reconciliation Prometheus dispatch.

## 6. Notes for Cerebro

- **Use the RAG aggressively.** Phase 2 OBSERVE benefited substantially from RAG discovery — same pattern applies here, especially for design-context queries ("how was X discussed previously", "what prior art exists in project notes on Y"). The RAG knows the project's accumulated design thinking better than any single doc.
- **Outside research is in scope where design space justifies.** Q2 multi-waiter semantics, Q4 perf-harness measurement methodology, Q7 ready-queue structure choices are all design areas where outside reference (Linux, seL4, BSD, Plan 9, real-time OS literature) may inform good-design recommendations. Citations welcome.
- **Time-bound: 4–12 hours of focused work.** This is larger than OBSERVE (which was 4–8h). Q4 + Q7 alone could each be 2-3h of design exploration. If 12h is exceeded, checkpoint with partial deliverable + clearly-named continuation point in §9; PM dispatches continuation rather than press.
- **Recommendation density: prefer fewer well-reasoned recommendations to many lightly-reasoned ones.** A recommendation with three sentences of "here's the choice; here's why; here's the alternative ruled out" beats five sentences of qualified hedging. PL ratifies the recommendations against this dispatch's framing; better to commit to a recommendation and accept PM/PL refinement than to surface six options with no chosen direction.
- **If a recommendation requires a scope-class call PM/PL hasn't yet made (different from sub-question open dependency), flag in §9.** Don't unilaterally scope-decide.
- **`git push` returning status 1 is cosmetic stderr decoration.** Standard pattern.

## 7. Close

When the deliverable is filed and pushed, Cerebro returns with the commit hash. PM consumes the recommendations, refines into a PL-ratification-ready form (either directly or via a Prometheus polish dispatch — PM's call after seeing the v0.2 substance), routes to PL, ratification follows.

On PL ratification, PM enters PLAN sub-step: BUILD subphase enumeration against the answered questions. OBSERVE §6 candidate-subphases (capability reserved-bit activation, formal-close verification, ADR-019 B/C + ADR-016 A2 defense-in-depth) get refined into concrete subphase definitions with goal milestones, dependencies, anti-scope, and /simplify gates.

## 8. References

- `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) — the seven ratified questions.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md` (this commit) — batch ratification.
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) §5/§6 — substrate.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` (`ee5055e`+`7040a49`) — scope ratifications constraining answer space.
- Bible Articles 8, 12, 13, 14, 16.4.
- ADRs 007, 010, 012, 013, 016, 019.
- The pre-Phase-2 audit chain (audit close `199fb51`, F-005 close `5884ffd`).
- RAG service at `http://127.0.0.1:8404`; CLAUDE.md §"NeoPlutarch RAG" for usage discipline.
- Format reference: `Plans/PHASE-4-THINK-v0.2-2026-05-21.md` (the v0.2 answered batch from Phase 4).
