# Dispatch: H7 investigation — pre-kernel boot/menu stall characterization and fix

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-08
**Type:** Investigation + remediation. Reads existing serial logs, analyzes the H7 pre-kernel boot/menu stall pattern, identifies its cause (harness/QEMU/Limine config), and either (a) fixes the cause in the relevant config files and verifies the fix, or (b) produces a finding that the cause is upstream (QEMU bug, Limine bug) and recommends a workaround. Lands as a small commit on main if a config fix is identified, or as doc-only artifact if not. Targeted to be safe to run in parallel with Cerebro's WEDGE-INV-3 baseline measurement.
**Substrate:** Project HEAD `46984a1`. Cerebro is concurrently running `Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md` — a 50-run measurement loop on un-instrumented kernel. He is producing serial logs at `target/verify/WEDGE-INV-3-baseline-run-*.serial.log`. Read those freely; do not delete or modify them.

---

## 0. Standing instructions

1. **Doc-first, then config.** Start by reading and analyzing existing data. Only modify config files (e.g., `boot/limine.conf`, `docker-compose.yml`, harness scripts) if the analysis points to a specific, identifiable fix.
2. **Do not modify kernel code.** Not under any circumstances. The H7 stall happens before the kernel loads; it cannot be a kernel bug.
3. **Do not modify Cerebro's working tree or build cache.** He's running boots in parallel; do not interfere with `target/` beyond reading log files.
4. **Cerebro's tracked-file state should remain clean.** If you see `M` entries on tracked files when running `git status`, STOP and surface — that means something is wrong with the parallel-work isolation.
5. **Stop and surface** if anything is unexpected — the H7 pattern doesn't match what's described here, the cause turns out to be in code rather than config, or the fix would require changes to kernel or test-harness logic.
6. **Time budget: ~1-2h.** Most of that is reading logs and reasoning about the boot path. The actual config edit (if needed) is small.

---

## 1. The phenomenon

WEDGE-INV-2 run 3 timed out with a serial log of only 318 bytes, containing:

```
ALTIMIT NEO boot menu rendered
Booting automatically in 1, press any key to stop the countdown...
```

No further output. The kernel never loaded; `_kernel_start` never appeared; no Limine "Loading executable" line was emitted. The harness eventually killed the run on its outer timeout.

This is qualitatively different from every other TIMEOUT we've catalogued. All the prior "wedge" data assumed timeouts were kernel-side bugs. H7 is not a kernel bug at all — it's a boot/harness/QEMU concern that lives upstream of the kernel.

The headline question: **what's stopping Limine from loading the kernel after the boot menu countdown completes?**

## 2. The investigation

### 2.1 Catalog all H7 instances across existing logs

A pre-kernel stall has a clear signature: serial log contains "ALTIMIT NEO boot menu" or "Booting automatically" but does NOT contain "limine: Loading executable" (or whatever Limine's load-confirmation line is on this build) and does NOT contain any kernel output.

Sweep all available serial logs across all campaigns and identify every H7 instance:

```powershell
cd "E:\AI Automation\AltimitNeo"

$h7Candidates = Get-ChildItem "target/verify" -Filter "*.serial.log" | ForEach-Object {
    $content = Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue
    if (-not $content) { return }

    $hasMenuLine = $content -match "ALTIMIT NEO boot menu|Booting automatically|press any key to stop"
    # Adjust the kernel-loaded markers below to match what your build actually emits.
    $hasKernelLoaded = $content -match "limine: Loading executable|_kernel_start|kernel \] mm:|kernel \] sched:"

    if ($hasMenuLine -and -not $hasKernelLoaded) {
        [PSCustomObject]@{
            Log = $_.Name
            SizeBytes = $_.Length
            Campaign = if ($_.Name -match "^([A-Z][A-Z0-9-]*)-") { $matches[1] } else { "(unknown)" }
        }
    }
}

$h7Candidates | Sort-Object Log | Format-Table -AutoSize
$totalH7 = $h7Candidates.Count
$totalLogs = (Get-ChildItem "target/verify" -Filter "*.serial.log").Count
Write-Host "H7 candidates: $totalH7 / $totalLogs total logs ($(if ($totalLogs -gt 0) { [math]::Round(100.0 * $totalH7 / $totalLogs, 2) } else { 0 })%)"
```

This gives us:
- Total H7 instances across project history.
- Their distribution across campaigns (sub-commit 2 bisection, KASLR-Q1B, ADR-015, ADR-015 PATCH1, H1, WEDGE-INV-2, etc.).
- Each instance's serial log size (small = early stall, larger = stalled later).

### 2.2 Cross-reference H7 incidence with historical TIMEOUT counts

If H7 has been recurring across campaigns, some fraction of historical TIMEOUTs may have been H7 instances misclassified as in-kernel wedges. This affects how we interpret the 22-28% baseline.

For each campaign with a known TIMEOUT count, compare:

- Total TIMEOUTs reported in that campaign's close report.
- H7 instances found in that campaign's logs.
- Difference: TIMEOUTs that are not H7, i.e., presumably in-kernel.

Tabulate:

| Campaign | TIMEOUT count (close report) | H7 count (this analysis) | Non-H7 TIMEOUTs |
|---|---:|---:|---:|
| KASLR-Q1B | 14 | ? | ? |
| ADR-015 (initial loop) | 5 | ? | ? |
| ADR-015 PATCH1 | 11 | ? | ? |
| H1 experiment | 1 | ? | ? |
| WEDGE-INV-2 | 1 | 1 (the known one) | 0 |

The right column tells us the actual in-kernel wedge rate per campaign — the number that matters for the wedge-fix commitment.

### 2.3 Diagnose the cause

H7's serial signature suggests the issue is between Limine's boot menu countdown completing and Limine's kernel-load step starting. Candidate causes, in approximate order of likelihood:

**Most likely (a): Harness outer timeout firing during normal Limine load.** The harness has an outer timeout that kills runs after some duration. If that duration is shorter than `boot menu countdown + Limine kernel-load + kernel boot to first serial output`, runs that get unlucky on Limine load timing get killed before the kernel emits anything. Easy to verify: check the harness timeout value vs. typical successful-run time-to-first-kernel-output. Easy to fix: increase the harness timeout, or modify Limine to skip the boot menu countdown.

**Plausible (b): Limine boot menu countdown is interactive-hostile.** The "press any key to stop the countdown" implies Limine waits for input. If the harness's serial pipe is providing characters from QEMU stdin or some equivalent, those characters could be interpreted as keystrokes that pause the menu. Worth checking the Limine config for `timeout` value and whether the menu is configured to skip on no-input.

**Less likely (c): QEMU + KVM scheduling glitch.** QEMU sometimes pauses VM execution under host-side load. If the host is under contention (other docker workloads, system updates running, etc.), QEMU's vCPU may stall for seconds. Hard to diagnose without `qemu -d` traces; if (a) and (b) are ruled out, this becomes the residual.

**Unlikely (d): Limine bug in kernel-load path.** If Limine had a bug where ~3% of boot attempts hung between menu and kernel-load, that would be a notable bug worth filing upstream. But we'd expect to see it across every project that uses Limine, and we don't have evidence of that. Treat as residual after (a)-(c) are ruled out.

### 2.4 Verify the fix

If (a) is the cause: increase the harness timeout (or shorten the Limine countdown to 0) and verify by running 30 boots with the new config. The H7 rate should drop to 0 (or to whatever (b)/(c)/(d) contribute as residual).

If (b) is the cause: configure Limine to skip the menu (`timeout=0` in `boot/limine.conf` if that's the right syntax for the Limine version in use) and verify by running 30 boots.

If (c) or (d): document the cause, propose a workaround (e.g., retry-on-H7-classification in the harness), and surface for a separate decision.

The 30-boot verification loop should NOT use `smp_per_cpu_scheduler_smoke` — that scenario also exposes the in-kernel wedge, which would confound the H7 results. Use a simple scenario like `kaslr_smoke` (or whatever the project's lowest-overhead PASS-baseline scenario is) so any TIMEOUT in the verification loop is attributable to H7, not to other bugs.

## 3. The classifier improvement

Cerebro's WEDGE-INV-3 dispatch already specifies a three-bucket classifier (PRE_KERNEL / BOOT_KERNEL_PRE_SMOKE / IN_KERNEL_SMOKE). Read his dispatch (`Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md` §3) for the regex patterns and confirm consistency with your H7 detection from §2.1.

If your H7 detection uses different regex patterns than Cerebro's PRE_KERNEL bucket, reconcile them. The project should have one canonical classifier going forward, used by both campaigns and any future ones. Propose the canonical version in your findings report.

If the project's existing tooling (e.g., `tools/xtask/src/main.rs` test classification, or any harness script) currently treats all timeouts as one bucket, propose a code change to adopt the three-bucket scheme. Do NOT make the change in this dispatch — propose it in the findings report and PM will write a separate dispatch for it.

## 4. The findings report (and possibly fix commit)

Produce `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md`. Required sections:

### §1. Summary

3-5 sentences. Total H7 instances found across all campaigns. Diagnosed cause (a/b/c/d from §2.3). Whether a config fix was identified and applied (and verified) or whether a finding was produced.

### §2. Catalog

The full table of H7 instances from §2.1: log filename, size, campaign, any other distinguishing detail.

### §3. Reinterpretation of historical TIMEOUT counts

The cross-reference table from §2.2. Specifically: how does subtracting H7 from each campaign's TIMEOUT count change the in-kernel wedge rate for that campaign? Are some campaigns *primarily* H7, with little or no in-kernel wedge?

This may be the most important section in the report. If it turns out, e.g., that ADR-015 PATCH1's 11 TIMEOUTs were 8 H7 + 3 in-kernel, then the in-kernel wedge rate has been ~6%, not 22%, the whole time. The historical baseline we've been chasing may have been mostly an artifact of a harness/config issue.

### §4. Diagnosis and fix

Which cause (a/b/c/d) the data supports. The reasoning. If a config fix was applied, what was changed and how was it verified.

### §5. Classifier proposal

The canonical three-bucket classifier (or four, including PASS), with regex patterns. To be adopted across campaigns going forward.

### §6. Recommendations

What the project should do next regarding:
- Classifier adoption (propose a separate dispatch for code changes).
- The historical baseline reinterpretation — does the wedge-fix commitment look different now that we know the historical numbers were partially H7?
- Any follow-up needed (e.g., upstream Limine bug report if (d) is the cause).

### §7. Cross-references

Standard cross-reference list.

## 5. If a config fix is identified

If §2.3 points to a clear config-level fix (e.g., Limine `timeout=0`, harness timeout increase, etc.), make the change and commit it as a small focused commit on main:

```powershell
git add boot/limine.conf  # or whichever file
# (verify the diff is minimal and only what was specified)

$msg = @"
boot: <component>: <change> to eliminate H7 pre-kernel stall

The H7 pre-kernel boot/menu stall (~N% of historical TIMEOUTs) was caused
by <root cause from §2.3>. <The fix> eliminates this class of stall as
verified by 30-boot loop showing 0 H7 instances vs. baseline of <prior
rate>.

This change does not affect kernel correctness or in-kernel wedge behavior.
It improves the project's ability to distinguish kernel bugs from harness
artifacts in future verification runs.

Refs: Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md,
Plans/WEDGE-INV-2-FINDINGS-2026-05-08.md (the source of the H7 observation),
Plans/DISPATCH-2026-05-08-h7-investigation-jarvis.md (this dispatch).
"@
$msg | Out-File -FilePath ".git/COMMIT_EDITMSG_H7" -Encoding utf8 -NoNewline
git commit -F ".git/COMMIT_EDITMSG_H7"
Remove-Item ".git/COMMIT_EDITMSG_H7"
```

If no config fix is identified (the cause is upstream or requires separate decision), the dispatch produces only the findings report — no commit on main.

## 6. Pre-flight and post-flight

```powershell
cd "E:\AI Automation\AltimitNeo"

# Pre-flight.
git log --oneline -1
# Expected: 46984a1

git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty (clean tree). If you see Cerebro's WEDGE-INV-3 has produced any
# tracked-file changes, STOP and surface — his dispatch was supposed to be measurement-only.

git stash list
# Expected: 2 entries (preserved instrumentation). Verify; do not pop or drop.

# Post-flight (after findings report and any commit).
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty (your H7 fix commit is committed, not staged; or no commit if no fix).

git stash list
# Expected: same 2 entries.

git log --oneline -2
# If you committed: <new hash> boot: ... + 46984a1 docs: ...
# If you didn't: 46984a1 docs: ...
```

Ping PM with:
- Path to findings report.
- Total H7 instances catalogued.
- Diagnosed cause.
- Whether a fix was applied (with new commit hash) or only documented.
- The reinterpreted historical wedge rate after subtracting H7.
- Any recommendations for the canonical classifier or follow-up work.

## 7. What you do NOT do

- Do not modify kernel code.
- Do not modify Cerebro's working files or in-flight serial logs.
- Do not bundle the classifier code change into this dispatch's commit. The classifier change (if recommended) is its own dispatch.
- Do not file an upstream Limine bug report from this dispatch. If (d) is the cause, document the recommendation; PM and project lead decide whether to file.
- Do not assume the H7 instances all have the same root cause. They might split into sub-categories. If they do, name them.
- Do not reclassify historical TIMEOUTs in any source-of-truth artifact yet. The reinterpretation in §3 of the findings is a *proposal* for how to read the numbers, not a rewrite of the existing close reports.

## 8. Why this matters

Two reasons:

1. **The historical baseline may be misleading.** If a meaningful fraction of the 22-28% TIMEOUT rate has been H7, the actual in-kernel wedge rate may be much lower than we've been thinking, and the wedge-fix commitment looks easier to discharge.

2. **The classifier is project hygiene.** Going forward, every verification gate will produce TIMEOUTs at some baseline rate. Knowing what fraction of those are kernel concerns vs. harness artifacts is the difference between "we have a wedge problem" and "we have a config problem we already fixed plus a smaller wedge problem we're investigating." The latter is the truthful project state.

This is also the kind of work that compounds — the better the classifier, the cleaner every future verification gate's signal-to-noise.

---

**End of dispatch.**
