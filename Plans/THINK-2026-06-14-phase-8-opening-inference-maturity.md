<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 8 opening — inference maturity (the JARVIS L1 layer)

**Date:** 2026-06-14
**Seat:** PM/Cerebro (phase-opening design pass — grounded in a verified file:line map of the current substrate; not yet a dispatch)
**Context:** The PL authorized opening Phase 8 in parallel with the Phase 7.5 WiFi tail (W1/W2), using a third worker (Janus). This is the VISION doc's **Layer 1** — "inference maturity → real LLM serving" — the first additive layer toward a JARVIS-class assistant. The substrate (Phases 5–7.5) is proven; this layer is **additive on top**, not a rework.
**Related:** `docs/VISION-logansos-to-jarvis.md` §3 (the additive layers), ADR-028 (.neom format), `docs/complexity-governance.md` §4 (the latency oracle — becomes load-bearing here; BUILD-LO is in flight), Article 6.5 (inference only, training out of scope), Article 28 (accessibility).

## 1. The thesis (why this is additive, not a rework)
LogansOS today **computes a single transformer forward pass, governed** — it does not yet **serve** a model. The gap between "forward pass" and "assistant" is four well-bounded, mostly-orthogonal pieces. The hard, novel layer (governing intelligence as a kernel resource) is built and proven; Phase 8 adds the **conventional LLM-serving machinery on top of it**, and the governance is what makes that serving *denser and safer* than a userspace runtime (VISION §5). "How much do we change about what exists?" → almost nothing. "How much do we build on top?" → these four builds.

## 2. What EXISTS vs what is NET-NEW (verified, file:line — not assumed)
A map pass (this session) established the honest baseline:

**Exists (the substrate Phase 8 rides):**
- **12 f32 op kernels** — Identity/Add/Mul/Matmul/Relu/Gelu/Softmax/LayerNorm/RmsNorm/Embedding/Reshape/Transpose (`kernel/src/ai/ops/mod.rs:26-650`), with host numerical oracles (`tools/ai-ops-tests/src/lib.rs` — `valid_mlp_forward_matches_hand_oracle`, `transformer_forward_references_match_embedded`).
- **A single-pass graph executor** — `kernel/src/ai/exec.rs::run()` (topological, residency-aware: mints weights, places each op, room-checks/evicts, dispatches, recency-touches). Proven on `valid_transformer_block.neom` (one block).
- **The .neom v1 format + loader** — `kernel/src/ai/model_logic.rs`, `model.rs`, ADR-028. Declares `F32/F16/BF16/I32/I8/U8/Bool` dtypes but **every op runs f32**; no dequant path.
- **The full governance hot path** a decode loop would call per token — `exec_dispatch::allocate_result()` (place/room/evict/daemon), `arena::mint()`, `reservation::guarded_clone()`.

**Net-new (literally zero code today — the Phase 8 scope):**
1. **Autoregressive decode loop + sampling** — no generation loop, no temperature/top-k/top-p, no logits→token. Inference stops at one forward pass.
2. **KV cache** — no `KvCache`, no attention-with-cache, no position state. (The *governance* for it exists; the cache is net-new wiring.)
3. **Tokenizer** — none (no BPE/SentencePiece/vocab).
4. **Quantization** — none (no dequant op, no quantized matmul; f32 only).

## 3. The build sequence (recommended)
Decomposed by dependency + independent-testability, smallest-foundational first:

- **8.1 — the autoregressive decode loop + sampling.** *(Janus's first build.)* Turn the single-pass executor into generation: a loop that feeds the sampled token back as the next input, plus the sampler (greedy/argmax + temperature/top-k/top-p with a seeded RNG). Operates on **token IDs** (no tokenizer needed yet) and **recomputes the full context each step** (no KV cache yet — **correctness-first**, the CPU-fallback thesis; O(n²) but provably correct, and the KV cache is 8.2's optimization). Rides the existing `exec::run` per token — **purely additive, a new `generate` module; does not modify any function BUILD-LO benchmarks.** Gate = host KATs: greedy is fully deterministic (argmax over fixed logits → known token sequence); seeded temperature/top-k/top-p against an independent reference (I-12 — published-answer, not round-trip).
- **8.2 — the KV cache wired to residency governance.** *(The showcase build — the thesis made concrete.)* K/V append buffers as **governed tensors** (the policy layer manages their residency/eviction — exactly the VISION claim that "KV-cache management ≈ solved" because KV cache *is* governed tensors); attention-with-cache replacing the recompute. Likely needs **one new invariant** (KV-cache eviction mid-generation must fail safe / be recomputable) — named + guarded in-build per Rule 2. Depends on 8.1.
- **8.3 — the tokenizer (BPE).** Encode/detokenize against a **published** vocab+merges fixture (the raw-source-KAT rule applies to the vocab bytes — pull raw, never a summarizing fetch). Independent of 8.1/8.2 — parallelizable.
- **8.4 — quantization (int8 → int4).** A dequant op + a quantized matmul kernel, KAT'd against a numerical reference. "So a real model fits." Independent-ish (a format/numerics extension).
- **8.5 — end-to-end: a real small quantized model served.** Integration: tokenizer → decode loop + KV cache → quantized weights → text out. The phase's capstone gate.

### Amendment (2026-06-14) — the build sequence re-grounded by the gap analysis, and the format decision ratified
The Phase-8 gap analysis (`Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md`, on `main`; PM-verified — it even caught its own wrong `block_q4_K` byte size, corrected 72→**144** against the `ggml-common.h:328` static_assert) replaces the *tentative* 8.2–8.5 above with a **dependency-grounded** sequence against a **concrete first-light target: Qwen2.5-0.5B at Q4_K_M** (~491 MB GGUF, ~276 MB weights, fits the OptiPlex; ~5–30 s/token soft-f32 first light). **8.1 (Janus, in flight) is unchanged.** The grounded 8.2→8.5:
- **8.2 — GGUF importer + BPE tokenizer.** Parse GGUF v3 (header/KV-metadata/tensor headers), extract quantized weight blocks, implement the BPE tokenizer (raw-source vocab/merges KAT). Gate: load the real `.gguf`, print tensor names/shapes/dtypes, tokenize a test string == published token-ids.
- **8.3 — Q4_K dequant + f32 inference path.** `dequantize_row_q4_K` → f32 (the 144-byte block, per `ggml-quants.c`), route weight loads through dequant into the **existing f32 matmul**; wire the forward pass with existing ops. Gate: single-layer forward vs a llama.cpp activation dump.
- **8.4 — the net-new elementwise/positional ops: RoPE (YaRN, θ=1e6) + SiLU/SwiGLU.** Gate: per-layer output within 1e-4 of llama.cpp.
- **8.5 — GQA causal attention + KV cache** (2 KV heads → 14 query heads), KV append/reuse — **this is where the "KV-cache-as-governed-tensors" showcase lands** (the §3 8.2 framing moves here). Gate: token-in → logits-out, generate N tokens, top-1 vs llama.cpp.
- **Deferred to Phase 9:** the **quantized matmul** (Q4_K×Q8_0 direct GEMM) — a *performance* optimization; first light dequants to f32 and reuses the proven matmul (**correctness-first**, our thesis). Also SIMD, multi-batch.
- **Net-new ops total: 6** — RoPE, SiLU, SwiGLU, GQA-attention+KV, Q4_K-dequant, (quantized-matmul, Phase 9). Each carries a raw-source `file:line` spec in the FINDINGS §2.

**Format decision — RATIFIED (PL, 2026-06-14): option #1, a GGUF importer.** `.neom` stays the **native** model object (Articles 21 / 23.4 — *"we define our own; importers handle compatibility"*); GGUF (the universal quantized-model format) enters through a **8.2 importer** that loads into the existing `Model` object. **No `.neom` v2 quant format is built now** — it's deferred until/unless we want to author/store quantized models in our own format. Rejected: adopting GGUF as native (contradicts 21/23.4); a GGUF→`.neom`-v2 on-disk step (re-implements GGUF's quant layout for no first-light payoff).

## 4. The first build (8.1) — enough to ratify
- **Scope:** a `generate`/decode module (host-testable logic tier where possible, mirroring the `*_logic` pattern) that drives `exec::run` autoregressively, a sampler (argmax + temperature/top-k/top-p, seeded deterministic RNG), and a stop condition (max-tokens + an EOS token-id). No tokenizer, no KV cache, no quant — token-IDs in, token-IDs out.
- **Correctness-first:** recompute the full growing context each step. This is O(n²) and *deliberately un-optimized* — 8.2's KV cache is the optimization, and doing it second keeps each build's correctness gate clean. The latency this exposes is exactly what BUILD-LO will measure (the per-token tax) — a real workload for the oracle.
- **Gate (host + QEMU):** host sampling KATs (greedy deterministic sequence; seeded temperature/top-k/top-p vs an independent reference — I-12, not round-trip); a QEMU end-to-end generating N tokens from `valid_transformer_block.neom` (or a small purpose-built `.neom`), the family green, kill-switch (feature-off ≡ today).
- **Governance posture:** additive; kill-switch trivially holds (a new feature/module, off ⇒ bit-for-bit); **no new invariant** (8.1 adds orchestration, not a cross-cutting property — 8.2's KV cache is where an invariant likely lands); no new lock; no new `unsafe`. Article 15.7 satisfied (generation is the committed Layer-1 deliverable).

## 5. Non-conflict with the in-flight work (the parallelism is clean)
- **vs BUILD-LO (Jarvis):** LO *instruments* the governance hot-path functions (`mint`, `decide`, accounting); 8.1 *adds a generation module that calls* `exec::run` in a loop. Disjoint surfaces — 8.1 modifies none of the functions LO benchmarks. The only shared file is `complexity-governance.md` (audit-log append — trivial). In fact they're synergistic: 8.1 is LO's first real per-token workload.
- **vs Phase 7.5 (W1/W2, Prometheus mine):** entirely different module trees (`net/`/`drivers/usb/` vs `ai/`). Zero overlap.
- **Two concurrent phases:** 7.5 (connectivity) and 8 (inference) are independent surfaces; the per-build kill-switch + invariant-budget discipline bounds the combined complexity. The one-page ledger stays the governor (Rule-4 consolidation still cadenced).

## 6. Constraints stated honestly (VISION §4)
- **Compute gates scale, not architecture.** The OptiPlex UHD 630 is a proof-of-concept; a real JARVIS wants a real accelerator. But the architecture scales, and a **3–7B quantized model is the credible near-term target** — Phase 8 builds toward *that*, not a 70B.
- **Latency becomes load-bearing here.** "Designed-for, not measured" is acceptable for a batch governor, not a conversation — which is *why* BUILD-LO (the §4 oracle) is in flight now, ahead of 8.2's KV cache. The decode loop's per-token number is the first place the oracle's budget bites.
- **Inference only** (Article 6.5 — no training). Single-node (no distributed serving). Minimal-sufficient: the smallest pipeline that *serves*, then grow.

## 7. Design questions for the PL
- **Q1 — Open Phase 8 with the §3 build sequence (8.1 decode loop → 8.2 KV cache → 8.3 tokenizer → 8.4 quantization → 8.5 end-to-end), running parallel to the 7.5 WiFi tail?** *(Recommended — §3; the surfaces are independent and the discipline bounds the combined complexity.)*
- **Q2 — Dispatch 8.1 (the decode loop + sampling) first, as Janus's opening build?** *(Recommended — §3/§4: the most foundational piece, host-KAT'able, cleanly non-conflicting with BUILD-LO, and LO's first real workload.)*
- **Q3 — Correctness-first: 8.1 recomputes the full context each step; the KV cache is 8.2's optimization, not folded into 8.1?** *(Recommended — keeps each correctness gate clean; matches the CPU-fallback "correct first, optimize second" thesis.)*
- **Q4 — Scope guard: inference only, single-node, 3–7B-quantized as the near-term target (no training, no distributed, no 70B assumption)?** *(Recommended — §6, Article 6.5.)*

## 8. If ratified — the first dispatch
**BUILD-8.1 (the autoregressive decode loop + sampling) → Janus:** a `generate` module driving `exec::run` autoregressively + a seeded sampler (argmax / temperature / top-k / top-p), token-IDs in/out, recompute-context (no KV cache/tokenizer/quant yet), gated by host sampling KATs (I-12, published-answer not round-trip) + a QEMU N-token generation from a small `.neom`. Additive, feature-gated, kill-switch ≡ today, no new invariant/lock/`unsafe`. Opens ANSPEC §11.x (the inference-serving model) + ADR-053 ⟦confirm next free ADR at dispatch — LO took 052⟧. Branch off current `main`. The KV-cache showcase (8.2) follows once generation is proven.
