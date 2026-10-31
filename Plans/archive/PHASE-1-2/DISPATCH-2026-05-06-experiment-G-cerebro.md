# Dispatch: Experiment G — instrumented `reap()` to localize the hang in ipc_heap_kill_smoke

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 + Q5 restoration applied), unchanged since the EXP-F close.
**Substrate:** Experiments A, B, D, E, F have narrowed the hang to "ipc_heap_kill_smoke at SMP=4 specifically." Cheap bisection is exhausted. Instrumentation localizes the hang to a specific operation in `reap()` so we know what to investigate next.
**Type:** Diagnostic. NOT a fix attempt. NOT a permanent commit. Working tree stays uncommitted until PM reviews data and decides.

---

## 0. Standing instructions

Same as Experiments A, B, D, E, F.

1. **Do NOT commit anything.** Working tree stays uncommitted.
2. **Do NOT attempt fixes.** Hypotheses go in close report only.
3. **Do NOT modify scheduler / IPC / per-CPU code beyond the instrumentation specified in §3 below.** The instrumentation is bracketed and removable; no other timing-sensitive paths are touched.
4. **Capture verbatim output.**
5. **REVERT the instrumentation when this experiment closes.** The instrumentation lives in the working tree only for the duration of this experiment. After capture, `git checkout -- kernel/src/sched/scheduler.rs` (or whatever scope cleanly removes it).

---

## 1. The question

The bisection chain so far:

- A + B: needs SMP + test harness configuration.
- D: not QEMU flags, kernel binary specifically.
- E: not generic `mod test_harness` presence.
- F: not generic `spawn_neo_init` presence (cap_mint_and_use also calls it; passes 100/100).

The remaining differential between ipc_heap_kill_smoke (hangs ~1-in-80) and cap_mint_and_use (passes 100/100):

- The kernel-side `cfg(feature = "neo-test-ipc-smoke")` arm in `run_selected_scenario` (different `pass()` strings, different FAIL reason; trivial code differences).
- The neo-init user binary built with default features (ipc) vs `cap-mint-and-use` user feature (cap_mint_and_use). Different binary contents.
- Because Limine loads neo-init as a module BEFORE `_kernel_start`, different neo-init binary sizes can shift kernel heap placement under KASLR.

The hang occurs INSIDE `smoke_test()`, AFTER `[ kernel ] sched: reaped 'pong' (id=5)` is emitted, BEFORE the scenario body or `spawn_neo_init` are reached. So the differential cannot be in scenario-execution code. It must be in:
- Pre-smoke_test memory layout (which depends on kernel binary size + module layout + KASLR), OR
- Static initializer state that differs between binaries, OR
- Cache/TLB locality effects from binary-layout differences.

Experiment G localizes WHERE in `reap()` the hang lands. That answer determines which mechanism is actually wedging — it could be `Thread::free`'s slab path, the for-loop iterator, an implicit `Box::drop`, the next iteration's println, or something else entirely.

---

## 2. The setup

The experiment instruments `kernel/src/sched/scheduler.rs::reap()` with debug `serial_println!` calls bracketing each operation in the per-zombie for-loop. When the hang happens, the last `[ DBG ]` line emitted localizes the hang to a specific operation.

This experiment is instrumentation, not a fix. The instrumentation is removed before any commit.

---

## 3. Required modification to `kernel/src/sched/scheduler.rs::reap()`

Locate `pub fn reap()` (around line 525 per prior OBSERVE). Add four `serial_println!` calls inside the for loop body, each prefixed with `[ DBG ]`:

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

Specifically:

- `[ DBG ] reap: drained N zombie(s)` after `drained` is computed.
- `[ DBG ] reap: pre-println id=N name='X'` BEFORE the existing "reaped" println, with the id+name extracted from the zombie before consuming it.
- The existing `[ kernel ] sched: reaped 'X' (id=N)` println stays exactly as before.
- `[ DBG ] reap: pre-Thread::free id=N` BEFORE `Thread::free(zombie)`.
- `[ DBG ] reap: post-Thread::free id=N` AFTER `Thread::free(zombie)`.
- `[ DBG ] reap: post-on_thread_reap id=N` AFTER `on_thread_reap(pid)`.
- `[ DBG ] reap: loop end` AFTER the for-loop terminates.

**Important:** the `name` and `id` must be extracted BEFORE `Thread::free(zombie)` consumes the Box (a `let` binding that copies them out — `zombie.name` is `&'static str` and `zombie.id.0` is `u64`, both Copy). Using `zombie.name` after `Thread::free(zombie)` would be a use-after-move and won't compile.

Adapt to the actual function shape in the working tree if it differs from this template (it should be very close).

The instrumentation does change `reap()`'s timing — extra `serial_println!` calls each take a serial-port write, which under `_print`'s `without_interrupts` briefly disables interrupts. This is an acknowledged limitation of the experiment: the instrumentation MAY mask the bug if the bug requires the exact unmodified timing window. We accept this risk because we have no cheaper option.

---

## 4. Run the instrumented gate

After applying the instrumentation, rebuild and run the 100x ipc_heap_kill_smoke gate at NEO_SMP=4:

```powershell
cd "E:\AI Automation\AltimitNeo"
$results = @()
for ($i = 1; $i -le 100; $i++) {
    Write-Output "=== G-Run $i ==="
    $env:NEO_SMP = "4"
    $r = docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke 2>&1 | Out-String

    if ($r -match "NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke") {
        Write-Output "G-Run $i`: PASS"
        $results += "PASS"
    } else {
        Write-Output "G-Run $i`: FAIL"
        $serial = "target/neo-test/ipc_heap_kill_smoke.serial.log"
        if (Test-Path $serial) {
            Copy-Item $serial "target/verify/EXP-G-fail-run-$i.serial.log"
        }
        $r | Out-File "target/verify/EXP-G-fail-run-$i.txt" -Encoding utf8
        $results += "FAIL"
        break
    }
}

$passCount = ($results | Where-Object { $_ -eq "PASS" }).Count
"EXP-G-PASS-COUNT: $passCount/100" | Out-File "target/verify/EXP-G-summary.txt"
Write-Output "EXP-G-PASS-COUNT: $passCount/100"
```

Adapt to the same shell environment as A/B/D/E/F.

---

## 5. What you do NOT do

- **Do not modify any code other than `reap()`'s body in `kernel/src/sched/scheduler.rs`.**
- **Do not commit the instrumentation.** Even on a side branch.
- **Do not rebuild between runs** — `cargo neo-test` already rebuilds the kernel for the active scenario; xtask handles it.
- **Do not extend instrumentation to other functions** during this experiment. If `reap()` instrumentation localizes the hang to (e.g.) `Thread::free`, PM will dispatch a follow-up that instruments inside `Thread::free`. Don't preempt.
- **Do not reduce the timeout.** 180s xtask timeout matches all prior experiments.

---

## 6. Result to capture

```
EXP-G-PASS-COUNT: N/100
First failure run: <number or "n/a">
Failing serial log: <path or "n/a">
Last [ DBG ] line emitted before hang: <verbatim, or "n/a">
Full last 20 lines of failing serial log: <verbatim>
```

The "last [ DBG ] line" is the critical finding. It tells us:

- `pre-Thread::free` printed but `post-Thread::free` missing → hang inside `Thread::free` (likely slab path: per-CPU magazine push, global cache lock, fresh-page free, or an underlying buddy free).
- `post-Thread::free` printed but `post-on_thread_reap` missing → hang inside `on_thread_reap` for that pid (unexpected since pid is KERNEL_PID for ping/pong/hog and `on_thread_reap` early-returns).
- `post-on_thread_reap` printed but next iteration's `pre-println` missing → hang in iterator advance, implicit Drop chain, or between iterations.
- `pre-Thread::free` printed for ping (after pong's full sequence) and missing `post-Thread::free` → hang inside `Thread::free` specifically for ping.
- `[ DBG ] reap: drained N` printed but no `[ DBG ] reap: pre-println` → hang in for-loop entry (very unlikely).

If the instrumentation masks the bug (i.e., 100/100 PASS with instrumentation), report that — that's also a result. It tells us the hang requires unmodified timing in this exact code region, which is itself diagnostic information.

---

## 7. Decision rule

After this experiment:

- **G fails with `pre-Thread::free` printed but `post-Thread::free` missing:** the hang is in the slab path. Next: PM dispatches an instrumented `Slab::free` (via `Thread::free`) to localize within the slab's alloc/free state machine.

- **G fails with `post-Thread::free` printed but next iteration's `pre-println` missing:** the hang is between iterations of the for-loop, or in implicit `Drop` running on the next iteration's setup. Surprising. PM thinks harder; possibly add finer-grained instrumentation around the loop.

- **G fails with some other pattern:** report verbatim. PM dispatches based on what landed.

- **G passes 100/100:** the instrumentation masked the bug. The hang requires exact unmodified timing of the code being instrumented. PM thinks harder; alternative instrumentation strategies needed (e.g., periodic counters that don't require taking the SERIAL lock as often, or non-instrumentation observability like reading machine-state registers from a watchdog).

Stop and report regardless of result.

---

## 8. Time budget

Same as E/F: ~10–15 seconds per run, ~90s overhead on run 1 for the rebuild with instrumentation. Total: ~16-20 minutes for 100 runs. Earlier failures stop the loop early.

The 100-run loop matters here: hang rate is ~1.25%, so 100 runs gives high confidence of seeing a failure if the instrumentation didn't mask the bug. If the first 100 runs all PASS, that's meaningful — the instrumentation masked the timing window.

---

## 9. After capture: revert the instrumentation

After the close report is written:

```bash
git checkout -- kernel/src/sched/scheduler.rs
```

(Or whatever clean-revert command applies to your working tree state.) Verify with `git diff kernel/src/sched/scheduler.rs` showing no change relative to the rest of your sub-commit 2 working tree.

The instrumentation is data-collection, not code. It does not commit. The working tree stays in the same state it was in pre-G, modulo the diagnostic data captured in `target/verify/`.

---

## 10. Why instrumentation now, not earlier

The original sub-commit-2-isolate dispatch had Experiment C as the instrumentation step, gated behind A and B both producing reproducible-without-test-harness hangs. A=100/100 and B=100/100 didn't meet that gate, so C was skipped.

Instead, we ran D, E, F to bisect along non-perturbing axes first. Each of those experiments gave us a meaningful narrowing. Now we've reached the wall of cheap experiments — the bisection has narrowed to "ipc_heap_kill_smoke at SMP=4 specifically" and there are no further axes to bisect without instrumentation or partial rollback.

G is where instrumentation finally pays its cost. The bug is localized enough that G's output, even with timing perturbation risk, will be informative.

The §Q5 lesson keeps applying: verify, don't assume. G verifies where the hang is. The follow-up dispatches will verify what's wedged at that location.

---

**End of dispatch.**

*Standard close report shape: PASS/FAIL count, last [ DBG ] line, full last 20 lines of any failing log, plus any observations about whether the instrumentation appeared to mask the bug. Then revert the instrumentation. ~20 minutes if you're efficient.*
