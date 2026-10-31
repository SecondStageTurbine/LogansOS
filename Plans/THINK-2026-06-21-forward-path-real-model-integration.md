<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Forward-path real-model integration — gap analysis + scoping

**Date:** 2026-06-21
**Author:** Prometheus (Qwen-27B — research/design seat)
**Branch:** prometheus/forward-path-real-model-integration-design
**Dispatch:** DISPATCH-2026-06-21-forward-path-real-model-integration-design-prometheus.md
**Scope:** Design THINK only. No code, no ci.yml. Builds (F1-F3) dispatch to a main-tier coder post-ratification.

---

## 0. The problem

L1 produced a real Qwen2.5-0.5B .neom (485 MB, validates). L2 lets NeoFS store/serve it. But the kernel forward path and Arthur's tokenizer were built for **fixtures**. A PM code audit found three concrete gaps that block running the *real* model on silicon:

1. The forward matmul is **q4_K-only** but the real model has 5 quant types (mostly q5_0/q6_K).
2. The model config is **hardcoded to 0.5B consts** — cannot run 1.5B or any other variant.
3. Arthur uses a **7-token fixture vocab** — needs the real 151,936-token sidecar.

This THINK resolves D1-D6 and produces a ratifiable F1/F2/F3 slice plan.

---

## 1. References (read before this THINK)

| File | Relevance |
|------|-----------|
| kernel/src/ai/forward_logic.rs | Qwen2 graph: hardcoded QWEN25_05B_* consts; matmul_q4_k_q8_k is the only quant path. Qwen2LayerConfig struct drives shapes. |
| kernel/src/ai/gguf_dequant_logic.rs | LANDED dequant primitives for q4_K, q5_0, q6_K, q8_0 to f32. Exist but the forward matmul does not call them. |
| kernel/src/ai/ops/qk_matmul.rs | q4_K x q8_K matmul + q8_K activation quantization. Scalar + AVX2 variants. |
| kernel/src/ai/generate_logic.rs | Generation loop: seeded sampling, logits slicing, PCG32 RNG. Model-agnostic. |
| userland/neo-arthur/src/main.rs | BPE_VOCAB — the hardcoded FIXTURE vocab (7 tokens). SyscallBackend::tokenize() / decode() use fixture tables. |
| userland/neo-arthur/src/conversation_logic.rs | ConversationBackend trait with tokenize(), generate(), decode(). The seam. |
| kernel/src/ai/tokenizer_logic.rs | BPE tokenizer that accepts raw-source vocab + merge slices — built for the real tables. |
| tools/neomc/src/gguf_import.rs | L1 GGUF to .neom importer. 291 tensors (f32=121 q4_K=12 q5_0=133 q6_K=12 q8_0=13), vocab sidecar (151,936 tokens / 151,387 merges). |

**L1 proof — exact target:** 291 tensors. Vocab: 151,936 tokens, 151,387 merges. Model size: 485 MB (0.5B) / ~1.1-1.3 GB (1.5B).

---

## 2. D1 — Mixed-quant weights in the forward matmul (THE CRUX)

### The gap

forward_logic.rs::matmul_vec_weight() has two paths:
- **Feature-gated quantized path:** matmul_q4_k_q8_k — only handles q4_K weight blocks.
- **f32 fallback:** plain f32 matmul via ops::matmul.

The real model's weight tensors are: **q5_0 (133), q6_K (12), q8_0 (13), q4_K (12), f32 (121)**. Only 12 of 291 tensors are q4_K. The quantized matmul path covers ~4% of the weight tensors.

### Tensor type distribution by layer role

| Role | Count | Typical types (0.5B) |
|------|-------|---------------------|
| Token embedding | 1 | f32 or quantized |
| Output projection | 1 | q4_K/q5_0/q6_K |
| Output bias | 1 | f32 |
| Attn norm (24 layers) | 24 | f32 |
| FFN norm (24 layers) | 24 | f32 |
| Q/K/V projections (72 total) | ~60+ | **q5_0** (hot path) |
| Q/K/V biases (72 total) | 72 | f32 |
| Attn output projection (24 layers) | 24 | q5_0/q6_K |
| FFN gate/up (48 total) | ~36+ | **q5_0** (hot path) |
| FFN down (24 layers) | 24 | q5_0/q6_K |

**Hot path:** Q/K/V projections and FFN gate/up/down are the compute-dominant matmuls. These are **q5_0** in the real model. q6_K covers 12 tensors (likely output/head layers).

### Option A: Per-tensor dequant-to-f32, then f32 matmul (RECOMMENDED)

**Mechanism:** For each quantized weight tensor, call gguf_dequant_logic::dequantize_tensor_data_to_f32() to produce an f32 slice, then feed it to the existing f32 matmul_vec() path. Dequant is **per-matmul, streaming** — dequant one weight tensor at a time, use it for the matmul, drop the f32 buffer before the next tensor.

**Rationale:**
- **Correctness on silicon first.** The dequant primitives are landed, tested, and produce bit-correct f32. The f32 matmul path is proven. No new kernel to byte-oracle.
- **Minimal forward_logic change.** matmul_vec_weight() gains a rhs_dtype parameter and a dequant dispatch before the f32 matmul.
- **Memory-safe streaming.** Largest weight is ffn_gate.weight = 896x4864 = ~4.3M elements = 17.5 MB f32. Tiny vs 485 MB model. Buffer freed before next matmul.
- **No new byte-oracle.** Reusing primitives that already have host KATs.

**Cost:** Dequant is ~5-10% of total per-token latency. Working-set bound: <20 MB transient.

### Option B: Native q5_0/q6_K/q8_0 x q8_K matmul kernels (REJECTED for now)

**Rationale for rejection:**
- **3 new kernels x 2 variants = 6 kernels to write and byte-oracle.** Weeks of main-coder work.
- **Delays silicon correctness.** Option A gives a working real-model forward path in one slice.
- **Performance is not the bottleneck yet.** D5 shows 0.4s/token for 0.5B. Dequant overhead is invisible next to that. Native kernels are a later perf slice (F4).

### Decision: Option A (dequant-to-f32 streaming)

**Implementation sketch (no code):**
1. Add TensorDtype enum: F32, Q4_K, Q5_0, Q6_K, Q8_0.
2. Extend Qwen2LayerWeights to carry dtype for each weight field.
3. In matmul_vec_weight(), dispatch on rhs dtype: all quant types dequant via gguf_dequant_logic then f32 matmul.
4. Model loader reads tensor dtype from .neom inventory and populates weights struct.

**Residual risk:** LOW. Dequant to f32 is numerically identical to CPU dequant (scalar f32 primitives). No precision loss beyond quantization.

---

## 3. D2 — Model config: data-driven from .neom

### The gap

forward_logic.rs hardcodes 8 consts for Qwen2.5-0.5B (hidden=896, intermediate=4864, layers=24, q_heads=14, kv_heads=2, head_dim=64, context=4096, rope_theta_scale=0.64938164). Cannot run 1.5B (hidden=1536, intermediate=8192, layers=28, q_heads=12, kv_heads=4, head_dim=128) without recompiling.

### Decision: Two-source approach

1. **Tensor shapes from .neom inventory** (already there): Derive hidden_size, q_len, kv_len, intermediate_size from tensor dimensions. token_embd.weight dims are [hidden, vocab]. blk.0.attn_q.weight dims are [hidden, q_heads x head_dim].
2. **Scalar metadata from .neom header extension** (L1 dependency): layer_count, rope_theta, rms_norm_eps not derivable from shapes. **L1 note:** importer must write these 3 values into .neom header (~24-byte addition). Not a build in this THINK.

**Where config is consumed:** Qwen2LayerConfig drives attention_seam(), swiglu_ffn(), gqa_attention_*, Qwen2KvCacheSet::new(), qwen2_output_logits(). Forward graph topology does not change — only the **values** come from the model object.

### Implementation sketch (no code)

1. Add ModelConfig struct with all shape fields.
2. Kernel model loader constructs ModelConfig from .neom at load time.
3. Replace Qwen2LayerConfig::qwen25_05b() with from_model_config().
4. Hardcoded consts remain as fixture defaults for arthur_boots_thinking.

**Boundary vs OT-16:** Config-from-.neom only. Graph topology unchanged. OT-16 frozen.

**Residual risk:** LOW. Shapes are a cross-check; header metadata is the primary source.

---

## 4. D3 — Real vocab/tokenizer wiring

### The gap

neo-arthur/src/main.rs has hardcoded BPE_VOCAB (7 tokens) and BPE_MERGES (10 entries). Real model: 151,936 tokens, 151,387 merges.

### Decision: Load sidecar at Arthur startup into owned tables

**Mechanism:**
1. Read vocab sidecar from NeoFS at startup. Parse into Vec<BpeVocabEntry> + Vec<BpeMergeEntry>.
2. Build BpeTokenizer::from_slices() at startup. Immutable after construction.
3. Replace fixture references in SyscallBackend::tokenize() / decode().
4. **Qwen chat special tokens:** sidecar carries bos/eos/pad IDs. Conversation logic wraps each turn with chat template before tokenizing.

**Memory cost:** ~25 MB for tokenizer state (one-time startup).

**Kill-switch:** Feature arthur-real-vocab. Off = fixture bit-for-bit. On = load sidecar, fall back to fixture if unreadable.

**Residual risk:** MEDIUM (perf). BPE apply_merges() is O(n^2) for 151K merges. Acceptable for Stage-B short conversations. Faster merge is a later perf slice.

---

## 5. D4 — Memory/residency at real scale

| Component | Fixture | Real 0.5B | Real 1.5B |
|-----------|---------|-----------|-----------|
| .neom file | ~4 KB | 485 MB | ~1.2 GB |
| KV cache (ctx=4096) | ~1 KB | ~192 MB | ~448 MB |
| Tokenizer | ~1 KB | ~25 MB | ~25 MB |
| Dequant transient | N/A | <20 MB | <40 MB |

**Peak resident:** 0.5B = ~700 MB. 1.5B = ~1.7 GB (tight on 2 GiB cap). /model fallback to 0.5B is the escape hatch.

**No blockers.**

---

## 6. D5 — Latency honesty (Stage-B expectations)

### Assumptions

- CPU: OptiPlex 7910, i5-10505, 4.5 GHz turbo. Native scalar f32 (SSE always available on x86_64). soft_f32 only for exp() in softmax. ~1.5 GFLOPS single-threaded.
- Dequant: ~5-10% overhead.

### Honest latency table

| Model | Per-token FLOPs | Est. latency | With dequant |
|-------|----------------|-------------|-------------|
| 0.5B | ~527M | ~0.35s | **~0.4s/token** |
| 1.5B | ~1.55B | ~1.0s | **~1.1s/token** |

**Conservative (with scheduler, memory BW, softmax overhead): 0.5-1.5s/token for 0.5B, 1-2s/token for 1.5B.** A 50-token reply: 25-75s (0.5B), 50-100s (1.5B). Justifies /model fallback.

---

## 7. D6 — Slice plan + kill-switch

### Invariant: The fixture path stays green

Every slice is **kill-switched default-off**. When all three features are disabled, arthur_boots_thinking runs bit-for-bit identically to today. Non-negotiable.

### F1: Mixed-quant forward (kernel) — THE CRUX

**Changes:** forward_logic.rs + model loader glue. Dtype dispatch in matmul_vec_weight(). Wire gguf_dequant_logic for all quant types. Extend Qwen2LayerWeights with per-tensor dtype.
**Kill-switch:** Feature phase9-real-quant (default-off).
**Gate:** Host KAT: dequant each type to f32 matmul vs hand-laid oracle. QEMU: fixture green off; arthur_real_model_05b scenario produces sane token on.
**Dispatch to:** Main-tier coder. Kernel forward-path work.

### F2: Config from .neom (kernel)

**Changes:** forward_logic.rs. ModelConfig struct, derive from .neom tensor shapes + header metadata. Replace hardcoded consts. Fixture consts remain as default fallback.
**Kill-switch:** Feature phase9-data-driven-config (default-off).
**Dependency:** L1 importer writes layer_count, rope_theta, rms_norm_eps to .neom header. Note to L1 dispatcher.
**Gate:** Load 0.5B .neom, derive config, assert matches hardcoded consts. Load 1.5B, assert correct values.
**Dispatch to:** Main-tier coder. Depends on F1 (shares forward_logic.rs).

### F3: Real-vocab tokenizer (userland)

**Changes:** neo-arthur/src/main.rs. Load vocab sidecar at startup. Build BpeTokenizer from real tables. Wire into SyscallBackend. Add Qwen chat template framing in conversation_logic.rs.
**Kill-switch:** Feature arthur-real-vocab (default-off).
**Gate:** Tokenize/decode round-trip on known tokens. Chat template framing: verify bos/eos present.
**Dispatch to:** Main-tier coder. Userland, independent of kernel slices.

### Slice dependency order

`
F1 (mixed-quant forward) --> F2 (config from .neom) --> end-state scenario
                                                     (real .neom produces sane token)
F3 (real-vocab tokenizer) -+  (userland, parallel with F1/F2)
`

F1 and F3 are independent. F2 depends on F1's dtype infrastructure. All three enabled simultaneously for end-state: Arthur loads real .neom, reads config, dequantizes mixed-quant weights through f32, tokenizes with real vocab, produces coherent reply.

---

## 8. Honest uncertainties table

| Uncertainty | Impact | Resolution path |
|-------------|--------|-----------------|
| D1: Dequant streaming latency on bare metal (QEMU memory BW differs) | Could add 20-50% to D5 estimates | Measure on first bare-metal run; if >2s/token for 0.5B, consider F4 sooner |
| D2: .neom header metadata extension — L1 importer timing | Blocks F2 build | L1 note dispatched in parallel; F2 coder can stub metadata for KAT |
| D3: BPE merge performance on long inputs | Tokenization could dominate for >512-token prompts | Acceptable for Stage-B. Monitor; fast-merge slice if needed |
| D5: soft_f32::exp() in softmax — cycle count on Comet Lake | Softmax over 151K logits = ~151K exp calls | Profile on first bare-metal run. If dominates, use native SSE for exp |
| KV cache at full context for 1.5B | ~448 MB + 1.2 GB model = tight on 2 GiB cap | /model fallback to 0.5B is the escape hatch |

---

## 9. Prior-art note

### llama.cpp / ggml mixed-quant matmul

llama.cpp maintains a library of quantized dot-product kernels for every type pair (vec_dot_q4_0_q8_0, vec_dot_q5_0_q8_0, vec_dot_q6_K_q8_0, etc.). Each kernel dequantizes in-register during the dot product — no f32 materialization. This is Option B: fast, memory-lean, but requires ~40 kernels across architectures. We have 1 kernel for 1 type pair on 2 SIMD targets. **Option A (dequant to f32) is what llama.cpp's ggml_mul_mat does when a quantized kernel is unavailable** — it falls back to dequant then f32 matmul. Our choice matches llama.cpp's fallback path, which is correct and well-tested.

### Qwen2 chat template

Raw source: HuggingFace Qwen2.5-Instruct tokenizer_config.json. Chat template uses special tokens (bos_token_id / eos_token_id from GGUF metadata). The L1 sidecar exports these IDs. Arthur's conversation logic wraps each turn with the chat template before tokenizing. These are single tokens in the vocab, not byte-level composition.

---

## 10. Summary

**D1:** Dequant-to-f32 streaming (Option A). Reuses landed primitives, minimal change, correctness-first. Native kernels deferred to post-silicon perf slice.
**D2:** Config from .neom tensor shapes + header metadata. Two-source derivation. L1 note for 3 missing metadata keys. OT-16 frozen.
**D3:** Load real vocab sidecar at Arthur startup. Wire through tokenizer_logic. Add Qwen chat template framing. Userland-only, independent of kernel slices.
**D4:** No blockers. 0.5B fits (~700 MB peak). 1.5B tight but feasible (~1.7 GB peak) with /model fallback.
**D5:** ~0.4s/token (0.5B), ~1.1s/token (1.5B) scalar f32. Conservative: 0.5-2s/token. Honest, no over-claim.
**D6:** F1 (kernel) -> F2 (kernel), F3 (userland) parallel. All kill-switched default-off. Fixture stays green. End-state: real .neom produces a sane token.
