<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Deterministic parallel matmul — prior-art mine (GGML/llama.cpp, OpenBLAS, PyTorch)

**Date:** 2026-07-04
**Lane:** Prometheus (deep prior-art mining — NO kernel code)
**Delivers for:** OT-34 threaded matmul oracle (`DISPATCH-2026-07-04-ot34-threaded-matmul-oracle.md`) — de-risks ISC-1/ISC-2 determinism constraint
**Sources:** ggml-org/llama.cpp `src/ggml-cpu/ggml-cpu.c` (matmul dispatch, threadpool, barrier), `src/ggml-cpu/ggml-quants.c` (vec_dot accumulation), `ggml/src/ggml-cpu/simd-gemm.h` (SIMD GEMM ukernel); OpenBLAS `driver/level3/level3_thread.c` (2D thread partition); PyTorch `torch.use_deterministic_algorithms` docs; NumPy BLAS delegation notes

---

## 0. Executive summary

**The recipe for byte-identical threaded matmul:** Partition by **output rows** so each thread owns disjoint output elements, accumulate sequentially within each thread, and join with a barrier. No cross-thread reduction of the same output element = trivially deterministic regardless of thread count.

**GGML does this.** Its matmul partitions output row-tiles across threads. Each thread computes its tile's dot products with strict sequential accumulation. No two threads write the same output element. The result is structurally deterministic — identical bytes for any thread count, because each output element always has the same accumulation order regardless of which thread computes it.

**OpenBLAS does this too** (2D M×N tile partition), but its micro-kernel K-block accumulation order changes with thread count because different threads process different tiles with different K-block trip counts. This means OpenBLAS is deterministic for a **fixed** thread count, but NOT byte-identical across different thread counts.

**PyTorch's `use_deterministic_algorithms()` does NOT address CPU matmul determinism.** It targets CUDA atomic-reduce operations only. CPU BLAS-backed matmul is conspicuously absent from the coverage list.

---

## 1. The determinism recipe (ISC-1)

### 1.1 The core insight: partition by output rows, never split the K-dimension

For `C[i,j] = Σ_k(A[i,k] * B[k,j])`, there are two ways to parallelize:

**Path A — Row partition (DETERMINISTIC):** Split the output matrix C along its row dimension. Thread T computes rows `[row_start_T .. row_end_T)`. Each output element `C[i,j]` is computed by exactly ONE thread, with the full K-dimension accumulated sequentially inside that thread. No cross-thread reduction of any output element.

**Path B — K-dimension split (NONDETERMINISTIC):** Split the contraction dimension K across threads. Thread T computes partial sums for `k ∈ [k_start_T .. k_end_T)`, then all threads must reduce their partial sums into the same `C[i,j]`. The reduction order depends on thread boundaries, which depend on thread count. Float addition is non-associative: `(a+b)+c ≠ a+(b+c)` in IEEE-754.

**Recommendation: Path A exclusively.** It eliminates the parallel reduction problem entirely. No ordered combine needed, no atomics on output data, no reduction-tree shape to reason about.

### 1.2 GGML's implementation (the proven reference)

**Source:** `ggml-cpu.c` — `ggml_backend_cpu_compute_forward_mul_mat()`

GGML partitions the output matrix into row-tiles:

```c
int chunk_size = 16;
int64_t nchunk0 = (nr0 + chunk_size - 1) / chunk_size;  // chunks along src0 rows
int64_t nchunk1 = (nr1 + chunk_size - 1) / chunk_size;  // chunks along src1 rows
```

Each chunk computes a disjoint output region `dst[ir0_start..ir0_end][ir1_start..ir1_end]`. The work-stealing counter (`atomic_fetch_add` on `current_chunk`) determines which thread picks up each chunk, but **the numerical result is independent of assignment** because:
- Each chunk writes to a unique output region (no overlap)
- Within a chunk, accumulation is strictly sequential and ordered

When chunks are fewer than `nth * 4`, GGML falls through to direct per-thread partition:
```c
nchunk0 = nr0 > nr1 ? nth : 1; // parallelize by src0 rows
nchunk1 = nr0 > nr1 ? 1 : nth; // parallelize by src1 rows
```

This is the simplest deterministic partition: N threads, N row-slices, no work-stealing.

**Per-chunk computation (`ggml_compute_forward_mul_mat_one_chunk`):** No atomics touch the output data. Each chunk's `vec_dot` calls accumulate into a local `float tmp[32]` buffer, then memcpy to the disjoint output region.

### 1.3 The barrier join

GGML uses `ggml_barrier()` with `memory_order_seq_cst` fences between graph nodes. For our use case, Oracle's SMP barrier (`smp.rs`) serves the same purpose — all threads complete their partition, then the next forward layer proceeds.

### 1.4 Recommended recipe for Oracle

```
// Pseudocode for deterministic threaded matmul
partition rows across N CPUs:
    row_start[T] = (rows * T) / N
    row_end[T]   = (rows * (T+1)) / N

barrier()  // sync before work dispatch

for CPU T in parallel:
    for row in row_start[T]..row_end[T]:
        for col in 0..cols:
            out[row * cols + col] = vec_dot_q4_k_q8_k(rhs_col, lhs_row)

barrier()  // sync before next forward layer
```

**Determinism proof:** Each `out[i,j]` is written by exactly one CPU. The `vec_dot` accumulation order is fixed (sequential block iteration). Different thread counts produce different row-slice boundaries, but each output element's computation is self-contained within its owning thread — no cross-thread interaction affects any output value.

**Caveat:** If the number of rows is smaller than the thread count, some threads get zero rows. This is fine (they idle through the barrier). If `rows < N`, fall back to single-threaded or partition by columns instead (same principle — disjoint output elements).

---

## 2. The trap bank (ISC-2)

### Trap 1: Float reassociation in parallel reduction

**What breaks it:** Splitting the K-dimension across threads and reducing partial sums into the same output element. IEEE-754 float addition is non-associative: `(a+b)+c ≠ a+(b+c)` when operands differ in magnitude by more than one ULP. With 256-element dot products (Q4_K block size), the last few bits of the result depend on accumulation order.

**How to avoid:** Partition by output rows (Path A above). Each thread computes the full K-dimension for its rows. No cross-thread reduction of any output element.

**Severity:** CRITICAL — this is exactly what breaks the OT-30 parity oracle. The near-tie fragility we saw at 1.5B means a single-bit change in a logit can flip the top-1 token.

### Trap 2: Thread-count-dependent tiling

**What breaks it:** OpenBLAS partitions output tiles using `nthreads_m = args->nthreads`. Different thread counts produce different tile boundaries. Each C[i,j] element is still computed by exactly one thread, but **which thread** changes. The micro-kernel's K-block accumulation order depends on the tile size, so different threads accumulate the same logical dot product in a different order.

**Evidence:** OpenBLAS `level3_thread.c` — `mypos_m = mypos % nthreads_m` determines the M-range per thread. The GEMM kernel iterates K in blocks of `GEMM_Q`. Different tile sizes mean different numbers of K-block invocations per output element, which changes the FMA accumulation order.

**How to avoid:** Use a FIXED partition scheme that doesn't depend on thread count. Simplest: always compute each row the same way regardless of which thread owns it. GGML achieves this because its `vec_dot` iterates blocks sequentially within each chunk, and chunks are row-aligned.

**Severity:** HIGH — changing from 1-thread to 4-threads changes bytes even with row partitioning IF the dot product implementation itself tiles differently based on available parallelism. Our F4 kernels don't do this (they're sequential per-dot), so we're safe as long as we don't introduce K-dimension parallelism within a single dot product.

### Trap 3: SIMD horizontal-sum order

**What breaks it:** Converting a SIMD vector of partial sums to a scalar via horizontal reduction. The order of additions in `_mm256_hadd_ps`-style intrinsics is fixed by the instruction, but differs from sequential scalar accumulation. This is fine for a single implementation, but becomes a trap when:
1. You have both scalar and SIMD paths and expect byte-identical results
2. The SIMD path changes (AVX2 vs AVX-512)

**Our code:** `horizontal_sum_i32x8()` in `qk_matmul.rs:1122-1136` stores to a 32-byte local buffer then sums lanes 0..7 sequentially. This is deterministic within the AVX2 path. The scalar `vec_dot_block_q4_k_q8_k()` accumulates into `i32` variables (exact integer arithmetic, no float reassociation risk).

**How to avoid:** If mixing scalar and SIMD paths, verify they produce identical results for the same input. Our code already does this — the AVX2 `quantize_block_q8_k` is documented as "bit-identical to the scalar oracle" (`qk_matmul.rs:1138`). The dot products use integer accumulation (i32) for the heavy lifting, with only the final scale-to-f32 being a float operation.

**Severity:** MEDIUM — relevant only if Oracle introduces AVX-512 or NEON paths alongside existing scalar/AVX2. Not a threat for the initial threaded implementation using the same kernel per thread.

### Trap 4: `-ffast-math` / FMA contraction

**What breaks it:** Compiler flag `-ffast-math` enables:
- **FMA fusion:** `a * b + c` becomes `fmadd(a, b, c)` instead of separate mul+add. FMA rounds only once (mul-add is one exact product, then one round) vs. two rounds (mul → round, add → round). Different last bits.
- **Reassociation:** The compiler may reorder floating-point additions.
- **NaN/Inf relaxation:** Changes behavior for edge cases.

**Evidence in our code:** `qk_matmul.rs:1195` explicitly documents this:
> "No FMA: the scalar is a separate mul then add, and `_mm256_fmadd_ps` would fuse and diverge."

The AVX2 quantize path uses `_mm256_mul_ps` then `_mm256_add_ps` (separate), not `_mm256_fmadd_ps`. The dequantize path similarly avoids FMA (`qk_matmul.rs:1270`).

**How to avoid:** 
- Build without `-ffast-math` (Rust's default is safe — no implicit FMA)
- In `#[target_feature]` intrinsics, use separate mul/add, not fused
- Document the constraint: "F4 kernels are NOT FMA-compatible with the scalar oracle"

**Severity:** HIGH — a single FMA instruction in the hot dot product path would silently produce different bytes under threading vs. single-thread if the compiler decides to fuse one path but not the other. Rust's default compilation doesn't do this, but hand-written intrinsics can accidentally use `_mm256_fmadd_ps`.

### Trap 5: Reduction-tree shape changes

**What breaks it:** If you use a tree-based reduction (pairwise sum of partial results) instead of sequential accumulation, the tree shape depends on the number of elements being reduced. Different thread counts → different partition sizes → different tree shapes → different results.

**How to avoid:** Don't use parallel reduction for the dot product. Sequential accumulation within each thread is already fast enough because the quant dot products are integer-heavy (i32 accumulators) and only the final scale-to-f32 is a float operation. Our `vec_dot_q4_k_q8_k` iterates blocks sequentially (`block_idx = 0..blocks_per_dot`) — no tree reduction.

**Severity:** MEDIUM — not currently used in our code, but a common optimization trap when porting from GPU (where tree reductions are mandatory).

### Trap 6: Work-stealing nondeterminism (subtle)

**What breaks it:** GGML uses `atomic_fetch_add` with `memory_order_relaxed` for work-stealing. Which thread picks up which chunk is non-deterministic. However, GGML's determinism holds because each chunk writes to a **disjoint** output region. If two chunks could write overlapping regions, the assignment order would matter.

**How to avoid:** Use static partition (no work-stealing) for the initial implementation. Divide rows evenly: `row_start[T] = (rows * T) / N`. Simple, deterministic, no atomics needed for work distribution. Add work-stealing later only if load imbalance is measured to be a problem (unlikely for matmul — each row does roughly the same work).

**Severity:** LOW with row partition (disjoint output). HIGH if using K-dimension split (overlapping reductions).

---

## 3. Applicability note for our F4-kernel matmul (ISC-3)

### 3.1 Our matmul shape

From `qk_matmul.rs:386-443`, the current scalar `matmul_q4_k_q8_k`:
```rust
lhs_q8: &[BlockQ8K]   // (rows × shared) quantized activation
rhs_q4: &[u8]          // (cols × shared) Q4_K quantized weight
out: &mut [f32]        // rows × cols, row-major
```

The outer loop iterates `row`, then inner loop iterates `col`. Each `(row, col)` pair calls `vec_dot_q4_k_q8_k` which accumulates over the shared (K) dimension sequentially.

**Key property:** The row loop is naturally parallelizable. Each row's output is independent. No data dependency between rows.

### 3.2 The threading change

**Minimal edit to the matmul:** Replace the outer `while row < rows` loop with a per-CPU partition:

```rust
// Before (single-threaded):
for row in 0..rows { compute_row(row) }

// After (threaded, deterministic):
let my_rows = partition(rows, cpu_id, num_cpus);  // disjoint ranges
for row in my_rows { compute_row(row) }
barrier_wait(num_cpus);
```

**No change to the F4 kernels.** The `vec_dot_q4_k_q8_k` function remains identical — it already accumulates sequentially over blocks. The threading sits **outside** the dot product, not inside it. This is the GGML pattern: partition around the kernel, not within it.

### 3.3 SMP infrastructure

Oracle's `smp.rs` provides the per-CPU barrier. The partition + compute + join pattern maps directly:
1. Each CPU computes its row slice
2. Barrier wait ensures all rows are complete before the next forward layer reads the output
3. No shared mutable state — each thread writes to disjoint `out[]` indices

### 3.4 Determinism verification strategy

For Oracle's ISC-2, the proof is straightforward:
1. Run single-threaded matmul → capture output bytes
2. Run N-threaded matmul (N=2,4) → compare output bytes
3. Byte-identical = pass. Any difference = trap hit.

Additionally, the OT-30 parity oracle (llama.cpp top-1 token match) serves as the integration-level proof: if threading changes a logit enough to flip the near-tie, the Stage-A oracle catches it.

### 3.5 Column partition alternative

If `rows` is very small (fewer than CPU count), partition by columns instead. Same principle — disjoint output elements. For transformer inference, `rows` is typically the batch size × sequence length, which is large enough for row partitioning. The edge case of `rows < N` should fall back to single-threaded (the kill-switch default).

### 3.6 Q8 activation quantization

The `quantize_row_q8_k()` function runs BEFORE the matmul and produces the `lhs_q8` input. It's already per-row, so it can also be parallelized using the same row-partition pattern. Each thread quantizes its own rows. No cross-thread dependency.

---

## 4. Summary table

| Source | Partition scheme | Cross-thread reduction? | Deterministic across thread counts? | Notes |
|--------|-----------------|------------------------|-------------------------------------|-------|
| **GGML/llama.cpp** | Output row-tiles (16-row chunks) | No | Yes | Work-stealing is safe because chunks are disjoint |
| **OpenBLAS** | 2D M×N tiles | No (per-element) | **No** | K-block accumulation order changes with tile size |
| **MKL** | Output rows | No | Similar to OpenBLAS | Intel TBB scheduling may vary accumulation |
| **PyTorch CPU** | Delegates to BLAS | Depends on BLAS | **No guarantee** | `use_deterministic_algorithms()` doesn't cover CPU matmul |
| **Our F4 kernels** | Sequential per-row (single-thread) | N/A | Trivially deterministic | Ready for row-partition threading — no kernel changes needed |

---

## 5. Recommended implementation order for Oracle

1. **Static row partition** — `(rows * T) / N` to `(rows * (T+1)) / N`, no work-stealing
2. **Barrier join** — reuse `smp.rs` barrier, no new locks
3. **Kill-switch** — feature flag off = single-threaded loop (byte-identical to today)
4. **Determinism KAT** — compare 1-thread vs 2-thread vs 4-thread output bytes on a known input
5. **OT-30 parity oracle** — verify top-1 tokens unchanged under threading
6. **OT-7 latency measurement** — measure speedup ratio (QEMU-TCG = relative only)
7. **Work-stealing** (optional, later) — only if load imbalance is measured to matter

**Do NOT:** Split the K-dimension. Use atomics on output data. Introduce ordered reduction of partial sums. Change the F4 kernel accumulation order.

---

*This is a research document. No kernel code was modified. All claims about GGML/OpenBLAS behavior are based on reading the actual source files (ggml-cpu.c, ggml-quants.c, level3_thread.c), not summaries.*
