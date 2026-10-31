# Dispatch: Sub-commit 2 partial-rollback bisection (Step 1 — revert §Q9)

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 + Q5 restoration applied), unchanged since the EXP-G close.
**Substrate:** Sub-commit 2 has a second latent bug that instrumentation amplified rather than masked. We bisect by reverting components one at a time and re-running the 100x gate. See `Plans/PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md` for full rationale.
**Type:** Code rollback + 100x gate. NOT a fix attempt. NOT a commit. Working tree stays uncommitted.

---

## 0. Standing instructions

1. **Do NOT commit anything.** Working tree stays uncommitted regardless of result.
2. **Do NOT attempt fixes** beyond the specified rollback. Hypotheses go in close report only.
3. **Capture before-state.** `git stash` is NOT the right tool — you'd lose the §Q5 restoration work. Instead, work IN your working tree, applying targeted reverts. PM will review the diff before any decision about commit/keep/redo.
4. **Capture verbatim output.**
5. **At end of dispatch, working tree should be: §Q3 retained, §Q5-restore retained, AP-GS-base retained, §Q9 reverted.** §Q9's revert is staged in working tree (uncommitted).

---

## 1. The question

Sub-commit 2 has three independent components plus the Q5 restoration:

- **A. AP GS-base rewire after GDT load** (BUILD-3 amendment to ADR-013).
- **B. §Q3 — per-CPU schedulers.** The big refactor.
- **C. §Q9 — per-CPU reapers + `live_thread_count`.** Distributed reaper work + Process atomic counter.
- **D. §Q5 mechanism restoration** (the per-CPU form of `preempt_defer_ticks` and friends).

Step 1 of bisection: **revert C only**, keep A, B, D. Re-run the 100x gate.

- **If hang persists:** C is not the (or only) cause. Restore C and proceed to Step 2 (revert B).
- **If hang disappears:** C is a cause. Investigate `live_thread_count` and per-CPU reaper distribution.

This dispatch covers Step 1 only. Step 2 will be a separate dispatch if needed.

---

## 2. Pre-revert OBSERVE pass (mandatory)

Before reverting anything, identify exactly what §Q9 introduced:

1. **`Process.live_thread_count: AtomicU32` field.** Should be in `kernel/src/sched/process.rs` Process struct.
2. **`live_thread_count: AtomicU32::new(1)` initializer.** In `Process::new_kernel` and `Process::new_user` (or wherever Process is constructed).
3. **`fn proc_on_thread_spawn(pid)`.** A new helper added by §Q9 — may or may not have call sites yet (sub-commit 2 didn't add multi-thread-per-process spawn paths; the helper exists for forward compat).
4. **Modified `on_thread_reap` body.** The fetch_sub-and-double-check pattern. If `live_thread_count` reaches 0, proceed; otherwise return early.
5. **Per-CPU reaper distribution.** `reap()` becomes `reap_local()` (or stays as `reap()` but accesses `current_scheduler().inner.zombies` instead of a global). Each CPU's bootstrap thread runs its own reaper loop.
6. **AP bootstrap loop.** `run_ap_bootstrap` includes a `reap()` call in its loop. Pre-§Q9 (if §Q9 had not been applied), APs would just `yield_now; pause` without reaping.

Locate each of these in the working tree before reverting. Write down line numbers in your close report's OBSERVE notes section.

If any §Q9 component is structurally entangled with §Q3 (per-CPU schedulers) such that a clean revert of C while keeping B is impossible, **stop and surface to PM.** The bisection assumes A, B, C, D are separable. If they're not separable, the bisection design changes.

---

## 3. The revert

Revert §Q9 in the working tree. The shape:

### 3.1 Remove `live_thread_count` field

In `kernel/src/sched/process.rs`:

- Remove the `live_thread_count: AtomicU32` field from `Process` struct.
- Remove the `live_thread_count: AtomicU32::new(1)` initializers from `Process::new_kernel` and `Process::new_user`.
- Remove the `fn proc_on_thread_spawn(pid)` helper if it exists (no callers; was forward-compat).
- Revert `on_thread_reap` to its pre-§Q9 form. The pre-§Q9 form is the one that existed before sub-commit 2 — read it via `git show HEAD:kernel/src/sched/process.rs` and find the `on_thread_reap` function. Copy that body back. It does the registry lookup + handle drain + close_channel_peers + remove from PROCESSES + insert into COMPLETED_EXIT_CODES, all WITHOUT the `live_thread_count.fetch_sub` decrement and without the double-check-under-lock pattern.

The pre-§Q9 `on_thread_reap` is what shipped on `main` at HEAD (commit `e092217` / `e6edc65` / etc. — none of those touch sched/process.rs, so HEAD's version is the pre-sub-commit-2 version).

### 3.2 Revert per-CPU reaper distribution

In `kernel/src/sched/scheduler.rs`:

- If `reap()` was renamed to `reap_local()`, rename it back to `reap()`.
- Change `reap()` to operate on a global zombies queue (the pre-§Q9 form). This is more involved: §Q3 made queues per-CPU, so reverting §Q9's "per-CPU reaper" means EITHER:
  - **(a)** Add a global zombies queue alongside the per-CPU `current`/`ready` queues. `exit()` rotates to the global zombies queue. `reap()` drains the global queue. Per-CPU `inner.zombies` no longer exists. **This is intrusive.**
  - **(b)** Keep `inner.zombies` per-CPU but have `reap()` only run on BSP, draining ALL CPUs' zombie queues by iterating over `PER_CPU[0..MAX_CPUS]` and acquiring each scheduler's lock. **This is simpler but cross-CPU-locks-y.**
  - **(c)** Mark this combination unworkable, restore §Q9, surface the structural entanglement to PM.

Read the actual code first. If §Q9's per-CPU reaper distribution is tightly bound to §Q3's per-CPU containers (which it likely is — §Q9 was written specifically to fit §Q3), option (a) and (b) require new code, not just a revert. **In that case, choose option (c): restore §Q9, surface to PM, and we'll re-design the bisection.** That's NOT a failure of the bisection; it's information about how the components are coupled.

### 3.3 Revert AP bootstrap loop's `reap()` call

In `kernel/src/sched/scheduler.rs::run_ap_bootstrap` (or wherever it lives):

- If the AP bootstrap loop calls `reap()`, remove that call. APs do not reap in the post-§Q9-revert world; only BSP reaps.
- The AP bootstrap loop becomes `loop { yield_now(); core::hint::spin_loop(); }` — no reaper call.

### 3.4 Verify the revert builds

```
docker compose run --rm neo cargo k
```

Expected: green build. Warning count may shift slightly; surface deltas with explanation.

If the build fails because §Q9 components are referenced from places we didn't account for, **stop and surface**. The build failure tells us §Q9 is more entangled than the OBSERVE pass suggested; the bisection plan needs revision.

---

## 4. Run the 100x gate

After the revert builds, run the 100x ipc_heap_kill_smoke gate at NEO_SMP=4:

```powershell
cd "E:\AI Automation\AltimitNeo"
$results = @()
for ($i = 1; $i -le 100; $i++) {
    Write-Output "=== H-Run $i ==="
    $env:NEO_SMP = "4"
    $r = docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke 2>&1 | Out-String

    if ($r -match "NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke") {
        Write-Output "H-Run $i`: PASS"
        $results += "PASS"
    } else {
        Write-Output "H-Run $i`: FAIL"
        $serial = "target/neo-test/ipc_heap_kill_smoke.serial.log"
        if (Test-Path $serial) {
            Copy-Item $serial "target/verify/EXP-H-fail-run-$i.serial.log"
        }
        $r | Out-File "target/verify/EXP-H-fail-run-$i.txt" -Encoding utf8
        $results += "FAIL"
        # DON'T break here — we want to see if the failure rate changed
        if ($results.Count -ge 3 -and ($results | Where-Object { $_ -eq "FAIL" }).Count -ge 3) {
            Write-Output "Three or more failures observed; stopping for analysis"
            break
        }
    }
}

$passCount = ($results | Where-Object { $_ -eq "PASS" }).Count
$failCount = ($results | Where-Object { $_ -eq "FAIL" }).Count
"EXP-H-PASS-COUNT: $passCount/$($results.Count)" | Out-File "target/verify/EXP-H-summary.txt"
"EXP-H-FAIL-COUNT: $failCount/$($results.Count)" | Out-File "target/verify/EXP-H-summary.txt" -Append
Write-Output "EXP-H-PASS-COUNT: $passCount / $($results.Count)"
Write-Output "EXP-H-FAIL-COUNT: $failCount / $($results.Count)"
```

**Note the change from previous experiments:** this loop does NOT stop on first failure. It stops after 3 failures total OR after 100 runs. The reason: if the bug is still present at any rate, we want to see the rate, not just whether it's nonzero. Three failures observed gives us a rate estimate (e.g., 3/30 = 10%, 3/85 = 3.5%).

### 4.1 What "PASS" means in this experiment

Same as before: `NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke` line in the captured output.

### 4.2 What "FAIL" means

Anything that's not the PASS line: timeout, hang, panic, FAIL frame, etc. Capture each failing run's serial log.

---

## 5. Result to capture

```
EXP-H-PASS-COUNT: N/total
EXP-H-FAIL-COUNT: M/total
First failure run: <number or "n/a">
First failure serial log: <path>
First failure last line: <verbatim>
Second failure run: <number or "n/a">
Second failure last line: <verbatim>
Third failure run: <number or "n/a">
Third failure last line: <verbatim>
```

If failures cluster around the same locus (all hang at same point in the boot sequence), surface that. If they hang at different points, surface that too.

---

## 6. Decision rule

- **EXP-H passes 100/100:** §Q9 is implicated in the bug. PM proceeds to investigate `live_thread_count` and per-CPU reaper distribution as the cause. Sub-commit 2 may close with §Q9 deferred to a future sub-commit.

- **EXP-H fails at any rate ≥ ~5%:** §Q9 is not the (sole) cause. The bug is elsewhere — most likely in §Q3 (per-CPU schedulers) or in the §Q5 restoration's per-CPU form. PM dispatches Step 2 (revert §Q3 too).

- **EXP-H fails at significantly lower rate (e.g., 1/100 or 0/100 in a partial run):** ambiguous. Could be that §Q9 contributes but isn't the only cause. PM thinks harder.

- **The build doesn't compile after §Q9 revert:** §Q9 is structurally entangled with §Q3. PM revises the bisection plan; possibly we need a different rollback strategy.

In all cases: **report and stop.** Do not proceed to Step 2 (reverting §Q3) without PM dispatch.

---

## 7. What you do NOT do

- **Do not commit anything.** The working tree state at end of dispatch should be: §Q9 reverted, everything else intact, no commits made.
- **Do not modify §Q3 or §Q5 or AP-GS-base code.** Only §Q9.
- **Do not add instrumentation.** No `[ DBG ]` lines. We've learned what we can from instrumentation; this experiment is about which subsystem.
- **Do not retry-to-pass.** If the gate fails, surface; don't try to make it pass.

---

## 8. After capture: what happens to the working tree

The working tree at end of EXP-H is: sub-commit 2 minus §Q9. PM reviews the diff. Options downstream:

- If §Q9 is the cause: keep this state, proceed to commit as a smaller sub-commit 2. §Q9 becomes a future sub-commit with cleaner design. ADR-013 amendment records the deferral.
- If §Q9 is NOT the cause: restore §Q9 (your `git diff` from this dispatch will tell you exactly what to put back), then proceed to Step 2 (revert §Q3).
- If structurally entangled: surface to PM, we redesign the bisection.

For now: leave the working tree in the post-revert state. Do not restore anything until PM directs.

---

## 9. Time budget

- §Q9 revert: 30–60 min (mostly OBSERVE + trace dependencies).
- Build + 100x gate: 20–30 min depending on failure rate.
- Total: ~1–1.5 hr.

If the §Q9 revert is harder than 60 min (because of structural entanglement), surface to PM rather than push through. We'd rather know about coupling sooner than later.

---

## 10. Why this matters

EXP-G told us instrumentation amplifies the bug rather than localizing it. Instrumentation alone won't pin the cause. Partial rollback IS the next bisection step — it cuts the hypothesis space (which subsystem in sub-commit 2) without requiring kernel internals knowledge that we don't have.

§Q9 first because:
- Smaller surface (Process struct, on_thread_reap, AP reaper loop, ~3 files vs §Q3's ~6 files).
- More likely to be cleanly revertable (§Q9 was added on top of §Q3; reverting it should be subtractive, not architectural).
- If §Q9 is the cause, it's a more contained fix — `live_thread_count` is a forward-compat addition that wasn't load-bearing for Phase 1.5b semantics.

If Step 1 doesn't resolve, Step 2 (revert §Q3) is the bigger structural retreat. We do that only after Step 1 narrows the hypothesis.

---

**End of dispatch.**

*Standard close report shape: §Q9 revert summary, build status, 100x gate result with rate computation, hang locus pattern across failures (if any), conclusions you can defensibly draw, hypotheses you have but did NOT act on. Do NOT restore §Q9 or commit anything; PM reviews the working tree state.*
