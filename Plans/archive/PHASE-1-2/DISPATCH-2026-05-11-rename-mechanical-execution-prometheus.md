# DISPATCH — AltimitNeo→NeoPlutarch RENAME-MECHANICAL execution (Prometheus)

**Date filed:** 2026-05-11
**From:** PM
**To:** Prometheus (Qwen3.6-27B, docs + cleanup + small-coding-trial lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.2)
**Ferry:** PL ferries to Prometheus session.
**Status:** Follow-up dispatch executing the RENAME-MECHANICAL items ratified in `Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md` §3.5. Single commit. Parallel-with-Jarvis-safe (no `kernel/src/*` overlap; Jarvis remains on SC-3C BUILD per `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md`).

---

## 1. Mandate

Execute the 35 strictly-text RENAME-MECHANICAL items + 1 README caveat cleanup enumerated in `Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md`. Single commit. Push to `origin/main` via clean fast-forward.

**No new audit work.** The audit artifact is the canonical enumeration; execute exactly that list (minus the deferred items per PM consumption note §3.5).

## 2. Inputs to consume

1. **`Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md`** — the canonical line-by-line enumeration. Sections §2.1 (CLAUDE.md), §2.7 (research artifact), §2.10 (redox-dwrr PM-NOTE) provide the per-line specifications.
2. **`Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md`** — PM ratification with deferrals enumerated.

## 3. Scope: 35 mechanical renames + 1 README cleanup

### 3.1 `docs/research/ALTIMIT-real-world-implementation.md` (34 renames)

Apply RENAME-MECHANICAL to all references enumerated in audit §2.7. Bare "ALTIMIT" references (fictional .hack OS) stay untouched — the audit's Type A vs Type B distinction is the load-bearing classification. Audit §2.7 enumerates the 34 RENAME-MECHANICAL lines with proposed replacements; apply exactly as specified.

Line 274 ("The name 'AltimitNeo' inherits both the aspiration and that warning") stays as INTENTIONAL-HISTORICAL per audit classification.

Filename `ALTIMIT-real-world-implementation.md` stays — the filename references the fictional ALTIMIT OS, not the project.

### 3.2 `CLAUDE.md` (1 rename + deferrals)

**Execute:**
- Line 5: section header `## AltimitNeo RAG — Semantic Code Search` → `## NeoPlutarch RAG — Semantic Code Search`.

**Defer (do NOT execute):**
- Line 7, 23, 91, 96: filesystem path references — STAY AS-IS per PM consumption note §3.2 (disk directory not renamed).
- Line 15: Qdrant collection `altimitneo-code` — STAY AS-IS per PM consumption note §3.5 (requires Qdrant reindex; operational decision deferred).

### 3.3 `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` (1 rename)

Line 82: stale ANSPEC filename reference `docs/specs/altimitneo-ai-subsystem-spec.md` → `docs/specs/neoplutarch-ai-subsystem-spec.md`. This is a cross-reference repair pointing at the actual current filename (the file was renamed at commit `b6764be`).

### 3.4 `README.md` (1 cleanup)

Per consumption note §3.6, line 120 caveat is now stale post-rename (`...may still reference 'AltimitNeo' until the kernel-side rename...`). The kernel-rename bundle closed at `27d1b99`. Update or remove the caveat to reflect current state.

Recommended text: replace the stale caveat with a brief footnote-style acknowledgment such as `*Pre-rename project artifacts in Plans/ retain their original "AltimitNeo" naming as historical record per Bible Article 25.1.*` or simply remove the caveat if it adds no current information.

Prometheus exercises discretion on exact phrasing; surface to PM if uncertain. The substantive requirement is that line 120 no longer suggests the rename is in-progress when it has completed.

### 3.5 Deferrals (no action)

- `docs/HANDOFF.md` line 152 Docker service name: STAY AS-IS per PM consumption note §3.5 (requires coordinated `docker-compose.yml` update, off-limits).
- `docs/HANDOFF.md` line 168 Qdrant container `altimitneo-qdrant`: STAY AS-IS (operational decision deferred).
- `Plans/INDEX.md` line 59 NAS backup path: STAY AS-IS per PM consumption note §3.3 (defer to infrastructure decision).
- `docs/bible.md` line 240: separate Bible amendment dispatch handles this (`Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`).
- `docs/bible.md` lines 307, 308: STAY AS-IS — intentional historical references in the rename narrative (Article 25.1 and 25.2 explicitly reference the rename event; this is institutional memory, not stragglers).
- `docs/ADR-002-source-layout-migration.md` lines 20, 111: STAY AS-IS — INTENTIONAL-HISTORICAL per audit §2.6 (ADR records historical layout state at decision time).
- `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` line 50: STAY AS-IS — INTENTIONAL-HISTORICAL (verification report documenting THE rename event).

## 4. Single commit

Title:

```
docs: complete AltimitNeo→NeoPlutarch rename stragglers per audit
```

Body (substance; Prometheus may revise):

```
Executes the RENAME-MECHANICAL items ratified in
Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md
based on the per-reference audit in
Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md.

Changes:
- docs/research/ALTIMIT-real-world-implementation.md: 34 project-name
  references to "NeoPlutarch". Bare "ALTIMIT" (fictional .hack OS) stays.
  Line 274 INTENTIONAL-HISTORICAL meta-discussion stays.
- CLAUDE.md line 5: RAG section header.
- Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md line 82: stale ANSPEC
  filename cross-reference repaired.
- README.md line 120: stale caveat updated to reflect post-rename state.

Deferred (not in this commit):
- CLAUDE.md filesystem paths and Qdrant collection name (require disk /
  Qdrant infrastructure decisions).
- HANDOFF.md Docker service and container references (require coordinated
  docker-compose.yml update).
- Plans/INDEX.md NAS backup path (infrastructure decision).
- docs/bible.md Article 17 source tree diagram (separate amendment
  dispatch per Bible Article 27.1).
- docs/bible.md Article 25.1/25.2 historical references in rename
  narrative (PM verdict: intentional historical, stays).
- ADR-002 historical layout diagrams (INTENTIONAL-HISTORICAL).
- Kernel-rename verification report (INTENTIONAL-HISTORICAL).

See:
- Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md
- Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md
```

## 5. Verification gates

Doc-only work; no `cargo k` gates apply. Required checks:

1. Diff review: total lines changed should match the audit enumeration (34 + 1 + 1 + 1 = 37 line touches, possibly more if README cleanup involves multiple lines).
2. Spot-check three random RENAME-MECHANICAL items in the research artifact diff against audit §2.7 line numbers and proposed replacements; verify match.
3. Bare "ALTIMIT" references in research artifact untouched. Quick spot-check: search for "ALTIMIT" (all caps, no "Neo") in the diff; count should be 0 changes on that pattern.
4. No `kernel/src/*` touches in diff. No `Cargo.toml`/`Cargo.lock` touches.
5. No file renames; only content changes.

## 6. Push

After commit:

```bash
git push origin HEAD:main
```

Should be a clean fast-forward — Jarvis is on a separate branch (`jarvis/sc-3c-build-2026-05-11`) and has not pushed any SC-3C commits to `origin/main` yet (verify with `git fetch origin && git rev-parse origin/main` immediately before push). If rejected, surface to PL.

**Branch convention:** `prometheus/rename-mechanical-execution-2026-05-11`. Push from branch tip directly to `origin/main` (same pattern as prior Prometheus dispatches).

## 7. Verification report

File `Plans/RENAME-MECHANICAL-EXECUTION-VERIFICATION-2026-05-11.md` documenting:

- Diff summary (files changed, total lines touched).
- Spot-check results (three random research-artifact lines verified against audit).
- Bare "ALTIMIT" preservation check.
- Commit SHA after push.

Format matches prior Prometheus verification reports.

## 8. Stop conditions

- Any deferred item accidentally modified → STOP, revert, surface to PM.
- Bare "ALTIMIT" references modified in the research artifact → STOP, revert that file's changes, surface to PM. The Type A vs Type B distinction is load-bearing.
- Push rejected on `origin/main` advancement → STOP, surface to PL (do NOT rebase without PM authorization given parallel-with-Jarvis dynamics).

## 9. Closure protocol

After commit + push + verification report:

1. Prometheus reports back to PL with: (a) commit SHA, (b) verification report filename, (c) diff summary, (d) any incidental observations.
2. PL ferries to PM.
3. PM consumes; no follow-up PM-NOTE required unless something unexpected surfaced. The Bible amendment and SC-3C closure integration dispatches proceed independently.

---

**End of dispatch.**

*35 RENAME-MECHANICAL + 1 README cleanup. Single commit. Parallel-with-Jarvis-safe. Audit enumeration is canonical; do not re-audit. Deferred items explicitly listed; do not touch.*
