<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: F4 — make the native q5_0 path actually faster than dequant-to-f32 (HELD until it wins) → Jarvis

**Date:** 2026-06-22 (updated — PL redirect: *optimize* the native path to win, not measure-and-maybe-shelve)
**From:** PM (Claude)
**To:** Jarvis (F4 author). Branch `codex/forward-path-f4-native-quant-kernels-jarvis @ f65ce72` (rebased on F2).
**Status:** HELD AT PM VERIFY — **correct + green, but currently ~19% SLOWER than the path it replaces** (Stage-A ~1866.7 ms/token vs OT-30's 1563.4). The code is sound (parity holds; SIMD-`unsafe` only; ff-clean). **PL decision: make the native q5_0 path *actually faster*** — for a memory-bound batch-1 decode, native int8 (no f32 materialization, less memory traffic) *should* beat dequant-to-f32; the current regression is an optimization gap to close, not a reason to shelve. **F4 merges when it measurably beats 1563 ms/token (AVX2 engaged, fair host).**

## 0. The decision this build makes
Turn "native quant kernels are correct but slower" into "**the native q5_0 dot-product beats OT-30's dequant-to-f32 path on real-model Stage-A latency**." q5_0 is the **hot path** (133 of 291 tensors — the Q/K/V + FFN projections), so it's where the win is won. q6_K (12) / q8_0 (13) are secondary; get q5_0 fast first.

## 1. Why it's likely slow now (diagnose, then fix)
- **AVX2 not engaged?** Stage-A is a *host* release build, so AVX2 should be on. **Confirm the native `vec_dot_q5_0_q8_0` AVX2 path actually runs** (log/assert the dispatch) — if it's silently scalar, that alone explains the regression. Fix the runtime-detect first and re-measure.
- **AVX2 path under-vectorized.** Beating a tuned f32 AVX2 matmul requires the int8 path to exploit int8-MAC throughput + the memory-traffic advantage. Match llama.cpp's `vec_dot_q5_0_q8_0` structure (**raw-source it, don't summarize**): the 5-bit unpack (4-bit base nibbles + the high-bit plane injected via byte-from-bit shuffles), `_mm256_maddubs_epi16`/`_mm256_madd_epi16` int8 dot, per-block fp32 scale fold, accumulate across blocks in YMM. Avoid per-block scalar tails.
- **Per-block / setup overhead.** The activation→q8 quantization is one-time per token (amortized) — confirm it's not re-run per weight tensor. Process whole blocks in-register; no `Vec` churn on the hot path.

## 2. ISCs
- **ISC-1 — Optimize native q5_0 (the hot kernel) to beat dequant-to-f32.** Profile the q5_0 dot-product vs OT-30's dequant+f32-AVX2-matmul on a real q5_0 tensor; close the gap (AVX2 engaged + well-vectorized per §1). Secondary: keep q6_K/q8_0 correct + at least not-slower (optimize if cheap; q5_0 is the priority).
- **ISC-2 — Correctness preserved (non-negotiable).** The F1/F4 per-type byte-oracle + the equivalence KAT vs the dequant-to-f32 reference stay green; Stage-A **top-1 llama.cpp parity must still hold** (`first_divergence=None`). Optimization must not perturb the numerics beyond the stated rounding tolerance.
- **ISC-3 — The success bar = a measured drop, fairly.** Re-measure **back-to-back on a QUIET host** (no concurrent QEMU gates — the 1866 was taken under 3-gate contention), OT-30-path vs optimized-F4-path, same 0.5B/prompt/K, median per-token ms. **F4 merges only when it's materially < 1563 ms/token with AVX2 confirmed engaged.** Report both numbers + the delta.
- **ISC-4 — kill-switch + standards unchanged.** `phase9-real-quant` off ≡ fixture bit-for-bit; SIMD-`unsafe` only (no new beyond the AVX2 entry points); no new invariant; SPDX; fmt/clippy via Docker; kernel-touching ⇒ 4-shard QEMU re-gate on the optimized SHA.

## 3. Scope guard
Optimize the native q5_0 (and cheaply q6_K/q8_0) dot-products + prove the win **only**. **Do NOT**: change the dequant *values*/numerics (the equivalence KAT pins them), rewrite the forward graph (OT-16), touch config (F2)/vocab (F3), or add threading (that's the *separate* OT-34 lever — one axis at a time). Keep OT-30's dequant-to-f32 path present as the equivalence reference.

## 4. Merge (PM)
F4 merges on ISC-3's measured drop (AVX2 engaged, quiet-host A/B) + the 4-shard QEMU green + parity preserved. **Close-gate: `scripts/worker-close-gate.sh`**; RETURN the green push-CI + 4-shard QEMU URLs **and the before/after per-token ms** (the win is the deliverable). PM scrutinizes the parity (unchanged) + the measured delta.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-f4-prove-latency-win-jarvis.md` (updated). **Branch:** your `codex/forward-path-f4-native-quant-kernels-jarvis`.
- **One-line:** **PL goal — make the native q5_0 path actually FASTER than OT-30's dequant-to-f32** (it's currently ~19% slower: 1866 vs 1563 ms/token; F4 holds until it wins). q5_0 is the hot path (133 tensors) — optimize it first. **Diagnose first:** confirm the AVX2 `vec_dot_q5_0_q8_0` is actually engaged on the host (if it's silently scalar, that's the regression — fix the runtime-detect, re-measure); then vectorize to beat the tuned f32 AVX2 matmul — match **llama.cpp's `vec_dot_q5_0_q8_0`** structure (raw-source it): 5-bit unpack (nibbles + high-bit plane via byte-from-bit), `_mm256_maddubs_epi16` int8 dot, fp32 scale fold, YMM accumulate, no per-block scalar tails / `Vec` churn. **Correctness frozen:** the byte-oracle + equivalence KAT + Stage-A top-1 llama.cpp parity (`first_divergence=None`) must stay green. **Success bar:** a **quiet-host** back-to-back A/B (OT-30 vs optimized-F4), **materially < 1563 ms/token with AVX2 confirmed**, report both numbers. Kill-switch off ≡ fixture; SIMD-`unsafe` only; **NOT threading (separate OT-34), not config/vocab/graph.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN green push-CI + 4-shard QEMU URLs + before/after per-token ms.**
