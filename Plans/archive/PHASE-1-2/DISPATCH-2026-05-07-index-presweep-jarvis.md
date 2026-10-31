# Dispatch: Pre-draft the INDEX.md sweep for sub-commit 2 closure

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-07
**Type:** Doc-only pre-draft. NOT a commit. Output is `Plans/INDEX-DRAFT-2026-05-07.md` — a polished INDEX.md replacement ready to be `mv`'d to `Plans/INDEX.md` and committed when sub-commit 2 closes.
**Substrate:** `Plans/INDEX.md` (the current committed INDEX, ~stale by 2 days). All artifacts in `Plans/` are the source of truth for what entries exist; the INDEX is a navigation aid that summarizes them.
**Sequence:** This dispatch runs *after* dispatch A (`DISPATCH-2026-05-07-adr-015-finalize-jarvis.md`) completes. PM signals A's completion before C starts.

---

## 0. Standing instructions

1. **Do NOT commit anything.** Output is a single `Plans/` file (the pre-drafted INDEX).
2. **Do NOT modify the existing `Plans/INDEX.md`.** That stays as-is until sub-commit 2 closes; the swap from pre-draft to live INDEX happens then.
3. **Do NOT modify any tracked file** under `kernel/`, `boot/`, `userland/`, `docs/`, `tools/`, `tests/`, or anywhere else outside `Plans/`.
4. **Do NOT touch any stash.**
5. Output: `Plans/INDEX-DRAFT-2026-05-07.md`. Same shape as current `Plans/INDEX.md`, updated with all artifacts from 2026-05-06 14:00 onward through 2026-05-07.

---

## 1. The task

The current `Plans/INDEX.md` is stale by ~2 days. It's missing entries for everything from 2026-05-06 14:00 onward (the EXP-F/G/H experiments, the bisect-step1 dispatch, the failed close attempt, the kaslr-overflow PM decision, the kaslr diagnostic dispatch, and all of 2026-05-07's artifacts). It also still lists older items as `active` that have since been superseded or closed.

Pre-draft a polished replacement INDEX.md that captures all current artifacts in `Plans/` with correct statuses, ready to be `mv`'d to `Plans/INDEX.md` after sub-commit 2 closes.

---

## 2. Sources

### 2.1 Existing INDEX

Read `Plans/INDEX.md` for the current shape, status legend, and table format. Match that format for the pre-draft.

### 2.2 Artifact list

The full list of artifacts to include is `Get-ChildItem Plans -File | Where-Object { $_.Name -ne 'INDEX.md' }`. Run that command to get the authoritative list. Some you'll recognize from the existing INDEX (kept, possibly with status flipped); many are new since 2026-05-06 14:00.

### 2.3 Sub-commit 2 closure context

Per `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md` and `Plans/PM-DECISION-2026-05-07-kaslr-q1-higher-n.md`, sub-commit 2 closes today (or tomorrow morning if Cerebro defers per the dispatch's fatigue clause). The pre-draft INDEX assumes sub-commit 2 will commit and updates statuses accordingly.

If sub-commit 2 hasn't actually committed when the pre-draft is `mv`'d to live INDEX, the pre-draft's status assumptions need adjustment. PM handles that adjustment at swap time — your job is to write the pre-draft assuming closure has happened.

---

## 3. Status assignments

For each artifact, assign one of the five statuses from `Plans/INDEX.md`'s legend:

| Status | When to use |
|--------|-------------|
| **closed** | The artifact's intended commit has landed on `main`. |
| **superseded** | Overtaken by a follow-up that did the same work differently. |
| **draft** | A working artifact that produced its commit but stays in tree as the source of the prose. |
| **active** | The artifact is in flight; closure gate has not been met. |
| **historical** | Captures past state (handoffs, decision notes, helper files). |

### 3.1 Specific status calls

For sub-commit 2's closure dispatch chain, follow this rubric:

- **Original closure dispatches that failed** (e.g., the §Q5-deletion attempt, the smoke-hang investigation chain): `superseded` — they were overtaken by the bisect-rollback and the eventual EXP-H success. Note in the description what superseded them.

- **Bisection experiments (A through H)**: `closed` — diagnostic-only, no commit, but they "closed" in the sense of producing their decisive result. EXP-A through EXP-H each have a result line; record it briefly.

- **The 2026-05-07 closure dispatch** (`DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md`): `closed` — sub-commit 2 commit landed (assume yes for the pre-draft).

- **KASLR diagnostic chain**: 
  - `DISPATCH-2026-05-06-kaslr-overflow-diagnostic-cerebro.md` → `closed` (Q1+Q2 produced their result).
  - `DISPATCH-2026-05-07-kaslr-q1-higher-n-cerebro.md` → `closed` (KASLR-Q1B produced 4/50 panics, pre-existence confirmed).
  - `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` → `historical` (the close report itself is the record of the diagnostic; not a dispatch).

- **ADR-015 chain**:
  - `Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md` → `superseded` by the FINAL version once Jarvis finishes dispatch A.
  - `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` → `draft` (it's ready for commit but hasn't committed to `docs/` yet — the implementation dispatch does that).
  - `Plans/DISPATCH-2026-05-07-adr-015-finalize-jarvis.md` → `closed` once dispatch A's artifact lands.

- **§Q9 redo notes chain**:
  - `Plans/DISPATCH-2026-05-07-q9-redo-design-notes-jarvis.md` → `closed` (the design notes were drafted).
  - `Plans/DISPATCH-2026-05-07-q9-redo-revisions-jarvis.md` → `closed` (the four edits were applied).
  - `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` → `draft` (research artifact, future ADR-016 input — analogous to ADR-015-FINAL's status, not yet ratified).

- **SC-3A pre-thinking**:
  - `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` → `draft` (PM design notes, becomes input to SC-3A's pre-implementation checkpoint when scheduled).

- **PM decisions**: 
  - PM-DECISIONs that drove dispatches whose work is now closed → `historical`.
  - PM-DECISIONs awaiting implementation (none currently) → `active`.

- **Handoffs**:
  - `Plans/HANDOFF-2026-05-05-build3-sub-commit-1a-close.md` → `historical`.
  - `Plans/HANDOFF-2026-05-06-end-of-day-kaslr-blocked.md` → `historical`.

- **Yesterday's INDEX dispatch** (`Plans/DISPATCH-2026-05-06-plans-index-jarvis.md`): `closed` (the previous INDEX update landed; this current sweep supersedes it).

### 3.2 Flip statuses on existing entries

The current `INDEX.md` lists several entries as `active` that should now flip:

- `DISPATCH-2026-05-06-experiment-E-cerebro.md` is currently `active` — flip to `closed`.
- `DISPATCH-2026-05-06-plans-index-jarvis.md` is currently `active` — flip to `closed`.

The old `superseded` entries (sub-commit 2 original dispatches) stay `superseded` but the descriptions can be updated to reference the newer chain (bisect → EXP-H → 2026-05-07 closure).

---

## 4. Structure

Match `Plans/INDEX.md`'s existing structure:

```markdown
# Plans Directory Index

[same intro paragraph as current INDEX]

## Status Legend

[same table as current]

## Artifact Table

[the big table — sorted alphabetically by filename, OR grouped by date with subheadings,
your call. See §5 for the structural decision.]

## Maintenance

[same maintenance paragraph as current]
```

---

## 5. Structural decision: alphabetical vs date-grouped

The current INDEX uses a single alphabetical table. With ~30 artifacts now, that's getting long. Two options:

### Option α — Keep alphabetical (current shape, just expanded)

One table, sorted alphabetically by filename. Works because dispatch filenames already encode the date (`DISPATCH-2026-05-06-...`), so alphabetical effectively groups by date anyway.

### Option β — Group by date

Three subsections: "2026-05-05 and earlier," "2026-05-06," "2026-05-07." Each section has its own table. More navigable for a coordinator scanning for "what happened today" but introduces three tables instead of one.

**PM decision: keep alphabetical (Option α).** Reasons:
- Date-grouped subsections become an organizational burden to maintain on every update.
- Alphabetical-by-filename already produces effective date grouping because of the date-in-filename convention.
- The "what happened today" use case is better served by `Get-ChildItem Plans -Filter "*2026-05-07*"` than by INDEX structure.

So: one alphabetical table, expanded to include all current entries. This is the same shape as the current INDEX, just with ~3x the row count.

---

## 6. Field discipline per row

Each row is `| File | Status | What it is |`. The "What it is" column should be one sentence, ~10-25 words. Don't summarize the dispatch's contents at length — just identify it. Example shapes:

- For dispatches: "Dispatched X to do Y. Result: Z." (e.g., "EXP-H: 100x sub-commit 2 minus Q9 — 100/100 PASS, confirmed Q9 was the cause.")
- For PM decisions: "PM decision recording <reason>; <what it drove>." (e.g., "PM decision: take Q1 to N=50 on main HEAD; drove KASLR-Q1B dispatch.")
- For handoffs: "Handoff from <event>; entry point into the <date> session."
- For drafts: "Draft <type>; awaits <event> to <transition>." (e.g., "Draft ADR-015 final text; awaits SC2 closure + implementation dispatch to commit to docs/.")

If the original entry in the current INDEX has a good description, keep it. Don't rewrite for the sake of rewriting.

---

## 7. Time budget

- 5 min: read this dispatch and the existing `Plans/INDEX.md`.
- 5 min: enumerate all artifacts via `Get-ChildItem Plans -File | Where-Object { $_.Name -ne 'INDEX.md' }`.
- 30 min: assign statuses per §3 rubric and write descriptions per §6 discipline.
- 15 min: format into the table, sort alphabetically, fold into the INDEX shape per §4.
- 5 min: re-read for consistency (status legend usage, description tone).

**Total: ~1h.** If you hit 90 min and aren't done, surface — PM picks up the rest.

---

## 8. What you do NOT do

- Do not modify `Plans/INDEX.md` itself. Pre-draft only.
- Do not invent statuses outside the five-status legend.
- Do not add new sections (Recent Activity, Project Map, etc.) — match the existing structure.
- Do not include artifacts that are not in `Plans/`. The INDEX is for `Plans/` only; ADRs in `docs/`, ANSPECs, phase plans, and Bible amendments have their own homes.
- Do not assume your status calls are infallible. If a status assignment is genuinely ambiguous, mark with `active` and add a note in the description; PM resolves at swap time.

---

## 9. Close

When done, ping PM with: "INDEX pre-draft: `Plans/INDEX-DRAFT-2026-05-07.md`, <total entry count>, any ambiguous status calls or structural deviations."

PM reviews the pre-draft, makes any final adjustments, and holds it ready for swap-to-live when sub-commit 2 closes.

---

## 10. Why this matters

INDEX.md is the navigation aid that future PMs and project lead use to find artifacts. After 2 days of activity, it's stale enough that "where's the close report for Q1B?" is harder than it should be. Pre-drafting now while the artifacts are fresh in the dispatching queue means the swap-to-live happens cleanly the moment sub-commit 2 closes — no "let me reconstruct what happened today" mental load.

---

**End of dispatch.**
