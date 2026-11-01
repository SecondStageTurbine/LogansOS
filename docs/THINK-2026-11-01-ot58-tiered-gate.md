<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the tiered gate on the free tier (OT-58, 2026-11-01)

**Status:** proposal for the PM's read before building (dispatch: `Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`,
"Seat A, next").
**Author:** Seat A.
**Builds on:** `docs/THINK-2026-07-12-ci-gate-scaling-SYNTHESIS.md` (two tiers, one registry; fail safe, never fail
silent; escaped scenario promoted; measure first). That design assumed self-hosted runners and a nightly full run.
Neither exists now, so the "full tier" moves from the nightly to a **batch of PRs before they land**.

## 0. One breath

Every PR gets a **quick tier**: 4 jobs, one of them a fixed ~30-scenario QEMU smoke shard, targeting about 15 minutes
of machine time. Nothing lands on `main` without the **full tier**. It runs once over a *batch*: `main` plus several
quick-green PRs merged in order, as one tree. A PR that touches the SMP/concurrency core still gets the full tier on
its own. The PM then lands the batch in the same order and checks that `main`'s code tree equals the gated tree.
Coverage before merge does not drop at all; the cost is shared across the batch.

## 1. Measured baseline (run 41253927386, PR #13's gate, 2026-10-31, GitHub-hosted, all green)

| Job group | Jobs | Each | Notes |
|---|---|---|---|
| Nightly skip, SPDX, identity guard | 3 | < 0.2 min | |
| Host tests, fmt, lint (one job) | 1 | 7.0 min | |
| Product build | 1 | 2.8 min | |
| Feature-gated kernel clippy | 13 | 0.6-0.8 min | Most of each job is checkout plus toolchain install |
| QEMU shards | 4 | 38 / 54 / 66 / 97 min | Start at +7 min, after host-checks |
| **Total** | **22** | **wall 104 min** | |

What this shows:

1. **One full gate is 22 jobs, and the free tier runs 20 at once.** A single gate already exceeds the limit, so two
   seats gating at once queue behind each other.
2. **The shards are badly unbalanced on hosted runners: 38 to 97 minutes.** Yet `shard-coverage 4` reports
   109/114/114/116 scenarios and estimated weights of 12,212/12,197/12,197/12,193. The weight model
   (`scenario_shard_weight`) was calibrated on the old self-hosted fleet. It misses whatever makes shard 2 slow on a
   4-vCPU TCG runner.
3. **Typical scenario cost (the ~90 scenarios at the end of each shard):**
   - The kernel rebuild is 6-9 s and QEMU is 3-15 s.
   - 76 of those 90 scenarios spend under 10 s in QEMU.
   - The outliers are the NeoFS large-file scenarios: `neofs_large_write` 156 s, `phase3_neofs_cache_evict` 92 s.
   - The other 360 scenarios (the heavy desktop, login, install and Arthur ones included) are **not measured**. The
     job-log API returns only the last 5,000 lines of a 31-35k-line log, and the blob log host is not reachable from
     the seats. **So the first build slice is to record timings in a form that survives log truncation** (section 5,
     S0). This is the synthesis's own §3 rule: measure before tuning.

### 1.1 Measured: the full suite on hosted runners (S0, run 41267613561, 2026-11-01)

S0 (#32) prints every scenario's wall, QEMU and build time at the end of each shard. Its second gate gave the first
complete table: 449 scenarios, and every one ran. Shard 3's `neofs_large_write` timed out 4 × 180 s; it passes in
156 s, and #32 raises its limit.

| What | Measured |
|---|---|
| Whole suite, scenario time only | 307 min (shards 75 / 89 / 65 / 77 min) |
| Median scenario | 13.8 s: about 9 s build, 4-5 s QEMU |
| The 5 slowest | 90 min of the 307: `install_stop_finish` 1,839 s, `login_settings_cut` 1,227 s (71 boots), `login_cut_steps` 1,065 s (48 boots), `login_arthur_permission` 541 s, `install_console` 509 s |
| The 24-scenario smoke list | 352 s in total; slowest `wait_any_core` at 34 s. Every candidate meets the 20 s QEMU cost rule. |

What a partition on measured times (longest first, onto the lightest shard) would give, scenario time only:

| Shards (K) | Largest shard |
|---|---|
| 4 | 77 min (today's split: 89) |
| 6 | 51 min |
| 8 | 38 min |

Past K = 8, `install_stop_finish` alone (31 min) is the floor, so the next lever is that scenario, not more shards.

So the quick tier's smoke job is about 6 minutes of scenarios plus about 5 of setup. K = 6 is about 51 minutes plus
setup, in 6 + 4 = 10 jobs; K = 8 is about 38 minutes, in 12 jobs. With seats running quick tiers alongside, K = 6 still
looks like the right default. S3 will make the partition use these measured times.

## 2. The two tiers

### 2.1 Quick tier: every PR, 4 jobs

| Job | Contents | Expected |
|---|---|---|
| `plan` | `cargo xtask gate-plan --base <base>`: the tier for this diff (section 2.3), and the scenarios the PR adds or changes | < 1 min |
| `host-checks` | Unchanged: fmt, host tests, workspace clippy, kernel clippy (plain and neo-test-gated), shard coverage | ~7 min |
| `build-checks` | Product build, the 13 feature-gated clippy configurations **in one job, one after another, sharing one target dir**, SPDX, identity guard | ~8-12 min (to measure) |
| `smoke` | One QEMU job: `cargo neo-test --tier quick`, which runs the smoke list (2.2) plus every scenario the PR adds or changes | target ≤ 15 min |

- **Five seats at once:** 5 × 4 = 20 jobs, at the limit, but `plan` finishes in under a minute. That leaves about
  5 slots for a batch's full tier.
- **Folding the feature matrix** saves 12 checkout-and-toolchain setups. It also stops the 13 small jobs filling the
  20 slots that matter.

### 2.2 The smoke list

The smoke list is a reviewed `const SMOKE_SCENARIOS` in xtask. The rules for what goes on it:

1. **The boot and capability floor:**
   - one boot plus IPC smoke;
   - capability mint, grant and revoke;
   - the ring-3 fault set.
2. **Entry-path invariants that every kernel change could break:**
   - `exception_entry_flags`, `syscall_return_clean_regs`, `smap_policy`, `user_copy_bounds`, `irq_xmm_preserve`;
   - `user_copy_fault`, once #13 lands.
3. **The I-14 containment discriminators:** `dma_arena_fence`, `vtd_fence`.
4. **Lifecycle and SMP basics:**
   - `kill_at_exit` (CP-98), `exit_codes_bounded`, `threads_futex`, `smp_per_cpu_scheduler_smoke`;
   - `smp_cross_cpu_migration_reap`, `wait_any_core`, `sys_wait_blocking`.
5. **Every escaped-defect scenario.** This is the promotion rule from section 4.

**Cost rule:** a smoke scenario must spend under 20 s in QEMU on a hosted runner. A slower scenario that belongs on
the list goes to the full tier with a note, never to the quick tier.

**Budget:** the smoke step's measured wall-clock stays ≤ 12 minutes, checked by the S0 timing record.
- At ~15-25 s per scenario, that budget is about 30 scenarios.
- When a promotion would exceed it, the PM picks what moves out of the *quick* tier.
- Nothing ever leaves the full tier (discriminators stay armed).

The candidate list is 24 names, all in the default suite today (checked against `shard-coverage 4`'s listing):

- **Floor (9):** `kaslr_smoke`, `smp_same_cpu_ipc`, `cap_mint_and_use`, `cap_grant_basic`,
  `cap_revoke_descendants_walk`, `ring3_fault_de`, `ring3_fault_ud`, `ring3_fault_gp`,
  `boot_neofs_userland_persistence`.
- **Entry paths (6):** `exception_entry_flags`, `syscall_return_clean_regs`, `smap_policy`, `user_copy_bounds`,
  `irq_xmm_preserve`, `kaslr_heap_contract`.
- **I-14 (2):** `dma_arena_fence`, `vtd_fence`.
- **Lifecycle and SMP (7):** `kill_at_exit`, `exit_codes_bounded`, `threads_futex`, `smp_per_cpu_scheduler_smoke`,
  `smp_cross_cpu_migration_reap`, `wait_any_core`, `sys_wait_blocking`.

Plus `user_copy_fault` once #13 lands. Each one's hosted QEMU time is checked against the cost rule in S0 before the
list is fixed.
The build's host test asserts that every smoke name exists in the default suite, so a rename can't silently drop a
scenario.

**Changed scenarios run too.** `gate-plan` adds every scenario whose `TestScenario` entry, kernel feature or user
feature appears in the diff's added lines. A PR that adds or edits a scenario therefore runs it on its own quick tier.
Today that only happens at the full gate.

### 2.3 What forces the full tier on a PR alone (the concurrency-core filter)

`gate-plan` returns `tier=full-alone` when the diff touches the SMP/concurrency core. That is the same trigger as
CLAUDE.md's pitfall-bank and cross-family-AAG rules. The path list:

- **Scheduler, threads, kill, futex, wait:** `kernel/src/sched/{scheduler,thread,process,kthread,futex,wait_any,tick_sleep,mod}.rs`.
- **Locks:** `kernel/src/sync/**`.
- **Memory, all of `kernel/src/mm/**`:**
  - TLB shootdown and address-space teardown (`tlb_shootdown`, `pmap`);
  - the buddy/frame allocator, named 10 times in the pitfall bank; CP-29 is a cross-CPU deadlock on its lock;
  - the per-CPU slab (9 times) and the heap (35 times).
  
  The only files there that look non-core are pure `*_logic.rs` helpers, and the classification test lists them by
  name.
- **Per-CPU, SMP bring-up, entry paths, IPIs, timers, the SMAP window, FPU state:**
  `kernel/src/arch/x86_64/{smp,per_cpu,context,idt,apic,apic_logic,timer,lapic_timer,syscall,mod,gdt,fpu}.rs`.
  - `mod.rs` holds `with_user_access`, the SMAP window (CP-49).
  - `gdt.rs` holds the per-CPU TSS.
  - `fpu.rs` is the context-switch FPU save.
- **Cross-CPU kernel work outside these directories:**
  - `kernel/src/ai/par_matmul.rs` (threaded matmul, 3 bank entries);
  - `kernel/src/ipc/mod.rs` (channel parks and wakes);
  - `kernel/src/watchdog.rs`.
- **The test harness itself and xtask's runner:** `kernel/src/test_harness.rs`, `tools/xtask/**`.
  - A change to these changes what every scenario *means*, so it is also gated alone.
  - Adding a scenario touches both files. So `gate-plan` treats a diff that touches them *only* in lines that add a
    scenario as a quick-tier change.
  - That is a narrow, host-tested pattern: a new `TestScenario { … }` block, its `cfg` arm and its `SCENARIO`
    const, and nothing else in either file.
  - **Not built yet.** S1 treats every change to these two files as full-alone until the PM answers question 3.
- **The workflow itself:** `.github/workflows/**`.

`syscall.rs` is on the list because every syscall's entry, exit and SMAP window runs there. #13 is the example: it
would have been gated alone.

**Fail-safe (synthesis C4):** a path `gate-plan` can't classify, or a diff it can't compute (no merge base, a
shallow fetch), gives `full-alone`, never `quick`. An xtask host test pins the path list and asserts that every file
under `kernel/src/sched`, `kernel/src/sync`, `kernel/src/mm` and `kernel/src/arch/x86_64` is either on the core list
or on an explicit non-core list.

The list was checked against `docs/concurrency-pitfalls.md`. I counted the kernel files each entry names, and every
file named twice or more is on the core list. So a new file must be classified before CI goes green.

### 2.4 Full tier: on a batch, before landing

A `workflow_dispatch` input, `batch`: a space-separated list of PR numbers, in landing order.

1. `plan` resolves each PR's head SHA once and prints the list. If any PR is not quick-green on that exact head, or
   is `full-alone`, the job fails: it can't share a batch.
2. Every later job assembles the same tree: `main` at a fixed SHA (also resolved once), then
   `git merge --no-ff --no-edit <head>` for each PR in order, with a fixed committer name and date.
   - **A conflict fails the batch** and names the PR.
   - Each job prints the tree hash, and a final `batch-report` job checks they are all equal.
   - Merging this way is deterministic, so every job tests the same tree without anyone pushing a branch.
   - Pushing one would need write access, and a push that touches `.github/workflows/` is refused for
     `GITHUB_TOKEN` anyway.
3. **Jobs:** `host-checks` + `build-checks` + `neo-test` shards (the full suite) + `batch-report`. That is K+4
   jobs; with K = 6 it is 10, leaving room for a few quick tiers alongside.
4. **Landing:**
   - The PM lands the PRs in the batch's order.
   - Then the PM checks that `main`'s code subtrees equal the gated tree. This is the existing tree-identity rule:
     `kernel`, `userland`, `tools`, `Cargo.*`, `.github`.
   - If `main` moved meanwhile with docs-only commits, the code trees still match. Any code difference means
     re-batching.
5. **A batch of one is the same thing.** `batch: 13` is how a `full-alone` PR gets its gate. With no `batch` input,
   `workflow_dispatch` on a branch runs today's full gate on that branch (the kill-switch, section 6).

The shard count becomes an input, `shards`, defaulting to 4. The workflow matrix is built from it by `plan`, and the
`shard-coverage` check runs for the same N.

### 2.5 Keep going on failure

`neo-test` stops at the first failing scenario (the `?` in `cmd_neo_test`'s loop). On #13's first gate, shard 3 failed
on `exception_entry_flags` with 101 of its 115 scenarios never run.

For a batch that's the wrong behaviour: the bisect (section 4) needs every failing scenario, not the first. So:
- `--keep-going` runs the rest and exits non-zero with the list of failures. CI's full tier uses it.
- Local runs keep stopping at the first failure.

## 3. Throughput

These figures are machine time on the free tier, and estimates until S0 measures them.

- **Quick tier:**
  - The smoke job's ~5 minutes of setup (free disk, the dev container from cache, toolchain) is about the same as a
    shard's today.
  - About 30 scenarios at 15-25 s each adds 8-12 minutes, so roughly 15 minutes per PR.
  - Five seats fit at once.
- **Full tier:**
  - At 453 scenarios, the full suite's QEMU work is fixed per batch, so batching N PRs divides it by N.
  - With 6 shards **rebalanced on hosted-runner timings** (S3), the batch's wall-clock is set by the largest shard. A
    balanced 6-way split of today's ~255 shard-minutes is about 45 minutes, plus setup.
  - Today one gate costs 104 minutes and 22 jobs per PR.
- **Merge funnel:** the PM's verify-before-merge stays per PR. Only the expensive QEMU half is shared.

## 4. Red batches, escapes, promotion

- **A red batch is bisected by PR, cheaply.**
  - The `batch-report` job lists the failing scenarios (from `--keep-going`).
  - A dispatch with `scenarios: "<names>"` and `ref` set to a PR's head runs only those scenarios, in one job, on
    that PR alone. It runs on each member's head, plus the batch base.
  - The PR whose head reproduces the failure is bounced, and the batch re-forms without it.
  - If no single head reproduces it, the failure is an interaction. Re-run the scenarios on growing prefixes of the
    batch, in order; the first prefix that fails names the PR.
- **`main` stays green.** Only batch-gated trees land, so a red can reach `main` only through a landing that didn't
  match the gated tree, which the tree-identity check catches. The escaped-defect contract (revert first; the PL's
  4-hour fix-forward window; infra reds exempt) is unchanged.
- **Promotion:**
  - When a scenario fails in a batch after its PR's quick tier passed, the defect got past the quick tier.
  - The PM adds that scenario to `SMOKE_SCENARIOS` (a reviewed one-line PR), within the cost rule.
  - That is the synthesis's C6, with "core" now meaning the quick tier.

## 5. Build slices

| Slice | What | Gate |
|---|---|---|
| S0 | Timings that survive log truncation:<br>- `neo-test` writes `target/neo-test/timings.tsv`: scenario, build s, image s, QEMU s, attempts, result.<br>- At the end of a shard it prints a sorted summary table to `$GITHUB_STEP_SUMMARY`.<br>- It uploads the TSV as an artifact on every run (3-day retention, `continue-on-error`).<br>- `--keep-going` (2.5). | Host tests; one full-tier run to collect the data |
| S1 | xtask:<br>- `SMOKE_SCENARIOS`, with a host test that every name exists, is in the default suite, and is not hardware-only.<br>- `--tier quick`.<br>- `gate-plan` (changed scenarios, core paths, fail-safe), with host tests over fixture diffs: a core path gives full-alone; a scenario-only harness diff gives quick; an unknown kernel arch file fails the classification test; no merge base gives full-alone.<br>- `--scenarios`. | Host only (push-CI). No kernel change. |
| S2 | Workflow:<br>- the `plan` job;<br>- `build-checks` folding the feature matrix;<br>- the `smoke` job;<br>- the `tier`, `batch`, `shards` and `scenarios` inputs;<br>- batch assembly with the tree-hash check;<br>- `batch-report` with the failing-scenario list;<br>- quick tier on `pull_request` (also on `synchronize`, with concurrency cancel: hosted runners don't make a re-run cost a self-hosted slot). | The defaults reproduce today's gate. A dispatched full run, a quick run and a 2-PR batch, all on the branch. |
| S3 | Rebalance `scenario_shard_weight` on the S0 hosted-runner timings. Choose the default K from the data (6 is the working figure). | `shard-coverage` plus one full run, comparing the spread to today's 38-97 min |
| S4 | Docs: CLAUDE.md's close-gate and merge-bar sections; the runbook, with batch landing and the tree-hash check, red-batch bisect and the promotion rule. | Docs only |

S0 and S1 are independent, and both are host-only. S2 needs S1. S3 needs S0's data.

## 6. Kill-switch, invariants, what stays the same

- **Kill-switch:** `workflow_dispatch` with no `batch` and `tier: full` (the default during the transition) runs
  exactly today's gate:
  - the same jobs (bar the folded feature matrix, which runs the same 13 clippy invocations);
  - 4 shards and the same partition;
  - stop at the first failure.
  
  A host test pins `--shard i/4` without `--tier` to today's partition. `shard-coverage 4` already checks
  completeness. Every seat's current gate command keeps working unchanged until S4 switches the documented default.
- **New invariant (named, with its guard):** *nothing lands on `main` unless a full-tier run passed on a tree whose
  code subtrees equal `main`'s after the landing.*
  - The guard is the batch's tree-hash check plus the PM's post-landing tree comparison.
  - It replaces "every PR's own head passed the full gate". It is the same guarantee on the tree that actually
    lands, which today's gate does not check: a PR gated on a stale base that is then rebased has had its tree
    changed.
- **Unchanged:**
  - hermeticity;
  - the per-scenario timeouts (a scenario that fails on time on hosted runners still gets its timeout raised, with
    the elapsed time recorded);
  - the infra-red exemption, the AAG, cross-family reviews, the close-gate script;
  - the PM's per-PR `git show --stat` inspection.

## 7. Prior art (checked 2026-11-01; read for design, no code taken)

| Project | Last commit | License | What it does | What we take |
|---|---|---|---|---|
| rust-lang/bors | `68c45b6` 2026-10-25 | MIT OR Apache-2.0 | Rust's merge bot: try builds, and "rollups" that test several approved PRs as one merge | The rollup idea: one expensive run over several PRs, plus unrolled per-PR builds to find the culprit (our `scenarios` bisect) |
| rust-lang/homu | `8ead3f8` 2025-11-04 | MIT | The previous Rust merge bot. Introduced the rollup. | The same design, in its first form |
| bors-ng/bors-ng | `ca72579` 2024-01-31 | Apache-2.0 | Merge queue for GitHub. Deprecated by its author in favour of GitHub's merge queue. | The queue model; abandoned, so nothing vendored |
| GitHub merge queue | (product) | n/a | `merge_group` event: tests PRs merged together before they land | Considered as the batch mechanism. Not chosen: it needs repo-settings changes only J.c. can make, and its availability on this repo's account type is unverified. Our `batch` input does the same thing in the workflow we already run. Worth a look later. |

Commits and licenses were read from shallow clones (`git log -1`, the license files), not from a summary.

## 8. Questions for the PM

1. **Smoke list sign-off.** Do the 24 candidates in 2.2 look right? Is anything on the list there for a reason that
   isn't concurrency-core?
2. **Quick tier on every PR push (`synchronize`).** Hosted runners make that cheap. Today PRs run once, at open.
3. **`tools/xtask/**` and `test_harness.rs` as full-alone, with the scenario-only exception.** Is that the line you
   want, or should any xtask change go quick and rely on the batch?
4. **K = 6 default shards** until S3's data says otherwise?
