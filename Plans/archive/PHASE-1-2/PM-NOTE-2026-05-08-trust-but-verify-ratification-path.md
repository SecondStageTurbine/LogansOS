# PM-NOTE: "Trust but Verify" — A2-direct + A1 + Bible amendment 8.5 ratification path

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures project lead's "Trust but Verify" decision in this conversation: A2-direct (scheduler decoupling from global heap) + A1 (IRQ-safe heap wrapper as defense-in-depth) + co-ratified Bible amendment 8.5 (encoded critical-section discipline). Family L is skipped entirely as throwaway work given A2 supersedes the data structures L would have patched. Sets the ratification path, the implementation plan, and the new disposition of the wedge-fix-before-SC-3B commitment.
**Authority:** PM. Records the ratification decision and the resulting forward path. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` remains active and will discharge on ADR-016's Phase 6 verification (200 cargo boots, all four historical signatures absent), not on the (skipped) L verification.

---

## 1. The decision

Project lead's response to the ratification prompt: **"Trust but Verify. - Skip L"**.

Translated to the three-option framing in the prior PM message:

- **"Trust"**: Bible amendment 8.5 — encoded critical-section discipline. Trust that future contributors will follow the rule because the rule is written down.
- **"Verify"**: Family A1 — IRQ-safe heap allocator wrapper. Structural enforcement of the rule; even if a future contributor violates the discipline, the wrapper prevents same-CPU deadlock.
- **A2 (already approved earlier in conversation)**: scheduler decoupling from global heap. Architectural elimination of the proven bug's enabling pattern.
- **Skip L**: confirmed. L's data-structure-specific patches become moot once A2 lands; per Bible Article 1.3 ("no toy code, no throwaway shortcuts"), L is not done at all.

The result is the strongest available posture: three coordinated layers (architectural decoupling, structural enforcement, encoded discipline), all ratified together.

## 2. Artifacts on disk

This conversation produced the following new or updated artifacts in `Plans/`:

**New, awaiting ratification:**

- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` — the ADR covering A2 (Decision 1), A1 (Decision 2), and the Bible amendment co-ratification (Decision 3). Replaces the same-day earlier draft. Substrate of project lead's ratification call.
- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` — the Bible amendment proposing new Article 8.5 (Critical-section discipline, sub-articles 8.5.1 through 8.5.5). Co-ratified with ADR-016.

**New, marked superseded:**

- `Plans/DISPATCH-2026-05-08-wedge-fix-L-cerebro.md` — the L fix dispatch, with supersession header at top. Original body preserved verbatim. Do not forward to Cerebro.
- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md` — the earlier-same-day A1-only ADR draft, with supersession header. Body preserved (sections beyond Context are summarized rather than full-text duplicated since the new draft covers the same ground).

**This note:**

- `Plans/PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` — captures this decision and the forward path.

**No commits yet. No backups since `Z:\AltimitNeo\2026-05-08-212442\` (HEAD `3b3b4db`).** All current artifacts are working-tree-only. Backup will run after the first ratification commit lands (the Bible amendment).

## 3. Ratification path

Project lead's gate. The next two ratification actions are sequential:

### 3.1 Ratify Bible amendment 8.5

Project lead reviews `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md`.

Possible outcomes:
- **Ratify as-is.** PM drafts the bible-update commit (single commit on `main`: appends Article 8.5 to `docs/bible.md`, updates footer with version 1.0.5 and last-amended date 2026-05-08). Cerebro lands the commit (PM doesn't touch tracked bible content per project-lead-only governance). Backup runs.
- **Modify.** PM revises per project lead's edits, re-presents.
- **Reject.** PM-NOTE records rejection rationale; ADR-016's "Decision 3" is null; ADR-016 still ratifies Decisions 1 and 2 if PL chooses, but without the encoded-discipline layer the structural enforcement (A1) is the sole "verify" mechanism.

### 3.2 Ratify ADR-016

Project lead reviews `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md`.

Possible outcomes:
- **Ratify as-is.** PM finalizes as ADR-016-FINAL-...md (preserving DRAFT for history). Implementation work proceeds per §4 below.
- **Modify.** PM revises per project lead's edits, re-presents.
- **Reject (any subset).** Possible to ratify Decision 1 (A2) without Decision 2 (A1), or Decision 2 without Decision 3 (Bible amendment), etc. Each subset has different consequences documented in the ADR's "Alternatives" section.

The two ratifications are intentionally independent. The Bible amendment can ratify even if ADR-016 doesn't (the discipline stands as project policy, the implementation work is just deferred). The ADR can ratify even if the Bible amendment doesn't (the implementation work proceeds, the discipline is implicit rather than encoded).

The expected outcome given project lead's "Trust but Verify" framing: both ratify together, possibly with edits.

## 4. Implementation plan (post-ratification)

Per ADR-016's "Implementation plan" section, summarized here:

| Phase | Action | Owner | Estimated effort |
|---|---|---|---:|
| 2 | PL ratifies Bible amendment + ADR-016 | PL | review time |
| 2.5 | Bible amendment lands as commit | Cerebro (PM drafts commit text) | minutes |
| 3 | A1 implementation dispatch | PM drafts → Cerebro implements | ~1-2 hours |
| 4 | A2 design dispatch | PM drafts → Cerebro proposes design | ~1-3 days |
| 4.5 | PL ratifies A2 design choice | PL | review time |
| 5 | A2 implementation dispatch | PM drafts → Cerebro implements | ~1-2 weeks |
| 6 | Verification | Cerebro runs 200+ cargo boots covering all four signatures | ~2-3 hours runtime |
| 7 | Close report | PM authors | ~1 hour |

Total elapsed: ~2-4 weeks from this turn to wedge-fix-before-SC-3B discharge. SC-3B work does not begin until Phase 7 closes successfully.

The wedge stays open during this period. Per project lead's standing directive in `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md` ("we cannot ship it with errors"), this is acceptable — we are not shipping during this window.

## 5. Disposition of related artifacts

The following project artifacts have updated dispositions in light of this decision:

- `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` — the L+A framing from this morning. Now superseded by the broader A2+A1+Bible-amendment posture. Rather than rewrite, this PM-NOTE supersedes its §3-§4 forward-looking sections; the §1-§2 mechanism description remains canonical.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` and `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — both directly cited in the new ADR's rationale. The "trust but verify" framing in the second research note's §0 is operationally what got ratified.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — the lock-free `zombie_count == 0` short-circuit composition-safety analysis. Survives the ratification: A2 may eliminate or refactor the short-circuit (depending on Cerebro's design choice), but the analysis itself remains correct as a record of how the existing code reasoned about composition.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` and `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` — historical findings from prior investigation phases. Unchanged.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment. Discharge condition refined: discharges on ADR-016's Phase 6 verification (200 cargo boots, all four historical signatures absent), not on L's verification (skipped) and not on A1 alone (insufficient — needs A2 to address the proven mechanism).

## 6. INDEX.md update queued

The Plans/INDEX.md is now meaningfully behind. New artifacts to register in the next commit cycle (likely the Bible amendment commit, since that's the next planned commit):

- `DISPATCH-2026-05-08-wedge-investigation-7-cargo-stub-cerebro.md` (closed at `6edb012`)
- `WEDGE-INV-7-FINDINGS-2026-05-08.md`
- `DISPATCH-2026-05-08-wedge-investigation-8-nmi-watchdog-cerebro.md` (closed at `3b3b4db`)
- `WEDGE-INV-8-FINDINGS-2026-05-08.md`
- `PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md`
- `PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md`
- `RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md`
- `RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`
- `DISPATCH-2026-05-08-wedge-investigation-J1-audit-cerebro.md` (closed at static-audit completion)
- `WEDGE-INV-J1-AUDIT-2026-05-08.md`
- `PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md`
- `DISPATCH-2026-05-08-wedge-fix-L-cerebro.md` (SUPERSEDED — note in INDEX)
- `ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md` (SUPERSEDED — note in INDEX)
- `ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md`
- `BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md`
- `PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` (this note)

Plus `Plans/INDEX.md` itself for the §"Open Project Commitments" section refresh: the wedge-fix-before-SC-3B commitment's discharge condition has refined to ADR-016 Phase 6.

## 7. What does NOT change

- The Bible's existing 1-23 articles (until amendment 8.5 lands).
- ADR-015 (KASLR-tolerant heap placement). ADR-016 builds atop it; doesn't supersede.
- Phase 1.4's overall scope or the Phase ledger.
- The wedge-fix-before-SC-3B commitment itself (still active; discharge condition refined).
- Any other project-state artifact not enumerated in §5.

## 8. Stash topology

Unchanged from `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` §7. Three stashes preserved:

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

All preserved in the latest backup (`Z:\AltimitNeo\2026-05-08-212442\`). No backup needed yet from this turn — no commits since `3b3b4db`.

## 9. Cross-references

- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` — the ADR being ratified.
- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` — the Bible amendment being co-ratified.
- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — the bug analysis the ratification responds to.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` — the prior project-state note (superseded forward-looking sections).
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — the prior art across Rust no_std kernels.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment.
- `Plans/ALTIMIT-real-world-implementation.md` — the ALTIMIT posture cited in the project-lead-test conversation that elevated A2.

---

**End of note.**

*Three coordinated changes ratify together. A2 architecturally decouples scheduler from heap. A1 makes the heap IRQ-safe as defense-in-depth. Bible amendment 8.5 encodes the discipline as project policy. Trust the rule (Bible). Verify with mechanism (A1). Eliminate the underlying coupling (A2). PL gates the next move; PM stands by to draft the Bible-update commit and ADR-016-FINAL on signal.*
