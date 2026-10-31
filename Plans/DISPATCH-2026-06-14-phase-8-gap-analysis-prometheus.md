<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 real-model gap analysis (raw-source) → Prometheus

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Prometheus (research / raw-source mining lane; NO kernel code)
**Status:** DISPATCHED. **Goal: turn "Phase 8" from a sketch into a checklist — answer concretely "what does it actually take to serve a real small model on LogansOS?" — so Janus's next builds (8.2 KV cache → 8.3 tokenizer → 8.4 quantization → 8.5 real-model-served) are scoped on verified facts, not discovered mid-build.**
**Why now:** Janus is *actively* building Phase 8 (8.1 decode loop in flight). Arthur (the integral assistant, Bible Article 29) is the committed telos, and inference maturity is its first layer. The two format pieces (quant blocks, tokenizer vocab) are **exact byte layouts** — so this lane binds the raw-source-KAT rule hard.
**Related (read for the baseline):** `Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md` (the verified substrate map), `docs/VISION-logansos-to-jarvis.md`, ADR-028 (`.neom` v1).

## 0. The verified baseline you're measuring against (on `main`, don't re-derive)
- **Op set today (12, all f32):** `kernel/src/ai/ops/mod.rs` — Identity, Add, Mul, **Matmul**, Relu, **Gelu**, **Softmax**, **LayerNorm**, **RmsNorm**, **Embedding**, Reshape, Transpose. No `unsafe`-free quant path, no dequant.
- **Executor:** single forward pass, `kernel/src/ai/exec.rs::run`. 8.1 (Janus) adds the autoregressive loop + sampling.
- **`.neom` format:** f32 only; the header *declares* `F16/BF16/I32/I8/U8` dtypes but there is **no dequant op or quantized kernel** (ADR-028, `model_logic.rs`).
- The net-new ops a modern decoder-LLM is *likely* to need (your job to confirm precisely): **RoPE** (rotary positional), **SiLU / SwiGLU** (gated activation — we have GELU+Relu, not SiLU), **GQA / KV-cache-aware causal attention** (we have matmul+softmax, not a fused/masked/cached attention), and a **quantized matmul + dequant**.

## 1. The mission — five deliverable sections
- **§1 — Pick a concrete first-light target + document its exact architecture.** Choose a *small, real, open-weight* decoder model that could plausibly run on the OptiPlex (UHD 630 + CPU fallback) — candidates to weigh: Qwen2.5-0.5B / 1.5B, TinyLlama-1.1B, a Phi-mini, or even a GPT-2-class model for the *very first* light. For the chosen target, document **from its reference config/source** (raw `config.json` + the model paper/reference impl): hidden size, layers, heads, **attention type (MHA vs GQA — kv-head count)**, **positional encoding (RoPE base/theta vs learned)**, **activation (SiLU/SwiGLU vs GELU)**, **norm (RMSNorm vs LayerNorm)**, vocab size, context length, tied embeddings.
- **§2 — The op gap (the load-bearing section).** Map that architecture's required ops against the 12 we have. Produce a table: op → have it? / net-new? / notes. **Name every net-new op precisely** (the likely set: RoPE, SiLU/SwiGLU, GQA causal attention with KV cache, quantized matmul, dequant). For each net-new op, cite a reference implementation (file:line, raw source — e.g. llama.cpp `ggml.c`/the model's reference) so the build has a spec.
- **§3 — Quantization format (RAW BYTES — the raw-source-KAT rule binds).** Pick a target quant scheme (e.g. **Q4_K_M** and/or **Q8_0**) and document the **GGUF block byte layout VERBATIM from `ggml-quants.c`** (the `block_q4_K` / `block_q8_0` structs: scales, mins, quantized nibbles, super-block structure) + the **exact dequant math**. `git clone` llama.cpp; read the literal struct + the dequant kernel; **never WebFetch the byte layout**. This feeds the `.neom` quant extension (8.4) and its I-12 byte-oracle. Also document the **GGUF container format** enough to know how weights+metadata are laid out (the `gguf.md` spec + `gguf.c`).
- **§4 — Tokenizer (RAW — raw-source-KAT rule binds).** The target's tokenizer: BPE or SentencePiece/Unigram, the **vocab + merges file format** (HF `tokenizer.json`, or the GGUF-embedded tokenizer), and the **exact encode/decode algorithm** (the BPE merge loop, byte-level pre-tokenization, special tokens). Pull a small slice of the real vocab/merges raw so 8.3 has a published encode→token-ids known-answer test. **Never WebFetch the vocab bytes.**
- **§5 — Honest scoping.** Smallest credible "first light" (a tiny real model end-to-end, even GPT-2-small) vs the 3–7B aspiration; memory math (model size at the chosen quant vs the OptiPlex's RAM + UHD 630); what runs CPU-fallback vs needs the accel path; the honest gaps and risks (e.g. if GQA+RoPE+SwiGLU+quant is a lot of net-new op surface, say so and suggest a slicing order).

## 2. Deliverable
`Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md` containing the five sections, with: the op-gap table (have/net-new), the **verbatim quant block layout + dequant math** (raw-source, file:line), the **tokenizer format + a small raw vocab/merges slice** for a KAT, and a recommended **build-slicing order** for 8.2–8.5 grounded in what you found. Plus a provenance table (repo / commit / file:line / how-obtained) and honest negatives.

## 3. Scope guard
Research/acquisition only. **Do NOT** write Rust, design the `.neom` v2 quant format, or implement any op (you supply the mined facts the builds are made of). **Do NOT** WebFetch exact byte layouts (quant blocks, vocab) — `git clone`/`curl` raw and read the literal bytes (the raw-source-KAT rule; it caught the SAE mac2 slip and the AX88179 day). **Do NOT** over-commit to a 70B fantasy — the question is the *smallest real thing that runs*, then scale.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-phase-8-gap-analysis-prometheus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-phase-8-gap-analysis-prometheus.md
  git show origin/main:Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md
  ```
- **One-line:** Scope Phase 8 from facts. Pick a concrete small open-weight target model; document its exact architecture (attention type/GQA, RoPE, SiLU/SwiGLU, RMSNorm, vocab); **map its ops against our 12** (`kernel/src/ai/ops/mod.rs`) and name every net-new op (likely RoPE / SwiGLU / GQA-causal-attention-with-KV-cache / quantized-matmul / dequant) with a raw-source reference; document the **GGUF quant block byte layout VERBATIM from `ggml-quants.c`** (Q4_K_M / Q8_0 + dequant math) and the **tokenizer vocab/merges format + a raw slice** for a KAT — **`git clone`, never WebFetch the exact bytes** (raw-source-KAT rule). Add honest scoping (smallest first-light vs 3–7B; OptiPlex memory math) + a recommended 8.2→8.5 slicing order. Deliver `Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md`. Pure research — no code. This turns Phase 8 into a checklist for the builds Janus is writing right now.
