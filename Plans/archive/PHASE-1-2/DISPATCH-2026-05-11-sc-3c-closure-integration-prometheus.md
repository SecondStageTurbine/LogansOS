# DISPATCH — SC-3C closure integration into PHASE-2.md and INDEX.md (Prometheus)

**Date filed:** 2026-05-11
**From:** PM
**To:** Prometheus (Qwen3.6-27B, docs + cleanup + small-coding-trial lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.2)
**Ferry:** PL ferries to Prometheus session **after** Jarvis SC-3C BUILD closes (all three sub-commits 4a + 4b + 4c landed on `origin/main`; closure verification reports filed in `Plans/`).
**Status:** Queued. Gated on Jarvis SC-3C close. **Do NOT execute before Jarvis SC-3C closure event.** This dispatch is filed in advance so it's ready to ferry when the gate opens; the work itself does not begin until Jarvis closure verification reports are available to consume.

---

## 1. Mandate

Execute the integration plan from `Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md` §E: take the pre-staged PHASE-2.md and INDEX.md additions, fill in the post-Jarvis placeholders (commit SHAs, gate scores, verification log dates), apply the two PM-noted format-precision fixes, integrate into the live files, single commit, push.

## 2. Pre-execution gate (verify before starting)

Confirm before beginning work:

1. **Jarvis SC-3C closure complete.** All three commits on `origin/main`:
   - Sub-commit 4a (Q7.b CR3 cache).
   - Sub-commit 4b (Q7.c TLB shootdown machinery).
   - Sub-commit 4c (Q7.c call-site wiring).
   Use `git log --oneline -5 origin/main` to verify the three sub-commits are present with their expected commit messages.
2. **Three verification reports filed:**
   - `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-<date>.md`
   - `Plans/SC-3C-SUB-COMMIT-4B-Q7C-MACHINERY-VERIFICATION-<date>.md`
   - `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-<date>.md`
3. **Closure report filed** (Jarvis or PM authors at SC-3C close; verify existence).

If any gate item is missing, STOP and surface to PL — the dispatch should not have been ferried yet.

## 3. Inputs to consume

1. **`Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md`** — the pre-staged draft. Contains §A (PHASE-2.md SC-3C section template with placeholders), §B (ADR ledger update determination), §C (Phase 2+ forward markers), §D (INDEX.md catalog entries), §E (integration plan).
2. **`Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md`** §2.1 and §2.2 — **two PM-noted format-precision fixes to apply during integration:**
   - **Fix §2.1:** "Fifteen D-decisions" → "Fourteen BUILD-relevant D-decisions (D1–D14) plus one meta-decision (D15 — Cerebro deep-research dispatch). Four BUILD decisions revised post-prior-art research..."
   - **Fix §2.2:** Spectre-v2 BTB forward marker citation source: the citation should point to the SC-3C PLAN-stage dispatch §2 context and the Cerebro D15 research artifact §2 input context, NOT to `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md §4 item 7` (which doesn't exist; that PM-NOTE has only four D-revisions enumerated).
3. **Jarvis SC-3C verification reports** (existence verified in §2 above) — source of placeholder fill-ins:
   - Commit SHAs (4a, 4b, 4c).
   - Gate scores (`cargo k`, `cargo neo-test all`, 200/200 NEO_SMP=4, D14 missing-shootdown-detection scenario).
   - Verification log filenames (the three SUB-COMMIT-4*-VERIFICATION-*.md filenames).
   - Closure date.
   - Any course corrections (likely "No course corrections" if BUILD landed clean; populate with actual content if BUILD surfaced any).
4. **`docs/phases/PHASE-2.md`** — target file for §A and §C integrations. SC-3C section inserts immediately after the existing SC-3B section (look for `## BUILD-3 sub-commit 3 (SC-3B)` and place SC-3C section after that section closes, before "## Transition from BUILD-1"). Forward markers from §C integrate as a new subsection or extend the existing "Forward reference" section.
5. **`Plans/INDEX.md`** — target file for §D integrations. Add catalog entries for all 8 SC-3C artifacts enumerated in closure-prep §D plus the three verification reports + closure report (the post-Jarvis placeholder group, now resolved with actual filenames).

## 4. Scope: integration of pre-staged draft into live files

### 4.1 PHASE-2.md SC-3C section integration

Take closure-prep §A. Apply the §2.1 fix (Fifteen → Fourteen D-decisions framing). Fill placeholders from Jarvis verification reports. Insert into PHASE-2.md after the SC-3B section.

### 4.2 PHASE-2.md ADR Ledger

Per closure-prep §B determination: no new ADR rows. Verify Jarvis verification reports did not surface a course correction that would warrant a new ADR; if they did, surface to PM before adding a row (governance act requires PM ratification).

### 4.3 PHASE-2.md Phase 2+ forward markers

Take closure-prep §C. Apply the §2.2 fix (Spectre-v2 BTB citation source). Integrate as a new "## Forward reference: Phase 2+ TLB management optimizations" section. Position: in the "Forward reference" cluster near the end of PHASE-2.md (after the existing "Forward reference: async discipline for Phase 8+ libneo" section).

### 4.4 INDEX.md catalog entries

Take closure-prep §D. Replace the "post-Jarvis placeholder group" with actual entries for the three verification reports + closure report (now resolved). Integrate all entries into INDEX.md in date-sorted position or topic-clustered position per existing INDEX.md conventions (match Prometheus's read of the file's organization).

## 5. Single commit

Title:

```
docs/phases: integrate SC-3C closure section + Phase 2+ TLB forward markers
```

Body (substance; Prometheus revises wording, fills SHAs):

```
Integrates the SC-3C closure documentation pre-staged in
Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md.

PHASE-2.md additions:
- New section "BUILD-3 sub-commit 4 (SC-3C) — Q7.b CR3 cache + Q7.c IPI
  TLB shootdowns" inserted after the SC-3B section. References commits
  <4a-sha>, <4b-sha>, <4c-sha>; verification logs and closure report
  cited.
- New "Forward reference: Phase 2+ TLB management optimizations"
  subsection documenting 7 deferred items: PCID/ASID, T2 targeted
  shootdown with strictly-vs-loosely-accurate cpumask choice, lazy TLB
  with freed_tables exception, batching, INVLPGB/RAR, Spectre-v2 BTB.
- ADR Ledger unchanged (SC-3C used already-ratified machinery; no new
  ADRs).

INDEX.md additions: catalog entries for 11 SC-3C-related artifacts
(THINK, D-revisions ratification, Cerebro D15 dispatch + research +
consumption, PM-preliminary, PLAN dispatch, three sub-commit
verification reports, closure report).

Two minor format-precision fixes applied per
Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md
§2.1 and §2.2:
- "Fourteen BUILD-relevant D-decisions (D1-D14) plus one meta-decision
  (D15)" instead of "Fifteen D-decisions".
- Spectre-v2 BTB citation updated to point at the SC-3C PLAN dispatch
  context and Cerebro D15 artifact, not the D-revisions PM-NOTE.

See:
- Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md
- Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md
- Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-
  finalized.md
- (SC-3C closure report and three sub-commit verification reports,
  filenames inline)
```

## 6. Verification gates

Doc-only integration; no `cargo k` gates apply. Required checks:

1. **Placeholder fill audit.** All `<PLACEHOLDER: ...>` markers in closure-prep §A must be resolved before commit. Grep the diff for `<PLACEHOLDER`; count should be 0.
2. **Format-precision fixes applied.** Verify §2.1 (Fourteen + meta-decision framing) and §2.2 (Spectre-v2 BTB citation source) in the integrated text.
3. **Cross-reference integrity.** Every filename cited in the integrated PHASE-2.md additions exists in `Plans/` or `docs/`. Spot-check 5 random citations.
4. **PHASE-2.md SC-3B section unchanged.** The new SC-3C section is inserted after the SC-3B section; the SC-3B section content stays identical (diff should show only insertions, not modifications, within the existing SC-3B block).
5. **INDEX.md format-conformance.** New entries match existing entry format.
6. **No `kernel/src/*` touches.**

## 7. Push

Branch convention: `prometheus/sc-3c-closure-integration-2026-05-<date>`.

```bash
git push origin HEAD:main
```

Clean fast-forward expected. If rejected on `origin/main` advancement (other dispatches landed between Jarvis SC-3C close and this integration), surface to PL.

## 8. Verification report

File `Plans/SC-3C-CLOSURE-INTEGRATION-VERIFICATION-2026-05-<date>.md` documenting:

- Diff summary (files changed: `docs/phases/PHASE-2.md`, `Plans/INDEX.md`; lines touched).
- Placeholder-fill audit (0 remaining placeholders).
- Format-fix application confirmation (Fourteen/meta-decision framing; Spectre-v2 BTB citation).
- Cross-reference integrity spot-check.
- Commit SHA after push.

Format matches prior Prometheus verification reports.

## 9. Stop conditions

- Pre-execution gate (§2) fails — Jarvis SC-3C not closed → STOP, surface to PL.
- Any placeholder remains unresolved in the diff → STOP, fix, retry verification.
- Jarvis verification reports surface a course correction requiring a new ADR row → STOP, surface to PM (governance act).
- Push rejected → STOP, surface to PL.
- The SC-3C closure date / commit SHAs don't match what's actually on `origin/main` → STOP. Integration must reflect actual state.

## 10. Closure protocol

After commit + push + verification report:

1. Prometheus reports back to PL with: (a) commit SHA, (b) verification report filename, (c) summary of placeholder fill-ins (count + categories), (d) any incidental observations.
2. PL ferries to PM.
3. PM consumes; closure complete. SC-3C documentation now reflects actual closed state in PHASE-2.md + INDEX.md.

---

**End of dispatch.**

*Post-Jarvis-close integration of pre-staged SC-3C closure documentation into PHASE-2.md + INDEX.md. Two PM-noted format-precision fixes applied. Single commit. Gated on Jarvis SC-3C BUILD complete with three verification reports + closure report filed.*
