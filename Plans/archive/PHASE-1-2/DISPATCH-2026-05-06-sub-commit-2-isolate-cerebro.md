# Dispatch: Isolate the sub-commit-2 in-kernel smoke-test hang

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 + Q5 mechanism restoration applied).
**Substrate:** 100x ipc_heap_kill_smoke gate failed at run 81 with a kernel hang in the in-kernel scheduler smoke test, NOT in the IPC scenario. See `Plans/PM-DECISION-2026-05-06-sub-commit-2-smoke-hang.md` for the diagnosis.
**Type:** Diagnostic investigation. NOT a fix attempt. NOT a commit. Working tree stays uncommitted.

---

## 0. Standing instructions

These are even more important than usual because this is investigation, not implementation.

1. **Do NOT commit anything during this dispatch.** Working tree stays in its current state until PM has the diagnostic data and decides next steps.
2. **Do NOT attempt fixes.** If you have a hunch about the cause, write it down in your close report; don't act on it. Hunches without root-cause evidence have already cost us time on this sub-commit.
3. **Do NOT modify scheduler / IPC / per-CPU code.** Instrumentation that goes into the close report is fine; live changes that would alter scheduler timing are not. The bug is timing-sensitive at 1-in-80; touching scheduler timing risks masking it.
4. **Run experiments in the order specified.** Each experiment's result feeds the next decision. Don't skip ahead.
5. **Capture verbatim output.** The hang's signature is "no further serial output after `[ kernel ] sched: reaped 'pong'`". We need to know if other experiments produce different signatures.

---

## 1. The question we're answering

The hang appeared 1-in-80 in the 100x ipc_heap_kill_smoke loop on NEO_SMP=4. Sub-commit 2 changed scheduler internals across the SMP boundary; pre-sub-commit-2 had no such hang. We need to localize the hang's necessary conditions before deciding whether to fix it, work around it, or partially roll back sub-commit 2.

Three orthogonal questions structure the experiments below:

- **Q-A: SMP-specific or sub-commit-2-specific?** (Experiment A: NEO_SMP=1 100x.)
- **Q-B: Scenario-specific or kernel-side smoke test alone?** (Experiment B: 100x boots WITHOUT a test scenario, just kernel boot + smoke test + halt.)
- **Q-C: Where exactly in the for-loop does it hang?** (Experiment C: minimal instrumentation that, on a fresh failure, will show whether the hang is before or after `Thread::free`.)

Each experiment is small. Run them in order. Stop and report after each one before moving to the next.

---

## 2. Experiment A — NEO_SMP=1 100x ipc_heap_kill_smoke

Goal: determine whether the hang requires SMP at all.

```
# In the same shell environment you used for the failed 100x run:
for ($i=1; $i -le 100; $i++) {
    Write-Output "=== A-Run $i ==="
    $env:NEO_SMP = "1"
    $r = docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke 2>&1
    if ($r -match "PASS") {
        Write-Output ("A-Run {0}: PASS" -f $i)
    } else {
        Write-Output ("A-Run {0}: FAIL" -f $i)
        Write-Output $r
        # capture serial log
        Copy-Item "target/verify/build-3-2-restore-ipc-100x-run-$i.serial.log" `
                  "target/verify/EXP-A-fail-run-$i.serial.log" -ErrorAction SilentlyContinue
        break
    }
}
```

Adapt to whatever the actual harness shape is. Save the summary to `target/verify/EXP-A-summary.txt`.

**Result you need to capture:**

- `EXP-A-PASS-COUNT = N/100`. If 100/100, that's the answer.
- If any FAIL, the run number and a copy of the failing serial log.

**Stop and report after this experiment.** Do NOT proceed to B or C until PM has the result.

### Decision rule

- **A passes 100/100:** the hang is SMP-specific. Proceed to Experiment B.
- **A fails at any rate ≥ 1/100:** the hang is not SMP-specific — sub-commit 2's scheduler-internals refactor introduced a single-CPU-reproducible bug. We isolate differently. Proceed to Experiment B but the framing changes to "the bug is single-CPU-reachable".
- **A fails at significantly higher rate than NEO_SMP=4 did (e.g., 1-in-10):** also single-CPU-reachable, but more easily reproducible — useful, since it speeds investigation.

---

## 3. Experiment B — 100x kernel-only boot, no test scenario

Goal: determine whether the hang requires a test scenario at all, or whether it's just the in-kernel smoke test path that's wedging.

The hang in run 81 happened before any test-scenario code ran. It was inside `sched::smoke_test()` — specifically inside the first `reap()` call after pong's exit. If that's all that's needed to reproduce, we don't need ANY test scenario; we just need to boot 100 times.

Use `cargo neo-tty` (or whatever the bare-boot kernel runner is — check the existing xtask harness for the right command). Kernel boots, runs through `_kernel_start`, runs `sched::smoke_test()`, then halts (or whatever the steady-state-no-test behavior is). If the hang reproduces here, we've confirmed it's not test-runner artifact; it's the kernel's own smoke test.

```
for ($i=1; $i -le 100; $i++) {
    Write-Output "=== B-Run $i ==="
    $env:NEO_SMP = "4"
    # Use `cargo neo-tty` style invocation; capture serial output and exit code
    $r = docker compose run --rm neo cargo neo-tty 2>&1 | Out-String
    # Detect success: "smoke test passed" line appears
    if ($r -match "sched: smoke test passed") {
        Write-Output ("B-Run {0}: PASS (smoke test completed)" -f $i)
    } else {
        Write-Output ("B-Run {0}: FAIL" -f $i)
        # Capture full output
        $r | Out-File "target/verify/EXP-B-fail-run-$i.txt" -Encoding utf8
        break
    }
}
```

If `cargo neo-tty` is not the right harness (it may not auto-halt and may need a manual interrupt), substitute a 30-second timeout per run instead. The detection criterion is whether the kernel emitted `[ kernel ] sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=N)` within 30 seconds of boot.

**Result to capture:**

- `EXP-B-PASS-COUNT = N/100`.
- Failing runs: serial log copied to `target/verify/EXP-B-fail-run-N.txt`.

**Stop and report after this experiment.** Do NOT proceed to C.

### Decision rule

- **B passes 100/100 (no smoke-test hang in 100 boots without test scenario):** very interesting — the hang requires the test-scenario harness for some reason. Possibly the harness's own setup runs concurrent with smoke_test in a way that perturbs it. We dig into the harness next.
- **B fails at any rate:** the hang is intrinsic to the kernel's smoke test under sub-commit 2. The test harness is not implicated. We focus on Experiment C.

---

## 4. Experiment C — minimal instrumentation in `reap()`

ONLY run this experiment if Experiments A and B both indicate the hang is reproducible without the test harness AND PM has confirmed proceeding.

Goal: pin the hang to a specific point in the for-loop iteration in `reap()`.

Add four extra `serial_println!` calls inside the for loop body, marked as instrumentation that will be removed before commit:

```rust
pub fn reap() {
    let sched = current_scheduler();
    if sched.zombie_count.load(Ordering::Acquire) == 0 {
        return;
    }
    let drained: VecDeque<Box<Thread>> = without_interrupts(|| {
        let mut s = sched.inner.lock();
        let taken = mem::take(&mut s.zombies);
        sched.zombie_count.fetch_sub(taken.len(), Ordering::Release);
        taken
    });
    serial_println!("[ DBG ] reap: drained {} zombie(s)", drained.len());
    for zombie in drained {
        let pid = zombie.pid;
        let id = zombie.id.0;
        let name = zombie.name;
        serial_println!("[ DBG ] reap: pre-println id={} name='{}'", id, name);
        serial_println!("[ kernel ] sched: reaped '{}' (id={})", name, id);
        serial_println!("[ DBG ] reap: pre-Thread::free id={}", id);
        Thread::free(zombie);
        serial_println!("[ DBG ] reap: post-Thread::free id={}", id);
        super::process::on_thread_reap(pid);
        serial_println!("[ DBG ] reap: post-on_thread_reap id={}", id);
    }
    serial_println!("[ DBG ] reap: loop end");
}
```

(Adjust to match the actual shape of `reap()` in the working tree. The four `[ DBG ]` lines bracket the operations on each zombie.)

Re-run the 100x ipc_heap_kill_smoke gate on NEO_SMP=4. Save outputs to `target/verify/EXP-C-*`.

**Result to capture:**

- The full serial log of any failing run, showing where exactly the `[ DBG ]` trail stops.
- The expected stop point distinguishes:
  - `pre-Thread::free` printed but `post-Thread::free` missing → hang inside `Thread::free` (slab/heap path).
  - `post-Thread::free` printed but next iteration's `pre-println` missing → hang in iterator advance or implicit Drop chain.
  - `post-on_thread_reap` printed but next iteration's `pre-println` missing → hang in iterator advance.
  - `[ DBG ] reap: drained N` printed but no `[ DBG ] reap: pre-println` → hang in for-loop entry (very unusual).

**After capturing, REVERT the instrumentation.** Don't commit it. The investigation goal is the data, not a permanent log change.

### Decision rule

After Experiment C produces a failure with the instrumentation:

- **Hang inside `Thread::free`**: the slab/heap interaction is the cause. Likely candidates: per-CPU magazine corruption, global cache Mutex deadlock with some other path, heap allocator corruption from fresh-alloc path. We'd add MORE instrumentation inside `Slab::free` next.
- **Hang in iterator advance / Drop chain**: this is harder — possibly a stack alignment issue or a corrupted heap state. We'd inspect the Box's heap region post-Thread::free.
- **Hang in for-loop entry**: very unlikely; would point at VecDeque's iter implementation interacting with something.

In any case, this experiment localizes the hang to a specific operation. PM then decides next steps.

---

## 5. What you do NOT do

- **Do not modify scheduler.rs internals beyond the `reap()` instrumentation in Experiment C, and only after A and B are reported.** Touching `block_current_into`, `wake_thread*`, `on_tick`, `rotate`, etc., risks perturbing the timing of the bug in ways that hide it.
- **Do not modify ipc/mod.rs.** The bug isn't in IPC; the IPC scenario passed 80/80.
- **Do not modify the test harness.** xtask is not implicated.
- **Do not add reuse-the-binary-across-runs optimization to xtask.** Each run boots fresh; that property is what lets us trust the run-by-run summary.
- **Do not commit the instrumentation.** Even temporarily on a side branch. The instrumentation lives in the working tree only for the duration of Experiment C.
- **Do not retry the failed run-81 hoping it works the second time.** Each run is an independent boot; rerunning gives no information about the bug.
- **Do not draw conclusions until all three experiments are reported.** A vs B vs C cross-cuts the hypothesis space; partial data is misleading.

---

## 6. Close report shape

Format:

```
== Experiment A ==
PASS-COUNT: N/100
First failure run: <number or "n/a">
Failing serial log: <path or "n/a">

== Experiment B ==
PASS-COUNT: N/100
First failure run: <number or "n/a">
Failing serial log: <path or "n/a">

== Experiment C ==  (only if A and B both indicate reproducible hang)
PASS-COUNT: N/100
First failure run: <number>
Failing instrumented serial log path: <path>
Last [ DBG ] line emitted before hang:
  <verbatim line>

== Conclusions you can defensibly draw ==
<bullets — only what the data SUPPORTS, not what you SUSPECT>

== Hypotheses you have but did NOT act on ==
<bullets — these are notes for PM, not action items for you>
```

The "Hypotheses you have but did NOT act on" section is welcome but explicitly informational. Don't conflate hypotheses with conclusions.

---

## 7. Time budget

Each experiment takes about ~15 minutes (100 boots × ~8s each = ~13 min plus framing overhead). All three together: ~45–60 minutes. Don't optimize for speed — capture is more important than rate. If a single run hangs for 180s, let it.

If something blocks an experiment from running cleanly (e.g., the harness can't bare-boot without a test scenario, or `cargo neo-tty` doesn't terminate predictably), STOP and surface to PM. Don't synthesize an alternative experiment without surfacing first — the experiment design is what gives the result interpretive weight.

---

## 8. Why this much ceremony for a 1-in-80 hang

Because:

1. **An intermittent kernel hang is a kernel bug.** We don't ship those even at low rates.
2. **The smoke test runs ahead of every test scenario.** A 1.25% flake rate in the smoke test contaminates EVERY test result for sub-commit 2's lifetime.
3. **Sub-commit 3+ depends on a stable scheduler.** Compounding mystery hangs into 3a's ManuallyDrop+asm release work makes 3a undebuggable.
4. **We just learned (the hard way) that "the bet should hold under per-CPU schedulers" is not the same as "the bet holds under per-CPU schedulers."** Q5 taught us to verify, not assume. This investigation is the verification.

---

**End of dispatch.**
