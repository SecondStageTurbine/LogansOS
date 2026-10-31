# Dispatch: Wedge investigation 4 — Limine fix + extended baseline with signature classification

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Combined config change + extended diagnostic measurement. Lands ONE small commit (the Limine `timeout: 0` config change) and produces ONE findings report (the 100-boot extended baseline with signature classification). Bundles two things because the verification of the config fix and the extended baseline measurement want the same 30-100 boots; running them together is more efficient than serially.
**Substrate:** Project HEAD `46984a1`. Working tree should be clean on tracked files. Both H1 and WEDGE-INV-2 instrumentations remain stashed (`stash@{0}: wedge-investigation-2-instrumentation`, `stash@{1}: h1-experiment-instrumentation`); they are NOT used here. The kernel under test is exactly what's at `46984a1` plus the small Limine config change this dispatch applies.

---

## 0. Standing instructions

1. **One small commit on main.** The Limine config change to `boot/limine.conf` (`timeout: 1` → `timeout: 0`). No other code changes. Drafted commit message in §6 below.
2. **No kernel code changes.** Not under any circumstances. The wedge investigation is currently in the "measure to confirm absence, do not assume bug to fix" phase.
3. **Do not pop or drop the existing stashes.** They are reference state, preserved across this dispatch.
4. **Stop and surface** if anything is unexpected — Limine fix verification produces non-zero H7 instances, the extended baseline produces unexpected timeout rates (≥10% IN_KERNEL_SMOKE), or any in-kernel timeout's signature doesn't match the three named in §4.2.
5. **Time budget: ~3-4h.** ~10 min Limine config edit + verify build, ~30 min Limine fix verification (30 boots simple scenario), ~120 min extended baseline (100 boots), ~45 min analysis + findings report, ~15 min commit + post-flight.

---

## 1. The two measurements

This dispatch performs two related measurements:

**Measurement 1: Limine fix verification (30 boots, simple scenario).** Apply `timeout: 0` to `boot/limine.conf`, run 30 boots of a simple scenario (e.g., `kaslr_smoke`), confirm 0 H7 instances. Pre-fix H7 rate was ~0.36% across 2,504 historical logs; post-fix expectation is 0/30. Single H7 instance would still be statistically possible but would warrant investigation.

**Measurement 2: Extended baseline (100 boots, smp_per_cpu_scheduler_smoke).** With the Limine fix in place, run 100 boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` and classify each result. Combined with WEDGE-INV-3's 50 prior observations on un-modified `46984a1`, this gives 150 total un-instrumented observations on functionally-equivalent kernel substrate.

The two measurements use different scenarios deliberately: the simple scenario in M1 isolates H7 (no in-kernel-wedge confound); the extended baseline in M2 specifically measures the in-kernel wedge rate.

## 2. Why the extended baseline matters

WEDGE-INV-3 returned 0/50 in-kernel wedges on `46984a1`. ADR-015 PATCH1 measured 14% on `47972c2` (functionally identical kernel). The 95% CI for 0/50 has an upper bound of ~6%; before declaring the wedge effectively resolved, we want a tighter bound.

Additionally, PM-side analysis of PATCH1's 7 non-H7 timeout logs (`Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`) shows the wedges fall into **three distinct signatures**, not one:

- **SMOKE-MID-PHASE:** wedged mid-smoke-test, some threads reaped others alive (matches H1 run-4 pattern). 2 of 7 in PATCH1.
- **SMOKE-COMPLETION:** all smoke threads reaped, then silence (transition to neo-init never happens). 2 of 7 in PATCH1.
- **POST-SMOKE-USER-PROCESS:** smoke completes, neo-init/worker run, wedge during user-process lifecycle code. 3 of 7 in PATCH1.

The "wedge" is probably 2-3 distinct bugs presenting as TIMEOUTs. A regression guard needs to catch all three signatures, not just one. The extended baseline's signature classification gives us per-signature rates to inform that guard.

## 3. Apply the Limine fix

```powershell
cd "E:\AI Automation\AltimitNeo"

# Verify pre-flight state.
git log --oneline -1
# Expected: 46984a1 docs: Plans/INDEX.md ...

git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty.

# Edit boot/limine.conf: change timeout: 1 to timeout: 0.
# Use an editor or PowerShell:
(Get-Content "boot/limine.conf") -replace '^timeout:\s*1\s*$', 'timeout: 0' | Set-Content "boot/limine.conf"

# Verify the edit was correct (single occurrence, exact replacement):
Select-String -Path "boot/limine.conf" -Pattern "^timeout:" | Format-Table LineNumber, Line -AutoSize
# Expected: one line, "timeout: 0"

git diff boot/limine.conf
# Expected: a single -timeout: 1 / +timeout: 0 hunk, no other changes.
```

Build sanity:

```powershell
cargo k
# Expected: 0 errors, ~18 warnings (the established baseline). The Limine config is read at boot, not compile time, so cargo-k doesn't directly verify the change took effect — that's M1's job.
```

## 4. Measurement 1 — Limine fix verification

```powershell
# Run 30 boots of kaslr_smoke (or whichever simple scenario the project has that takes ~5 sec/boot).
# kaslr_smoke is recommended because it touches minimal post-boot kernel paths and is therefore most diagnostic for H7 isolation.
$m1Results = @()
for ($i = 1; $i -le 30; $i++) {
    $log = "target/verify/WEDGE-INV-4-limine-fix-run-$i.serial.log"
    Write-Host "M1 Run $i/30..."
    docker compose run --rm neo cargo neo-test --scenario kaslr_smoke 2>&1 | Tee-Object -FilePath $log
    $m1Results += [PSCustomObject]@{ Run = $i; Classification = (Classify-Run -LogPath $log) }
}
$m1Results | Group-Object Classification | Format-Table -AutoSize
```

Use the canonical classifier from `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 (paste it into your loop wrapper if you don't have it as a function already). Specifically the H7-detection version with the negative check for `limine: Loading executable`, not the version in your earlier WEDGE-INV-3 dispatch.

**Expected outcome:** 30/30 PASS. Anything else is unexpected:

- 1+ TIMEOUT_PRE_KERNEL: Limine fix didn't fully eliminate H7. Surface with the offending log; we may need to investigate further (the H7 mechanism analysis in Jarvis's report may be incomplete).
- 1+ TIMEOUT_IN_KERNEL_SMOKE: a non-H7 wedge surfaced even on the simple scenario. This would be unusual (the simple scenario has historically had near-zero IN_KERNEL_SMOKE rate) but possible. Surface.
- Any PANIC: stop immediately; something broke that wasn't related to this dispatch.

If M1 passes cleanly (30/30 PASS), proceed to M2. If anything anomalous, surface to PM before continuing.

## 4.2 The signature classifier (for M2)

For each TIMEOUT_IN_KERNEL_SMOKE run, sub-classify the wedge signature based on terminal serial log content:

```powershell
function Classify-WedgeSignature {
    param([string]$LogPath)

    $content = Get-Content $LogPath -Raw

    # Did neo-init or any user-space process spawn?
    $hasNeoInit = $content -match "proc: spawned 'neo-init'"
    $hasWorker = $content -match "proc: spawned 'worker'|sched: enqueued user thread"

    # Did all three smoke threads complete (reaped or HOG_TRIGGER)?
    $reapedPing = $content -match "sched: reaped 'ping'"
    $reapedPong = $content -match "sched: reaped 'pong'"
    $reapedHog = $content -match "sched: reaped 'hog'"
    $hogExited = $content -match "HOG_TRIGGER seen, exiting|hog.*exiting"

    if ($hasNeoInit -or $hasWorker) {
        return "POST_SMOKE_USER_PROCESS"
    }

    if ($reapedPing -and $reapedPong -and ($reapedHog -or $hogExited)) {
        return "SMOKE_COMPLETION"
    }

    # Some but not all smoke threads reaped, no user-process activity.
    return "SMOKE_MID_PHASE"
}
```

Apply this only to runs classified TIMEOUT_IN_KERNEL_SMOKE. PASS runs and other timeouts are not signature-classified.

**Reference signatures from PATCH1** (from `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`):

- **SMOKE_MID_PHASE example (run 13):** terminal `[ kernel ] sched: reaped 'pong' (id=5)` with no ping/hog reap.
- **SMOKE_COMPLETION example (run 21):** terminal `[ kernel ] sched: reaped 'ping' (id=4)` with all three smoke threads accounted for, no neo-init line.
- **POST_SMOKE_USER_PROCESS example (run 11):** terminal `[ frame ] phase=post-natural-exit ...` with neo-init and worker spawned and worker exited.

If a TIMEOUT_IN_KERNEL_SMOKE run doesn't fit any of these three, return a fourth bucket name and surface in the findings report — that would be a new signature.

## 5. Measurement 2 — Extended baseline

```powershell
$m2Results = @()
$kernelSmokeTimeouts = @()

for ($i = 1; $i -le 100; $i++) {
    $log = "target/verify/WEDGE-INV-4-extended-run-$i.serial.log"
    Write-Host "M2 Run $i/100..."
    docker compose run --rm neo cargo neo-test --scenario smp_per_cpu_scheduler_smoke 2>&1 | Tee-Object -FilePath $log
    $cls = Classify-Run -LogPath $log
    $sig = if ($cls -eq "TIMEOUT_IN_KERNEL_SMOKE") { Classify-WedgeSignature -LogPath $log } else { $null }
    $m2Results += [PSCustomObject]@{ Run = $i; Classification = $cls; Signature = $sig }
    if ($cls -eq "TIMEOUT_IN_KERNEL_SMOKE") {
        $kernelSmokeTimeouts += [PSCustomObject]@{ Run = $i; LogPath = $log; Signature = $sig }
    }
}

# Summary.
$m2Results | Group-Object Classification | Format-Table -AutoSize

# If any IN_KERNEL_SMOKE timeouts, summarize signatures.
if ($kernelSmokeTimeouts.Count -gt 0) {
    Write-Host "IN_KERNEL_SMOKE timeouts by signature:"
    $kernelSmokeTimeouts | Group-Object Signature | Format-Table Name, Count -AutoSize
}
```

**Optional early stop:** if 5+ TIMEOUT_IN_KERNEL_SMOKE accumulate before run 50, the wedge has reproduced at near-historical rates and the extended baseline data is sufficient — surface immediately, don't run further.

**Expected outcomes:**

- **Case I — 0/100 IN_KERNEL_SMOKE:** combined with WEDGE-INV-3's 0/50, the rate is 0/150. 95% CI upper bound ~2%. Strong evidence the wedge is effectively absent on current substrate.
- **Case II — 1-3/100 IN_KERNEL_SMOKE:** rate is 1-3% in this batch. Combined with WEDGE-INV-3's 0/50, rate is ~1-2% across 150 observations. Wedge is rare but not gone. Signature classification tells us which of the three (or new) is residual.
- **Case III — 4+/100 IN_KERNEL_SMOKE:** rate is 4%+. The 0/50 in WEDGE-INV-3 was lucky sampling. Wedge is still meaningfully present. Investigation continues with signature breakdown as substrate.

## 6. Stage and commit the Limine fix

After M1 verifies (30/30 PASS) and M2 has run (whatever the result), stage and commit ONLY the Limine config change:

```powershell
git add boot/limine.conf
git diff --staged
# Expected: single hunk in boot/limine.conf, "timeout: 1" -> "timeout: 0".

$msg = @'
boot: skip Limine boot menu (timeout: 0) to eliminate H7 pre-kernel stall

The H7 pre-kernel boot/menu stall (~0.36% of historical boots, 9 instances
across 2504 serial logs) occurs when Limine's countdown completes but the
kernel ELF read from QEMU's emulated CD-ROM stalls. Setting timeout: 0
eliminates the interactive menu phase, removing the timing-sensitive race
window between countdown completion and CD-ROM I/O.

The single-entry boot menu serves no purpose in automated testing. Limine's
serial: yes still mirrors boot diagnostics to COM1 for visibility on
early-boot failures.

Verified: 30/30 PASS on kaslr_smoke loop with timeout: 0, 0 H7 instances
(vs. ~0.36% baseline pre-fix). See Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md
section 4 for verification data.

Refs: Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md (root cause analysis),
Plans/PM-NOTE-2026-05-08-h7-finding-integration.md (project integration),
Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md
(this dispatch).
'@

# Do NOT use here-strings with $variable interpolation in the message; the
# message is fixed text. Save to file and commit -F to avoid PowerShell
# parser issues.
$msg | Out-File -FilePath ".git/COMMIT_EDITMSG_LIMINE" -Encoding utf8 -NoNewline
git commit -F ".git/COMMIT_EDITMSG_LIMINE"
Remove-Item ".git/COMMIT_EDITMSG_LIMINE"

git log --oneline -2
# Expected:
#   <new hash> boot: skip Limine boot menu (timeout: 0) ...
#   46984a1    docs: Plans/INDEX.md ...
```

If M1 fails (any non-PASS), do NOT commit. Surface with the failing log and let PM decide whether to investigate the Limine fix further or proceed differently.

## 7. The findings report

Produce `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md`. Required sections:

### §1. Summary

3-5 sentences. M1 result (Limine fix verified 30/30 or anomaly). M2 result (Case I/II/III from §5). Combined 150-observation summary. Top-line implication for the wedge-fix-before-SC-3B commitment.

### §2. Methodology

Substrate (`46984a1` + Limine `timeout: 0` for M1 and M2). M1 scenario, M2 scenario, sample sizes. The canonical classifier (cite §5.1 of H7 findings) and the signature sub-classifier from §4.2 of this dispatch.

### §3. Measurement 1 — Limine fix verification

Per-run table for the 30 boots. Aggregate. Comparison to pre-fix H7 rate.

### §4. Measurement 2 — Extended baseline

Per-run table for the 100 boots (or however many run before early-stop). Aggregate by classification. For any IN_KERNEL_SMOKE timeouts: signature breakdown.

### §5. Combined 150-observation analysis

WEDGE-INV-3's 50 + this dispatch's 100 = 150 total un-instrumented observations on functionally-equivalent kernel substrate. Aggregate IN_KERNEL_SMOKE rate. 95% CI calculation.

### §6. Verdict and recommendation

Per Case I/II/III from §5 of this dispatch:

- Case I (0/150): wedge appears effectively absent. Recommend regression-guard scenario design and commitment-disposition decision. PM drafts both.
- Case II (1-3/150): wedge is rare. Recommend signature-targeted investigation OR commitment amendment to accept residual rate, depending on which signatures fired.
- Case III (4+/150): wedge is still meaningfully present. Investigation continues; PM and project lead decide next dispatch.

### §7. Cross-references

Standard list including this dispatch, the PATCH1 signature analysis PM-NOTE, the H7 findings, the WEDGE-INV-3 findings, the wedge-fix commitment.

## 8. Restoring the substrate

```powershell
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty (commit landed; nothing else modified).

git stash list
# Expected: same 2 entries preserved.

git log --oneline -2
# Expected: <Limine commit> + 46984a1 + (older commits).
```

Ping PM with:
- Path to findings report.
- M1 result (30/30 PASS or anomaly).
- M2 IN_KERNEL_SMOKE count and rate.
- Combined 150-observation rate.
- Signature breakdown if any IN_KERNEL_SMOKE timeouts.
- New commit hash for the Limine fix.

## 9. What you do NOT do

- Do not modify kernel code. Not under any circumstances.
- Do not modify the test scenarios. Use them as currently committed.
- Do not pop or drop stashes. Stashes are reference state.
- Do not bundle a regression-guard test into this dispatch. The regression guard is its own work after we know the rate is durable.
- Do not draft the wedge-fix commitment amendment. PM does that based on the findings.
- Do not declare the wedge "fixed" in the commit message or findings report. The Limine commit message is for the H7 fix only; the in-kernel wedge framing in the findings report should be calibrated language ("appears effectively absent on the current substrate," not "fixed").

## 10. Why this matters

The wedge-fix-before-SC-3B commitment exists. The current measurement evidence suggests the in-kernel wedge is effectively absent on `46984a1`, but with one 50-boot observation and a non-trivial historical baseline, the evidence isn't yet decisive. This dispatch produces 150 total un-instrumented observations on functionally-equivalent kernel substrate; with that data, the commitment disposition becomes a clean project-lead decision rather than an under-evidenced guess.

The H7 fix is independent and unambiguous — it eliminates a known harness artifact. Bundling its verification with the extended baseline is efficient because the verification needs the same kind of multi-boot loop the baseline already requires. Two birds, one dispatch.

The signature classification is project-hygiene work that improves every future wedge investigation. If any IN_KERNEL_SMOKE timeouts occur, knowing whether they are SMOKE_MID_PHASE / SMOKE_COMPLETION / POST_SMOKE_USER_PROCESS / new is much more informative than just "a wedge happened." Future investigations build on this taxonomy.

---

**End of dispatch.**
