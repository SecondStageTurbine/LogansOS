<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 9 prep — quantized-matmul (the perf path) deep research → Prometheus

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Prometheus (deep-research lane). New branch off `origin/main` HEAD: `prometheus/phase-9-quantized-matmul-prep`.
**Status:** DISPATCHED. **Research + raw-source mining only — NO production code, NO `ci.yml`.** Output is one FINDINGS doc. Runs fully parallel to 8.3-integration (Janus) and W2 (Cerebro) with zero collision (it touches only `Plans/`).

## 0. The decision this build sets up
Phase 8 builds the **f32** inference path (8.3 dequant→matmul, 8.4 RoPE/SwiGLU, 8.5 GQA+KV → a real Qwen2.5-0.5B forward pass). **Phase 9 = make it fast: matmul directly on quantized blocks** (q4_K weights × q8 activations, integer dot) instead of dequant-to-f32-then-matmul — the memory-bandwidth + SIMD win that makes on-device inference (Arthur) viable. This dispatch mines the approach **now**, so the moment the f32 path is proven, Phase 9 is ready to slice and dispatch — and so 8.3-8.5 are built without foreclosing the quantized path.

## 1. What to mine (raw-source — `git clone`/`curl`, NEVER WebFetch the exact kernels/numbers)
- **The ggml quantized-matmul kernels:** how `llama.cpp`/`ggml` does q4_K matmul — `ggml_vec_dot_q4_K_q8_K` (and the block-q8_K activation-quantization path `quantize_row_q8_K`). The core trick: **quantize the activations to q8_K, then integer dot-product against the q4_K weight blocks** (no full f32 dequant of the weights). Pull the literal kernel source + the block layouts (`block_q8_K`) and read them.
- **The accuracy story:** q4_K×q8_K integer-dot vs dequant-then-f32-matmul — what's the numerical difference, and what reference do we KAT against (the same `llama.cpp` op output on a known tensor+input — an exact-byte/float oracle, raw-sourced)?
- **The perf delta:** why it wins (weight bytes stay 4.5-bpw through the matmul; SIMD integer MAC; cache/bandwidth). Note what it needs from our substrate (AVX2/`target-cpu` per the CachyOS ADOPT note; the op-kernel SIMD).
- **The I-12 / kill-switch shape:** the byte-oracle plan for a quantized-matmul KAT (raw-source vectors), and how it stays a default-off-switchable layer over the proven f32 path (disabled ≡ the f32 matmul, bit-for-bit-equivalent within tolerance).

## 2. Deliverable
`Plans/RESEARCH-2026-06-16-phase-9-quantized-matmul-prep-FINDINGS.md`:
- the q4_K×q8_K kernel + `block_q8_K` layout, **raw-source-cited** (file:line, commit SHA);
- the activation-quantization step + the integer-dot math;
- the accuracy + perf analysis (with the caveat that exact numbers are second-hand unless raw-pulled);
- a proposed **Phase-9 build slicing** (e.g., 9.1 q8_K activation-quant + byte-oracle; 9.2 the q4_K×q8_K dot kernel + KAT; 9.3 wire into the forward pass behind a kill-switch, disabled≡f32);
- the I-12 oracle-acquisition plan (which upstream test vectors, raw-pulled).

## 3. Scope guard
Research + `Plans/` FINDINGS **only**. **Do NOT** write production code, touch `ci.yml`, or start a Phase-9 build (this is prep). **Raw-source every exact kernel/layout/vector — never WebFetch** (the q4_K 72→144 catch + the BUILD-W0 WebFetch-mangled-vector are why). Flag any number you couldn't raw-pull as second-hand.
**⚠ PUSH the branch before reporting done** (the standing reminder — prior lanes needed a push follow-up).

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-phase-9-quantized-matmul-prep-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/phase-9-quantized-matmul-prep` off `origin/main`.
- **One-line:** Deep-research the **quantized-matmul perf path** for Phase 9 — how `ggml` does **q4_K weights × q8_K activations integer-dot** (no full f32 dequant): raw-source `ggml_vec_dot_q4_K_q8_K` + `quantize_row_q8_K` + `block_q8_K` (file:line/SHA, **never WebFetch the kernels/vectors**), the accuracy-vs-f32 + perf story, the I-12 byte-oracle plan, and a proposed 9.1/9.2/9.3 slicing that stays a kill-switchable layer over Phase-8's f32 path. **FINDINGS doc only — no production code, no `ci.yml`.** Push the branch before you report done.
