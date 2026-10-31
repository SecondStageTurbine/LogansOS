# Dispatch: Experiment D — bare-boot kernel under test-harness QEMU flags

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 + Q5 restoration applied), unchanged since the EXP-CLOSE-REPORT.
**Substrate:** Experiments A and B passed 100/100 each. The hang correlates with "test-harness configuration", but we don't yet know whether the trigger is (a) the QEMU command-line differences between `cargo neo-test` and `cargo run-headless`, or (b) the kernel binary differences (compiled-in `test_harness` module + `cfg(feature = "neo-test")` paths). Experiment D pins this down.
**Type:** Diagnostic. NOT a fix attempt. NOT a commit. Working tree stays uncommitted.

---

## 0. Standing instructions

Same as the previous isolation dispatch.

1. **Do NOT commit anything.** Working tree stays uncommitted.
2. **Do NOT attempt fixes.** Hypotheses go in the close report; live changes do not.
3. **Do NOT modify scheduler / IPC / per-CPU code.**
4. **Do NOT modify the kernel binary at all.** This experiment varies QEMU flags only.
5. **Capture verbatim output.**

---

## 1. The question

PM did the code-inspection step that the EXP-CLOSE-REPORT proposed as Option 2. Specifically:

- **`cfg(feature = "neo-test")` does NOT reach SMP-sensitive code.** The flag is only used in `kernel/src/main.rs` (gates the `mod test_harness` import and the `run_selected_scenario` call after `smoke_test`) and `kernel/src/panic.rs` (gates panic-handler differences). It does not touch `sched/`, `ipc/`, `mm/slab.rs`, or `arch/x86_64/`. Hypothesis 3 from your close report is RULED OUT by inspection.
- **Test-harness scenario setup runs strictly AFTER `smoke_test` returns**, on the same BSP main thread. There is no concurrency between them. Hypothesis 1 is also RULED OUT by inspection.
- **The QEMU command-line for `cargo neo-test` differs from `cargo run-headless` in three significant ways:**
  1. `-serial file:<path>` vs `-serial stdio`. Different host-side I/O backend with very different stall semantics under load.
  2. `-monitor none` (test only). Disables QEMU's internal monitor.
  3. `-device isa-debug-exit,iobase=0xf4,iosize=0x04` (test only). The debug-exit device, used for kernel→host exit-code signaling.

Your hypothesis 4 had the right intuition (QEMU host-side perturbation) but identified the wrong specific cause — `isa-debug-exit` is one of three differences, not the only one.

The question Experiment D answers: **does the hang reproduce when the bare-boot kernel binary runs under the test-harness QEMU flag set?**

- **If YES:** the cause is QEMU command-line flags, not the kernel binary. We then narrow to which of the three flag differences matters.
- **If NO:** the cause is kernel binary differences. We then proceed to Experiment C (instrumented `reap()`) on the original failing config.

Either way, this experiment tells us where to dig next.

---

## 2. Experiment D — the bare-boot kernel under test-harness QEMU flags

### 2.1 Setup

You need:

- The **bare-boot kernel ISO** (the one Experiment B used; built via `cargo run-headless` or equivalent path that does NOT enable `feature = "neo-test"` or any per-scenario feature). Confirm: `boot/altimit-neo.iso` after a `cargo iso` run with no features should be the bare-boot binary. The kernel binary inside the ISO must NOT have `mod test_harness` compiled in.
- A QEMU invocation that mirrors `cargo neo-test`'s flags **except** the `-cdrom` points at the bare-boot ISO.

### 2.2 Verify the bare-boot ISO

Before the loop, confirm the ISO is what we think it is:

```
cargo iso       # produces boot/altimit-neo.iso with no test features
ls -la boot/altimit-neo.iso
```

The ISO size should match what was produced for Experiment B's runs. (If you have a saved Experiment B ISO and it's still on disk, even better — use that one verbatim.)

### 2.3 The 100x loop

Use the test-harness QEMU command line **without** rebuilding the kernel between runs. The QEMU launch shape is:

```
qemu-system-x86_64 \
    -machine q35 \
    -cpu qemu64,+rdrand,+smep,+smap \
    -m 512M \
    -display none \
    -monitor none \
    -no-reboot \
    -no-shutdown \
    -boot d \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -serial file:<path-to-per-run-serial-log> \
    -smp 4 \
    -cdrom <path-to-bare-boot-iso>
```

The matching PowerShell script:

```powershell
cd E:\AI Automation\AltimitNeo
$iso = "boot\altimit-neo.iso"  # Confirm this is bare-boot, not test-harness
$results = @()
for ($i = 1; $i -le 100; $i++) {
    Write-Output "=== D-Run $i ==="
    $serial = "target\verify\EXP-D-run-$i.serial.log"
    if (Test-Path $serial) { Remove-Item $serial }

    # Launch QEMU with test-harness flag set, but bare-boot ISO.
    # 30s timeout per run — bare boot completes smoke_test in ~3-4s,
    # so 30s is generous. Use a short timeout here because there's no
    # test scenario; the kernel halts forever after rendering boot frame.
    $proc = Start-Process -FilePath "qemu-system-x86_64" `
        -ArgumentList @(
            "-machine", "q35",
            "-cpu", "qemu64,+rdrand,+smep,+smap",
            "-m", "512M",
            "-display", "none",
            "-monitor", "none",
            "-no-reboot",
            "-no-shutdown",
            "-boot", "d",
            "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-serial", "file:$serial",
            "-smp", "4",
            "-cdrom", $iso
        ) `
        -PassThru -NoNewWindow

    $finished = $proc.WaitForExit(30000)  # 30s
    if (-not $finished) {
        Stop-Process -Id $proc.Id -Force
        # Wait briefly for stop to take effect
        Start-Sleep -Milliseconds 500
    }

    # Inspect the serial log for the smoke test completion line.
    if (Test-Path $serial) {
        $content = Get-Content $serial -Raw
        if ($content -match "sched: smoke test passed") {
            Write-Output "D-Run $i`: PASS"
            $results += "PASS"
        } else {
            Write-Output "D-Run $i`: FAIL (no smoke test passed line)"
            $results += "FAIL"
            Write-Output "Saving full log to EXP-D-fail-run-$i.serial.log"
            Copy-Item $serial "target\verify\EXP-D-fail-run-$i.serial.log"
            break
        }
    } else {
        Write-Output "D-Run $i`: FAIL (no serial log produced)"
        $results += "NO_LOG"
        break
    }
}

$passCount = ($results | Where-Object { $_ -eq "PASS" }).Count
"EXP-D-PASS-COUNT: $passCount/100" | Out-File "target\verify\EXP-D-summary.txt"
Write-Output "EXP-D-PASS-COUNT: $passCount/100"
```

Adapt the script to the actual path / shell environment you're using. If you're running QEMU through `docker compose` for consistency with Experiments A and B, do that instead — but make sure the QEMU **flags** match what's specified above, not what `cargo run-headless` would produce.

### 2.4 Critical detail: WHICH ISO

This is the key. The kernel binary inside the ISO must be the bare-boot binary (no `feature = "neo-test"`). The QEMU flags must be the test-harness flags.

To verify the binary, before the loop runs, do:

```
strings boot/altimit-neo.iso | grep -c "neo_test\|test_harness\|run_selected_scenario"
```

A bare-boot kernel should return 0 (or a very low number — strings can produce false positives but the test_harness module code is substantial). A test-harness kernel returns dozens to hundreds.

If the count is high, the ISO is the wrong one — re-build with no features and re-verify.

---

## 3. What you do NOT do

- **Do not rebuild the kernel between runs.** The whole point is testing the same binary against varying QEMU flags. The 100 runs use one ISO.
- **Do not modify kernel source code.** Including the `[ DBG ]` instrumentation from the previous dispatch's Experiment C.
- **Do not run the test-harness scenario alongside the bare-boot binary.** That's a different experiment.
- **Do not commit anything.**

---

## 4. Result you need to capture

```
EXP-D-PASS-COUNT: N/100
First failure run: <number or "n/a">
Failing serial log: <path or "n/a">
ISO verified: bare-boot (string check returned <N>)
```

If a failure happens, the failing serial log content matters. Specifically, does it stop at the same line (`reaped 'pong'`) as run 81 of the original failing config?

---

## 5. Decision rule

After this experiment:

- **D fails at any rate:** the hang is QEMU-command-line-related, not kernel-binary-related. Next step: a follow-up experiment that varies each of the three flag differences independently to identify which one matters. PM dispatches the follow-up.
  - If the failure rate matches the original ~1.25%, even better — same root cause.
  - If the failure rate is significantly different (e.g., much higher or much lower), there's an interaction between QEMU flags and kernel binary; we'd need both axes to fully pin it.
- **D passes 100/100:** the hang is kernel-binary-related. Next step: Experiment C (instrumented `reap()`) on the original failing config. PM dispatches.

Stop and report after this experiment regardless of result.

---

## 6. Time budget

Each run is bare-boot, no test scenario, ~3-4 seconds of useful work then halt-forever. With 30s timeout each, worst case is 50 minutes for 100 runs (if every run timed out). Realistic case is 6–12 minutes (each run completes smoke test in seconds; QEMU takes a moment to spawn and tear down).

If you observe rapid failures (e.g., D fails at run 5), stop and report immediately — we have plenty of data.

---

## 7. Why this experiment, why now

The EXP-CLOSE-REPORT correctly localized the hang to the SMP+test-harness intersection. Experiment D is the next bisection: which axis of "test-harness configuration" is the trigger? QEMU flags or kernel binary?

This is the right experiment to run BEFORE Experiment C because:

- **C tells us WHERE in the kernel the hang lands.** That's useful only after we know whether the kernel is even the wrong place to be looking.
- **D tells us WHAT environment the hang requires.** If it's QEMU flags, the kernel is fine and the entire investigation moves to "what does our kernel do that QEMU's host-side serial backend doesn't tolerate" — a very different debugging conversation.
- **D is cheap.** No kernel source changes; one ISO build; 100 QEMU spawns. No risk of perturbing kernel timing.
- **C is invasive.** Adding `serial_println!` calls in `reap()` perturbs the very timing window we're investigating. We want to do C only if we have to, and only after D rules out the simpler explanation.

The shape of this investigation matches good debugging discipline: vary one axis at a time, narrow before instrumenting, explicit decision rules at each step.

---

**End of dispatch.**
