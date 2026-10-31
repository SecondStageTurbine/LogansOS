# Dispatch: Wedge investigation 3 — clean baseline on un-instrumented substrate

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Diagnostic measurement run, NO instrumentation, NO kernel changes. Establishes the actual TIMEOUT rate on un-instrumented `46984a1` with a three-bucket classifier (PRE_KERNEL / BOOT_KERNEL_PRE_SMOKE / IN_KERNEL_SMOKE). Produces a findings report. No commit on main; the only artifacts produced are serial logs and a Plans/ findings note.
**Substrate:** Project HEAD `46984a1`. Working tree should be clean on tracked files. Both H1 and WEDGE-INV-2 instrumentations remain stashed (`stash@{0}: wedge-investigation-2-instrumentation`, `stash@{1}: h1-experiment-instrumentation`); they are NOT used here. The kernel under test is exactly what's at `46984a1`, no changes.

---

## 0. Standing instructions

1. **No code changes.** Not to `kernel/`, not to `tools/`, not to `userland/`. Pure measurement. The only files this dispatch creates are serial logs in `target/verify/` and a single Plans/ findings note.
2. **No commit on main.** Nothing to commit; this is read-only kernel + new logs.
3. **Do not pop the existing stashes.** They are preserved for future reference. Confirm they remain at `stash@{0}` and `stash@{1}` after the experiment.
4. **Stop and surface** if anything is unexpected — the build won't reproduce cleanly from `46984a1`, the wedge rate is wildly different from prior data (>50% or <0.5%), the classifier produces ambiguous results that don't fit any of the three buckets, or the runs are taking dramatically longer than ADR-015 PATCH1's loop.
5. **Time budget: ~2-2.5h.** ~10 min build verification, ~90 min for 50-run loop (or ~150-180 min for 100-run if we go bigger; see §3.3), ~30-45 min analysis + findings report.

---

## 1. The question this experiment answers

Two consecutive instrumented experiments showed wedge rates well below the historical baseline:

| Loop | Sample | TIMEOUT rate | Substrate notes |
|---|---|---|---|
| KASLR-Q1B | 50 runs | 14/50 (28%) | un-instrumented, sub-commit 2 substrate |
| ADR-015 PATCH1 | 50 runs | 11/50 (22%) | un-instrumented, post-ADR-015 substrate |
| H1 experiment | 20 runs | 1/20 (5%) | reaper instrumented, lock-free short-circuit removed |
| WEDGE-INV-2 | 30 runs | 1/30 (3.3%) | smoke-test/thread/scheduler instrumented |

Two interpretations are possible:

- **(A) The wedge is timing-sensitive (Heisenbug).** Any instrumentation that changes timing reduces the rate, including instrumentation that doesn't logically affect the bug.
- **(B) The wedge has effectively gone away on the post-ADR-015 substrate.** ADR-015's heap fix incidentally moved memory layout enough to reduce or eliminate the bug's trigger conditions.

We don't currently have data to distinguish (A) from (B). This experiment produces that data: a 50-run loop on **un-instrumented** `46984a1`, classified into three buckets, gives us the actual current rate and decomposition.

If the IN_KERNEL_SMOKE rate is at or near KASLR-Q1B's 28%, interpretation (A) is right and the bug is real but hides from instrumentation. The next investigation needs a much larger sample to overcome the Heisenbug problem, or needs an out-of-band observation method (e.g., QEMU's GDB stub).

If the IN_KERNEL_SMOKE rate is much lower (say ≤5%), interpretation (B) is plausible. The wedge fix may not need to be a kernel code change — it may already be effectively fixed by ADR-015, and the wedge-fix commitment can be discharged with a regression-guard test rather than a kernel patch.

If the rate is somewhere in between (say 10-20%), neither interpretation cleanly wins. The next dispatch becomes about mechanism — possibly out-of-band observation, possibly characterizing the timing dependency directly.

## 2. Pre-flight

```powershell
cd "E:\AI Automation\AltimitNeo"

# Confirm HEAD.
git log --oneline -1
# Expected: 46984a1 docs: Plans/INDEX.md — post-ADR-015 sweep + open commitments section

# Confirm working tree clean on tracked files.
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty.

# Confirm both prior instrumentations are stashed.
git stash list
# Expected: at least two entries.
#   stash@{0}: ... wedge-investigation-2-instrumentation
#   stash@{1}: ... h1-experiment-instrumentation

# Confirm a clean build reproduces from 46984a1.
cargo k
# Expected: 0 errors, ~18 warnings (the established baseline).
```

If anything unexpected, STOP and surface.

## 3. The classifier

### 3.1 Three buckets

For each run's serial log, classify according to **the highest line marker reached** before the timeout fires:

- **PRE_KERNEL:** the log contains Limine boot menu lines but no `limine: Loading executable` and no `_kernel_start`. The kernel never loaded.
- **BOOT_KERNEL_PRE_SMOKE:** `_kernel_start` (or equivalent kernel entry marker) is present, but no smoke-test markers are emitted. The kernel started booting but didn't reach the smoke-test phase. Look for early-boot markers like `[ kernel ] mm:`, `[ kernel ] sched:`, etc.
- **IN_KERNEL_SMOKE:** the log shows kernel boot reaching at least the in-kernel smoke test phase (i.e., `[ kernel ] sched: spawned 'ping'` or equivalent). This is the bucket that contains the original wedge concern.

A PASS run completes the entire smoke test and the scenario, ending with the `NEO_TEST/1 event=PASS` frame.

### 3.2 Implementation in PowerShell

The simplest implementation is a function that reads each log and returns the bucket:

```powershell
function Classify-Run {
    param([string]$LogPath)

    if (-not (Test-Path $LogPath)) { return "MISSING" }
    $content = Get-Content $LogPath -Raw

    # Did the run pass?
    if ($content -match "NEO_TEST/1 event=PASS") { return "PASS" }

    # Did it panic?
    if ($content -match "panicked at") { return "PANIC" }

    # Three-bucket TIMEOUT classification, ordered most-progress to least.
    if ($content -match "sched: spawned 'ping'") { return "TIMEOUT_IN_KERNEL_SMOKE" }
    if ($content -match "_kernel_start|kernel \] mm:|kernel \] sched:") { return "TIMEOUT_BOOT_KERNEL_PRE_SMOKE" }
    if ($content -match "ALTIMIT NEO boot menu|limine") { return "TIMEOUT_PRE_KERNEL" }

    return "TIMEOUT_UNCLASSIFIED"
}
```

Adjust the regex anchors to match what your existing log format actually contains. Check 2-3 known PASS logs and the WEDGE-INV-2 run-3 (PRE_KERNEL example) to confirm the patterns hit correctly.

### 3.3 Sample size

Run **50 boots** as the primary loop. If the IN_KERNEL_SMOKE TIMEOUT rate at 50 runs is unclear (e.g., 1-2 timeouts, where the rate could be anywhere from 2-10%), extend to 100 runs.

The 50/100 split is because: at 50 runs, an IN_KERNEL_SMOKE rate of 0/50 has a 95% CI upper bound of ~6%; an observed 5/50 (10%) has a CI of ~3-22%. If we see clearly low (0-1) or clearly elevated (5+), 50 runs is enough. If we see something in between (2-4), we need more samples.

## 4. The runs

```powershell
$results = @()
$runDir = "target/verify"

for ($i = 1; $i -le 50; $i++) {
    $log = "$runDir/WEDGE-INV-3-baseline-run-$i.serial.log"
    Write-Host "Run $i/50..."
    docker compose run --rm neo cargo neo-test --scenario smp_per_cpu_scheduler_smoke 2>&1 | Tee-Object -FilePath $log
    $cls = Classify-Run -LogPath $log
    $results += [PSCustomObject]@{ Run = $i; Classification = $cls }

    # Optional early stop: if we hit 5 IN_KERNEL_SMOKE timeouts before run 30,
    # the wedge is reproducible at the historical rate; no need for the full 50.
    $smokeTimeouts = ($results | Where-Object { $_.Classification -eq "TIMEOUT_IN_KERNEL_SMOKE" }).Count
    if ($i -ge 20 -and $smokeTimeouts -ge 5) {
        Write-Host "Early stop: $smokeTimeouts IN_KERNEL_SMOKE timeouts in $i runs; rate consistent with historical baseline."
        break
    }
}

# Summary.
$results | Group-Object Classification | Sort-Object Name | Format-Table Name, Count -AutoSize
```

Adapt the loop wrapper to fit your existing pattern. The early-stop is optional but saves time when the result is already clear.

## 5. Decision matrix on the results

After the loop, compute IN_KERNEL_SMOKE rate (count divided by total runs) and decide which interpretation the data supports.

### Case A — IN_KERNEL_SMOKE rate is high (≥15%)

The wedge is real on un-instrumented substrate. Interpretation (A) is correct: instrumentation perturbs timing and hides the bug. Next investigation needs out-of-band observation. The findings report should explicitly recommend: "The wedge reproduces at $RATE% on un-instrumented substrate but disappears under instrumentation. Next dispatch must use observation methods that don't change kernel timing — candidates are QEMU GDB stub, hardware breakpoints, or post-mortem state dumps from a wedged VM."

### Case B — IN_KERNEL_SMOKE rate is low (≤5%)

The wedge is mostly absent on `46984a1`. ADR-015 may have incidentally fixed it. The findings report should recommend: "The IN_KERNEL_SMOKE wedge rate has dropped from the 22-28% historical baseline to $RATE%. The wedge-fix-before-SC-3B commitment is likely discharged by ADR-015 plus a regression-guard test. Next dispatch is a regression-guard scenario, NOT a kernel patch."

### Case C — IN_KERNEL_SMOKE rate is intermediate (5-15%)

The bug is reduced but not gone. We need to characterize the timing dependency directly. The findings report should recommend: "The IN_KERNEL_SMOKE wedge rate is $RATE%, intermediate between historical baseline and zero. Next investigation should compare un-instrumented runs against runs with minimal logging-only instrumentation to map how sensitive the rate is to timing."

## 6. The findings report

Produce `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md`. Required sections:

### §1. Summary

3-5 sentences. Total runs, breakdown by classification (PASS / TIMEOUT_PRE_KERNEL / TIMEOUT_BOOT_KERNEL_PRE_SMOKE / TIMEOUT_IN_KERNEL_SMOKE / PANIC / OTHER), and the verdict (Case A / B / C from §5).

### §2. Methodology

Substrate (`46984a1`, no kernel changes). Sample size and any early-stop rationale. The three-bucket classifier and how it maps onto serial log patterns.

### §3. Quantitative results

Table: run number, classification. Aggregate counts. Cross-reference to the historical baselines (KASLR-Q1B, ADR-015 PATCH1, H1, WEDGE-INV-2).

### §4. Qualitative observations

For each TIMEOUT_IN_KERNEL_SMOKE (the bucket of interest):
- The last `[ kernel ]` line emitted before timeout.
- Was it a partial-reap signature (some threads reaped, others not)? A scheduler-decision signature (specific CPU stuck)? Something else?
- Compare to H1 run 4's signature.

For TIMEOUT_PRE_KERNEL: just count and note. (Jarvis is investigating these in parallel.)

For TIMEOUT_BOOT_KERNEL_PRE_SMOKE: this would be a third bug class we haven't seen before. If any appear, capture details.

### §5. Verdict and recommendation

Per §5 of this dispatch — Case A, B, or C, and what the next dispatch should look like. Be specific.

### §6. Cross-references

Standard cross-reference list.

## 7. Restoring the substrate

```powershell
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty.

git stash list
# Expected: same two stashes preserved.

git log --oneline -1
# Expected: 46984a1 ...
```

Ping PM with:
- Path to findings report.
- IN_KERNEL_SMOKE rate (count and percentage).
- Verdict (Case A / B / C).
- Any TIMEOUT_BOOT_KERNEL_PRE_SMOKE instances (a previously-unseen bucket).
- Whether there were any panics or OTHER classifications.

## 8. What you do NOT do

- Do not change kernel code. Not for instrumentation, not for "minor cleanup," not for anything.
- Do not change the test scenario. `smp_per_cpu_scheduler_smoke` at NEO_SMP=4, exactly as currently committed.
- Do not pop the stashes. They are reference state, not work to restart.
- Do not write a fix dispatch — that's PM's job after reading the findings.
- Do not surface "high TIMEOUT count" as a blocker. Timeouts are the data; the dispatch is designed to measure them.

## 9. Why this matters

Three previous wedge investigations have produced inconsistent rates (28%, 22%, 5%, 3.3%) across different instrumentation regimes. We don't currently know which rate represents reality on the production substrate. The wedge-fix-before-SC-3B commitment can't be discharged correctly without that ground truth: if the wedge is mostly gone, we need a regression guard; if it's hiding from instrumentation, we need out-of-band observation; if it's intermediate, we need timing characterization. This dispatch produces the data that makes the choice.

This is also a pattern recognition for future investigations: when instrumented experiments produce surprising results, the un-instrumented baseline should always be re-measured before drawing conclusions. The 22-28% baseline numbers were on different substrates (sub-commit 2 vs. post-ADR-015) and may not reflect current reality. We should know what current reality is.

---

**End of dispatch.**
