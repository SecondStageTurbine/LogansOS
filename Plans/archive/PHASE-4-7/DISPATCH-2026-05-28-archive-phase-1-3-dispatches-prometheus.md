# DISPATCH: Prometheus — Archive Phase 1-3 Dispatch Artifacts (460+ files)

**Dispatch date:** 2026-05-28
**Target:** Prometheus (document cleanup lane)
**Deadline:** ASAP (single-commit archive operation)
**Authority:** PL operational (Option A selection)
**Status:** READY FOR EXECUTION

---

## Objective

Archive ~460+ Phase 1–3 closed dispatch artifacts, verification logs, PM notes, research returns, and handoff documents from Plans/ and docs/ root into their respective `Plans/archive/PHASE-N/` and `docs/archive/` subdirectories. This is the Phase 2 archival operation following Phase 1 cleanup (deletion of 3 obsolete specs + archive structure creation, committed 4620f02).

---

## Reference Documents

**Cleanup audit:** Plans/CLEANUP-AUDIT-2026-05-28-prometheus.md (your completed audit; use §3.1–§3.9 for detailed file lists per phase and category)

**Archive structure (already created):**
- `Plans/archive/PHASE-1-2/` — May 6–12 work (wedge investigation, SC-3B/C/D, rename, ADR-014/015/016)
- `Plans/archive/PHASE-3-BUILD-1-3/` — May 12–15 (BUILD-1 through BUILD-3)
- `Plans/archive/PHASE-3-BUILD-4/` — May 16–19 (stability campaign, stale-fsync, wake-token)
- `Plans/archive/PHASE-3-BUILD-5/` — May 20–21 (BUILD-4B/4C/4D/5a, VMware harness)
- `Plans/archive/PHASE-3-CLOSE/` — May 22–28 (bare-metal verification, formal close)
- `Plans/archive/PHASE-2-REOPEN/` — May 24–27 (Phase 2 revisit campaign)
- `Plans/archive/PHASE-4-THINK/` — May 20–22 (Phase 4 THINK campaign, closed)
- `Plans/archive/evidence/` — Phase 2 bare-metal evidence logs
- `docs/archive/` — docs-root historical content (e.g., folded lessons from HANDOFF.md)

---

## Process

### 1. Source: Audit File Lists

The cleanup audit (Plans/CLEANUP-AUDIT-2026-05-28-prometheus.md) contains detailed file inventories:

- **§3.2** — Phase 1-2 archive (May 6–12): 120+ files. Detailed list by sub-campaign.
- **§3.3** — Phase 3 BUILD-1–3 (May 12–15): 90+ files. Dispatch, verify, PM-DECISION, close reports.
- **§3.4** — Phase 3 BUILD-4 stability (May 16–19): 40+ files. stale-fsync, wake-token investigation.
- **§3.5** — Phase 3 BUILD-4B through BUILD-5a (May 20–21): 50+ files. BUILD-4B/4C/4D/5a, VMware harness.
- **§3.8** — Phase 2 re-open/close (May 24–27): 50+ files. Phase 2 revisit, F006, blocker handling.
- **§3.9** — Phase 3 bare-metal log-sink and close (May 22–28): 30+ files. Block-device log sink, final close.
- **§3.10** — Evidence directory (Phase 2): 48+ files. Serial logs, hexdumps from bare-metal runs.

**Total: ~460+ files**

### 2. Archive Operations

For each archive directory:

1. **Read the audit section** for the phase-specific file list.
2. **Check each file exists** in Plans/ (handle file-not-found gracefully).
3. **Move file to archive directory** (e.g., `DISPATCH-2026-05-09-sc-3b-*.md` → `Plans/archive/PHASE-1-2/DISPATCH-2026-05-09-sc-3b-*.md`).
4. **Verify move** (file exists in archive, no longer in Plans/ root).

### 3. INDEX.md Update

Plans/INDEX.md currently tracks ~200 dispatch artifacts. After archival:

- Remove archived files from the active section (they're now in Plans/archive/)
- Add an "ARCHIVED" section documenting the phase-by-phase breakdown: "Phase 1-2: 120 files archived to Plans/archive/PHASE-1-2/", etc.
- Update any forward-references to archived files (e.g., if INDEX.md mentions "DISPATCH-2026-05-09-sc-3b-*.md", note it's now in archive)

Example new INDEX.md entry:

```
## ARCHIVED DISPATCHES

The following phases' dispatches have been archived to Plans/archive/ per cleanup audit 2026-05-28:

- **Phase 1-2 (May 6–12):** 120 files → `Plans/archive/PHASE-1-2/`
- **Phase 3 BUILD-1–3 (May 12–15):** 90 files → `Plans/archive/PHASE-3-BUILD-1-3/`
- **Phase 3 BUILD-4 (May 16–19):** 40 files → `Plans/archive/PHASE-3-BUILD-4/`
- **Phase 3 BUILD-5 (May 20–21):** 50 files → `Plans/archive/PHASE-3-BUILD-5/`
- **Phase 3 CLOSE (May 22–28):** 30 files → `Plans/archive/PHASE-3-CLOSE/`
- **Phase 2 REOPEN (May 24–27):** 50 files → `Plans/archive/PHASE-2-REOPEN/`
- **Phase 4 THINK (May 20–22):** 8 files → `Plans/archive/PHASE-4-THINK/`
- **Evidence (Phase 2):** 48 files → `Plans/archive/evidence/`

Total archived: ~460+ files. Historical reference preserved; Plans/ root is now clean.
```

### 4. Single Commit to main

After all moves:

```bash
git add -A
git commit -m "refactor: archive phase 1-3 dispatches to plans/archive/"
git push origin main
```

Verify: All ~460+ files moved to archive/; Plans/ root contains only active Phase 4 work + INDEX.md.

---

## Guardrails

- **No deletions.** All files are moved (archived), not deleted. Git history is preserved.
- **Verify source audit.** Cross-check your file counts against the audit (§3 detailed listings).
- **Handle missing files gracefully.** If a file listed in the audit doesn't exist on disk (it may have been consolidated or deleted earlier), note it and continue.
- **Preserve git history.** The `git mv` or `cp` + `rm` + `git add` sequence preserves file history.

---

## Success Criteria

- [ ] All ~460+ files listed in audit §3.1–§3.9 are moved to their respective archive directories
- [ ] Plans/ root contains only active Phase 4 work (DISPATCH, RESEARCH, PM-NOTE, PM-DECISION for current work)
- [ ] Plans/INDEX.md is updated with archived-section summary
- [ ] Single commit lands on main with message "refactor: archive phase 1-3 dispatches..."
- [ ] `git log` confirms all files moved (not deleted)
- [ ] `ls Plans/archive/PHASE-*` shows populated directories

---

## Reference Counts (from audit)

| Phase | Files | Archive dir |
|-------|-------|-------------|
| Phase 1-2 | ~120 | PHASE-1-2/ |
| Phase 3 BUILD-1-3 | ~90 | PHASE-3-BUILD-1-3/ |
| Phase 3 BUILD-4 | ~40 | PHASE-3-BUILD-4/ |
| Phase 3 BUILD-5 | ~50 | PHASE-3-BUILD-5/ |
| Phase 3 CLOSE | ~30 | PHASE-3-CLOSE/ |
| Phase 2 REOPEN | ~50 | PHASE-2-REOPEN/ |
| Phase 4 THINK | ~8 | PHASE-4-THINK/ |
| Phase 2 EVIDENCE | ~48 | evidence/ |
| **TOTAL** | **~460+** | — |

---

**Dispatch end.**

Execute when ready. Single commit to main. Archive is preserving history, not destruction.