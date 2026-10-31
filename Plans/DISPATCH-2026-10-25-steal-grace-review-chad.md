<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #52): post-merge review of the time-based matmul steal grace (#116) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. The change touches the SMP fork-join core: when the BSP may take over an
AP's unclaimed matmul partition. Claude-authored SMP-core changes get a Codex-family review (CLAUDE.md).

**Type:** review only, post-merge (the PL's rule: merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `2817bbaa` (main; the squash of PR #116, gated as `0c700823`,
run `40149510145`, 23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/steal-grace-review-chad
          git fetch origin && git checkout -B codex/steal-grace-review-chad 2817bbaa2338f910c8787162a5309e0220b1e543

git show origin/main:Plans/DISPATCH-2026-10-25-steal-grace-review-chad.md   # this file
git diff f450f4f9 2817bbaa -- kernel/src/ai/par_matmul.rs kernel/src/ai/par_matmul_logic.rs \
    kernel/src/arch/x86_64/cpufreq.rs kernel/src/arch/x86_64/cpufreq_logic.rs kernel/src/ai/prefill_perf.rs \
    tools/ai-ops-tests/src/lib.rs docs/concurrency-pitfalls.md
```

Read `docs/concurrency-pitfalls.md` CP-20, CP-25, CP-26 and CP-80 first.

RAG (semantic code search) is available; use it for "where is X done?". Health check:
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note it in the RETURN and use grep/Read; the PL manages
it. Example query: `$body=@{query='par_matmul run_generic barrier_action steal claim ap_poll'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with the enforcing `file:line`. For a FAIL, give a concrete sequence (which CPU does
what, in what order) or input.

1. **Only unclaimed slots are stolen.** With `matmul-steal-grace-time` on, the BSP can steal a slot only while
   `pending == true` (unclaimed), and stealing and an AP's claim go through one atomic claim, so exactly one executor
   runs each partition. The time grace changes only *when* the BSP tries.
2. **A claimed slot is never stolen or abandoned.** A slot an AP has claimed but not finished (`pending == false`,
   `done == false`) is waited out up to `BARRIER_LIMIT` (the fail-stop), never stolen, whatever the grace says.
3. **The grace arithmetic.** `past_steal_grace` (par_matmul_logic.rs): `tsc_hz / 1_000_000 * grace_us` can't
   overflow or truncate to 0 for any TSC rate `lapic_timer::tsc_hz()` can return. `rdtsc().wrapping_sub(published)`
   is correct when the BSP reads TSC on the same CPU it published from, and wrap-around can't cause an early steal.
   When `tsc_hz() == 0` (uncalibrated), it falls back to the spin count.
4. **Ordering.** The `published` timestamp is taken after every slot's `pending.store(true, Release)`, so the grace
   can't start before an AP can see the job.
5. **Liveness (CP-20/CP-25).** With an AP absent or busy for the whole job, the BSP still completes the job in bounded
   time: steal after the grace, run the partition, join. Shortening the grace can't cause a deadlock or a livelock.
   For example: BSP steals → AP claims late → both run the same columns.
6. **Results unchanged (CP-26).** The output columns are byte-identical whether a partition ran on its AP or was stolen.
   `threaded_matmul_parity` covers this with the feature on. Cite the scenario and the CI arm that runs it.
7. **The per-partition timing (`prefill_perf::partition_begin/end`).** It takes no lock and writes only per-CPU or
   atomic state, so it can't change claim/steal/join ordering. Check the AP path in `ap_poll` too.
8. **The MSR reads (`cpufreq.rs`).** Every RDMSR is behind the CPUID bit for that MSR, on the CPU that executes it.
   HWP_CAPABILITIES/HWP_REQUEST are read only when PM_ENABLE reads 1. No MSR is written unless `hwp-performance` is
   built, and that feature isn't in image A's list (`tools/xtask/src/main.rs`).
9. **Kill-switch.** With `matmul-steal-grace-time` off, `run_generic`'s grace is the old `spins >= STEAL_GRACE_SPINS`
   exactly.

## Close condition

- Commit `Plans/RETURN-2026-10-25-steal-grace-review-chad.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the pushed SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
