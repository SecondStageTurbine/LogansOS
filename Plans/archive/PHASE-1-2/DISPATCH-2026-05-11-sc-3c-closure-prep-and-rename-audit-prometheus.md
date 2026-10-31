# DISPATCH — SC-3C closure prep + AltimitNeo→NeoPlutarch rename audit (Prometheus)

**Date filed:** 2026-05-11
**From:** PM
**To:** Prometheus (Qwen3.6-27B, docs + cleanup + small-coding-trial lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.2)
**Ferry:** PL ferries to Prometheus session.
**Status:** Parallel-with-Jarvis dispatch. Jarvis is currently on SC-3C BUILD-stage (`Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md`, three sub-commits 4a/4b/4c). This dispatch produces doc-only deliverables that do not touch any kernel source path Jarvis is working in. Two deliverables: (A) SC-3C closure prep — pre-stage PHASE-2.md and INDEX.md additions for PM consumption after Jarvis closes; (B) AltimitNeo→NeoPlutarch rename audit — produce a per-file/per-reference disposition table without executing any renames.

---

## 1. Context

**Prometheus lane recap.** Per agent-roster §2.2, Prometheus's responsibilities are: docs, amendments, document cleanup, and trial small-coding work. Prior successful Prometheus dispatches:

- ADR ratification six-commit dispatch (Prometheus translated three `ADR-PROPOSAL-*` files into ratified ADRs in `docs/`, plus auxiliary ledger updates).
- Two README updates landed parallel to the Jarvis kernel-rename bundle (`c35386c Update README with AI-native OS definition` and `d60f416 Eliminate duplicate content in README`).

Both demonstrated mechanical doc work + multi-commit cadence + understanding of project conventions. This dispatch is two doc-only deliverables in the same lane.

**Parallel-with-Jarvis constraint.** Jarvis is touching `kernel/src/arch/x86_64/*`, `kernel/src/sched/*`, `kernel/src/mm/*`, `kernel/src/sync/*`, `kernel/src/test_harness.rs`, and may create `kernel/src/mm/tlb_shootdown.rs`. **Prometheus must NOT touch any path under `kernel/src/`.** Prometheus's working surface is `docs/`, `Plans/`, project-root files (`README.md`, `CLAUDE.md`, `HANDOFF.md` if applicable).

**Substrate baseline at dispatch time:** `origin/main` HEAD = `27d1b99` (kernel-rename bundle fully closed). The two README commits referenced above are already on `origin/main`. Phase-2 ADR ledger in `docs/phases/PHASE-2.md` ends at ADR-020 (ratified 2026-05-10). 20 ADRs ratified total (`docs/ADR-001-*.md` through `docs/ADR-020-*.md`).

---

## 2. Deliverable A — SC-3C closure prep

### A.1 Scope

Pre-stage the documentation updates that will need to land **after** Jarvis closes SC-3C (all three sub-commits landed on `origin/main`). PM consumes the pre-staged drafts; a follow-up dispatch (likely Prometheus again) integrates them into the live `docs/phases/PHASE-2.md` and `Plans/INDEX.md` files after Jarvis's final SC-3C commit lands.

**This deliverable produces DRAFT artifacts**, not commits to tracked files. The drafts live in `Plans/` (untracked per hybrid posture). Format/content review is by PM; integration is a separate dispatch after SC-3C closes.

Why pre-stage rather than wait? Because the SC-3C closure-marker structure is mechanical (parallel to the SC-3B closure marker that already lives in PHASE-2.md), and pre-staging lets PM review the format before the time-pressured integration moment. Closure prep landing speed-of-light after Jarvis closes is the project pattern; pre-staging means the speed-of-light path is "PM reviewed-once draft → integrate" rather than "PM drafts from scratch under time pressure."

### A.2 Files to read for format anchoring

1. **`docs/phases/PHASE-2.md`** (25 KB) — read in full. The SC-3B section (titled "## BUILD-3 sub-commit 3 (SC-3B) — Q4/Q6 IPC Retrofit") is the canonical voice anchor for the SC-3C closure section. Same template: "Closed YYYY-MM-DD via commit `<sha>`. Verification log: ... Closure report: ... Gate scores: ... Mechanism named: ..." followed by Follow-up ADR proposals, What landed, Course corrections, Verification Log subsections.

2. **`docs/phases/PHASE-2.md` ADR Ledger section** (near end of file, table format) — voice anchor for any new ADR ledger entries. SC-3C is not expected to land new ADRs (the work is gated by ADR-019/020 already-ratified machinery), so the ADR ledger likely needs no SC-3C-specific rows; verify during draft.

3. **`Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`** (if it exists) — voice anchor for the closure-report style that the PHASE-2.md SC-3C section will reference. SC-3C's own closure report will be filed by Jarvis or PM at close-time; Prometheus's deliverable A does NOT include the closure report itself, only the PHASE-2.md and INDEX.md additions.

4. **`Plans/INDEX.md`** (40.4 KB) — read in full. This is the Plans-directory catalog. Voice anchor for the catalog entries to add for all SC-3C-related artifacts.

5. **All SC-3C-related artifacts already in `Plans/`** that will need INDEX.md entries:
   - `Plans/SC-3C-THINK-2026-05-10.md`
   - `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (FINALIZED 2026-05-11)
   - `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md`
   - `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md`
   - `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`
   - `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`
   - `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md`
   - (When Jarvis closes: three verification reports `SC-3C-SUB-COMMIT-4{A,B,C}-*-VERIFICATION-2026-05-MM.md`; a closure report; possibly a closure PM-NOTE. These will be added by the post-Jarvis integration dispatch, not pre-staged here.)

### A.3 Output for deliverable A

Single artifact:

**`Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md`**

Structure:

1. **Header** — date, author (Prometheus), type (closure-prep draft, doc-only, awaits Jarvis SC-3C close before integration), status (DRAFT pending PM review).
2. **§ A — Draft PHASE-2.md SC-3C section** — a markdown block ready to insert into `docs/phases/PHASE-2.md` immediately after the existing SC-3B section. Format matches the SC-3B section template. Placeholder fields explicitly marked `<PLACEHOLDER: ...>` for content Prometheus cannot know until Jarvis closes (commit SHAs, verification log filenames, gate scores, "Mechanism named" — if any).
3. **§ B — Draft ADR ledger updates** — if applicable. SC-3C is not expected to add ADRs; if Prometheus's read of the SC-3C THINK + D-revisions artifacts agrees, write a one-line "No new ADRs land with SC-3C; ledger unchanged" note. If Prometheus finds reason to draft ledger entries (e.g., the strictly-vs-loosely-accurate T2 cpumask design choice from Cerebro D15 §3.1 might warrant a Phase-2-forward-marker ADR row), surface that finding for PM review rather than committing to a row.
4. **§ C — Draft Phase 2+ forward markers** — three new forward markers from `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` §§ 3.1, 4, 3.5:
   - Strictly-accurate vs loosely-accurate per-mm cpumask design choice for T2 migration.
   - Lazy-TLB page-table-teardown exception (Linux `freed_tables` flag pattern).
   - Linux global-ASID promotion mechanism (sampling-gate-plus-rising-floor, not deterministic 3-CPU threshold) for INVLPGB/RAR forward work.
   
   Plus pre-existing SC-3C-relevant forward markers from `Plans/SC-3C-THINK-2026-05-10.md` §8:
   - PCID enablement (deferred per D2; future ADR-driven sub-commit).
   - Lazy TLB (deferred per D6).
   - Shootdown batching (deferred per D7).
   - T2/T3 targeted shootdown (deferred per D4; Phase 2+ forward optimization).
   
   Format: markdown bullet list ready to insert into the PHASE-2.md "Forward reference" or equivalent section. Cite source PM-NOTE / THINK artifact for each marker.
5. **§ D — Draft INDEX.md catalog entries** — one entry per SC-3C-related artifact listed in A.2 #5 above. Format matches the existing INDEX.md entry format (Prometheus reads INDEX.md to extract the format). Each entry includes filename, brief description (1-2 lines), date, and cross-references to related artifacts.
6. **§ E — Integration plan** — short section describing what the post-Jarvis integration dispatch will need to do: insert §A into PHASE-2.md at the correct position, fill in placeholders from Jarvis's verification reports and commit SHAs, apply §C forward markers to the appropriate PHASE-2.md section, apply §D entries to INDEX.md, commit and push. Estimated integration scope: 1-2 small edits + 1 commit.

### A.4 Specific content requirements

**For § A (PHASE-2.md SC-3C section draft):**

- Section heading: `## BUILD-3 sub-commit 4 (SC-3C) — Q7.b CR3 cache + Q7.c IPI TLB shootdowns`
- Opening template (placeholders bracketed):
  ```
  **Closed <PLACEHOLDER: YYYY-MM-DD> via commits `<PLACEHOLDER: 4a-sha>`, `<PLACEHOLDER: 4b-sha>`, `<PLACEHOLDER: 4c-sha>`.** Three sub-commits per SC-3C D12 ratification: 4a (Q7.b CR3 cache), 4b (Q7.c TLB shootdown machinery), 4c (Q7.c call-site wiring). Verification logs: `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-<PLACEHOLDER: date>.md`, `Plans/SC-3C-SUB-COMMIT-4B-Q7C-MACHINERY-VERIFICATION-<PLACEHOLDER: date>.md`, `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-<PLACEHOLDER: date>.md`. Closure report: `<PLACEHOLDER: filename>`. Gate scores: <PLACEHOLDER>. Mechanism named: <PLACEHOLDER or "N/A — no new wedge mechanism">.
  ```
- "What landed" subsection: three bullets matching the sub-commit scopes (Q7.b CR3 compare-and-skip; Q7.c shootdown machinery; Q7.c call-site wiring).
- "D-decisions ratified" subsection: brief reference to the D1-D14 finalization, citing `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` and `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`.
- "Verification Log" subsection: placeholders for `cargo k`, `cargo neo-test all`, `200/200 NEO_SMP=4`, and the new D14 missing-shootdown-detection scenario.
- "Course corrections" subsection: leave a one-line "No course corrections" placeholder; if Jarvis hits any during BUILD (e.g., environment-anchored threshold revisions parallel to the ADR-019 bench lesson), the integration dispatch fills this in.

**For § C (forward markers):**

Each marker should be ~2-4 lines: identifier (PCID / Lazy TLB / Batching / T2-T3 / Strictly-vs-loose-cpumask / Lazy-TLB-freed-tables-exception / INVLPGB-RAR-promotion), 1-sentence description, citation pointing to source PM-NOTE or THINK section, no-action-required-now framing. Prometheus may use the format from the Tweede Golf "Debloat your async Rust" forward reference already in PHASE-2.md as the voice anchor.

**For § D (INDEX.md entries):**

Prometheus reads INDEX.md to extract its entry format. Each new entry must:
- Cite filename exactly as it exists in `Plans/`.
- 1-2 line description in the same style as existing entries.
- Date.
- Cross-references to related artifacts where they exist (e.g., the Cerebro D15 research artifact's INDEX.md entry should cross-reference the dispatch and the consumption note).

### A.5 Stop conditions for deliverable A

- If `docs/phases/PHASE-2.md` SC-3B section format has drifted in a way that makes the SC-3C section template ambiguous, surface to PM rather than guessing.
- If `Plans/INDEX.md` format has drifted (e.g., existing entries don't match a single template), surface and request format clarification.
- If reading the SC-3C THINK + D-revisions + Cerebro consumption artifacts surfaces a forward marker Prometheus believes deserves an ADR row but PM hasn't explicitly authorized, surface it as a "candidate for PM consideration" finding rather than drafting the row directly. ADR ledger entries are governance acts.

---

## 3. Deliverable B — AltimitNeo→NeoPlutarch rename audit

### B.1 Scope

The kernel-rename bundle (commits `053a5bc` + `de8965c` + `27d1b99`) renamed AltimitNeo → NeoPlutarch in the kernel source tree and a subset of docs (README updates landed in parallel at `c35386c` + `d60f416`). PM's substrate scan finds **10 tracked files** with surviving `AltimitNeo`/`altimitneo`/`altimit-neo`/`ALTIMITNEO`/`ALTIMIT_NEO` references at 57 total occurrences. Some of those references are stragglers from the rename (should rename); some are intentional historical (research artifacts citing the fictional ALTIMIT OS, verification reports referring to the rename event itself, ADR-002 documenting historical source layout); some require PL-tier decision (the Bible itself).

**Produce a per-file/per-reference disposition audit. Do NOT execute any renames.** The audit is input for a follow-up dispatch that executes approved mechanical renames after PM review and PL ratification of PL-tier items.

### B.2 File list (from PM substrate scan)

| File | Refs | First-pass classification (verify) |
|---|---|---|
| `docs/research/ALTIMIT-real-world-implementation.md` | 36 | Mostly INTENTIONAL HISTORICAL — this artifact is *about* the .hack fictional ALTIMIT OS as a research reference for NeoPlutarch's design. Most refs are to the fictional system, not to the project. **Some** refs may be to "AltimitNeo" the project name; those should rename. Per-reference audit required. |
| `CLAUDE.md` | 6 | Likely RENAME (project-naming references in the project guide for Claude sessions); verify. |
| `docs/bible.md` | 4 | **PL-TIER DECISION.** The Bible itself. Filename + content references AltimitNeo throughout. Constitutional document. Audit lists the references with proposed dispositions but does NOT recommend execution — this is for PL governance. Note: the Bible's title may be intentional even if renamed (e.g., "TheAltimitNeoBible.md" as the project's foundational charter may be kept under the original filename as historical record, or the Bible may be renamed and the old filename redirected). PL decides. |
| `README.md` | 3 | Likely INTENTIONAL HISTORICAL or PL-TIER — already had two cleanup commits; remaining refs may be deliberate or stragglers. Verify per-reference. |
| `docs/ADR-002-source-layout-migration.md` | 2 | INTENTIONAL HISTORICAL — ADR-002 documents the source-layout migration that happened under the AltimitNeo name. Historical refs stay. |
| `docs/HANDOFF.md` | 2 | Likely RENAME (operational doc); verify. |
| `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` | 1 | INTENTIONAL HISTORICAL — verification report for THE rename. Refs to the rename event itself. Stays. |
| `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` | 1 | Verify. May be incidental reference in a research note. |
| `Plans/INDEX.md` | 1 | Likely RENAME if it's a project-name reference; INTENTIONAL HISTORICAL if it's referring to the rename event or a pre-rename artifact. Verify. |
| `docs/design.md` | 1 | Verify. May be project-name reference (RENAME) or historical (HISTORICAL). |

### B.3 Disposition framework

For each reference, classify as one of:

- **RENAME-MECHANICAL** — straightforward project-name reference that should become "NeoPlutarch". Mechanical replacement; no semantic shift.
- **RENAME-CONTEXTUAL** — reference where the surrounding context needs adjustment beyond the name (e.g., "AltimitNeo kernel" → "NeoPlutarch kernel" but the sentence's meaning shifts and needs review).
- **INTENTIONAL-HISTORICAL** — reference to past project state, the rename event itself, the fictional ALTIMIT OS, etc. Stays as-is.
- **PL-TIER-DECISION** — reference where renaming is a constitutional / governance act (Bible content, Bible filename, possibly project tagline material). Audit flags but does NOT recommend disposition.
- **AMBIGUOUS** — Prometheus cannot determine the disposition from context. Surface for PM review.

### B.4 Output for deliverable B

Single artifact:

**`Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md`**

Structure:

1. **Header** — date, author (Prometheus), type (audit artifact, doc-only, no renames executed), status (DRAFT pending PM review of dispositions and PL ratification of PL-tier items).
2. **Summary** — total file count, total reference count, breakdown by disposition class.
3. **Per-file sections** — one section per tracked file with AltimitNeo references. Each section contains:
   - Filename.
   - Total reference count in this file.
   - Per-reference disposition table: line number / context snippet (1-2 lines around the reference) / classification / proposed replacement (for RENAME-* classes) / notes.
4. **PL-tier items** — separated section listing every PL-TIER-DECISION reference with brief framing of the governance question (e.g., "Bible content references AltimitNeo; rename to NeoPlutarch implies amending Article N text; per Bible 27.1, amendments require PR with rationale and version bump").
5. **Recommended follow-up dispatches** — short section listing the dispatches that would execute the audit's recommendations:
   - One dispatch for RENAME-MECHANICAL items (Prometheus executes; PM reviews; commit and push as a single rename-stragglers commit).
   - One dispatch for RENAME-CONTEXTUAL items (Prometheus drafts; PM reviews; possibly PL ratifies if context shifts are non-trivial; commit and push).
   - One dispatch for PL-TIER items, deferred until PL ratifies the relevant governance questions (Bible amendment, etc.).
6. **Out-of-scope notes** — any references Prometheus encounters that are clearly out of scope (e.g., commit messages in `git log`, ELF binary strings in built artifacts) and explicitly NOT audited.

### B.5 Bible carve-out

`docs/bible.md` references are flagged as PL-TIER-DECISION uniformly. Prometheus's audit includes them in the per-file section with disposition `PL-TIER-DECISION` for each reference and a brief description of what the reference is, but **does NOT propose replacement text** for Bible references. The Bible is a constitutional document; amending its text is a PL governance act per Bible Article 27.1, which requires PR with rationale and version bump. The audit surfaces "this is what's there; PL decides whether and how to amend." Same treatment for the Bible filename question (is it `TheAltimitNeoBible.md`, `bible.md`, `NeoPlutarchBible.md`, or something else?) — that's a PL filename-decision, not Prometheus's call.

### B.6 Stop conditions for deliverable B

- If Prometheus encounters >5 AMBIGUOUS references in a single file, pause and surface to PM. The ambiguity may indicate the disposition framework needs refinement before the audit can complete cleanly.
- If a file has references that suggest a third disposition category not anticipated in §B.3, propose the new category as an addition to the framework and surface to PM for ratification before applying.
- If Prometheus discovers tracked files with AltimitNeo references that PM's scan missed (e.g., binary build artifacts that grep didn't catch, or files where the reference is in non-text content like an image), surface them and request scope clarification.

---

## 4. No-conflict-with-Jarvis discipline

Critical constraint:

- **Do NOT touch `kernel/src/*`.** Jarvis owns this surface for SC-3C BUILD.
- **Do NOT touch `tools/xtask/src/*`.** Jarvis may modify if SC-3C requires test-harness changes.
- **Do NOT touch `Cargo.toml` or `Cargo.lock` at any level.** Jarvis may modify if SC-3C adds dependencies or features.
- **Permitted surfaces:** `docs/`, `Plans/` (untracked or doc-only commits), project-root `README.md` / `CLAUDE.md` / `HANDOFF.md` (but ONLY for the audit reading; do NOT modify these in this dispatch — disposition is audit-only).

If Prometheus needs to read `kernel/src/*` files for context (e.g., to verify a forward-marker description), reading is fine; modification is not.

Branching: this dispatch produces only `Plans/` artifacts (untracked per hybrid posture). **No branch + push cycle required.** If Prometheus's deliverables grow to include any tracked-file modifications (they shouldn't), pause and surface to PM before committing — that may indicate scope creep.

---

## 5. Verification expectations

This is doc-only work; no `cargo k` gates apply. The verification model is:

1. **Format conformance** — drafts match the voice anchors specified in §A.2 and §B.4.
2. **Cross-reference integrity** — every artifact cited in the drafts exists at the cited path. Run a self-check: for each cross-reference in the deliverable A draft and the deliverable B audit, verify the cited file exists.
3. **Disposition reasoning quality** — each disposition in deliverable B has a brief justification (1-2 sentences) that PM can evaluate. Reasoning-free dispositions ("RENAME-MECHANICAL because looks like a rename") are insufficient; "RENAME-MECHANICAL because the reference is in a 'project artifact path' context that uniquely identifies the project name, not the fictional ALTIMIT system" is sufficient.
4. **PM review gate** — both artifacts (closure prep draft + audit) are reviewed by PM before any integration / follow-up dispatch lands. PM ratification of dispositions, format approval, scope ratification all happen at PM review.

---

## 6. Stop conditions (general)

- Any deliverable's output fails the cross-reference integrity self-check → STOP, fix cross-references, re-run check, file note about what was broken in case it reveals a systemic issue.
- Either deliverable surfaces enough ambiguity to question the dispatch's scope framing → STOP, file findings, surface to PM for scope clarification.
- Time-budget actual exceeds 2x estimate (see §8) → pause and surface to PM. Calibration dispatches are not the place to push through significant overruns.
- Jarvis pushes a commit to `origin/main` mid-dispatch → no impact (Prometheus is not committing anywhere; reading new state is fine; deliverables A and B's substrate baselines do not shift since Jarvis is on `kernel/src/*`).

---

## 7. Calibration framing

Prometheus has already demonstrated competence on the project:

- ADR ratification six-commit dispatch landed clean.
- Two README updates landed clean on `origin/main` parallel to Jarvis.

This dispatch is therefore not a "first calibration" event; it's an "additional dispatch in established lane." Calibration interest here is on:

1. **Multi-deliverable bundle handling.** Both deliverables A and B are in scope simultaneously. Prometheus's prior dispatches were single-purpose. PM observes how Prometheus structures parallel-deliverable work — interleave, complete one then the other, or some other pattern.
2. **Dispositional reasoning.** Deliverable B's audit requires per-reference classification with justification. Prometheus's reasoning quality on RENAME vs INTENTIONAL HISTORICAL vs PL-TIER is calibration data on the agent's contextual judgment.
3. **Format-anchoring discipline.** Both deliverables specify voice anchors (PHASE-2.md SC-3B section, INDEX.md existing entries). Prometheus's adherence to anchored format is calibration data on the agent's docs-conformance discipline.

No "calibration N of M" framing — Prometheus is past the first-dispatch phase.

---

## 8. Time budget

PM estimate:

- Deliverable A: **1-2 hours** (read voice anchors; draft sections with placeholders; cross-reference verification).
- Deliverable B: **2-3 hours** (read 10 files; per-reference audit with reasoning; assemble disposition table; flag PL-tier items).
- **Total: 3-5 hours wall-clock.**

Generous bound. If the audit surfaces enough complexity to push deliverable B past 4 hours alone, pause and surface — that may indicate the disposition framework needs refinement before completion.

---

## 9. Closure protocol

After both deliverables filed:

1. Prometheus reports back to PL with: (a) two artifact filenames, (b) summary statistics from each (deliverable A: number of forward markers drafted, number of INDEX.md entries; deliverable B: total references, per-disposition counts), (c) any incidental observations, (d) time-budget actual.
2. PL ferries the report back to PM.
3. PM consumes both artifacts. Deliverable A is reviewed for format/content; PM either approves or surfaces revisions. Deliverable B's dispositions are reviewed; PM ratifies the RENAME-* classifications, surfaces any disagreements, escalates PL-TIER items to PL for governance ratification.
4. After PM ratifies (and where needed, PL approves PL-tier items), follow-up dispatches execute:
   - Post-Jarvis: integration of deliverable A into PHASE-2.md and INDEX.md (likely Prometheus again, since the work is identical-pattern to what was drafted).
   - Post-PM-ratification: mechanical rename execution per deliverable B's RENAME-MECHANICAL list.
   - Post-PL-ratification: Bible amendment work (separate PL-tier dispatch).

---

## 10. Cross-references

**Lane reference:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.2 (Prometheus lane definition).

**Concurrent work (no conflict):**
- `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` — Jarvis SC-3C BUILD dispatch. Touches `kernel/src/*`; Prometheus does not.

**Substrate anchors:**
- `docs/phases/PHASE-2.md` — voice anchor for SC-3C closure section format.
- `Plans/INDEX.md` — voice anchor for catalog entry format.
- `docs/ADR-001-*.md` through `docs/ADR-020-*.md` — full ADR ledger (20 ratified) for cross-reference checks.

**Inputs for deliverable A:**
- `Plans/SC-3C-THINK-2026-05-10.md` — design intent + D-table + Phase 2+ forward markers.
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (FINALIZED 2026-05-11).
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — canonical prior art.
- `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` — three new forward markers from §§ 3.1, 4, 3.5.
- `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` — sub-commit structure and verification gate references.

**Inputs for deliverable B:**
- 10 tracked files with AltimitNeo references (enumerated in §B.2).
- `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` — context for the rename event itself (intentional historical references).
- `docs/bible.md` Article 27 — PL-tier amendment process governance.

**Voice anchors specified throughout:**
- PHASE-2.md SC-3B section (closure marker template).
- INDEX.md existing entries (catalog format).
- Prior Prometheus dispatches (multi-commit cadence, conformance discipline).

---

**End of dispatch.**

*Two doc-only deliverables parallel to Jarvis SC-3C BUILD. (A) SC-3C closure prep: pre-stage PHASE-2.md + INDEX.md additions in a Plans/ draft for post-Jarvis integration. (B) AltimitNeo→NeoPlutarch rename audit: per-file/per-reference disposition table with RENAME-MECHANICAL / RENAME-CONTEXTUAL / INTENTIONAL-HISTORICAL / PL-TIER-DECISION / AMBIGUOUS classifications. No renames executed in this dispatch. No `kernel/src/*` touches. Estimated 3-5 hours wall-clock. PM reviews and ratifies; follow-up dispatches execute approved changes.*
