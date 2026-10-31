<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Phase 9 quantized-matmul prep — FINDINGS

**Date:** 2026-06-16
**Researcher:** Prometheus
**Source repo:** `ggerganov/llama.cpp` commit `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3` (shallow clone, 2026-06-16)
**Local codebase:** NeoPlutarch `origin/main` HEAD `9f10c79`

---

## 1. The q4_K × q8_K kernel — raw source

### 1.1 Block layouts (ggml/src/ggml-common.h)

#### block_q4_K — **ggml-common.h:317-328** — 144 bytes / 256 elements (4.5 bpw)

```c
typedef struct {
    ggml_half d;              // super-block scale for quantized scales
    ggml_half dmin;           // super-block scale for quantized mins
    uint8_t   scales[12];     // packed 6-bit scales + mins (8 sub-blocks)
    uint8_t   qs[128];        // 4-bit quants (256 nibbles)
} block_q4_K;
```

| Field | Size | Purpose |
|-------|------|---------|
| `d` | 2 B (fp16) | Global scale multiplier for all sub-block scales |
| `dmin` | 2 B (fp16) | Global scale multiplier for all sub-block mins |
| `scales[12]` | 12 B | Packed: first 4 bytes = scales[0..3], next 4 = mins[0..3], last 4 encode upper-2-bits of scales[4..7]/mins[4..7] in top bits of each byte |
| `qs[128]` | 128 B | 4-bit values, 2 per byte (lo/hi nibble) |

Reconstruction: `x[i][j] = d * scale[sub] * q[j] - dmin * min[sub]` per sub-block of 32 elements.

#### block_q8_K — **ggml-common.h:360-366** — 292 bytes / 256 elements (1.125 bpw + overhead)

```c
typedef struct {
    float   d;              // global scale (full fp32)
    int8_t  qs[256];        // one int8 per value
    int16_t bsums[16];      // precomputed sums of qs in groups of 16
} block_q8_K;
```

| Field | Size | Purpose |
|-------|------|---------|
| `d` | 4 B (fp32) | Global scale for the entire block |
| `qs[256]` | 256 B | Full int8 quantized values |
| `bsums[16]` | 32 B | Sum of qs in groups of 16 — enables fast min-correction without iterating all 256 elements |

Reconstruction: `x[j] = d * qs[j]` (simple global scale, no per-sub-block scales).

**Constants:** **ggml-common.h:89-90**
```c
#define QK_K       256    // super-block size
#define K_SCALE_SIZE 12   // bytes for packed scales/mins
```

### 1.2 The AVX2 integer-dot kernel — arch/x86/quants.c:1900-2076

**Function:** `ggml_vec_dot_q4_K_q8_K(int n, float *s, size_t bs, const void *vx, size_t bx, const void *vy, size_t by, int nrc)`

**Critical AVX2 intrinsics used:**
| Intrinsic | Role |
|-----------|------|
| `_mm256_maddubs_epi16` | **Core MAC:** unsigned dot product 16×(u8×u8) → 16×i16 |
| `_mm256_madd_epi16` | Signed scale multiply: 8×(i16×i16) → 8×i32 |
| `_mm256_shuffle_epi8` | Per-sub-block scale selection via lookup table |
| `_mm256_fmadd_ps` | Fused multiply-add for float accumulation |
| `_mm_hadd_epi16` | Horizontal add of bsums for min-correction |

**Algorithm (per block, lines 1919-1976):**

```
For each QK_K-block i:
  1. d    = y[i].d * fp16_to_fp32(x[i].d)
  2. dmin = -y[i].d * fp16_to_fp32(x[i].dmin)

  3. DEPACK scales from 12-byte packed format:
     - Read x[i].scales as 4×u32 into utmp[]
     - Bit-manipulate with kmask1/kmask2/kmask3 to extract
       16 values: [scale0..7, min0..7] → utmp[0..3]

  4. MIN-CORRECTION TERM (dmin contribution):
     - Load y[i].bsums (precomputed group-16 sums of q8)
     - Horizontal-add to pair up bsums into 8×i16
     - Dot with mins: prod = madd_epi16(mins_and_scales, q8_bsums)
     - acc_m += dmin * hsum(prod)

  5. MAIN DOT PRODUCT (per sub-block j = 0..3):
     For each pair of 32-element sub-block halves:
       a. Get scale_l, scale_h via get_scale_shuffle_k4(j) lookup
       b. Load 32 bytes q4 → split into q4l (lo nibble) and q4h (hi nibble)
          using _mm256_and_si256(0xF) and _mm256_srli_epi16(4)
       c. Load 32 bytes q8 for lo half → p16l = maddubs(q4l, q8l)
       d. Scale: p16l = madd_epi16(scale_l, p16l)
       e. Same for hi half with q8h and scale_h
       f. sumi += p16l + p16h

  6. acc += d * sumi (fused multiply-add to float accumulator)

Final: *s = hsum_float_8(acc) + acc_m[0]
```

**Scale shuffle table — x86/quants.c:527-539:**
256-byte lookup: 16 rows × 16 bytes. Each row encodes which of the 16 unpacked scale/min values applies to each element in a 32-element sub-block half. Pattern: pairs `(2j, 2j+1)` repeated 16 times per row.

### 1.3 Scalar reference — quants.c:645-718

Identical math, no SIMD:
- Same scale depacking (kmask1/2/3 bit-manipulation)
- Expands q4 to int8_t aux8[QK_K]
- Min-correction: `sumi += bsums[j] * mins[j/2]`
- Per sub-block: `aux16[l] = q8[l] * a[l]`, then `aux32[l] += scale * aux16[l]`
- Final: `sums[l] += d * aux32[l]`, `sumf -= dmin * sumi`

**This scalar version is the cleanest reference for our soft-f32 implementation.**

### 1.4 Other architecture implementations (for future GPU/EU port)

| Arch | File:line | Notes |
|------|-----------|-------|
| ARM NEON/I8MM | `arch/arm/quants.c:2260` | Uses I8MM dotprod when available — relevant for mobile Arthur |
| WASM simd128 | `arch/wasm/quants.c:919` | 128-bit SIMD via wasm simd128 intrinsics |
| RISC-V | `arch/riscv/quants.c:1634-2077` | 3 variants: XTheadVector, VL128, VL256 |
| PowerPC VMX/VSX | `arch/powerpc/quants.c:896` | VMX vector intrinsics |

---

## 2. Activation quantization to q8_K — ggml-quants.c:2696-2733

```c
void quantize_row_q8_K_ref(const float *x, block_q8_K *y, int64_t k) {
    for each QK_K-block i:
        1. Find max absolute value and its sign in the block
        2. iscale = -127.f / max          // NOTE: -127, not -128
        3. qs[j] = clamp(-127..127, round(iscale * x[j]))
        4. Precompute bsums[16]: sum of qs in groups of 16
        5. d = 1 / iscale                 // = -max / 127
}
```

**Critical detail:** Uses `-127.f/max` instead of `-128.f/max`. Comment: *"Needed for IQ2_XXS compatibility; else the AVX implementation becomes very awkward."* This leaves value 0 reserved and avoids sign-bit edge cases in SIMD.

**bsums precomputation is essential:** It lets the dot product compute the min-correction term in O(16) instead of O(256) per block. Without bsums, you'd need to iterate all 256 q8 values against each sub-block's min — which destroys the performance advantage.

---

## 3. Higher-level matmul dispatch — ggml-cpu.c

### Type traits registration (line 301-310):
```c
[GGML_TYPE_Q4_K] = {
    .from_float     = quantize_row_q4_K,
    .vec_dot        = ggml_vec_dot_q4_K_q8_K,
    .vec_dot_type   = GGML_TYPE_Q8_K,   // activations MUST be Q8_K
    .nrows          = 1,
},
```

### Dispatch flow (ggml_compute_forward_mul_mat):
1. Look up `vec_dot_type` for src0's type → `GGML_TYPE_Q8_K`
2. If src1 (activations) is not Q8_K, quantize to Q8_K in scratch workspace (`params->wdata`)
3. Call `vec_dot(k, src0_row_i, src1_col_j)` for each output element with 16×16 block tiling

**Key insight:** The activations are quantized **on-the-fly** from f32 to q8_K right before the matmul. They are NOT stored as q8_K — only the weights are persistently quantized. This is a one-time per-forward-pass quantization of the activation buffer.

---

## 4. Accuracy analysis

### From llama.cpp test infrastructure (tests/test-quantize-fns.cpp):
- Tests Q4_K with synthetic cosine data over 4096 elements, seed-dependent
- **Round-trip RMSE:** max 0.002 for Q4_K vs fp32
- **Dot product accuracy:** `|quantized_dot - fp32_dot| / N` ≤ 0.02 (normalized per-element)
- No fixed golden vectors — all tests are relative to FP32 baseline on synthetic data

**Second-hand accuracy data (NOT raw-pulled):**
- Llama.cpp benchmarks report Q4_K perplexity degradation of ~1-2% vs F16 for 7B-class models
- The q4_K format was specifically designed to reduce the "outlier" problem of naive Q4_0 by using per-sub-block scales + explicit mins

### Accuracy implications for NeoPlutarch:
The matmul output is f32 (the accumulator). The quantization error comes from:
1. **Weight quantization:** q4_K stores 4.5 bpw weights → ~2% absolute error vs original f32 weights
2. **Activation quantization:** q8_K stores activations at 8 bpw → ~0.1% error (int8 is fine-grained)
3. **Integer-dot accumulation:** The integer MAC is exact within i32 range; no truncation in the accumulator

The dominant error source is the weight quantization, not the activation quantization or the integer arithmetic. This means the KAT oracle needs to compare against the SAME quantized weights dequantized to f32 then multiplied — not against original f32 weights.

---

## 5. Performance analysis

### Why q4_K × q8_K wins over dequant-then-f32-matmul:

| Factor | Dequant→f32 matmul | q4_K×q8_K integer-dot |
|--------|-------------------|----------------------|
| Weight memory read | 144 B → 1024 B f32 (7x expansion in-cache) | 144 B stays 144 B through the kernel |
| Activation memory | 1024 B f32 | 292 B q8_K (3.5x smaller, but computed on-the-fly) |
| MAC operation | FMA (f32 × f32 → f32), 256-bit = 8 per cycle | Integer maddubs + scale, 256-bit = 16 u8×u8 per cycle |
| Cache pressure | Must materialize full f32 weight tile | Weights stay compressed through the dot |
| Bandwidth bound at large K | ~4x slower (reads 4x more bytes) | Memory-bandwidth optimal for 4.5 bpw weights |

**The win is primarily memory-bandwidth:** At large K dimensions, dequantizing to f32 reads 1024 B of f32 per block vs 144 B of q4_K. That's a 7x reduction in weight memory traffic. The integer SIMD also does more work per cycle (16 u8×u8 vs 8 f32×f32).

### What our substrate provides:
- **CPU:** AVX2 available on the gaming PC (Ryzen with X86-7_0/BMI2 — CachyOS ADOPT note confirms AVX2). The soft-f32 path in `ops/soft_f32.rs` is pure Rust and doesn't use SIMD intrinsics yet.
- **GPU:** Intel Gen9 EU (from Phase 6) has `ADDB8`, `ADDB16`, `ADDW` integer instructions but NO native 8-bit MAC. The GPU quantized matmul would need manual unpack-and-multiply in the kernel, which is less efficient than CPU AVX2's `_mm256_maddubs_epi16`.
- **target-cpu:** Per CachyOS research, `target-cpu = "native"` or explicit `-C target-cpu=znver3` (for Ryzen) enables AVX2 auto-vectorization in Rust.

---

## 6. Current NeoPlutarch state — what exists vs. what's needed

### Already present (gguf_logic.rs):
| Item | Status | Line |
|------|--------|------|
| `BlockQ4K` struct | Defined, matches llama.cpp layout | :276 |
| `BlockQ8_0` struct | Defined (note: Q8_0 ≠ q8_K — smaller block) | :284 |
| `parse_q4_k_blocks()` | Exists but **dead code** (never called) | present |
| `parse_q8_0_blocks()` | Exists but **dead code** | present |
| GGUF type enum with Q4K, Q8_0 | Complete parser | :125-147 |
| Quantized weights as U8 blobs | Stored in arena, frozen at load | model.rs:147 |

### Gaps for Phase 9:
| Gap | Description | Effort |
|-----|-------------|--------|
| `Dtype` enum | Add `Q4K`, `Q8K` variants (tensor_logic.rs:18) | Small |
| q8_K block struct | Current `BlockQ8_0` is 32-element Q8_0, not 256-element q8_K with bsums | Medium |
| Activation quantizer | `quantize_row_q8_K()` — f32→q8_K on-the-fly before matmul | Medium |
| Quantized matmul kernel | `matmul_q4k_q8k()` — block-aware integer-dot, output f32 | Large |
| exec_ops dispatch | Accept Q4K weight + f32 activation → auto-quantize to q8_K → call quantized kernel | Medium |
| KAT oracle | Byte-oracle test vectors for q4_K × q8_K matmul | Medium |
| Kill-switch | Feature flag / config: disabled ≡ f32 matmul path | Small |

### Critical design note — Q8_0 vs q8_K:
The existing `BlockQ8_0` in gguf_logic.rs is the **32-element** format (34 bytes, used for GGUF weight storage). Phase 9 needs `block_q8_K` which is the **256-element** intermediate format (292 bytes with bsums) used only for activation quantization. These are different types and must not be confused.

---

## 7. Proposed Phase-9 build slicing

### 9.1 — q8_K activation quantizer + byte-oracle infrastructure
- Implement `quantize_row_q8_K()` (f32 → block_q8_K with bsums) in Rust
- Define `BlockQ8K` struct (292 bytes, distinct from existing `BlockQ8_0`)
- Add `Dtype::Q4K`, `Dtype::Q8K` to tensor_logic.rs
- Build KAT oracle: raw-pull llama.cpp test vectors, run reference f32 matmul on known weights + activations → record golden output
- **Gate:** q8_K quantizer produces bit-identical bsums to llama.cpp reference on the same input

### 9.2 — The q4_K × q8_K dot kernel + KAT verification
- Implement scalar `vec_dot_q4k_q8k()` in Rust (based on ggml scalar ref at quants.c:645)
- Implement block-aware `matmul_q4k_q8k()` that tiles over blocks, calls vec_dot per output element
- Wire scale depacking, min-correction via bsums, integer MAC accumulation
- **Gate:** KAT passes — quantized matmul output matches byte-oracle within tolerance (≤ 0.02 normalized per llama.cpp test)

### 9.3 — Kill-switch integration into forward pass
- Modify `exec_ops.rs` matmul validation: accept Q4K weights + f32 activations
- Auto-path: if weight is Q4K → quantize activations to q8_K in scratch → call quantized kernel
- Feature flag / config toggle; disabled ≡ existing f32 matmul path (bit-for-bit identical)
- **Gate:** Toggle off → CI green on existing f32 tests. Toggle on → KAT passes + same model output within tolerance

---

## 8. I-12 byte-oracle acquisition plan

### Strategy: Raw-pull llama.cpp, generate oracle vectors from its own test infrastructure

**Step 1 — Pull reference implementation:**
Already cloned at commit `74ade52`. Build llama.cpp's `llama-quantize` and `llama-cli` binaries.

**Step 2 — Generate known weight tensor:**
```bash
# Create a deterministic f32 weight tensor (e.g., 1024×256, seeded PRNG)
# Quantize to Q4_K using llama.cpp's quantize tool
./llama-quantize input-f32.gguf output-q4k.gguf q4_k
```

**Step 3 — Generate known activation tensor:**
Deterministic f32 activations (e.g., 256×1, same seeded PRNG).

**Step 4 — Run reference matmul and capture output:**
Use llama.cpp's internal test harness or a minimal C program that:
1. Loads the Q4_K weight block + f32 activation
2. Runs `ggml_vec_dot_q4_K_q8_K` directly
3. Outputs the f32 result to a file

**Step 5 — Store oracle vectors:**
Save as binary f32 arrays in `tools/ai-ops-tests/src/fixtures/quantized_matmul_oracle/`:
- `weights_q4k.bin` (raw block bytes)
- `activations_f32.bin` (input activations)
- `output_f32.bin` (golden output from llama.cpp reference)

**Alternative — use llama.cpp's existing test data:**
The `tests/test-quantize-fns.cpp` uses `0.1 + 2*cosf(i + offset)` over 4096 elements with a fixed seed. We can replicate this exact generator in Rust and compare our quantized dot against the f32 baseline computed on the same data. This avoids binary pulling entirely — just replicate the synthetic data generator.

**Recommendation:** Use the synthetic-data approach (alternative) as primary — it's fully deterministic, no binary dependency, and directly comparable to llama.cpp's own test tolerances. Supplement with real-model vectors (Step 1-5) once Phase 8 has a loaded GGUF model for end-to-end verification.

---

## 9. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Scale depacking bit-manipulation is subtle; off-by-one in kmask → silent wrong answers | High (incorrect inference, no crash) | KAT oracle with known vectors; start with scalar ref, verify against llama.cpp output byte-for-byte |
| bsums precomputation must match exactly; sign handling of -127 vs -128 matters | Medium | Unit-test quantize_row_q8_K independently before integrating into matmul |
| Q8_0 (existing) vs q8_K (needed) confusion — different block sizes, different purposes | Medium | Name types distinctly in code: `BlockQ8K` (256-elem activation) vs `BlockQ8_0` (32-elem GGUF weight) |
| GPU quantized matmul is less efficient on Gen9 (no native 8-bit MAC) | Low for Phase 9 | Phase 9 targets CPU first; GPU quantized kernel is Phase 9.5+ optimization |
| Integer overflow in accumulation: 32 × (15 × 127) × 63 scale ≈ fits in i32 but tight | Medium | Verify accumulator range analytically; use i32 (not i16) for accumulation as llama.cpp does |

---

## 10. Raw-source citations

| Item | Source path | Lines | Commit |
|------|-----------|-------|--------|
| QK_K, K_SCALE_SIZE constants | `ggml/src/ggml-common.h` | :89-90 | `74ade52` |
| block_q4_K struct | `ggml/src/ggml-common.h` | :317-328 | `74ade52` |
| block_q8_K struct | `ggml/src/ggml-common.h` | :360-366 | `74ade52` |
| AVX2 vec_dot_q4_K_q8_K | `ggml/src/ggml-cpu/arch/x86/quants.c` | :1900-2076 | `74ade52` |
| Scale shuffle table | `ggml/src/ggml-cpu/arch/x86/quants.c` | :527-539 | `74ade52` |
| Scalar vec_dot_q4_K_q8_K ref | `ggml/src/ggml-cpu/quants.c` | :645-718 | `74ade52` |
| quantize_row_q8_K_ref | `ggml/src/ggml-quants.c` | :2696-2733 | `74ade52` |
| quantize_row_q4_K_ref | `ggml/src/ggml-quants.c` | :1399-1469 | `74ade52` |
| Type traits registration | `ggml/src/ggml-cpu/ggml-cpu.c` | :301-310 | `74ade52` |
| Matmul dispatch | `ggml/src/ggml-cpu/ggml-cpu.c` | :1155, :1245, :1313-1348 | `74ade52` |
| Quantization test harness | `tests/test-quantize-fns.cpp` | full file (196 lines) | `74ade52` |
