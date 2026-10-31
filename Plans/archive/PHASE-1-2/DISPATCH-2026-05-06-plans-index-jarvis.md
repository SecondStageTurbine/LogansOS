# Dispatch: Create `Plans/INDEX.md` — manifest and status of in-tree dispatch artifacts

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `e6edc65` (currently HEAD on `main`)
**Type:** Doc-only. New file. Single commit on `main`. Does not touch any in-flight work.
**Estimated surface:** ~80–150 lines in a new `Plans/INDEX.md`. No code, no Bible, no spec, no ADR.

---

## 0. Standing instructions

Same as the PHASE-2.md and ADR-014 dispatches you closed at `2016a33` and `e6edc65`.

1. **Commit on `main`, not detached HEAD.** Verify with `git status` and `git branch --show-current` before committing.
2. **Run `git pull --ff-only origin main`** before starting in case Cerebro's investigation lands a commit while you work. (Unlikely — Cerebro is currently running Experiment E and is explicitly NOT committing — but `--ff-only` costs nothing.)
3. **Doc-only.** New file in `Plans/`. No edits to existing files.
4. **OBSERVE before writing.** §3 of this dispatch is the mandatory pre-implementation reading list.
5. **Match the existing style of doc-only artifacts.** ADR-014 is a good model for a freshly-created small doc.

---

## 1. What this dispatch produces

A new file `Plans/INDEX.md` that catalogs every artifact currently in `Plans/`, with a status indicator for each: **closed** (the dispatch was completed and the resulting commit landed on `main`), **superseded** (the dispatch was overtaken by a follow-up that did the same work differently), **draft** (a working artifact that produced its commit but stays in tree as a record), **active** (the dispatch is in flight; coder is working on it), or **historical** (the artifact records past state — handoffs, decision notes, etc.).

The point: future-PM, future-Jarvis, or future-Cerebro opening this directory should be able to tell at a glance which files are live, which are done, and what each one was for — without having to grep through filenames or open every file.

This is bookkeeping, not investigation. The information all already exists; you're just collating it into one place.

---

## 2. What this dispatch is NOT

- **Not a phase document update.** PHASE-2.md is separate and gets updated when sub-commits close. INDEX.md is a working-directory manifest, not a phase log.
- **Not a decision log.** PM decision artifacts already exist as their own files (`PM-DECISION-*.md`); INDEX.md just lists them.
- **Not a complete project history.** It indexes only what's currently in `Plans/`. Past HANDOFFs that no longer live there are not your concern.
- **Not normative.** INDEX.md is a working-tree convenience, not a governance artifact. It does not require ADR ratification or Bible reference. It does not get cited from elsewhere.
- **Not an opportunity to editorialize.** Each entry's description should be one or two sentences naming what the artifact is and what its closure state is. Not "Cerebro did a great job here" or "this dispatch was over-scoped." Just facts.

---

## 3. Pre-implementation OBSERVE pass

Read or skim each file in `Plans/` to understand what it is. The current state (as of dispatch drafting):

```
Plans/
├── .commit-msg-bible-6-6.txt
├── BIBLE-AMENDMENT-DRAFT-2026-05-06-article-6-6-logit-port.md
├── DISPATCH-2026-05-06-adr-014-stub-jarvis.md
├── DISPATCH-2026-05-06-anspec-001-bible-version-fix-jarvis.md
├── DISPATCH-2026-05-06-build3-sub-commit-1b-cerebro.md
├── DISPATCH-2026-05-06-build3-sub-commit-2-cerebro.md
├── DISPATCH-2026-05-06-build3-sub-commit-2-restore-cerebro.md
├── DISPATCH-2026-05-06-experiment-D-cerebro.md
├── DISPATCH-2026-05-06-experiment-E-cerebro.md
├── DISPATCH-2026-05-06-phase-2-md-update-jarvis.md
├── DISPATCH-2026-05-06-sub-commit-2-isolate-cerebro.md
├── HANDOFF-2026-05-05-build3-sub-commit-1a-close.md
├── PM-DECISION-2026-05-06-Q5-bet-failed.md
└── PM-DECISION-2026-05-06-sub-commit-2-smoke-hang.md
```

For each file, you need to know:

- **What it is.** Read the front-matter header or first paragraph; that's enough.
- **Whether it closed.** Run `git log --oneline | grep -i <relevant-term>` to find the matching commit, OR check the commit messages on `main`. The dispatch commit messages typically reference the dispatch by a clearly recognizable phrase.
- **Whether it was superseded.** Some dispatches were rewritten as follow-ups (e.g., the original sub-commit 2 dispatch was overtaken by the restore dispatch and then the isolate dispatch).

The mapping table in §4 below tells you what status each file should get; verify by reading the artifact and checking the commit log. If your verification disagrees with §4, **flag it in your close report** and surface to PM rather than silently changing the table.

---

## 4. The expected status mapping

Use this as your starting point. Verify each entry against the actual git history and artifact contents; if anything contradicts, surface in close report.

| File | Status | Closed at / current state |
|------|--------|---------------------------|
| `.commit-msg-bible-6-6.txt` | historical | Helper file used to compose the Article 6.6 commit message. Commit landed at `503d3dc`. |
| `BIBLE-AMENDMENT-DRAFT-2026-05-06-article-6-6-logit-port.md` | closed | Article 6.6 ratified into Bible v1.0.4 at `503d3dc`. |
| `DISPATCH-2026-05-06-adr-014-stub-jarvis.md` | closed | ADR-014 stub committed at `e6edc65`. |
| `DISPATCH-2026-05-06-anspec-001-bible-version-fix-jarvis.md` | closed | ANSPEC-001 §15 stale Bible reference fixed at `3d1b908`. |
| `DISPATCH-2026-05-06-build3-sub-commit-1b-cerebro.md` | closed | AP bring-up via Limine MP request committed at `3bfcfc8`. |
| `DISPATCH-2026-05-06-build3-sub-commit-2-cerebro.md` | superseded | Original sub-commit 2 dispatch including §Q5 deletion. The §Q5 bet failed at the closure gate; superseded by the restore dispatch. Working tree from this dispatch's implementation is still uncommitted. |
| `DISPATCH-2026-05-06-build3-sub-commit-2-restore-cerebro.md` | superseded | Restore dispatch (§Q5 mechanism reintroduced in per-CPU form). Restoration applied; closure gate failed at run 81 with a different bug; superseded by the isolation dispatch. |
| `DISPATCH-2026-05-06-experiment-D-cerebro.md` | closed | EXP-D ran 100/100 PASS. Result: hang requires test-harness kernel binary, not QEMU flags. No commit (diagnostic only); result captured in `target/verify/EXP-D-summary.txt` and PM context. |
| `DISPATCH-2026-05-06-experiment-E-cerebro.md` | active | EXP-E in flight — `kaslr_smoke` 100x at NEO_SMP=4 to bisect "any test-harness binary" vs "specific to IPC scenario." |
| `DISPATCH-2026-05-06-phase-2-md-update-jarvis.md` | closed | PHASE-2.md status update through sub-commit 1a committed at `2016a33`. |
| `DISPATCH-2026-05-06-sub-commit-2-isolate-cerebro.md` | closed | Three-experiment isolation dispatch (A/B/C). A and B ran 100/100 each; C was skipped per dispatch decision rule. Result: hang requires SMP+test-harness intersection. No commit (diagnostic only); result captured in `target/verify/EXP-CLOSE-REPORT.md`. |
| `HANDOFF-2026-05-05-build3-sub-commit-1a-close.md` | historical | Handoff document from sub-commit 1a close. Used as the entry point into the 2026-05-06 session. Items it flagged are tracked individually below. |
| `PM-DECISION-2026-05-06-Q5-bet-failed.md` | historical | PM decision note recording why §Q5 was reverted. The decision drove the restore dispatch. |
| `PM-DECISION-2026-05-06-sub-commit-2-smoke-hang.md` | historical | PM decision note recording the sub-commit 2 smoke-test hang and the choice to investigate before committing. The decision drove the isolation dispatch and the subsequent experiment dispatches. |

---

## 5. The structure `INDEX.md` should have

Write a doc with three sections.

### 5.1 Title and intent

```markdown
# Plans Directory Index

This directory holds in-flight and recently-closed working artifacts:
PM dispatches to coders, PM decision notes, Bible amendment drafts, and
session handoffs. Files in `Plans/` are working artifacts, not normative
project documents — normative docs (Bible, ADRs, ANSPEC, phase plans)
live in `docs/`.

This index is updated by hand. It is not authoritative; the truth is in
the files themselves and in `git log`. The index is a navigation aid.
```

Adjust the wording to match the project voice (look at how ADR-014 opens its Context section for tone). Keep it short.

### 5.2 Status legend

A short table or paragraph defining the status terms used:

- **closed** — the artifact's intended commit has landed on `main`. The artifact is kept for record.
- **superseded** — overtaken by a follow-up that did the same work differently. Kept for record of what was tried.
- **draft** — a working artifact (e.g., a Bible amendment draft) that produced its commit but stays in tree as the source of the prose.
- **active** — the artifact is in flight; the coder is working on it now or the closure gate hasn't been met.
- **historical** — captures past state (handoffs, decision notes, helper files). Not in flight, not a dispatch.

### 5.3 The artifact table

The table from §4 above, in `INDEX.md`'s own words. Same shape: file, status, what-it-is. Match the existing markdown table style elsewhere in the repo (look at `docs/research/ALTIMIT-real-world-implementation.md` or ADR-013 for table syntax conventions).

### 5.4 Optional fourth section: maintenance protocol

A short note describing when this index gets updated. Suggested wording:

```markdown
## Maintenance

This index gets updated when:
- A new dispatch lands in `Plans/` (add an `active` entry).
- A dispatch's commit lands on `main` (flip status to `closed`, add commit hash).
- A dispatch is superseded by a follow-up (flip status to `superseded`).

The index is not a dependency for any other artifact. If it falls out of
date, that's an inconvenience, not a correctness issue.
```

Keep this section optional — include it if you think it lands cleanly; skip if it makes the doc feel bureaucratic. PM will have no objection either way.

---

## 6. What you do NOT do

- **Do not modify any existing file in `Plans/`.** Only the new `INDEX.md`.
- **Do not modify files outside `Plans/`.** No Bible, ADR, spec, or kernel source changes.
- **Do not summarize each artifact's full content.** One or two sentences per row is plenty. Future readers go to the artifact for detail; the index just points them there.
- **Do not include line counts, file sizes, or other metadata that would go stale fast.** Status + intent only.
- **Do not commit instrumentation or temporary files into `Plans/`.** Anything in `target/verify/` (experiment outputs) stays there; the index references the close-report files by path but does not duplicate their content.
- **Do not editorialize.** No "this was a great dispatch" or "this was over-scoped." Just status + one-sentence intent.
- **Do not create `INDEX.md` in any other directory.** This dispatch is specifically for `Plans/INDEX.md`.

---

## 7. Verification gates

### 7.1 Doc-only confirmation

```
git diff --stat <parent>..HEAD
```

Expected: exactly one file changed (`Plans/INDEX.md`), and it's a new file (only insertions, zero deletions).

### 7.2 Markdown sanity

The file should render cleanly:

- Tables align.
- No broken cross-references (every file you list must actually exist in `Plans/`).
- Heading levels consistent.

### 7.3 Status accuracy spot-check

For at least three entries you mark as `closed`, verify the commit hash exists on `main`:

```
git log --oneline | findstr <hash>
```

(Or `Select-String` if you prefer PowerShell.) If a commit hash you cited doesn't exist on `main`, the status is wrong — fix before committing.

### 7.4 Length sanity

If your draft is over ~200 lines, you've over-engineered it. The point is a manifest, not a treatise. Trim.

If under ~50 lines, you've probably skipped the maintenance section or compressed entries too much. Verify each artifact has at least its file + status + one-sentence intent.

### 7.5 Commit shape

Single commit on `main`. Suggested commit message:

```
docs: Plans/INDEX.md — manifest of in-flight and closed working artifacts

The Plans/ directory has accumulated dispatches, PM decision notes, and
handoffs from the 2026-05-05 to 2026-05-06 sessions. Without an index,
future readers parse filenames to figure out what's live, what's done,
and what each artifact was for.

INDEX.md catalogs every file currently in Plans/ with one of five
statuses: closed (commit landed on main), superseded (overtaken by a
follow-up), draft (artifact produced its commit but stays in tree),
active (in flight), historical (captures past state). Each entry is a
one-sentence intent + status indicator + (where applicable) commit
hash.

The index is a navigation aid, not a normative document. It is not
cited from elsewhere; it is updated by hand when artifacts are added,
closed, or superseded.
```

Adjust to match what you actually wrote. The substance must reflect the doc's content.

---

## 8. If your verification finds discrepancies

If the §4 mapping table is wrong about anything (e.g., a "closed" entry's commit doesn't exist on `main`, or a "superseded" entry was actually closed), **stop and surface to PM** with the specific discrepancy. Don't silently rewrite — the discrepancy might indicate that the working tree is in an unexpected state, which is information PM needs.

The most likely place this could go wrong: the dispatches Cerebro's been working on (sub-commit 2, restore, isolate, D, E). Those track investigative work that doesn't always end in a commit. The expected pattern is:

- **Sub-commit 2 original**: implementation applied; closure gate failed; superseded.
- **Sub-commit 2 restore**: restoration applied to working tree; closure gate failed differently; superseded.
- **Sub-commit 2 isolate**: experiments A/B run; result was diagnostic; no commit.
- **Experiment D**: experiment ran; result was diagnostic; no commit.
- **Experiment E**: in flight; no commit.

If your verification finds something different from this pattern (e.g., one of these dispatches did produce a commit you didn't expect), surface it.

---

## 9. Why this matters

The original sub-commit 2 investigation has produced a substantial paper trail (~10 dispatches and decision notes) over a few hours. If a fresh session opens this project tomorrow — or if PM resumes after a break, or if Cerebro hits a new failure and needs to retrace what's been tried — the time-to-context is much shorter with an index than without one.

This is the same reason the bible has a footer with version history, ADRs have a "supersedes" field, and PHASE-N.md docs maintain a status section. Working-directory artifacts deserve the same minimum bookkeeping.

This dispatch can run in parallel with Cerebro's Experiment E without any risk of conflict — different file, different concern, no shared state.

---

**End of dispatch.**

*Standard close report shape: confirm the new file lands at the right path, paste the commit hash, list any status-mapping discrepancies you found and how you resolved them. ~20–30 minutes if you're efficient.*
