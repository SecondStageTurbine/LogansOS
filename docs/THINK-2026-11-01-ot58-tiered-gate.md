<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the tiered gate on the free tier (OT-58, 2026-11-01)

**Status:** revision 2. It answers the cross-family review (Dexter, relay 235,
`Plans/RETURN-2026-11-01-pr31-tiered-gate-review-dexter.md` on `dexter/pr31-review`, findings F1-F31). Section 9 maps
every finding to its resolution. For the PM's read before building (dispatch:
`Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`, "Seat A, next").
**Author:** Seat A.
**Builds on:** `docs/THINK-2026-07-12-ci-gate-scaling-SYNTHESIS.md`:
- two tiers, one registry;
- fail safe, never fail silent;
- escaped scenarios are promoted;
- measure first.

That design assumed self-hosted runners and a nightly full run. Neither exists now, so the "full tier" moves from the
nightly to a **batch of PRs before they land**.

## 0. One breath

**Quick tier, on every PR:** 3 jobs, one of them a fixed 31-scenario QEMU smoke shard, about 13 minutes of machine
time.

**Full tier, before anything lands on `main`:**
- It runs once over a *batch*: `main` at a fixed base plus several quick-green PRs, merged in order, as one tree.
- A PR that touches the SMP/concurrency core, the test machinery, the workflow or the build configuration gets the
  full tier on its own instead.

**Landing:**
- Before pushing, the PM rebuilds the batch tree locally and checks its hash against the gated one.
- The PM lands the batch in one push.
- One script reverts the whole batch in one commit if needed.

Coverage before merge does not drop at all; the cost is shared across the batch. Several standing rules change with
the merge unit, and section 6 states each one.

## 1. Measured baseline (run 41253927386, PR #13's gate, 2026-10-31, GitHub-hosted, all green)

| Job group | Jobs | Each | Notes |
|---|---|---|---|
| Nightly skip, SPDX, identity guard | 3 | < 0.2 min | |
| Host tests, fmt, lint (one job) | 1 | 7.0 min | |
| Product build | 1 | 2.8 min | |
| Feature-gated kernel clippy | 13 | 0.6-0.8 min | Most of each job is checkout plus toolchain install |
| QEMU shards | 4 | 38 / 54 / 66 / 97 min | Start at +7 min (`needs: host-checks, product-build`) |
| **Total** | **22 jobs** | **104 min, first job start to last job end** | |

What this shows:

1. **One full gate is 22 jobs, and the free tier runs 20 at once (account-wide).** A single gate already exceeds the
   limit, so two seats gating at once queue behind each other.
2. **The shards are badly unbalanced on hosted runners: 38 to 97 minutes.** Yet `scenario_shard_weight` estimates
   them equal to 0.2%. It was calibrated on the old self-hosted fleet.

### 1.1 Measured: every scenario on hosted runners (S0, run 41267613561, 2026-11-01)

S0 (#32) prints every scenario's wall, QEMU and build time at the end of each shard. Its second gate gave the first
complete table: 449 scenarios, and every one ran. `neofs_large_write` timed out 4 × 180 s; it passes in 156 s, and
main's #19 raised its limit to 300 s.

| What | Measured |
|---|---|
| Whole suite, **sum** of scenario wall times | 307 min (per-shard sums 75 / 89 / 65 / 77) |
| Median scenario | 13.8 s wall: about 9 s build, 4-5 s QEMU |
| The 5 slowest | 90 of the 307 min: `install_stop_finish` 1,839 s, `login_settings_cut` 1,227 s (71 boots), `login_cut_steps` 1,065 s (48 boots), `login_arthur_permission` 541 s, `install_console` 509 s |

**Largest shard (= the matrix's scenario wall time)** under a partition on measured times (longest first, onto the
lightest shard). Each shard also has about 5 min of setup.

| Shards (K) | Largest shard |
|---|---|
| 4 | 77 min (today's split: 89) |
| 6 | 51 min |
| 8 | 38 min |

Past K = 8, `install_stop_finish` alone (31 min) is the floor, so the next lever is that scenario, not more shards.

## 2. The two tiers

### 2.1 Quick tier: every PR, 3 jobs, no `needs`

Every quick job runs `cargo xtask gate-plan` as its first step (about 1 minute, including the xtask build), so there
is no separate `plan` job. A job whose plan says `full-alone` stops at once with a notice: the PR needs a batch of
one.

The three jobs start together:

| Job (check-run name) | Contents | Measured or expected |
|---|---|---|
| `Quick: host-checks` | Unchanged: fmt, host tests, workspace clippy, kernel clippy (plain and neo-test-gated), shard coverage | 7.0 min (measured) |
| `Quick: build-checks` | The product build, the 13 feature-gated clippy configurations **in one job, sharing one target dir**, SPDX, identity guard | about 10 min (S2 measures it) |
| `Quick: smoke` | `cargo neo-test --tier quick --changed-from <base>`: the smoke list (2.2) plus the PR's eligible changed scenarios (2.2.1). It builds its own kernels, as a shard does today. | about 5 min setup + 7.7 min of scenarios = about 13 min |

**Critical path:** about 13 minutes, set by `smoke`. Nothing waits on another job.

**What the smoke job checks for itself (F6).** It reads its own `timings.tsv` at the end and fails if the scenarios
took more than 12 minutes of wall time. The failure names the slowest scenario, so a budget overrun is a red, never a
silent slowdown.

### 2.2 The smoke list (31 scenarios, 461 s measured)

**Cost rule:** a smoke scenario's measured hosted **wall** time (build + image + QEMU) is at most 35 s. Wall, not
QEMU, because build is most of a typical scenario's cost: `kill_ready_in_kernel` spends 3.5 s in QEMU and 63 s in
total. A scenario that belongs on the list but breaks the rule goes in the full-tier-only table below, with its
numbers.

The list, with measured wall / QEMU seconds from run 41267613561:

- **Boot and capability floor (11):**
  - `kaslr_smoke` (11.9/3.4): the kernel boots at a randomized base, the floor every other scenario stands on;
  - `smp_same_cpu_ipc` (13.4/3.8): boot plus IPC;
  - `cap_mint_and_use` (12.6/4.3), `cap_grant_basic` (12.4/4.2), `cap_revoke_descendants_walk` (12.6/4.3);
  - `boot_neofs_userland_persistence` (21.8/11.3);
  - **the whole ring-3 fault set in the default suite:** `ring3_fault_de` (15.2/5.1), `_db` (15.1/5.2), `_ud`
    (14.5/5.2), `_gp` (13.9/4.4), `_mf` (13.9/5.0). `_ss` and `_xm` are `ignored: true` in xtask.
- **Entry paths and context (7):** `exception_entry_flags` (12.2/4.3), `syscall_return_clean_regs` (12.3/4.3),
  `smap_policy` (13.2/4.1), `user_copy_bounds` (13.8/4.1), `irq_xmm_preserve` (12.8/3.4), `kaslr_heap_contract`
  (12.1/4.2), `xsave_ymm_context_switch` (12.9/4.2).
- **Containment and disk safety (3):** `dma_arena_fence` (12.5/3.5), `vtd_fence` (12.8/4.8), and `block_write_guard`
  (13.2/5.3), the `GuardedDevice` "never write the internal disk" discriminator.
- **Lifecycle and SMP (7):** `kill_at_exit` (18.6/10.2), `exit_codes_bounded` (13.9/4.7), `threads_futex` (18.4/8.5),
  `smp_per_cpu_scheduler_smoke` (12.0/3.9), `smp_cross_cpu_migration_reap` (11.9/4.1), `wait_any_core` (33.9/25.4),
  `sys_wait_blocking` (13.8/4.9).
- **Escaped defects (3; the promotion rule, applied to past escapes):**
  - `swapgs_irq_ring3` (26.8/18.3): OT-59, the TLB-shootdown/EOI deadlock;
  - `virtio_blk_io_gate_contention` (13.8/5.6): OT-59 #2, the single-in-flight submit/wait;
  - `lock_route_races` (12.9/4.1): CP-97, the lock-route races.

  `threads_futex` (the C1.1 lost wakeup) and `kill_at_exit` (CP-98) are listed under lifecycle above.

**Full-tier only, excluded by the cost rule (F1's "with a note"):**

| Scenario | Why it would belong | Measured wall / QEMU |
|---|---|---|
| `login_lock_park` | CP-97's second discriminator (the WM parked during Super+L) | 102.9 s / 88.3 s |
| `kill_ready_in_kernel` | A kill of a thread that is Ready inside a syscall (CP-95) | 63.3 s / 3.5 s |

`user_copy_fault` joins the list once #13 lands, if its measured wall time meets the rule.

**Host tests in S1:**
- every smoke name exists, is in the default suite and is not hardware-only;
- the list matches a checked-in timing table (`tools/xtask/data/hosted-timings.tsv`, S0's TSV) and every entry meets
  the 35 s rule;
- the excluded table's names exist.

#### 2.2.1 Changed scenarios (F3)

`gate-plan` finds the scenarios a diff adds or edits: their name in quotes, or their kernel or user feature as a
whole token in an added line. Such a scenario joins the quick tier only if it passes the same budget:

1. **It is in the timing table:** its measured wall time is at most 35 s.
2. **It is new (no row):** its `scenario_timeout` is the 180 s default, and it is not a multi-boot or install
   scenario (xtask knows both: `run_two_boot_scenario_with_retries`, `is_install_scenario`).

Anything else is full-tier only for that PR, printed as `changed_full_only=<names>`. The 12-minute budget check (2.1)
backs this up: a new scenario that is slower than its kind suggests turns the smoke job red. Its PR then adds the
scenario's row to the timing table.

### 2.3 What forces the full tier on a PR alone

`gate-plan` returns `tier=full-alone` when the diff touches any path in the classes below. The list follows the
AAG's SMP-core definition in CLAUDE.md: scheduler, thread lifecycle/teardown, futex/wait-queue, cross-CPU migration,
per-CPU accounting, lock ordering and kill paths. It was checked against `docs/concurrency-pitfalls.md`: every kernel
file the bank names twice or more is on it.

1. **Scheduler, threads, kill, futex, wait:**
   - `kernel/src/sched/{scheduler,thread,process,kthread,futex,wait_any,tick_sleep,mod,wake_trace}.rs`;
   - `wake_trace.rs` is per-CPU and is `threads_futex`'s instrument.
2. **Locks:** `kernel/src/sync/**`.
3. **Memory:** all of `kernel/src/mm/**` except four pure `*_logic.rs` / requirements helpers that the classification
   lists by name. That covers:
   - the buddy/frame allocator: 10 bank entries, and CP-29 is a cross-CPU deadlock on its lock;
   - the per-CPU slab: 9 entries;
   - the heap: 35 entries;
   - TLB shootdown and `pmap`.
4. **Per-CPU, AP bring-up, entry paths, IPIs, timers, the SMAP window, FPU state:**
   `kernel/src/arch/x86_64/{smp,per_cpu,context,idt,apic,apic_logic,timer,lapic_timer,syscall,mod,gdt,fpu}.rs` and
   `kernel/src/main.rs` (`bring_up_aps`, the matmul thread count).
5. **Cross-CPU work in other directories:**
   - `kernel/src/ai/par_matmul.rs`;
   - `kernel/src/ipc/mod.rs` (channel parks and wakes);
   - `kernel/src/watchdog.rs`;
   - `kernel/src/drivers/block/virtio_blk.rs` (OT-59 #2's `io_gate`, a driver-owned `SwitchMutex`).
6. **Build configuration (F16):**
   - `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `Dockerfile`, `docker-compose.yml`, `.cargo/**`,
     `kernel/.cargo/**`, and the kernel's target spec and linker script;
   - `kernel/Cargo.toml`, except when every changed line in it adds a `neo-test-*` feature. A change to `default`,
     or to any feature that isn't a test feature, is full-alone.
7. **The test machinery:**
   - `kernel/src/test_harness.rs` and `tools/xtask/**`, with the scenario-only exception in 2.3.1;
   - `scripts/**`, which CI and the close-gate run;
   - `.github/workflows/**`.

**Totality (F15).** Every file under `kernel/src/` must be on the core list or the non-core list, and a host test
fails on any file that is on neither.
- The non-core list mostly names whole directories: `fs/`, `net/`, `atrium/`, `acpi/`, `loader/`, `log/`, `rng/`,
  `init/`, `scenario/`, `ai/`, `drivers/`.
- It also names single files: `sched/*_logic.rs`, `arch/x86_64/` helpers like `rtc.rs` and `pit.rs`, and the
  top-level `*_logic.rs`.
- A specific core entry beats a non-core directory: `drivers/block/virtio_blk.rs` is core, while the rest of
  `drivers/` is not.
- At run time, any `kernel/**` path that is on neither list is full-alone. So the check fails safe if the test is
  ever skipped.
- A git failure (no merge base, a shallow fetch) is also full-alone, never quick.

#### 2.3.1 The scenario-only exception (F17; PM question 3)

A real new scenario touches:
- `tools/xtask/src/main.rs`: a `TestScenario { … }` block;
- `kernel/Cargo.toml`: a `neo-test-<new>` feature;
- `kernel/src/test_harness.rs`: the `SCENARIO` const, the `any(...)` feature-list arms, and a run body;
- often `userland/` code, which is quick anyway.

The exception applies only when all of these hold:
- every change to those three files is an **added** line (no removed or modified lines);
- every added line in `test_harness.rs` is a `feature = "neo-test-<new>",` list arm, a
  `#[cfg(feature = "neo-test-<new>")] const SCENARIO…` line, or sits inside an item or block whose opening line is
  `#[cfg(feature = "neo-test-<new>")]` (tracked by brace depth over the added lines);
- `<new>` is a feature this same diff adds in `kernel/Cargo.toml`;
- every added line in `main.rs` lies inside the new `TestScenario { … }` block.

Code gated on a feature that only the new scenario enables cannot change any other scenario's kernel. Anything else
in those files is full-alone. S1 ships fixture diffs for the match (a #13-shaped addition) and for three near-misses:
a removed line, an ungated line, and a block gated on an existing feature.

**Until the PM answers question 3, S1 builds without this exception** (all harness/xtask changes are full-alone).

### 2.4 Full tier: on a batch, before landing

**Inputs** (`workflow_dispatch`):
- `batch`: PR numbers, in landing order;
- `base_sha` (optional): defaults to main's tip at plan time;
- `prefix` (optional): run only the first N members, for bisecting.

**Quick-green, by machine rule (F7).** For each member, `batch-plan` resolves the head SHA once. It then reads
`GET /repos/{owner}/{repo}/commits/{sha}/check-runs` and requires each of `Quick: host-checks`,
`Quick: build-checks` and `Quick: smoke` to be present with conclusion `success` on **that exact SHA**.

These don't count as green:
- `skipped`, `neutral` or `cancelled`;
- a missing check run;
- a green full-gate run without the quick checks;
- any member whose `gate-plan` is `full-alone`.

The job prints every accepted check-run ID and outputs `base_sha` and the ordered head SHAs.

**Assembly.** Every later job builds the same tree:
- `main` at `base_sha`, then `git merge --no-ff --no-edit <head>` for each member in order;
- the committer name and date are fixed;
- a conflict fails the batch and names the PR.

Each job prints the tree hash; `batch-report` checks they are all equal and prints the batch tree hash.

**Jobs:**
- `Batch: host-checks`, `Batch: build-checks` and K neo-test shards, with `--keep-going`;
- `batch-report`, a small job at the end that collects every shard's `timings.tsv` from the artifacts and lists the
  failures, classified (2.5).

**Landing (F9):**
1. **Before pushing,** the PM rebuilds the batch tree locally (the same base, merges, committer and date) and checks
   that its hash equals `batch-report`'s. A mismatch stops the landing; `main` is untouched.
2. **One push** lands the whole batch: the N merge commits on `base_sha`, in order. If `main` moved past `base_sha`:
   - a docs-only move means the PM rebases the batch's merges onto it, and the check below must still hold;
   - any code move means re-batching.
3. **The check:** `git diff --quiet <batch tree> <landed main> -- . ':!Plans' ':!docs' ':!*.md'`. That compares
   everything except docs (F28), so `Dockerfile`, `scripts/`, `tests/`, `fixtures/`, `boot/` and the toolchain file
   are all included.
4. **Revert unit:** `scripts/revert-batch.sh <base_sha>` reverts the batch's merge commits in reverse order into
   **one** commit (`git revert --no-commit -m 1 …`, then one commit naming the batch). That keeps `main` one action
   from green.

**A batch of one** is how a `full-alone` PR gets its gate.

**The kill-switch.** With no `batch` input, `workflow_dispatch` on a branch runs today's full gate on that branch.

### 2.5 Red batches: failures, bisect, owners

- **`--keep-going` (S0, done):** a batch shard runs every scenario and reports every failure.
- **Classification (F11, F12).** `batch-report` sorts what went wrong:
  - **Scenario failures:** rows in the TSV marked `FAIL` with an assertion or exit status.
  - **Build failures:** a scenario's kernel or userland build failed. S2 has `--keep-going` record these as `FAIL`
    rows tagged `build`, instead of exiting.
  - **Infra:** a job that ended with no TSV, or with runner loss, disk-full or OOM in its log. These are re-fired once
    and never bisected (CLAUDE.md's infra-red exemption).
  - **Timeouts:** re-run once on the full batch before bisecting. Hosted TCG time varies, and #19 is the precedent.
- **Bisect by prefix (F8, F13).** Prefix runs are the primary method.
  - A dispatch with `batch` plus `prefix=N` runs only the failing scenarios (`cargo neo-test <names>`, in one job) on
    the first N members.
  - Binary search over N takes ⌈log₂ members⌉ rounds of one job each. A round is about 5 min of setup plus the
    failing scenarios, warm from the cache.
  - The first prefix that fails names the PR that introduced the failure, even when the cause is an interaction with
    an earlier member.
  - `prefix=0` is the base alone. A failure there is a `main` problem, handled by the escaped-defect contract.
- **Owner (F10).** The PM names the owner: the author of the first failing prefix's last member. That PR is bounced,
  and the batch re-forms without it. If an interaction failure has no single owner, the batch is not landed. Nothing
  is on `main` to revert, because nothing lands red.

## 3. Throughput (free tier: 20 jobs at once, account-wide)

Counted as jobs running at the same time:

| What is running | Jobs |
|---|---|
| One seat's quick tier | 3 (critical path about 13 min) |
| A batch, K = 6 | 2 + 6 shards = 8, then `batch-report` (1, about 1 min, after the shards) |
| `main`'s post-landing push run | 2 (S2 folds it to host-checks + build-checks; no QEMU, as today) |

| Mix | Jobs | Fits 20? |
|---|---|---|
| 4 seats' quick tiers | 12 | yes |
| 4 seats + a batch (K = 6) | 20 | yes, exactly |
| 5 seats + a batch (K = 6) | 23 | no: 3 jobs queue |
| 5 seats + a batch (K = 5) | 22 | no: 2 queue |
| A landing's `main` run + 4 seats | 14 | yes. The batch has finished before it lands, so the two never overlap. |

**Scheduling rule (F19, F20):**
- The PM dispatches a batch only when at most 4 quick tiers are running.
- While a batch runs, a fifth seat's quick tier queues for at most one smoke duration (about 13 min). That is
  acceptable, and the PM sizes K to the free slots.
- `ot61-closure.yml` also runs on dispatch, and on pushes to `claude/ot61-**` branches and `main` that touch the
  browser paths, `tools/xtask/**` or `rust-toolchain.toml`. Its jobs count toward the 20, so a batch can be squeezed
  by a browser push. Seat D's pushes are the main source; the PM can see them in the run list before dispatching a
  batch.

**Batch wall time (F22):**
- The largest measured shard at K = 6 is 51 min, plus about 5 min of setup: about 56 min per batch, whatever the
  batch size.
- Today the same coverage costs 104 min and 22 jobs **per PR**.
- With batches of three or more, the full tier costs each PR less than a fifth of today's.

## 4. Escapes and promotion

- **Promotion.** A scenario that fails in a batch, after its PR's quick tier passed, is a defect that got past the
  quick tier. The PM promotes it to `SMOKE_SCENARIOS`, or to the full-tier-only table with its numbers if it breaks
  the cost rule.
- **What can still reach `main` (the honest list):**
  1. **A timing-rare regression** that passes the batch's attempts (4 per scenario). The C1.1 futex hang was about
     2-in-3; one batch run is evidence, not proof of absence. This is the same limit today's per-PR gate has.
  2. **A hardware-only class** (real silicon, real devices). That is the acceptance bar, not the merge bar,
     unchanged.
- **`main`'s push run is host-only, as it is today (F30).** The batch run is the QEMU evidence for what lands. A red
  on `main` can come only from a landed tree that differs from the gated one, and the pre-landing check prevents that.

## 5. Build slices

| Slice | Status | What | Gate |
|---|---|---|---|
| S0 | **Done** (#32; gated on run 41287591876) | Per-scenario timings that survive log truncation; `--keep-going`; the failure dump shows only failing logs; the end-of-log table. | Host tests; full gate |
| S1 | Next, after the PM's read | xtask:<br>- `SMOKE_SCENARIOS` and the full-tier-only table;<br>- the checked-in timing table, with its host tests;<br>- `--tier quick` with the changed-scenario budget;<br>- `gate-plan`: the core/non-core classification over all of `kernel/src`, the build configuration, the scenario-only exception if the PM says yes, and the fail-safe;<br>- `--keep-going` recording build failures. | Host only (push-CI) |
| S2 | After S1 | Workflow:<br>- quick-tier jobs (`gate-plan` as a step, no `needs`, the smoke budget check);<br>- the batch inputs and `batch-plan` (the quick-green predicate);<br>- assembly with the tree-hash check, `batch-report`;<br>- the folded feature matrix; `main`'s push run folded to 2 jobs;<br>- `scripts/revert-batch.sh` and the local re-assembly script the PM runs before landing. | Defaults reproduce today's gate. A dispatched quick run, a 2-PR batch, and a prefix run, all on the branch. |
| S3 | After S0's data (done) | `scenario_shard_weight` from the measured table; K = 6 default | `shard-coverage`; one full run |
| S4 | With S2 | CLAUDE.md and the runbook, the rule changes in section 6 | Docs only |

## 6. What changes in the standing rules (stated, F24-F28)

Each change is written into CLAUDE.md in S4, **before the first batch lands**.

| Rule (CLAUDE.md) | Today | With batches |
|---|---|---|
| Merge bar: "4-shard gate green on the exact SHA" | Per-PR run on the PR's head | The batch run on the batch tree. The merge report names the base SHA, every head SHA, the batch tree hash and the run URL. The PM's own `git show --stat` stays **per PR**. |
| The close-gate's "green CI URL" in the RETURN | The PR's own `workflow_dispatch` run | The PR's **quick-tier** run URL (or its batch-of-one run, if it is `full-alone`). The batch run URL is the PM's merge evidence. |
| AAG on every kernel-touching merge | Per PR | Still per PR, **plus one AAG over the batch's composed diff** when two or more members touch the kernel. The composition is a new seam, and the merge report names it. The cross-family lens is required for SMP-core changes; those are `full-alone` and never share a batch, so their lens stays per PR, as today. |
| Escaped-defect contract (revert first, one action from green) | `git revert` of one merge | `scripts/revert-batch.sh`: one command, one commit. The owner rule is in 2.5. The ~4-hour fix-forward window applies to the named owner. |
| "At most ~2-3 kernel-touching QEMU gates in flight" | Self-hosted runner capacity | **Superseded for GitHub-hosted runs** by the 20-job account limit and the scheduling rule in section 3. It still applies if `QEMU_RUNNER_LABELS` points the QEMU jobs back at self-hosted runners. |
| Tree identity (docs-only staleness keeps a gate) | Compares `kernel`, `userland`, `tools`, `Cargo.*` | Compares everything except `Plans/`, `docs/` and `*.md` (F28). The same rule serves the landing check. |

**Unchanged:**
- hermeticity;
- per-scenario timeouts: a scenario that fails on time on hosted runners still gets its timeout raised, with the
  elapsed time recorded;
- the infra-red exemption;
- cross-family code reviews;
- `scripts/worker-close-gate.sh`;
- verify-before-merge.

**The new invariant (named, with its guard):** nothing lands on `main` unless a full-tier run passed on a tree that
equals, outside docs, `main` after the landing. The guards are the batch's tree-hash check, the PM's pre-landing
re-assembly, and the post-landing diff.

## 7. Prior art (checked 2026-11-01; read for design, no code taken)

| Project | Last commit | License | What it does | What we take |
|---|---|---|---|---|
| rust-lang/bors | `68c45b6` 2026-10-25 | MIT OR Apache-2.0 | Rust's merge bot: try builds, and "rollups" that test several approved PRs as one merge | The rollup idea: one expensive run over several PRs, plus unrolled builds to find the culprit (our prefix bisect) |
| rust-lang/homu | `8ead3f8` 2025-11-04 | MIT | The previous Rust merge bot. Introduced the rollup. | The same design, in its first form |
| bors-ng/bors-ng | `ca72579` 2024-01-31 | Apache-2.0 | Merge queue for GitHub. Deprecated by its author in favour of GitHub's merge queue. | The queue model; abandoned, so nothing vendored |
| GitHub merge queue | (product) | n/a | `merge_group` event: tests PRs merged together before they land | Considered as the batch mechanism. Not chosen: it needs repo-settings changes only J.c. can make, and its availability on this repo's account type is unverified. Our `batch` input does the same thing in the workflow we already run. |

Commits and licenses were read from shallow clones (`git log -1`, the license files), not from a summary.

## 8. Questions for the PM

1. **Smoke list sign-off.** The 31 in 2.2, plus the two full-tier-only exclusions.
2. **Quick tier on every PR push (`synchronize`).** On hosted runners it costs only free-tier slots. Today PRs run
   once, at open.
3. **The scenario-only exception (2.3.1):** build it, or keep every harness/xtask change full-alone?
4. **K = 6 default shards and the scheduling rule** in section 3: at most 4 quick tiers while a batch runs.

## 9. Review resolution (Dexter, relay 235, on `730b0da0`)

| Finding | Resolution |
|---|---|
| F1 escapes missing | `swapgs_irq_ring3`, `virtio_blk_io_gate_contention`, `lock_route_races` added (2.2). The two over-budget discriminators are named in the full-tier-only table with their numbers. |
| F2 cost rule ambiguous | The rule is now on measured **wall** time, at most 35 s. Every entry shows wall/QEMU. `wait_any_core` is 33.9 s wall and 25.4 s QEMU, which broke the old QEMU rule and meets the new one. |
| F3 changed scenarios uncapped | Section 2.2.1: they share the budget, and slow or unknown multi-boot ones are full-only. |
| F4 ring-3 subset, `kaslr_smoke` | All five ring-3 scenarios in the default suite are listed (`_ss`, `_xm` are ignored). `kaslr_smoke` is justified as the boot floor. |
| F5 write guard | `block_write_guard` added. |
| F6 budget unenforced | The smoke job checks its own `timings.tsv` (2.1). |
| F7 quick-green undefined | The check-run predicate is in 2.4. |
| F8 bisect | Prefix bisect is primary; `base_sha` and `prefix` are inputs (2.4, 2.5). |
| F9 landing / revert | Pre-landing re-assembly, a one-push landing, and `scripts/revert-batch.sh` as the one action (2.4). |
| F10 owner | 2.5. |
| F11, F12 infra / build | The classification in 2.5. |
| F13 bisect cost | 2.5: ⌈log₂ N⌉ one-job rounds. |
| F14 missing paths | `gdt`, `fpu`, `arch/x86_64/mod.rs`, `main.rs`, `wake_trace.rs`, `virtio_blk.rs`, `slab`, `ipc` are core (2.3). |
| F15 not total | Every file under `kernel/src` is classified, with a host test; unknown paths are full-alone (2.3). |
| F16 build configuration | Item 6 of 2.3. |
| F17 scenario-only pattern | Defined exactly, with fixtures (2.3.1). Still the PM's question 3. |
| F18 AAG alignment | 2.3 follows the AAG's definition, and `virtio_blk` and `slab` are in. |
| F19, F20 arithmetic | Section 3: no `plan` job, `main`'s run counted, the scheduling rule. |
| F21 critical path | No `needs` in the quick tier: about 13 min (2.1). |
| F22 batch estimate | Uses the measured largest shard: 51 min + setup (section 3). |
| F23 counts | One list (31), one budget (12 min of scenarios), one target (about 13 min). |
| F24-F28 rule changes | Section 6. |
| F29 | Same as F9. |
| F30 `main` push is host-only | Section 4. |
| F31 S0 status | Marked done (section 5). |
