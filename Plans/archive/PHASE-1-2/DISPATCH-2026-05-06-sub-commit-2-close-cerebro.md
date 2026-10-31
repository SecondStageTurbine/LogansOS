# Dispatch: Close sub-commit 2 — commit current working tree with ADR-013 §Q9 amendments

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing post-EXP-H working tree. §Q9 reverted, BSP-scans-all-CPUs zombie reap (option b) applied, §Q3 + §Q5-restore + AP-GS-base intact.
**Type:** Single commit on `main`. Closes sub-commit 2.
**Substrate:** EXP-H 100/100 PASS. PM decision at `Plans/PM-DECISION-2026-05-06-sub-commit-2-close-without-q9.md`. The bisection found §Q9 implicated; we close without it and defer.

---

## 0. Standing instructions

1. **Commit on `main`, not detached HEAD.** Verify with `git status` and `git branch --show-current` before committing.
2. **Run `git pull --ff-only origin main`** before starting. Three doc-only commits landed today (`e092217`, `9505619`, plus the earlier `2016a33`/`3d1b908`/`e6edc65`/`503d3dc`). Your branch should already be up to date with main, but `--ff-only` costs nothing and confirms.
3. **Do NOT modify the existing rollback work.** The §Q9 revert plus option-(b) BSP-scans-all-CPUs zombie reap is the shape we're committing. Don't second-guess it. If you find yourself wanting to re-add §Q9 pieces or change the reap shape, **stop and surface to PM** — that's a design conversation, not a closure step.
4. **The only NEW changes in this dispatch are the two ADR-013 amendment subsections** specified in §3 below. Everything else is committing what's already in your working tree.
5. **Verify before committing.** §4 has the gates.

---

## 1. What this dispatch commits

The current uncommitted working tree, exactly as it stands after EXP-H. Specifically:

- **§Q3 — per-CPU schedulers:** `PerCpuScheduler` per CPU; `current` / `ready` / `zombies` queues per-CPU; `current_scheduler()` reads via GS-base.
- **§Q5 mechanism restoration:** `Thread.preempt_defer_ticks`, `wake_thread_front` setting the credit, `continue_current_after_receive` calling `defer_current_preempt_for_handoff`, per-CPU forms of the consume/clear/defer helpers. All restored in per-CPU form.
- **AP GS-base rewire after GDT load:** BUILD-3 amendment to ADR-013. APs initialize GS-base after GDT load instead of the original ordering.
- **BSP-scans-all-CPUs zombie reap:** option (b) from the bisection dispatch. BSP's `reap()` iterates over every initialized per-CPU scheduler, takes its lock, drains its zombie queue, and processes locally. APs do NOT participate in reaping.
- **§Q9 (per-CPU reaper distribution + `live_thread_count`) → DEFERRED.** The `live_thread_count` field, `proc_on_thread_spawn` helper, modified `on_thread_reap` body, and AP-side reaper participation are all reverted. Recorded in ADR-013 §Q9 amendment.
- **Existing ADR-013 amendments** Cerebro already added in earlier rounds: BUILD-3 amendment for AP GS-base, §Q5 amendment for the deletion-then-restoration chain. Keep both; this dispatch only ADDS to ADR-013, never removes.

The 12 modified tracked files in `git status` are exactly the closure scope:

```
M docs/ADR-013-phase-2-smp-retrofit.md
M kernel/Cargo.toml
M kernel/src/arch/x86_64/mod.rs
M kernel/src/arch/x86_64/smp.rs
M kernel/src/arch/x86_64/syscall.rs
M kernel/src/ipc/mod.rs
M kernel/src/main.rs
M kernel/src/sched/mod.rs
M kernel/src/sched/scheduler.rs
M kernel/src/sched/thread.rs
M kernel/src/test_harness.rs
M tools/xtask/src/main.rs
```

If `git status` shows any DIFFERENT modified files, **stop and surface to PM.** Something has shifted that this dispatch wasn't designed for.

---

## 2. What this dispatch is NOT

- **Not a fix attempt for §Q9.** §Q9 is deferred to a future sub-commit. Don't add a "minimal fix" for the leading hypothesis; that's design work for a later sub-commit, not closure work for this one.
- **Not a redesign of the BSP-scans-all-CPUs reap.** Option (b) is what we're shipping in sub-commit 2. Future sub-commit work that adds cross-CPU thread routing will need to revisit the reap shape; that's not now.
- **Not a PHASE-2.md update.** That's a separate Jarvis dispatch after this commit lands.
- **Not multi-commit.** One commit, one shape, atomic close.

---

## 3. ADR-013 §Q9 amendment text to append

Cerebro's working tree already has Cerebro's prior amendments (the BUILD-3 amendment subsection and the §Q5 amendment subsection). Append two MORE subsections under §Q9. Match the existing amendment-subsection style — same heading depth, same prose-paragraphs-then-bullets shape Cerebro has been using.

Locate §Q9's existing content in `docs/ADR-013-phase-2-smp-retrofit.md`. Append the following two subsections at the END of §Q9 (after whatever §Q9 currently contains, before the next top-level heading).

### 3.1 Subsection: "Q9 amendment: deferred with bisection evidence"

The amendment records the bisection chain that led to §Q9's deferral. Substance the subsection must convey (paraphrase to fit the prose style of the existing amendments — do NOT copy this verbatim):

- §Q9 was implemented as part of sub-commit 2's first attempt. The 100x ipc_heap_kill_smoke gate at NEO_SMP=4 failed at run 81 with a kernel hang in `sched::smoke_test`'s `reap()` (after `[ kernel ] sched: reaped 'pong'`). Adjacent runs passed cleanly. Failure rate ~1.25%.
- The §Q5 mechanism restoration was applied (PM-DECISION-2026-05-06-Q5-bet-failed). Gate still failed at the same locus with same shape. Confirmed the failure was NOT the §Q5 race.
- A six-experiment bisection (A, B, D, E, F, G — see `target/verify/EXP-CLOSE-REPORT.md` plus the per-experiment summary files) narrowed the trigger:
  - **A** (NEO_SMP=1, 100x ipc_heap_kill_smoke): 100/100 PASS — hang requires SMP.
  - **B** (NEO_SMP=4, 100x bare boot): 100/100 PASS — hang requires test-harness configuration.
  - **D** (bare-boot kernel under test-harness QEMU flags, NEO_SMP=4, 100x): 100/100 PASS — hang requires the test-harness kernel binary, not QEMU flags.
  - **E** (kaslr_smoke at NEO_SMP=4, 100x): 100/100 PASS — hang not caused by generic `mod test_harness` presence.
  - **F** (cap_mint_and_use at NEO_SMP=4, 100x): 100/100 PASS — hang not caused by generic `spawn_neo_init` presence; specific to `ipc_heap_kill_smoke`.
  - **G** (instrumented `reap()` on ipc_heap_kill_smoke at NEO_SMP=4): instrumentation amplified the hang from ~1.25% to ≥10%; failed at run 10 after `[ DBG ] reap: post-on_thread_reap id=8` with no `loop end` line emitted. Hang locus shifted from in-kernel smoke test to after `sched::exit()` from neo-init's `sys_exit`.
- **H** (sub-commit 2 minus §Q9, NEO_SMP=4, 100x): **100/100 PASS.** §Q9 implicated.
- §Q9 reverted and replaced with BSP-scans-all-CPUs zombie reap (option b in `Plans/DISPATCH-2026-05-06-sub-commit-2-bisect-step1-cerebro.md`). BSP's `reap()` iterates initialized per-CPU schedulers, takes each scheduler's lock, drains its zombie queue, processes locally. APs do not reap. Acceptable for sub-commit 2 because no thread routes to APs in this commit's scope; APs' zombie queues are always empty, so the cross-CPU scan is effectively a no-op steady-state. Sub-commit 3+ that introduces cross-CPU thread movement will need to revisit reap distribution.

### 3.2 Subsection: "Q9 hypothesis (informational, not investigated)"

Records the leading theory of failure for future-us. Substance the subsection must convey:

- We did NOT investigate within §Q9 to identify the precise failure mechanism. The closure gate passes with §Q9 reverted; drilling further would have cost 2-3 more bisection rounds without changing the closure decision.
- The leading hypothesis (NOT proven, marked clearly as hypothesis-not-investigated): the modified `on_thread_reap` decremented `proc_arc.live_thread_count` with `fetch_sub(1, AcqRel)` and returned early when `prev_count != 1`. In Phase 1.5b's one-thread-per-process world, prev_count should always be 1. If it isn't, cleanup is skipped — the Process stays in PROCESSES, the wait loop's `any_user_process()` returns true forever, kernel hangs. Possible races that could produce `prev_count != 1`:
  - Counter init race: `live_thread_count: AtomicU32::new(1)` runs after Process registration AND after first thread becomes runnable. If the thread can exit before the counter is fully observable to other CPUs, `fetch_sub` sees 0 and wraps to u32::MAX.
  - Double-reap from AP-side reaper participation: if the same zombie thread ends up on both AP's and BSP's zombie queues (shouldn't happen but the bisection didn't prove it can't), `fetch_sub` fires twice; second call wraps.
  - Memory-ordering subtlety: AcqRel on `fetch_sub` may not synchronize with the counter's initialization in the way the code assumed.
- The instrumentation-amplification observation in EXP-G is consistent with all of these — extra serial calls increase total time spent in the affected code paths, widening race windows.
- Recommendations for any future sub-commit that re-introduces `live_thread_count` (or equivalent multi-thread-per-process readiness):
  - Initialize the counter BEFORE the thread becomes runnable (eliminate init-race window).
  - Verify exactly-once decrement semantics under SMP.
  - Do NOT rely on AP-side reaper participation while the per-CPU scheduler shape doesn't yet route threads to APs.
  - Use `SeqCst` ordering until proven `AcqRel` is sufficient.
  - Add a regression test that exercises the counter under multi-thread-per-process load, not just the current Phase 1.5b single-thread invariant.
- These recommendations are informational. The amendment is descriptive of what we observed, not normative for future implementation.

### 3.3 Style notes

- Match the existing amendment-subsection heading depth (the BUILD-3 amendment and §Q5 amendment use the same depth).
- Refer to past dispatches and decision notes by their actual filenames where helpful (e.g., `Plans/PM-DECISION-2026-05-06-sub-commit-2-close-without-q9.md`, `target/verify/EXP-CLOSE-REPORT.md`). Future readers grep these.
- Keep prose tight. Each subsection should be ~15-30 lines depending on prose density. Don't pad.

---

## 4. Verification gates BEFORE committing

In order:

### 4.1 `cargo k`

```
docker compose run --rm neo cargo k
```

Expected: PASS, 18 warnings. **Match the baseline of 18 exactly.** If the count drifted (e.g., 19, 17, etc.), surface the delta and what changed before committing.

### 4.2 Full default test suite at NEO_SMP=1

```
$env:NEO_SMP = "1"; docker compose run --rm neo cargo neo-test
```

Expected: 25/25 PASS plus deliberate-failure meta-FAIL detected. If any scenario regresses, surface and stop.

### 4.3 Full default test suite at NEO_SMP=4

```
$env:NEO_SMP = "4"; docker compose run --rm neo cargo neo-test
```

Expected: 25/25 PASS plus deliberate-failure meta-FAIL detected. The closure gate. If any scenario regresses, surface and stop.

### 4.4 100x ipc_heap_kill_smoke at NEO_SMP=4

You already ran this in EXP-H (100/100 PASS). **Do not re-run** unless the working tree has changed since then (it shouldn't have). Reference the EXP-H result in the commit message. If `git status` shows any modification newer than the EXP-H run, re-run the 100x gate before committing.

### 4.5 Sanity: `live_thread_count` is gone

```
git grep "live_thread_count" -- 'kernel/src/**'
```

Expected: zero results. If anything matches, the revert is incomplete.

### 4.6 Sanity: no `[ DBG ]` instrumentation remains

```
git grep "\[ DBG \]" -- 'kernel/src/**'
```

Expected: zero results. The EXP-G instrumentation should have been reverted before EXP-H. Cerebro's close report claims it was; this verifies.

---

## 5. The commit

Single commit on `main`. Suggested commit message:

```
build-3 sub-commit 2: per-CPU schedulers + Q5 mechanism restoration + AP GS-base fix; Q9 deferred

Phase 2 BUILD-3 sub-commit 2 closes with the following scope:

- Q3: per-CPU schedulers. PerCpuScheduler per CPU; current/ready/zombies
  queues per-CPU. current_scheduler() reads via GS-base. Replaces the
  pre-sub-commit-2 single global SCHED Mutex.

- Q5: deferred-quantum mechanism restored in per-CPU form. Thread.preempt_defer_ticks,
  wake_thread_front sets the credit, continue_current_after_receive calls
  defer_current_preempt_for_handoff. The §Q5 deletion bet (in the original
  sub-commit 2) failed at the closure gate; mechanism restored. See ADR-013
  §Q5 amendment.

- AP GS-base rewire: APs initialize GS-base after GDT load instead of
  the original ordering. BUILD-3 amendment to ADR-013. Fixes a real AP
  bring-up issue that surfaced during sub-commit 2 development.

- BSP cross-CPU zombie reap: BSP's reap() iterates initialized per-CPU
  schedulers and drains each one's zombie queue. APs do not reap. This is
  option (b) from the §Q9 bisection dispatch — replaces §Q9's per-CPU
  reaper distribution.

- Q9 (per-CPU reaper distribution + Process.live_thread_count + AP-side
  reaper participation): DEFERRED to a future sub-commit. The §Q9
  implementation produced a 1.25% intermittent hang at NEO_SMP=4 in the
  ipc_heap_kill_smoke 100x gate; instrumentation amplified the hang to
  ≥10% rather than localizing it. A six-experiment bisection (A, B, D, E,
  F, G — see target/verify/EXP-CLOSE-REPORT.md and per-experiment
  summaries) implicated §Q9 as the cause. EXP-H (sub-commit 2 minus §Q9)
  passes 100/100. The leading hypothesis for §Q9's failure is recorded
  in ADR-013 §Q9 hypothesis subsection. Future re-introduction of
  multi-thread-per-process readiness will need to investigate carefully.

Verification:
- cargo k: 18 warnings (matches baseline)
- NEO_SMP=1 default suite: 25/25 PASS + meta-FAIL detected
- NEO_SMP=4 default suite: 25/25 PASS + meta-FAIL detected
- NEO_SMP=4 100x ipc_heap_kill_smoke: 100/100 PASS (EXP-H)

ADR-013 amendments in this commit:
- BUILD-3 amendment subsection (AP GS-base rewire)
- §Q5 amendment subsection (deletion failed; mechanism restored)
- §Q9 amendment subsection (deferred with bisection evidence)
- §Q9 hypothesis subsection (informational; counter init race / double-reap /
  memory ordering as leading theories)

Sub-commit 3a (ManuallyDrop+asm release in block_current_into) inherits
this scheduler shape: per-CPU schedulers, per-CPU current/ready/zombies
(zombies drained by BSP scanning all CPUs), deferred-quantum per-CPU,
no AP-side reaper, no live_thread_count.
```

Adjust to match what you actually wrote in ADR-013 §Q9 amendments. The substance must reflect the doc and the working tree.

---

## 6. What you do NOT do

- **Do not introduce new code beyond ADR-013 amendment text.** No "minimal fix" attempts for §Q9. No tweaks to the BSP-scans-all-CPUs reap.
- **Do not split into multiple commits.** One commit closes sub-commit 2.
- **Do not create a side branch.** `main`, atomic.
- **Do not re-run experiments unless the working tree has changed since EXP-H.**
- **Do not push if the verification gates fail.** Surface and stop.

---

## 7. After commit

The commit hash will be referenced in the next dispatch — Jarvis updates `PHASE-2.md` to mark sub-commit 2 as closed and folds in the §Q9 deferral note. That dispatch lands AFTER yours commits successfully.

For Plans/INDEX.md: leave it alone. Jarvis will sweep the indexed manifest in a separate pass once the dust settles.

For the boot-log-2-*.txt files at repo root: still gitignored per `9505619`. Leave on disk; do not delete.

---

## 8. Time budget

- ADR-013 amendment text: ~30-45 min for two subsections of 15-30 lines each, matching existing prose style.
- Verification gates 4.1 through 4.6: ~10-15 min total (cargo k, two suite runs, three grep checks).
- Commit + push: ~2 min.
- Total: ~50-75 min.

If anything blocks (verification fails, working tree shows unexpected file modifications, ADR amendment conflicts with the existing §Q9 content), surface to PM rather than push through.

---

## 9. Why this matters

Sub-commit 2 has been in flight since this morning. Multiple rounds of investigation. The Phase 2 SMP foundation is what 3+ depends on. Closing this — with honest scope, documented deferrals, and the closure gate met — unblocks the rest of Phase 2.

The §Q5 lesson kept applying through every step: bets that look defensible can fail; verify before committing; bisect to where the closure gate passes; document what we don't know. That discipline produced a smaller-scope sub-commit 2 than originally planned, but the shipping sub-commit is one we trust. That's the whole point.

---

**End of dispatch.**

*Standard close report shape: commit hash; verification gates' results (4.1-4.6); confirmation that ADR-013 has the two new §Q9 amendment subsections; any deviations from the dispatch and how you resolved them. ~1 hour if you're efficient.*
