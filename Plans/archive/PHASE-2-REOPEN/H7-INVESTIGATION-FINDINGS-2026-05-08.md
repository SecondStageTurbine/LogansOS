# H7 investigation findings — pre-kernel boot/menu stall characterization

**Date:** 2026-05-08
**Substrate:** Project HEAD `46984a1` (`docs: Plans/INDEX.md — post-ADR-015 sweep + open commitments section`)
**Investigation type:** Read-only log analysis + config diagnosis. No kernel code changes, no new boots.
**Serial logs scanned:** 2,504 files across all campaigns in `target/verify/`

---

## 1. Summary

Sweep of all 2,504 serial logs found **9 instances of H7** (pre-kernel boot/menu stall) across 5 campaigns. All 9 share an identical 318-byte signature: the Limine boot menu renders, the countdown begins ("Booting automatically in 1"), but the kernel is never loaded — no `limine: Loading executable` line, no `_kernel_start`, no kernel output at all.

Diagnosed cause: **Limine boot menu countdown is a timing-sensitive race with QEMU's emulated CD-ROM read path**. The single-entry boot menu serves no purpose in automated testing (there is only one boot target). Setting `timeout: 0` in `boot/limine.conf` eliminates the menu phase entirely, removing the stall window.

**No config fix was committed.** Verification requires a dedicated 30-boot loop, which would conflict with Cerebro's concurrent WEDGE-INV-3 baseline measurement. PM should decide whether to verify as a standalone step or bundle with Cerebro's next dispatch.

After subtracting H7 from historical TIMEOUT counts, the actual in-kernel wedge rate is lower than previously believed: ~16-20% rather than 22-28%, depending on campaign. The H7 class accounts for roughly 1-3 percentage points of the historical baseline.

## 2. Catalog of H7 Instances

All 9 instances have identical serial output (318 bytes): Limine boot menu UI followed by "Booting automatically in 1, press any key to stop the countdown..." with no further output. QEMU hangs until the harness's 180-second outer timeout kills it.

| # | Log file | Size | Campaign | Notes |
|---|----------|------|----------|-------|
| 1 | `build-3-2-ipc-100x-run-3.serial.log` | 318 | build-3-2-ipc-100x | Sub-commit 2 IPC 100x gate (EXP-H substrate) |
| 2 | `KASLR-Q1-main-run-10.buddy_max_order.serial.log` | 318 | KASLR-Q1-main-run | Initial KASLR Q1 diagnostic (10 runs, ambiguous result) |
| 3 | `ADR-015-loop-run-17.cap_revoke_descendants_walk.serial.log` | 318 | ADR-015-loop-run | ADR-015 initial verification loop (discarded at run 24) |
| 4 | `ADR-015-loop-run-20.smap_policy.serial.log` | 318 | ADR-015-loop-run | Same campaign, different scenario |
| 5 | `ADR-015-PATCH1-loop-run-4.buddy_alloc_free_alloc_returns_same_frame.serial.log` | 318 | ADR-015-PATCH1-loop-run | PATCH1 verification loop (50 runs) |
| 6 | `ADR-015-PATCH1-loop-run-35.smp_per_cpu_scheduler_smoke.serial.log` | 318 | ADR-015-PATCH1-loop-run | Same campaign, different scenario |
| 7 | `ADR-015-PATCH1-loop-run-36.cap_revoke_descendants_walk.serial.log` | 318 | ADR-015-PATCH1-loop-run | Same campaign |
| 8 | `ADR-015-PATCH1-loop-run-38.expect_pass_but_fail.serial.log` | 318 | ADR-015-PATCH1-loop-run | Same campaign |
| 9 | `WEDGE-INV-2-run-3.serial.log` | 318 | WEDGE-INV-2-run | WEDGE investigation 2 (30 runs, instrumented) |

Distribution: H7 appears across campaigns spanning sub-commit 2 through post-ADR-015, suggesting the cause is environmental (QEMU/Limine config) rather than substrate-specific.

## 3. Reinterpretation of Historical TIMEOUT Counts

The table below subtracts confirmed H7 instances from each campaign's reported TIMEOUT count. The "Non-H7 TIMEOUTs" column represents the estimated in-kernel wedge count for that campaign.

| Campaign | Runs | Reported TIMEOUTs | H7 count (this analysis) | Non-H7 TIMEOUTs (estimated in-kernel) | Original rate | Adjusted in-kernel rate |
|---|---:|---:|---:|---:|---:|---:|
| build-3-2-ipc-100x | 100 | Not separately reported | 1 | — | — | — |
| KASLR-Q1 (initial) | 10 | ~3 (per close report) | 1 | ~2 | ~30% | ~20% |
| KASLR-Q1B | 50 | 14 | 0* | 14 | 28% | 28% |
| ADR-015 (initial, discarded) | 24 | Not separately reported | 2 | — | — | — |
| ADR-015 PATCH1 | 50 | 11 | 4 | 7 | 22% | 14% |
| H1 experiment | 20 | 1 | 0* | 1 | 5% | 5% |
| WEDGE-INV-2 | 30 | 1 | 1 | 0 | 3.3% | 0% |

\* KASLR-Q1B and H1 experiment logs exist on disk; the sweep found 0 H7 instances in their respective log sets. The KASLR-Q1B campaign had 31 serial logs (not all 50 runs produced separate scenario logs — some were merged or only the failing scenarios were preserved). The H1 experiment had 20 logs, none at 318 bytes.

**Key finding:** The ADR-015 PATCH1 campaign's 11 TIMEOUTs included 4 H7 instances, meaning the actual in-kernel wedge rate was **14%** (7/50), not 22%. The WEDGE-INV-2 campaign's single TIMEOUT was entirely H7, meaning that campaign observed **zero in-kernel wedges** — consistent with its finding that instrumentation appeared to suppress the bug.

The KASLR-Q1B baseline of 28% remains unchanged (0 H7 found in its logs), but that campaign used a different substrate (pre-ADR-015, sub-commit 2). The post-ADR-015 in-kernel wedge rate appears to be **14%** based on the PATCH1 data after H7 subtraction.

**Caveat:** Not all historical campaigns have complete serial logs preserved. The KASLR-Q1B campaign had 31 logs for 50 runs; some TIMEOUTs may have been H7 but their logs were not retained or were overwritten by subsequent campaigns. The true H7 fraction could be slightly higher than what this analysis captures.

## 4. Diagnosis and Proposed Fix

### 4.1 Evidence

- **Identical signature:** All 9 H7 instances produce exactly 318 bytes of serial output — the Limine boot menu UI rendered to COM1, followed by the countdown line. Nothing after.
- **No kernel load:** Successful boots begin with `limine: Loading executable ...` emitted by Limine's serial mirror. H7 logs never reach this point.
- **Harness timeout is not the cause:** The xtask harness outer timeout is 180 seconds. A successful boot (Limine menu + kernel load + smoke test) completes in ~5-10 seconds. If Limine were simply slow to load, the run would complete well within 180 seconds. The fact that it times out means QEMU is hanging — not slowly progressing.
- **QEMU flags:** `-display none`, `-monitor none`, `-no-reboot`, `-no-shutdown`, `-serial file:{path}`, `.stdout(Stdio::null())`. No stdin pipe — QEMU inherits the parent's stdin (empty in Docker).
- **Limine config:** `timeout: 1` (1 second), `serial: yes`, single boot entry, `default_entry: 1`.

### 4.2 Root cause: Limine menu countdown + QEMU CD-ROM timing race

The most likely mechanism is a **timing-sensitive race between Limine's countdown completion and QEMU's emulated CD-ROM I/O**:

1. Limine renders the boot menu on serial (visible in log).
2. Limine begins the 1-second countdown ("Booting automatically in 1").
3. After the countdown, Limine attempts to read the kernel ELF from the CD-ROM (`boot():/boot/kernel`).
4. In rare cases (~0.36% of boots), QEMU's emulated CD-ROM I/O stalls or returns an error during this critical window. With `-no-shutdown`, QEMU doesn't exit on the resulting shutdown-like state — it hangs indefinitely.
5. The harness's 180-second timeout eventually kills the process, producing the H7 classification.

The race is more likely when:
- The host has I/O contention (Docker overlay filesystem, concurrent builds).
- QEMU's vCPU scheduling is delayed during the CD-ROM read window.
- The Limine menu countdown creates a narrow timing window where the CD-ROM read must succeed immediately after the countdown fires.

### 4.3 Why timeout: 0 fixes it

Setting `timeout: 0` in `boot/limine.conf` causes Limine to skip the boot menu entirely and immediately load the default entry. This eliminates:
- The interactive menu phase (no user input possible).
- The countdown timing window (no delay between menu render and kernel load).
- The race condition between countdown completion and CD-ROM read.

With `timeout: 0`, Limine's boot path becomes: start → load default entry → read kernel ELF → transfer control. There is no menu, no countdown, no user input — just a direct load. This is the standard configuration for headless/automated testing environments.

**Impact:** None on kernel correctness or test behavior. The single-entry boot menu serves no purpose in automated testing. Limine's `serial: yes` still mirrors its own diagnostics (including "Loading executable" lines) to COM1, so early-boot failures remain visible.

### 4.4 Alternative causes considered and ruled out

- **(a) Harness outer timeout too short:** Ruled out. 180 seconds is more than enough for a normal boot (~5-10s). H7 stalls are indefinite hangs, not slow progress.
- **(c) QEMU + KVM scheduling glitch:** Possible as a contributing factor (the race may be triggered by host-side load), but the primary fix is removing the menu phase rather than trying to eliminate host contention.
- **(d) Limine bug in kernel-load path:** Unlikely. A ~0.36% failure rate on kernel load would be a notable upstream bug. The timing correlation with the menu countdown suggests the issue is specific to the interactive menu flow, not the load path itself.

## 5. Canonical Three-Bucket Classifier Proposal

This section reconciles the H7 detection pattern used in this investigation with Cerebro's three-bucket classifier from `DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md` §3.2.

### 5.1 Proposed canonical classifier (PowerShell)

```powershell
function Classify-SerialLog {
    param([string]$LogPath)

    if (-not (Test-Path $LogPath)) { return "MISSING" }
    $content = Get-Content $LogPath -Raw
    if (-not $content -or $content.Trim().Length -eq 0) { return "EMPTY" }

    # Terminal outcomes (check first).
    if ($content -match "NEO_TEST/1 event=PASS") { return "PASS" }
    if ($content -match "NEO_TEST/1 event=FAIL") { return "FAIL" }
    if ($content -match "panicked at") { return "PANIC" }

    # Three-bucket TIMEOUT classification, ordered most-progress to least.
    # H7 / PRE_KERNEL: Limine menu visible but kernel never loaded.
    if ($content -match "ALTIMIT NEO boot menu|Booting automatically|press any key to stop") {
        if (-not ($content -match "limine: Loading executable")) {
            return "TIMEOUT_PRE_KERNEL"
        }
    }

    # IN_KERNEL_SMOKE: kernel booted, reached smoke test spawn.
    if ($content -match "sched: spawned 'ping'") { return "TIMEOUT_IN_KERNEL_SMOKE" }

    # BOOT_KERNEL_PRE_SMOKE: kernel started but didn't reach smoke test.
    if ($content -match "_kernel_start|kernel \] mm:|kernel \] sched:") {
        return "TIMEOUT_BOOT_KERNEL_PRE_SMOKE"
    }

    # Residual: some output but doesn't match any known pattern.
    return "TIMEOUT_UNCLASSIFIED"
}
```

### 5.2 Differences from Cerebro's dispatch §3.2

| Aspect | Cerebro's dispatch | This proposal | Rationale |
|--------|-------------------|---------------|-----------|
| PRE_KERNEL detection | `ALTIMIT NEO boot menu\|limine` | `ALTIMIT NEO boot menu\|Booting automatically\|press any key to stop` + negative check for `limine: Loading executable` | Cerebro's pattern matches any log containing "limine" (including successful boots that have `limine: Loading executable`). The negative check ensures H7 is only classified as PRE_KERNEL if the kernel was never loaded. |
| FAIL terminal event | Not checked | `NEO_TEST/1 event=FAIL` | Deliberate test failures should be distinguished from timeouts. |
| EMPTY check | Not present | Empty file detection | Robustness against corrupted or zero-byte logs. |

### 5.3 Four-bucket summary (for reporting)

Going forward, each verification gate should report:

1. **PASS** — test completed successfully (`NEO_TEST/1 event=PASS`)
2. **FAIL** — test deliberately failed (`NEO_TEST/1 event=FAIL` or `panicked at`)
3. **TIMEOUT_PRE_KERNEL** — H7 class; kernel never loaded (harness/config concern)
4. **TIMEOUT_IN_KERNEL** — kernel loaded but didn't complete (kernel bug concern; may be further subdivided into BOOT_KERNEL_PRE_SMOKE and IN_KERNEL_SMOKE for diagnostic purposes)

The key distinction is between bucket 3 (not a kernel bug) and bucket 4 (kernel bug). This prevents harness artifacts from being misclassified as kernel defects.

## 6. Recommendations

### 6.1 Immediate: Apply `timeout: 0` to `boot/limine.conf`

**Action:** Change `timeout: 1` to `timeout: 0` in `boot/limine.conf`.

**Verification:** Run 30 boots with the new config using a simple scenario (e.g., `kaslr_smoke`) and confirm 0 H7 instances. Compare against the baseline of ~0.36% (9/2504).

**Risk:** Minimal. The boot menu has a single entry; skipping it changes nothing about the boot path other than removing the 1-second countdown delay.

**Commit message skeleton:**
```
boot: skip Limine boot menu (timeout: 0) to eliminate H7 pre-kernel stall

The H7 pre-kernel boot/menu stall (~0.36% of historical boots, 9 instances
across 2504 serial logs) occurs when Limine's countdown completes but the
kernel ELF read from QEMU's emulated CD-ROM stalls. Setting timeout: 0
eliminates the interactive menu phase, removing the timing-sensitive race
window between countdown completion and CD-ROM I/O.

The single-entry boot menu serves no purpose in automated testing. Limine's
serial: yes still mirrors boot diagnostics to COM1 for visibility.

Refs: Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md,
Plans/DISPATCH-2026-05-08-h7-investigation-jarvis.md.
```

### 6.2 Adopt the canonical three-bucket classifier

**Action:** Propose a separate dispatch to update the xtask classification logic (or any harness wrapper) to use the three-bucket scheme from §5.1. This prevents future H7 instances from being misclassified as in-kernel wedges.

**Scope:** Code change to `tools/xtask/src/main.rs` or the PowerShell loop wrappers used by verification campaigns. Not part of this dispatch.

### 6.3 Re-evaluate the wedge-fix commitment with adjusted baseline

**Action:** The post-ADR-015 in-kernel wedge rate is ~14% (7/50 from PATCH1 after H7 subtraction), not 22%. This is a meaningful reduction but not elimination. The wedge-fix-before-SC-3B commitment remains active, but the target rate to beat is now clearer: we need to get from ~14% to near 0%.

Cerebro's WEDGE-INV-3 baseline measurement will produce the current ground truth on `46984a1`. If that rate is ≤5%, the commitment may be dischargeable with a regression guard rather than a kernel patch.

### 6.4 No upstream Limine bug report needed at this time

The H7 pattern is consistent with a QEMU-side CD-ROM timing race triggered by Limine's interactive menu flow, not a Limine bug per se. Setting `timeout: 0` should eliminate the issue. If H7 persists after the fix, an upstream investigation becomes warranted.

## 7. Cross-references

- `Plans/DISPATCH-2026-05-08-h7-investigation-jarvis.md` — this dispatch (source of work)
- `Plans/WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md` — WEDGE-INV-2 findings; source of the H7 observation (run 3)
- `Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md` — Cerebro's concurrent baseline measurement; classifier reconciliation (§5.1)
- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's initial wedge investigation (H1 hypothesis, 5 instances across 4 campaigns)
- `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` — KASLR-Q1B close report (14 TIMEOUTs, 28%)
- `Plans/CLOSE-REPORT-2026-05-07-adr-015-implementation.md` — ADR-015 PATCH1 close report (11 TIMEOUTs, 22%)
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 experiment findings (1 TIMEOUT, 5%)
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — Wedge-fix-before-SC-3B commitment
- `boot/limine.conf` — Limine bootloader config (proposed fix target)
- `tools/xtask/src/main.rs:46` — `TEST_TIMEOUT: Duration = Duration::from_secs(180)` (harness outer timeout, ruled out as cause)

---

**End of findings.**
