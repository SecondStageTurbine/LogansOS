# Dispatch: Diagnostic — confirm KASLR overflow is pre-existing in `main`, not introduced by sub-commit 2

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing post-EXP-H working tree (sub-commit 2 minus §Q9, ADR-013 amendments appended uncommitted), unchanged since the closure attempt was blocked.
**Type:** Diagnostic. NOT a fix attempt. NOT a commit. Two QEMU runs, brief writeup. Working tree stays uncommitted.
**Substrate:** Closure attempt blocked by `addr.rs:390` overflow panic in `mm::init`. PM hypothesis: pre-existing latent KASLR bug. See `Plans/PM-DECISION-2026-05-06-kaslr-overflow-blocks-closure.md`.

---

## 0. Standing instructions

1. **Do NOT commit anything.** Working tree stays as it is at end of dispatch.
2. **Do NOT attempt to fix the KASLR overflow.** That's a separate triage item with its own design conversation.
3. **Do NOT modify `kernel/src/mm/heap.rs` or `kernel/src/mm/paging.rs`.** No changes anywhere in `kernel/src/mm/`.
4. **Capture verbatim output for both runs.**

---

## 1. The two questions

**Q1.** Does the panic reproduce on `main` HEAD (commit `9505619`, no sub-commit 2 changes) under conditions known to have produced it on the current working tree?

**Q2.** Does the panic vanish when KASLR is disabled or pinned to a known-safe seed?

YES + NO answers (in that order) confirm the bug is pre-existing latent and KASLR-roll-dependent. Any other answer pattern means we have to think harder.

---

## 2. Q1: reproduce on `main` HEAD

The challenge: the bug is rare under random KASLR. We saw it once in this dispatch's NEO_SMP=4 default-suite run. EXP-H's 100 runs all got lucky. Single-shot reproduction is not statistically meaningful.

The cheapest reliable approach: **run the same default suite at NEO_SMP=4 ten times** on `main` HEAD. If the panic surfaces in any of the ten runs, that's enough to confirm pre-existence. If it doesn't, it's still suggestive (because it's intermittent), but we need stronger evidence.

### 2.1 Set up a clean checkout of `main` HEAD

You currently have an uncommitted working tree. **Do NOT lose it.** Use `git stash` to save the current state, run the diagnostic, then `git stash pop` to restore.

```powershell
cd "E:\AI Automation\AltimitNeo"

# Save current uncommitted state (sub-commit 2 + §Q9 revert + ADR amendments)
git stash push -u -m "sub-commit-2-close-attempt-blocked-on-kaslr"

# Confirm we're at clean main HEAD (9505619)
git status
git rev-parse HEAD   # should print 9505619... (or whatever main HEAD is)
```

Verify with `git status` that the working tree is clean. If anything is still listed as modified, **stop and surface to PM** — `git stash` should have caught everything, and if it didn't we have a state-tracking problem.

### 2.2 Run NEO_SMP=4 default suite 10 times on main HEAD

```powershell
$results = @()
for ($i = 1; $i -le 10; $i++) {
    Write-Output "=== KASLR-Q1 Run $i ==="
    $env:NEO_SMP = "4"
    $r = docker compose run --rm neo cargo neo-test 2>&1 | Out-String
    $r | Out-File "target/verify/KASLR-Q1-main-run-$i.txt" -Encoding utf8

    if ($r -match "attempt to add with overflow") {
        Write-Output "KASLR-Q1 Run $i`: PANIC reproduced on main HEAD"
        $results += "PANIC"
        # Capture serial log if available
        $serial = "target/neo-test/ipc_heap_kill_smoke.serial.log"
        if (Test-Path $serial) {
            Copy-Item $serial "target/verify/KASLR-Q1-main-run-$i.serial.log"
        }
    } elseif ($r -match "NEO_TEST/1 event=PASS.*ipc_heap_kill_smoke") {
        Write-Output "KASLR-Q1 Run $i`: clean PASS"
        $results += "PASS"
    } else {
        Write-Output "KASLR-Q1 Run $i`: other failure (capture for review)"
        $results += "OTHER"
    }
}

$panic = ($results | Where-Object { $_ -eq "PANIC" }).Count
$pass = ($results | Where-Object { $_ -eq "PASS" }).Count
$other = ($results | Where-Object { $_ -eq "OTHER" }).Count
"Q1 RESULT: PANIC=$panic PASS=$pass OTHER=$other / 10 runs on main HEAD" | Out-File "target/verify/KASLR-Q1-summary.txt"
Write-Output "Q1 RESULT: PANIC=$panic PASS=$pass OTHER=$other / 10 runs on main HEAD"
```

Adapt to whatever shell/Docker invocation you've been using for the prior experiments (the loops in EXP-A through EXP-H are the reference shape).

### 2.3 Restore the uncommitted state

After Q1 runs:

```powershell
git stash pop
git status   # should show the same 12 modified files + ADR-013 amendments
```

Verify that `kernel/src/sched/process.rs`, `kernel/src/sched/scheduler.rs`, `docs/ADR-013-phase-2-smp-retrofit.md` etc. are back to the post-EXP-H state. If anything is missing or different, **stop and surface to PM**.

---

## 3. Q2: confirm KASLR is the trigger

Two sub-questions, run on the CURRENT working tree (post-stash-pop, sub-commit 2 + ADR amendments):

### 3.1 Q2a: KASLR disabled

If there's a way to disable KASLR (e.g., a build feature flag, a Limine config option, an environment variable that controls it), use it to do a single boot run.

The cleanest approach is probably modifying the Limine boot config to specify a fixed kernel base address, OR rebuilding without the KASLR feature. Look for:

- A `kaslr` feature in `kernel/Cargo.toml` or `Cargo.toml`.
- A `KASLR=on/off` or similar env var consumed by `xtask`.
- A Limine config option for fixed kernel addresses.

If you can disable KASLR cleanly, run one boot of NEO_SMP=4 ipc_heap_kill_smoke and capture the result. Expected: the kernel virt_base prints as the NON-randomized base (some specific deterministic value), and the boot completes without the overflow panic.

If you cannot disable KASLR cleanly (no flag exists, or disabling it requires non-trivial code changes), **skip Q2a** and rely on Q2b. **Do not write code to disable KASLR for this diagnostic** — that's scope creep.

### 3.2 Q2b: pin KASLR to a known-safe seed

If KASLR uses a seed (e.g., from RDRAND, RDTSC, or a Limine-provided random value), find the seed source. If the seed is logged at boot or can be set via Limine config, pin it to a safe value (any value that produced a successful boot in EXP-H — those 100 boots all had safe seeds).

If the seed is not exposed or is from RDRAND with no override, **skip Q2b**.

### 3.3 If both Q2a and Q2b are infeasible, skip Q2 entirely

We learn something useful even from Q1 alone. The decision tree:

- **Q1 = PANIC reproduces on main HEAD:** sub-commit 2 didn't introduce the bug. Pre-existence confirmed. Proceed to closure with corrected gate (PM dispatches separately).
- **Q1 = no panic in 10 runs on main HEAD:** suggestive but not conclusive (10 runs could all be lucky). PM thinks harder.

Q2 is corroborating evidence, not the load-bearing answer. Q1 is the load-bearing answer.

### 3.4 Capture for Q2

```powershell
# Q2a (if attempted)
docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke 2>&1 | Out-File "target/verify/KASLR-Q2a-no-kaslr.txt"

# Q2b (if attempted)
docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke 2>&1 | Out-File "target/verify/KASLR-Q2b-pinned-seed.txt"
```

Annotate each file with the actual KASLR-disabling mechanism you used.

---

## 4. Result to capture

Write a brief close report:

```
KASLR Q1 Result (10 runs on main HEAD at NEO_SMP=4):
  PANIC: N
  PASS: M
  OTHER: K
  First panic run (if any): <number or "n/a">
  First panic serial log: <path or "n/a">

KASLR Q2a Result (KASLR disabled, sub-commit 2 working tree):
  Mechanism: <how you disabled KASLR, or "infeasible">
  Result: <PASS / PANIC / SKIP>
  Boot virt_base observed: <value, if applicable>

KASLR Q2b Result (KASLR pinned to safe seed, sub-commit 2 working tree):
  Mechanism: <how you pinned the seed, or "infeasible">
  Result: <PASS / PANIC / SKIP>
  Boot virt_base observed: <value, if applicable>

Conclusion: <pre-existing / introduced-by-sub-commit-2 / inconclusive>

Working tree state: confirmed identical to pre-diagnostic state (12 modified
tracked files matching post-EXP-H, plus ADR-013 amendments).
```

---

## 5. What you do NOT do

- **Do not modify `mm/heap.rs` or `mm/paging.rs`** to add overflow guards. Even small ones.
- **Do not commit anything.**
- **Do not attempt fixes for the overflow** in any code anywhere.
- **Do not extend the diagnostic.** If Q1 reproduces and Q2 is infeasible, that's enough.

---

## 6. Decision rule

After this dispatch:

- **Q1 PANIC reproduces ≥ 1 time on main HEAD:** confirmed pre-existing. PM issues a corrected closure dispatch with the gate downgraded to the original "100x ipc_heap_kill_smoke at NEO_SMP=4" (which EXP-H already met). KASLR overflow becomes a separate triage item.

- **Q1 PANIC does NOT reproduce in 10 runs on main HEAD AND Q2a/Q2b show KASLR is the trigger:** ambiguous. Could mean main HEAD got lucky 10 times in a row (~99% probability if base rate is even ~1/30), or could mean sub-commit 2 amplified the bug. PM dispatches a higher-N Q1 (50 or 100 runs).

- **Q1 PANIC does NOT reproduce AND Q2a/Q2b show KASLR is NOT the trigger:** something else is overflowing on sub-commit 2's working tree specifically. New investigation needed; sub-commit 2 closure stays blocked.

- **Q2a or Q2b show KASLR-disabled DOES still panic:** KASLR is not the cause; something else overflows. New investigation.

In all cases: report results and stop. PM makes the next call.

---

## 7. Time budget

- Stash + clean checkout verification: 5 min.
- Q1 ten runs at NEO_SMP=4 default suite: ~30 min (each suite run is ~3 min).
- Stash pop + verify restore: 5 min.
- Q2a/Q2b (if feasible): ~15 min total.
- Writeup: ~10 min.
- Total: ~1 hour.

If anything blocks (e.g., stash doesn't restore cleanly, or a Q2 mechanism is unclear), surface to PM rather than push through.

---

## 8. Why this matters

The §Q5 lesson keeps applying: bets that look defensible can fail. The bet "this overflow panic is unrelated to sub-commit 2" looks defensible because the code is unchanged, but committing on that bet without verification is exactly the kind of move the §Q5 deletion bet failure warned against. The diagnostic is cheap; not running it would be cheaper but riskier.

This dispatch produces the evidence that allows sub-commit 2 closure to proceed honestly. If the bug IS introduced by sub-commit 2, we need to know that *now*, not after committing.

---

**End of dispatch.**

*Standard close report shape: Q1 result table, Q2 results (if attempted), conclusion (pre-existing / introduced / inconclusive), confirmation that working tree state is unchanged. ~1 hour.*
