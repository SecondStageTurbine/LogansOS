# PL-RATIFICATION: Phase 2 THINK v0.1 question batch

- Recorded by: PM
- Date: 2026-05-24
- Ratifying authority: Project Lead
- Ratification form: "Yes to all" (this session, in response to PM brief naming the seven polished questions for batch ratification)
- Source artifact: `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (polished, `46426d9`)
- Pattern reference: `Plans/PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md` (`f0785c8`)

---

## 1. Ratified question batch

All seven design questions in `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` are ratified as the THINK substrate for Phase 2 PLAN. Each is binding as a question PLAN must resolve before BUILD subphase enumeration. The Q11 deferral disposition recorded in the polished doc's "Q11 Disposition" section (deferred to dedicated ADR at future trigger) is also ratified.

| Q# | Topic | Disposition |
|---|---|---|
| **Q1** | `sys_close` use case + semantics | Ratified as question. Built-in fallback disposition (if no use case surfaces → defer `sys_close`, ship sys_wait+sys_inspect) is part of the ratified framing. |
| **Q2** | `sys_wait(handle)` semantics + FM-D14 retirement | Ratified as question. Five sub-questions (a-e) all in scope for THINK v0.2 answer-development. |
| **Q3** | `sys_inspect(handle)` surface | Ratified as question. Includes the meta-question of Phase 2 vs Phase 4 inclusion. |
| **Q4** | Bible Art 16.4 perf-regression harness | Ratified as question. Five sub-questions (a-e) in scope. |
| **Q5** | ADR-019 B/C scope + pairing | Ratified as question. Three sub-questions (a-c) in scope. |
| **Q6** | Bare-metal multi-CPU verification protocol | Ratified as question. Five sub-questions (a-e) in scope; (d) hardware target and (e) procedure shape benefit from PL operational input. |
| **Q7** | ADR-016 A2 implementation shape | Ratified as question. Four sub-questions (a-d) in scope. ADR-016 explicitly says A2 implementation structure is "Cerebro follow-up dispatch" — Q7 IS that follow-up substrate. |
| **Q11 deferral** | Spectre-v2 / IBPB on context switch | Ratified deferral to dedicated future-triggered ADR (already in PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items §1.5; redundantly confirmed by THINK-batch ratification). |

No question rejected. No question amended. No question deferred from the THINK batch (Q11 is recorded as previously deferred, not a fresh deferral).

## 2. What "ratified as question" means

Bible Article 14 cadence: OBSERVE (context gathering) → THINK (enumerate alternatives, **choose**) → PLAN (criteria, /simplify) → BUILD/VERIFY. PL's ratification at this stage establishes the seven questions as the design questions THINK must choose answers to. The CHOOSING work — developing recommended answers per sub-question, ratifying them — is THINK v0.2 substance, not THINK v0.1 substance.

THINK v0.1 (this artifact + the polished question batch) closes the "what are we answering" half of THINK. THINK v0.2 will close the "what are the answers" half.

## 3. Next steps

1. **PM dispatches Cerebro for THINK v0.2 substance development.** Same commit as this ratification (`Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-cerebro.md`). Cerebro produces recommended dispositions across Q1-Q7 sub-questions, with reasoning anchored in OBSERVE artifacts, ADRs, audit findings, and the RAG.
2. **PM consumes Cerebro output and refines** into a PL-ratification-ready batch (similar pattern to OBSERVE → THINK v0.1 DRAFT refinement).
3. **Optional Prometheus polish** of the answered batch into final form.
4. **PL ratifies** the answered batch (THINK v0.2 ratification).
5. **PM enters PLAN sub-step** — translate ratified answers into BUILD subphase enumeration. OBSERVE §6 candidate-subphase enumeration (capability reserved-bit activation, formal-close verification, ADR-019 B/C + ADR-016 A2 defense-in-depth) gets refined against the answered THINK questions.
6. **/simplify gate** before each BUILD subphase per Bible Article 14.

Parallel: Prometheus doc-reconciliation dispatch (`Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md` at `255372a`) remains pending route. Independent of THINK v0.2; PL routes when ready.

## 4. References

- `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) — the ratified question batch.
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.1-polish-close.md` (`96d0b46`) — polish acceptance + naming this ratification as next gate.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` — scope binding for the THINK v0.1 batch.
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) — substrate the questions derive from.
- `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-cerebro.md` (this commit) — the THINK v0.2 substance dispatch.
- Pattern reference: `Plans/PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md` (`f0785c8`).
