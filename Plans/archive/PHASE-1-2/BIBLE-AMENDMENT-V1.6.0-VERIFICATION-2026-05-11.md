# Bible Amendment v1.6.0 Verification Report

**Date:** 2026-05-11
**Author:** Prometheus
**Dispatch:** `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`
**Commit:** `6231cf0` on `origin/main`

---

## 1. Diff Summary

| Metric | Value |
|--------|-------|
| Files changed | 1 (`docs/bible.md`) |
| Insertions | 3 |
| Deletions | 3 |
| Net lines changed | 0 (pure replacements) |
| Non-whitespace line touches | 4 (diagram root, version, last amended, footer history reference) |

**Changes:**
- Line 240: `altimitneo/` → `neoplutarch/` (Article 17 source tree diagram root)
- Line 341: `Version: 1.5.0` → `Version: 1.6.0`
- Line 344: `Last amended:` line updated with v1.6.0 amendment summary

---

## 2. Article 25.1 and 25.2 Unchanged — Evidence

Both articles verified intact post-amendment (current content from working copy):

**Article 25.1 (line 307):**
> "NeoPlutarch" is the project name. "NeoPlutarch" in formal docs and commits. The project was originally named "AltimitNeo" (Phase 0 through Phase 2 BUILD-3 sub-commit 2); a 2026-05-10 trademark-driven rename adopted "NeoPlutarch" to remove derivative-name exposure to Bandai Namco's `.hack` franchise. The rename is recorded in this commit; pre-rename project artifacts in Plans/ retain their original "AltimitNeo" naming as historical record.

**Article 25.2 (line 308):**
> Subsystem prefixes: neo- for userland binaries (neo-init, neo-shell), libneo for the userland runtime, .neom for native model files, NeoFS for the filesystem. These subsystem-name prefixes are preserved as established convention even after the 2026-05-10 project rename from "AltimitNeo" to "NeoPlutarch"; the "neo" prefix is now a project-internal convention, not a brand fragment.

**Verification:** Neither line appears in `git diff`. Articles 25.1 and 25.2 are RETAINED AS-IS per PM disposition.

---

## 3. Footer Version-Bump Confirmation

Post-amendment footer:
```
Document: NeoPlutarch Bible
Version: 1.6.0
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-11 — Article 17 source tree diagram root renamed `altimitneo/` → `neoplutarch/` to align with the project rename ratified in Article 25.1. Articles 25.1 and 25.2 historical references retained as institutional memory per PM disposition in `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`. Earlier amendments preserved from v1.5.0 footer history.
Authority: Project Lead
```

Version: 1.6.0 ✓
Amendment summary present ✓
"Earlier amendments preserved from v1.5.0 footer history" phrasing ✓

---

## 4. Push Verification

- Branch: `prometheus/bible-amendment-v1.6.0-2026-05-11`
- Commit SHA: `6231cf0`
- Push target: `origin/main`
- Result: Clean fast-forward (`ead8b19..6231cf0  HEAD -> main`)
- No merge conflicts, no rebase, no force push

---

## 5. Incidental Observations

None. Amendment scope matched dispatch exactly: one diagram root rename + version bump + footer update. Articles 25.1/25.2 untouched. No other articles affected.

---

**End of verification report.**

*Bible amendment v1.5.0 → v1.6.0 executed. Single commit `6231cf0` pushed to `origin/main`. Article 17 diagram root renamed. Articles 25.1 and 25.2 retained as institutional memory. All verification gates passed.*