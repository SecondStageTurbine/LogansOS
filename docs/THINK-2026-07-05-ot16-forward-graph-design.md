<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-16 — data-driven forward-graph design (the DeltaNet appetite decision)

**Date:** 2026-07-05
**Source:** `DISPATCH-2026-07-05-ot16-forward-graph-design-think-prometheus.md`; OT-38 candidate THINK; Q1 FINDINGS (`RESEARCH-2026-07-04-gated-deltanet-reference-FINDINGS.md`); `forward_logic.rs` (current hardcoded Qwen2 forward); ANSPEC §19.2; Bible Articles 1.2, 5.7, 6.6; ADR-001
**Status:** **RATIFIED (PL, 2026-07-05) — all three points as recommended:** (1) layer-kind enum-dispatch schedule; (2) `.neom` format-version bump with the ADR-028-family byte oracle when B1 starts; (3) **Phase-9 slice, sequenced AFTER the 1.5B/3B config-swap** (which remains the immediate capability step). The B1→B2→B3 plan (§6) is the dispatchable decomposition; B1 opens when the sequencing point is reached and a seat frees.

---

## 0. The problem

`kernel/src/ai/forward_logic.rs` is Qwen2-specific: the op primitives (matmul, softmax, RoPE, RMSNorm, SwiGLU) are substrate-stable, but the *model-graph orchestration* — the sequence of operations per layer, the attention vs FFN wiring, the KV cache management — is hardcoded in ring 0. A new architecture (Gated DeltaNet's linear-attention hybrid) would be a kernel patch with `if qwen / if delta_net` branches. Per ANSPEC §19.2 and Article 5.7, the forward graph should be **data-driven from the `.neom` description**: a new architecture becomes a new `.neom`, not new kernel branches.

**Q1 is RESOLVED (YES)** — a correct Gated DeltaNet reference exists (HF `torch_recurrent_gated_delta_rule`). The remaining question is Q2: do we build OT-16? Nobody can answer an appetite question about an unscoped build. This THINK makes it concrete.

---

## 1. D1 — The graph-description shape

### 1.1 What stays compiled-in (the op primitives)

The kernel's `ai/ops/` module is the substrate: matmul (scalar + AVX2, quantized), RMSNorm, RoPE, SwiGLU, softmax, causal attention. These are **compiled Rust functions** — not data-driven. They don't need to be; they're proven, byte-oracled, and architecture-independent. A new architecture doesn't add new primitives; it adds new *wiring* and possibly a few new ops (the DeltaNet recurrence).

### 1.2 What becomes data (the layer schedule)

The minimum expressiveness that covers {Qwen2 today, Gated DeltaNet hybrid tomorrow} is a **layer-kind schedule**: an ordered list of layer descriptors, each specifying its kind and parameters. This is NOT a general NN compiler (no dynamic control flow, no arbitrary tensor reshaping). It's a typed table the kernel walks sequentially.

**Shape:**
```
LayerSchedule {
    version: u32,          // format version for forward-compat
    layer_count: u32,
    layers: [LayerDescriptor; layer_count],
}

enum LayerKind {
    Qwen2AttentionFFN,    // existing: attn + FFN, KV cache
    GatedDeltaNetLinear,   // new: delta-rule recurrence + FFN, recurrent state
}

struct LayerDescriptor {
    kind: LayerKind,
    config: LayerConfig,   // tagged union: Qwen2LayerConfig | GDNLayerConfig
}
```

The `.neom` already carries per-layer weight tensors with typed names. The schedule tells the kernel *which layer kind to apply to which tensor group*. The weights are already there; the schedule is metadata.

### 1.3 Why "layer-kind schedule" and not full graph

Qwen2 layers are homogeneous (24 identical layers). Qwen 3.5 hybrids interleave two layer kinds (e.g., layers 0-3: full attention, layers 4-7: linear attention, etc.). A layer-kind schedule is the minimal data structure that covers both:
- **Qwen2:** 24 × `Qwen2AttentionFFN` — the schedule reproduces today's hardcoded loop
- **Qwen 3.5 hybrid:** interleaved `Qwen2AttentionFFN` / `GatedDeltaNetLinear` — the schedule drives the interleave

A full graph (arbitrary DAG of ops with tensor dependencies) would be a ring-0 NN compiler — exactly what ANSPEC §19.2 warns against. The layer-kind schedule is a **typed enum dispatch**, not a graph interpreter. It's a `match` on data, not a VM.

### 1.4 Where the schedule lives

The schedule is carried in the `.neom` header (a new section, version-gated). The GGUF→`.neom` importer extracts it from the model architecture config. The kernel validates it at load time (§2) and walks it during forward.

**Open item:** `.neom` format-version bump for the schedule section — an ADR-028-family change with its own byte oracle. Name it, don't do it here.

---

## 2. D2 — Mechanism-not-policy (Article 6.6) + validation invariants

### 2.1 Why a graph walker is mechanism

The kernel does not decide *what* architecture to run. It executes a validated schedule that the `.neom` supplies. This is the same mechanism-not-policy line as the A1 logits-vs-bitmask decision: the kernel provides the execution substrate; the model file provides the configuration. The kernel's job is to **execute correctly**, not to **choose what to execute**.

### 2.2 Load-time validation invariants

A malformed or hostile schedule must be rejected at load, not crash ring 0. Validation happens once during `.neom` load (before any forward execution):

1. **Version gate:** `schedule.version` must match a supported range. Unknown versions → reject with `ModelError::UnsupportedScheduleVersion`.
2. **Layer count bounds:** `layer_count > 0 && layer_count <= MAX_LAYERS` (currently 128, covers all deployed models). Prevents allocation overflow.
3. **Known kinds only:** Each `LayerDescriptor.kind` must be a recognized variant. Unknown kind → reject with `ModelError::UnknownLayerKind`.
4. **Config consistency:** Each layer's config fields are validated (non-zero dimensions, valid head counts, etc.). Same validation as `Qwen2LayerConfig::from_model_config()`.
5. **Weight tensor presence:** For each layer, the expected weight tensors exist in the `.neom` tensor registry. Missing tensor → reject with `ModelError::MissingTensor`.
6. **State budget:** The total recurrent state size (for GDN layers) must fit within the model's declared memory budget. Prevents OOM during forward.
7. **No dynamic control flow:** The schedule is a static ordered list. No branching, no loops, no conditional execution based on tensor values. This is enforced by the data structure itself (it's an array of descriptors, not a graph with control-flow edges).

These invariants are **mechanism**: they define what a valid schedule looks like. They are not policy: they don't prefer one architecture over another.

---

## 3. D3 — The double kill-switch proof

### Kill-switch 1: OFF ≡ hardcoded Qwen2, bit-for-bit

When the data-driven walker is disabled (feature flag `ot16-graph-walker` off), the hardcoded Qwen2 forward runs exactly as today. No code path changes. The OT-30 top-1 parity oracle proves this: the same model, same output, same bytes.

**Proof:** The feature flag gates the walker entirely. Off = the exact current `forward_layer_with_attention_output()` loop. No diff in behavior.

### Kill-switch 2: ON with Qwen2 schedule ≡ hardcoded, bit-for-bit

When the walker is enabled and given a Qwen2 schedule (24 × `Qwen2AttentionFFN`), it must produce byte-identical output to the hardcoded path. This is the **real acceptance test**: does the data-driven walker reproduce today's forward exactly?

**Proof:**
1. Run hardcoded forward on a known input → capture output bytes
2. Run walker forward with equivalent Qwen2 schedule → capture output bytes
3. Byte-identical = pass. Any difference = walker bug.

This is the I-1 byte-oracle applied to the graph generalization: placement changes (hardcoded → data-driven), but the computed bytes don't. The OT-30 parity oracle also stays green: same model, same top-1 tokens.

**Regression family:** Both kill-switches run in the host KAT suite (no QEMU needed for the walker's correctness — it's a dispatch table over proven ops). The QEMU gate runs the full forward at NEO_SMP=4 to prove parallelism compatibility (§4).

---

## 4. D4 — OT-34 threaded matmul compatibility

### 4.1 Which ops thread

From `RESEARCH-2026-07-04-deterministic-parallel-matmul-prior-art-FINDINGS.md`: the matmul (both F32 and quantized variants) is the primary threading target. OT-34 implements deterministic column-partition fork-join: each CPU owns disjoint output columns, accumulates sequentially, joins with a barrier. Byte-identical regardless of thread count.

**All other ops are serial:** RMSNorm, RoPE, SwiGLU, softmax, causal attention. These are per-token or per-head operations that don't benefit from threading at our scale (single-token decode). The matmul is the compute-dominant op (~90% of forward time).

### 4.2 How the schedule preserves determinism

The layer-kind schedule doesn't change *how* ops execute — it changes *which* ops are called and in what order. The threading discipline (deterministic column-partition for matmul, serial for everything else) is an op-level concern, not a schedule-level concern. The walker dispatches to the same threaded matmul that OT-34 proved deterministic.

### 4.3 DeltaNet recurrence determinism

The Gated DeltaNet recurrence (`torch_recurrent_gated_delta_rule`) is **inherently sequential in the state dimension**: each token updates the recurrent state `S`, and the next token reads the updated `S`. This is a data dependency — you can't parallelize across tokens.

**This doesn't break determinism:** sequential execution IS deterministic. The determinism concern from OT-34 was about *parallel* execution producing different results at different thread counts. A sequential recurrence has no parallelism, hence no determinism risk. It's slow (one token at a time), but correct.

**Prefill optimization:** The HF reference provides `torch_chunk_gated_delta_rule` for parallel prefill (Yang et al.'s chunking algorithm). This is a future optimization (OT-34 follow-on), not the initial implementation. The initial decode path is sequential and deterministic by construction.

---

## 5. D5 — The DeltaNet op inventory

### 5.1 New ops required

From the HF `torch_recurrent_gated_delta_rule` reference and `Qwen3_5GatedDeltaNet.forward()`:

| Op | What it does | Byte-oracle/KAT plan | Parity anchor | Rough size |
|----|-------------|---------------------|---------------|------------|
| **GDN recurrent step** | Gate → kv-memory → delta → state update → output. Per-token recurrence over state `S`. | Host KAT: port `torch_recurrent_gated_delta_rule` to Rust, run on known input, capture output bytes + final state `S`. Independent hand-laid fixture for a 2-block input. | HF `torch_recurrent_gated_delta_rule` (pure PyTorch) | ~200 lines (the recurrence is compact: 5 matrix ops per token) |
| **GDN causal conv1d** | Depthwise conv1d on QKV projection, SiLU activation. | Host KAT: `torch_causal_conv1d_update` from HF source. Verify against known convolution fixture. | HF `torch_causal_conv1d_update` | ~80 lines (wrapper around existing conv infrastructure) |
| **GDN gating projections** | `in_proj_qkv`, `in_proj_z`, `in_proj_b`, `in_proj_a` — four projections from hidden to QKV/z/b/a. | Same matmul KAT as existing ops (the math is identical: `matmul_vec_weight`). No new oracle needed. | Existing matmul KATs | ~40 lines (projection dispatch) |
| **GDN L2 normalization** | L2 norm of Q and K before recurrence. | Host KAT: HF `l2norm()` function. Verify on known vector. | HF `l2norm` | ~15 lines |
| **GDN RMSNormGated output** | RMSNorm + SiLU gate from `z` projection. | Host KAT: HF `Qwen3_5RMSNormGated`. Verify against known input. | HF `Qwen3_5RMSNormGated` | ~30 lines (extends existing RMSNorm) |
| **GDN recurrent state cache** | Maintains state `S` across tokens. Shape: `(batch, heads, k_dim, v_dim)`. | No KAT needed — it's a buffer management concern. Verified by the recurrent step KAT. | HF cache handling in `Qwen3_5GatedDeltaNet` | ~50 lines (state buffer + position tracking) |

**Total new code:** ~415 lines of new op implementations, plus the layer-kind schedule walker (~150 lines). Total: ~565 lines. This is a Phase-9-scale build, not a Phase-2 monster.

### 5.2 GGUF import path (Q5)

llama.cpp has `GGML_OP_GATED_DELTA_NET` and `LLM_ARCH_QWEN35`. GGUF files for Qwen 3.5 carry the GDN tensors. The GGUF→`.neom` importer needs to:
1. Recognize `qwen3_5` architecture
2. Extract the layer-kind schedule from the config (which layers are `linear_attention` vs `full_attention`)
3. Map GDN weight tensors to `.neom` tensor names
4. Import the recurrent state shape parameters

**Text-only scope (Q3):** Park the vision tower. The GGUF importer extracts only the text model tensors. The vision encoder (`Qwen3_5VisionModel`) is not imported. This is stated in the THINK and tracked as a separate OT.

---

## 6. D6 — Build plan + verdict

### 6.1 ISC-decomposed build plan

**Build B1: Walker + Qwen2 schedule parity** (~1 week, Oracle tier)
- ISC-1: Layer-kind schedule data structure + `.neom` header section
- ISC-2: Schedule walker dispatching to existing Qwen2 ops
- ISC-3: Load-time validation (§2 invariants)
- ISC-4: Kill-switch 2 proof — walker with Qwen2 schedule ≡ hardcoded, bit-for-bit (host KAT + OT-30 parity)
- ISC-5: OT-34 compatibility — threaded matmul works through the walker
- **Risk:** Low. The ops are proven; the walker is a dispatch table.

**Build B2: DeltaNet ops + KATs** (~2 weeks, Oracle tier)
- ISC-1: GDN recurrent step (port from HF reference)
- ISC-2: GDN causal conv1d, gating projections, L2 norm, RMSNormGated
- ISC-3: Per-op KATs (byte-oracle fixtures for each new op)
- ISC-4: GDN layer descriptor in the schedule
- **Risk:** Medium. The recurrence is new math, but the HF reference is self-contained and well-understood.

**Build B3: Model bring-up + OT-30 parity vs HF anchor** (~2 weeks, Oracle tier)
- ISC-1: GGUF→`.neom` importer for Qwen 3.5 (text-only)
- ISC-2: Load a real Qwen 3.5 model, run forward through the walker
- ISC-3: OT-30 parity — top-1 tokens match HF reference on known prompts
- ISC-4: Latency measurement via OT-7 oracle
- **Risk:** Medium-high. Real model integration always has surprises (quantization edge cases, weight layout mismatches).

### 6.2 Effort summary

| Build | Effort | Risk | Coder tier | Dependencies |
|-------|--------|------|-----------|--------------|
| B1: Walker + parity | ~1 week | Low | Oracle | OT-34 (threaded matmul) |
| B2: DeltaNet ops | ~2 weeks | Medium | Oracle | B1, Q1 FINDINGS |
| B3: Model bring-up | ~2 weeks | Medium-high | Oracle | B2, GGUF import |

**Total: ~5 weeks across 3 builds.** This is a Phase-9 slice — substantial but not a quarter-long project.

### 6.3 Verdict

**Recommendation: ADOPT — this is a Phase-9 slice worth the payoff.**

The payoff is concrete:
- **Tool-calling strength:** Qwen 3.5's tool calling lifts Arthur's agentic reliability from a higher floor (A1/A2/A3 + yolo-within-guardrails)
- **262K–1M context:** Pairs with Arthur's persistent memory (OT-17) — vast in-context recall
- **Constant-size state:** Better memory profile than growing KV cache — relevant to the heap budget fight
- **2B size band:** Between the landed 1.5B and the OT-38-targeted 3B

The risk is bounded:
- Q1 is RESOLVED (correct reference exists)
- The build is decomposed into 3 slices, each independently verifiable
- Kill-switch discipline ensures rollback to the proven Qwen2 forward
- ~565 lines of new code — auditable in a single sitting

**NOT the next immediate step.** The OT-38 course (Qwen2.5 1.5B/3B config-swap) is the faster capability win. This is the follow-on: after the 1.5B lands and the agentic stack matures, the DeltaNet hybrid becomes the natural next brain for its tool-calling + long-context advantages.

### 6.4 PL ratification points

1. **Schedule format:** Layer-kind enum dispatch (as designed) vs. more expressive graph format. Recommendation: layer-kind enum (minimal, covers both architectures, no VM in ring 0).
2. **`.neom` format-version bump:** Required for the schedule section. Recommendation: proceed with ADR-028-family byte oracle when B1 starts.
3. **Build priority:** Phase-9 slice (after 1.5B/3B config-swap) vs. defer entirely. Recommendation: Phase-9, after the immediate capability step.

---

## 7. Summary

| Decision | Verdict | Rationale |
|----------|---------|-----------|
| **D1** — Graph-description shape | Layer-kind schedule (typed enum dispatch, not graph VM) | Minimal expressiveness covering {Qwen2, DeltaNet hybrid}. No ring-0 NN compiler. |
| **D2** — Mechanism-not-policy | Kernel executes validated `.neom` schedule; 7 load-time invariants | Same mechanism-not-policy line as A1 logits-vs-bitmask. Hostile schedules rejected at load. |
| **D3** — Double kill-switch | OFF ≡ hardcoded bit-for-bit; ON+Qwen2-schedule ≡ hardcoded bit-for-bit (OT-30 parity) | I-1 byte-oracle applied to graph generalization. Two independent proofs. |
| **D4** — OT-34 compatibility | Threading is op-level, not schedule-level. DeltaNet recurrence is sequential (deterministic by construction). | No new determinism risk. Prefill chunking is a future optimization. |
| **D5** — DeltaNet op inventory | 6 new ops (~415 lines) + walker (~150 lines) = ~565 lines total. Per-op KATs defined. GGUF import viable. Text-only scope. | Bounded new surface. HF reference is self-contained parity anchor. |
| **D6** — Build plan + verdict | 3 builds (B1: walker+parity, B2: ops+KATs, B3: model bring-up). ~5 weeks total. ADOPT as Phase-9 slice after 1.5B/3B config-swap. | Concrete payoff (tool-calling + long-context + constant-state). Bounded risk. Correct reference exists. |

*This is a design THINK. No code was modified.*
