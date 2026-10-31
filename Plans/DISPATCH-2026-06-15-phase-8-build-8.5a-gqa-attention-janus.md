<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.5a — the GQA causal-attention compute kernel, standalone + KAT'd → Janus

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** Janus (new branch off `origin/main` HEAD: `codex/phase-8-build-8.5a-gqa-attention-janus`).
**Status:** DISPATCHED. **Host-tier only — pure compute kernel, no model, no QEMU, no silicon.** The same clean-slice pattern as 8.4a: build the **attention *compute*** standalone + byte-oracle-KAT'd now, **without** the KV cache (that's 8.5's stateful part). Parallel + non-colliding with Jarvis's 8.2.
**Why it doesn't get in the way:** it lives in `kernel/src/ai/ops/` (attention as an op composed from the existing matmul/softmax + a causal mask + GQA head-broadcast) — **disjoint from 8.2's `gguf`/`tokenizer` modules, from 8.1's `generate`, and from W1.** Only `tools/ai-ops-tests/src/lib.rs` overlaps (you append KATs — additive, trivial rebase reconcile, same as 8.4a). Stay in `ops/` + the tests.

## 0. The decision this build makes
Prove the attention math in isolation so **8.5** (the integration build) is just *wiring it + adding the KV cache* — not also implementing attention under integration pressure. This is the **single forward attention** (full sequence, no cache): the compute a prefill pass does. The **KV cache** (incremental append/reuse for per-token decode — *and the "KV-cache-as-governed-tensors" residency showcase, the VISION thesis*) is explicitly **8.5, not this build.**

## 1. Reference (on `main`)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §1 Qwen2.5-0.5B (14 q-heads / 2 kv-heads, head_dim), §2 GQA attention ref (qwen2.cpp:82-105 build_qkv/build_attn)
```
Build it like 8.4a's ops — in `ai/ops/`, composed from the existing `matmul`/`softmax`/`transpose` where natural, host-tested with hand-computed oracles. Qwen2.5-0.5B's shape: **14 query heads, 2 KV heads** (GQA group = 7), `head_dim` per the config.

## 2. ISCs — each a hand-computed KAT (I-12, NOT round-trip)
- **ISC-1 — Scaled dot-product attention (single head, causal).** `scores = (Q·Kᵀ)/√head_dim` → **causal mask** (position *i* attends only to keys ≤ *i*; masked entries → −∞ before softmax) → `softmax` over keys → `out = softmax·V`. KAT: a tiny fixed Q/K/V (e.g. seq=3, head_dim=4) with the attention output **hand-computed** and asserted.
- **ISC-2 — GQA head expansion.** `n_kv_heads < n_q_heads`: each KV head is shared across a group of `n_q/n_kv` query heads. KAT: a small GQA config (e.g. 4 q-heads / 2 kv-heads) asserting each query head attends against the **correct shared KV head** (a wrong-grouping bug must fail the KAT). **Trap: the q-head→kv-head grouping/index math** — pin it explicitly.
- **ISC-3 — Causal-mask correctness.** Assert masked (future) positions contribute exactly zero to the output (a small seq where the unmasked-vs-masked difference is hand-checkable). **Trap: off-by-one on the diagonal** (does position *i* attend to *i*? yes) — KAT it.
- **ISC-4 — Standards.** A standalone op/kernel in `ai/ops/` (an `OpKind` variant or a `*_logic` fn + kernel, the 8.4a pattern); host KATs (hand-reference, never round-trip); **no KV cache, no `exec`/forward-pass wiring, no feature beyond what 8.4a used**; explicit `unsupported` arms in `exec_ops`/`accel_sim` for exhaustiveness (like 8.4a); no new invariant, no new `unsafe`. ANSPEC §11.x op note; fold the ADR into **8.5's ADR-056** (or a short standalone note).

## 3. Gate (host only)
Host KATs green for the three (single-head causal SDPA, GQA grouping, causal-mask zeroing — each vs hand-computed reference); the existing op family unaffected (104/104 + the new ones); `fmt`/`clippy -D warnings`; builds on+off. **No QEMU** (pure host op, like 8.4a + the existing op oracles).

## 4. Scope guard
The **attention compute kernel + its KATs only.** **Do NOT** build the **KV cache** (append/reuse — that's 8.5, *and its residency-governance showcase*); **do NOT** wire attention into `exec::run`/the forward pass (8.5); **do NOT** touch the GGUF importer/tokenizer (8.2 — Jarvis), `generate.rs`/the decode loop (8.1), or W1; **do NOT** dequant (8.3). Stay in `ops/` + `ai-ops-tests`. **Raw-source the GQA grouping convention** Qwen uses (FINDINGS §1/§2) — never WebFetch exact numbers; the KATs are hand-computed. No new invariant/`unsafe`.

## 5. Merge (PM)
Close-gate: rebase onto current `main` (**reconcile `ai-ops-tests/src/lib.rs` — 8.4a is already there and 8.2 may land first; keep all KATs, drop none**), re-run the attention KATs + the op family, push (CI runs the cheap host gate). PM `pm-merge-check` → `--ff-only` → ledger (ANSPEC §11.x; ADR-056). With 8.5a + 8.4a banked, **8.5 = wire RoPE→attention→SwiGLU into the forward pass + add the KV cache** (the governed-tensors showcase) — pure integration of proven ops.

---

## Relay — what to tell Janus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-phase-8-build-8.5a-gqa-attention-janus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.5a-gqa-attention-janus.md
  git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §1/§2 shapes + GQA ref
  ```
- **Branch:** `codex/phase-8-build-8.5a-gqa-attention-janus` off `origin/main` HEAD (has 8.4a; disjoint from Jarvis's 8.2 except the additive `ai-ops-tests`).
- **One-line:** Build the **GQA causal-attention *compute* kernel standalone + host-KAT'd** (the 8.4a pattern), parallel to 8.2. Scaled-dot-product attention `(Q·Kᵀ/√d)` → **causal mask** → softmax → `·V`, with **GQA head expansion** (Qwen2.5-0.5B = 14 q / 2 kv heads, group=7). Three hand-computed KATs (I-12, not round-trip): single-head causal SDPA, the **q→kv grouping math** (trap), and **causal-mask zeroing + diagonal off-by-one** (trap). **NO KV cache, NO forward-pass wiring — those are 8.5** (incl. the KV-cache residency showcase). In `ai/ops/` + `ai-ops-tests` only; explicit unsupported arms in exec_ops/accel_sim; no new invariant/`unsafe`; host-only, no QEMU. fmt/clippy clean, push (let CI run). Lands ready for 8.5 to wire RoPE→attention→SwiGLU + the cache.
