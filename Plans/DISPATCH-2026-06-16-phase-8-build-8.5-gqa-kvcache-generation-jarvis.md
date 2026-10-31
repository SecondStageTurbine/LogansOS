<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.5 — GQA attention + governed KV cache → first real-model token generation → Jarvis

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Jarvis (parked for this; strong fit — he built the CI/QEMU gate this build must pass). New branch off `origin/main` HEAD (`c6712a9`): `codex/phase-8-build-8.5-gqa-kvcache-generation-jarvis`. *(Janus authored 8.4's scaffold + 8.5a's attention op + 8.3's dequant — ping her for the forward-pass seams.)*
**Status:** DISPATCHED. **Host-tier KATs + the pre-merge QEMU gate. THE PHASE-8 MILESTONE: token-in → logits-out → real autoregressive generation on Qwen2.5-0.5B.** Pure integration of proven, ratified pieces.

## 0. The decision this build makes
Complete the forward pass: fill 8.4's `AttentionUnsupported` seam with **8.5a's GQA causal attention**, add the **governed KV cache**, and run **token-in → all 24 layers → logits → sample (8.1) → next token** on the real model. This is where LogansOS first **generates text from a real LLM** — and where the KV cache becomes **governed kernel memory** (the VISION thesis). The design is **already ratified** — build it, don't re-decide it.

## 1. Authoritative design — RATIFIED (read first, build to it)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-16-phase-8.5-kvcache-governed-tensors.md   # §10 = the ratified decisions; Option A; the equivalence KAT; the risk register
```
Reuse: **8.4** `forward_logic.rs` (the scaffold + RoPE'd Q/K + the attention seam), **8.5a** GQA attention op, **8.3** `gguf_dequant_logic` (real weights → f32), **8.1** decode loop + sampler. No net-new op math.

## 2. ISCs (the ratified design is law here)
- **ISC-1 — Wire GQA attention into the forward pass.** Fill 8.4's `AttentionUnsupported` with 8.5a's scaled-dot-product GQA causal attention (14 q-heads / 2 kv-heads, group 7), consuming the **RoPE'd Q/K from 8.4** and the KV cache. **RoPE is applied at cache-WRITE time, never at read** (THINK §9 risk note — `llama-graph.cpp:2340`); document that invariant.
- **ISC-2 — The governed KV cache (Option A, ratified).** Per-layer K+V as **fixed-size arena tensors pre-reserved at generation start** (sized for the chosen max context; f32; **single-stream**), **registered with the governance substrate at pre-reserve** so I-9 (production tensors)/I-13 (liveness)/the reservation accounting hold **unmodified** — the substrate sees a *constant* byte footprint; `used_len` tracked internally. Append K/V per token; read for attention. **Confirm NO new invariant is needed** (the pre-reserved tensor is governed by existing I-9/I-13 — state this explicitly; if you find a genuine new cross-cutting invariant, STOP and BLOCKER-to-PM per Rule-2).
- **ISC-3 — Full forward + generation.** token-in → 24× decoder layers → `output_norm` → `lm_head` → logits → sample (8.1) → next token, autoregressive, on real Qwen2.5-0.5B-Q4_K_M weights (via 8.3 dequant).
- **ISC-4 — The gates (I-12).** (a) **Equivalence KAT (self-contained, ratified):** cached decode of N tokens == uncached recompute-all, logits equal within f32 tolerance (~2⁻¹²) at each step — no external dep. (b) **Real-generation oracle:** generate N tokens from a fixed prompt; assert the **top-1 token ids match `llama.cpp`** for the same model+prompt (raw-source the reference run; **never WebFetch** exact ids). The position/off-by-one + RoPE-at-write traps (THINK §9) each get an assertion.
- **ISC-5 — Standards + kill-switch.** `kv_cache_enabled` boolean — **disabled ≡ recompute-all ≡ 8.5a uncached, byte-identical** (the Rule-1 regression). Host-testable (`*_logic` + `ai-ops-tests`); behind the `gguf`/`ai-generate` gate; reuse 8.5a/8.4/8.3/8.1 (no net-new math, no quantized-matmul = Phase 9); `fmt`/`clippy -D warnings`; **no new `unsafe`**; no AI-hot-path/governance-lock churn beyond the KV registration (which uses the existing mint/slot path). Open ANSPEC §11.x; ADR-056 (or fold into 055). Update the bible Phase-8 ledger pointer in the RETURN.

## 3. Gate
- Host: the **equivalence KAT** (cached==uncached) + the **real-generation top-1-vs-llama.cpp** oracle green; the op + KAT family unaffected; kill-switch regression (disabled ≡ uncached); builds on+off the feature; `fmt`/`clippy -D warnings`.
- **Kernel-touching ⇒ run the pre-merge `workflow_dispatch` QEMU gate green (all 4 shards) before declaring close** — the gate you just built; the command flow is in `Plans/RUNBOOK-second-self-hosted-runner.md`. Any new scenario stays out of the QEMU shards unless it's host-runnable (no dongle/hardware dep).

## 4. Scope guard
GQA attention wired + the pre-reserved governed KV cache + full generation + the equivalence/real-gen KATs **only**. **Do NOT** build active residency eviction/prefetch (**8.5b** — DEFERRED per ratification; the full cache fits at 0.5B/4096) or segment-level eviction (**8.5c** — design-only), implement quantized-matmul (Phase 9), or add multi-stream. Reuse the proven pieces. No new `unsafe`/invariant (KV uses existing I-9/I-13).

## 5. Merge (PM)
Close-gate: rebase onto current `main`; re-run the equivalence + real-gen KATs + family; **pre-merge `workflow_dispatch` QEMU green**; push (CI). PM `pm-merge-check` → `--ff-only` → ledger (Article 13 — **Phase-8 inference arc COMPLETE: first real-model token generation** — + ANSPEC §11.x + ADR-056). This **closes Phase-8's inference arc** — the model talks. Next: Phase-8 arc-2 (the userland/serving surface — Prometheus's Arthur design feeds it) + Phase 9 (quantized matmul — Prometheus's FINDINGS feeds it).

**Disk reclaim (after PM confirms merge):** once this branch is on `origin/main` and the PM confirms it, delete this build's working tree to reclaim its `target/` — `git worktree remove <path>` (then `git worktree prune` if needed) if it's a worktree, or delete the folder if a standalone clone. **Never delete a tree whose branch is unpushed/unmerged** — verify the work is on `origin/main` first. (`cargo clean` is the interim if you want to keep the checkout — `target/` is ~80% of the folder.) See CLAUDE.md "Disk hygiene — reclaim the build tree after merge."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-phase-8-build-8.5-gqa-kvcache-generation-jarvis.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-build-8.5-gqa-kvcache-generation-jarvis` off `origin/main` (`c6712a9` — has 8.4 + 8.5a + 8.3 + 8.1).
- **One-line:** THE milestone — **first real token generation.** Fill 8.4's `AttentionUnsupported` with 8.5a's GQA attention (consume the RoPE'd Q/K + the KV cache), add the **governed KV cache per the RATIFIED THINK** (`Plans/THINK-2026-06-16-phase-8.5-kvcache-governed-tensors.md` §10): **Option A pre-reserved** per-layer K+V arena tensors, f32, single-stream, **registered at pre-reserve so I-9/I-13/reservation hold unmodified** (constant footprint; **no new invariant** — if you find one, BLOCKER-to-PM). **RoPE applied at cache-WRITE, never read.** Then full forward → sample (8.1) → autoregressive gen on real Qwen2.5-0.5B. Two I-12 gates: the **cached==uncached equivalence KAT** (self-contained, ~2⁻¹²) and **generated top-1 == llama.cpp** (raw-source, never WebFetch). Kill-switch `kv_cache_enabled` (off ≡ recompute-all ≡ 8.5a). **NO active eviction (8.5b deferred), NO segment eviction (8.5c design-only), NO quantized-matmul (Phase 9), NO multi-stream.** Kernel-touching ⇒ **run the pre-merge QEMU gate green before close** (the one you built). fmt/clippy clean, no new `unsafe`.
