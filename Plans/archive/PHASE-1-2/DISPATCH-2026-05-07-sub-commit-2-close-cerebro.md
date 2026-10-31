# Dispatch: sub-commit 2 closure — corrected

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-07
**Type:** Closure dispatch. Compose commit message, stage, commit. End of dispatch. (No push step — the project is local-only per project-lead clarification 2026-05-07; see `Plans/PM-NOTE-2026-05-07-local-only-posture.md`.)
**Substrate:** KASLR-Q1B confirmed pre-existence (4/50 panics on clean main, plus 0/100 on EXP-H sub-commit 2 tree). Per `Plans/PM-DECISION-2026-05-07-kaslr-q1-higher-n.md`'s pre-committed decision tree, branch "Q1 panic reproduces ≥ 1 time in 50 runs on main HEAD" is now active: pre-existence confirmed, closure dispatch issued. KASLR overflow is a separate triage item (ADR-015 draft already in flight at `Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md`).

---

## 0. Standing instructions

1. The working tree is exactly the EXP-H tree. EXP-H produced 100/100 PASS on `ipc_heap_kill_smoke` at `NEO_SMP=4`. That run is the bisection gate per ADR-013 §Q5/§Q9 amendments. **The gate is met.** Do NOT re-run the 100x EXP-H verification — re-running is closure-gate overspecification and we've explicitly learned not to do that.
2. **No source changes.** This is a commit-only dispatch. The working tree is what gets committed verbatim.
3. **Don't drop either stash yet.** Both stashes are safety nets until the commit lands on `main`. Drop them in §4 (cleanup) only after `git log --oneline -3` shows the new commit on `main`.
4. If anything is unexpected — `git status` shows files you didn't expect, the commit message gets rejected by hooks — STOP and surface to PM. The commit is the load-bearing event; getting it wrong here costs time we don't need to spend. **(Note: an empty `git remote -v` is expected and is NOT a blocker. The project operates local-only; do not surface remote-related findings as blockers.)**

---

## 1. Pre-flight check

```powershell
cd "E:\AI Automation\AltimitNeo"

# Confirm working tree state matches EXP-H expectation.
git diff --stat HEAD
# Expected: 11 files changed, 366 insertions(+), 112 deletions(-)
# Files: docs/ADR-013-phase-2-smp-retrofit.md, kernel/Cargo.toml,
#        kernel/src/arch/x86_64/mod.rs, kernel/src/arch/x86_64/smp.rs,
#        kernel/src/arch/x86_64/syscall.rs, kernel/src/main.rs,
#        kernel/src/sched/mod.rs, kernel/src/sched/scheduler.rs,
#        kernel/src/sched/thread.rs, kernel/src/test_harness.rs,
#        tools/xtask/src/main.rs

# Confirm HEAD and stashes.
git rev-parse HEAD                  # expected: 9505619... (or full hash)
git stash list                      # expected: both stashes still present

# Confirm we're on main.
git branch --show-current           # expected: main

# Confirm no untracked tracked-file-shaped surprises (Plans/ untracked is fine).
git status --short
# Expected: only `??` entries under Plans/ — all 7-8 of today's artifacts.
# No `M`, no `A`, no `D` outside the 11 expected modifications.
```

If any of the above doesn't match, **STOP** and surface to PM. Do not proceed.

---

## 2. Compose the commit message

Use the message below verbatim, with the body wrapped at ~72 columns. The summary line is intentionally concise; the body carries the detail.

```
sched: Phase 2 BUILD-3 sub-commit 2 — per-CPU schedulers + §Q5 restore + AP GS-base rewire; §Q9 deferred

Phase 2 BUILD-3 sub-commit 2. Three independent components plus one
restoration:

* §Q3 — per-CPU schedulers. Replaces the single global `SCHED` Mutex
  with per-CPU scheduler containers (`PerCpuScheduler`) accessed via
  `current_scheduler()`. Each CPU's container holds its own `current`,
  `ready`, and `zombies`. The BSP runs the only active reaper; APs do
  not reap (§Q9 deferred — see ADR-013 amendment). Containers compile
  for `MAX_CPUS=64`; only CPU 0 is brought up to scheduling at this
  sub-commit (APs initialize through trap setup but spin-wait, per
  sub-commit 1b's discipline).

* §Q5 — deferred-quantum mechanism restored. The earlier sub-commit-2
  attempt deleted `HANDOFF_PREEMPT_DEFER_TICKS`,
  `Thread.preempt_defer_ticks`, and the surrounding consume/clear/defer
  helpers as a §Q5 bet that the per-CPU shape made them unnecessary.
  That bet failed: the resulting kernel hung in `ipc_heap_kill_smoke`
  closure verification. The mechanism is restored verbatim in per-CPU
  form. See ADR-013 §"Q5 bet failed; deferred quantum stays" amendment.

* §Q9 — per-CPU reapers and `Process.live_thread_count` deferred. The
  full §Q9 design (per-CPU reaper bootstrap threads, `live_thread_count`
  atomic, double-check deregistration under PROCESSES write lock) failed
  the closure gate at run 81 of 100 with a kernel hang in
  `sched::smoke_test`'s `reap()` path. Bisection (EXP-A through EXP-H)
  isolated §Q9 as the cause. Sub-commit 2 ships with the BSP scanning
  all initialized per-CPU schedulers and reaping each CPU's zombies
  locally; APs do not participate in reaping. EXP-H verified this shape
  passes the original gate 100/100 at `NEO_SMP=4`.
  See ADR-013 §"Q9 deferred with bisection evidence" and §"Q9
  hypothesis (informational)" amendments. The §Q9 redo design notes are
  in `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` (research artifact, not
  yet normative).

* AP GS-base rewire after GDT load (BUILD-3 amendment). Each AP loads
  its per-AP GDT, then rewrites `GS.base` from the per-CPU TSS slot.
  Without this, syscalls from a process running on an AP would land on
  the wrong kernel stack. See ADR-013 §"AP GS-base rewire after GDT
  load" amendment.

The closure gate (100x `ipc_heap_kill_smoke` at `NEO_SMP=4`, zero
flakes) was met by EXP-H on this exact tree. KASLR-Q1B (50 runs on
clean main HEAD `9505619`) produced 4 `attempt to add with overflow`
panics independent of this sub-commit, confirming the KASLR
heap-placement bug is a pre-existing latent defect since `3aa07d1`
("BUILD-3 enable KASLR"). KASLR triage is tracked separately as
`Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md`.

Files changed:
  docs/ADR-013-phase-2-smp-retrofit.md  (+amendments for Q5/Q9/GS-base)
  kernel/Cargo.toml                     (no functional change; deps if any)
  kernel/src/arch/x86_64/mod.rs         (per-CPU export wiring)
  kernel/src/arch/x86_64/smp.rs         (AP GS-base rewire)
  kernel/src/arch/x86_64/syscall.rs     (per-CPU current()-via-gs)
  kernel/src/main.rs                    (boot ordering for per-CPU init)
  kernel/src/sched/mod.rs               (PerCpuScheduler exports)
  kernel/src/sched/scheduler.rs         (the bulk of the §Q3 refactor + §Q5 restore + BSP-scans-all-CPUs reap)
  kernel/src/sched/thread.rs            (preempt_defer_ticks restored)
  kernel/src/test_harness.rs            (new test scenarios)
  tools/xtask/src/main.rs               (NEO_SMP plumbing)

Refs: ADR-013, ADR-011 (superseded), Plans/PM-DECISION-2026-05-06-Q5-bet-failed.md, Plans/PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md, Plans/PM-DECISION-2026-05-07-kaslr-q1-higher-n.md, Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md
```

If you spot a factual error in the body — e.g., the file list doesn't match what's actually changed, or one of the bullets misstates what the code does — pause, surface, and we'll correct before commit. The body is load-bearing for project history; we don't ship a wrong narrative.

---

## 3. Stage and commit

```powershell
# Stage exactly the 11 expected files. Do not stage anything in Plans/.
git add docs/ADR-013-phase-2-smp-retrofit.md
git add kernel/Cargo.toml
git add kernel/src/arch/x86_64/mod.rs
git add kernel/src/arch/x86_64/smp.rs
git add kernel/src/arch/x86_64/syscall.rs
git add kernel/src/main.rs
git add kernel/src/sched/mod.rs
git add kernel/src/sched/scheduler.rs
git add kernel/src/sched/thread.rs
git add kernel/src/test_harness.rs
git add tools/xtask/src/main.rs

# Verify the stage matches expectation: 11 files, exact same numbers.
git diff --staged --stat
# Expected: 11 files changed, 366 insertions(+), 112 deletions(-)

# If the staged diff doesn't match: STOP. Run `git reset` to unstage and surface.

# Commit using the message from §2.
# Save the message to a temp file first so editor isn't required.
$msg = @'
sched: Phase 2 BUILD-3 sub-commit 2 — per-CPU schedulers + §Q5 restore + AP GS-base rewire; §Q9 deferred

[paste the entire body from §2 here]
'@
$msg | Out-File -FilePath ".git/COMMIT_EDITMSG_SC2" -Encoding utf8 -NoNewline
git commit -F ".git/COMMIT_EDITMSG_SC2"
Remove-Item ".git/COMMIT_EDITMSG_SC2"

# Verify the commit landed.
git log --oneline -3
# Expected: top line is the new sub-commit 2; second line is 9505619 chore: gitignore...
```

If the commit fails (hook rejection, encoding issue, etc.): **STOP and surface.** Do not push a force-amended-on-the-fly commit. We can adjust and retry.

---

## 4. Cleanup

After §3 confirms the commit is on `main` via `git log --oneline -3`:

```powershell
# Drop both stashes — they were safety nets and are no longer needed.
git stash drop "stash@{0}"  # kaslr-q1-higher-n-pre-diagnostic
git stash drop "stash@{0}"  # was stash@{1}; becomes stash@{0} after the first drop. Was: sub-commit-2-close-attempt-blocked-on-kaslr.

# Verify clean state.
git stash list      # expected: empty.
git status --short  # expected: only Plans/ untracked items.
```

The Plans/ artifacts stay untracked; they're the working artifacts for the closure conversation and get an INDEX.md sweep dispatched separately to Jarvis.

---

## 5. Close report

Write `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md` with:

```markdown
# Close report: sub-commit 2

## Commit
- Hash: <new commit hash>
- Branch: main
- Lands at: 2026-05-07 ~<time>
- (Project is local-only; no push step.)

## What was committed
- §Q3 per-CPU schedulers
- §Q5 deferred-quantum mechanism (restored from the earlier failed deletion)
- §Q9 deferred (BSP-scans-all-CPUs fallback shipped instead)
- AP GS-base rewire amendment
- ADR-013 amendments documenting all of the above
- Two new test scenarios in test_harness.rs

## Closure gate
EXP-H: 100/100 PASS on ipc_heap_kill_smoke at NEO_SMP=4. Gate met on 2026-05-06.

## What was NOT included
- §Q9 redo (deferred to ADR-016 / sub-commit 5+)
- KASLR heap-placement fix (separate triage; ADR-015 in draft)
- §Q9 redo design notes (in Plans/, not yet committed; awaits INDEX.md sweep)
- ADR-015 (in draft; awaits ratification)

## Stashes
Both stashes dropped after successful commit. `git stash list` is empty.

## Next steps for PM
- ADR-015 ratification + KASLR triage dispatch
- Sub-commit 3a checkpoint conversation (pre-thinking already at Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md)
- Jarvis: INDEX.md sweep covering 2026-05-06 14:00 onward
```

Ping PM with the commit hash on completion.

---

## 6. What you do NOT do

- Do not re-run any verification (EXP-H is the gate; it was met on this tree).
- Do not modify any file in the working tree before committing.
- Do not stage anything in `Plans/`.
- Do not amend the commit message after committing without explicit PM approval.
- Do not drop stashes before the commit verification (`git log --oneline -3` showing the new commit) completes.
- Do not attempt to set up a remote, configure `origin`, or push. Project is local-only per project-lead clarification.
- Do not run `cargo neo-test` or any other build/test command unless something looks wrong and you need to confirm the kernel still builds. If you do need to verify a build, run `cargo k` (build only, no test loop) and surface the result.

---

## 7. Time budget

- 5 min: pre-flight check (§1).
- 5 min: review and adjust the commit message body if anything looks off (§2).
- 5 min: stage + commit (§3).
- 2 min: stash cleanup (§4).
- 10 min: close report writeup (§5).

**Total: ~25 min.** This is mechanical work; the complexity is in being careful with the commit message and the stash ordering, not in execution time.

If you're tired after the 4h35m diagnostic loop, **deferring this to tomorrow morning is acceptable.** The closure isn't urgent enough to risk a botched commit. Surface the deferral and PM holds the closure dispatch until you're ready.

---

## 8. Why this matters

Sub-commit 2 has been pending all day plus most of yesterday. Its closure unblocks:

- Sub-commit 3a (the asm release primitive — pre-thinking already drafted).
- ADR-015 ratification (KASLR fix — needs the post-§Q9 Phase 2 substrate to land first).
- INDEX.md sweep (Jarvis — needs the closure event to flip statuses).
- Sub-commit 3b, 4, 5 (the rest of Phase 2 BUILD-3).

Closing it now is the right move. The evidence is in (EXP-H 100/100 + KASLR-Q1B 4/50 confirms pre-existence). The decision tree was committed in advance. We execute.

---

**End of dispatch.**

*Standard close report shape: commit hash + gate evidence + stashes-dropped confirmation + handoff to next steps. ~30 min total.*
