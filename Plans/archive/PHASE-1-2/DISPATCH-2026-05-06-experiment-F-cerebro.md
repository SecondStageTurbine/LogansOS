# Dispatch: Experiment F — different IPC scenario at SMP=4 to bisect "ipc_heap_kill_smoke specifically" vs "any scenario that calls spawn_neo_init"

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 + Q5 restoration applied), unchanged since the EXP-E close.
**Substrate:** Experiment E passed 100/100 with `kaslr_smoke`. The hang is NOT triggered by generic `mod test_harness` presence. It is specific to `ipc_heap_kill_smoke` OR scenarios that share its critical code path (spawning neo-init via `spawn_neo_init`). This experiment bisects which.
**Type:** Diagnostic. NOT a fix attempt. NOT a commit. Working tree stays uncommitted.

---

## 0. Standing instructions

Same as Experiments A, B, D, E.

1. **Do NOT commit anything.** Working tree stays uncommitted.
2. **Do NOT attempt fixes.** Hypotheses go in close report only.
3. **Do NOT modify scheduler / IPC / per-CPU code.**
4. **Do NOT modify the kernel binary source.** This experiment varies which scenario gets compiled in.
5. **Capture verbatim output.**

---

## 1. The question

Experiments D and E narrowed:

- **D:** hang requires the test-harness kernel binary (not bare-boot under test-harness QEMU flags).
- **E:** hang does NOT reproduce with `kaslr_smoke` (which has `mod test_harness` compiled in but doesn't call `spawn_neo_init`).

The remaining differential between `ipc_heap_kill_smoke` (hangs ~1-in-80) and `kaslr_smoke` (passes 100/100) is what code paths their respective `cfg(feature = ...)` arms light up — specifically:

- `ipc_heap_kill_smoke` activates the `_ipc_smoke()` call in `run_selected_scenario`, which IS `spawn_neo_init`. That function loads neo-init via `process::spawn_user`, constructs a user Pmap, transitions to ring 3, etc.
- `kaslr_smoke` activates `run_kaslr_smoke()` instead, which does NOT call `spawn_neo_init`. Rust's dead-code analysis likely strips `spawn_neo_init` from the `kaslr_smoke` kernel binary.

But the hang occurs in `smoke_test()` BEFORE either scenario's body runs. So the differential isn't about what runs — it's about what's in the binary. Either:

- The presence of `spawn_neo_init` (and its transitive callees: ELF loader, Pmap construction, capability table init paths) in the kernel image perturbs cache/memory layout in a way that makes `smoke_test`'s `reap()` race differently, OR
- Something specific to `ipc_heap_kill_smoke`'s compile config that's NOT shared with other scenarios that also call `spawn_neo_init`.

Experiment F answers: **does another scenario that also calls `spawn_neo_init` hang at the same rate as `ipc_heap_kill_smoke`?**

If yes (cap_mint_and_use also hangs ~1-in-80): the trigger is "any scenario whose binary contains spawn_neo_init code." That's a sharp result — we'd know the binary-layout effect is real, and we'd next instrument `reap()` to localize the hang point (Experiment G, deferred until after F).

If no (cap_mint_and_use passes 100/100): the trigger is something ipc_heap_kill_smoke-specific that goes BEYOND just calling spawn_neo_init. That would be surprising and would re-open the hypothesis space; we'd dig into what's compiled-in differently.

---

## 2. Choosing the control scenario

The right pick is **`cap_mint_and_use`**:

- Activates the `cfg(any(feature = "neo-test-cap-mint-and-use", ...))` arm in `run_selected_scenario`, which calls `_ipc_smoke()` — i.e., `spawn_neo_init`. So the spawn path is in the binary and gets exercised.
- Scenario dispatch in `test_harness.rs` is structurally identical to `ipc_heap_kill_smoke`'s (`_ipc_smoke` arm). If F passes, the differential is between two structurally-identical arms; that's information.
- Its `user_feature: Some("cap-mint-and-use")` rebuilds neo-init/neo-worker with a different feature flag, but the **kernel-side** path is the same: load neo-init via `process::spawn_user`, run `spawn_neo_init` to completion.
- Cap scenarios are well-established and known-passing under SMP=1; if it hangs at SMP=4, the hang is real and not a flaky scenario itself.

Note: this picks a scenario whose user binaries are DIFFERENT from `ipc_heap_kill_smoke` (because of the `cap-mint-and-use` user feature). That's intentional — if F passes at 100/100, we'd then know the user-binary differences are NOT what perturb smoke_test (since the user binaries are loaded by Limine at boot, before the kernel runs, with the same memory-layout effects). If F fails at ~1-in-80, then the kernel-side `spawn_neo_init` code-path presence is the trigger.

---

## 3. Experiment F — `cap_mint_and_use` 100x at NEO_SMP=4

Same shape as Experiment E:

```powershell
cd "E:\AI Automation\AltimitNeo"
$results = @()
for ($i = 1; $i -le 100; $i++) {
    Write-Output "=== F-Run $i ==="
    $env:NEO_SMP = "4"
    $r = docker compose run --rm neo cargo neo-test cap_mint_and_use 2>&1 | Out-String

    if ($r -match "NEO_TEST/1 event=PASS name=cap_mint_and_use") {
        Write-Output "F-Run $i`: PASS"
        $results += "PASS"
    } else {
        Write-Output "F-Run $i`: FAIL"
        $serial = "target/neo-test/cap_mint_and_use.serial.log"
        if (Test-Path $serial) {
            Copy-Item $serial "target/verify/EXP-F-fail-run-$i.serial.log"
        }
        $r | Out-File "target/verify/EXP-F-fail-run-$i.txt" -Encoding utf8
        $results += "FAIL"
        break
    }
}

$passCount = ($results | Where-Object { $_ -eq "PASS" }).Count
"EXP-F-PASS-COUNT: $passCount/100" | Out-File "target/verify/EXP-F-summary.txt"
Write-Output "EXP-F-PASS-COUNT: $passCount/100"
```

Adapt to the same shell environment as A/B/D/E. Detection: `NEO_TEST/1 event=PASS name=cap_mint_and_use` line.

---

## 4. What you do NOT do

- **Do not modify any source.** Including `reap()` instrumentation.
- **Do not run any other scenario.** Only `cap_mint_and_use`. If results are unclear, PM dispatches a follow-up — don't curiosity-run others.
- **Do not adjust the timeout.** 180s xtask timeout matches all prior experiments.
- **Do not commit anything.**

---

## 5. Result to capture

```
EXP-F-PASS-COUNT: N/100
First failure run: <number or "n/a">
Failing serial log: <path or "n/a">
Failing run's last serial line: <verbatim or "n/a">
```

If a failure happens, the failing serial log content matters. Specifically:

- Does it stop at `[ kernel ] sched: reaped 'pong'` like run 81 of the original config did? Same hang signature → same root cause → bisection complete on this axis.
- Does it stop somewhere else? Different signature → different bug or different manifestation; surface with literal lines.
- Does the cap scenario produce a FAIL frame from its own assertions? Different failure mode → not the smoke-test hang; surface with literal lines.

---

## 6. Decision rule

After this experiment:

- **F fails at any rate near 1.25%, hung at `reaped 'pong'`:** the trigger is "any scenario that calls `spawn_neo_init`." Likely cause: presence of `spawn_neo_init` (and its transitive callees — ELF loader, Pmap construction, etc.) in the kernel binary perturbs `smoke_test`'s `reap()` timing. **Next step:** Experiment G — instrumented `reap()` on the original failing config (`ipc_heap_kill_smoke` at SMP=4). PM dispatches.

- **F fails at significantly different rate or different signature:** mixed result; surface literal lines and stop.

- **F passes 100/100:** the trigger is something specific to `ipc_heap_kill_smoke`'s exact compilation that other spawn_neo_init scenarios don't share. Hypothesis space re-opens; PM thinks harder about what to dispatch next. Likely candidate: the specific `_ipc_smoke()` cfg-arm in `run_selected_scenario` does something subtly different that triggers it (re-read arm).

Stop and report regardless of result.

---

## 7. Time budget

Same as Experiment E: ~10–15 seconds per run, ~90s overhead on run 1 for the initial cap_mint_and_use kernel + ISO build, then ~10s × 99 = ~16 minutes total. Stop early if failures appear.

If you observe rapid failures (e.g., fails at run 5), stop and report immediately — we have plenty of data.

---

## 8. Why this experiment, why now

The bisection chain so far:

- A + B narrowed to "needs SMP + test harness."
- D narrowed to "test-harness binary, not QEMU flags."
- E narrowed to "specific to scenarios beyond just `mod test_harness`."
- F narrows to "any spawn_neo_init scenario, or specific to ipc_heap_kill_smoke."

Each step has cut the hypothesis space roughly in half with non-perturbing experiments. F is the last cheap bisection; after this, we move to instrumentation (Experiment G — the originally-deferred Experiment C, now properly scoped to whichever scenario F identifies as the right one to instrument).

This is exactly the discipline the original sub-commit 2 dispatch's §6 anticipated. Stay narrow, surface promptly, don't synthesize when the evidence isn't in.

---

**End of dispatch.**
