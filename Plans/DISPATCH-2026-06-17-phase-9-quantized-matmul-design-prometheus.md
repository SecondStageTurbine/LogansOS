<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 9 prep — the quantized-matmul design note → Prometheus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Prometheus (deep-research/design lane). **Fresh** branch off `origin/main` HEAD: `prometheus/phase-9-quantized-matmul-design` *(start clean off `origin/main` — do NOT reuse any old phase-9 branch; the FINDINGS is already on main via `06d191d`)*.
**Status:** DISPATCHED. **Design + raw-source research only — NO production code, NO `ci.yml`.** Output is one design THINK under `Plans/`. Fully parallel to Arthur Slice 1 (Janus), W2 silicon (Cerebro), the benchmark (teed for Jarvis) — touches only `Plans/`.

## 0. The decision this sets up
Phase 9 = **quantized matmul**: multiply q4_K weights by q8_K-quantized activations with an **integer dot product** (no full dequant to f32), the path that makes a genuinely capable model run at usable speed on ordinary hardware — the VISION's *"useful, not a demo"* (`docs/VISION-arthur.md` Phase 9). Today's forward pass (8.3→8.5) is **dequant → f32 → matmul**; Phase 9 replaces the inner loop with **quantized × quantized → integer-accumulate → dequant the result**, cutting weight-memory traffic ~7× (per the FINDINGS). This note designs that op + its wiring + its correctness oracle so the build is *wiring a decided design*, not inventing it under pressure. **It does not commit to building Phase 9 now** (still deferred) — it readies it.

## 1. References (on `main`)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-16-phase-9-quantized-matmul-prep-FINDINGS.md   # your own prep — build on it
git show origin/main:kernel/src/ai/forward_logic.rs                                          # the f32 dequant→matmul path Phase 9 replaces
git show origin/main:kernel/src/ai/gguf_dequant_logic.rs                                      # the block_q4_K layout (144B/256)
```
Raw-source llama.cpp's reference kernel (`ggml_vec_dot_q4_K_q8_K` and the q8_K activation quantizer) — `git`/`curl` the source, **never WebFetch** exact layouts/numbers; cite file:line/SHA. Reference, not copy (we govern ours).

## 2. What to work out (the design questions)
- **The op + its math.** The q4_K×q8_K integer dot: how an f32 activation vector is quantized to **q8_K** per-block on the fly (the block layout, the scale extraction), the `_mm256_maddubs_epi16`-style integer multiply-accumulate over the 256-element super-blocks, and the final dequant of the accumulator (weight-scale × activation-scale). Quantify the weight-traffic reduction vs the f32 path on Qwen2.5-0.5B.
- **Where it wires in.** Today `forward_logic` does dequant→f32→matmul. Phase 9 swaps the matmul inner loop. Identify the exact seam (the matmul call site) and how the quantized path slots behind it. **Kill-switch (Rule 1):** quantized-off ≡ the existing 8.3 dequant→f32→matmul path **bit-for-bit** (the disabled≡prior regression).
- **The correctness oracle (I-12) — and the honest tolerance question.** Quantized×quantized is **lossy relative to f32 dequant**, so the KAT is NOT "matches the f32 path exactly." The right oracle is a **byte/value match against `llama.cpp`'s own q4_K×q8_K kernel output** on a known input (the reference *also* quantizes activations, so the comparison is apples-to-apples) — raw-sourced, never WebFetch. State the exact reference and the tolerance, and how the q8_K activation-quantization step is itself KAT'd (a hand-laid block oracle).
- **CPU-first vs accel.** Decide whether Phase 9 is CPU-SIMD only (the `_mm256_maddubs` path) or also targets the accel-driver class; recommend (the FINDINGS likely scoped CPU-first). State it.
- **The perf claim stays falsifiable.** ~7× less weight traffic is the *hypothesis*; the design must say how it's **measured** (the latency oracle, ADR-052) and that until measured it stays **§3 Unproven** in the Strategic Honesty Ledger (`docs/STRATEGIC-HONESTY-LEDGER.md`). Phase 9 is where a §3 row *can* finally move — name the measurement that would move it.

## 3. Deliverable
`Plans/THINK-2026-06-17-phase-9-quantized-matmul-design.md`:
- the op math + the q8_K activation-quant step + the weight-traffic quantification for Qwen2.5-0.5B;
- the wiring seam in `forward_logic` + the kill-switch (quantized-off ≡ 8.3 path bit-for-bit);
- the I-12 oracle plan (the raw-sourced `llama.cpp` q4_K×q8_K reference + the tolerance + the q8_K-quant hand-laid oracle);
- the CPU-first-vs-accel scope call (with a recommendation);
- a proposed **build slicing** (e.g. 9.1 standalone quantized-matmul op + KAT → 9.2 wire into the forward pass behind the kill-switch → 9.3 the latency-oracle measurement vs the f32 path), each kill-switchable, each with its gate;
- the raw-sourced `llama.cpp` reference (file:line/SHA);
- **open decisions for PM/PL ratification** before any of it is dispatched.

## 4. Scope guard
Design + `Plans/` THINK **only**. **Do NOT** write production code, touch `ci.yml`/kernel/`ai-ops-tests`, or pre-empt the Arthur arc / the benchmark. Start a **clean branch off `origin/main`**. Raw-source any exact layout; flag second-hand numbers. **⚠ PUSH the branch before reporting done.**

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-phase-9-quantized-matmul-design-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/phase-9-quantized-matmul-design` off `origin/main` (fresh — NOT the old phase-9 branch).
- **One-line:** Turn your Phase-9 **research FINDINGS** (`git show origin/main:Plans/RESEARCH-2026-06-16-phase-9-quantized-matmul-prep-FINDINGS.md`) into a ratifiable **design THINK** for **quantized matmul** — q4_K weights × q8_K activations, integer-dot (`_mm256_maddubs`), no full dequant (~7× less weight traffic; the VISION "useful, not a demo"). Work out: the op math + the on-the-fly q8_K activation-quant step + the weight-traffic numbers for Qwen2.5-0.5B; the wiring seam in `forward_logic` (Phase 9 swaps the dequant→f32→matmul inner loop) + the **kill-switch** (quantized-off ≡ the 8.3 f32 path bit-for-bit); the **I-12 oracle** — and note the honest catch: quantized×quantized is lossy vs f32, so the KAT matches **llama.cpp's own q4_K×q8_K kernel** output (raw-source, **never WebFetch**), not the f32 path; CPU-first vs accel (recommend); and that the ~7× perf claim stays **§3 Unproven** in the Strategic Honesty Ledger until the latency oracle measures it — name the measurement. Deliver `Plans/THINK-2026-06-17-phase-9-quantized-matmul-design.md` with a build slicing (9.1 op+KAT → 9.2 wire+kill-switch → 9.3 measure) + open decisions to ratify. **Design doc only — no code, no `ci.yml`.** Push before reporting done.
