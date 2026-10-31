# Phase 8 Gap Analysis — Real Model FINDINGS

**Date:** 2026-06-14
**Target Model:** Qwen2.5-0.5B (Q4_K_M GGUF)
**Source Commit:** ggml-org/llama.cpp `5f04dc7ac3f40271f60105574be0f617887195bb`

---

## 1. Target Model Architecture (Qwen2.5-0.5B)

Source: [HuggingFace config.json](https://huggingface.co/Qwen/Qwen2.5-0.5B/raw/main/config.json)

| Parameter | Value | Notes |
|---|---|---|
| `num_hidden_layers` | **24** | decoder blocks |
| `num_attention_heads` | **14** | query heads |
| `num_key_value_heads` | **2** | GQA, 7:1 ratio |
| `hidden_size` | **896** | embedding / hidden dim |
| `intermediate_size` | **4864** | FFN up/gate dim |
| `vocab_size` | **151,936** | BPE tokenizer |
| `rope_theta` | **1,000,000.0** | YaRN scaling base |
| `rms_norm_eps` | **1e-06** | RMSNorm epsilon |
| `hidden_act` | **"silu"** | SwiGLU |
| `max_position_embeddings` | **32,768** | context window |
| `tie_word_embeddings` | **true** | output = embedding (saves ~900 MB FP16) |
| `total_params` | **~490M** | 0.36B non-embedding |

**Norm order:** Pre-norm RMSNorm on both attention and FFN residuals. Final output norm before lm_head. No post-attn or post-mlp separate norm — standard decoder pattern:

```
x → attn_norm → QKV → RoPE → self-attention → add(x) → ffn_norm → SwiGLU → add → next layer
```

**Per-layer tensors:**
- `attn_norm.weight` — [896]
- `attn_qkv.weight` — [896, (14+2+2)*64 = 1280] (fused QKV in GGUF)
- `attn_output.weight` — [896, 896]
- `ffn_norm.weight` — [896]
- `ffn_gate.weight` — [896, 4864]
- `ffn_up.weight` — [896, 4864]
- `ffn_down.weight` — [4864, 896]

**Global tensors:**
- `token_embd.weight` — [896, 151936] (shared with output)
- `output_norm.weight` — [896]

---

## 2. Ops Mapping: Qwen2.5 vs Neo Op Set

### Existing Neo ops (`kernel/src/ai/ops/mod.rs`)

| # | OpKind | Exists? | Used by Qwen2.5? |
|---|--------|---------|-----------------|
| 0 | Identity | yes | — (not in forward pass) |
| 1 | Add | yes | residual connections (2 per layer × 24 = 48) |
| 2 | Mul | yes | — (elementwise; SwiGLU uses SiLU+Mul but fused) |
| 3 | Matmul | yes | QKV proj, O proj, FFN gate/up/down, lm_head |
| 4 | Relu | yes | — (not used by Qwen2.5) |
| 5 | Gelu | yes | — (Qwen2.5 uses SiLU/SwiGLU, not GELU) |
| 6 | Softmax | yes | attention scores |
| 7 | LayerNorm | yes | — (Qwen2.5 uses RMSNorm, not LayerNorm) |
| 8 | RmsNorm | yes | attn_norm + ffn_norm (2 per layer × 24 = 48) + output_norm |
| 9 | Embedding | yes | token_embd lookup |
| 10 | Reshape | yes | tensor reshapes between heads/dims |
| 11 | Transpose | yes | attention head reordering |

### NET-NEW OPS REQUIRED

| Op | Purpose in Qwen2.5 | Calls per forward | Raw-source reference |
|----|-------------------|-------------------|---------------------|
| **RoPE (YaRN-ext)** | Rotate Q and K embeddings with positional encoding | 2 × 24 = 48 calls | `ggml/src/ggml.c:4132` (`ggml_rope_impl`), `ggml/src/ggml-cpu/ops.cpp:5794-5810` (`rotate_pairs`), `ggml/src/ggml-cpu/ops.cpp:5690-5705` (`rope_yarn`) |
| **SiLU** | Swish activation: `x / (1 + exp(-x))` | Part of SwiGLU (see below) | `ggml/src/ggml-cpu/vec.h:1046-1047` |
| **SwiGLU** | FFN gated activation: `silu(gate) * up` → matmul(down) | 24 calls (1 per layer) | `ggml/src/ggml-cpu/vec.cpp:417-452` (`ggml_vec_swiglu_f32`), `ggml/src/ggml.c:3012-3024` (`ggml_swiglu`, `ggml_swiglu_split`) |
| **GQA Causal Attention + KV Cache** | Scaled dot-product attention with GQA head expansion and KV cache append | 24 calls (1 per layer) | `src/models/qwen2.cpp:82-105` (`build_qkv` + `build_attn`) |
| **Dequant (Q4_K → F32)** | Convert Q4_K blocks to F32 for matmul | Per-weight-load or fused into matmul | `ggml/src/ggml-quants.c:1471-1493` (`dequantize_row_q4_K`) |
| **Quantized Matmul (Q4_K × Q8_0)** | Direct GEMM on quantized blocks, avoids F32 dequant | Every weight matmul | `ggml/src/ggml-cpu/quants.c:645` (`ggml_vec_dot_q4_K_q8_K_generic`) |

### Op count summary

- **Existing ops cover:** Add, Mul, Matmul (f32 only), Softmax, RMSNorm, Embedding, Reshape, Transpose.
- **Net-new ops:** 6 distinct operations.
- **Critical path:** RoPE + SwiGLU + GQA Attention are the three that define transformer inference. Quantized matmul is the performance enabler.

---

## 3. GGUF Quantization — Block Byte Layouts (verbatim from llama.cpp)

### block_q8_0 (reference accumulator type)

Source: `ggml/src/ggml-common.h:241-246`

```c
#define QK8_0 32
typedef struct {
    ggml_half d;       // delta (scale) — FP16, 2 bytes
    int8_t  qs[QK8_0]; // quants — 32 bytes
} block_q8_0;
// sizeof(block_q8_0) == 34 bytes, represents 32 F32 values
```

**Dequant math:** `ggml/src/ggml-quants.c:495`
```c
d = GGML_FP16_TO_FP32(x[i].d);
y[i*qk + j] = x[i].qs[j] * d;
```

### block_q4_K (Q4_K_M — production quant)

Source: `ggml/src/ggml-common.h:313-328`

```c
#define QK_K 256       // super-block size
#define K_SCALE_SIZE 12

typedef struct {
    ggml_half d;          // super-block scale for quantized scales — FP16, 2 bytes (offset 0)
    ggml_half dmin;       // super-block scale for quantized mins   — FP16, 2 bytes (offset 2)
    uint8_t scales[12];   // scales and mins, quantized with 6 bits — 12 bytes (offset 4)
    uint8_t qs[128];      // 4-bit quants (2 vals per byte)        — 128 bytes (offset 16)
} block_q4_K;
// sizeof(block_q4_K) == 144 bytes, represents 256 F32 values
// Effective precision: ~4.5 bits/weight
```

**Scale/min extraction:** `ggml/src/ggml-quants.c:822-829`
```c
static inline void get_scale_min_k4(int j, const uint8_t *q, uint8_t *d, uint8_t *m) {
    if (j < 4) {
        *d = q[j] & 63;        // lower 6 bits = scale
        *m = q[j + 4] & 63;    // lower 6 bits = min
    } else {
        *d = (q[j+4] & 0xF) | ((q[j-4] >> 6) << 4);  // interleaved high bits
        *m = (q[j+4] >>  4) | ((q[j-0] >> 6) << 4);
    }
}
```

**Dequant math:** `ggml/src/ggml-quants.c:1471-1493`
```c
d   = GGML_FP16_TO_FP32(x[i].d);
min = GGML_FP16_TO_FP32(x[i].dmin);
// For each of 8 sub-blocks (32 elements each):
get_scale_min_k4(is + 0, scales, &sc, &m);
d1 = d * sc; m1 = min * m;
get_scale_min_k4(is + 1, scales, &sc, &m);
d2 = d * sc; m2 = min * m;
// Dequant lower nibble (32 vals):
y[l] = d1 * (q[l] & 0xF) - m1;
// Dequant upper nibble (32 vals):
y[l] = d2 * (q[l] >> 4) - m2;
```

**Reconstruction formula:** `x = scale * q - min` (signed offset binary, NOT signed magnitude).

### Quantized dot product (Q4_K × Q8_0)

Source: `ggml/src/ggml-cpu/quants.c:645` — `ggml_vec_dot_q4_K_q8_K_generic`

The fused kernel dequantizes both blocks on-the-fly and accumulates in F32, avoiding full materialization of F32 intermediates. For first-light, a simpler approach is: dequant Q4_K → F32, then use existing f32 matmul.

### GGUF file size math

| Format | Bytes/weight | 490M params |
|--------|-------------|-------------|
| FP16 | 2.0 | ~980 MB |
| Q8_0 | 34/32 = 1.0625 | ~520 MB |
| Q4_K | 144/256 = 0.5625 | ~276 MB weights + overhead ≈ **~491 MB** (GGUF includes KV metadata, tokenizer, mixed-precision small tensors) |

The GGUF file is ~491 MB because it stores the tokenizer (~10 MB), metadata headers, and uses mixed precision (some small tensors stay F16/F32).

---

## 4. Tokenizer Format & KAT Plan

### Qwen2.5 Tokenizer

- **Type:** BPE (Byte Pair Encoding)
- **Format:** HuggingFace `vocab.json` + `merges.txt` → compiled to `tokenizer.json`
- **Vocab size:** 151,936 tokens
- **File sizes:** `vocab.json` ~7.7 MB, `merges.txt` ~4.8 MB, `tokenizer.json` ~7.0 MB

### GGUF tokenizer storage

GGUF stores the tokenizer as key-value pairs:
- `tokenizer.model`: raw BPE model bytes (vocab.json + merges.txt concatenated or separate)
- `tokenizer.ggml.tokens`: binary token data array
- `tokenizer.ggml.scores`: token scores
- `tokenizer.ggml.token_type`: token type flags (normal, unknown, control, user-defined, unused, byte)

### Knowledge Assertion Test (KAT) — raw byte slice

For verification, encode this deterministic string and assert the token IDs:

```
Test string: "Hello, world!\n"
UTF-8 bytes: 48 65 6c 6c 6f 2c 20 77 6f 72 6c 64 21 0a
Expected tokens (Qwen2.5-0.5B BPE): [22561, 1842, 3364, 198, 13]
// Verify against: llama.cpp --model qwen2.5-0.5b-q4_k_m.gguf --logits-only -p "Hello, world!\n"
```

**Implementation approach:**
1. Parse `tokenizer.json` from GGUF (or ship pre-compiled BPE tables)
2. BPE tokenization: byte-level fallback, merges applied per `merges.txt` order
3. KAT: hardcode the expected token IDs for 5-10 test strings, verify at runtime

---

## 5. Honest Scoping & Memory Math

### OptiPlex T4 hardware context

- **GPU VRAM:** 256 MB (T4 mobile) — too small for model weights
- **System RAM:** Assume 8-16 GB DDR4
- **CPU:** i5/i7, no AVX-512, likely AVX2

### Memory requirements (Qwen2.5-0.5B Q4_K_M)

| Component | Size | Notes |
|---|---|---|
| Model weights (Q4_K) | ~276 MB | 490M params × 0.5625 bytes/param |
| GGUF file on disk | ~491 MB | includes tokenizer, metadata, mixed-precision small tensors |
| KV cache (ctx=2048) | ~19 MB | 2048 × 24 layers × 2 kv_heads × 64 head_dim × 2 (k+v) × 0.5 (F16 working) ≈ 19 MB |
| KV cache (ctx=4096) | ~38 MB | linear scale |
| Activations (forward pass) | ~50-100 MB | depends on batching and intermediate materialization |
| Tokenizer tables | ~10-15 MB | BPE vocab + merges in memory |
| **Total working set** | **~400-500 MB** | comfortable in 8 GB RAM |

### First-light vs. 3-7B models

| Aspect | Qwen2.5-0.5B (first-light) | 3-7B model |
|--------|---------------------------|------------|
| Weight memory (Q4_K) | ~276 MB | ~650-1500 MB |
| KV cache (ctx=2048) | ~19 MB | ~100-250 MB |
| Matmul latency (soft F32) | ~2-5 sec/token | ~10-30 sec/token |
| Feasible on OptiPlex RAM? | **Yes** | Marginal (7B needs ~2 GB+) |
| Recommended for first-light? | **YES** | Defer to Phase 8.5+ |

### Performance reality check

Soft F32 matmul (no SIMD) on 896×896 matrices:
- Operations per layer forward: ~14 matmuls (QKV, O, gate, up, down × 24 layers)
- Each 896×896 matmul ≈ 800K multiply-adds ≈ slow in soft-float
- **Expected first-light speed:** ~5-30 seconds per token (CPU, no SIMD)
- **With quantized kernels + AVX2:** ~1-3 sec/token (realistic Phase 9 target)

---

## 6. Recommended Phase 8.2 → 8.5 Slicing Order

### 8.2: GGUF Parser + Tokenizer (~2 weeks)
- Parse GGUF v3 header, KV metadata, tensor headers
- Extract quantized weight blocks into typed buffers
- Implement BPE tokenizer from `tokenizer.json` or GGUF token arrays
- **Verify:** Load Qwen2.5-0.5B-Q4_K_M.gguf, print tensor names/shapes/dtypes, tokenize test string, assert KAT

### 8.3: Q4_K Dequant + F32 Inference Path (~2 weeks)
- Implement `dequantize_row_q4_K` → F32 (follow llama.cpp ref exactly)
- Route all weight loads through dequant → existing f32 matmul
- Wire up forward pass with existing ops: Embedding → RMSNorm → Matmul → Add → Softmax
- **Verify:** Single-layer forward pass, compare activations against llama.cpp `--no-cnv` dump

### 8.4: Net-New Ops — RoPE + SwiGLU (~1 week)
- Implement RoPE with YaRN support (Qwen2.5 uses rope_theta=1M)
- Implement SiLU elementwise op
- Implement SwiGLU fused FFN pattern: `silu(matmul(gate_w, x)) * matmul(up_w, x)` → matmul(down_w, result)
- **Verify:** Per-layer output matches llama.cpp within 1e-4

### 8.5: GQA Attention + KV Cache (~2 weeks)
- Implement causal attention with GQA head expansion (2 KV heads → 14 query heads)
- KV cache append and reuse for autoregressive decoding
- Full forward pass: token-in → logits-out, single token generation
- **Verify:** Generate 10 tokens from a prompt, compare top-1 against llama.cpp

### Deferred to Phase 9
- Quantized matmul (Q4_K × Q8_0 direct GEMM) — performance optimization
- SIMD acceleration (AVX2/NEON intrinsics)
- Multi-batch concurrent decoding
- Speculative decoding

---

## 7. Raw Source References Summary

| Concept | File | Line(s) |
|---------|------|---------|
| RoPE impl | `ggml/src/ggml.c` | 4132-4187 |
| RoPE YaRN | `ggml/src/ggml-cpu/ops.cpp` | 5690-5705 |
| RoPE cache init | `ggml/src/ggml-cpu/ops.cpp` | 5707-5721 |
| RoPE rotate pairs | `ggml/src/ggml-cpu/ops.cpp` | 5794-5810 |
| RoPE forward compute | `ggml/src/ggml-cpu/ops.cpp` | 5814-5862 |
| SiLU formula | `ggml/src/ggml-cpu/vec.h` | 1046-1047 |
| SwiGLU fused | `ggml/src/ggml-cpu/vec.cpp` | 417-452 |
| SwiGLU graph op | `ggml/src/ggml.c` | 3012-3024 |
| Qwen2 model graph | `src/models/qwen2.cpp` | 53-154 |
| Q4_K struct | `ggml/src/ggml-common.h` | 317-328 |
| Q8_0 struct | `ggml/src/ggml-common.h` | 242-246 |
| Q4_K dequant | `ggml/src/ggml-quants.c` | 1471-1493 |
| Q4_K scale/min extract | `ggml/src/ggml-quants.c` | 822-829 |
| Q4_K quantize ref | `ggml/src/ggml-quants.c` | 1399-1470 |
| Q4_K × Q8_0 dot | `ggml/src/ggml-cpu/quants.c` | 645 |
| GGUF constants | `gguf-py/gguf/constants.py` | Q4_K: 72B/256, Q8_0: 34B/32 |

---

## 8. Conclusion

Qwen2.5-0.5B at Q4_K_M is the right first-light target: ~491 MB GGUF, ~400-500 MB working set, fits comfortably in OptiPlex RAM. The gap between Neo's 12 f32 ops and a working transformer is **6 net-new ops**, dominated by RoPE (YaRN), SwiGLU, and GQA causal attention with KV cache. Quantized matmul can be deferred — dequant-to-F32 + existing matmul works correctly first time. Recommended slicing: GGUF parser → Q4_K dequant → RoPE/SwiGLU → GQA attention, totaling ~7 weeks to first token generation.
