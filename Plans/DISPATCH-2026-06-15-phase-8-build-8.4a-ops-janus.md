<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.4a — the net-new op kernels (RoPE / SiLU / SwiGLU), standalone + KAT'd → Janus

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** Janus (new branch off `origin/main` HEAD: `codex/phase-8-build-8.4a-ops-janus`).
**Status:** DISPATCHED. **Host-tier only — pure op kernels, no model, no QEMU, no silicon.** A *parallel, non-colliding* slice: build the three net-new transformer ops as **standalone, byte-oracle-KAT'd kernels now**, while Jarvis does 8.2 (the GGUF importer + tokenizer). They get *wired into* the real forward pass at **8.5**; this build proves the math in isolation.
**Why it doesn't get in the way:** these live in `kernel/src/ai/ops/` — **disjoint from 8.2's `gguf`/`tokenizer` modules and from 8.1's `generate` module.** The only shared file is `tools/ai-ops-tests/src/lib.rs` (you each *append* KATs — additive, a trivial rebase reconcile at merge, like the 8.1/LO `test_harness` reconcile). Stay out of the importer, the decode loop, and the AI hot-path and there's zero collision.

## 0. The decision this build makes
The gap analysis (FINDINGS §2) named 6 net-new ops for a real decoder LLM. Three are **standalone math kernels** with no dependency on weights-loading or the decode loop — build + KAT them now so 8.5 (GQA attention) is just *wiring proven ops*, not implementing them under integration pressure. (The other net-new pieces — Q4_K dequant/quantized-matmul = 8.3; GQA-attention+KV-cache = 8.5 — are *not* this build.)

## 1. Authoritative reference (on `main`)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §2 the net-new ops with raw-source file:line refs
```
The existing op set + soft-float core is `kernel/src/ai/ops/mod.rs` (+ `soft_f32.rs`) — you already have `exp`/`tanh`/etc. from the transformer ops (gelu/softmax). Add the new ops in the same pattern (an `OpKind` variant or a `*_logic` fn + the kernel), host-tested like the existing `valid_*` oracles.

## 2. ISCs — each a standalone op + a published/hand-computed KAT (I-12, NOT round-trip)
- **ISC-1 — SiLU** (Swish): `silu(x) = x · σ(x) = x / (1 + e^(−x))`, elementwise (reuse the soft-float `exp`). KAT: hand-computed expected outputs for a fixed input vector (deterministic; like the gelu/relu oracles). Raw-source ref: `ggml-cpu/vec.h:1046-1047`.
- **ISC-2 — SwiGLU** (the gated FFN activation): `swiglu(gate, up) = silu(gate) ⊙ up` (elementwise), the fused pattern an FFN uses as `down · (silu(gate_proj·x) ⊙ up_proj·x)` — build the **activation** (`silu(gate)⊙up`); the matmuls are the existing op. KAT: hand-computed over fixed `gate`/`up` vectors. Raw-source ref: `ggml-cpu/vec.cpp:417-452` (`ggml_vec_swiglu_f32`), `ggml.c:3012-3024`.
- **ISC-3 — RoPE (rotary positional, YaRN-capable; Qwen2.5 uses `rope_theta = 1e6`)**: rotate the Q/K pair dimensions by position-dependent angles. Build the rotate-pairs kernel + the YaRN frequency scaling. KAT: RoPE is **deterministic trig** — assert the rotated output for a fixed `(input, position, theta, head_dim)` against the formula computed independently (hand/Python, an *independent* primitive over the formula — not the kernel under test). Raw-source refs: `ggml.c:4132` (`ggml_rope_impl`), `ggml-cpu/ops.cpp:5794-5810` (`rotate_pairs`), `:5690-5705` (`rope_yarn`). **Trap: the pair-interleaving convention (adjacent vs split-half) and the YaRN/theta scaling** — KAT both explicitly against the reference convention Qwen2.5 uses.

## 3. Gate (host only)
- Host KATs green for SiLU, SwiGLU, RoPE (each vs hand-computed/reference values — the I-12 oracle, never round-trip); the existing op family unaffected; `fmt`/`clippy -D warnings`. **No QEMU** (pure host ops, like the existing op oracles). Builds on+off any inference feature gate.

## 4. Scope guard
The **three op kernels + their KATs only.** **Do NOT** wire them into the forward pass / decode loop (that's 8.5); **do NOT** touch the GGUF importer or tokenizer (8.2 — Jarvis); **do NOT** touch `generate.rs`/`exec::run` (8.1) or the AI hot-path; **do NOT** implement dequant/quantized-matmul (8.3) or attention/KV-cache (8.5). Stay in `ops/` + `ai-ops-tests`. **Raw-source the RoPE convention + any reference vectors — never WebFetch the exact numbers** (the raw-source-KAT rule; the q4_K 72→144 catch is why). No new invariant, no new `unsafe`.

## 5. Merge (PM)
Close-gate: rebase onto current `main` (**reconcile `ai-ops-tests/src/lib.rs` if 8.2 landed first — both append KATs; keep both, drop nothing**), re-run the op KATs, push (CI runs the cheap host gate). PM `pm-merge-check` → `--ff-only` → ledger (ANSPEC §11.x op note; fold into 8.5's ADR or a short ADR-056). These ops then sit ready for 8.5 to wire into GQA attention.

---

## Relay — what to tell Janus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-phase-8-build-8.4a-ops-janus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.4a-ops-janus.md
  git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §2 op refs
  ```
- **Branch:** `codex/phase-8-build-8.4a-ops-janus` off `origin/main` HEAD.
- **One-line:** Build the 3 net-new transformer ops **standalone + host-KAT'd**, in parallel with Jarvis's 8.2 (disjoint — you're in `ops/`, he's in `gguf`/`tokenizer`; only `ai-ops-tests/lib.rs` overlaps, additive). **SiLU** (`x·σ(x)`), **SwiGLU** (`silu(gate)⊙up`), **RoPE** (YaRN, `theta=1e6`, watch the pair-interleaving convention) — each a kernel in `kernel/src/ai/ops/` + a hand-computed/reference KAT (I-12, not round-trip; raw-source the RoPE convention + vectors, never WebFetch). Refs in gap-analysis §2 (file:line). **Do NOT wire into the forward pass (8.5), touch the importer (8.2), or the decode loop (8.1)** — just the ops + KATs. Host-only, no QEMU, no new invariant/`unsafe`. fmt/clippy clean, push (let CI run). These land ready for 8.5 to wire into GQA attention.
