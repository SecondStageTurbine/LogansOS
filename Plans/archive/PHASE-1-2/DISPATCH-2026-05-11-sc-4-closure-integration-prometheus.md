# DISPATCH — SC-4 closure integration into PHASE-2.md + INDEX.md (Prometheus)

**Date filed:** 2026-05-11
**From:** PM
**To:** Prometheus (Qwen3.6-27B)
**Ferry:** PL ferries to Prometheus session.
**Status:** Post-Jarvis-close integration. SC-4 closed at commit `57f3cb1`. Single commit; doc-only; parallel-with-anything-safe (kernel substrate complete).

---

## 1. Mandate

Draft + integrate SC-4 closure documentation into `docs/phases/PHASE-2.md` and `Plans/INDEX.md`. **No pre-staging this cycle** (process gap noted in close report §5); draft + integrate in one pass.

Parallel to SC-3C closure integration (commit `165a2e2`) — same shape, same file targets.

## 2. Inputs to consume

1. **`Plans/CLOSE-REPORT-2026-05-11-sc-4.md`** — SC-4 close report (just filed by PM). Canonical source for sub-commit attribution, gate results, forward markers, calibration notes.
2. **Three verification reports:**
   - `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-2026-05-11.md`
   - `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-2026-05-11.md`
   - `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-2026-05-11.md`
3. **Voice anchors:**
   - `docs/phases/PHASE-2.md` — read the existing SC-3C section (commit `165a2e2` integrated it) as the template for the SC-4 section.
   - `Plans/INDEX.md` — read for entry format.
   - `Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md` — the SC-3C pre-staging artifact; useful as a format reference even though SC-4 didn't pre-stage.

## 3. Scope: integration in one pass

### 3.1 PHASE-2.md SC-4 section

Insert new section after the SC-3C section (currently the latest, integrated at commit `165a2e2`), before "## Transition from BUILD-1" or equivalent.

Section heading: `## BUILD-3 sub-commits 5+6 (SC-4) — Q8.a/b capability table SMP + Q8.c blocked-thread-kill walk`

Format matches SC-3C section template. Substantive content:
- Closed 2026-05-11 via commits `c18646e` (5a), `c02abf7` (5b), `57f3cb1` (6).
- Verification logs: three filenames per §2.
- Closure report: `Plans/CLOSE-REPORT-2026-05-11-sc-4.md`.
- Gate scores: cargo k PASS, cargo k --no-default-features PASS, cargo neo-test all PASS, GitHub CI run 25705327371 green.
- Mechanism named: N/A — correctness-implementation work for ratified D1-D18.
- "What landed" subsection: three bullets matching sub-commit scopes (5a SwitchMutex retrofit; 5b Phase A cr3_for_thread retirement + exiting flag + exit_state CAS; 6 blocked-thread-kill walk + close_peer fix + zombify_thread primitive).
- "D-decisions ratified" subsection: 18 D-decisions per `Plans/SC-4-THINK-2026-05-11.md` + Cerebro D5 reframing of D4 to Phase A per `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`.
- "Verification Log" subsection: the three sub-commit verification reports + closure report.
- "Course corrections" subsection: one operational adjustment (PL commit `5a88925` for transient neo-test retry; parallel to SC-3C `e7782fc` pattern).
- "Calibration" subsection: Cerebro D5 architectural reframing as headline; PM operational discipline updated to incorporate "have I framed the right question?" pre-dispatch gate.

### 3.2 PHASE-2.md ADR ledger

No new ADRs from SC-4. Verify by reading the existing ADR ledger; if Prometheus's read of the SC-4 artifacts suggests an ADR-worthy item (e.g., FM-D5.1 spin::RwLock pathological warning could warrant a normative ADR), surface as a finding for PM consideration rather than drafting a row.

### 3.3 PHASE-2.md Phase 2+ forward markers

Add six new SC-4-introduced forward markers per close report §6.2 to the existing "Forward reference: Phase 2+ TLB management optimizations" section (or split into a separate "Forward reference: Phase 2+ capability + process-exit optimizations" subsection if Prometheus judges the topic distinct enough):

- FM-D5.1: spin::RwLock pathological at 4-64 CPU range.
- FM-D5.2: arc-swap Phase B fallback deferred.
- FM-D5.3: illumos doors architectural reference for cross-process IPC.
- FM-D5.4: PROCESSES single-Mutex correctness boundary validated.
- FM-D13: Single-thread-per-process invariant for blocked-thread-kill walk.
- FM-D14: COMPLETED_EXIT_CODES growth bound deferred to sys_wait consumer.

Cite source artifact for each marker (close report §6.2 + Cerebro D5 §5.4, §6, §10 + SC-4 THINK §7 D13/D14).

### 3.4 INDEX.md catalog entries

Add entries for SC-4 artifacts. Match existing INDEX.md format. Required entries:

- `Plans/SC-4-THINK-2026-05-11.md` (THINK artifact, 39.3 KB)
- `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` (Cerebro D5 dispatch)
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` (Cerebro D5 research artifact, 52.1 KB)
- `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`
- `Plans/DISPATCH-2026-05-11-sc-4-plan-stage-jarvis.md` (PLAN-stage dispatch)
- `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-2026-05-11.md`
- `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-2026-05-11.md`
- `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-2026-05-11.md`
- `Plans/CLOSE-REPORT-2026-05-11-sc-4.md`
- `Plans/DISPATCH-2026-05-11-sc-4-closure-integration-prometheus.md` (this dispatch)

## 4. Single commit

Title:

```
docs/phases: integrate SC-4 closure section + Phase 2+ capability/exit forward markers
```

Body (substance; Prometheus revises):

```
Integrates the SC-4 closure documentation per
Plans/CLOSE-REPORT-2026-05-11-sc-4.md.

PHASE-2.md additions:
- New section "BUILD-3 sub-commits 5+6 (SC-4) — Q8.a/b capability table
  SMP + Q8.c blocked-thread-kill walk" inserted after the SC-3C section.
  References commits c18646e (5a), c02abf7 (5b), 57f3cb1 (6).
- New forward-reference subsection documenting 6 deferred items:
  spin::RwLock pathological warning, arc-swap Phase B fallback, illumos
  doors architectural reference, PROCESSES single-Mutex correctness
  boundary, single-thread-per-process invariant, COMPLETED_EXIT_CODES
  growth bound deferred.
- ADR Ledger unchanged (SC-4 used ratified D1-D18 design space; no new
  ADRs).

INDEX.md additions: catalog entries for 10 SC-4-related artifacts (THINK,
Cerebro D5 dispatch + research + consumption, PLAN dispatch, three
sub-commit verification reports, closure report, this integration
dispatch).

See:
- Plans/CLOSE-REPORT-2026-05-11-sc-4.md
- Plans/SC-4-THINK-2026-05-11.md
- Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md
- Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md
- Plans/CLOSE-REPORT-2026-05-11-sc-3c.md (precedent)
```

## 5. Verification gates

Doc-only integration; no `cargo k` gates. Required checks:

1. Diff review: two files changed (`docs/phases/PHASE-2.md`, `Plans/INDEX.md`).
2. Cross-reference integrity: every filename cited exists in `Plans/` or `docs/`. Spot-check 5 random citations.
3. SC-3C section unchanged. Existing forward-reference subsection structure preserved (new markers added, existing markers untouched).
4. INDEX.md format-conformance.
5. No `kernel/src/*` touches.

## 6. Push

Branch convention: `prometheus/sc-4-closure-integration-2026-05-11`. Clean fast-forward to `origin/main`.

## 7. Verification report

File `Plans/SC-4-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md`. Format matches `Plans/SC-3C-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md` precedent.

## 8. Stop conditions

- Diff exceeds 200 lines → STOP, surface to PM. SC-3C closure integration was 88 insertions / 3 deletions; SC-4 should be similar magnitude.
- Push rejected on origin/main → STOP, surface to PL.
- ADR-worthy finding surfaces during integration (e.g., FM-D5.1 spin::RwLock warning may warrant a normative ADR) → STOP, surface to PM as a candidate for PM consideration (do NOT draft an ADR row directly).

## 9. Closure protocol

After commit + push + verification report:

1. Prometheus reports back to PL with: (a) commit SHA, (b) verification report filename, (c) diff summary, (d) any incidental observations.
2. PL ferries to PM.
3. PM consumes; no follow-up PM-NOTE required unless something unexpected surfaced.

---

## 10. Cross-references

**Inputs:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-4.md`
- `Plans/SC-4-THINK-2026-05-11.md`
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md`
- `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`
- Three SC-4 sub-commit verification reports.

**Precedent:**
- `Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md` — SC-3C pre-staging artifact.
- Commit `165a2e2` — SC-3C closure integration (the structural precedent).
- `Plans/SC-3C-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md` — verification report format anchor.

---

**End of dispatch.**

*SC-4 closure documentation integration. Draft + integrate in one pass (no pre-staging this cycle). Single commit. Estimated diff ~80-100 line touches matching SC-3C precedent.*
