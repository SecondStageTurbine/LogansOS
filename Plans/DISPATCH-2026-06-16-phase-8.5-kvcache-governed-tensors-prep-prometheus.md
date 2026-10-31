<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8.5 prep — the KV-cache-as-governed-tensors design note → Prometheus

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Prometheus (deep-research/design lane). **Fresh** branch off `origin/main` HEAD: `prometheus/phase-8.5-kvcache-governed-tensors-design` *(start clean off `origin/main` — abandon the old `phase-9` branch; its FINDINGS is already on main via cherry-pick `06d191d`)*.
**Status:** DISPATCHED. **Design + raw-source research only — NO production code, NO `ci.yml`.** Output is one design doc under `Plans/`. Fully parallel to 8.3 (Janus), W2 (Cerebro), CI (Jarvis) — touches only `Plans/`.

## 0. The decision this sets up
8.5 = "wire GQA-attention into the forward pass **+ add the KV cache**, the governed-tensors residency showcase." The KV cache is where Phase 8 stops being "an inference engine that happens to run in our kernel" and becomes the **VISION thesis**: the per-token attention state is **kernel-governed tensor memory** under the same invariants (I-9 production tensors, I-13 liveness, the reservation/`ARENA`, the residency daemon's hot/prefetched prediction) as everything else — *intelligence as a kernel primitive*. This note designs that mapping so 8.5 is *wiring a decided design*, not inventing it under integration pressure.

## 1. What to work out (the design questions)
- **What the KV cache *is*:** per-layer K and V tensors, shape ~`[n_kv_heads=2, head_dim, seq_len]` for Qwen2.5-0.5B, **growing by one position per generated token** across a generation. Quantify the size growth (per-token bytes, worst-case at the model's context length).
- **The growth tension (the core design problem):** the existing tensor-governance substrate is built for **fixed-size production weights** (minted once, deduped, slot-keyed). A KV cache **grows monotonically during decode**. How does a growing tensor live under I-9/I-13/the reservation? Options to weigh: pre-reserve max-context capacity vs grow-and-remint vs a ring buffer; which preserves the invariants (no mid-decode realloc that breaks a Handle; liveness still probeable) with least churn.
- **The residency showcase:** long-context KV caches are large; the residency daemon could **govern** them — keep recent positions hot, evict/spill cold ones (recompute or page). Is that in scope for 8.5, or designed-now-built-later? Decide and say so. This is the part that makes the cache a *showcase* and not just a buffer.
- **Kill-switch shape:** 8.5 disabled ≡ 8.5a's standalone attention with **no cache** (recompute-all per token — correct but slow); enabled = the cached path. The disabled≡prior regression (Rule-1).
- **Correctness oracle (I-12):** how 8.5 KATs the cached path == the uncached path (cached decode of N tokens produces byte-identical logits to recompute-all) — the equivalence gate.
- **Raw-source** how llama.cpp/ggml lays out its KV cache (the `kv_cache`/`kv_self` struct, contiguous-vs-ring, the per-layer layout) — `git`/`curl` the source, **never WebFetch exact layouts/numbers**; cite file:line/SHA. Use it as a reference point, not a thing to copy (we govern ours).

## 2. Deliverable
`Plans/THINK-2026-06-16-phase-8.5-kvcache-governed-tensors.md`:
- the cache shape + growth math for Qwen2.5-0.5B;
- the growth-vs-governance options table with a **recommendation** (which representation, why, which invariants it preserves);
- the residency-management scope call (8.5 vs later) with rationale;
- the kill-switch + the cached==uncached equivalence-KAT plan;
- the raw-sourced llama.cpp KV-layout reference (file:line/SHA);
- **open decisions for the PM/PL to ratify** before 8.5 is dispatched.

## 3. Scope guard
Design + `Plans/` doc **only**. **Do NOT** write production code, touch `ci.yml`/kernel/`ai-ops-tests`, or pre-empt 8.3/8.4 (those land the f32 forward path 8.5 builds on). Start a **clean branch off `origin/main`** (not the contaminated phase-9 branch). Raw-source any exact layout; flag second-hand numbers. **⚠ PUSH the branch before reporting done.**

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-phase-8.5-kvcache-governed-tensors-prep-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/phase-8.5-kvcache-governed-tensors-design` off `origin/main` (fresh — drop the old phase-9 branch).
- **One-line:** Design **the KV cache as governed tensors** for 8.5 — the residency showcase / VISION thesis. Work out: the cache shape + per-token growth for Qwen2.5-0.5B; the **growth-vs-governance tension** (a monotonically-growing tensor under I-9/I-13/reservation built for fixed weights — pre-reserve vs grow-remint vs ring; recommend one); whether the residency daemon governs eviction in 8.5 or later; the kill-switch (disabled ≡ uncached recompute-all) + the **cached==uncached equivalence KAT** (I-12); and the raw-sourced llama.cpp KV layout (file:line/SHA, **never WebFetch**). Deliver `Plans/THINK-2026-06-16-phase-8.5-kvcache-governed-tensors.md` with a recommendation + open decisions for ratification. **Design doc only — no code, no `ci.yml`.** Push before reporting done.
