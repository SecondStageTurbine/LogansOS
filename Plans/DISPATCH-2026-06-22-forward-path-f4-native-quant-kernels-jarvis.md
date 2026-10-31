<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Forward-path F4 — native in-register quant matmul kernels (the real latency lever) → Jarvis

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Jarvis (forward-path + OT-30 continuity). New branch off `origin/main` HEAD: `codex/forward-path-f4-native-quant-kernels-jarvis`.
**Status:** DISPATCHED — **PL-promoted from "post-silicon perf slice" to now (2026-06-22).** OT-30 made the real 0.5B forward *runnable* (1.56 s/token) by killing the per-token materialization, but the quant matmul is still **dequant-to-f32 + generic soft-float** — that residual is the bottleneck. F4 is the THINK's deferred **Option B**: native **in-register quant×q8_K dot-product kernels** (dequant *inside* the dot product, no f32 materialization) — memory-lean + fast, exactly llama.cpp's speed story. This is the honest path to a snappy Arthur before Stage-B (and it scales to 1.5B). *(Speculative decoding was considered and deferred — wrong regime: no large target, 0.5B↔1.5B is only a 3× gap; see OT-33. Native kernels are the real win.)*

## 0. The decision this build makes
Turn "the forward dequantizes to f32 then does a generic soft-float matmul (1.56 s/token)" into "**the forward does native in-register quant dot products**" — a clean multiple on the actual hot path, **with correctness preserved** (still top-1 parity vs llama.cpp on the real model; the native approach matches llama.cpp's own, so parity should be ≥ OT-30's). Success is *measured*: per-token latency drops materially AND Stage-A parity still holds.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/ai/ops/qk_matmul.rs        # the EXISTING q4_K×q8_K native kernel (scalar + AVX2) — the template for q5_0/q6_K/q8_0
git show origin/main:kernel/src/ai/forward_logic.rs        # OT-30: the matmul dispatch (post-materialization-removal) — where the native kernels plug in under phase9-real-quant
git show origin/main:kernel/src/ai/gguf_dequant_logic.rs   # the per-type dequant primitives — the REFERENCE for the equivalence KAT (parity-validated via OT-30)
git show origin/main:tools/ai-ops-tests/src/lib.rs         # F1 byte-oracle (one-hot KAT) + the Stage-A harness — the success oracle (latency + llama.cpp parity)
git show origin/main:Plans/THINK-2026-06-21-forward-path-real-model-integration.md  # D1 Option B (native kernels), §9 (llama.cpp vec_dot_q*_q8 prior art — raw-source the block layouts)
git show origin/main:docs/ADR-066-forward-mixed-quant-dequant.md  # amend with F4
```
**Prior art (raw-source, not a summarizing fetch):** llama.cpp's `vec_dot_q5_0_q8_0` / `vec_dot_q6_K_q8_K` / `vec_dot_q8_0_q8_0` — read the *actual* block layouts + the dot-product structure; do not pull exact constants through an LLM-mediated read (the byte-oracle acquisition rule).

## 2. ISCs
- **ISC-1 — Native kernels for the real model's quant types.** Implement in-register `q5_0×q8_K`, `q6_K×q8_K`, `q8_0×q8_K` dot-product matmul kernels (q4_K already exists — reuse/align with it), modeled on the existing `qk_matmul` q4_K path: quantize the activation to q8_K once, then per weight block dequant-and-multiply-accumulate **in registers**, no full-tensor f32 buffer. **Scalar + AVX2 variants** (match the q4_K kernel's structure).
- **ISC-2 — Wire into the forward dispatch.** Under `phase9-real-quant`, route each weight tensor's matmul to its native kernel by dtype (replacing OT-30's dequant-to-f32 compute for these types). No per-tensor f32 materialization remains on the hot path.
- **ISC-3 — Hermeticity (the gate invariant — critical).** AVX2 availability differs by runner, so: **runtime-detect AVX2 with a scalar fallback**, and the result MUST be identical on both paths. The byte-oracle (ISC-4) asserts **both** scalar and AVX2 produce the hand-computed answer. A scenario must never depend on a CPU feature that differs by runner (the established hermeticity rule) — if AVX2 needs `-cpu max`-style determinism in the gate, follow the `kernel_rng_selftest` precedent. (No RNG here, but the SIMD-feature divergence is the same class.)
- **ISC-4 — Correctness: two oracles + the measured win.** (a) **Per-kernel byte-oracle** — hand-laid quant block × known activation → assert the **hand-computed dot product** (the F1 one-hot pattern), for each type, scalar AND AVX2, independent of round-trip. (b) **Equivalence KAT** — native-kernel output vs the **dequant-to-f32 reference** (`gguf_dequant_logic`, which is OT-30/llama.cpp-parity-validated) on real-shaped tensors, within a stated rounding tolerance — this transfers the parity guarantee to the native path. (c) **Measured win (latency-oracle, OT-7 discipline)** — re-run the Stage-A harness on the real 0.5B: **top-1 llama.cpp parity still holds** (`first_divergence=None` for K tokens) AND **per-token latency drops materially** vs OT-30's 1.56 s/token — report before/after. A latency claim without the measured number is not done.
- **ISC-5 — kill-switch + standards.** `phase9-real-quant` OFF ≡ fixture bit-for-bit (unchanged); the F1 `…direct_matmul_path_kat` stays green. Keep the dequant-to-f32 path available as the equivalence-KAT reference (don't delete it — it's the correctness anchor). New `unsafe`: only the SIMD intrinsics (the q4_K AVX2 path already uses them — consistent; flag/justify, no more than needed). No new invariant. SPDX; `cargo fmt --all --check`; clippy `-D` (kernel blocking) via Docker. ADR-066 amendment (F4 native kernels + the equivalence-KAT correctness argument).

## 3. Gate
- **Host KATs:** the per-type byte-oracles (scalar + AVX2) + the equivalence KAT (native vs dequant-f32 within tolerance) + F1's KAT still green.
- **The measured Stage-A re-run** (reported — artifact not in git): llama.cpp top-1 parity preserved + the new per-token latency.
- **QEMU `forward_mixed_quant`** green (feature ON, now native) + **`arthur_boots_thinking`** green (OFF ≡ fixture). **Kernel-touching ⇒ 4-shard QEMU `workflow_dispatch`.** Serialize behind any in-flight `forward_logic.rs` work (F2 — coordinate/rebase LAST to avoid the config-vs-kernel collision).

## 4. Scope guard
Native quant matmul kernels + their oracles **only**. **Do NOT**: add multi-threading (a separate banked lever — OT-34; this is the SIMD/in-register axis, one concern at a time), change config (F2), the `.neom`/vocab (L1/F3), or the graph topology (OT-16); change the dequant *values* (the equivalence KAT pins them). Keep the kill-switch + the dequant reference path. No new invariant; SIMD `unsafe` only.

## 5. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST — coordinate with F2 on `forward_logic.rs`; full set; SPDX; self-stat). Green push-CI + 4-shard QEMU URLs — **paste them**; PM re-inspects `git show --stat` + the equivalence-KAT + the measured latency. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**F4 — native quant matmul; Arthur's per-token latency down from 1.56 s to <measured>; Stage-B bundles the fast forward**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-forward-path-f4-native-quant-kernels-jarvis.md` (+ `ops/qk_matmul.rs` [the q4_K template], `forward_logic.rs` [OT-30 dispatch], `gguf_dequant_logic.rs` [the equivalence reference], the Stage-A harness, THINK §9). **Branch:** `codex/forward-path-f4-native-quant-kernels-jarvis` off `origin/main`.
- **One-line:** **PL-promoted F4 — the real latency lever (THINK's Option B, now-not-later).** OT-30 made the 0.5B forward run (1.56 s/token) but it still **dequants-to-f32 + generic soft-float**; replace that with **native in-register `q5_0/q6_K/q8_0 ×q8_K` dot-product matmul kernels** (dequant in the dot, no f32 materialization; q4_K exists — model on it; **scalar + AVX2**). **Hermeticity:** runtime-detect AVX2 + scalar fallback, both paths byte-identical (a gate can't depend on runner CPU features). **Two correctness oracles:** per-type hand-computed byte-oracle (scalar AND AVX2, F1 one-hot pattern) + an **equivalence KAT** (native vs the dequant-to-f32 reference within tolerance — that reference is OT-30/llama.cpp-parity-validated, so equivalence transfers correctness). **Measured win (latency-oracle):** re-run Stage-A — **top-1 llama.cpp parity must still hold** AND **per-token latency drops materially vs 1.56 s/token** (report before/after; a latency claim w/o the number isn't done). Raw-source llama.cpp's `vec_dot_q*` block layouts (no summarizing fetch). Kill-switch `phase9-real-quant` OFF ≡ fixture; keep the dequant path as the equivalence reference; SIMD `unsafe` only, no new invariant. **Scope: native kernels ONLY — NOT threading (OT-34), not F2/config, not the graph.** Kernel-touching ⇒ 4-shard QEMU; **coordinate with F2 on `forward_logic.rs`, rebase LAST.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN the green push-CI + 4-shard QEMU URLs + the before/after latency + parity.**
