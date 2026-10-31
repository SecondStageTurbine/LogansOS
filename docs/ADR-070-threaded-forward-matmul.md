<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-070: Threaded forward matmul via deterministic column-partition fork-join

**Date:** 2026-07-04
**Status:** Accepted (build dispatch
`Plans/DISPATCH-2026-07-04-ot34-threaded-matmul-oracle.md`; OT-34)
**Builds on:** F4 native quant kernels (`546ef3d`), the OT-30 llama.cpp top-1
parity oracle, the OT-7 latency oracle (`305b04c`/ADR-052), the SMP AP-ready
barrier (`86f2058`), the complexity-governance ledger (I-1, I-2, I-5, I-14),
Prometheus's deterministic-parallel-matmul prior-art mine
(`RESEARCH-2026-07-04-deterministic-parallel-matmul-prior-art-FINDINGS.md`,
`eae1de8`).

## Context

The forward pass is single-threaded: every matmul runs on the BSP, so the
OptiPlex's other cores sit idle during inference (1.5B is ~1.86 s/token
single-thread). Threading the matmul across cores is a modeled ~4–6× speedup —
a direct usability win for a real-time assistant. The banked lever (OT-34)
unblocked once F4 landed.

The hard constraint is **determinism**: the result must be **byte-identical
regardless of thread count**, or it breaks the OT-30 parity oracle. A naive
parallel float reduction changes accumulation order (float add is
non-associative) → different bytes → the exact near-tie fragility seen at 1.5B.
So a nondeterministic reduction that is merely "close enough" is a FAIL.

## Decision

Thread the forward matmul by **partitioning the OUTPUT COLUMNS across CPUs**,
never the contraction (reduction) axis.

### Why columns are the safe axis (the determinism argument)

The forward matmul is `out[col] = vec_dot(rhs_col, lhs_row)` with a **single LHS
row** (single-position decode). Each output column is an **independent dot
product** — there is no cross-column reduction. So splitting the output columns
across cores keeps every dot product **whole inside one core**: column `j`'s
bytes depend only on `lhs` and `rhs`'s column `j`, computed by the identical F4
kernel, and are independent of which core ran it and of how many partitions
there are. The assembled output is therefore byte-identical for every thread
count. **This is I-1 (result invariance / the byte-oracle) applied to
parallelism.**

Splitting the *contraction* axis instead would force each core to compute a
partial dot product and then sum the partials across cores — a cross-thread
float reduction whose order changes with thread count → non-deterministic bytes.
That axis is **never** split. (Prometheus's determinism trap-mine is expected to
confirm this: partition the output axis, never the reduction.)

### The kernel math is unchanged (scope guard)

A partition slices `rhs`/`out` to its column range (contiguous for a single
output row) and calls the **existing F4 kernels verbatim** — no op-math change,
no `.neom`/vocab change, no model-graph change. The single-threaded / feature-off
path is the whole-column range `[0, cols)`, which is bit-for-bit the pre-OT-34
kernel call.

### Cross-core execution: per-CPU work-slot fork-join

`kernel/src/ai/par_matmul.rs` is the executor (gated `threaded-matmul`, default
off). It is deliberately the *obviously-correct* design, not the fastest:

- **Per-CPU work slots, not a shared descriptor.** The BSP fills AP `t`'s slot
  with that partition's work, publishes `pending` (Release), executes partition 0
  itself, then barriers. Each slot is written by the BSP only when the previous
  job on it is fully drained, and read by exactly one executor — so there is no
  cross-CPU descriptor-reuse race.
- **Single-winner claim + fail-STOP join (F1).** A partition is executed by
  whoever wins `CAS(pending: true → false)` — either the assigned AP (from
  `ap_poll`, hooked into the AP idle loop) or the BSP steal-back. Exactly one
  writer per disjoint output range; forward progress is guaranteed even if an AP
  never polls (the BSP steals every un-claimed partition, degrading to
  single-threaded — still byte-identical). The join loop's backstop is
  **fail-STOP, never fail-continue**: the hard limit (`BARRIER_LIMIT`) may only
  skip/steal an *unclaimed* slot (`pending == true`); a slot an AP has *claimed*
  (`pending == false`) that is not yet `done` past the limit is an executor wedged
  mid pure-compute, so the BSP **halts** rather than returning. Returning would end
  `run_generic`, drop the `&mut out` borrow, and let the still-live executor write
  freed memory (UAF). The decision is a pure function (`par_matmul_logic::
  barrier_action`, host-KAT'd). This was the AAG's one finding — the executor is
  **interruptible** (the AP idle loop runs interrupts-enabled), so the earlier
  "impossible for lock-free pure compute" justification was unsound.
- **Jobs are BSP-serialized** by a non-blocking `PAR_BUSY` try-guard: a second
  concurrent caller falls back to single-threaded rather than clobber a live
  job's slots.
- **Runtime `thread_count`, default 1.** Even with the feature enabled, the
  matmul is single-threaded until a caller raises `thread_count` (clamped to the
  online CPU count) — the conservative kill-switch default. `should_parallelize`
  also gates on a column-width threshold (`PAR_MIN_COLS = 512`), so the tiny K/V
  projections stay serial and only the FFN/lm_head matmuls fork.

### Memory safety

The scoped-barrier invariant carries soundness: `run_generic` never returns
(freeing the `lhs`/`rhs`/`out` borrows) while a claimed partition is unfinished —
it drains every claimed slot to `done` or fail-stops (halts). This holds on
**every** path, including the hard-limit backstop (F1), so the borrowed
`lhs`/`rhs`/`out` outlive all executor access. Draining every claimed slot before
return also closes the **cascade**: no abandoned-but-live executor can survive
into a later job to trip its barrier with a stale `done` store (so no per-job slot
epoch is needed). Each executor reconstructs a **disjoint** `&mut [f32]` for its
columns only (no overlapping `&mut`, no aliasing UB); `lhs`/`rhs` are shared
immutable reads. The `unsafe` is confined to `par_matmul.rs`'s
pointer-reconstruction, each site carrying the disjointness + lifetime contract.

## Consequences

- **Invariant budget (Rule 2): no new invariant.** OT-34 preserves the ledger.
  I-1 is the load-bearing row (byte-identity under parallelism). The join path
  takes **no AI-governance lock** (I-14: not INFLIGHT/RECENCY/ARENA/reservation/
  TABLE) and adds **no ordered lock** (I-5) — `PAR_BUSY` is an `AtomicBool`
  try-flag, not a lock primitive, so the production lock census is unchanged
  (70 statics).
- **Kill-switch (Rule 1):** feature off ⇒ `par_matmul` and the `ap_poll` hook are
  removed and the forward is bit-for-bit today; `thread_count == 1` (the default
  even when enabled) ⇒ single-threaded. Proven both structurally and by the host
  KAT's parts=1 == literal-`qk_matmul`-call equality.
- **Speedup is modeled, not yet measured on silicon.** QEMU-TCG serializes
  vCPUs, so the gate cannot show wall-clock overlap; the OT-7 latency measurement
  reports the per-core work ratio (T1 / one-partition ≈ N) as the modeled speedup
  basis, explicitly flagged virtualized. The absolute silicon number is a
  bare-metal OT-7 read (follow-on).

## Verification (ISC evidence)

- **ISC-2 determinism (host KAT, default `cargo test`):**
  `ot34_threaded_matmul_column_partition_is_byte_identical` — for all four
  quantized families (Q4_K/Q6_K/Q5_0/Q8_0), the column-partitioned output is
  byte-identical at partition counts 1/2/3/4/5/7/8/cols (uneven splits included)
  AND equals the literal pre-OT-34 `qk_matmul` call at parts=1.
- **ISC-2 cross-core (QEMU, `neo-test-threaded-matmul-parity`, NEO_SMP=4):** a
  real single-row quantized matmul on real APs at `thread_count` 1/2/4 is
  byte-identical at every count. The **byte-identity is the hermetic PASS gate**
  (deterministic regardless of which core computed a partition); cross-core
  participation is reported, not gated (a green gate must mean the code is
  correct, never that the scheduler cooperated).
- **ISC-3 latency (OT-7):** rdtsc median relative ratio, TCG-virtualized.
- **ISC-4 kill-switch:** the parts=1 == literal-kernel equality above, plus the
  feature-off structural argument.
- **AAG:** centered on ISC-2 (byte-identity + parity invariance) — the
  correctness ballgame.

## Prior-art reconciliation (Prometheus determinism mine)

Prometheus's mine (GGML/llama.cpp, OpenBLAS, PyTorch) is the prior-art this build
was built to. The reconciliation, point by point:

- **Core recipe — MATCHED.** The mine's recipe is "partition the OUTPUT axis so
  each output element is computed whole by one thread; never split the K
  (contraction) axis (a cross-thread float reduction is non-associative → bytes
  change with thread count → poisons OT-30)." OT-34 partitions the output axis and
  never touches K. GGML does exactly this.
- **Row vs column — the regime inverts the mine's default.** The mine prescribes
  **row** partition because it assumes `rows = batch × seqlen` is large. Our
  autoregressive decode forward is **`rows = 1` always** (one position at a time),
  so row-partition yields zero parallelism. The mine's own §1.4 caveat and §3.5
  cover exactly this: *"If `rows < N` … partition by columns instead — same
  principle, disjoint output elements."* So column-partition is not a deviation —
  it is the mine's **prescribed path for our regime**, and the *only* axis that
  parallelizes a single-row decode. (Multi-row prefill, if ever added, would take
  the row axis; today rows>1 stays single-threaded — a named follow-on.)
- **Work-stealing — added deliberately, rated LOW-risk by the mine.** The mine
  recommends static partition for a first cut and adding work-stealing "later only
  if load imbalance is measured." OT-34 adds BSP steal-back up front — not for
  load-balancing but for **fault-tolerance**: an absent/slow AP can never wedge the
  forward (the BSP steals its partition). The mine rates work-stealing **LOW-risk
  with an output-axis partition** (Trap 6) because the numerical result is
  independent of which core runs a chunk — exactly GGML's `atomic current_chunk`
  argument. Our steal-back recomputes the *same columns* via the *same kernel*, so
  bytes are identical; the QEMU gate's byte-identity holds whether a partition ran
  on its AP or was stolen (participation varies run to run; bytes do not).
- **The 6-trap bank — covered.** (1) Float reassociation [CRITICAL]: no K-split ⇒
  no cross-thread reduction. (2) Thread-count-dependent tiling [HIGH]: fixed
  `column_partition` rule + the frozen F4 kernels tile per-dot identically at any
  thread count — the KAT asserts byte-identity at parts 1/2/3/4/5/7/8/cols. (3)
  SIMD horizontal-sum order [MEDIUM]: same `AutoAvx2` kernel single vs threaded,
  no new scalar/SIMD mix. (4) FMA/fast-math [HIGH]: F4 kernels frozen (the mine
  confirms they already avoid FMA). (5) Reduction-tree shape [MEDIUM]: no tree
  reduction anywhere (each dot is whole, sequential). (6) Work-stealing [LOW with
  output partition]: see above.

## Alternatives considered

- **Partition the contraction axis / a tree reduction:** rejected — a cross-core
  float reduction is non-deterministic and poisons the parity oracle.
- **Route matmul work through the scheduler (spawn CPU-pinned threads):**
  rejected for the per-token hot path — ~169 matmuls/token would mean
  thread create/destroy churn; the persistent AP-poll fork-join has no per-matmul
  allocation.

## Addendum — OT-34 Milestone-A: production enablement + the CP-19 guard (2026-07-19, Oracle)

The original OT-34 build (2026-07-04) landed the executor, the `ap_poll` hook in
`run_ap_bootstrap`, and the synthetic parity KAT — all behind `threaded-matmul`,
but **`thread_count` was never raised in production**, so the real forward pass
still ran single-threaded. A subsequent bare-metal enable attempt deadlocked on
the 12-core OptiPlex and was reverted; the concurrency-pitfall bank
(`docs/concurrency-pitfalls.md`, CP-19…CP-29) was mined against that failure.
Milestone-A ("JC can sit down and *use* it — a faster Arthur is a better Arthur")
completes the enablement, pre-inoculated by that bank.

### What changed

1. **Production enablement (ISC-1).** `main.rs` raises `thread_count` to
   `smp::online_cpu_count()` once, after `bring_up_aps()` + `init_bsp()` (APs are
   up and busy-yielding in `run_ap_bootstrap`, each polling `ap_poll`; the BSP is
   shootdown-ready). The per-token forward matmul now forks across the online cores.
   Kill-switch intact: feature-off removes the call and the module; a single-CPU
   boot clamps to 1 (single-threaded); the parity KATs still override the count.

2. **CP-19 guard closed (ISC-2).** The AP matmul-work-slot poll is now a single
   shared hook (`scheduler::poll_ap_matmul_work`) called from **both** AP idle
   loops — `run_ap_bootstrap`'s busy-yield (the live path today) **and**
   `idle_thread_entry`'s park/halt (latent until an AP-efficiency pass). Carrying
   the hook into the idle loop closes the CP-19 seam *before* it can bite: an AP
   that ever parks and wakes on a tick still drains its work slot instead of
   sleeping through the assignment. The poll runs at the idle loop's top with IF=1,
   *before* the IF-off ready-queue check, so the CP-22/CP-28 `sti; hlt`
   check-then-halt discipline is byte-for-bit unchanged. Feature-off compiles the
   hook to an empty no-op, so the pre-OT-34 idle path is unaffected.

### Why enabling is deadlock-free on current `main` (the CP-20 argument)

`run_generic`'s **steal-back** means the BSP steals any partition an AP has not
claimed within a grace window and completes it inline, so the join **cannot hang
on an absent or parked AP** — correctness and liveness never depend on AP
participation (a missed wake is a perf loss, never a hang: the CP-20 guard). The
original 12-core deadlock was **CP-29** (concurrent AP per-CPU heap-init buddy-lock
+ shootdown-to-IF=0-AP), which is **already fixed on `main`** (serialized bringup
+ the I-16 `shootdown_ready_cpus_mask`, `c498f5ce`) — neither of which this build
touches. So the enablement is deadlock-free by construction under the current
busy-yield AP model; the CP-19 hook is defense-in-depth for the latent park path.

### Determinism proof (ISC-3, extended) + the bare-metal close

- **In-gate hermetic (`neo-test-threaded-matmul-parity`, NEO_SMP=4):** real
  quantized matmuls on real APs at `thread_count` 1/2/4, now asserting **both**
  byte-identity **and** greedy-argmax **token-id** identity vs the single-threaded
  truth. A **negative control** arms a sub-ULP mantissa-bit perturbation on the
  `col_start > 0` partitions (`par_matmul::set_perturb`, test-only) and proves the
  byte-parity compare **reds** on exactly the kind of partition-order-dependent,
  "usually matches" divergence the determinism rule forbids — then disarms and
  proves parity is restored. This makes the parity KAT falsifiable (the B2b /
  fast-gen discipline).
- **End-to-end real-generate token parity:** because neomc's `.neom` writer emits
  only F32/F16/BF16 (no quantized dtypes), a hermetic fixture cannot exercise the
  quantized fork; the full generate-path `tc=1 ≡ tc=N` token-identity on the **real
  Qwen brain** is therefore proven on the **bare-metal OptiPlex SMP run** (which
  also proves the deadlock is gone on real silicon) — per the dispatch's
  bare-metal-close clause and `docs/runbook-baremetal-arthur-optiplex.md` (serial
  log is the proof; no photo).
