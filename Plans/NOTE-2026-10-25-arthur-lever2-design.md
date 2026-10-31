<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: lever 2: what's left serial after lever 1, and when to parallelize it

**Seat:** S4. **Status:** design only, no code. The PM asked for it after #128 (lever 1) merged. It covers
how lever 2 would split the per-element soft work across CPUs, and what A7/A8's overlay numbers would have
to show to justify building it. Background: `Plans/NOTE-2026-10-25-arthur-non-matvec-cost.md`.

## What's left serial after lever 1 (host, one thread, POS=25)

| piece per layer | before lever 1 | after lever 1 | what's inside now |
|---|---|---|---|
| `ops::swiglu` (4864 elements) | 1.18 ms | 0.39 ms | about 80 ns per element: hardware `exp` (about 12 ns), integer `div` (about 18 ns), and the `read_bits`/`write_bits` byte access with bounds checks |
| `gqa_causal_attention_last` at the last position | 2.2 ms | 0.35 ms | grows with position: 14 heads × (2 × pos dot products of 64 + pos `exp` + 64 × pos multiply-adds + 64 integer `div`) |
| seam: norm, bias, RoPE | about 0.16 ms | below 0.16 ms | RoPE `sin`/`cos` via soft ops, norms with integer `div`/`sqrt` |
| **total serial** | **about 3.5 ms** | **about 0.9 ms** | |
| matvecs (split across CPUs in the kernel) | 4.2 ms single-thread | unchanged | |

So after lever 1, on 4 CPUs, a layer is roughly 1.1 ms of parallel matvec work plus about 0.9 ms serial.
**About 45% of the layer is still serial on c0**, and the serial part grows with position (the attention
core).

## Lever 1b: serial work removed without new threading (byte-identical)

Read off `gqa_causal_attention_last` (`kernel/src/ai/ops/mod.rs`):
- **Each score is computed twice.** `gqa_score_max` computes every score to find the max, then the
  softmax loop recomputes each one with `gqa_attention_score`. Keeping the scores from the first pass in the
  existing `weights` buffer halves the Q·K dot products. It's the same function on the same inputs, so the
  same bits come out.
- **The V sum is column-major.** For each of the 64 output dims it walks `k_pos` and recomputes `gqa_index`
  plus a bounds-checked `read_bits` per element. Reordering the loops isn't allowed: it changes the
  summation order, and with it the bytes. Hoisting the index arithmetic and reading the value row once per
  `k_pos` keeps the order and drops the per-element overhead.
- **Element access.** `read_bits`/`write_bits` return `Result` per element. Validating the slice length once
  per op and reading through a checked `u32` view keeps the bytes and removes most per-element work. The
  same applies to `swiglu`.

Expected effect: the attention core is roughly two-thirds dot products and indexing, so about 0.35 →
0.2 ms/layer, and `swiglu` sheds most of its non-arithmetic overhead. That still has to be measured. It's
cheap, serial, and byte-identical by construction, gated by the same both-paths KATs as lever 1. **Do it
before lever 2:** it shrinks what lever 2 would have to split, and it can't make lever 2's case worse.

## Lever 2: split the per-element work across CPUs

**What splits, and why the bytes can't change:**
- **SwiGLU.** Each output element depends only on its own `gate[i]` and `up[i]`. Split the 4864 elements
  into contiguous ranges, one per CPU. No reduction crosses a boundary, so any split gives the same bytes.
- **Attention core.** Each q-head's output (`max`, `denom`, the 64 accumulators) depends only on that head,
  its kv-head (read-only, shared in GQA) and the cache. Split the 14 heads across CPUs (4/4/3/3 on 4 CPUs).
  Each head's summation order is unchanged, so the bytes are the same.
- **Not split:** `rms_norm` (one reduction over the row; splitting would reorder the sum and change bytes),
  the seam's RoPE (small; revisit only if the overlay says so).

**How, on the existing executor (`kernel/src/ai/par_matmul.rs`):**
- Add two partition kinds next to the matmul ones: an element range for SwiGLU, and a head range for
  attention. Same per-CPU slots, same single-winner CAS claim, same BSP steal-back, same `PAR_BUSY`
  try-guard and single-threaded fallback.
- **No new lock, no new invariant:**
  - the partitions write disjoint output ranges, as the matmul partitions do;
  - the join touches no AI-governance lock (I-14), and nothing parks.
- **Thresholds.**
  - SwiGLU uses the existing `PAR_MIN_COLS = 512` on element count.
  - Attention splits only when `n_q_heads × pos × head_dim` passes a threshold set from the overlay. At
    position 1 the core is about 0.07 ms, and the fork-join costs more than it saves.
- **Kill-switch:** `THREAD_COUNT = 1`, or a new `par-elementwise` feature off, gives the single-threaded
  path bit-for-bit.
- **Cost:**
  - two more fork-joins per layer, 48 per token on 24 layers;
  - the forward already does about 120 matvec fork-joins per token, and A6-hwp's overlay put the
    barrier at `bar=3%`;
  - so the added barriers should cost about 1–1.5% of forward time.

**Expected gain on 4 CPUs:** the split pieces drop to about a quarter, plus barrier cost. With lever 1b
first, roughly 0.5 → 0.15 ms/layer, so about 8 ms/token of host-speed serial time. That's about 15% of a
lever-1 layer. The Zenbook figure scales from A8's measured split, not from this.

## What A7/A8 must show (the go/no-go)

A7 is the image without lever 1; A8 includes it. The overlay already prints
`perf pre/dec ms/t`, `mv=` (matvec share of forward cycles, fork-join included) and `bar=`.

1. **Lever 1 worked as predicted:** A8's `dec ms/t` is well below A7's, and `mv=` rises from about 17%
   (A6-hwp) towards about 50%.
   - **If `mv=` barely moves** while ms/t falls, something other than matvecs and soft f32 dominates. Lever 2
     is the wrong next step; check c0 preemption and core type first (the discriminators in the parent
     NOTE).
2. **Serial share still worth splitting:** on A8, `1 − mv − bar` is the serial share.
   - **Build lever 2 if it's ≥ about 35%.** A 4-way split of that share saves at least about a quarter of
     forward time, minus about 1.5% of barrier.
   - **Skip it if it's ≤ about 20%.** The gain falls below about 15% and competes with fix 4b and other
     levers.
   - In between, decide on the per-piece split below.
3. **Which piece dominates.** The overlay doesn't split the serial share yet. Before building lever 2, add
   two cycle counters to the `prefill-perf` overlay, `swiglu` and `attn` (next to `MATVEC_CYCLES`, same
   feature gate, no effect on bytes), and print `sw=`/`at=` shares. The case for lever 2 rests on:
   - **`at=` large and growing with position:** split attention by heads. This is the bigger and growing
     piece.
   - **`sw=` large:** split SwiGLU by element range.
   - Both small: lever 2 isn't worth the executor change.

## Falsification

- **Lever 1b** is wrong if the both-paths KATs show any byte difference. It doesn't pay if the attention
  core drops less than about 30% on the host split.
- **Lever 2** is wrong if a partitioned run's bytes differ from the single-threaded run's: the same
  determinism KAT the matmul partitions use, extended to the new kinds. It doesn't pay if the overlay
  after it shows `bar=` rising by more than the serial share it removed.

## Proposed order

1. **Overlay counters `sw=`/`at=`** (tiny, perf feature only). These ride whichever image follows A8.
2. **Lever 1b** (serial, byte-identical, no threading).
3. **Lever 2**, only if the A8 numbers clear the thresholds above.
4. **Fix 4b** (dynamic chunking, ≤ 7%) stays last.
