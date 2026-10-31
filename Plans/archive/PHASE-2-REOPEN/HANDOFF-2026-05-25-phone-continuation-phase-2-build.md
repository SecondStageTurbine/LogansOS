# HANDOFF: 2026-05-25 phone continuation, Phase 2 BUILD in flight

- **Author:** Codex PM/operator
- **Date:** 2026-05-25
- **Repo:** `github.com/SecondStageTurbine/NeoPlutarch`
- **Desktop workspace:** `E:\AI Automation\AltimitNeo`
- **Origin/main HEAD at handoff:** `79e746f` (`ci: skip heavy tests for docs-only changes`)
- **Purpose:** Self-contained handoff for continuing from Codex on phone when prior chat history is unavailable.

---

## 1. Authority model while PL is unavailable

The active operator may make and record PM decisions without PL ratification when the decision stays within all three boundaries:

1. NeoPlutarch image/brand.
2. `docs/bible.md` and ratified ADR/PLAN constraints.
3. The stated objective: finish all Phase 2 work up to the hardware-only barrier, leaving the OptiPlex hardware session as the final Phase 2 step.

Pause for PL review if a decision changes scope, contradicts the Bible, amends architecture outside a ratified path, or would make a claim that cannot be backed by evidence.

---

## 2. Continuation environment

Use GitHub/origin as the source of truth. On phone, the workspace path may differ from the desktop path. Pull `origin/main` first and verify HEAD before acting.

Minimum startup checks:

```powershell
git fetch origin
git status --short --branch
git log --oneline -8
```

Expected starting point if no newer worker result has landed:

```text
79e746f ci: skip heavy tests for docs-only changes
d612d0a docs: add compatibility boundary rule
ca03d92 docs: add Windows personality roadmap marker
22cb54d plans: accept Phase 2 subphase 2 and dispatch subphase 3
950ece5 docs: close ADR-016 scheduler ring subphase
5aa5baa sched: replace ready queues with fixed rings
14b3697 plans: accept Phase 2 subphase 1 and dispatch subphase 2
f87a26e docs: close phase2 wait inspect subphase
```

If the phone environment uses agent output files, isolate worker output under a phone-local path such as:

```text
/home/<phone-user>/codex_workspace/jarvis_output/YYYYMMDD_HHMMSS_jarvis_result.txt
```

This file path is only a transport convention. The authoritative project state remains the repository plus committed `Plans/` artifacts.

---

## 3. Current Phase 2 state

Phase 2 PLAN is ratified as four BUILD subphases with hardware last.

| Subphase | Status | Key artifact/commit |
|---|---|---|
| 1. Capability reserved-bit activation | Complete | `f87a26e` close report; sys_wait/sys_inspect landed; sys_close deferred |
| 2. ADR-016 A2 scheduler ring | Complete | `950ece5` close report; `5aa5baa` implementation |
| 3. ADR-019 B/C type-level IRQ safety | In flight with Jarvis | Dispatched by `22cb54d`; worker result pending |
| 4. Formal close, non-hardware perf harness, F-004, barrier prep | Pending | Dispatch after subphase 3 is consumed |
| Hardware session | Final Phase 2 step only | OptiPlex 7080 SMP + perf/hardware evidence |

Binding planning artifacts:

- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md`
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md`
- `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md`
- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md`

Operational target remains: complete every non-hardware Phase 2 item before the OptiPlex hardware session.

---

## 4. Active worker lane: Jarvis subphase 3

Jarvis is working subphase 3: ADR-019 B/C type-level IRQ safety.

Expected result shape:

- Source/documentation commits on `main`, or a local worker output file that names those commits.
- Close report under `Plans/`, expected naming similar to:
  `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md`
- Evidence for:
  - mixed-context `SwitchMutex` audit/classification;
  - mixed-context migration to `IrqSafeMutex`;
  - `Channel.state` migration;
  - removal of IRQ-wrapper discipline where structural type safety replaces it;
  - `ipc_rendezvous_translate_fault` / INV-IPC-5 absorption;
  - full suite and SC-3B 200/200 gate if claimed;
  - ADR-019 amendment.

Desktop-local caution at handoff: the working tree contained Jarvis's in-flight source edits plus pre-existing untracked `ai_digest.py`. Do not stage or overwrite unrelated worker edits. Stage only files belonging to the current PM/docs action unless explicitly consuming Jarvis's completed result.

---

## 5. First moves on phone

1. Pull and verify `origin/main`.
2. Check whether Jarvis has returned:
   - read any pasted worker summary;
   - inspect newest commits after `79e746f`;
   - look for the subphase 3 close report under `Plans/`.
3. If Jarvis is complete, consume the result as PM:
   - verify claimed artifacts exist;
   - inspect the close report and diff summary;
   - check that verification claims are concrete and not overbroad;
   - file a PM-DISPOSITION accepting, accepting-with-notes, or rejecting/rerouting.
4. If accepted, dispatch subphase 4/pre-hardware close work.
5. Stop before hardware-only execution unless the OptiPlex session is actually available and PL/hardware procedure conditions are satisfied.

---

## 6. CI note

Commit `79e746f` updated `.github/workflows/ci.yml`:

- workflow-only changes are ignored by the heavy QEMU workflow;
- `tests/**` is ignored while it remains documentation/spec-only;
- `workflow_dispatch` allows manual QEMU runs.

Important behavior: a push containing a code commit and a later docs commit can still trigger CI while GitHub displays the run title from the final docs commit. That was the explanation for CI #76 on commit `950ece5`: the same push included parent source commit `5aa5baa`.

---

## 7. Parallel agent rules

Parallel agents are allowed only with namespace isolation:

- each worker writes to a unique `Plans/` artifact, output file, or subdirectory;
- no worker edits another worker's artifact unless the dispatch explicitly defines a handoff step;
- PM/operator polls results, reads outputs, and decides routing;
- source-code lanes and docs-only lanes must not stage each other's files.

Use RAG (`http://127.0.0.1:8404`) for semantic discovery when available, then verify against live files. Use exact search (`rg`) for exact symbols and filenames.

---

## 8. Current strategic notes

- Windows personality is now a roadmap marker (`ca03d92`) and the compatibility boundary rule is now in the Bible (`d612d0a`).
- Rule: compatibility at the boundary, NeoPlutarch in the core.
- Linux/Windows/native personalities may coexist, but each process has one personality at a time and no personality may pollute native ABI semantics.
- Novelty is allowed: if the clean design has not been done before, NeoPlutarch may invent it, but invention must strengthen the boundary rather than dissolve it.

---

## 9. Hardware-last boundary

Do not close Phase 2 until the final hardware session is complete. Before that session, finish everything that can be completed in QEMU/docs/procedure form:

- subphase 3 accepted;
- subphase 4 non-hardware perf/proxy work accepted;
- F-004 and remaining audit evidence accepted or explicitly bounded;
- barrier checklist satisfied;
- OptiPlex procedure ready.

Hardware-only remainder:

- OptiPlex 7080 SMP verification at `>=4` cores and full core count;
- absolute-latency baseline;
- ADR-019 micro-bench on hardware;
- any F-004 hardware evidence if required;
- final Phase 2 close disposition.
