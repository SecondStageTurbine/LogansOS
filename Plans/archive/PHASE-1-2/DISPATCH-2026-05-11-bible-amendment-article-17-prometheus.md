# DISPATCH — Bible amendment: Article 17 source tree diagram root (Prometheus)

**Date filed:** 2026-05-11
**From:** PM
**To:** Prometheus (Qwen3.6-27B, docs + cleanup + small-coding-trial lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.2)
**Ferry:** PL ferries to Prometheus session.
**Status:** PL-tier-authorized amendment. PL approved the Bible amendment workstream on 2026-05-11; PM owns operational dispositions on the three PL-TIER items surfaced in `Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md` §3.4. Per Bible Article 27.1, amendments require PR with rationale + version bump. Bible currently at v1.5.0 (footer: ratified 2026-05-02, last amended 2026-05-10 for Article 8.5.4 expansion); this amendment bumps to v1.6.0.

---

## 1. Mandate

Amend `docs/bible.md` Article 17 to rename the source tree diagram root from `altimitneo/` to `neoplutarch/`. Update footer with amendment summary. Single commit per Bible Article 27.1 governance. Push to `origin/main`.

**PM dispositions on the three PL-TIER items** (refined from `Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md` §3.4 after PM read the actual Article 25.1/25.2 text):

| # | Reference | PM disposition |
|---|---|---|
| 1 | Article 17 line 240 — source tree diagram root `altimitneo/` | **AMEND to `neoplutarch/`.** The Bible documents the project's normative source layout; the project name is NeoPlutarch (Article 25.1); the diagram root should match the project name. Consistent with Article 25.2's convention. |
| 2 | Article 25.1 line 307 — "AltimitNeo" in rename narrative | **RETAIN AS-IS.** Reading the actual text: "The project was originally named 'AltimitNeo' (Phase 0 through Phase 2 BUILD-3 sub-commit 2); a 2026-05-10 trademark-driven rename adopted 'NeoPlutarch'..." This is intentional institutional memory documenting why the project was renamed. Removing it would lose the rationale (trademark-driven, Bandai Namco `.hack` franchise exposure). |
| 3 | Article 25.2 line 308 — "AltimitNeo" in subsystem prefix clause | **RETAIN AS-IS.** Reading the actual text: "These subsystem-name prefixes are preserved as established convention even after the 2026-05-10 project rename from 'AltimitNeo' to 'NeoPlutarch'; the 'neo' prefix is now a project-internal convention, not a brand fragment." This is load-bearing context explaining why `neo-` prefixes persist post-rename. Removing it would invite future confusion about whether `neo-` is a brand-fragment vestige. |

**Net scope:** one Article 17 amendment + version bump + footer update. Articles 25.1 and 25.2 unchanged.

## 2. Inputs to consume

1. **`docs/bible.md`** — constitutional document at v1.5.0.
2. **Bible Article 27.1** (governance process):
   > 27.1 This bible is amended by pull request. An amendment names the article(s) modified, includes a rationale section, bumps the version in the footer, and is approved by the project lead.
3. **`Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md`** §3.4 — PM ratification with original dispositions; §1 of this dispatch refines those dispositions post-Bible-read.
4. **`Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md`** §3 — audit reference for the line numbers.
5. **Bible footer current state** (lines near end of `docs/bible.md`):
   ```
   Document: NeoPlutarch Bible
   Version: 1.5.0
   Status: Ratified
   Ratified: 2026-05-02 (commit aa2c949)
   Last amended: 2026-05-10 (later session) — Article 8.5.4 expanded; ...
   Authority: Project Lead
   ```

## 3. Scope: single Article 17 amendment

### 3.1 Article 17 source tree diagram

Locate the source tree diagram in `docs/bible.md` Article 17. The diagram currently begins:

```
altimitneo/
├── boot/                  # Limine config, boot artifacts
│   └── firmware/          # Vendor firmware blobs by family (Phase 7.5+, see ADR-017)
│       └── <vendor>/<chipset>/
├── kernel/                # The kernel crate (#![no_std])
│   └── src/
│       ├── arch/x86_64/   # Architecture-specific code
[... rest of diagram ...]
```

**Change:** rename the root from `altimitneo/` to `neoplutarch/`. **One line modified.** All other lines in the diagram (sub-directory names, comments) stay identical.

Do NOT modify anything else in Article 17. The article's prose, the diagram's structure, the surrounding context — all unchanged.

### 3.2 Footer version bump

Update the footer:

```
Document: NeoPlutarch Bible
Version: 1.6.0
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-11 — Article 17 source tree diagram root renamed `altimitneo/` → `neoplutarch/` to align with the project rename ratified in Article 25.1. Articles 25.1 and 25.2 historical references retained as institutional memory per PM disposition in `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`. Earlier amendments preserved from v1.5.0 footer history.
Authority: Project Lead
```

The "Earlier amendments preserved from v1.5.0 footer history" phrasing matches the existing v1.5.0 footer's "Earlier same-day amendments preserved from v1.4.0 footer history" pattern.

### 3.3 No other Bible changes

This dispatch touches:
- Article 17 source tree diagram root (one line).
- Footer (version + amendment summary).

This dispatch does NOT touch:
- Any other Article 17 content.
- Article 25.1, 25.2, or any other naming-conventions content.
- Article 8.5.4 (recently amended; no further changes).
- Any other article.
- The Bible filename (already `bible.md`, no rename needed).

## 4. Single commit

Title:

```
bible: Article 17 source tree diagram root altimitneo/ → neoplutarch/ (v1.6.0)
```

Body (substance; Prometheus may revise):

```
Bible amendment per Article 27.1.

Amends Article 17 source tree diagram root from `altimitneo/` to
`neoplutarch/` to align with the project rename ratified in Article 25.1
(2026-05-10 trademark-driven rename from AltimitNeo to NeoPlutarch). The
diagram documents the normative source layout; consistency with the
project name is the load-bearing concern.

Articles 25.1 and 25.2 references to "AltimitNeo" are RETAINED — those
references are intentional institutional memory documenting why the
project was renamed (Article 25.1 cites trademark-driven Bandai Namco
.hack franchise exposure as the rationale) and why the `neo-` subsystem
prefixes persist post-rename (Article 25.2 explicitly preserves them as
project-internal convention). Removing those references would lose the
rationale.

Version bump 1.5.0 → 1.6.0. Footer updated with amendment summary.

PM dispositions ratified after PM read the actual Article 25.1/25.2
context; see Plans/DISPATCH-2026-05-11-bible-amendment-article-17-
prometheus.md §1 for the full reasoning. PL approved the amendment
workstream on 2026-05-11 in chat.

See:
- Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md
- Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md
- docs/bible.md Article 27.1 (amendment process)
```

## 5. Verification gates

Doc-only constitutional amendment; no `cargo k` gates apply. Required checks:

1. Diff review: exactly two lines changed (Article 17 diagram root + footer version) plus the footer "Last amended" line. Spot-check: total non-whitespace line touches should be 2-4.
2. Article 17 diagram is intact except for the root line — no accidental changes to sub-directory entries.
3. Article 25.1 and 25.2 are UNCHANGED — verify by diff inspection.
4. Footer version is now 1.6.0; "Last amended" line includes the amendment summary; "Earlier amendments preserved" phrasing is present.
5. No other articles touched.

## 6. Push

Branch convention: `prometheus/bible-amendment-v1.6.0-2026-05-11`.

```bash
git push origin HEAD:main
```

Clean fast-forward expected. If rejected on `origin/main` advancement (e.g., RENAME-MECHANICAL execution dispatch pushed first), surface to PL — do NOT rebase without PM authorization (this is a constitutional amendment; rebase semantics require careful handling).

## 7. Verification report

File `Plans/BIBLE-AMENDMENT-V1.6.0-VERIFICATION-2026-05-11.md` documenting:

- Diff summary (file changed: `docs/bible.md` only; lines touched).
- Verification that Articles 25.1 and 25.2 are unchanged (quote both articles' current content as evidence).
- Footer version-bump confirmation.
- Commit SHA after push.

Format matches prior Prometheus verification reports.

## 8. Stop conditions

- More than 5 lines changed in the diff (excluding whitespace) → STOP, surface to PM. Amendment scope is intentionally narrow; anything beyond Article 17 root + footer suggests scope creep.
- Article 25.1 or 25.2 accidentally modified → STOP, revert those changes, surface to PM. PM disposition is explicit: those references stay.
- Footer version doesn't match v1.6.0 pattern → STOP, fix, retry verification.
- Push rejected → STOP, surface to PL.

## 9. Closure protocol

After commit + push + verification report:

1. Prometheus reports back to PL with: (a) commit SHA, (b) verification report filename, (c) confirmation that Articles 25.1 and 25.2 are untouched.
2. PL ferries to PM.
3. PM consumes; no follow-up PM-NOTE required unless something unexpected surfaced. The Bible is now at v1.6.0 with one new amendment.

If a future PL session re-evaluates the dispositions on Articles 25.1/25.2 and decides those references should also be trimmed/reworded, that becomes a separate amendment dispatch with its own version bump.

---

**End of dispatch.**

*Bible amendment v1.5.0 → v1.6.0. Article 17 source tree diagram root `altimitneo/` → `neoplutarch/`. Articles 25.1 and 25.2 unchanged (intentional institutional memory). Per Bible Article 27.1 governance.*
