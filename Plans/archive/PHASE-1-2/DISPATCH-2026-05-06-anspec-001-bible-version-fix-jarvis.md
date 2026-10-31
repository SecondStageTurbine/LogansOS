# Dispatch: ANSPEC-001 §15 stale Bible reference fix

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `503d3dc` (currently HEAD on `main`)
**Type:** Doc-only. One file. One line. Single commit on `main`.
**Estimated surface:** Tiny. Single-line edit.
**Triage origin:** Handoff §7 documentation-work queue, item 3.

---

## 0. Standing instructions

1. **Commit on `main`, not detached HEAD.** Run `git status` and `git branch --show-current` before your first commit. Quick sanity check.

2. **Run `git pull --ff-only origin main`** before starting if anything's pushed remotely. Local-only setup, so probably not relevant, but the discipline is the discipline.

3. **Doc-only.** No code. No ADR changes. No header bumps to ANSPEC-001 — see §3 for why.

4. **This dispatch is trivially small.** No close report theatrics needed; a one-line "done at <hash>" is sufficient.

---

## 1. The bug

`docs/specs/altimitneo-ai-subsystem-spec.md` line 934 reads:

```
- AltimitNeo Bible v1.0.0, Articles 2, 6, 19–23.
```

The Bible is now v1.0.4 (committed today at `503d3dc`). The version pin in ANSPEC-001's bibliography is three amendments out of date:

- v1.0.1 ratified 2026-05-02 (initial ratification)
- v1.0.2 ratified 2026-05-04 (or thereabouts; clip from earlier history)
- v1.0.3 added Article 4.5 (single-node scope) on 2026-05-05
- v1.0.4 added Article 6.6 (inference-governance hooks) on 2026-05-06

Verify the current Bible version by reading `docs/bible.md` footer block; do not trust this dispatch's number from memory.

The other parts of line 934 — `Articles 2, 6, 19–23` — remain correct. Article 6 has expanded (6.6 was added) but the spec is still grounded in Article 6, transitively including its sub-clauses. Article 4.5 isn't in the cited range and ANSPEC-001 doesn't lean on it. No expansion of the cited-articles range is needed.

---

## 2. The fix

Line 934 changes from:

```
- AltimitNeo Bible v1.0.0, Articles 2, 6, 19–23.
```

to:

```
- AltimitNeo Bible v1.0.4, Articles 2, 6, 19–23.
```

That's the entire diff. One line, one substitution.

---

## 3. What you do NOT touch

- **The ANSPEC-001 header block** (lines 7-12, `Document ID` through `Last revised`). Don't bump `Version:` from `0.3.0` to `0.3.1`. Don't update `Last revised:` from `2026-05-05` to `2026-05-06`. Reasoning: §15 is the bibliography. Fixing a stale citation is the documentation equivalent of correcting a typo in a footnote — it does not change the spec's normative content and does not represent a revision of the spec itself. ANSPEC-002 followed the same discipline today: I updated three lines in ANSPEC-002 to cross-reference Bible 6.6 without bumping ANSPEC-002's version, and that was correct. Same rule applies here.

  If a future reader wants to know "which Bible version was ANSPEC-001 authored against?", `git blame` answers that question precisely and version-bumps don't.

- **Any other line in ANSPEC-001.** No prose touch-up, no formatting cleanup, no §3.7 reconciliation tweaks. ANSPEC-001 §3.7 was already amended in commit `2f6d969` (ANSPEC-002 introduction) — leave it alone.

- **No other docs.** ANSPEC-002, the Bible, ADR-013 are all current. Don't touch them.

- **No code.** This is a docs/specs/ edit only.

---

## 4. Pre-implementation OBSERVE

Before editing:

1. `Filesystem:read_text_file docs/bible.md` — confirm the current Bible footer reads `Version: 1.0.4`. If it reads anything different, **stop and report**; the dispatch's premise is wrong.
2. `Filesystem:read_text_file docs/specs/altimitneo-ai-subsystem-spec.md` (or use `Select-String` to find line 934) — confirm the line currently reads `- AltimitNeo Bible v1.0.0, Articles 2, 6, 19–23.` exactly. If the line has drifted (different version, different article range), **stop and report**.

---

## 5. Verification

After commit:

```
git diff --stat <parent>..HEAD
```

Expected: exactly one file changed (`docs/specs/altimitneo-ai-subsystem-spec.md`), 1 insertion(+), 1 deletion(-).

Verify the line content with:

```
Select-String -Path "docs\specs\altimitneo-ai-subsystem-spec.md" -Pattern "AltimitNeo Bible v"
```

Expected: one match, line 934, reading `- AltimitNeo Bible v1.0.4, Articles 2, 6, 19–23.`

---

## 6. Commit shape

Single commit on `main`. Suggested commit message:

```
docs: ANSPEC-001 §15 — bump stale Bible v1.0.0 reference to v1.0.4

ANSPEC-001's References section pinned Bible v1.0.0; the Bible has since
been amended four times (most recently to v1.0.4 in commit 503d3dc, which
added Article 6.6). The spec's normative content is unchanged — Articles
2, 6, 19–23 remain the substrate ANSPEC-001 is grounded in, and Article
6 transitively includes 6.6. Only the version pin in the bibliography
entry is updated.

ANSPEC-001 header (Version: 0.3.0, Last revised: 2026-05-05) is
intentionally untouched: §15 is bibliography, not normative content.
This is the same discipline ANSPEC-002 followed in 503d3dc when its
own §14 reference was updated.

Triaged from HANDOFF-2026-05-05-build3-sub-commit-1a-close §7
documentation-work queue.
```

---

## 7. Why this matters even though it's tiny

Stale version pins compound. Today it's "v1.0.0 → v1.0.4 (4 versions late)." If unfixed, in three months it might be "v1.0.0 → v1.2.7 (well, the 6.6 reference doesn't even apply anymore because 6.6 was renumbered when Part IV got reorganized)." The cost of a stale citation grows nonlinearly — fix when noticed.

This also closes a triage-queue item, leaving the queue smaller for the next handoff.

---

**End of dispatch.**

*This is a 5-minute fix. Quick close report acknowledging the commit hash is sufficient — no full structured close needed.*
