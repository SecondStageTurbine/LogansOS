# DISPATCH: Phase 2 PLAN substance - BUILD-subphase enumeration (Prometheus)

- Author: PM
- Date: 2026-05-25
- Agent: Prometheus (Qwen3.6-27B - documenting, amending, document cleanup lane)
- PL routing decision: PL chose Prometheus for this docs-only PLAN-substance pass after the initial Cerebro-targeted dispatch landed.
- Supersedes: `Plans/DISPATCH-2026-05-25-phase-2-plan-substance-cerebro.md`
- PL ratification: explicit. `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` ratifies all seven THINK v0.2 dispositions and both PL-operational items.
- Scope class: PLAN-stage substance dispatch. Documentation/planning only; no source changes.
- Relationship to other workstreams: opens Phase 2 PLAN after THINK v0.2 ratification. NVMe FLUSH durability diagnosis remains paused until the serial-console adapter is installed; if PL resumes that hardware diagnosis, it may briefly preempt this PLAN lane.

---

## 1. Inputs

Read these first:

- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` (`349151e`) - ratified answered dispositions.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` - ratification of all seven answers plus Q4/Q6 operational items.
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-polish-close.md` (`4469c6b`) - PM acceptance of polish and next-gate framing.
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) - full reasoning behind the ratified dispositions.
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (`8bdb4be`) - PM rulings on Q5 re-routing, Q7 ring, and PL-operational items.
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` (`255372a`) and `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) - OBSERVE substrate.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` - binding scope-class calls.
- Audit chain: `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md`, `Plans/PM-DISPOSITION-2026-05-24-f005-close.md`, and the audit (b) items named in THINK v0.2 (`F-004`, `INV-IPC-5`, `INV-CAP-11`, `INV-SYNC-5`).
- Bible Articles 1.2, 4.2, 8.5.3, 12.2, 13, 14, and 16.4; ADRs 007, 010, 013, 016, 019.
- RAG service at `http://127.0.0.1:8404` for semantic discovery. Use it for "where is this done?" exploration; verify final claims against live files.

## 2. The Task

Produce the Phase 2 PLAN substance: concrete BUILD-subphase enumeration from the ratified THINK v0.2 answers.

The expected starting shape is the ratified two-subphase synthesis:

1. **Capability reserved-bit activation** - `sys_wait` + `sys_inspect`; `sys_close` deferred; absorbs `INV-CAP-11`.
2. **Formal close + defense-in-depth** - ADR-016 A2 + `INV-SYNC-5`; ADR-019 B then C + `INV-IPC-5`; tiered performance harness; bare-metal multi-CPU verification; remaining audit (b) `F-004`; Q4/Q6 share a hardware session where possible.

Your job is not to re-open THINK. Your job is to turn those ratified answers into a PLAN-ready artifact: milestones, dependency order, acceptance gates, anti-scope, dispatch lanes, and /simplify checks.

If the two-subphase shape should be split further for operational safety, you may recommend a split, but only as a PLAN-level subdivision justified against Bible Article 14 /simplify. Do not overturn ratified THINK decisions.

## 3. Deliverable

**Path:** `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md`

Contents:

1. **Frontmatter** - author, date, scope binding, inputs consumed with commit hashes.
2. **PLAN overview** - one-paragraph statement of the Phase 2 close objective and the ratified two-subphase shape.
3. **Subphase plan sections** - for each BUILD subphase or recommended subdivision:
   - Goal milestone.
   - In-scope implementation/test/documentation items.
   - Dependencies and ordering.
   - Anti-scope.
   - Acceptance criteria and verification gates.
   - Expected close artifacts.
   - Recommended execution lane (Jarvis/Codex, Cerebro, Prometheus, PL hardware session).
4. **Audit (b) mapping table** - map `F-004`, `INV-IPC-5`, `INV-CAP-11`, and `INV-SYNC-5` to the exact subphase, expected test/scenario, and acceptance signal.
5. **Hardware/perf session plan** - compact plan for combining Q4 and Q6: what the OptiPlex session must capture, where VMware fits, where QEMU-TCG proxies gate, and what procedure document is needed before execution.
6. **/simplify pass** - explicit cuts/deferments from PLAN, including `sys_close`, full FM-D14 structural retirement, AMD cross-vendor run, PID recycling, Spectre-v2/IBPB, PCID/ASID, RCU, load-balancing, x2APIC, hot-plug, and unrelated Phase 3 NVMe durability work.
7. **Risks and stop conditions** - small list only; focus on risks that affect BUILD sequencing or gate reliability.
8. **References**.

## 4. Acceptance Criteria

- **C1.** All seven ratified THINK v0.2 dispositions are reflected without re-opening their choices.
- **C2.** PLAN enumerates concrete BUILD subphases or justified subdivisions with goal milestones, dependencies, anti-scope, acceptance gates, expected artifacts, and execution lanes.
- **C3.** The four audit (b) items are mapped to subphases and verification signals: `INV-CAP-11`, `INV-SYNC-5`, `INV-IPC-5`, `F-004`.
- **C4.** Q4/Q6 shared hardware/perf session is specified enough for PM to dispatch a procedure-writing pass.
- **C5.** /simplify pass is explicit and preserves all ratified deferrals/anti-scope.
- **C6.** No code changes, no ADR drafting, no bible amendments, no direct BUILD dispatches.
- **C7.** Deliverable filed at the section 3 path and pushed to `origin/main`.

## 5. Out of Scope

- Implementing `sys_wait`, `sys_inspect`, A2, ADR-019 B/C, performance harness code, or test scenarios.
- Writing the final polished PLAN artifact. PM consumes this substance first.
- Drafting the bare-metal procedure itself. This dispatch should define the procedure's PLAN requirements, not write the operator walkthrough.
- Executing the OptiPlex hardware session.
- Resuming or fixing NVMe FLUSH durability. That remains governed by `Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md`.

## 6. Notes for Prometheus

- Treat `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` as normative. `CEREBRO-OUTPUT` and PM-DISPOSITION artifacts explain why; they do not reopen the choices.
- This is a docs-only PLAN-substance pass, but it is not mere formatting. Produce the actual PLAN skeleton PM can consume and route for ratification.
- Use RAG for discovery, then verify against live files. If RAG is stale or degraded, fall back to direct reads and say so in the artifact.
- Keep the output PLAN-shaped. This is not another OBSERVE or THINK pass.
- Be explicit about sequencing boundaries: where a BUILD item can be parallelized, where it cannot, and where a hardware session gates closure.
- If you find a mismatch between ratified PLAN shape and live code reality, surface it as a PLAN risk/stop condition rather than silently changing scope.
- `git push` returning status 1 can be cosmetic stderr decoration; verify remote HEAD if needed.

## 7. Close

When the deliverable is filed and pushed, return the commit hash and a short summary of:

- Final subphase count.
- Any recommended split from the ratified two-subphase starting shape.
- Audit (b) mapping outcome.
- Hardware/perf session requirements.
- Any stop conditions PM must resolve before PLAN polish/ratification.

PM will consume the artifact by PM-DISPOSITION, then either polish directly or dispatch a final Prometheus polish pass only if needed before PL ratifies PLAN.

## 8. Tell-Prometheus Text

```text
Prometheus - Phase 2 PLAN substance dispatch. Repo on main after PL ratification of THINK v0.2. Active spec: Plans/DISPATCH-2026-05-25-phase-2-plan-substance-prometheus.md. Note: this supersedes the earlier Cerebro-targeted dispatch because PL routed the docs-only pass to Prometheus. Inputs: Plans/PHASE-2-THINK-v0.2-2026-05-24.md, Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md, PM polish/substance dispositions, OBSERVE close, scope-class ratifications, and audit chain. Output: Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md. Task: turn the ratified seven answers into PLAN-ready BUILD-subphase enumeration with milestones, dependencies, anti-scope, acceptance gates, audit (b) mapping, Q4/Q6 hardware/perf session shape, execution lanes, and /simplify cuts. Do not reopen THINK or write code. Push to origin/main and return commit hash.
```
