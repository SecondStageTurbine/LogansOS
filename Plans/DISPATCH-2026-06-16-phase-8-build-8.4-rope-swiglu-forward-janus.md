<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.4 — wire RoPE + SwiGLU-FFN into the forward pass (on real weights) → Janus

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Janus (built 8.1 decode loop + 8.4a ops + 8.3-integration — full context). New branch off `origin/main` HEAD (`7f1c718`): `codex/phase-8-build-8.4-rope-swiglu-forward-janus`. **No wait** — 8.3-integration (dequant path) + 8.4a (standalone RoPE/SwiGLU) are both already on main.
**Status:** DISPATCHED. **Host-tier — float-oracles (I-12) on real Qwen2.5-0.5B weights.** Assemble the **non-attention** half of a decoder layer's forward compute: dequant'd weights (8.3) + 8.4a's RoPE/SwiGLU, validated per-op against `llama.cpp`. Attention + KV-cache is **8.5**.

## 0. The decision this build makes
Turn the proven seam (8.3: real weights → f32 → matmul) + the proven ops (8.4a: RoPE/SwiGLU standalone) into **real per-layer compute**: RMSNorm → projections → **RoPE on Q/K** → (attention is 8.5) … → RMSNorm → **SwiGLU FFN** → residual. This build wires **RoPE** (applied to real projected Q/K) and the **SwiGLU FFN** (on real `ffn_gate/up/down` weights) into the forward path and validates each against a reference. It does **not** do attention or the KV cache (8.5), nor full end-to-end generation (8.5's job once attention lands).

## 1. References (on `main`)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §1 Qwen shapes, §2 RoPE/SwiGLU refs (file:line)
git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.4a-ops-janus.md                # the standalone RoPE/SwiGLU you built
git show origin/main:Plans/DISPATCH-2026-06-16-phase-8-build-8.3-integration-janus.md          # the dequant→f32→matmul seam you just landed
```
Reuse 8.4a's RoPE/SwiGLU kernels + 8.3's `gguf_dequant_logic` (dequant a real weight tensor → f32) + the existing matmul/RMSNorm. No net-new op math.

## 2. ISCs (each a float-oracle on REAL weights — I-12, not round-trip)
- **ISC-1 — SwiGLU FFN on real weights.** Wire the FFN block: `down_proj · (silu(gate_proj·x) ⊙ up_proj·x)` using 8.3-dequant'd real `blk.N.ffn_{gate,up,down}.weight` + the existing matmul + 8.4a's SwiGLU. **Oracle:** a fixed input `x` through the real FFN block matches `llama.cpp`'s output for that block+input (raw-source the reference, **never WebFetch**; tolerance stated).
- **ISC-2 — RoPE on real projected Q/K.** Apply 8.4a's RoPE (YaRN, `theta=1e6`, Qwen2.5's pair-interleaving convention) to Q/K produced by the real `q_proj`/`k_proj` matmuls at a fixed position. **Oracle:** rotated Q/K match the `llama.cpp` reference for that position+weights. **Pin the interleaving convention** (8.4a's known trap) against the real-weight output.
- **ISC-3 — the layer scaffold (no attention).** A `forward_layer`-style path that runs RMSNorm → projections → RoPE(Q/K) → [attention placeholder/`unsupported` — 8.5] → RMSNorm → SwiGLU-FFN → residual, structured so 8.5 drops attention+KV in. **Do NOT** implement attention or generate end-to-end. Leave the attention seam explicit + unsupported.
- **ISC-4 — Standards.** Host-testable (`*_logic` + `ai-ops-tests`); behind the `gguf`/`ai-generate` gate (kill-switch ≡ today bit-for-bit); reuse 8.4a + 8.3 + existing ops (no net-new math, no quantized-matmul = Phase 9); `fmt`/`clippy -D warnings`; **no new `unsafe`/invariant**; no AI-hot-path/governance touch. ANSPEC §11.x; fold into ADR-055 or a short 8.4 note. **Raw-source the `llama.cpp` references.**

## 3. Gate
- Host float-oracles green: real-weight SwiGLU-FFN and RoPE-on-real-Q/K each match `llama.cpp` (tolerance stated); existing op + KAT family unaffected; builds on+off the feature; `fmt`/`clippy` clean. **No QEMU scenario** — but **this branch touches `kernel/` ⇒ run the pre-merge `workflow_dispatch` QEMU gate green before declaring close** (the BUILD-W1 coverage-gap fix Jarvis just landed; the command flow is in `Plans/RUNBOOK-second-self-hosted-runner.md`). If you add any hardware-only/default-off scenario, gate it out of the QEMU shards.

## 4. Scope guard
RoPE + SwiGLU-FFN wired into the layer scaffold on real weights, validated per-op. **Do NOT** implement GQA-attention or the KV cache (8.5), run full end-to-end generation, implement quantized-matmul (Phase 9), or wire into `generate.rs`'s loop beyond the layer scaffold. Reuse 8.4a/8.3/existing ops. No new `unsafe`/invariant; raw-source references.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; re-run the float-oracles + family; **run the pre-merge `workflow_dispatch` QEMU gate green** (kernel-touching); push (CI host gate). PM `pm-merge-check` → `--ff-only` → ledger. Next: **8.5** — GQA attention + KV cache (Prometheus's `phase-8.5-kvcache-governed-tensors` design note feeds it), completing the forward pass + token generation on real weights.

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-phase-8-build-8.4-rope-swiglu-forward-janus.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-build-8.4-rope-swiglu-forward-janus` off `origin/main` (`7f1c718` — has 8.3 + 8.4a). **Start now, no wait.**
- **One-line:** Wire **RoPE + the SwiGLU FFN into the forward path on real Qwen weights** — reuse your 8.4a ops + 8.3's dequant + the existing matmul. Two float-oracles (I-12, not round-trip): real-weight **SwiGLU-FFN** block (`down·(silu(gate·x)⊙up·x)`) vs `llama.cpp`, and **RoPE on real projected Q/K** (pin the pair-interleaving convention) vs `llama.cpp` — raw-source the references, **never WebFetch**. Build the per-layer scaffold (RMSNorm→proj→RoPE→[attention=unsupported, 8.5]→RMSNorm→SwiGLU→residual) but **NO attention / KV cache / end-to-end gen (that's 8.5)**. Host-tier, `gguf`/`ai-generate` gate (kill-switch ≡ today), no net-new math/`unsafe`/invariant. **Touches `kernel/` ⇒ run the pre-merge `workflow_dispatch` QEMU gate green before close** (per the runbook). fmt/clippy clean.
