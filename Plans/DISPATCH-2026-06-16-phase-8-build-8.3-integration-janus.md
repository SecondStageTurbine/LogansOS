<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.3 (integration) — dequant the real model + the f32 weight path → Janus

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Janus (built 8.1 decode loop + 8.3a/8.4a/8.5a ops — best fit). New branch off `origin/main` HEAD (`b44f0f9`+): `codex/phase-8-build-8.3-integration-janus`. *(Jarvis authored 8.2/GGUF — ping him for importer questions.)*
**Status:** DISPATCHED. **Host-tier — byte/float oracles (I-12) + the real Qwen2.5-0.5B-Q4_K_M.** The first build that *wires the banked pieces together*: now that 8.2 (importer) and 8.3a (standalone Q4_K dequant) are on main, prove the **GGUF blocks → dequant → f32 → existing matmul** seam on real weights. **Pure integration of proven parts** — no net-new math.

## 0. The decision this build makes
Turn "we can read the file" (8.2) + "we can dequant a hand-laid block" (8.3a) into "**we can dequant the real model's weights and compute with them.**" Route a real tensor from the loaded `Model` (8.2's importer) through 8.3a's `dequantize_q4_K` (and Q8_0 where present) to `[f32]`, then through the **existing** matmul — and validate the result against an independent reference. This is the first real-weight computation; it does **not** yet run RoPE/SwiGLU in the forward graph (8.4) or GQA+KV-cache (8.5) — those wire the remaining 8.4a/8.5a ops next.

## 1. References (on `main`)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §3 dequant ref, §1 Qwen shapes
git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.2-gguf-importer.md             # the importer → Model path
git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.3a-q4k-dequant-janus.md        # the standalone dequant you built
```
8.3a's `dequantize_q4_K(&[u8] 144) -> [f32;256]` + 8.2's typed quantized-block buffers (`block_q4_K`/`block_q8_0`) in the `Model` are the inputs; the existing `ai/ops` matmul is the consumer. Reuse, don't reimplement.

## 2. ISCs
- **ISC-1 — Model-tensor → f32 dequant path.** A function that takes a named quantized tensor from the loaded `Model` (8.2) and produces its full `[f32]` buffer by iterating 8.3a's per-block dequant across the tensor's blocks (Q4_K and Q8_0). **Float-oracle (I-12):** dequant a **real Qwen2.5-0.5B-Q4_K_M tensor** (e.g. `blk.11.ffn_down.weight`, which 8.2 already enumerates) and assert the f32 output **matches an independent reference** — `llama.cpp`'s `dequantize_row_q4_K` over the *same* raw tensor bytes (raw-source the reference values; **never WebFetch** them), within a stated f32 tolerance. Not round-trip.
- **ISC-2 — f32 matmul on real weights.** Feed the dequantized weight matrix + a fixed input vector through the **existing** `ai/ops` matmul; assert the product against a hand/Python/`llama.cpp`-computed reference for that exact tensor+input (the first real-weight compute, oracle-checked). Pick one concrete, checkable op — the embedding row-gather or the first `q_proj`/`ffn` matmul — and pin it.
- **ISC-3 — Standards.** Host-testable (`*_logic` + `ai-ops-tests`); behind the existing `gguf`/`ai-generate` feature gate (kill-switch: off ≡ today, bit-for-bit); reuse 8.3a's dequant + the existing matmul (no net-new op math, no quantized-matmul — that's Phase 9); `fmt`/`clippy -D warnings`; **no new `unsafe`, no new invariant**, no AI-hot-path/governance-lock touch. ANSPEC §11.x note; ADR-055 amendment (or fold into a short 8.3 note). **Raw-source the `llama.cpp` reference dequant/values** (the q4_K 72→144 catch is why).

## 3. Gate (host)
Float-oracle: a real Qwen Q4_K tensor dequants to f32 matching the `llama.cpp` reference (tolerance stated); the f32 matmul on that real tensor matches an independent reference; the existing op + KAT family unaffected; builds on+off the feature; `fmt`/`clippy` clean. **No QEMU** (pure host path; if you add a `_baremetal`/hardware-only or default-off scenario, register it in the hardware-only exclusion per the BUILD-W1 shard-3 fix + its regression test — do NOT let a new scenario into the QEMU shards un-gated).

## 4. Scope guard
The dequant-tensor → f32 → existing-matmul seam on the real model **only**. **Do NOT** wire RoPE/SiLU/SwiGLU into a forward graph (8.4), GQA-attention/KV-cache (8.5), implement quantized-matmul (Phase 9), or touch `generate.rs`/the decode loop beyond what's needed to feed one matmul. Reuse 8.3a + the existing matmul. Raw-source the reference; no new `unsafe`/invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main` (reconcile the additive `ai-ops-tests/src/lib.rs` — keep all KATs), re-run the float-oracle + family, push (CI host gate; **let it run** — no `[skip ci]`). PM `pm-merge-check` → `--ff-only` → ledger. Next: **8.4** (wire RoPE/SwiGLU into the forward pass using 8.4a) → **8.5** (GQA + KV cache using 8.5a — the governed-tensors residency showcase) → a full Qwen2.5-0.5B forward pass + token generation on real weights.

---

## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-phase-8-build-8.3-integration-janus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-16-phase-8-build-8.3-integration-janus.md
  ```
- **Branch:** `codex/phase-8-build-8.3-integration-janus` off `origin/main` HEAD.
- **One-line:** Wire the banked pieces — **8.2's GGUF Model tensors → 8.3a's `dequantize_q4_K` → `[f32]` → the existing matmul** — and prove it on the **real Qwen2.5-0.5B-Q4_K_M**. Two oracles (I-12, not round-trip): (1) a real Q4_K tensor dequants to f32 matching **`llama.cpp`'s `dequantize_row_q4_K`** over the same bytes (raw-source the reference, never WebFetch); (2) an f32 matmul on that real tensor matches an independent reference. Host-tier, behind the `gguf`/`ai-generate` gate (kill-switch off ≡ today), reuse-only (no net-new op math, no quantized-matmul). If you add any hardware-only/default-off scenario, **gate it out of the QEMU shards** (BUILD-W1's shard-3 lesson). fmt/clippy clean, no new `unsafe`/invariant, let CI run. Lands the first real-weight computation; 8.4/8.5 then wire RoPE/SwiGLU/GQA+KV for the full forward pass.
