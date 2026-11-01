<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #31 OT-58 tiered gate on the free tier: design review (Section 235)

**Seat:** Dexter (DeepSeek) · **Reviewed at:** #31 head `730b0da0` = `730b0da0203313062e9feedeb76a98f06b5fd5f1`
**Base:** `origin/main` = `9ae1b02232afb4205db2e0b762c6367755b0d1b3`
**Branch:** `dexter/pr31-review` · **Answers:** `Plans/DISPATCH-2026-11-01-think-reviews-235-237.md` §235
**Scope:** static review of `docs/THINK-2026-11-01-ot58-tiered-gate.md` at the head (one file, +283; no code in the
PR). No CI/QEMU run was made for this review. RAG was not used (the dispatch says it is unavailable); the PR tree and
the `origin/main` tree were read directly.
**Read:** `git fetch origin +pull/31/head:pr31`; `git diff origin/main...pr31`; the kernel/xtask/workflow tree at the
head. **Line references below are to the THINK at the head** unless a repo path is named.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | Quick tier: is every smoke scenario justified, and does it cover the escaped failure classes? | **FAIL** — good core, but it does not meet its own "every escaped-defect scenario" rule, and the cost rule is not applied to changed scenarios. |
| 2 | Batch full tier: is a red batch bisected to one PR mechanically, and is `main` one action from green? | **FAIL** — the bisect primitive exists, but the batch/landing path is not one action, and "quick-green" is not machine-defined. |
| 3 | Concurrency-core path filter: which SMP-core kernel paths miss it? | **FAIL** — several real SMP-core paths are outside the list, and the "classify every file" test is not total. |
| 4 | Free-tier arithmetic (20 concurrent jobs): do four seats' quick gates avoid queueing? | **FAIL** — four seats alone fit; the stated five-seat/batch arithmetic does not, and the 15-minute target assumes no job dependency. |
| 5 | Does anything weaken a standing `CLAUDE.md` rule without saying so? | **FAIL** — the gate change is disclosed, but the AAG, cross-family AAG, batch-revert unit, 2–3-QEMU stagger, and close-gate evidence contract are not. |

**Bottom line.** The two-tier direction is right and the measured-first S0 work is the strongest part of the THINK.
The quick tier is a credible 4-job shape, and the batch tree-hash idea is the right primitive. But this is not ready
to build: the smoke list's own promotion rule is unmet, the batch landing does not yet preserve "`main` one action
from green", the core filter has concrete kernel misses, and the 20-job arithmetic only works if no batch is running.
Items 1–3 should be resolved in the THINK/S1/S2 specs before code; items 4–5 are corrections to the plan's claims
and to the rules the plan says are unchanged.

## What holds (so the PM can separate the findings from the core)

- **Measured before tuning.** S0's per-scenario timings are the right first slice, and the THINK uses them
  (`:47-72`). The 449-scenario/307-minute table is the kind of data the earlier synthesis asked for.
- **`--keep-going` is necessary and correctly motivated** (`:188-195`): `cmd_neo_test` stops at the first final
  failure today (`tools/xtask/src/main.rs:8566-8675`), so a batch would otherwise hide all later failures.
- **The batch tree-hash idea** (`:167-171`) is the right way to make every job test one tree without pushing a
  branch. A batch of one is a clean way to gate a `full-alone` PR (`:182`).
- **The fail-safe direction** ("no merge base / shallow fetch ⇒ full-alone", `:156-159`) is the right rule; the
  implementation gap is that the path map is not total (finding F15).
- **The quick-tier 4-job shape** (plan, host-checks, build-checks, smoke) is a real reduction from the current 22-job
  gate, and the folded feature matrix is a genuine win (`:76-88`).
- **The 24-name core is mostly well chosen**: boot/caps/ring-3/entry/I-14/Lifecycle-SMP is the right spine
  (`:115-124`), and `threads_futex` is correctly kept (`:103`).
- **The kill-switch dispatch** (`:244-251`) and the prior-art table (`:265-274`) are present and usable.

---

## 1. Quick tier: every smoke scenario justified? Escaped classes covered? — **FAIL**

### F1 (blocking) — the list does not satisfy its own "Every escaped-defect scenario" rule

`§2.2` rule 5 (`:105`) says **"Every escaped-defect scenario."** The list at `:117-124` contains `threads_futex`
(the C1.1 lost-wakeup escape) but **not** the two OT-59 escapes:

- `swapgs_irq_ring3` — the deterministic TLB-shootdown/EOI deadlock that was one of the two live defects the B3
  gate surfaced (`tools/xtask/src/main.rs:2457`; `Plans/DISPATCH-2026-07-12-b3-blocked-disposition-two-main-defects.md:15`).
  Its proven pass took **90.5 s** (`Plans/RETURN-2026-07-12-tlb-shootdown-lock-deadlock-jarvis.md:95`).
- `virtio_blk_io_gate_contention` — the single-in-flight submit/wait witness for OT-59 #2
  (`tools/xtask/src/main.rs:5317`); its recorded gate-ON pass is **48 s/800 I/Os** (OT-59 in
  `docs/OPEN-THREADS.md:61`).

Both are in the default suite today, and both are exactly the "escaped when push-CI skipped QEMU" class the rule
exists for. The cost rule at `:107-108` says a slower scenario that belongs on the list **goes to the full tier with
a note** — there is no such note. As written, a quick-green PR can regress the shootdown-IPI/EOI path or the
block-driver submit/wait path and the quick tier will not notice. The promotion loop is only half-installed.

**Concrete case:** a direct edit to `tlb_shootdown.rs` is `full-alone`, so the omission is not exposed there. It is
exposed by a non-core change that alters the same path: `kernel/src/mm/slab.rs` is not in the filter (F14), and a
change to its per-CPU allocation/`init_per_cpu` path can break the reaping/context-switch timing that
`swapgs_irq_ring3` and `xsave_ymm_context_switch` exercise. The quick smoke list contains neither scenario, so the
first evidence is the batch full tier. That is the feedback loop the THINK says it is restoring (`:225-228`).

**Fix:** either add both names to `SMOKE_SCENARIOS` (if their measured QEMU time can be brought under 20 s), or list
them explicitly in a "full-tier-only, excluded by measured cost" table with the measured numbers, and say why the
promotion rule's "with a note" is satisfied. Do not leave the rule asserting "every" while the list omits two known
escapes.

### F2 (blocking) — the 24-list cost measurement is internally ambiguous

`§1.1:58` says the 24-scenario list is **352 s in total; slowest `wait_any_core` at 34 s. Every candidate meets the
20 s QEMU cost rule.** Two readings:

- If 34 s is the QEMU time, `wait_any_core` violates the 20 s rule and the list is not legal by its own test.
- If 34 s is total (build + image + QEMU), the table must say so and cite the `QEMU s` column. S0 records exactly
  those three columns (`:234`), so the data exists; the summary does not expose it.

Either way a reviewer cannot verify the cost rule for the 24. **Fix:** print the S0 `QEMU s` for every smoke
candidate in the THINK (or in a checked-in `docs/` table) and state the max QEMU time; the 352 s total is not a
substitute for the per-scenario rule.

### F3 (blocking) — "changed scenarios run too" has no cost cap

`§2.1:83` and `§2.2:131-133` run **every scenario the PR adds or changes** on the quick tier. The 20 s rule is
applied only to the fixed `SMOKE_SCENARIOS` list. A PR that adds a `neofs_large_write`-class scenario (300 s
timeout, 156 s measured, `tools/xtask/src/main.rs:1711`) makes the "≤ 15 min" smoke job unbounded, and a PR that
edits a slow scenario can do the same. The quick tier then becomes the queue problem it is meant to fix.

**Concrete case:** a PR that adds one multi-boot scenario (the `login_two_boot`/`login_cut_steps` family; the
latter boots 48 times) passes `gate-plan` as "scenario-only" and runs it in the smoke job; the job's 15-minute
target is gone and the seat's next push is delayed. **Fix:** give `TestScenario` a measured-cost/`quick_ok` field (or a reviewed
`SMOKE_SCENARIOS`-adjacent `QUICK_CHANGED` rule); unknown or >20 s scenarios run full-tier-only, and the quick tier
still runs the cheap changed scenarios. The changed-scenario idea is good; it needs the same budget gate as the
fixed list.

### F4 (note) — the ring-3 "set" is a subset, and `kaslr_smoke` is unjustified

- `§2.2:96` says "the ring-3 fault set"; `:120` lists `ring3_fault_de`, `_ud`, `_gp`. The harness also has
  `_ss`, `_db`, `_xm`, `_mf` (`kernel/src/test_harness.rs:175-181`). If the three are chosen because the others are
  slow or less likely, say so; otherwise the "set" is 3/7.
- `kaslr_smoke` (`:117`) is not in any of the four stated groups (boot+IPC, caps, ring-3 faults, entry paths, I-14,
  lifecycle/SMP). `kaslr_heap_contract` is separately in the entry group (`:121`). Give `kaslr_smoke` a reason (boot
  floor / KASLR entry) or move it to the full tier.

These are notes, not blockers, but they are the kind of "why is this here?" the PM's sign-off question
(`:277-279`) asks.

### F5 (note) — no write-guard discriminator in the quick tier

`block_write_guard` / `block_write_guard_4k` (`tools/xtask/src/main.rs:4697`, `:4707`;
`kernel/src/test_harness.rs:20601`) are the `GuardedDevice` "never write the internal disk" discriminators. They are
not in the 24, and `kernel/src/drivers/block/mod.rs` (where the guard lives) is not `full-alone`. The batch full
tier runs them, so this is not a `main`-red class, but the quick tier's stated premise "entry-path invariants that
every kernel change could break" (`:98`) should include the disk-safety invariant or say why it does not. At
minimum, name the omission so the PM's smoke-list sign-off is informed.

### F6 (note) — the 12-minute smoke budget is not enforced

`§2.2:110` says the wall-clock "stays ≤ 12 minutes, checked by the S0 timing record." S0 is a one-time artifact;
a later scenario slowdown is not detected by any test. The smoke job should parse its own `timings.tsv` (S0 already
writes it, `:234`) and fail if the smoke wall-clock exceeds 12 minutes, or a host test should check a checked-in
timing table. Otherwise the budget is a hope, and the free-tier arithmetic silently degrades as scenarios grow.

### What a quick-green PR can still carry to `main` (the direct answer)

Because the batch full tier is the merge gate, a quick-green PR should not reach `main` on quick evidence alone.
The classes that can still reach `main` are those the **batch full tier also misses or passes by luck**:

1. A **timing-rare** regression in a scenario that is not in the quick smoke list and passes the batch's 4 attempts
   (`TEST_SCENARIO_ATTEMPTS_DEFAULT = 4`, `tools/xtask/src/main.rs:1852`). The C1.1 futex hang is the precedent:
   it was ~2-in-3; one batch run is not proof of absence.
2. A regression in a **build-environment file** the smoke list does not exercise and the tree-identity set omits
   (`Dockerfile`, `rust-toolchain.toml`, `kernel/Cargo.toml`, `tests/`, `scripts/`) — see F28.
3. A regression in a **changed scenario whose cost blew the smoke budget** (F3), if the job is re-run/waived rather
   than treated as a red.
4. A **hardware-only / bare-metal** class (the Zenbook write guard on real silicon, real USB/PCI devices) that no
   QEMU gate can prove — this is the acceptance bar, not a merge-bar escape, but it should be named in the THINK's
   completeness section.

---

## 2. Batch full tier: mechanical bisect and `main` one action from green — **FAIL**

### F7 (blocking) — "quick-green" is not machine-defined

`§2.4:165` says "If any PR is not quick-green on that exact head ... the job fails." A batch `plan` job must query
the check-runs/check-suites for the **exact** head SHA and require the named quick-tier jobs (`plan`,
`host-checks`, `build-checks`, `smoke`) to be `success`. The THINK does not name the check names, the API, or what
counts as skipped/neutral. **Concrete failures:** (a) a PR with an old green *full-gate* run but no quick-tier run
is admitted; (b) a quick-tier run whose `smoke` job was skipped by `paths-ignore` counts as green; (c) a PR whose
quick run is `cancelled` after a force-push is neither green nor red. Define the predicate, and make the `plan` job
print the SHAs/check-run IDs it accepted.

### F8 (blocking) — the single-head bisect does not reconstruct the batch tree

`§4:217-219` runs the failing scenarios "on each member's head, plus the batch base." A PR head is based on an
older `main` and **lacks the other batch members**; a failure caused by `main` or by an earlier member will not
reproduce on the member's head. The THINK's fallback — growing prefixes (`:220-221`) — is the only reliable test;
make it primary, not a fallback.

**Concrete case:** PR A changes the scheduler; PR B is an unrelated userland PR. The batch reds in a scenario that
fails only when A's change and B's timing combine. B's head alone passes (A's may too), so the THINK's first step
names neither; the prefix run is what finds A+B, and re-forming without A (or B) is the action. The "PR whose head
reproduces" is a heuristic, not a mechanical bisect.

**Also:** `workflow_dispatch` `ref` is a branch/tag. The "batch base" is a SHA that may have no ref; the workflow
needs a `base_sha`/`batch_prefix` input (or a temporary ref) to run the prefix legs. The THINK does not say how the
base-alone leg is dispatched. `plan` should pass the resolved base SHA as an output, not only print it (`:165`).

### F9 (blocking) — the post-landing check is too late, and a batch revert is not one action

`§2.4:176-181` lands the PRs **first**, then checks the tree. If the check fails, `main` is already red. The
escaped-defect contract's promise is "`main` stays one action from green" (`CLAUDE.md:472-491`); a batch of N merges
is N merges to revert (reverse order, conflicts possible), not one action. The THINK says the contract is
"unchanged" (`:222-224`) without defining a batch revert.

**Fix (two parts):**
1. **Pre-landing:** the PM reproduces the batch merge locally (`main` at the fixed base + `git merge --no-ff` each
   resolved head in order) and asserts the resulting tree hash equals `batch-report`'s before pushing. That turns
   the post-landing check into a gate, not a post-mortem.
2. **Batch revert unit:** define it. Either land a batch as one recorded merge/tree (then a single `git revert -m 1`
   is the one action) or document the reverse-order N-step revert with a named owner. The current "unchanged"
   claim is false for a batch.

### F10 (blocking) — no owner for a batch-interaction failure

The 4-hour fix-forward hatch says "the owning coder" (`CLAUDE.md:481-484`). A failure caused by two PRs
interacting has no single owner; the PM must assign one at bounce time, or the batch is reverted (F9). The THINK
should state: "the PM names the owner from the first failing prefix; if no single owner, revert the batch." Without
this, the contract's clock cannot start.

### F11 (note) — infra vs code classification is missing from the batch report

`§4:216` lists the failing scenarios; a runner OOM/disk/timeout is an **infra red** per `CLAUDE.md:485-487`, not a
PR to bisect. The `neo-test` error text includes the reason (`run_test_scenario_with_retries` returns the error
string, `tools/xtask/src/main.rs:8846-8890`); the `batch-report` should classify `timeout`/`runner loss`/
`disk full` as infra and re-fire, and only scenario assertion/build failures as bisect candidates. Otherwise the
first hosted-runner hiccup bounces a random PR.

### F12 (note) — build failures are not in the batch report

`--keep-going` handles scenario failures; a kernel/scenario **build** failure still exits the job via `?`
(`tools/xtask/src/main.rs:8674-8676`). The report should name the PR whose head fails to build (the same way a
conflict names a PR, `:169`), otherwise the PM has a red batch with no scenario list and no PR attribution.

### F13 (note) — the bisect is O(N) dispatches, not "one job"

`§4:217` says "in one job, on that PR alone"; the next clause says it runs on each member's head plus the base —
that is N+1 jobs, plus N prefix jobs in the interaction case. Each has checkout/setup/build. "Cheaply" should be
quantified, and the legs should reuse the batch's build cache/artifact (or run only the failing scenarios with a
warm target dir) to keep the cost below the batch itself.

---

## 3. Concurrency-core path filter: kernel paths that miss it — **FAIL**

`§2.3:135-159` is the key fail-safe. The list is a good start, but it is a **core allowlist**, not a total map, and
several real SMP-core paths are outside it.

### F14 (blocking) — concrete missing paths

| Path | Why it is SMP/concurrency core | Evidence |
|---|---|---|
| `kernel/src/arch/x86_64/gdt.rs` | Per-CPU GDT/TSS; `init_ap` loads the AP's TSS, `set_kernel_rsp` writes the current CPU's TSS. AP bring-up and context switch. | `:116` `init_ap`, `:211-215` `set_kernel_rsp` |
| `kernel/src/arch/x86_64/fpu.rs` | FPU/XSAVE save/restore is called from the context-switch shim; `xsave_ymm_context_switch` is a context-switch scenario. The filter lists `context.rs` but not the FPU half. | `:45` `fxsave`, `:60` `xsave`, `:87`/`:96` save/restore; `context.rs:122-124` calls them |
| `kernel/src/arch/x86_64/mod.rs` | `init_traps_ap` is the AP's trap/GDT/APIC/syscall setup; it is the AP bring-up sequence. | `:413-420` `init_traps_ap` → `gdt`, `per_cpu`, `apic`, `syscall` |
| `kernel/src/main.rs` | `smp::bring_up_aps()` is the AP start; `online_cpu_count` feeds `par_matmul::set_thread_count`. | `:904` `bring_up_aps`, `:965` `set_thread_count` |
| `kernel/src/sched/wake_trace.rs` | Per-CPU wake-delivery tracer; it is the `threads_futex` discriminator and changes what the scenario reports. The sched list (`:140`) omits it. | `:62` `per_cpu`, `:68` `CpuCounters`, `:181` `note_ipi_sent` |
| `kernel/src/drivers/block/virtio_blk.rs` | The OT-59 #2 single-in-flight `io_gate` and its owner-park protocol; a driver-owned `SwitchMutex`, not under `kernel/src/sync/**`. | `:398`/`:654` `io_gate`, `:634-651` invariant comment |
| `kernel/src/mm/slab.rs` | Per-CPU slab state (`init_per_cpu`, `per_cpu::slab_state`), per-CPU accounting; a cross-CPU allocation change is core-adjacent. | `:567`, `:651` `per_cpu::slab_state`; `smp.rs:275` `init_per_cpu` |
| `kernel/src/ipc/mod.rs` | Per-channel `IrqSafeMutex` and `sched::yield_now`; the IPC wait/wake path is cross-CPU. | `:119` `IrqSafeMutex`, `:157` `yield_now` |

At minimum, add `gdt.rs`, `fpu.rs`, `arch/x86_64/mod.rs`, `main.rs`, `sched/wake_trace.rs`, and `virtio_blk.rs` to
the core list; audit `slab.rs` and `ipc/mod.rs` against the AAG definition (`CLAUDE.md:593-605`: "lock-ordering"
and "per-CPU accounting" are explicitly in scope).

### F15 (blocking) — the fail-safe is not total

`§2.3:156-159` says "a path `gate-plan` can't classify ... gives full-alone," and the host test asserts every file
under `kernel/src/sched`, `kernel/src/sync` and `kernel/src/arch/x86_64` is on the core or non-core list. But the
design is an allowlist: **anything not on the core list is quick by default**. The test does not cover
`kernel/src/mm/`, `kernel/src/ipc/`, `kernel/src/drivers/block/`, `kernel/src/fs/`, `kernel/src/net/`, or the
top-level `kernel/src/*.rs`. A new `kernel/src/mm/foo.rs` that implements a lock is quick with no classification.
That is the opposite of the synthesis's C4 ("a changed path matching no area/scenario mapping promotes to the full
suite"). **Fix:** maintain an explicit `NON_CORE` allowlist for **every** `kernel/**` file (the host test then
forces a new file to be classified), or default any unknown `kernel/**` path to `full-alone`. The current test's
scope is too narrow for the claim it is making.

### F16 (blocking) — build configuration is not in the filter

`kernel/Cargo.toml` (`[features]` `:76`, `default` `:77`, `lapic-timer` `:223`, `xsave-fpu` `:325`), `Cargo.toml`,
`Cargo.lock`, `rust-toolchain.toml`, `Dockerfile`, and `docker-compose.yml` can change what is built or how it is
tested, but none is `full-alone` in §2.3. A feature-default flip that disables `lapic-timer` or `xsave-fpu` would
be a quick-tier change. **Fix:** add these to the build-alone list (the same set the close-gate's Docker path
uses). The tree-identity set (`:178-179`) already includes `Cargo.*`; the tier filter should too.

### F17 (blocking) — the scenario-only exception cannot match a real new scenario

`§2.3:147-150` says a quick-tier scenario-only diff is "a new `TestScenario { … }` block, its `cfg` arm and its
`SCENARIO` const, and nothing else in either file." A real new scenario also needs:

- a `run_<name>()` body in `kernel/src/test_harness.rs` (e.g. `run_block_write_guard` at `:20601`, dispatched at
  `:4736`),
- a Cargo feature in `kernel/Cargo.toml` (`:409+`),
- often a `user_feature` in `userland/`.

With "nothing else," every new scenario is `full-alone`; with the body included, a scenario-only PR can change
arbitrary test code and still be quick. **Fix:** define the exact file/line allowlist (including the `run_*` body
and the Cargo feature) and ship a fixture diff in S1 that matches it; the current wording is either impossible or
too broad.

### F18 (note) — the filter's "same trigger as the cross-family-AAG" is incomplete

`§2.3:137-138` says the filter is the same trigger as the pitfall-bank and cross-family AAG. The AAG's scope
(`CLAUDE.md:593-605`) explicitly includes "lock-ordering" and "per-CPU accounting"; `virtio_blk.rs`'s `io_gate` and
`slab.rs`'s per-CPU state are in those classes but not in the list. Either align the list to the AAG definition or
state that the filter is deliberately narrower (and why that is safe).

---

## 4. Free-tier arithmetic (20 concurrent jobs): do four seats' quick gates avoid queueing? — **FAIL**

The narrow answer is **four seats alone, yes** (4 × 3 persistent jobs = 12, plus 4 one-minute `plan` jobs = 16
peak ≤ 20). The plan's own claims go further, and those do not hold.

### F19 (blocking) — the five-seat/batch arithmetic is not a queueing guarantee

`§2.1:85-86`: 5 seats × 4 = 20 at peak; `plan` finishes in <1 min, leaving **15 persistent jobs** and 5 free
slots. A batch is `K+4` jobs (`:174-175`): 10 at K=6, 9 persistent after `plan`. So:

| Scenario | Persistent quick jobs | Persistent batch jobs | Total | Over 20 |
|---|---:|---:|---:|---:|
| 4 seats, no batch | 12 | 0 | 12 | 0 |
| 5 seats, no batch | 15 | 0 | 15 | 0 |
| 4 seats + batch K=6 | 12 | 9 | 21 | **1** |
| 5 seats + batch K=6 | 15 | 9 | 24 | **4** |

The batch's 10 jobs include its own `plan` and `batch-report`; the table is the steady state. The claim "leaves
about 5 slots for a batch's full tier" is true (5 free), but the batch needs 10, so half of it queues. If the batch
is dispatched first, the overage lands on the quick tiers instead. The plan does not define priority. **Fix:** state
a scheduling rule — batch lowest priority (dispatch after the quick gates), or cap quick tiers at 3 while a batch is
in flight, or fold `plan` into `host-checks` to drop one job/seat (4 seats become 12 persistent; 12 + 9 batch = 21,
still 1 over; K=5 makes the batch 8 persistent and fits exactly at 20). A 3-seat cap while a batch runs is the
simplest; folding `plan` alone does not fit 4 seats + K=6.

### F20 (blocking) — the 20-job limit is account-wide, and the plan counts only its own jobs

The free-tier limit is across the account, not per workflow. The `push: main` run (`.github/workflows/ci.yml:34-35`)
still runs the full host set and the 13-arm feature matrix (`.github/workflows/ci.yml:847-949`), about 18 jobs; a
batch landing plus the next round of quick tiers is then far over the 20-slot ceiling. The THINK's table counts
only the quick tier and the batch. Add the post-merge `main` run and a margin, or fold the `main` push's feature
matrix into `build-checks` too. As written, a batch landing can starve every seat's next quick gate.

### F21 (blocking) — the 15-minute quick tier assumes the smoke job starts at t=0

Today `neo-test` `needs: [nightly-skip, host-checks, product-build]` (`.github/workflows/ci.yml:108`), and QEMU
starts at +7 min (`THINK:27`). The THINK's quick-tier table (`:78-84`) does not list `needs`. If `smoke` depends on
`build-checks` (8–12 min, `:82`) or `host-checks` (7 min), the quick-tier wall-clock is **23–27 minutes**, not 15;
five seats hold 10 host/build jobs for 8–12 min, then 5 smoke jobs for 15 min, and the queueing problem is worse. **Fix:** make `smoke` independent
(it builds the scenario image itself, with its own target dir) or fold `build-checks` into `smoke`; show the
critical path in the table. If `smoke` must wait for the product build, say so and change the target to ~25 min.

### F22 (blocking) — the 45-minute batch estimate contradicts the measured 307/51

`§1.1:55` measures 307 shard-minutes (largest shard 89). The K=6 table (`:65`) says 51 minutes. `§3:208-209` then
says "a balanced 6-way split of today's ~255 shard-minutes is about 45 minutes." 255 is not in the S0 table, and
the wall-clock of a shard matrix is the **largest shard**, not the sum divided by K. Use the measured 307 →
51-minute largest shard at K=6 (or 38 at K=8), plus setup. The "today one gate costs 104 minutes" (`:210`) is the
pre-S0 4-shard wall; it is not comparable to the 307-minute sum. State which number is wall and which is sum.

### F23 (note) — the 12-minute smoke budget vs the 24-list total

The 24-list is 352 s (5.9 min) scenario time, plus ~5 min setup = ~11 min; the 15-minute target has ~4 min of
headroom, which is fine. But `§3:204` says "about 30 scenarios at 15-25 s each adds 8-12 minutes" — a different
list size from the 24. Reconcile the 24-list, the 30-scenario estimate, and the 12-minute/15-minute budgets. The
same applies to the "Five seats fit at once" line (`:205`): it is true only with no batch (F19).

### F31 (note) — the S0 status is stale in §5

`§1.1` says S0 (#32) ran and produced the 449-scenario table (`:49-58`), but the build-slices table still lists S0
as "Timings that survive log truncation ... Gate: Host tests; one full-tier run to collect the data" (`:234`) — a
future slice. Mark S0 done (with the run ID), S1/S2 next, so the PM does not re-dispatch work already banked.

---

## 5. Anything that weakens a standing `CLAUDE.md` rule without saying so? — **FAIL**

The THINK **does** disclose the gate change and names a new invariant (`:252-257`); that part is good. The problem
is the list of things it calls "Unchanged" (`:258-263`) when the merge unit has changed.

### F24 (blocking) — the AAG is not "unchanged" for a batch

`CLAUDE.md:570-605` requires an adversarial-audit panel before merging **any** kernel-touching branch, and a
**cross-family lens** for the SMP/concurrency core. The batch merges N branches as one tree; per-PR AAGs do not
audit the composition. The THINK lists "the AAG" under "Unchanged" (`:262`) without saying how it runs on the
batch. **Concrete case:** two individually AAG-clean kernel PRs compose into a lock-order or wake-path interaction
that the QEMU gate may miss (the device-less-QEMU completeness blind spot the AAG exists for). **Fix:** require a
batch AAG on the composed diff before landing, with the cross-family lens if any member is SMP-core; the PM's merge
report names the batch AAG and the tree hash. This is the standing rule's own trigger, applied to the new merge
unit.

### F25 (blocking) — the 2–3 kernel-touching QEMU gates in flight rule is silently relaxed

`CLAUDE.md:31` says "keep at most ~2–3 kernel-touching QEMU gates in flight at once" because the shared runner
capacity is the binding constraint. The quick tier gives **every PR** a QEMU smoke job; five seats = five concurrent
QEMU jobs. The THINK moves the gate to GitHub-hosted (`:75-84`, S2 `:236`), so the *reason* changes, but the rule is
written as standing and is not mentioned. **Fix:** say explicitly that the hosted quick-tier smoke is exempt from
the self-hosted 2–3 stagger (the new bound is the 20-job free limit), or keep a cap. The current silence is a rule
change hidden inside a throughput table.

### F26 (blocking) — the close-gate evidence contract changes silently

`CLAUDE.md:526-530`: a RETURN must paste the green **`workflow_dispatch` QEMU run URL** for a kernel-touching
build. In the batch model the full-tier run is a batch run on a constructed tree, not the PR's own branch; the
worker cannot produce that URL before the batch. S4 says it will update `CLAUDE.md` (`:238`), but the first batch
would land before the docs catch up, and every kernel PR's RETURN is non-compliant by design. **Fix:** state the new
contract now — the PR RETURN cites the quick-tier run URL; the batch run URL + tree hash is the PM's merge evidence;
the PM's merge report carries it. The "green without a CI URL" rule survives in a different form; say so.

### F27 (blocking) — the merge bar's "exact SHA" becomes a tree hash without the base/head list

`CLAUDE.md:544-552` says the merge bar is "4-shard gate green on the exact SHA". The THINK replaces this with the
batch tree (`:252-257`), which is a reasonable change, but the PM's verify-before-merge still needs an exact
artifact. **Fix:** the merge report names the batch base SHA, each PR head SHA, and the batch tree hash; the PM's
`git show --stat` stays per PR. That is the exact-SHA discipline translated, not dropped.

### F28 (blocking) — the tree-identity set is incomplete

`§2.4:178-179` checks `kernel`, `userland`, `tools`, `Cargo.*`, `.github`. The existing tree-identity note
(`CLAUDE.md:369-386`) uses the same code-subtree idea for a different purpose; the batch set should be a superset of
the build inputs, not a subset. It omits `Dockerfile`,
`docker-compose.yml`, `rust-toolchain.toml`, `tests/`, `scripts/`, and `fixtures/`/`boot/` where they affect the
gate. A docs-only commit will not touch these, but a code commit can change the gate environment; the new
invariant's "code subtrees equal `main`'s after the landing" is then false. **Concrete case:** PR A changes the
`Dockerfile`; the batch gate runs with it; before landing, `main` gains a different Dockerfile change; the
tree-identity check sees the five listed subtrees equal and misses the environment difference; the next gate runs
in a different container. **Fix:** use the close-gate's build-input set (Dockerfile, compose, toolchain, tests,
scripts, fixtures) or state why each is out.

### F29 (note) — the escaped-defect contract's "revert first" is the batch-revert problem again

`§4:222-224` says the escaped-defect contract is "unchanged"; F9 shows a batch revert is N actions. This is the
same finding as F9 from the standing-rule side: the contract's "one action from green" is the load-bearing promise,
and batching changes the unit. Resolve F9 and this resolves F29.

### F30 (note) — the `main` push CI does not run QEMU

`.github/workflows/ci.yml:132` restricts `neo-test` to `workflow_dispatch`/`schedule`; a `push` to `main` runs the
host/build checks only (no QEMU). The batch gate is the only QEMU evidence, which is fine under the new invariant,
but the THINK
should say it explicitly: a post-merge red can only come from a tree mismatch, and the tree-identity check is the
guard. The current text implies `main` is re-gated (`:222` "Only batch-gated trees land") without noting that the
post-merge run is host-only.

---

## Required changes before S1/S2 (the short list)

1. **Smoke list:** add/named-exclude `swapgs_irq_ring3` and `virtio_blk_io_gate_contention` with measured costs
   (F1); give the 24-list per-scenario QEMU times (F2); cap changed-scenario cost (F3); justify `kaslr_smoke` and
   the 3/7 ring-3 subset (F4); decide the write guard (F5); enforce the 12-minute budget (F6).
2. **Batch:** machine-define quick-green (F7); make prefix bisect primary and pass the base SHA (F8); pre-landing
   tree-hash check and a one-action batch revert (F9); name the interaction owner (F10); classify infra (F11) and
   build failures (F12).
3. **Filter:** add the F14 paths; make the map total (F15); add build config (F16); fix the scenario-only pattern
   (F17); align with the AAG definition (F18).
4. **Arithmetic:** use the measured 307/51 (F22); show the smoke `needs`/critical path (F21); count the `main` push
   run (F20); define priority or drop a job (F19).
5. **Standing rules:** batch AAG + cross-family lens (F24); say the 2–3 QEMU stagger is superseded for hosted quick
   tiers (F25); state the new evidence/merge-report contract (F26/F27); complete the tree-identity set (F28);
   define batch revert (F9/F29); state that `main` push is host-only (F30).

The two-tier design is worth building. The above is what I would fix before the first S1 host test is written, not
after the first red batch.
