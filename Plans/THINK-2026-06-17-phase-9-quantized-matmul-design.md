<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 9 — quantized matmul design (q4_K × q8_K)

**Date:** 2026-06-17
**Author:** Prometheus
**Branch:** `prometheus/phase-9-quantized-matmul-design`
**Inputs:** `Plans/RESEARCH-2026-06-16-phase-9-quantized-matmul-prep-FINDINGS.md`, live code at `origin/main` HEAD `4024255`
**Scope:** Design THINK only. No production code, no `ci.yml`.

---

## 1. Problem statement

The Phase 8 forward pass (8.3 → 8.5) dequantizes Q4_K weights to f32, then runs the soft-f32 matmul kernel in `ops/mod.rs:267`. For Qwen2.5-0.5B each layer has three projection matmuls (Q/K/V) at 896×896 and one output projection (896×896), plus FFN gate/up at 896×4864 and down at 4864×896. Dequantizing the weight tile to f32 before every matmul expands each block_q4_K from 144 B → 1024 B (7.1×) and forces the matmul to read full f32 weights through L1/L2 on every layer. At 24 layers this is the dominant memory traffic in the forward pass.

**Phase 9 replaces:** `dequant(q4_K) → f32_matmul` **with:** `q4_K × q8_K_integer_dot → f32_accumulator`, keeping weights compressed through the multiply-accumulate and cutting weight-memory traffic ~7×.

---

## 2. The op math

### 2.1 Block layouts (pinned to llama.cpp `74ade52`)

**block_q4_K** — `ggml/src/ggml-common.h:317-328` — 144 B / 256 elements (4.5 bpw)

| Field | Size | Purpose |
|-------|------|---------|
| `d` | 2 B (fp16) | Global scale multiplier for sub-block scales |
| `dmin` | 2 B (fp16) | Global scale multiplier for sub-block mins |
| `scales[12]` | 12 B | Packed 6-bit scales + mins (8 sub-blocks) |
| `qs[128]` | 128 B | 4-bit quants, 2 per byte |

Reconstruction per sub-block of 32 elements: `x[i] = d * scale[sub] * q[i] - dmin * min[sub]`

**block_q8_K** — `ggml/src/ggml-common.h:360-366` — 292 B / 256 elements

| Field | Size | Purpose |
|-------|------|---------|
| `d` | 4 B (fp32) | Global scale |
| `qs[256]` | 256 B | int8 quantized values |
| `bsums[16]` | 32 B | Precomputed sums of qs in groups of 16 |

Reconstruction: `x[i] = d * qs[i]` (simple global scale, no per-sub-block scales).

**Constant:** `QK_K = 256`, `K_SCALE_SIZE = 12` — `ggml-common.h:89-90`.

### 2.2 The q4_K × q8_K dot product (per QK_K-block)

Reference: llama.cpp scalar at `ggml/src/ggml-cpu/quants.c:645-718`, AVX2 at `arch/x86/quants.c:1900-2076`. Commit `74ade52`.

Per super-block `i`:

```
d    = y[i].d * fp16_to_fp32(x[i].d)
dmin = -y[i].d * fp16_to_fp32(x[i].dmin)

// 1. Depack scales from 12-byte packed format → 16 values [scale0..7, min0..7]
//    Using kmask1/kmask2/kmask3 bit-manipulation (see §6.1)

// 2. Min-correction term (dmin contribution):
//    acc_m += dmin * hsum(madd_epi16(mins_and_scales, y[i].bsums))

// 3. Main dot product — per sub-block j = 0..3:
//    For each pair of 32-element sub-block halves:
//      a. Get scale_l, scale_h via get_scale_shuffle_k4(j) lookup table
//      b. Split q4 into lo/hi nibbles
//      c. Integer MAC: p16 = maddubs(q4_nibble, q8_chunk)  →  16×(u8×u8→i16)
//      d. Scale: p32 = madd_epi16(scale, p16)              →  8×(i16×i16→i32)
//      e. Accumulate to i32 sum

// 4. Final: *s = hsum_float_8(d * sumi) + acc_m[0]
```

**Critical intrinsics (AVX2 path):**
| Intrinsic | Role |
|-----------|------|
| `_mm256_maddubs_epi16` | Core MAC: 16×(u8×u8) → 16×i16 |
| `_mm256_madd_epi16` | Signed scale multiply: 8×(i16×i16) → 8×i32 |
| `_mm256_shuffle_epi8` | Per-sub-block scale selection via lookup table |
| `_mm256_fmadd_ps` | Fused multiply-add for float accumulation |

**Scale shuffle table:** `x86/quants.c:527-539` — 256 B, 16 rows × 16 bytes. Each row encodes which unpacked scale/min value applies to each element in a 32-element sub-block half.

### 2.3 q8_K activation quantization (on-the-fly)

Reference: `ggml/src/ggml-quants.c:2696-2733`. Commit `74ade52`.

Per QK_K-block of the f32 activation vector:

```
1. Find max_abs = max(|x[j]|) for j in 0..255, track its sign
2. iscale = -127.0f / max_abs          // NOTE: -127, not -128
3. qs[j] = clamp(-127..127, round(iscale * x[j]))
4. bsums[k] = sum(qs[16*k .. 16*k+15])  for k in 0..15   // precomputed sums
5. d = 1.0f / iscale                     // = -max_abs / 127
```

**Why -127 not -128:** Required for IQ2_XXS compatibility; avoids sign-bit edge cases in SIMD. Leaves value 0 reserved.

**bsums are essential:** They let the dot product compute the min-correction term in O(16) instead of O(256) per block. Without bsums, you iterate all 256 q8 values against each sub-block's min, destroying the performance advantage.

### 2.4 Weight-traffic quantification (Qwen2.5-0.5B)

**Model dimensions:** `hidden=896`, `intermediate=4864`, `layers=24`.

**Per-layer weight matmuls (weights are the RHS, stored as Q4_K in GGUF):**

| Matmul | Shape (M×K × K×N) | Weight elements (K×N) | f32 bytes | q4_K bytes |
|--------|-------------------|----------------------|-----------|------------|
| attn_q | 1×896 × 896×896 | 802,816 | 3,211,264 | 451,584 |
| attn_k | 1×896 × 896×128 | 114,688 | 458,752 | 64,512 |
| attn_v | 1×896 × 896×128 | 114,688 | 458,752 | 64,512 |
| attn_o | 1×896 × 896×896 | 802,816 | 3,211,264 | 451,584 |
| ffn_gate | 1×896 × 896×4864 | 4,358,016 | 17,432,064 | 2,447,232 |
| ffn_up | 1×896 × 896×4864 | 4,358,016 | 17,432,064 | 2,447,232 |
| ffn_down | 1×4864 × 4864×896 | 4,358,016 | 17,432,064 | 2,447,232 |
| **Per layer** | | **14,409,056** | **57,636,224** | **8,173,888** |

**Across 24 layers:** f32 = 1,383,269,376 B (~1.32 GiB) vs q4_K = 196,173,312 B (~187 MiB). **Ratio: 7.05×.**

The ~7× figure is the weight-memory-traffic reduction per forward pass. The activation traffic is unchanged (activations are f32 in both paths; Phase 9 quantizes them to q8_K on-the-fly in a scratch buffer before the matmul, so the activation read is still from f32 memory).

**Note:** The existing dequant path (`gguf_dequant_logic.rs:67`) materializes ALL weight tensors to f32 at load time. Phase 9's traffic win is realized when we skip that full-materialization for Q4_K tensors and stream the compressed blocks directly into the quantized matmul kernel.

---

## 3. Wiring seam in forward_logic

### 3.1 Current path

`forward_logic.rs:255` — `matmul_vec()` converts f32 slices to byte arrays, wraps them as `TensorView { dtype: Dtype::F32 }`, and calls `ops::matmul()`. The weights arrive as `&'a [f32]` in `Qwen2LayerWeights`, already dequantized by `gguf_dequant_logic.rs`.

The matmul call chain:
```
forward_layer_with_gqa_attention()  →  attention_seam()  →  matmul_vec()  →  ops::matmul()
                                                                          (ops/mod.rs:267, f32 only)
swiglu_ffn()  →  matmul_vec()  →  ops::matmul()
```

### 3.2 Phase 9 seam

The seam is at `matmul_vec()` in `forward_logic.rs:255`. Phase 9 introduces a parallel path:

```
matmul_vec_quantized(weights_q4k: &[u8], activations_f32: &[f32], ...) 
  → quantize_activations_to_q8k(activations_f32, scratch)
  → ops::matmul_q4k_q8k(weights_q4k, activations_q8k, out)
```

**Wiring options:**

**Option A — New function alongside `matmul_vec`:** Add `matmul_vec_quantized()` to `forward_logic.rs`. Callers (`attention_seam`, `swiglu_ffn`) check a feature flag and dispatch. Weights in `Qwen2LayerWeights` become `enum WeightData { F32(&'a [f32]), Q4K(&'a [u8]) }`.

**Option B — New OpKind:** Add `OpKind::MatmulQ4K` to `ops/mod.rs`. The executor dispatches based on the LHS tensor's dtype. `exec_ops.rs:57` Matmul arm splits on input dtype.

**Recommendation: Option A** for Phase 9.1-9.2. It keeps the change contained in `forward_logic.rs` and avoids touching the executor graph validation (`exec_ops.rs`) until we're ready to make quantized tensors first-class in the graph (a Phase 9.4+ concern). Option B is cleaner architecturally but requires parallel changes to `exec_ops.rs`, `Dtype` enum, tensor allocation, and the executor — more surface for regressions during the risky kernel development.

### 3.3 Kill-switch (Rule 1)

**Invariant:** `quantized_off ≡ Phase 8.3 dequant→f32→matmul path, bit-for-bit identical.`

Implementation: A module-level const flag in `forward_logic.rs`:

```rust
#[cfg(feature = "phase9-quantized-matmul")]
const QUANTIZED_MATMUL: bool = true;
#[cfg(not(feature = "phase9-quantized-matmul"))]
const QUANTIZED_MATMUL: bool = false;
```

When `QUANTIZED_MATMUL = false`:
- `Qwen2LayerWeights` holds `&'a [f32]` (dequantized at load, as today)
- `matmul_vec()` calls `ops::matmul()` (soft-f32, as today)
- Zero code path divergence — the quantized functions exist but are never called

When `QUANTIZED_MATMUL = true`:
- `Qwen2LayerWeights` holds raw Q4_K bytes alongside f32 fallback
- `matmul_vec_quantized()` is called; activations are quantized to q8_K in scratch
- Output is f32 (same as the current path)

**Gate:** Feature off → existing CI green on all 8.5 tests. Feature on → KAT passes + model output within tolerance.

---

## 4. Correctness oracle (I-12)

### 4.1 The honest catch

**Quantized × quantized is lossy vs f32.** The q4_K weight quantization introduces ~2% absolute error per weight. The q8_K activation quantization introduces ~0.1% error. The integer MAC accumulation is exact within i32 range. The dominant error source is the weight quantization.

Therefore: **The KAT does NOT compare against the f32 dequant→matmul path.** Comparing quantized output to f32 output would fail by construction — that's measuring quantization error, not kernel correctness.

### 4.2 Oracle: Match llama.cpp's q4_K × q8_K kernel output

**Reference:** llama.cpp `ggml_vec_dot_q4_K_q8_K` at `arch/x86/quants.c:1900-2076`, commit `74ade52`. The KAT verifies our Rust implementation produces the same f32 accumulator as llama.cpp's reference on identical input.

**Oracle strategy — synthetic data (primary):**

Replicate llama.cpp's test data generator from `tests/test-quantize-fns.cpp`:
- Synthetic cosine data: `0.1 + 2.0 * cosf(i + offset)` over 4096 elements, fixed seed
- Quantize to Q4_K using our q4_K quantizer (matching llama.cpp's `quantize_row_q4_K_ref`)
- Quantize activations to q8_K using our q8_K quantizer
- Run the dot product
- Compare output f32 against llama.cpp's reference output on the same data

**Tolerance:** `|our_output - reference_output| / N ≤ 0.02` (normalized per-element), matching llama.cpp's own test tolerance in `test-quantize-fns.cpp`. This accounts for fp16→fp32 conversion differences in the scale fields and platform-dependent rounding in the horizontal sum.

**Why synthetic, not real-model weights:** The synthetic data is fully deterministic, requires no binary dependency on llama.cpp's build, and directly exercises the scale depacking / min-correction / integer MAC path with known statistical properties. Real-model vectors are a Phase 9.3 supplement (end-to-end token match).

### 4.3 q8_K quantizer hand-laid oracle

Before integrating into matmul, the q8_K activation quantizer is verified independently:

**Test vector:** Known f32 block of 256 elements with controlled range (e.g., [-1.0, 1.0] linearly spaced). Manually compute expected `qs`, `bsums`, and `d` using the formula from §2.3. Verify our quantizer produces bit-identical `qs` and `bsums`. This catches sign-handling errors in the `-127/max` scaling before they propagate into the matmul.

### 4.4 End-to-end oracle (Phase 9.3)

Once the quantized path is wired: run Qwen2.5-0.5B on a known prompt, compare generated tokens against llama.cpp's output on the same model file. This is an end-to-end sanity check, not the kernel KAT. The tolerance here is "same top-1 token" (matching P8's oracle). Per-token logit divergence is expected due to accumulated quantization error across 24 layers.

---

## 5. CPU-first vs accelerator scope

**Recommendation: CPU-first (AVX2), no GPU in Phase 9.**

**Rationale:**

1. **The substrate advantage is on CPU.** Our gaming PC has AVX2 + BMI2 (Ryzen, per CachyOS research). The `_mm256_maddubs_epi16` intrinsic gives 16×(u8×u8→i16) per cycle — the exact operation q4_K×q8_K needs. This is a native, high-throughput instruction.

2. **Intel Gen9 EU has no native 8-bit MAC.** The GPU has `ADDB8`, `ADDB16`, `ADDW` but no integer multiply-accumulate. A GPU quantized kernel would need manual unpack-and-multiply (load u8 → mul → add), which is 2× the instruction count and loses the throughput advantage. The FINDINGS (§5) confirm: "The GPU quantized matmul would need manual unpack-and-multiply in the kernel, which is less efficient than CPU AVX2."

3. **Phase 9 is about proving the math + wiring.** Adding a GPU kernel path doubles the implementation surface during the riskiest phase (scale depacking bit-manipulation is subtle; an off-by-one in kmask produces silent wrong answers). Prove it on CPU first, then port to GPU in Phase 9.5+ with a known-correct reference.

4. **The memory-bandwidth win is primarily on CPU.** On GPU, weights would already be in device memory (VRAM) where bandwidth is orders of magnitude higher. The ~7× traffic reduction matters most on the CPU memory bus, where L3/cache-line pressure dominates latency.

**Scope:** Phase 9 delivers a CPU scalar reference kernel (portable, no intrinsics) + optional AVX2 acceleration behind `#[cfg(target_feature = "avx2")]`. The GPU path is out of scope, tracked as a Phase 9.5+ item.

---

## 6. Build slicing

### 9.1 — q8_K quantizer + scalar dot kernel + KAT

**Work:**
- Define `BlockQ8K` struct (292 B, distinct from existing `BlockQ8_0` — 32-elem GGUF weight format)
- Add `Dtype::Q4K`, `Dtype::Q8K` to `tensor_logic.rs:Dtype` enum
- Implement `quantize_row_q8_K()` in Rust (f32 → block_q8_K with bsums) — pure function, no simd
- Implement scalar `vec_dot_q4k_q8k()` in Rust based on llama.cpp scalar ref (`quants.c:645-718`)
- Implement scale depacking (kmask1/2/3 bit-manipulation), min-correction via bsums
- Block-aware `matmul_q4k_q8k()` that tiles over QK_K blocks, calls `vec_dot` per output element
- Build KAT oracle: replicate llama.cpp synthetic cosine data generator, run dot product, compare

**Gate:** q8_K quantizer produces bit-identical bsums to hand-computed oracle on known input. Scalar `vec_dot_q4k_q8k()` matches llama.cpp reference output within 0.02 normalized tolerance on 4096-element synthetic data.

**Kill-switchable:** Independent module, not wired into forward pass. Feature flag controls compilation.

### 9.2 — Wire into forward pass + kill-switch integration

**Work:**
- Modify `forward_logic.rs`: add `matmul_vec_quantized()` alongside `matmul_vec()`
- Extend `Qwen2LayerWeights` to carry raw Q4_K bytes (or parallel struct)
- Add `QUANTIZED_MATMUL` feature flag; when off, zero code-path divergence
- When on: quantize f32 activations to q8_K in scratch buffer → call `matmul_q4k_q8k()`
- Update `gguf_dequant_logic.rs`: for Q4_K tensors, store raw bytes alongside (or instead of) f32 dequant when quantized mode is active

**Gate:** Feature off → all existing 8.5 CI tests pass, bit-for-bit identical output. Feature on → KAT passes + forward_layer produces output within tolerance of the f32 path (accounting for quantization loss — compare against llama.cpp's output on same model, not against our own f32 path).

**Kill-switchable:** Cargo feature `phase9-quantized-matmul`. Default: off.

### 9.3 — AVX2 acceleration + latency oracle measurement

**Work:**
- Implement AVX2-accelerated `vec_dot_q4k_q8k_avx2()` using `_mm256_maddubs_epi16` path
- Gate behind `#[cfg(target_feature = "avx2")]` with scalar fallback
- End-to-end forward pass: run Qwen2.5-0.5B through all 24 layers with quantized matmul
- Top-1 token match against llama.cpp on same prompt + model file
- **Latency oracle:** Measure per-layer matmul time (quantized vs f32 path) using the ADR-052 latency harness. Measure weight memory traffic via `perf stat -e mem_load_uops_all,mem_store_uops_all` or equivalent cycle-accurate counter.

**Gate:** AVX2 kernel produces same output as scalar kernel (bit-identical f32 accumulator). Top-1 token matches llama.cpp. Latency oracle records the measured speedup factor for the Strategic Honesty Ledger.

**Strategic Honesty Ledger update:** The ~7× weight-traffic claim moves from §3 Unproven to §1 Proven (or §3 with measured data) once the latency oracle runs. The measurement is: **"Per-layer matmul latency and weight memory bytes read, measured by ADR-052 latency harness on Ryzen AVX2 hardware, comparing quantized path vs f32 dequant path on Qwen2.5-0.5B."** Until this measurement exists, the claim stays §3 Unproven.

---

## 7. Raw-source references (llama.cpp commit `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3`)

| Item | Source | Lines |
|------|--------|-------|
| QK_K, K_SCALE_SIZE | `ggml/src/ggml-common.h` | :89-90 |
| block_q4_K struct | `ggml/src/ggml-common.h` | :317-328 |
| block_q8_K struct | `ggml/src/ggml-common.h` | :360-366 |
| AVX2 vec_dot_q4_K_q8_K | `ggml/src/ggml-cpu/arch/x86/quants.c` | :1900-2076 |
| Scale shuffle table | `ggml/src/ggml-cpu/arch/x86/quants.c` | :527-539 |
| Scalar vec_dot_q4_K_q8_K ref | `ggml/src/ggml-cpu/quants.c` | :645-718 |
| quantize_row_q8_K_ref | `ggml/src/ggml-quants.c` | :2696-2733 |
| quantize_row_q4_K_ref | `ggml/src/ggml-quants.c` | :1399-1469 |
| Type traits (vec_dot_type = Q8_K) | `ggml/src/ggml-cpu/ggml-cpu.c` | :301-310 |
| Matmul dispatch (on-the-fly q8_K quant) | `ggml/src/ggml-cpu/ggml-cpu.c` | :1155, :1245, :1313-1348 |
| Quantization test harness | `tests/test-quantize-fns.cpp` | full file (196 lines) |

---

## 8. Open decisions for PM/PL ratification

| # | Decision | Options | Recommendation | Rationale |
|---|----------|---------|----------------|-----------|
| D1 | **Wiring approach:** Option A (new function in forward_logic) vs Option B (new OpKind in executor) | A: contained, less surface / B: architecturally clean, first-class tensors | **Option A** for Phase 9.1-9.2 | Less surface during risky kernel dev. Option B is Phase 9.4+ when quantized tensors are graph-first-class. |
| D2 | **Weight storage during quantized mode:** Keep f32 dequant + raw bytes (dual) vs raw bytes only | Dual: safe, more memory / Raw only: max traffic savings, breaks f32 fallback | **Dual** during 9.1-9.2; raw-only after 9.3 proves correctness | Kill-switch requires f32 path to remain available. Drop dual storage after latency oracle confirms quantized path is production-ready. |
| D3 | **KAT tolerance:** 0.02 normalized (llama.cpp's own) vs stricter bit-identical | 0.02: matches reference / bit-identical: catches rounding diffs | **0.02 normalized** | fp16→fp32 scale conversion and horizontal sum rounding are platform-dependent. Bit-identical is achievable for the integer paths but not guaranteed across compilers. 0.02 is what llama.cpp itself uses. |
| D4 | **AVX2 in Phase 9.1 or 9.3?** | 9.1: faster perf feedback / 9.3: prove scalar first, then accelerate | **Scalar in 9.1, AVX2 in 9.3** | Scale depacking is the riskiest code. Prove it correct on scalar first with full KAT coverage, then add SIMD as an optimization pass with its own correctness gate (bit-identical vs scalar). |
| D5 | **GPU quantized kernel scope:** Phase 9.5+ or defer to Phase 10 | 9.5: follow-on optimization / Phase 10: separate design cycle | **Phase 9.5+** (same arc, next slice) | Gen9 lacks native 8-bit MAC; the GPU path needs a different kernel strategy (manual unpack-mul-add). Worth designing after CPU proves the math correct. |
| D6 | **Strategic Honesty Ledger row:** Pre-register the ~7× claim now vs wait for measurement | Pre-register: visible commitment / Wait: only add when measured | **Pre-register in §3 now** with the named measurement from §9.3 | The ledger's purpose is to make claims visible and falsifiable. Pre-registering with the measurement definition is the honest stance — it signals "we know what would prove/disprove this." |

---

## 9. Risk register (design-phase)

| Risk | Impact | Mitigation |
|------|--------|------------|
| Scale depacking bit-manipulation off-by-one → silent wrong answers | High | KAT oracle with synthetic data; scalar-first (D4); hand-laid q8_K oracle before matmul integration |
| Q8_0 (existing 32-elem) vs q8_K (256-elem) type confusion | Medium | Distinct names: `BlockQ8K` (activation) vs `BlockQ8_0` (GGUF weight). Documented in code. |
| Integer overflow in i32 accumulator at extreme scale values | Medium | Analytically verify: 32 × (15 × 127) × 63 ≈ 3.8M, well within i32 (±2B). Llama.cpp uses i32 and has not reported overflow. |
| Kill-switch leakage: quantized-off path diverges from 8.3 | High | Gate: feature off → CI green on all existing tests. Automated regression. |
| Quantized forward pass produces different tokens than f32 (beyond tolerance) | Medium | End-to-end oracle in 9.3: top-1 token match vs llama.cpp. If mismatch, the issue is likely cumulative quantization error across layers, not a kernel bug — diagnose with per-layer output comparison. |

---

## 10. PM/PL ratification (2026-06-17)

PM verified the design against the tree — the `matmul_vec` seam is at `forward_logic.rs:566` (the §3 line ref drifted post-8.5; the seam is a named function, so the drift is cosmetic), and the I-12 oracle framing is correct (KAT vs `llama.cpp`'s q4_K×q8_K kernel, **not** the f32 path — §4.1). PL ratified the open decisions.

| OD | Ratified | Note |
|----|----------|------|
| **D1** wiring | **A** (new fn in `forward_logic`) for 9.1–9.2 | OpKind (B) deferred to 9.4+ — minimal surface during risky kernel dev. |
| **D2** weight storage | **Dual** (f32 + raw) thru 9.2; raw-only after 9.3 | the kill-switch needs the f32 path live; dual *temporarily raises* footprint (transitional, intended). |
| **D3** KAT tolerance | **0.02 normalized** (llama.cpp's own) | matches the reference's own tolerance; bit-identical not guaranteed across compilers. |
| **D4** AVX2 timing | **scalar 9.1, AVX2 9.3** (bit-identical-vs-scalar gate) | prove scale-depacking on scalar first; SIMD is an optimization with its own correctness regression. |
| **D5** GPU scope | **Phase 9.5+** | Gen9 lacks native 8-bit MAC; design the GPU kernel after CPU proves the math. |
| **D6** ledger | **pre-register in §3 now — REFINED** | see below. |

**D6 refinement (category honesty — the ledger's top rule).** "~7×" is *two* claims and they belong in different ledger categories:
- The **~7.05× weight-traffic reduction** (1.32 GiB f32 → 187 MiB q4_K/forward pass) is **arithmetic / structural** — computable from the format sizes, true by construction. It is the *basis*, **not** a measurable-and-unproven claim.
- The **latency/throughput improvement** that traffic reduction is meant to produce is the **§3 Unproven** claim — measured by **9.3** (per-layer matmul latency + weight-bytes-read, ADR-052 oracle, Ryzen AVX2). The §3 row added to the Strategic Honesty Ledger pre-registers the *perf* claim, not the traffic arithmetic. This keeps "7× less traffic" (true) from masquerading as "7× faster" (unproven) — the KC-5 drift guard.

**Status:** D1–D6 ratified; Phase 9 is **dispatchable** (slicing 9.1 scalar+KAT → 9.2 wire+kill-switch → 9.3 AVX2+measure). Per the PL sequence (Arthur → Benchmark → Consolidation) and Phase-9's deferred status, the **build stays queued** — and note the clean loop: 9.3's measurement is essentially the Benchmark re-run with quantized matmul, so Phase 9 naturally slots **after** the Benchmark establishes the CPU baseline it must beat.
