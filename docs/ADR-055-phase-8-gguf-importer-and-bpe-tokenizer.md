<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-055: Phase 8 BUILD-8.2 — GGUF importer and BPE tokenizer

**Status:** Accepted — BUILD-8.2 implementation.
**Date:** 2026-06-15
**Dispatch:** `Plans/DISPATCH-2026-06-15-phase-8-build-8.2-gguf-importer.md`
**Design:** `Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md` and `Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md`
**Builds on:** ADR-028 (`.neom` v1), ADR-053 (decode loop), ANSPEC-001 §11.2.
**Ledger:** no new invariant, no new lock, no new `unsafe`; feature-gated by `gguf`.
**Amended:** 2026-06-16 — BUILD-8.3 integration wires imported GGUF tensor
bytes through Q4_K/Q8_0 dequant into the existing f32 matmul path.
**Amended:** 2026-06-16 — BUILD-8.4 wires the dequantized real-weight path
through RoPE on projected Q/K and the SwiGLU FFN layer scaffold, with attention
left as an explicit unsupported seam for BUILD-8.5.
**Amended:** 2026-06-17 — the Phase 8 Rule-4 consolidation keeps BUILD-8.5
behavior unchanged while factoring the shared KV-cache layout convention and
promoting workspace/kernel clippy plus no_std userland hygiene to blocking CI
gates (ADR-056).

## The Decision

Add a default-off GGUF importer and byte-level BPE tokenizer for Phase 8 real
model loading.

`.neom` remains the native executable model format. GGUF is not adopted as
native and no `.neom` v2 quantized format is introduced. GGUF enters through an
importer that parses external bytes, preserves quantized tensor payloads
byte-for-byte, and loads them into the existing Model object as immutable byte
tensors with separate GGUF metadata for enumeration and future dequant.

BUILD-8.2 does not execute a GGUF model. It proves LogansOS can read a real
quantized model and tokenize text against raw-source BPE tables.

## Source Discipline

The byte layouts are pinned to raw source, not summaries:

- ggml-org/llama.cpp `5f04dc7ac3f40271f60105574be0f617887195bb`
  - `ggml/include/gguf.h`: GGUF v3 header/KV/tensor-table ABI.
  - `ggml/include/ggml.h`: `ggml_type` ABI (`Q8_0 = 8`, `Q4_K = 12`).
  - `ggml/src/ggml-common.h`: `block_q8_0` and `block_q4_K` static asserts.
  - `ggml/src/gguf.cpp`: tensor-data alignment and offset validation order.
- Qwen/Qwen2.5-0.5B raw tokenizer assets:
  - `tokenizer.json`
  - `vocab.json`
  - `merges.txt`

The raw-source check corrected one gap-analysis draft issue: the draft token IDs
for `"Hello, world!\n"` did not match Qwen2.5's tokenizer bytes. The KAT pins
the raw-source result:

```text
"Hello, world!\n" -> [9707, 11, 1879, 4894]
```

This follows the same discipline that corrected `block_q4_K` from the stale
72-byte summary to the raw-source 144-byte struct.

## Implementation

`kernel/src/ai/gguf_logic.rs` is pure logic:

- Parses GGUF v3 header, metadata KV entries, tensor infos, and aligned tensor
  data offsets.
- Supports GGUF scalar metadata types plus string and scalar/string arrays.
- Computes tensor byte lengths using GGML block-size/type-size math.
- Extracts `block_q4_K` as `d`, `dmin`, 12-byte `scales`, and 128-byte `qs`.
- Extracts `block_q8_0` as `d` and 32 signed quant bytes.
- Does not dequantize.

`kernel/src/ai/tokenizer_logic.rs` is pure logic:

- Accepts raw-source vocab and merge slices.
- Applies Qwen/GPT-style byte-level encoding.
- Applies BPE merges by rank.
- Emits token IDs and detokenizes by reversing the byte-level mapping.

`Model::from_gguf_bytes` is compiled only with the `gguf` feature. It imports
each GGUF tensor payload as an immutable `U8` tensor while retaining the GGUF
tensor table for enumeration. The existing `.neom` loader and executor remain
the native execution path.

## Proof Discipline

The gate is I-12 known-answer testing, not round-trip testing:

- A hand-laid GGUF byte fixture asserts header, metadata, tensor-table offsets,
  alignment, and data starts.
- Hand-laid Q4_K/Q8_0 blocks assert exact field offsets and reject stale block
  sizes.
- The BPE KAT asserts Qwen2.5 raw-source token IDs and detokenizes back to the
  source string.
- An env-gated integration test loads a real `Qwen2.5-0.5B-Q4_K_M.gguf` and
  enumerates tensor names/shapes/dtypes. The model artifact is not vendored.

## Anti-Scope

No dequantization, no RoPE, no SiLU/SwiGLU, no attention, no KV cache, no
forward pass, no quantized matmul, no native-GGUF execution, and no `.neom` v2
format.

## Consequences

Phase 8 can now consume real quantized-model bytes. Phase 8.3 has typed Q4_K
and Q8_0 block structures to feed into dequant and then the existing f32 matmul
path. `.neom` remains the stable native format while GGUF compatibility is
handled as an importer layer.

## BUILD-8.3 Integration Amendment

BUILD-8.3 adds the first computation on real imported GGUF weights without
making GGUF native and without adding quantized matmul. The imported tensor
payload remains byte-for-byte GGUF data in the existing Model object; a
default-off pure helper selects a named GGUF tensor, dequantizes supported
`Q4_K` and `Q8_0` payloads into f32, and feeds the resulting ordinary f32 buffer
to the existing op kernels.

The proof stays I-12: the Q4_K arithmetic remains pinned to raw llama.cpp
`dequantize_row_q4_K` / `get_scale_min_k4`, Q8_0 follows `d * qs`, and the
real Qwen2.5 `blk.11.ffn_down.weight` gate compares dequantized values plus a
small existing-matmul product against independent raw-source reference values.

Anti-scope remains unchanged for the forward graph: no RoPE/SwiGLU wiring, no
GQA/KV cache, no quantized matmul, no `.neom` v2, and no GGUF graph execution.

## BUILD-8.4 RoPE/SwiGLU Forward-Scaffold Amendment

BUILD-8.4 adds the first decoder-layer compute scaffold over imported real
Qwen2.5 weights without making GGUF executable and without introducing new op
math. Supported GGUF projection/FFN tensors are dequantized into f32 buffers,
then fed through the already-governed op kernels:

- `attn_norm -> attn_q/attn_k/attn_v -> RoPE(Q/K)` using Qwen2.5-0.5B's
  split Q `[896, 896]`, K `[896, 128]`, and V `[896, 128]` tensors plus the
  NEOX split-half RoPE pair convention.
- `ffn_norm -> gate/up matmul -> SwiGLU -> down matmul -> residual` using real
  `blk.N.ffn_{gate,up,down}.weight` tensors.

The scaffold deliberately stops before attention. The layer entry point exposes
the projected/rotated Q/K/V seam and returns `AttentionUnsupported` unless a
future attention result is injected. BUILD-8.5 owns GQA attention and KV-cache
behavior; BUILD-8.4 does not run generation end to end.

The proof stays host-tier I-12. The real-weight FFN and RoPE checks are
environment-gated on `NEO_QWEN25_GGUF`; the implementation side uses the
kernel GGUF dequant seam and existing ops, while the oracle side uses
raw-source llama.cpp `5f04dc7` quantized-block transcriptions plus independent f32
RMSNorm/matmul/SwiGLU/RoPE calculations. The tolerances are stated in the host
tests and account for the kernel soft-float op path.

The available Qwen2.5 Q4_K_M GGUF stores several projection and FFN tensors as
GGML `Q5_0`, so BUILD-8.4 extends the dequant seam with the 22-byte `Q5_0`
block path (`d`, 32 high-bit flags, and 16 packed low-nibble bytes) before
feeding those tensors into the same f32 matmul path. This remains dequant to
f32, not quantized matmul.

Anti-scope for BUILD-8.4: GQA attention, KV cache, quantized matmul, `.neom`
v2, native GGUF graph execution, hot-path generation wiring, new invariants,
and new `unsafe`.

## BUILD-8.5 GQA/KV-Cache Generation Amendment

BUILD-8.5 consumes the BUILD-8.4 Qwen2 layer scaffold and fills the attention
seam with causal GQA attention for the real Qwen2.5-0.5B shape: 24 layers,
hidden size 896, 14 Q heads, 2 KV heads, grouping factor 7, and Qwen2.5
`theta = 1e6` RoPE. The real GGUF also carries `attn_q.bias`, `attn_k.bias`,
and `attn_v.bias` tensors plus `attention.layer_norm_rms_epsilon = 1e-6`; the
forward helper applies those values before RoPE and RMSNorm respectively.

The KV cache follows the ratified Phase 8.5 Option A design. Each layer owns a
fixed-capacity f32 K tensor and f32 V tensor, sized for the chosen max context
and registered with the existing arena/tensor governance path at pre-reserve.
The governance substrate therefore sees a constant byte footprint and existing
I-9 production tensor, I-13 liveness, and reservation accounting rules hold
unchanged. `used_len` is internal cache state; no new cross-cutting invariant is
introduced.

RoPE is applied to K before the cache write and is never re-applied on cache
read, matching the raw-sourced llama.cpp attention graph convention. The
`kv_cache_enabled` kill switch selects the recompute-all uncached path when
disabled, leaving cache tensors unread and unwritten by attention and preserving
the 8.5a regression surface.

The end-to-end host path is now token embedding -> 24 decoder layers -> final
RMSNorm -> output head -> sampler. GGUF remains an importer format and all
supported quantized weights are dequantized to f32 before the existing matmul
ops. BUILD-8.5 adds the GGML `Q6_K` dequant block path because the real
Qwen2.5-Q4_K_M artifact stores several FFN tensors in that format; this is
still dequant-to-f32, not quantized matmul.

The proof stays I-12. A self-contained KAT asserts cached generation equals the
uncached recompute-all path within the stated f32 tolerance and separately
covers the kill-switch behavior and cache position bounds. The real-generation
oracle is raw-sourced from local llama.cpp commit
`5f04dc7ac3f40271f60105574be0f617887195bb`: prompt `Hello` tokenizes to
`9707`, and greedy `/completion` with `n_probs=5` returns top-1 token `271`
(`"\n\n"`). No WebFetch-derived token IDs are used.

Anti-scope for BUILD-8.5: active KV eviction/prefetch, segment eviction,
multi-stream cache state, quantized matmul, `.neom` v2, GGUF-native graph
execution, new governance invariants, and new `unsafe`.
