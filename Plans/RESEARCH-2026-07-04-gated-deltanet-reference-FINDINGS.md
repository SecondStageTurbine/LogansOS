<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gated DeltaNet reference implementation audit — Q1 verdict for Qwen 3.5 2B candidate

**Date:** 2026-07-04
**Lane:** Prometheus (deep prior-art mining — NO kernel code)
**Delivers for:** OT-38 next-brain strategy (`THINK-2026-07-03-next-brain-qwen35-deltanet-candidate.md`) — resolves gating question Q1
**Sources:** ggml-org/llama.cpp `src/models/delta-net-base.cpp`, `ggml/src/ggml-cuda/gated_delta_net.cu`, `src/llama-arch.h`; Hugging Face `transformers/models/qwen3_5/modeling_qwen3_5.py` (`Qwen3_5GatedDeltaNet`, `torch_recurrent_gated_delta_rule`, `torch_chunk_gated_delta_rule`); vLLM `vllm/model_executor/layers/mamba/gdn/qwen_gdn_linear_attn.py` (`QwenGatedDeltaNetAttention`); FLA (flash-linear-attention) library referenced by both HF and vLLM

---

## 0. Executive summary — Q1 verdict: **YES**

**A correct, trustworthy reference implementation of Gated DeltaNet exists to anchor our OT-30 parity oracle.** Three independent references implement the full forward correctly:

| Reference | Correct? | Anchor quality | Notes |
|-----------|----------|---------------|-------|
| **HF `transformers` (Python/PyTorch)** | **YES — canonical reference** | BEST for parity oracle | Self-contained `torch_recurrent_gated_delta_rule` is the purest reference. No CUDA kernels, no FLA dependency. Pure PyTorch, easy to port to Rust. |
| **llama.cpp / GGML (C/CUDA)** | **YES — full implementation** | Good for GGUF import path | `GGML_OP_GATED_DELTA_NET` is a first-class op. CPU autoregressive + chunking paths in `delta-net-base.cpp`. CUDA kernel in `gated_delta_net.cu`. Not the Gemma-4-PLE trap. |
| **vLLM (Python/CUDA/Triton)** | **YES — production inference** | Good for cross-check | `QwenGatedDeltaNetAttention` with FLA kernels. Production-grade, used in deployment. |

**This is NOT the Gemma-4-PLE trap.** The delta-rule state recurrence + gating is implemented in all three references. No stubbing, no partial implementation, no "runs but the mechanism is never applied."

---

## 1. Per-reference audit (ISC-1)

### 1.1 Hugging Face `transformers` — the canonical reference

**Source:** `transformers/models/qwen3_5/modeling_qwen3_5.py`

**Status: CORRECT — best parity anchor.**

The authoritative forward is in `Qwen3_5GatedDeltaNet.forward()`. The layer type schedule (`config.layer_types[layer_idx]`) selects `"linear_attention"` or `"full_attention"` per layer. For linear-attention layers, the GDN forward runs:

**The recurrent (autoregressive) path — pure PyTorch reference:**

```python
def torch_recurrent_gated_delta_rule(query, key, value, g, beta, initial_state, ...):
    for i in range(sequence_length):
        q_t, k_t, v_t, g_t, beta_t = extract_token_i(...)
        # Gate: S = g * S
        last_recurrent_state = last_recurrent_state * g_t
        # kv = S^T @ k
        kv_mem = (last_recurrent_state * k_t.unsqueeze(-1)).sum(dim=-2)
        # delta = (v - kv) * beta
        delta = (v_t - kv_mem) * beta_t
        # S = S + k * delta^T  (outer product update)
        last_recurrent_state = last_recurrent_state + k_t.unsqueeze(-1) * delta.unsqueeze(-2)
        # output = S^T @ q
        core_attn_out[:, :, i] = (last_recurrent_state * q_t.unsqueeze(-1)).sum(dim=-2)
```

This is the exact Gated Delta Rule from Yang et al. The state recurrence is:
1. **Gate:** `S = exp(a) * S` (state decay modulated by per-token gate `a`)
2. **Key-value memory:** `kv = S^T @ k` (project key through state)
3. **Delta:** `delta = (v - kv) * sigmoid(b)` (learning rate modulated by beta)
4. **State update:** `S = S + k * delta^T` (outer product: rank-1 update)
5. **Output:** `o = S^T @ q` (query projection through updated state)

**Why this is the best anchor for our parity oracle:**
- Pure PyTorch — no CUDA kernels, no FLA dependency, no fused ops
- Self-contained function — can be run independently of the model class
- Runs in float32 internally — maximum precision reference
- The chunked path (`torch_chunk_gated_delta_rule`) is provided for prefill but the recurrent path is the canonical single-token decode
- Easy to port to Rust for a host-side KAT

**Additional components in the GDN layer:**
- Causal conv1d on the QKV projection (depthwise, SiLU activation)
- L2 normalization of Q and K (`l2norm`) before the recurrence
- `RMSNormGated` output norm with SiLU gate from `z` projection
- Conv state + recurrent state caching for incremental decode

**Verdict:** Correct, complete, self-contained. This is our parity anchor.

### 1.2 llama.cpp / GGML — full implementation (NOT the Gemma-4 trap)

**Source:** `src/models/delta-net-base.cpp`, `ggml/src/ggml-cuda/gated_delta_net.cu`

**Status: CORRECT — full implementation, all paths.**

**GGML op registration:** `GGML_OP_GATED_DELTA_NET` is a first-class GGML tensor operation.

**Architecture registration:** `LLM_ARCH_QWEN35` and `LLM_ARCH_QWEN35MOE` are dedicated arch types in `llama-arch.h`. Dedicated tensor `ssm_alpha` for Qwen 3.5.

**CPU autoregressive path (`build_delta_net_autoregressive`):**
```cpp
// Gate: S = g * S
g = ggml_exp(ctx0, g);
s = ggml_mul(ctx0, s, g);

// kv = S^T @ k (sum over S_k dim)
sk = ggml_mul(ctx0, s, k);
sk = ggml_sum_rows(ctx0, sk);

// delta = (v - kv^T) * beta
d = ggml_sub(ctx0, v, ggml_transpose(ctx0, sk));
d = ggml_mul(ctx0, d, b);

// S = g*S + k * delta^T (outer product update)
kd = ggml_mul(ctx0, k, d_t);
s = ggml_add(ctx0, s, kd);

// output = S^T @ q
o = ggml_sum_rows(ctx0, ggml_mul(ctx0, s, q));
```

This matches the HF reference exactly: gate → kv-memory → delta → state update → query projection. Both KDA (per-dimension gate) and GDA (scalar gate) variants are supported.

**CPU chunking path (`build_delta_net_chunking`):** Full parallel prefill implementation following Yang et al. "Parallelizing Linear Transformers with the Delta Rule." Implements chunk decomposition, decay masks via cumulative gate sums, lower-triangular solve for intra-chunk attention, and cross-chunk state propagation.

**CUDA kernel (`gated_delta_net.cu`):** Token-sequential recurrence on GPU. Specialized for S_v = 16, 32, 64, 128. Supports `keep_rs` for MTP rollback (recurrent state snapshots). Both scalar and KDA gate paths.

**Active development:** PR #24561 adds chunked CUDA prefill (48/48 GATED_DELTA_NET test cases pass, max abs diff ~1.2e-4 vs per-token kernel). Issue #22384 fixed context caching for multi-turn.

**No TODOs, no stubs.** The implementation is production-grade with active optimization.

**Verdict:** Correct and complete. Not the Gemma-4-PLE trap. llama.cpp implements Gated DeltaNet properly — the state recurrence and gating are in the forward graph.

### 1.3 vLLM — production inference engine

**Source:** `vllm/model_executor/layers/mamba/gdn/qwen_gdn_linear_attn.py`

**Status: CORRECT — production-grade.**

`QwenGatedDeltaNetAttention` extends the base `GatedDeltaNetAttention` with Qwen-specific weight layouts. Three backend paths:
- **Triton/FLA** (default): Uses `fla_chunk_gated_delta_rule` for prefill, `fused_recurrent_gated_delta_rule_packed_decode` for decode
- **FlashInfer** (Hopper/Blackwell): `fi_chunk_gated_delta_rule`
- **CuteDSL** (Blackwell, opt-in)

The layer handles both Qwen3-Next (interleaved GQA layout) and Qwen3.5 (non-interleaved `[q, k, v, z]` layout). Full conv state + SSM state caching via `MambaStateShapeCalculator.gated_delta_net_state_shape`.

**Verdict:** Correct. Production deployment engine, used for serving Qwen 3.5 models.

### 1.4 FLA (flash-linear-attention) — the shared kernel library

Both HF transformers and vLLM delegate to FLA (`fla.ops.gated_delta_rule`) for the fast paths:
- `chunk_gated_delta_rule` — parallel prefill
- `fused_recurrent_gated_delta_rule` — autoregressive decode

HF falls back to pure PyTorch when FLA is unavailable. This confirms FLA is the canonical kernel implementation, with HF's pure-PyTorch as the reference fallback.

---

## 2. GGUF / quantization path

**GGUF support:** llama.cpp has `LLM_ARCH_QWEN35` registered, meaning GGUF files for Qwen 3.5 can be loaded and the GDN forward will execute through the GGML graph. The `ssm_alpha` tensor is mapped in `llama-model.h`.

**Quantization:** GGUF quantization (Q4_K_M, Q8_0, etc.) works on the linear projection weights. The GDN state matrices are runtime-allocated and not quantized (they're recurrent state, not stored weights). This is correct — you can't quantize a recurrent state without breaking the delta-rule math.

**No Gemma-4-style degradation:** Unlike Gemma 4's PLE (which was never injected into the forward), GDN's state recurrence IS in the forward graph. Quantized GGUF files will produce correct (if slightly lower-precision) output, not subtly-wrong output from a missing mechanism.

**Verdict:** GGUF path is viable for import → `.neom` conversion. The quantization round-trip preserves the GDN mechanism.

---

## 3. Parity anchor strategy (ISC-2)

### 3.1 The reference: HF `torch_recurrent_gated_delta_rule`

This is our parity anchor. Reasons:
1. **Pure PyTorch** — no external kernel dependencies, runs on CPU
2. **Float32 internal** — maximum precision reference
3. **Self-contained** — single function, easy to extract and run independently
4. **Matches the paper** — implements Yang et al.'s delta rule exactly

### 3.2 The parity protocol

For each GDN layer:
1. Run HF `torch_recurrent_gated_delta_rule` on a fixed input → capture output bytes (float32)
2. Run our Rust GDN forward on the same input → capture output bytes
3. **Byte-identical** = pass. Any difference = implementation bug.

The recurrent state `S` must also be compared at each token step (not just the final output), because a state divergence at token T will propagate through all subsequent tokens.

### 3.3 What's missing and is it fixable?

**Nothing is missing.** The reference exists, is correct, and is self-contained. This is NOT the Gemma-4 situation. The parity path is:
1. Port `torch_recurrent_gated_delta_rule` to Rust (host KAT)
2. Verify against HF on known inputs
3. Use as the OT-30 anchor for the kernel forward

### 3.4 New-op byte-oracles (Q4 from the candidate THINK)

Each new op needs its own oracle:
- **GDN recurrent step** (gate → kv → delta → state update → output): primary oracle via HF reference
- **Causal conv1d** with SiLU: oracle via `torch_causal_conv1d_update` (already in HF source)
- **L2 normalization** of Q/K: oracle via HF `l2norm()` function
- **RMSNormGated** output norm: oracle via HF `Qwen3_5RMSNormGated`

All four are self-contained in the HF source. No external dependency needed.

---

## 4. Updated recommendation (ISC-3)

### 4.1 Q1 answer: YES — correct reference exists

The gating question is resolved. A correct, trustworthy reference implementation of Gated DeltaNet exists in three independent codebases. The HF PyTorch reference is the cleanest parity anchor.

### 4.2 Verdict revision

**From:** "Strong eventual candidate — NOT the next step — gated on Q1"
**To:** "Strong eventual candidate — Q1 RESOLVED (YES) — viable pending OT-16 appetite"

The candidate moves from "parked until Q1 is answerable" to "ready for OT-16 + build planning." The remaining gates are:
- **Q2 (OT-16 appetite):** Do we implement the data-driven forward-graph generalization? GDN requires a new ring-0 forward, which triggers OT-16. Without OT-16, hardcoding a second forward is the anti-pattern.
- **Q3 (VLM scope):** Run text-only for Arthur, or is multimodal in scope?
- **Q5 (quant/import):** GGUF path is viable (see §2).

### 4.3 What this means for OT-38

The OT-38 next-brain strategy remains: **Qwen2.5 1.5B/3B config-swap for the immediate step** (proven forward, zero architecture risk). But Qwen 3.5 2B is now a **viable Phase-9-scale build** — the reference anchor exists, the GGUF import path works, and the parity protocol is defined. The decision to pursue it depends on OT-16 appetite and capacity, not on the existence of a correct reference.

---

## 5. Summary table

| Source | DeltaNet in forward? | Gating implemented? | State recurrence? | Parity anchor viable? |
|--------|---------------------|-------------------|------------------|----------------------|
| **HF `transformers`** | YES — `Qwen3_5GatedDeltaNet` | YES — `g = -A_log.exp() * softplus(a + dt_bias)` | YES — `torch_recurrent_gated_delta_rule` | **YES — best anchor** |
| **llama.cpp / GGML** | YES — `GGML_OP_GATED_DELTA_NET` | YES — KDA + GDA variants | YES — `build_delta_net_autoregressive` + CUDA kernel | YES — for GGUF import verification |
| **vLLM** | YES — `QwenGatedDeltaNetAttention` | YES — FLA kernels | YES — recurrent + chunked paths | YES — cross-check |
| **FLA library** | YES — `chunk_gated_delta_rule`, `fused_recurrent_gated_delta_rule` | YES | YES | Shared kernel backend |

**Q1: Is there a correct reference implementation of Gated DeltaNet to anchor our OT-30 parity oracle against?**

**ANSWER: YES.** The HF `torch_recurrent_gated_delta_rule` function is the canonical parity anchor. llama.cpp's GGML implementation confirms the GGUF import path is viable. This is NOT the Gemma-4-PLE trap.

---

*This is a research document. No kernel code was modified. All claims about reference implementations are based on reading the actual source files (modeling_qwen3_5.py, delta-net-base.cpp, gated_delta_net.cu, qwen_gdn_linear_attn.py), not summaries.*
