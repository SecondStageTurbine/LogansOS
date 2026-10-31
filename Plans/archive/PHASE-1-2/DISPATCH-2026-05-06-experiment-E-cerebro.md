# Dispatch: Experiment E — different scenario at SMP=4 to bisect "test-harness binary" axis

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 + Q5 restoration applied), unchanged since the EXP-D close.
**Substrate:** Experiment D passed 100/100. The hang requires the **test-harness kernel binary** at SMP=4 — QEMU flags are exonerated. Experiment E bisects "test-harness binary" into "any compiled-in test_harness module" vs "specific to the IPC scenario."
**Type:** Diagnostic. NOT a fix attempt. NOT a commit. Working tree stays uncommitted.

---

## 0. Standing instructions

Same as Experiments A, B, D.

1. **Do NOT commit anything.** Working tree stays uncommitted.
2. **Do NOT attempt fixes.** Hypotheses go in close report only.
3. **Do NOT modify scheduler / IPC / per-CPU code.**
4. **Do NOT modify the kernel binary source.** This experiment varies which scenario gets compiled in — same code, different `cfg` selection.
5. **Capture verbatim output.**

---

## 1. The question

Experiment D ruled out QEMU flags (100/100 PASS for bare-boot kernel under test-harness QEMU flags). The hang requires the test-harness kernel binary specifically.

The test-harness binary differs from bare-boot in two ways relevant here:

- **`mod test_harness;` is compiled in** (gated by `#[cfg(feature = "neo-test")]`). Adds ~1500 lines of kernel-resident code: scenario dispatch, `frame()`/`pass()`/`fail()` helpers, `EXPECTED_PAGE_FAULT`/`EXPECTED_CR2` statics, `handle_page_fault`, `unexpected_trap`, `panic_failed`, plus all the per-scenario gated helpers.
- **The active scenario feature** (`neo-test-ipc-smoke` for the failing config) gates additional code: in this case, the `_ipc_smoke()` call after `smoke_test()` returns, which calls `spawn_neo_init` — invoking the user-process spawn path, ELF loader, Pmap construction, and the IPC channel pre-spawn.

Experiment E answers: **does the hang reproduce with a different scenario whose kernel-side path is structurally simpler than `ipc_heap_kill_smoke`?**

If yes (other scenario hangs at similar rate): the trigger is `mod test_harness` mere presence. Cause is likely binary layout / size / static initializer effect on smoke_test's timing.

If no (other scenario passes 100/100 at SMP=4): the trigger is specific to `ipc_heap_kill_smoke`'s code path — most likely the user-process spawn (`spawn_neo_init`) interacting with the per-CPU scheduler refactor.

---

## 2. Choosing the control scenario

The right control is **`kaslr_smoke`**:

- Includes `mod test_harness` (same as `ipc_heap_kill_smoke`).
- `run_kaslr_smoke()` body is just two atomic reads (`executable_address()`), three guard checks, one serial println, `debug_exit(EXIT_SUCCESS)`. No IPC, no Pmap construction, no user process, no thread spawn beyond what `smoke_test` already did.
- Runs through `_kernel_start` → `init_traps` → `mm::init` → `bring_up_aps` → `init_bsp` → `smoke_test` → `run_selected_scenario` → `run_kaslr_smoke` → exit. Identical pre-smoke_test path to `ipc_heap_kill_smoke`. Diverges only AFTER smoke_test returns.
- The hang in run 81 occurred INSIDE `smoke_test`'s `reap()` call, BEFORE `run_selected_scenario` was reached. So if `kaslr_smoke` reaches the same point as `ipc_heap_kill_smoke` does (it must — same `smoke_test` code), and either both hang or only IPC hangs, the result is interpretable.

`kaslr_smoke` needs `limine_kaslr: Some(true)` per its scenario entry, but the production `limine.conf` already has `kaslr: yes`, so there's no observable difference — it's just the flag being explicit in the scenario table.

---

## 3. Experiment E — `kaslr_smoke` 100x at NEO_SMP=4

Use the same harness invocation that produced the 100x `ipc_heap_kill_smoke` run, just with the scenario name swapped:

```powershell
cd "E:\AI Automation\AltimitNeo"
$results = @()
for ($i = 1; $i -le 100; $i++) {
    Write-Output "=== E-Run $i ==="
    $env:NEO_SMP = "4"
    $r = docker compose run --rm neo cargo neo-test kaslr_smoke 2>&1 | Out-String

    # Detection: PASS frame for kaslr_smoke OR FAIL/timeout
    if ($r -match "NEO_TEST/1 event=PASS name=kaslr_smoke") {
        Write-Output "E-Run $i`: PASS"
        $results += "PASS"
    } else {
        Write-Output "E-Run $i`: FAIL"
        # Save the run's serial log if it exists
        $serial = "target/neo-test/kaslr_smoke.serial.log"
        if (Test-Path $serial) {
            Copy-Item $serial "target/verify/EXP-E-fail-run-$i.serial.log"
        }
        # Also save the xtask wrapper output for context
        $r | Out-File "target/verify/EXP-E-fail-run-$i.txt" -Encoding utf8
        $results += "FAIL"
        break
    }
}

$passCount = ($results | Where-Object { $_ -eq "PASS" }).Count
"EXP-E-PASS-COUNT: $passCount/100" | Out-File "target/verify/EXP-E-summary.txt"
Write-Output "EXP-E-PASS-COUNT: $passCount/100"
```

Adapt to whatever shell environment matches Experiments A/B/D. The detection criterion is the `NEO_TEST/1 event=PASS name=kaslr_smoke` line; absence of it (timeout, hang, FAIL frame, anything else) counts as FAIL and stops the loop.

---

## 4. What you do NOT do

- **Do not modify any source.** Including the Q5-restoration working tree, including `reap()` instrumentation. The kernel binary is whatever sub-commit 2 + Q5-restoration produces; the only thing varying across Experiments A, B, D, E is the runner config.
- **Do not run other scenarios.** Only `kaslr_smoke`. If E gives a clear answer, PM may dispatch a follow-up. If you start running `cap_*` or `smep_*` to satisfy curiosity, the time budget blows up and the dispatch shape becomes harder to reason about.
- **Do not adjust the timeout.** The xtask `TEST_TIMEOUT` of 180 seconds is what made the original failure visible at run 81. Same timeout here so a hang surfaces identically.
- **Do not commit anything.**

---

## 5. Result to capture

```
EXP-E-PASS-COUNT: N/100
First failure run: <number or "n/a">
Failing serial log: <path or "n/a">
Failing run's last serial line: <verbatim or "n/a">
```

If a failure happens, the failing serial log content matters. Specifically:

- Does it stop at `[ kernel ] sched: reaped 'pong'` like run 81 did? If yes, **same hang, same locus** — confirms the trigger is `mod test_harness` presence, not anything specific to IPC scenario code.
- Does it stop somewhere else? Different locus means a different bug; we're now looking at potentially two latent SMP issues. Surface clearly.
- Does the scenario produce a FAIL frame from `kaslr_smoke`'s own assertions (e.g., `kernel_base_not_higher_half`, `kaslr_not_randomized`)? Different failure mode entirely — that would suggest the hang is `kaslr_smoke`'s own bug, not the smoke-test hang. Surface clearly.

---

## 6. Decision rule

After this experiment:

- **E fails at any rate near 1.25%, hung at `reaped 'pong'`:** `mod test_harness` mere presence is the trigger. The bug is likely a binary-layout effect on smoke_test's timing (cache line layout, code locality, static initializer state). Next: PM-side analysis of what the test_harness module changes about kernel binary layout, possibly Experiment F (instrumented `reap()`) on the failing config to localize where in `reap()` the hang lands.

- **E fails at any rate near 1.25%, hung somewhere else:** two latent bugs. Stop and report; PM will need to think harder about the dispatch shape.

- **E fails at significantly higher rate (e.g., 1-in-10):** more reproducible, useful — shorter loop next time. Same `mod test_harness` interpretation but easier to investigate.

- **E passes 100/100:** the trigger is specific to `ipc_heap_kill_smoke`. Most likely the user-process spawn path (`spawn_neo_init`'s `process::spawn_user("neo-init", ...)` → ELF load → Pmap → first user thread → first context switch into ring 3 → `sys_spawn` worker → `sys_send` etc.) interacting with the per-CPU scheduler refactor. Next: PM dispatches Experiment G — run a different IPC-driving scenario to see if any IPC scenario hangs at the same rate, or only this specific one.

Stop and report regardless of result.

---

## 7. Time budget

Each `kaslr_smoke` run is fast — boots, runs smoke test (3-4s), runs the kaslr_smoke body (~microseconds), exits via debug_exit. Whole test cycle is ~10-15 seconds including ISO build (which gets cached after the first run; subsequent runs only re-link if features change, which they don't).

**Important caveat for harness rebuild caching.** The test harness rebuilds the kernel for each scenario (different feature set), and rebuilds the ISO. If `ipc_heap_kill_smoke` was the last scenario built before this experiment, the first run of this experiment will rebuild for `kaslr_smoke` features and incur a fresh kernel + ISO build (~60-90 seconds). Subsequent runs reuse the cached `kaslr_smoke` build. Plan for ~90s overhead on run 1, then ~10s per run × 99 = ~16 minutes total.

If you observe failures early, stop and report — we have plenty of data without finishing all 100.

---

## 8. Why this experiment

The bisection chain so far:

- A + B narrowed to "needs SMP + test harness."
- D narrowed to "test-harness binary, not QEMU flags."
- E narrows to "any test-harness binary, or specific to IPC scenario."

Each step has cut the hypothesis space roughly in half. After E, we either know it's a generic `mod test_harness` binary effect (one investigation path) or we know it's IPC-specific (a different investigation path). Both narrowings are interpretable; both inform the next step cleanly.

This is exactly the bisection discipline the original sub-commit 2 dispatch's §6 anticipated when it said "Don't try heroics. Stop and surface to PM." The §Q5 lesson keeps applying: verify, don't assume, keep the steps small.

---

**End of dispatch.**
