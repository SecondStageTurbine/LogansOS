# AUDIT — AltimitNeo to NeoPlutarch Rename Stragglers

**Date:** 2026-05-11
**Author:** Prometheus
**Type:** Audit artifact, doc-only. No renames executed in this dispatch.
**Status:** DRAFT pending PM review of dispositions and PL ratification of PL-tier items.

---

## 1. Summary

| Metric | Count |
|--------|-------|
| Tracked files with surviving references | 10 |
| Total reference occurrences | 57 |
| RENAME-MECHANICAL | 36 |
| RENAME-CONTEXTUAL | 5 |
| INTENTIONAL-HISTORICAL | 11 |
| PL-TIER-DECISION | 4 |
| AMBIGUOUS | 1 |

---

## 2. Per-File Audit

### 2.1 `CLAUDE.md` (6 references)

Project guide for Claude Code sessions. References are to the project name, RAG infrastructure paths, or collection names.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 5 | `## AltimitNeo RAG — Semantic Code Search` | RENAME-MECHANICAL | `## NeoPlutarch RAG — Semantic Code Search` | Section header identifies the project RAG infrastructure. Project-name reference in an operational guide. |
| 7 | Path `E:\AI Automation\RAG\AltimitNeo` | RENAME-CONTEXTUAL | Depends on physical directory rename | File path to the RAG directory. If the disk directory was renamed, update. If not, leave as factual path. PM must verify. |
| 15 | Collection `altimitneo-code` | RENAME-MECHANICAL | Collection `neoplutarch-code` | Qdrant collection name is an operational identifier matching project name. Requires reindexing if changed. |
| 23 | Path `E:\AI Automation\RAG\AltimitNeo` | RENAME-CONTEXTUAL | Same as line 7 | Physical path reference. Same disposition as line 7. |
| 91 | Path `E:\AI Automation\RAG\AltimitNeo` | RENAME-CONTEXTUAL | Same as line 7 | Physical path reference. Same disposition as lines 7, 23. |
| 96 | Path `E:\AI Automation\AltimitNeo` | RENAME-CONTEXTUAL | Same as line 7 | Physical workspace root path. Same disposition as lines 7, 23, 91. |

**Note:** Lines 7, 23, 91, 96 all reference physical filesystem paths. The repository root directory name is a filesystem concern separate from the project name. PM must verify whether `E:\AI Automation\AltimitNeo` was renamed on disk.

### 2.2 `README.md` (3 references)

Project front-page document. Two cleanup commits already landed (`c35386c`, `d60f416`). Remaining refs are historical rename narrative.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 11 | `The project was originally named "AltimitNeo" through Phase 2...` | INTENTIONAL-HISTORICAL | No change | Rename narrative explaining what the project was called before. Old name appears as historical fact, not current branding. Changing it would erase the rename record. |
| 19 | `Project renamed AltimitNeo to NeoPlutarch.` | INTENTIONAL-HISTORICAL | No change | Changelog entry documenting the rename event. Old name is part of the historical record. |
| 120 | `...may still reference "AltimitNeo" until the kernel-side rename...` | INTENTIONAL-HISTORICAL | No change | Status caveat referencing the rename event. Note: this caveat may now be stale since the kernel-rename bundle (`27d1b99`) has landed — PM should consider updating, but that is outside this audit scope. |

### 2.3 `docs/bible.md` (4 references)

Constitutional document. All references are PL-TIER-DECISION per dispatch section B.5.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 240 | `altimitneo/` (Article 17 source tree diagram root) | PL-TIER-DECISION | N/A — PL decides | Source layout diagram uses `altimitneo/` as root. Renaming requires amending Article 17 per Bible 27.1. Governance question: is the diagram normative (current state) or historical (Phase 0 record)? If normative, update to `neoplutarch/`. If historical, stays. |
| 307 | `The project was originally named "AltimitNeo"...` (Article 25.1) | PL-TIER-DECISION | N/A — PL decides | Bible naming clause records the rename. Old name is historical fact within the naming article. PL decides whether to keep, trim, or reword. Requires Bible amendment per 27.1 if changed. |
| 308 | `...rename from "AltimitNeo" to "NeoPlutarch"...` (Article 25.2) | PL-TIER-DECISION | N/A — PL decides | Same as 307 — historical reference in subsystem prefix article. PL decides retain/trim/reword. Requires Bible amendment per 27.1 if changed. |
| N/A | Bible filename is `bible.md` | PL-TIER-DECISION (non-issue) | No action needed | Filename does not contain "AltimitNeo". The filename question from dispatch section B.5 is moot — the file is already `bible.md`. |

### 2.4 `docs/design.md` (1 reference)

Design document, v0.4 identity update already landed.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 312 | `v0.4 — Identity update for the 2026-05-10 project rename (AltimitNeo to NeoPlutarch)...` | INTENTIONAL-HISTORICAL | No change | Version history note documenting what changed in v0.4. Old name is part of the rename narrative, not current branding. Same pattern as README lines 11 and 19. |

### 2.5 `docs/HANDOFF.md` (2 references)

Operational handoff document. References are to Docker environment names and container names.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 152 | `altimit-neo-dev` Docker environment | RENAME-MECHANICAL | `neoplutarch-dev` | Docker service name is an operational identifier in a project guide. Should match current naming. Requires coordinated `docker-compose.yml` update (OFF-LIMITS per dispatch constraints; PM must handle or authorize exception). |
| 168 | Orphan container `altimitneo-qdrant` | AMBIGUOUS | N/A — surface to PM | Refers to a Qdrant container from "another project" (the RAG infrastructure). Container name belongs to the RAG setup, not this repo. If the RAG was renamed, update. If not, leave as-is. PM must verify Qdrant container state. |

### 2.6 `docs/ADR-002-source-layout-migration.md` (2 references)

ADR documenting the source-layout migration under the original project name.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 20 | `altimitneo/` (pre-migration layout diagram) | INTENTIONAL-HISTORICAL | No change | ADR-002 records the source-layout migration under the original name. Directory name in the tree diagram is a historical artifact — what the layout looked like at decision time. Renaming would falsify the record. |
| 111 | `altimitneo/` (post-migration layout diagram) | INTENTIONAL-HISTORICAL | No change | Same as line 20 — ADR-002 record of post-migration layout at time of writing. Historical artifact, not a current identifier. |

### 2.7 `docs/research/ALTIMIT-real-world-implementation.md` (36 references)

Research report on the .hack franchise fictional ALTIMIT OS and its relationship to this project. Two distinct reference types:

**Type A — Fictional ALTIMIT OS references (all caps "ALTIMIT").** INTENTIONAL-HISTORICAL. Must NOT be renamed. Not counted in the 36 below.

**Type B — Project name "AltimitNeo" references.** Should become "NeoPlutarch."

| Line | Context snippet | Classification | Proposed replacement | Reasoning |
|------|----------------|---------------|---------------------|-----------|
| 1 | Title: `...Bringing the .hack OS Vision into AltimitNeo` | RENAME-MECHANICAL | `...into NeoPlutarch` | Project name in title. "ALTIMIT" portion (fictional OS) stays. |
| 9 | Scope: `...invention-frontier work for AltimitNeo` | RENAME-MECHANICAL | `...for NeoPlutarch` | Project name in scope table cell. |
| 16 | `AltimitNeo must commit to four non-negotiables` | RENAME-MECHANICAL | `NeoPlutarch must commit...` | Project name in architectural recommendation. "ALTIMIT" in same paragraph = fictional OS, stays. |
| 17 | `AltimitNeo's job is to be the substrate` | RENAME-MECHANICAL | `NeoPlutarch's job...` | Project name. Distinguishable from "ALTIMIT" (fictional) by capitalization and context. |
| 44 | `The taxonomy that matters for AltimitNeo` | RENAME-MECHANICAL | `...for NeoPlutarch` | Project name in primitives discussion. |
| 50 | `The implication for AltimitNeo is direct` | RENAME-MECHANICAL | `...for NeoPlutarch` | Project name in architectural prescription. |
| 62 | `For AltimitNeo, the implication is that` | RENAME-MECHANICAL | `For NeoPlutarch` | Project name in hardware assumptions discussion. |
| 81 | Section header: `AltimitNeo's stated direction` | RENAME-MECHANICAL | `NeoPlutarch's stated direction` | Project name in header. "ALTIMIT posture" = fictional OS, stays. |
| 83 | Table column: `AltimitNeo current direction` | RENAME-MECHANICAL | `NeoPlutarch current direction` | Project name in table column header. |
| 102 | `AltimitNeo's posture (kernel arbitrates AI...)` | RENAME-MECHANICAL | `NeoPlutarch's posture...` | Project name describing design posture. |
| 105 | `AltimitNeo would want TPP-style primitives` | RENAME-MECHANICAL | `NeoPlutarch would want...` | Project name in implementation recommendation. |
| 106 | `granularity for AltimitNeo's inference scheduler` | RENAME-MECHANICAL | `...for NeoPlutarch's inference scheduler` | Project name. |
| 108 | `Recommended primitive set for AltimitNeo's kernel` | RENAME-MECHANICAL | `...for NeoPlutarch's kernel` | Project name. |
| 124 | `For AltimitNeo this implies a kernel design` | RENAME-MECHANICAL | `For NeoPlutarch...` | Project name. |
| 141 | `For AltimitNeo, the right architectural commitment` | RENAME-MECHANICAL | `For NeoPlutarch...` | Project name. |
| 149 | `(an option AltimitNeo explicitly rejects)` | RENAME-MECHANICAL | `(an option NeoPlutarch explicitly rejects)` | Project name describing what the project rejects. |
| 151 | `Recommendation for AltimitNeo's monolith design` | RENAME-MECHANICAL | `...for NeoPlutarch's monolith design` | Project name. |
| 155 | `verification at AltimitNeo's planned scope` | RENAME-MECHANICAL | `...at NeoPlutarch's planned scope` | Project name. |
| 161 | `This is where AltimitNeo can do something genuinely novel` | RENAME-MECHANICAL | `...where NeoPlutarch can do...` | Project name. |
| 171 | `For AltimitNeo:` | RENAME-MECHANICAL | `For NeoPlutarch:` | Project name. |
| 181 | `The genuinely hard question for AltimitNeo` | RENAME-MECHANICAL | `...for NeoPlutarch` | Project name. |
| 185 | `Recommendation: AltimitNeo should be a single-node OS` | RENAME-MECHANICAL | `...NeoPlutarch should be...` | Project name. |
| 187 | `"AltimitNeo is single-node by design..."` | RENAME-MECHANICAL | `"NeoPlutarch is single-node by design..."` | Project name in proposed Bible amendment text. |
| 193 | `For AltimitNeo: a Surface capability` | RENAME-MECHANICAL | `For NeoPlutarch:` | Project name. |
| 203 | `AltimitNeo is correct to refuse it` | RENAME-MECHANICAL | `NeoPlutarch is correct to refuse it` | Project name. |
| 214 | `AltimitNeo should expose a public... kernel-identity primitive` | RENAME-MECHANICAL | `NeoPlutarch should expose...` | Project name. |
| 218 | `AltimitNeo should ignore this thread entirely` | RENAME-MECHANICAL | `NeoPlutarch should ignore...` | Project name. |
| 220 | `AltimitNeo could expose a ConfidentialInference capability` | RENAME-MECHANICAL | `NeoPlutarch could expose...` | Project name. |
| 230 | `Even though AltimitNeo is a monolith` | RENAME-MECHANICAL | `Even though NeoPlutarch is a monolith` | Project name. |
| 256 | `AltimitNeo adopts cell-swapping fault recovery` | RENAME-MECHANICAL | `NeoPlutarch adopts...` | Project name in proposed Bible amendment text. |
| 257 | `AltimitNeo is single-node by design` | RENAME-MECHANICAL | `NeoPlutarch is single-node by design` | Project name in proposed Bible amendment text. |
| 271 | `AltimitNeo rejecting it means re-inventing tooling` | RENAME-MECHANICAL | `NeoPlutarch rejecting it...` | Project name. |
| 272 | `AltimitNeo would, if executed against this roadmap` | RENAME-MECHANICAL | `NeoPlutarch would, if executed...` | Project name. |
| 273 | `test that AltimitNeo would survive what every other OS would not` | RENAME-MECHANICAL | `...that NeoPlutarch would survive...` | Project name. |
| 274 | `The name "AltimitNeo" inherits both the aspiration and that warning` | INTENTIONAL-HISTORICAL | No change | Meta-discussion about the old name itself — what it means, what warnings it carries. Changing it would break the sentence meaning. |
| 285 | `Article on scope: AltimitNeo is single-node by design` | RENAME-MECHANICAL | `...NeoPlutarch is single-node by design` | Project name in proposed Bible amendment text. |

**File summary:** 36 total references. 34 RENAME-MECHANICAL. 1 INTENTIONAL-HISTORICAL (line 274). The all-caps "ALTIMIT" references throughout the file refer to the fictional .hack OS and are not counted — they are correct as-is.

**Filename note:** `ALTIMIT-real-world-implementation.md` uses "ALTIMIT" (fictional OS), not the project name. Filename is correct, no rename needed.

### 2.8 `Plans/INDEX.md` (1 reference)

Project plans directory index.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 59 | Backup path `Z:\AltimitNeo\<timestamp>\` on Synology NAS | AMBIGUOUS | N/A — surface to PM | Physical backup directory path. If the NAS directory was renamed with the project, update. If still `AltimitNeo`, leave as factual path. PM must verify `Z:\AltimitNeo\` state. |

### 2.9 `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` (1 reference)

Verification report for the kernel-rename bundle sub-commit 1.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 50 | `altimit-neo-kernel` was removed and `neoplutarch-kernel` was added | INTENTIONAL-HISTORICAL | No change | Verification report documenting THE rename event. Reference describes what was removed during the rename. Changing it would falsify the record. |

### 2.10 `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` (1 reference)

Research note on Redox DWRR scheduling prior art.

| Line | Context | Classification | Proposed replacement | Reasoning |
|------|---------|---------------|---------------------|-----------|
| 82 | `docs/specs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) | RENAME-MECHANICAL | `docs/specs/neoplutarch-ai-subsystem-spec.md` | File path reference to ANSPEC-001. The file was renamed in the previous Prometheus dispatch (commit `b6764be`). This cross-reference is stale — points to a filename that no longer exists. |

---

## 3. PL-Tier Items

Four references in `docs/bible.md` require PL-tier governance decisions:

| # | Line | What | Governance question |
|---|------|------|---------------------|
| 1 | 240 | Article 17 source tree diagram root `altimitneo/` | Is the diagram normative (current state) or historical (Phase 0 record)? If normative, amend to `neoplutarch/`. Requires Bible amendment per Article 27.1. |
| 2 | 307 | Article 25.1: `"AltimitNeo"` in rename narrative | PL decides whether to keep the full rename narrative, trim it, or reword. The article establishes current naming conventions; historical detail may be more than needed. Requires Bible amendment per 27.1 if changed. |
| 3 | 308 | Article 25.2: `"AltimitNeo"` in subsystem prefix clause | Same as #2 — PL decides retain/trim/reword for historical reference in the subsystem prefix article. Requires Bible amendment per 27.1 if changed. |
| 4 | N/A | Bible filename question | Non-issue: Bible is `bible.md`, not `TheAltimitNeoBible.md`. No action needed. |

---

## 4. Recommended Follow-up Dispatches

### Dispatch 1 — RENAME-MECHANICAL execution (Prometheus)

After PM ratifies dispositions, one dispatch to execute all RENAME-MECHANICAL changes:

- `CLAUDE.md` line 5: section header rename
- `CLAUDE.md` line 15: Qdrant collection name rename (if PM decides to reindex; otherwise defer)
- `docs/research/ALTIMIT-real-world-implementation.md`: 34 project-name references to "NeoPlutarch" (all lines enumerated in section 2.7)
- `docs/HANDOFF.md` line 152: Docker service name (requires coordinated `docker-compose.yml` update — OFF-LIMITS; PM must handle separately or authorize exception)
- `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` line 82: stale ANSPEC filename reference

**Estimated scope:** ~37 mechanical replacements across 4-5 files. One commit.

### Dispatch 2 — RENAME-CONTEXTUAL execution (PM decision required first)

Pending PM verification of physical paths:

- `CLAUDE.md` lines 7, 23, 91, 96: depend on whether `E:\AI Automation\AltimitNeo` directory was renamed on disk
- `docs/HANDOFF.md` line 168: depends on whether Qdrant container was renamed

**Estimated scope:** 0-5 changes depending on PM path verification.

### Dispatch 3 — PL-TIER Bible amendment (deferred until PL ratifies)

Pending PL governance decision on `docs/bible.md` Article 17 and Article 25:

- Article 17 source tree diagram: rename or keep
- Article 25.1/25.2 historical references: retain, trim, or reword

**Estimated scope:** Bible amendment PR per Article 27.1. Version bump required.

---

## 5. Out-of-Scope Notes

The following were explicitly NOT audited:

- **Commit messages in `git log`:** Historical commits reference "AltimitNeo" extensively. Immutable once pushed; not in scope.
- **ELF binary strings in built artifacts:** Compiled binaries under `target/` may contain pre-rename strings. Not tracked files; not in scope.
- **`.git/` internal state:** Git internals are not auditable text; not in scope.
- **RAG index content:** Qdrant collection contains embedded chunks with "AltimitNeo" references. Reindexing would update, but RAG is infrastructure, not a tracked file. Noted in CLAUDE.md line 15 disposition.
- **Synology NAS backup contents:** `Z:\AltimitNeo\` backups contain pre-rename snapshots. Not in scope.
- **Untracked Plans/ artifacts with "AltimitNeo" in filenames:** Per Bible Article 25.1, "pre-rename project artifacts in Plans/ retain their original naming as historical record." Not in scope for renaming.

---

## 6. Cross-Reference Integrity Self-Check

| Cited artifact | Exists? | Notes |
|----------------|---------|-------|
| `docs/bible.md` Article 27.1 | Yes | Line 312+, amendment process governance |
| `docs/bible.md` Article 25.1 | Yes | Line 307, naming clause |
| `docs/specs/neoplutarch-ai-subsystem-spec.md` | Yes | Renamed from `altimitneo-ai-subsystem-spec.md` at commit `b6764be` |
| `docs/research/ALTIMIT-real-world-implementation.md` | Yes | 314 lines, research artifact |
| `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` | Yes | Verification report for rename sub-commit 1 |
| `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` | Yes | Committed at `585bdfb` |

All cross-references verified.

---

**End of audit.**

*Deliverable B: 10 tracked files audited, 57 total references classified. 36 RENAME-MECHANICAL (mostly in ALTIMIT-real-world-implementation.md), 5 RENAME-CONTEXTUAL (physical paths pending PM verification), 11 INTENTIONAL-HISTORICAL (rename narratives, ADR-002 historical diagrams, rename event records), 4 PL-TIER-DECISION (Bible Articles 17 and 25), 1 AMBIGUOUS (NAS backup path). No renames executed. Three recommended follow-up dispatches: mechanical execution, contextual pending PM verification, PL-tier Bible amendment.*
