# Dispatch: H1 validation experiment — measure the BSP reap race directly

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Diagnostic experiment. Throwaway instrumentation on a stash or temporary branch, no commit on main. Produces a data artifact (instrumented serial logs) and a findings report. Informs the next dispatch (Alpha-prime hardening) but is not itself the fix.
**Substrate:** Project HEAD `47972c2` (ADR-015 + PATCH 1). Working tree clean on tracked files. Wedge investigation hypothesis H1 named in `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` §5; convergence with §Q9 redo and SC-3A pre-thinking analyzed in `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md`.

---

## 0. Standing instructions

1. **No commit on main.** All instrumentation is throwaway. The only artifacts that survive are (a) serial logs in `target/verify/H1-EXPERIMENT-*.serial.log`, and (b) a findings report at `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`.
2. **Use a stash or temporary branch, your choice.** Whichever fits your workflow. If branch, name it `experiment/h1-reap-race`. If stash, give it a descriptive name (`git stash push -m "h1-experiment-instrumentation"`). Either way, the substrate at `47972c2` must be cleanly recoverable when the experiment finishes.
3. **Read-only on `Plans/`** — produce one new file (`Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`); do not modify existing Plans/ artifacts.
4. **Stop and surface** if anything is unexpected — instrumentation builds fail, the smoke test passes 20/20 cleanly (no wedges), or the data shows something neither H1 nor any plausible alternative predicts.
5. **Time budget: ~2-3h.** ~30 min instrumentation, ~60 min running 20 boots, ~30 min analyzing logs, ~30 min writing the findings report.

---

## 1. The hypothesis under test

H1 states: the BSP's `reap()` iterates all per-CPU schedulers and checks `zombie_count` (lock-free Acquire load) before acquiring the lock. If an AP produces a zombie between the BSP's `zombie_count == 0` check and the BSP's next iteration of that CPU, the zombie is missed.

The race window is between two reads:

1. **Pre-lock read:** `scheduler.zombie_count.load(Acquire)` — observed value Z_pre.
2. **Post-lock read:** the contents of `s.zombies` after `mem::take` under the lock — observed length Z_post.

Under H1, there exists at least one (CPU_id, iteration) pair where Z_pre == 0 but Z_post > 0 would be true if the lock had been acquired. Equivalently: zombies are produced by APs in the window between Z_pre's read and the next iteration's lock acquisition (which doesn't happen because the early-out fires).

The experiment instruments both reads — pre-lock (always; we want Z_pre on every iteration) and post-lock (forced; we want Z_post even when the lock-free path would have skipped) — and looks for divergence.

## 2. The instrumentation

### 2.1 What to change

In `kernel/src/sched/scheduler.rs`, locate the BSP `reap()` function. Per the §Q9 redo notes, the current shape is approximately:

```rust
pub fn reap() {
    for cpu_id in 0..MAX_CPUS {
        let scheduler = match get_per_cpu_scheduler(cpu_id) {
            Some(s) => s,
            None => continue,
        };

        // Lock-free short-circuit (THE H1 SUSPECT).
        if scheduler.zombie_count.load(Ordering::Acquire) == 0 {
            continue;
        }

        let zombies = without_interrupts(|| {
            let mut s = scheduler.lock();
            mem::take(&mut s.zombies)
        });

        for zombie in zombies {
            on_thread_reap(zombie.pid);
            // ... drop zombie, free Thread, etc.
        }
    }
}
```

Verify the actual shape by reading the file before instrumenting. The shape may differ in details (function name, lock acquisition pattern, where `zombie_count` lives) — adapt the instrumentation accordingly.

### 2.2 The instrumented version

Replace the lock-free short-circuit with a logging-and-forced-acquire variant. Key properties:

1. **Always read Z_pre.** Capture `zombie_count` before any lock.
2. **Always acquire the lock.** Skip the short-circuit. This makes the instrumentation also a partial implementation of Alpha-prime, which is fine for the experiment — the goal is to observe Z_pre vs Z_post, not to preserve the broken behavior.
3. **Always read Z_post.** Capture the actual zombie queue length after lock acquisition, before draining.
4. **Log both values.** Emit a serial line for every iteration where Z_pre or Z_post is non-zero, plus every iteration where they differ.
5. **Drain and reap as normal** if Z_post > 0.

Example skeleton (adapt to actual code shape):

```rust
pub fn reap() {
    let reap_seq = REAP_SEQ.fetch_add(1, Ordering::Relaxed);

    for cpu_id in 0..MAX_CPUS {
        let scheduler = match get_per_cpu_scheduler(cpu_id) {
            Some(s) => s,
            None => continue,
        };

        // INSTRUMENTATION: capture pre-lock zombie_count.
        let z_pre = scheduler.zombie_count.load(Ordering::Acquire);

        // INSTRUMENTATION: always acquire the lock (skip the short-circuit).
        let zombies = without_interrupts(|| {
            let mut s = scheduler.lock();
            // INSTRUMENTATION: capture post-lock queue length BEFORE drain.
            let z_post = s.zombies.len();

            if z_pre != z_post as u32 || z_pre > 0 || z_post > 0 {
                serial_println!(
                    "[ H1 ] seq={} cpu={} z_pre={} z_post={} divergence={}",
                    reap_seq, cpu_id, z_pre, z_post,
                    if z_pre != z_post as u32 { "YES" } else { "no" },
                );
            }

            mem::take(&mut s.zombies)
        });

        for zombie in zombies {
            on_thread_reap(zombie.pid);
            // ... whatever the existing reap body does
        }
    }
}

// Add at module scope:
static REAP_SEQ: AtomicU32 = AtomicU32::new(0);
```

Notes on this skeleton:

- **`z_pre as u32` comparison** assumes `zombie_count` is `AtomicU32` and `s.zombies.len()` is `usize`; adapt the cast to match actual types.
- **`reap_seq` is a global counter** so each `reap()` call produces a distinct sequence number across the whole boot. This makes log analysis tractable — without it, distinguishing iterations of the same `reap()` invocation from different invocations is hard.
- **The `divergence=YES` condition** is the H1 signal. If z_pre is 0 but z_post is non-zero, an AP produced a zombie between the pre-lock read and the lock acquisition. That is the race H1 names.
- **The existing reap body must run unchanged** after the instrumentation. The point is to measure, not to fix. If the reap drains z_post > 0 zombies that the lock-free path would have missed, that's evidence — and incidentally corrects the wedge for the duration of the experiment, which we want (otherwise the smoke test wedges at 20-28% rate and we get fewer data points).

### 2.3 What NOT to instrument

- Do NOT add logging inside `on_thread_reap` itself. We're measuring the race at the queue level, not the per-zombie level.
- Do NOT instrument AP code or zombie production sites. We want to observe the consumer side (BSP reap) and infer producer behavior from the divergence.
- Do NOT add timing measurements. The race is a state divergence, not a timing one. `serial_println!` is plenty slow already; adding `rdtsc` calls would add noise without insight.

## 3. The runs

### 3.1 Build the instrumented kernel

```powershell
cd "E:\AI Automation\AltimitNeo"
cargo k
# Expected: builds clean (or with the existing 18-warning baseline plus any new warnings from the instrumentation; a few are fine, dozens of new warnings means the instrumentation has a bug).
```

If errors, fix them and retry. If new warnings exceed ~5, surface — likely the instrumentation introduced something other than what was intended.

### 3.2 Single-run sanity

```powershell
docker compose run --rm neo cargo neo-test --scenario smp_per_cpu_scheduler_smoke
# Expected: PASS, with H1 log lines visible in the serial output.
```

A single run should produce some H1 log lines. Most will be `z_pre=0 z_post=0` (no zombies anywhere) or `z_pre=N z_post=N` (no divergence; lock-free read was correct). The interesting cases are `divergence=YES`.

If the single run produces no H1 log lines at all, the instrumentation isn't firing — STOP and check the build / log path / serial capture.

### 3.3 The 20-run loop

```powershell
# 20 runs at NEO_SMP=4, capture serial log per run, no rebuild between.
$results = @()
for ($i = 1; $i -le 20; $i++) {
    Write-Host "Run $i/20..."
    $log = "target/verify/H1-EXPERIMENT-run-$i.serial.log"
    docker compose run --rm neo cargo neo-test --scenario smp_per_cpu_scheduler_smoke 2>&1 | Tee-Object -FilePath $log
    # Classify: PASS, TIMEOUT, OTHER. PANIC would be unexpected.
    $classification = (Classify-Result $log)  # adapt to your existing classifier
    $results += [PSCustomObject]@{ Run = $i; Classification = $classification }
}
$results | Group-Object Classification | Format-Table -AutoSize
```

Adapt to your existing loop wrapper. Key invariants: 20 runs, fresh KASLR per boot, NEO_SMP=4, capture every serial log to `H1-EXPERIMENT-run-N.serial.log`.

Expected outcome:

- **0 PANIC** (the heap-overflow class is fixed; instrumentation doesn't touch it).
- **0 OTHER** (the alloc.rs:573 class is fixed; instrumentation doesn't touch it).
- **TIMEOUT rate substantially LOWER than 28%.** Because the instrumentation forces lock acquisition on every iteration, it incidentally implements Alpha-prime's behavior. If H1 is right, the wedge essentially stops happening during the experiment. If H1 is wrong, the wedge persists at near-baseline rate.
- **Most runs PASS, with H1 log lines visible.** The data is in the H1 lines, not in the pass/timeout count.

The TIMEOUT rate is itself partial evidence:

| TIMEOUT rate observed | Interpretation |
|---|---|
| 0/20 (0%) | Strong evidence H1 is correct AND complete (lock-acquire fully closes the race). |
| 1-2/20 (5-10%) | Consistent with H1 + sample variance. The remaining wedges may be a different signature. |
| 3-5/20 (15-25%) | Ambiguous. H1 may be one of multiple wedge causes, or the instrumentation isn't fully closing the race for some reason. |
| 6+/20 (30%+) | Evidence AGAINST H1. The lock acquisition didn't help; the wedge cause is elsewhere. |

The TIMEOUT rate alone isn't conclusive — we want the H1 log data to corroborate.

## 4. Analyzing the logs

For each `H1-EXPERIMENT-run-N.serial.log`:

1. **Extract all `[ H1 ]` lines.** Count total iterations logged (each line is one CPU's iteration of one reap() call).
2. **Count divergence cases.** How many lines have `divergence=YES`?
3. **Characterize divergences.** When divergence=YES, what is z_pre? what is z_post? Is the pattern "z_pre=0 z_post>0" (the H1 race) or something else (e.g., z_pre>z_post, which would indicate a different bug)?
4. **Per-CPU distribution.** Are divergences concentrated on specific CPU IDs, or evenly distributed?
5. **Per-sequence distribution.** Are divergences clustered in time (specific reap_seq ranges) or spread out?

The findings report distills this into:

- Total reap iterations across 20 runs (should be tens of thousands).
- Total non-zero iterations (z_pre > 0 OR z_post > 0).
- Total divergence cases (z_pre != z_post).
- Of those, how many are the H1 signature (z_pre = 0, z_post > 0)?
- Other divergence types and their counts.

## 5. The findings report

Produce `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`. Shape (matches the §Q9 redo and wedge investigation note styles):

### §1. Summary

3-5 sentences. Total iterations, total divergences, H1-signature divergence count, pass/timeout breakdown across the 20 runs, top-line conclusion (H1 confirmed / H1 refuted / H1 partially confirmed).

### §2. Methodology

What was instrumented (point to §2 of this dispatch). What was measured (z_pre, z_post per iteration). How the data was analyzed.

### §3. Quantitative results

Table per run: run number, classification (PASS/TIMEOUT/OTHER), total reap iterations, total non-zero iterations, total divergences, H1-signature divergences.

Aggregate row: totals across all 20 runs.

### §4. Qualitative observations

Patterns in the divergence data:
- Per-CPU distribution.
- Per-sequence distribution (early in boot vs. steady state).
- Z_post values when divergence fires (always 1? sometimes more?).
- Correlation between divergences in a run and that run's classification.

Anything surprising. If you see something the dispatch didn't predict, name it.

### §5. Verdict on H1

One of:
- **CONFIRMED:** H1 race observed directly. State the magnitude — how often does it fire? Per-CPU? Per-boot?
- **REFUTED:** No H1 signature observed across all runs. The wedge has a different cause; H1 needs to be replaced.
- **PARTIALLY CONFIRMED:** H1 fires but doesn't account for all observed wedges. The wedge is at least partially explained by H1; another cause may also exist.

### §6. Implications for Alpha-prime

If CONFIRMED: Alpha-prime (replace lock-free short-circuit with unconditional lock-acquire) is the right fix. The experiment's instrumentation is approximately the production form; the production version just removes the `serial_println!` and the explicit `z_pre`/`z_post` tracking.

If REFUTED: Alpha-prime would still be a strict tightening (correct regardless of which specific race is the wedge), but it would not fix the wedge. The next investigation step is to look elsewhere — possibly the smoke test's spawn-and-reap pattern, possibly a timer-interrupt interaction, possibly a slab-allocator path.

If PARTIALLY CONFIRMED: Alpha-prime fixes the H1 portion. The remaining wedge cases need their own investigation. PM and project lead decide whether to land Alpha-prime now (closing the H1 portion of the wedge) or wait for a more comprehensive fix.

### §7. Cross-references

- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's H1 hypothesis.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — convergence analysis.
- `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` — §Q9 redo design (relevant for understanding the broader reap distribution context).
- `target/verify/H1-EXPERIMENT-run-*.serial.log` — raw data from this experiment.
- `47972c2` — substrate commit; experiment instrumentation does NOT commit on main, but is built on top of this hash.

## 6. Restoring the substrate

When the experiment is done and findings are written:

```powershell
# If you used a stash:
git stash drop  # or keep it labeled if you want the instrumentation around for reference

# If you used a branch:
git checkout main
git branch -D experiment/h1-reap-race  # or keep it for reference

# Either way, verify HEAD is back at 47972c2 with clean working tree on tracked files.
git log --oneline -1
# Expected: 47972c2 mm: ADR-015 KASLR-tolerant heap placement ...
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: empty (no tracked-file mods).
```

Ping PM with:
- Path to the findings report.
- Verdict from §5 (CONFIRMED / REFUTED / PARTIALLY CONFIRMED).
- TIMEOUT rate from the 20-run loop.
- Any surprises worth flagging before the next dispatch.

---

## 7. What you do NOT do

- Do not commit the instrumentation on main.
- Do not modify `kernel/src/mm/`, `kernel/src/test_harness.rs`, or any file outside `kernel/src/sched/scheduler.rs`. The experiment is scoped to the reap path only.
- Do not change `NEO_SMP` from 4. The wedge is SMP-specific; running at SMP=1 is a different question.
- Do not optimize the `serial_println!` away to reduce log volume. The volume is part of the data — we want to see every iteration that has any zombie activity, not just the divergent ones, so we have denominator counts.
- Do not change the smoke test's body. We're observing how the existing test interacts with the existing reaper, not designing new tests.
- Do not write a fix in this dispatch's scope. Even if the data clearly shows H1, the fix lands in a separate dispatch (Alpha-prime). This dispatch only measures.

---

## 8. Why this matters

The wedge has been deferred four times. Tonight's commitment (`Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`) names a deadline (before SC-3B), but the deadline only matters if the fix is correct. Landing Alpha-prime without first verifying H1 would mean shipping a "fix" that may or may not address the actual bug. The Bible's "every commit is intended to ship" discipline (Article 1.3) requires that we earn the correctness claim. This experiment earns it.

It also produces a project-history artifact that's higher-quality than "we assumed H1 and tightened on that assumption." Future readers (post-mortem authors, contributors to the eventual §Q9 redo, anyone reasoning about reap correctness) get the actual data, not just the conclusion.

If H1 is REFUTED, the wedge fix dispatch is replaced by a different investigation — but we know that's the case before writing kernel code, not after deploying it. That's the right shape for a clean-slate kernel project where every commit is meant to ship.

---

**End of dispatch.**
