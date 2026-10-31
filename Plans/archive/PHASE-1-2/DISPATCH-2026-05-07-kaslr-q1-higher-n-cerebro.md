# Dispatch: KASLR diagnostic — Q1 higher-N (50 runs on main HEAD)

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-07
**Parent:** DISPATCH-2026-05-06-kaslr-overflow-diagnostic-cerebro.md (the original Q1+Q2). This is the higher-N continuation invoked by the original dispatch's §6 decision rule when Q1 was 0/10 and Q2a confirmed KASLR-positional. PM-DECISION-2026-05-07-kaslr-q1-higher-n.md is the calling decision.
**Type:** Diagnostic only. NOT a fix. NOT a commit. Loop, capture, report. Working tree state unchanged at end of dispatch.
**Substrate:** Same as the original Q1 dispatch. Sub-commit 2 working tree is currently applied (post-EXP-H state, ADR-013 amendments uncommitted). The original `stash@{0}: On main: sub-commit-2-close-attempt-blocked-on-kaslr` is still in the stash list as a safety net; do NOT drop it.

---

## 0. Standing instructions

1. **Do NOT commit anything.** Working tree stays as it is at end of dispatch.
2. **Do NOT attempt to fix the KASLR overflow.** Separate triage item, not for this dispatch.
3. **Do NOT modify anything in `kernel/src/mm/`.** No source changes anywhere in the kernel for this dispatch.
4. **Do NOT drop `stash@{0}`.** It's our safety net while sub-commit 2 is uncommitted. Use `git stash apply` (not `pop`) when restoring after Q1B.
5. Capture verbatim output for every run.

---

## 1. The question

**Q1B (higher-N).** Does the addr.rs:390 overflow panic reproduce on `main` HEAD (commit `9505619`) under 50 NEO_SMP=4 default-suite runs?

The original Q1 was 10 runs → 0 panics. By the original dispatch's §6 decision rule, that result is ambiguous because main could plausibly have been lucky 10 times in a row. 50 runs tightens the 95% upper bound on main's overflow rate to ~6% (rule of three), which is well below the ~0.8% observed rate that prompted the diagnostic. Enough resolution to call it.

Q2 is **not** repeated. Q2a's clean PASS with KASLR disabled is sufficient confirmation that KASLR is the positional trigger. Q2b is still infeasible (no seed override exposed).

---

## 2. Setup: stash sub-commit 2 working tree, verify clean main HEAD

You currently have the post-EXP-H sub-commit 2 working tree applied (11 modified tracked files + ADR-013 amendments). The original safety stash is still in the stash list. Add a second stash for the current state, then verify clean main HEAD.

```powershell
cd "E:\AI Automation\AltimitNeo"

# Stash the current applied state (will become stash@{0}; the safety stash becomes stash@{1})
git stash push -u -m "kaslr-q1b-higher-n-pre-diagnostic"

# Confirm clean main HEAD
git status                              # should be clean (Plans/ untracked is expected and gitignored)
git rev-parse HEAD                      # should print 9505619... (main HEAD)
git stash list                          # should show TWO stashes:
                                        #   stash@{0}: kaslr-q1b-higher-n-pre-diagnostic
                                        #   stash@{1}: sub-commit-2-close-attempt-blocked-on-kaslr
```

If `git status` shows any modified tracked files after the stash, **stop and surface to PM** — the stash should have caught them. (The handoff and Plans/ files in the working tree are expected to stay; they're untracked and gitignored.)

### 2.1 Capture main HEAD kernel binary metrics (one-time, before the loop)

For the sub-commit-2-vs-main binary-size delta argument in the eventual close report, capture the main HEAD kernel ELF size *once* before the loop:

```powershell
docker compose run --rm neo cargo k 2>&1 | Out-File "target/verify/KASLR-Q1B-main-cargo-k.txt" -Encoding utf8

# The kernel ELF path may be one of:
#   target/x86_64-altimit/debug/kernel
#   target/x86_64-altimit/debug/altimit-neo-kernel
#   (or wherever the existing ISO build pulls from)
# Check whatever the actual path is in your build pipeline.
$candidates = @(
    "target/x86_64-altimit/debug/kernel",
    "target/x86_64-altimit/debug/altimit-neo-kernel",
    "target/debug/kernel"
)
$mainKernel = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($mainKernel) {
    $size = (Get-Item $mainKernel).Length
    "main-HEAD kernel ELF: $mainKernel" | Out-File "target/verify/KASLR-Q1B-main-kernel-size.txt"
    "size: $size bytes ($([math]::Round($size/1MB, 2)) MB)" | Out-File "target/verify/KASLR-Q1B-main-kernel-size.txt" -Append
} else {
    "main-HEAD kernel ELF NOT FOUND in any of: $($candidates -join ', ')" | Out-File "target/verify/KASLR-Q1B-main-kernel-size.txt"
    "Surface this to PM if it happens; the binary-size delta is desirable but not load-bearing for the closure decision." | Out-File "target/verify/KASLR-Q1B-main-kernel-size.txt" -Append
}
Get-Content "target/verify/KASLR-Q1B-main-kernel-size.txt"
```

If none of the candidate paths exist, that's not a blocker — surface it in the close report and continue. The binary-size delta is desirable evidence but not load-bearing.

---

## 3. The 50-run loop

```powershell
$results = @()
$panics = @()
$timeouts = @()

for ($i = 1; $i -le 50; $i++) {
    Write-Output "=== KASLR-Q1B Run $i ==="
    $env:NEO_SMP = "4"
    $r = docker compose run --rm neo cargo neo-test 2>&1 | Out-String
    $r | Out-File "target/verify/KASLR-Q1B-main-run-$i.txt" -Encoding utf8

    if ($r -match "attempt to add with overflow") {
        Write-Output "KASLR-Q1B Run $i: PANIC reproduced on main HEAD"
        $results += "PANIC"
        # Capture the virt_base from the panicking run (high-bits placement is what we care about)
        if ($r -match "virt=0x([0-9a-f]+)") {
            $panics += [PSCustomObject]@{ Run = $i; VirtBase = "0x$($matches[1])" }
        } else {
            $panics += [PSCustomObject]@{ Run = $i; VirtBase = "unknown" }
        }
        # Snapshot any serial logs that exist
        Get-ChildItem "target/neo-test/*.serial.log" -ErrorAction SilentlyContinue | ForEach-Object {
            Copy-Item $_.FullName "target/verify/KASLR-Q1B-main-run-$i.$($_.Name)"
        }
    } elseif ($r -match "QEMU timed out after 180s for ``([^``]+)``") {
        $scenario = $matches[1]
        Write-Output "KASLR-Q1B Run $i: TIMEOUT on scenario '$scenario' (harness wedge, not KASLR)"
        $results += "TIMEOUT"
        $timeouts += [PSCustomObject]@{ Run = $i; Scenario = $scenario }
    } elseif ($r -match "neo-test meta: deliberate failure was detected") {
        Write-Output "KASLR-Q1B Run $i: clean PASS (full default suite + meta scenario)"
        $results += "PASS"
    } else {
        Write-Output "KASLR-Q1B Run $i: OTHER (capture for review)"
        $results += "OTHER"
    }
}

# Summarize
$panicCount = ($results | Where-Object { $_ -eq "PANIC" }).Count
$passCount  = ($results | Where-Object { $_ -eq "PASS" }).Count
$timeoutCount = ($results | Where-Object { $_ -eq "TIMEOUT" }).Count
$otherCount = ($results | Where-Object { $_ -eq "OTHER" }).Count

$summary = @"
KASLR-Q1B Result (50 runs on main HEAD 9505619, NEO_SMP=4 default suite):
  PANIC:   $panicCount
  PASS:    $passCount
  TIMEOUT: $timeoutCount  (harness wedge; not KASLR overflow; see triage queue)
  OTHER:   $otherCount

PANIC details (if any):
$($panics | ForEach-Object { "  Run $($_.Run): virt_base=$($_.VirtBase)" } | Out-String)

TIMEOUT details:
$($timeouts | ForEach-Object { "  Run $($_.Run): scenario=$($_.Scenario)" } | Out-String)

Per-run results array (1..50):
$($results -join ', ')
"@

$summary | Out-File "target/verify/KASLR-Q1B-summary.txt" -Encoding utf8
Write-Output ""
Write-Output $summary
```

Expectations on rates (for your sanity-checking, not a gate):
- **PANIC:** 0 (most likely) or 1–3 (still consistent with pre-existence). 4+ would be surprising and worth flagging.
- **PASS:** ~30–45 of 50 (extrapolating from the 7/10 PASS rate yesterday, allowing for variance).
- **TIMEOUT:** ~10–20 of 50 (extrapolating from 3/10 yesterday). If this is wildly different — e.g., zero timeouts, or 30+ timeouts — note it; the harness wedge is its own triage item and a meaningful rate change is data.

If during the loop you observe PANIC on **two consecutive runs**, you may stop the loop early and report partial results — pre-existence is confirmed and there's no value in finishing the 50. Don't stop early on a single PANIC; the rate matters for the close report.

If during the loop you observe **5+ consecutive timeouts with no PASS or PANIC in between**, stop and surface — that suggests the harness wedge has gotten worse in some way and the diagnostic environment is no longer stable enough to trust.

---

## 4. Restore working tree state

After the loop completes (or after early-stop):

```powershell
# Apply the most recent stash (pre-Q1B state with sub-commit 2 applied)
git stash apply "stash@{0}"

# Verify
git status                              # should show 11 modified tracked files
git diff --stat HEAD                    # should show ~366 insertions / 112 deletions across 11 files
git stash list                          # should still show BOTH stashes
```

**Do NOT use `git stash pop`.** Pop drops the stash; we want to keep both stashes in place as safety nets until sub-commit 2 commits.

If `git status` shows anything different from the pre-diagnostic 11-file modification set, **stop and surface to PM**.

### 4.1 Capture sub-commit-2 kernel binary metrics

For the size-delta comparison:

```powershell
docker compose run --rm neo cargo k 2>&1 | Out-File "target/verify/KASLR-Q1B-subcommit2-cargo-k.txt" -Encoding utf8

# Use whatever kernel ELF path you found above for main; the path is the same.
$sc2Kernel = $mainKernel  # from §2.1; if main path was found, use the same path
if ($sc2Kernel -and (Test-Path $sc2Kernel)) {
    $size = (Get-Item $sc2Kernel).Length
    "sub-commit-2 kernel ELF: $sc2Kernel" | Out-File "target/verify/KASLR-Q1B-subcommit2-kernel-size.txt"
    "size: $size bytes ($([math]::Round($size/1MB, 2)) MB)" | Out-File "target/verify/KASLR-Q1B-subcommit2-kernel-size.txt" -Append
} else {
    "sub-commit-2 kernel ELF NOT FOUND" | Out-File "target/verify/KASLR-Q1B-subcommit2-kernel-size.txt"
}
Get-Content "target/verify/KASLR-Q1B-subcommit2-kernel-size.txt"
```

Computing the delta inline (only if both files exist):

```powershell
if ((Test-Path "target/verify/KASLR-Q1B-main-kernel-size.txt") -and (Test-Path "target/verify/KASLR-Q1B-subcommit2-kernel-size.txt")) {
    $mainBytes = [int64]((Select-String -Path "target/verify/KASLR-Q1B-main-kernel-size.txt" -Pattern 'size: (\d+) bytes').Matches[0].Groups[1].Value)
    $sc2Bytes  = [int64]((Select-String -Path "target/verify/KASLR-Q1B-subcommit2-kernel-size.txt" -Pattern 'size: (\d+) bytes').Matches[0].Groups[1].Value)
    $delta = $sc2Bytes - $mainBytes
    $pct = [math]::Round(($delta / $mainBytes) * 100, 3)
    "main: $mainBytes bytes; sub-commit-2: $sc2Bytes bytes; delta: $delta bytes ($pct%)"
}
```

---

## 5. Close report shape

Write `target/verify/KASLR-Q1B-summary.txt` (the loop already does this) and then a brief Markdown wrap-up `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` with this shape:

```markdown
# Close report: KASLR-Q1B (higher-N pre-existence diagnostic)

## Q1B result
- 50 runs on main HEAD `9505619`, NEO_SMP=4 default suite
- PANIC: N (paths to serial logs if N > 0)
- PASS: M
- TIMEOUT: K (scenario distribution)
- OTHER: J (paths if J > 0)

## Conclusion (your read)
- Pre-existing / introduced-by-sub-commit-2 / inconclusive
- Brief reasoning (1–2 sentences)

## Binary size data
- main HEAD kernel ELF: X bytes
- sub-commit-2 kernel ELF: Y bytes
- Absolute delta: Z bytes
- Percent delta: P%

## Working tree state
- Confirmed identical to pre-diagnostic post-EXP-H state (11 modified tracked files + ADR-013 amendments)
- Both stashes still present in `git stash list`

## Anything weird
- Anything you noticed during the runs that doesn't fit the expected pattern (e.g., a new failure mode, an unusual virt_base distribution, harness behaving differently than yesterday)
```

PM reads this, applies the decision tree from PM-DECISION-2026-05-07-kaslr-q1-higher-n.md, and either issues the corrected closure dispatch or stops to think harder.

---

## 6. What you do NOT do

- Do not modify `kernel/src/mm/heap.rs`, `kernel/src/mm/paging.rs`, or anything else in `kernel/src/mm/`.
- Do not modify `boot/limine.conf` (Q2a's modification was scoped to the original dispatch and was already validated; we're back on default `kaslr: yes` per the working-tree-restore at end of yesterday's diagnostic).
- Do not commit.
- Do not drop either stash (the safety net or the new stash you create at the start of this dispatch).
- Do not run any additional Q2 variants or any new diagnostic. If you finish the 50 runs and have spare time, surface — PM has parallel work going on the KASLR overflow ADR draft and may want input.

---

## 7. Decision rule (PM, not Cerebro)

This is documented for the close-report writeup so you know what your numbers map to. You do **not** apply this rule yourself; just report the numbers honestly.

- **Q1B PANIC ≥ 1 in 50 runs:** PM issues corrected closure dispatch (gate = original 100x ipc_heap_kill_smoke at NEO_SMP=4, EXP-H satisfied). KASLR overflow → separate triage item.
- **Q1B PANIC = 0 in 50 runs:** PM accepts pre-existence as confirmed at ~6% upper bound (95% CI), issues corrected closure dispatch with same shape. KASLR overflow → separate triage item, with the higher-N evidence cited.
- **Q1B TIMEOUT rate materially different from yesterday's 3/10:** PM flags as separate signal for the harness-wedge triage item. Does not affect closure decision.
- **Anything else (5+ consecutive timeouts; PANIC count surprisingly high; new failure modes):** PM stops, re-thinks, possibly re-dispatches.

---

## 8. Time budget

Starting now (~09:10):
- 5 min stash + verify
- 3 min main HEAD `cargo k` + binary-size capture
- ~150–500 min for the 50-run loop (each run 3–10 min, depending on whether harness-wedge timeouts fire)
- 3 min sub-commit-2 `cargo k` + binary-size capture
- 5 min stash apply + verify restore
- 10 min close report writeup

**Total: ~3–9h.** Wide range because of harness-wedge timeout variance. Best case the report lands around midday; worst case end-of-day.

If you hit hour 6 and aren't done, surface — partial-N is acceptable as a fallback (30 runs gets a ~10% upper bound, which is weaker but still meaningful).

The loop is largely hands-off after kickoff. Periodically check progress; otherwise PM and you don't need to be in active synchronous communication during the run.

---

## 9. Why this matters

The §Q5 lesson keeps applying: bets that look defensible can fail. The bet "10 clean runs is enough" is defensible, but the diagnostic dispatch's own decision rule says to escalate. We escalate. If 50 runs come back clean, we have a much stronger pre-existence claim and the closure dispatch goes out with a tighter argument. If 50 runs surface a panic, we have unambiguous confirmation and the closure dispatch goes out with the same shape but stronger evidence. Either way, sub-commit 2 closes honestly.

The PM-DECISION accompanying this dispatch (PM-DECISION-2026-05-07-kaslr-q1-higher-n.md) commits the post-result decision tree in advance, so the post-result decision isn't made under closure pressure. Read it if you want to see the full reasoning.

---

**End of dispatch.**

*Standard close-report shape: counts table, conclusion, binary size data, working-tree state confirmation, anomalies (if any). ~3–9h total.*
