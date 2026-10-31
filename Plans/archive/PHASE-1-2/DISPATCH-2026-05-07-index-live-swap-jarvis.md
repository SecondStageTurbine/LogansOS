# Dispatch: live INDEX.md swap — post sub-commit 2 closure

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-07
**Type:** Doc-only commit. Replaces `Plans/INDEX.md` with the pre-drafted content from `Plans/INDEX-DRAFT-2026-05-07.md`, then commits to `main`.
**Substrate:** Sub-commit 2 closed at `0b5ffb4` (per `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md`). The pre-drafted INDEX is at `Plans/INDEX-DRAFT-2026-05-07.md` (40 entries, alphabetical, all 2026-05-06 14:00 onward + 2026-05-07 entries included). PM reviewed and added the three artifacts that landed after the pre-draft (PM-NOTE local-only-posture, INDEX-DRAFT itself, PM-DECISION ratification).

---

## 0. Standing instructions

1. The pre-draft is correct as-is. Do NOT rewrite, restructure, or re-evaluate status assignments. PM's review pass already happened.
2. Project is local-only per `Plans/PM-NOTE-2026-05-07-local-only-posture.md`. No push step.
3. After the swap and commit, the pre-draft file (`Plans/INDEX-DRAFT-2026-05-07.md`) gets deleted — it served its purpose. The live `Plans/INDEX.md` is now the source of truth.
4. If `git status` shows anything unexpected before commit, STOP and surface to PM.

---

## 1. Pre-flight

```powershell
cd "E:\AI Automation\AltimitNeo"

# Confirm sub-commit 2 is on HEAD.
git log --oneline -2
# Expected: 0b5ffb4 sched: Phase 2 BUILD-3 sub-commit 2 ...
#           9505619 chore: gitignore ...

# Confirm working tree is clean (only untracked Plans/ artifacts).
git status --short
# Expected: only `??` entries under Plans/. No `M`, no `A`, no `D`.

# Confirm both files exist.
Test-Path "Plans/INDEX.md"            # expected: True
Test-Path "Plans/INDEX-DRAFT-2026-05-07.md"  # expected: True
```

If any of the above doesn't match, STOP and surface to PM.

---

## 2. The swap

```powershell
# Replace INDEX.md content with INDEX-DRAFT content.
Copy-Item "Plans/INDEX-DRAFT-2026-05-07.md" "Plans/INDEX.md" -Force

# Delete the pre-draft — it served its purpose.
Remove-Item "Plans/INDEX-DRAFT-2026-05-07.md"

# Verify the swap landed.
Get-Content "Plans/INDEX.md" | Select-Object -First 5
# Expected: starts with "# Plans Directory Index"
(Get-Content "Plans/INDEX.md").Count
# Expected: ~75 lines (the new INDEX is bigger than the previous one)
```

---

## 3. Stage and commit

```powershell
# Stage only INDEX.md. Do not stage anything else in Plans/.
git add Plans/INDEX.md

# Verify the stage. The pre-draft deletion is OK — Plans/INDEX-DRAFT-2026-05-07.md
# was untracked, so its deletion isn't tracked by git either way.
git diff --staged --stat
# Expected: 1 file changed, large insertion count, modest deletion count
# (the new INDEX is ~3x the old one).

# Commit.
$msg = @"
docs: Plans/INDEX.md — sweep through sub-commit 2 closure

Update INDEX.md to reflect all artifacts created or status-changed since
the previous INDEX update on 2026-05-06 morning. Specifically:

- Adds 16 new entries covering: EXP-F, EXP-G, gitignore cleanup,
  bisect-step1, both 2026-05-06 closure attempts, the KASLR diagnostic
  chain (Q1+Q2, Q1B), ADR-015 chain (draft, finalize dispatch, FINAL,
  ratification), Q9 redo chain (initial dispatch, revisions dispatch,
  notes), SC-3A pre-thinking, and the 2026-05-07 PM decisions/notes
  (local-only posture, KASLR-Q1B escalation, ADR-015 ratification).

- Flips 2 entries from active -> closed: EXP-E and the previous INDEX
  update dispatch.

- Marks 1 new superseded: the 2026-05-06 first sub-commit 2 closure
  attempt, blocked by KASLR overflow and superseded by the 2026-05-07
  corrected closure dispatch after KASLR-Q1B confirmed pre-existence.

Structure preserved: single alphabetical table (Option alpha per the
pre-draft dispatch). 40 entries total.

Refs: 0b5ffb4 (sub-commit 2 closure that this sweep follows from),
Plans/DISPATCH-2026-05-07-index-presweep-jarvis.md (the pre-draft
dispatch).
"@
$msg | Out-File -FilePath ".git/COMMIT_EDITMSG_INDEX" -Encoding utf8 -NoNewline
git commit -F ".git/COMMIT_EDITMSG_INDEX"
Remove-Item ".git/COMMIT_EDITMSG_INDEX"

# Verify the commit landed.
git log --oneline -3
# Expected:
# <new hash> docs: Plans/INDEX.md — sweep through sub-commit 2 closure
# 0b5ffb4 sched: Phase 2 BUILD-3 sub-commit 2 ...
# 9505619 chore: gitignore ...
```

---

## 4. Close

```powershell
# Verify final state.
git status --short
# Expected: only `??` entries under Plans/ (the various working artifacts).

git log --oneline -3
# Expected as above.
```

Ping PM with the new commit hash. No formal close report needed — this is a single-file doc commit and the commit message itself is the record.

---

## 5. What you do NOT do

- Do not modify the INDEX content during the swap. PM reviewed it; ship as-is.
- Do not stage anything other than `Plans/INDEX.md`.
- Do not commit any of the other untracked Plans/ artifacts. They stay untracked until their own work commits.
- Do not attempt to set up a remote, configure `origin`, or push. Project is local-only.
- Do not re-update INDEX.md after the ADR-015 implementation commit — that gets its own INDEX update via a follow-on dispatch.

---

## 6. Time budget

- 2 min: pre-flight check.
- 3 min: swap + stage + commit.
- 1 min: verify final state + ping PM.

**Total: ~6 min.** Smallest dispatch this week.

---

## 7. Why this matters

INDEX.md being current is the difference between "future PM can find artifacts in 30 seconds" and "future PM has to grep `Plans/` and reconstruct context." Two days of activity have accumulated since the last INDEX update; locking in the current state now means the next session starts with accurate navigation.

---

**End of dispatch.**
