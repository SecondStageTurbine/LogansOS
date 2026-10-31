# DISPATCH-2026-05-21: ADR-023 commit + Bible v1.7.0 amendments

| | |
|---|---|
| **From** | Cerebro (PM) |
| **To** | Prometheus |
| **Date** | 2026-05-21 |
| **Type** | Documentation ratification — atomic single-commit |
| **Scope** | 3 files edited, 1 new file created, 1 commit, no code changes |
| **Authority** | `Plans/PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md` (PL signoff 2026-05-21, Bible Article 27) |
| **Estimated effort** | Mechanical text-edit work; the substantive judgment was done in the DRAFT/ratification step. Reviewer expected to be careful, not fast. |

---

## Task summary

PL has ratified ADR-023 (Linux binary compatibility via personality layer) and the co-ratified Bible amendment bumping v1.6.0 → v1.7.0 (Article 3 fourth bullet edited, new Article 5.6 added, Article 28.2 cross-reference updated). Apply the ratified text to in-tree artifacts as a single atomic commit. No interpretation, no expansion — verbatim application of the ratified text plus mechanical status flips and INDEX bookkeeping.

This dispatch supersedes the rescinded `Plans/DISPATCH-2026-05-21-bible-1.1.0-ratification-prometheus.md` (kept in tree as historical record per the 2026-05-20 precedent).

---

## Files to edit / create

1. **CREATE** `docs/ADR-023-linux-personality-layer.md` — copy from `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` with one targeted Status-row edit (see Edit 1 below).
2. **EDIT** `docs/bible.md` — apply three article amendments + footer bump (see Edits 2a–2d below).
3. **EDIT** `Plans/INDEX.md` — record DRAFT artifacts as closed; add this disposition + dispatch (see Edit 3 below).

Source DRAFTs to read from:

- `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` (ADR source)
- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md` (Bible amendment source)
- `Plans/PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md` (ratification authority)

---

## Edit 1 — Create `docs/ADR-023-linux-personality-layer.md`

Copy the entire body of `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` to a new file at `docs/ADR-023-linux-personality-layer.md`, with **one** edit to the Status row in the header table.

**Find** in the header table:

```
| **Status** | DRAFT — awaiting PL ratification |
```

**Replace with:**

```
| **Status** | Accepted (ratified 2026-05-21 per Bible Article 27; Bible v1.7.0 co-ratified) |
```

No other content changes. The §"Document end." closing note about PM/PL recommendation history (PM recommended Option A; PL chose Option B) is preserved as institutional record — do not delete it.

The DRAFT in `Plans/` is **not deleted**. It remains in tree as historical record per the established pattern (compare `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` alongside `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`).

---

## Edit 2 — Apply Bible amendments to `docs/bible.md`

Four sub-edits in order. Each has a verbatim find-and-replace from the ratified amendment text in `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md`.

### Edit 2a — Article 3 fourth bullet

**Find** (in Part I, Article 3 — What We Are Not — the bulleted list, the sixth and final bullet):

```
A POSIX-compatible system. We may offer a POSIX compatibility shim for porting userland tools, but the native ABI is our own.
```

**Replace with:**

```
A POSIX-compatible system natively. The native ABI is our own. Foreign-OS compatibility, including POSIX/Linux, is provided through opt-in personality subsystems per Article 5.6 — not by adopting POSIX as the native shape, and not by extending the native ABI to mimic POSIX semantics piecemeal.
```

Preserve the bullet formatting (the bible's article 3 uses unprefixed list items separated by paragraph breaks — match what's there). Do not introduce new emphasis or change the bullet's position in the list.

### Edit 2b — Insert new Article 5.6

**Locate** the end of existing Article 5.5 — the paragraph ending exactly with:

```
Fork is a 1970s artifact that fights every memory-management technique invented since.
```

**Insert immediately after that paragraph and immediately before the `Article 6 — The AI/ML Subsystem` heading:**

```
5.6 NeoPlutarch natively supports its own ABI. Foreign-OS binary compatibility, when provided, is delivered through opt-in personality subsystems that translate foreign syscalls to native kernel operations at the syscall boundary. Personality layers are kernel-resident, opt-in per process (bound at exec time and inherited by descendants), and isolated from the native ABI by construction: a personality-tagged thread cannot invoke a native syscall by number, and a native thread cannot invoke a personality syscall by number.

Personality layers must not alter native ABI semantics. Native applications never observe the presence or absence of any personality. The native ABI evolves on its own schedule and does not negotiate with personality requirements.

Personality subsystems live under kernel/src/personality/<name>/. Each personality is justified by its own ADR; the existence of one personality does not authorize others. Personalities are statically compiled (Bible Article 9.2 forbids loadable kernel modules) and may be conditionally compiled out via Cargo features for embedded or minimal builds.
```

**Formatting notes:**

- Match existing Article 5 clause formatting. Article 5's existing clauses (5.1 through 5.5) use the pattern `5.X <prose>` with paragraph breaks. Replicate.
- The inline path `kernel/src/personality/<name>/` should render as plain text (no backticks) to match how other inline paths render in the existing Bible (e.g., Article 6.1's `kernel/src/ai/` and Article 17's `docs/`).
- If existing clauses use bold for the "5.X" label, match. If they don't, don't.

### Edit 2c — Article 28.2 cross-reference update

**Find** (in Part VI, Article 28 — Product Accessibility, clause 28.2, the bulleted sub-list, the fourth and final bullet about running useful software):

```
Run useful software. The POSIX compatibility shim (Article 3) means common command-line software with sane build systems can be ported with reasonable engineer effort. NeoPlutarch is not Linux, but it is not so alien that the existing software ecosystem is unreachable.
```

**Replace with:**

```
Run useful software. Foreign-OS compatibility is delivered through opt-in personality subsystems (Articles 3, 5.6). The Linux personality (ADR-023) is the first such subsystem; once delivered in Phase 8+, common Linux software runs on NeoPlutarch hardware with no per-application porting. NeoPlutarch is not Linux, but it is not so alien that the existing software ecosystem is unreachable.
```

### Edit 2d — Footer version bump

**Find** the footer block at end of document:

```
Document: NeoPlutarch Bible
Version: 1.6.0
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-11 — Article 17 source tree diagram root renamed `altimitneo/` → `neoplutarch/` to align with the project rename ratified in Article 25.1. Articles 25.1 and 25.2 historical references retained as institutional memory per PM disposition in `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`. Earlier amendments preserved from v1.5.0 footer history.
Authority: Project Lead
```

**Replace with:**

```
Document: NeoPlutarch Bible
Version: 1.7.0
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-21 — Article 3 fourth bullet edited, new Article 5.6 added formalizing personality subsystems as the mechanism for foreign-OS binary compatibility, and Article 28.2 cross-reference updated to point at the new mechanism. Co-ratified with ADR-023 (Linux binary compatibility via personality layer). v1.6.0 history: 2026-05-11 — Article 17 source tree diagram root renamed `altimitneo/` → `neoplutarch/` to align with the project rename ratified in Article 25.1. Articles 25.1 and 25.2 historical references retained as institutional memory per PM disposition in `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`. Earlier amendments preserved from v1.5.0 footer history.
Authority: Project Lead
```

Preserve the surrounding closing-quote epigraph that follows the footer (`"We build real operating systems engineering. Every commit is intended to ship."`). Do not modify it.

---

## Edit 3 — Update `Plans/INDEX.md`

Three sub-edits to `Plans/INDEX.md`. The exact section names below match the format observed in `Plans/INDEX.md` as of 2026-05-21; if section names have evolved (additions happen routinely), match the existing format rather than imposing.

### 3a — Flip DRAFT artifacts to closed in the Artifact Table

In the SC-3B backlog Artifact Table (or its current-day successor — `Plans/INDEX.md` may have additional artifact tables added since the SC-3B backlog landed; use whichever artifact table the recent 2026-05-20 ADR-022 ratification entries appear in), add three new rows in the appropriate alphabetical/chronological position. Use this exact row format (match column widths and separators to existing rows):

```
| `ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` | closed | ADR-023 ratified draft (Linux binary compatibility via personality layer; co-ratified with Bible v1.6.0 → v1.7.0 covering Article 3 fourth bullet edit, new Article 5.6, Article 28.2 cross-reference). ADR text now in `docs/ADR-023-linux-personality-layer.md`. Committed at `<COMMIT_HASH>`. |
| `BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md` | closed | Bible amendment ratified into v1.7.0 at `<COMMIT_HASH>`. Three article touches: Article 3 fourth bullet edit (point at Article 5.6 for mechanism), new Article 5.6 (personality subsystems formalized), Article 28.2 cross-reference update (point at Articles 3, 5.6 and ADR-023). |
| `PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md` | historical | PM-NOTE recording the rescission of the stale-context dispatch and the Option A/B/C surfacing that led to the corrected ADR-023 / Bible v1.7.0 work. Second instance of the stale-context error class (first instance was 2026-05-20 ANSPEC-002 dispatch). Process correction reinforced. |
| `PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md` | closed | PM disposition recording PL ratification of ADR-023 + Bible v1.7.0. Authority chain for the commit produced by `Plans/DISPATCH-2026-05-21-adr-023-and-bible-amendment-prometheus.md`. |
| `DISPATCH-2026-05-21-adr-023-and-bible-amendment-prometheus.md` | closed | Dispatch executing the ratified work: ADR-023 to docs/, Bible amendments applied, INDEX updated. Committed at `<COMMIT_HASH>`. |
| `DISPATCH-2026-05-21-bible-1.1.0-ratification-prometheus.md` | superseded | Rescinded dispatch retained as historical record. Stale-context errors (wrong project name, wrong Bible version, wrong ADR number, wrong phase context, wrong file paths) detailed in `PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md`. Superseded by `DISPATCH-2026-05-21-adr-023-and-bible-amendment-prometheus.md`. |
```

Replace `<COMMIT_HASH>` placeholders with the actual commit hash *after* the commit is created (i.e., create the commit first, then amend INDEX.md with the resolved hash, then amend the commit — or omit the hash field in this commit and add it in a small INDEX-only follow-up commit if cleaner).

### 3b — No Open Project Commitments change

Verify the "Open Project Commitments" table is unchanged by this ratification. No new commitments are created. Article 28.2's accessibility commitment was already at Bible level prior to this work; this ratification adds *architectural mechanism* (Article 5.6, ADR-023), not new deadlines.

### 3c — No Met Commitments change

Verify the "Met Commitments" table is unchanged. No commitments are discharged by this ratification.

---

## Verification before commit

Run these checks in order. Do not commit until all pass.

1. **Git status sanity.** Exactly four files modified or created:
   - `docs/ADR-023-linux-personality-layer.md` (new)
   - `docs/bible.md` (modified)
   - `Plans/INDEX.md` (modified)
   - Nothing else.

   No spurious whitespace-only diffs anywhere.

2. **Diff review (manual).** `git diff --stat` shows:

   - `docs/ADR-023-linux-personality-layer.md`: new file, sizable (the ADR is ~600 lines including the header and forward markers).
   - `docs/bible.md`: small surgical changes — one bullet text edit, one paragraph insertion, one bullet text edit, footer block replaced.
   - `Plans/INDEX.md`: row additions only (no row deletions, no existing row modifications).

3. **ADR-023 status row check.** Open `docs/ADR-023-linux-personality-layer.md` and verify the Status row reads:

   ```
   | **Status** | Accepted (ratified 2026-05-21 per Bible Article 27; Bible v1.7.0 co-ratified) |
   ```

   And does NOT read "DRAFT — awaiting PL ratification".

4. **ADR-023 body unchanged.** Apart from the Status row, the body of `docs/ADR-023-linux-personality-layer.md` should be byte-identical to the body of `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md`. Diff the two files; the only difference should be the Status row.

5. **Bible v1.7.0 render check.** Open `docs/bible.md` and verify:

   - Article 3 has six bullets and the sixth bullet reads the new "A POSIX-compatible system natively…" text.
   - Article 5 has six sub-clauses (5.1 through 5.6); 5.6 is the new personality-subsystems clause.
   - Article 28.2's fourth bullet reads "Run useful software. Foreign-OS compatibility is delivered through opt-in personality subsystems (Articles 3, 5.6). The Linux personality (ADR-023)…"
   - Footer reads `Version: 1.7.0` and the amendment line lists today's changes.
   - Articles 4.5, 6.6, 8.5 (with all sub-clauses), 9.5-9.10, 17, 25.1, 26, 28.x are all unchanged from v1.6.0.

6. **INDEX.md format check.** Open `Plans/INDEX.md` and verify:

   - The new rows match the existing column widths and use the same status terms (`closed`, `historical`, `superseded`).
   - The rows are alphabetized correctly within their section.
   - No existing rows are modified.

7. **No unintended edits.** Run `git diff` and read every changed line. Each change must correspond to one of the four edits above. If anything else has changed (a different file, a different line in one of the four files, a whitespace shift), stop and investigate before committing.

---

## Commit message

Use this commit message verbatim (substituting only the commit-time date if running past midnight UTC):

```
docs: ratify ADR-023 + Bible v1.7.0 (Linux personality layer)

PL ratified 2026-05-21. Co-ratified pair:

  - ADR-023: Linux binary compatibility via personality layer. New
    ADR establishing the architectural mechanism (kernel-resident
    syscall dispatch, opt-in per process at exec, isolated from
    native ABI by construction) for foreign-OS binary compatibility.
    Three syscall tiers (Phase 8 Tier 1, Phase 9 Tier 2, Phase 10+
    Tier 3) plus bright-line exclusion list (namespaces, cgroups,
    eBPF, perf_event, kernel modules, Linux capabilities(7)). Phase
    placement: Phase 8+ for implementation. Forward markers in
    Phases 4 (sockets-equivalent translatability) and 6
    (DRM-compat-expressible accelerator drivers).

  - Bible v1.6.0 → v1.7.0. Three article touches:
      Article 3 fourth bullet: edited to point at Article 5.6 for
        the foreign-OS-compatibility mechanism, replacing the prior
        "POSIX compatibility shim" carve-out language with a
        defined-mechanism reference.
      Article 5.6 (NEW): formalizes personality subsystems —
        kernel-resident, exec-bound per process, isolated from
        native ABI by construction, statically compiled per Article
        9.2, each personality requires its own ADR.
      Article 28.2 cross-reference: updated to point at Articles 3,
        5.6 and ADR-023 (the first personality delivered).

Authority: Bible Article 27 (amendment procedure) + PL signoff per
Plans/PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md.

No code changes. Documentation-only ratification commit.

Refs: ADR-023, Bible v1.7.0,
Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md,
Plans/BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md,
Plans/PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md,
Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md
```

---

## Anti-scope (do not do)

- **Do not modify ADR-023's body content.** Only the Status row. The design discussion, alternatives, consequences, forward markers, and open questions are all already-ratified content and must not be touched.
- **Do not rewrite or reformat Bible articles other than the three amendments.** No "while I'm here" cleanups, no whitespace normalization, no reflowing prose. Atomic ratification commit only.
- **Do not delete the DRAFTs in `Plans/`.** They remain as historical record per the established pattern. The closed-DRAFT pattern is project convention.
- **Do not delete the rescinded `Plans/DISPATCH-2026-05-21-bible-1.1.0-ratification-prometheus.md`.** It also remains as historical record per the 2026-05-20 precedent.
- **Do not draft PERSPEC-001.** Reserved by ADR-023 for Phase 8 planning. The forward-marker checklist in ADR-023 records intent; no actual drafting work happens now.
- **Do not start any of the Phase 4, 6, or 8 forward markers.** Recording them in ADR-023 is the entire deliverable for this commit. Implementation happens in their respective phases.
- **Do not bump any other version numbers.** Only the Bible version field changes. ANSPEC-001, ANSPEC-002, ADRs other than the new ADR-023, the kernel crate version — none of these change.
- **Do not add a "this ADR was ratified" notice inside ADR-023's body.** The Status row flip plus the §"Document end." footer note already encode ratification. Additional ratification narrative inside the ADR is noise.

---

## Uncertainty escalation

If any of the following are true, stop and ferry back to Cerebro before committing:

- The "Find" text for any of the four Bible edits (Edit 2a, 2b anchor, 2c, 2d) does not match the current `docs/bible.md` byte-for-byte. The Bible may have been amended between this dispatch's drafting and your execution; the find-text would then be stale.
- The bullet format of Article 3 or Article 28.2 in the in-tree Bible differs materially from what this dispatch assumes (e.g., bullets are now numbered, or 28.2's sub-list has been restructured).
- The header table in `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` cannot be parsed as expected — the Status row find-text doesn't match.
- `Plans/INDEX.md`'s Artifact Table format has changed since this dispatch was drafted (column count, status terms, ordering).
- Any of the source DRAFT files have been modified since 2026-05-21 EOD — `git log Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` shows a commit after this dispatch was filed.

Stopping and asking is correct behavior. Do not improvise around mismatches. This dispatch's prior version (the rescinded 2026-05-21 v1.1.0-ratification dispatch) was caught by exactly this kind of skepticism; the discipline is working.

---

## Hand-back

On successful commit, ferry back through PL:

- The commit SHA.
- A brief confirmation that all seven verification checks passed.
- Any minor formatting decisions made where this dispatch was underspecified (e.g., "Article 28.2's bulleted sub-list uses leading `-`; the bullet I replaced retained the `-`"). Same hand-back format Prometheus used for the 2026-05-10 sub-step-6 docs closure.
- The INDEX rows should have `<COMMIT_HASH>` placeholders replaced with the actual commit hash. If you committed before resolving the placeholders, file a small follow-up INDEX-only commit to fix them — that follow-up commit's message can be `docs: Plans/INDEX.md — backfill ADR-023 ratification commit hashes`.

That's the entire deliverable. Atomic commit, four files (one new, three modified), no code.

---

**Dispatch issued by Cerebro (PM). Authority: PL ratification 2026-05-21 per `Plans/PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md`.**
