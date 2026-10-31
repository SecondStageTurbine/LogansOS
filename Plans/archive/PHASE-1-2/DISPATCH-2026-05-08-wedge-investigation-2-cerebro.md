# Dispatch: wedge investigation 2 — what stopped run 4?

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Diagnostic experiment. Throwaway instrumentation on a stash or temporary branch, no commit on main. Targets a specific reproducible wedge instance (~5% rate) by instrumenting the in-kernel smoke-test path, NOT the reaper. Produces a findings report. Sequel to `Plans/DISPATCH-2026-05-08-h1-validation-experiment-cerebro.md`, which refuted H1 in its narrow form.
**Substrate:** Project HEAD `46984a1`. Working tree should be clean on tracked files. The H1 experiment's instrumentation is preserved as `stash@{0}: h1-experiment-instrumentation` and is NOT used in this experiment — the new instrumentation is in different code.

---

## 0. Standing instructions

1. **No commit on main.** All instrumentation is throwaway. Use a stash or temporary branch (your choice; `experiment/wedge-investigation-2` if branch).
2. **Read-only on `Plans/`** — produce one new file (`Plans/WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md`); do not modify existing Plans/ artifacts.
3. **Do not run the H1 experiment's stashed instrumentation.** It is preserved for reference. This dispatch's instrumentation is in different code and may be combined later with the H1 instrumentation if useful, but for this experiment, start from clean `46984a1`.
4. **Stop and surface** if anything is unexpected — instrumentation builds fail, the wedge rate goes way up or way down from the observed ~5-22% baseline, the data shows something neither this dispatch nor the H1 findings predict.
5. **Time budget: ~3-4h.** ~30-45 min instrumentation, ~90 min running 30 boots (we want more samples than the H1 experiment's 20 because the wedge rate may be lower in some regimes), ~45 min analysis, ~30-45 min findings report.

---

## 1. The hypothesis under test

The H1 validation experiment showed run 4 wedge with the lock-free short-circuit removed (forced lock acquisition on every iteration) and the reaper draining correctly. The wedge happens in code outside `reap()`. The in-kernel smoke test in `kernel/src/main.rs` runs `ping`/`pong`/`hog` kernel threads before user-space neo-init starts. Run 4's serial log shows:

```
[ kernel ] sched: spawned 'ping' (id=4)
[ kernel ] sched: spawned 'pong' (id=5)
[ kernel ] sched: spawned 'hog' (id=6)
[ ping ] 1 ... [ ping ] 5
[ pong ] 1 ... [ pong ] 4
[ hog  ] preempted at least 5 ticks (start=4, now=10)
[ H1 ] seq=4 cpu=0 z_pre=1 z_post=1 divergence=no
[ pong ] 5
[ kernel ] sched: reaped 'ping' (id=4)
[no further output until QEMU timeout]
```

The wedge occurs between "ping reaped" and "next thing should happen." In a non-wedged run, after `ping` is reaped, `pong` and `hog` should also produce zombies, get reaped, and the smoke test should complete (likely emitting some "smoke test complete" line) before neo-init is spawned.

There are several plausible candidate causes, none of which the H1 experiment ruled out:

- **H2: pong/hog are stuck in an unreachable state.** They may have called something that put them on a wait queue that never wakes (e.g., a semaphore, a channel, a yield path that doesn't return). Their threads are alive, not zombies, so `reap()` doesn't drain them.
- **H3: pong/hog are running but their output is suppressed.** Less likely (we'd expect serial output for hog's preemption ticks to keep appearing), but possible if a serial lock is held by a wedged context.
- **H4: pong/hog completed but `on_thread_reap` failed silently for them.** Their threads moved to Zombie, the reaper drained them, but `on_thread_reap` didn't deregister them properly. The wait loop (which is for user processes, not kernel threads, so this isn't `main.rs:173`) — actually, wait: there's no wait loop for kernel threads to exit before neo-init spawns. Look at `main.rs` carefully; how does the kernel decide the smoke test is "done"?
- **H5: A kernel thread that should have been spawned but wasn't.** If the smoke test expected to spawn a fourth thread (or the smoke-test orchestrator) that races with `ping/pong/hog`, that thread missing the dispatch means progress halts.
- **H6: The scheduler's idle/yield path on cpu=0 has wedged.** If `ping`'s reap incidentally put cpu=0's scheduler into a state where `current` is set to something that can't run (or `next_runnable()` returns None when threads exist), no further scheduling happens.

The hypotheses are not mutually exclusive. The dispatch's job is to instrument enough code to distinguish them.

## 2. The instrumentation

Three areas to instrument, each producing a distinct event-stream prefix in the serial log:

### 2.1 Smoke test orchestration (`[ SMOKE ]`)

Locate the in-kernel smoke test in `kernel/src/main.rs` (around line 125–175). Identify the orchestration code that spawns `ping`/`pong`/`hog` and waits for them to complete. Add logging at every state transition:

```rust
serial_println!("[ SMOKE ] phase=spawn_ping start");
sched::spawn("ping", ping_fn);
serial_println!("[ SMOKE ] phase=spawn_ping done");

serial_println!("[ SMOKE ] phase=spawn_pong start");
// ... etc for all three threads.

// Wherever the orchestrator waits for them:
serial_println!("[ SMOKE ] phase=wait_for_smoke_done iter=N alive_threads=...");
```

The exact insertion points depend on how the smoke test is structured. Read `main.rs` carefully and identify:

- How are the three threads launched?
- What does the kernel do between launching them and either (a) reaping them all and proceeding to neo-init, or (b) emitting a "smoke test complete" message?
- If there is a wait condition, what is it? (A counter? A flag? Polling `any_kernel_thread_alive()`-style?)

Instrument the wait condition specifically. Each iteration of the wait should log its state.

### 2.2 Per-thread lifecycle (`[ THR ]`)

Instrument `Thread`'s state transitions in `kernel/src/sched/thread.rs` (or wherever the state field lives). For each transition:

```rust
// In whatever function transitions thread state:
serial_println!(
    "[ THR ] id={} name={} {:?} -> {:?}",
    thread.id, thread.name, old_state, new_state,
);
```

Specifically capture:

- `Running -> Ready` (preemption)
- `Running -> Blocked` (voluntary yield to a wait queue)
- `Ready -> Running` (scheduled in)
- `Blocked -> Ready` (woken)
- `Running -> Zombie` (exit)
- `Zombie -> [reaped/freed]` (reap completes)

If the codebase doesn't have a single state-transition function, you may need to add one or instrument multiple call sites. The point is to see each thread's full lifecycle.

### 2.3 Scheduler `next_runnable` decisions (`[ SCHED ]`)

In `kernel/src/sched/scheduler.rs`, instrument the per-CPU scheduler's `next_runnable()` (or whatever picks the next thread to run). Each invocation should log:

```rust
serial_println!(
    "[ SCHED ] cpu={} ready_q_len={} chose={}",
    cpu_id, ready_queue.len(),
    next.as_ref().map(|t| t.name.as_str()).unwrap_or("idle"),
);
```

This is potentially very high-volume — `next_runnable` is called on every preemption. To keep log size manageable:

- **Throttle to once per ~100 calls per CPU.** Use a per-CPU counter that increments every call and only logs when `count % 100 == 0`. This gives us periodic snapshots without flooding.
- **Always log when `chose=idle`.** An idle decision is cheap to log and informative — it tells us a CPU has nothing to run.
- **Always log when ready_q_len changes from 0 to non-zero or vice versa.** State changes are the interesting part.

### 2.4 What NOT to instrument

- Do not re-instrument `reap()`. The H1 experiment already covered that path.
- Do not modify `ping`, `pong`, or `hog` themselves. Their bodies are the test's input; changing them changes what's being tested.
- Do not add logging inside spinlocks. `serial_println!` is slow and should not run while holding any kernel lock.
- Do not change interrupt handlers or PIT tick logic. The wedge happens in scheduler/thread state, not in the timer path (probably).

## 3. The runs

### 3.1 Build and sanity

```powershell
cd "E:\AI Automation\AltimitNeo"
cargo k
# Expected: builds clean. Some new warnings from instrumentation are fine; >5 new warnings indicates a bug.

# Single-run sanity to confirm instrumentation fires.
docker compose run --rm neo cargo neo-test --scenario kaslr_smoke
# Expected: PASS, with [ SMOKE ], [ THR ], and [ SCHED ] log lines visible.
```

### 3.2 The 30-run loop

```powershell
# 30 runs of smp_per_cpu_scheduler_smoke at NEO_SMP=4. Same scenario as H1 experiment for direct comparability.
$results = @()
for ($i = 1; $i -le 30; $i++) {
    Write-Host "Run $i/30..."
    $log = "target/verify/WEDGE-INV-2-run-$i.serial.log"
    docker compose run --rm neo cargo neo-test --scenario smp_per_cpu_scheduler_smoke 2>&1 | Tee-Object -FilePath $log
    $classification = (Classify-Result $log)
    $results += [PSCustomObject]@{ Run = $i; Classification = $classification }
}
$results | Group-Object Classification | Format-Table -AutoSize
```

Expected outcome:

- **0 PANIC, 0 OTHER.** ADR-015's heap fix is solid; the new instrumentation shouldn't introduce panics.
- **Some TIMEOUTs.** Based on H1 experiment (1/20 = 5%) and PATCH1 50-run loop (~22%), expect 1-7 TIMEOUTs. The instrumentation may slow the boot enough to change the rate; that's fine.
- **The TIMEOUT instances are the gold.** Each one is a wedge with full instrumentation. Capture every one's serial log.

### 3.3 Classification quality

If the loop produces zero TIMEOUTs across 30 runs, that's a problem — the instrumentation may have masked the wedge (e.g., the slowdown from logging changes the timing enough that whatever races was racing no longer does). If that happens, surface immediately. Possible mitigations: run another 30, reduce log volume, or accept that this scenario doesn't reliably produce the wedge in this instrumentation regime.

If the loop produces 5+ TIMEOUTs, the data is plentiful. Focus analysis on 2-3 representative TIMEOUT logs in detail rather than analyzing all of them shallowly.

## 4. Analyzing the logs

For each TIMEOUT log:

1. **Locate the last `[ SMOKE ] phase=...` line.** What phase was the smoke test in when the wedge happened?
2. **Count `[ THR ]` lines per thread.** Did all three of `ping`/`pong`/`hog` reach `Zombie`? Or are some of them stuck in `Ready` or `Blocked`?
3. **Check the last `[ SCHED ]` line per CPU.** What was each CPU doing? Idle? Running a specific thread? Stuck on a thread that should have yielded?
4. **Cross-reference with previous lines.** If `pong` is stuck in `Blocked`, what was the wake-up condition? What was the last log line that should have woken it?

Compare across TIMEOUT logs. Do they share a signature?

- **All TIMEOUTs show the same thread stuck:** strong signal toward a specific bug (e.g., "pong always wedges in Blocked state").
- **TIMEOUTs spread across different threads/phases:** weaker signal; the wedge may have multiple causes or a higher-level issue (scheduler, IPI delivery, etc.).

For comparison with non-wedged runs, also examine 1-2 PASS logs to confirm the expected smoke-test flow looks like.

## 5. The findings report

Produce `Plans/WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md`. Same shape as H1 findings, with these required sections:

### §1. Summary

3-5 sentences. TIMEOUT count from the 30 runs. Number of distinct TIMEOUT signatures observed. Leading hypothesis about what's stopping progress (cite hypothesis numbers from §1 of this dispatch — H2 through H6 — or name a new one if none of them fit).

### §2. Methodology

What was instrumented (`[ SMOKE ]`, `[ THR ]`, `[ SCHED ]`). Why those three. What was deliberately NOT instrumented (and why).

### §3. Quantitative results

Table per run: classification, last `[ SMOKE ]` phase, threads in non-Zombie state at timeout, last `[ SCHED ]` decision per CPU.

### §4. Qualitative observations

For each TIMEOUT, what does the wedge look like? Are wedges identical, or are there sub-signatures?

The most informative observation: **what is NOT happening in a wedge that IS happening in a passing run?** If you compare a wedge run's `[ SMOKE ]` trace to a passing run's, where do they diverge?

### §5. Verdict on H2-H6

For each candidate hypothesis from §1, state:

- **CONFIRMED, REFUTED, or PARTIALLY supported** by the data.
- The specific evidence.

If none fits, name a new hypothesis (H7) and explain.

### §6. What this implies for the next investigation

Two cases:

- **A specific cause is identified:** the next dispatch is the fix dispatch, with whatever scope the cause requires. PM writes it.
- **Cause is narrowed but not pinpointed:** the next investigation has a smaller hypothesis space. Specify what additional instrumentation or experiment would close the remaining gap.

### §7. Cross-references

- This dispatch.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10.
- `Plans/PM-DECISION-2026-05-08-wedge-investigation-continues.md`.
- Raw logs: `target/verify/WEDGE-INV-2-run-*.serial.log`.

## 6. Restoring the substrate

```powershell
# If stash:
git stash drop  # or keep labeled
# If branch:
git checkout main
git branch -D experiment/wedge-investigation-2

git log --oneline -1
# Expected: 46984a1 docs: Plans/INDEX.md — post-ADR-015 sweep + open commitments section
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty (no tracked-file mods).
```

Ping PM with:
- Path to findings report.
- TIMEOUT count and rate.
- Number of distinct signatures.
- Leading hypothesis verdict.
- Anything surprising.

## 7. What you do NOT do

- Do not commit instrumentation on main.
- Do not modify code outside `kernel/src/main.rs`, `kernel/src/sched/scheduler.rs`, and `kernel/src/sched/thread.rs` (or wherever thread state lives). Other kernel files are out of scope.
- Do not write a fix. This dispatch is purely diagnostic. The fix lands in a separate dispatch when we know what the fix is.
- Do not remove or weaken existing assertions. If something is `assert!`-ing, that's intentional.
- Do not modify the smoke test's logic. Add observation; do not change behavior.

## 8. Why this matters

The wedge has been deferred four times. The H1 experiment ruled out the most obvious candidate (the lock-free short-circuit). We need to identify the actual cause to land an actual fix. Run 4 of the H1 experiment is the most informative wedge instance we have — full serial log, controlled instrumentation regime, no exotic conditions. The wedge cause must be in code path we have not yet instrumented; this dispatch instruments the most likely candidates.

If this dispatch's findings narrow the cause to a specific line of code, the next dispatch is the fix. If they don't, we have a smaller hypothesis space and the next investigation continues from there. Either way, we make progress toward an actual fix rather than landing an unmotivated tightening.

The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` is honored by this work. The investigation is the path to meeting the commitment.

---

**End of dispatch.**
