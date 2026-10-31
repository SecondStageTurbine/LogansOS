<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 9 BUILD-9.2 — wire quantized matmul into the forward pass (kill-switch + dual storage) → Jarvis

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Jarvis (built 9.1 + the consolidation's clippy gate — full Phase-9 context). New branch off `origin/main` HEAD: `codex/phase-9-build-9.2-wire-quantized-forward-jarvis`.
**Status:** DISPATCHED. **Host + the pre-merge QEMU gate (kernel-touching).** Wire 9.1's proven scalar Q4_K×Q8_K op (ADR-058) into the real forward pass, behind a kill-switch, with dual weight storage — so the quantized path is *selectable* and **off ≡ the 8.3 f32 path bit-for-bit**. Still scalar (AVX2 + the measurement are 9.3).

## 0. The decision this build makes
Make the forward pass *able to* run the quantized matmul. 9.1 proved the op standalone; 9.2 routes the per-layer matmul through it when `phase9-quantized-matmul` is on, and keeps the existing 8.3 dequant→f32→matmul when off. The model holds **both** weight representations (f32 + raw q4_K — D2) so the kill-switch's f32 fallback stays live. **No AVX2 (9.3), no latency measurement (9.3), no raw-only storage (after 9.3 proves it), no GPU.**

## 1. Authoritative design — RATIFIED (read first)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-17-phase-9-quantized-matmul-design.md   # §10 D1-D6; §3 the wiring seam
git show origin/main:docs/ADR-058-phase-9-scalar-quantized-matmul.md              # the 9.1 op this build wires
```
The seam is `matmul_vec()` in `kernel/src/ai/forward_logic.rs` (the named function; line drifted post-8.5). **D1 = Option A:** add `matmul_vec_quantized()` *alongside* the existing `matmul_vec()`, selected by the kill-switch — not a new OpKind (that's 9.4+).

## 2. ISCs
- **ISC-1 — Dual weight storage (D2).** During 9.1–9.2 the model keeps **both** the f32-dequant'd weights (the existing 8.3 path) **and** the raw q4_K bytes (for the quantized path). The kill-switch requires the f32 path available, so dual storage stays until 9.3 proves quantized production-ready (then raw-only). *(This temporarily raises footprint — intended, transitional; note it.)*
- **ISC-2 — Wire the quantized path (D1=A).** `matmul_vec_quantized()` runs the per-layer matmul via 9.1's op: q8_K-quantize the activation vector on the fly, integer-dot against the raw q4_K weights, dequant the result. The forward pass selects it when `phase9-quantized-matmul` is on; otherwise the existing f32 `matmul_vec()`. Scalar (9.1's op) — no SIMD.
- **ISC-3 — The two gates (I-12 + I-2).** **(a) Kill-switch regression (the Rule-1 / I-2 equality):** with the feature **OFF**, the forward pass is the 8.3 f32 path **bit-for-bit** — the 8.5 KATs (cached==uncached, the top-1 oracle) pass **unchanged**. **(b) Quantized integration:** with the feature **ON**, the quantized forward generates end-to-end on Qwen2.5-0.5B and its **top-1 matches the f32 path / `llama.cpp`** on the bring-up prompt (the 8.5 "Hello"→271 oracle). **Honest tolerance (per the THINK):** quantized vs f32 *logits* diverge by quantization error — the gate is **same top-1 token**, not bit-identical logits; a top-1 mismatch is diagnosed per-layer (quantization error vs wiring bug), not silently passed.
- **ISC-4 — Standards.** Kill-switch default-off ⇒ kernel **bit-for-bit today**. No new `unsafe` (9.1's op is scalar, no intrinsics; the wiring is plumbing) / no new invariant / no AI-governance-lock touch. `fmt`/`clippy -D warnings` (kernel clippy **blocking** — run the exact `--workspace --all-targets` via Docker). ANSPEC §11.x; fold into ADR-058 or a short 9.2 note.

## 3. Gate
- **Host:** the kill-switch regression (off ≡ 8.3 f32, the 8.5 KATs unchanged) + the quantized-on integration (top-1 match on the bring-up prompt) green; the op/KAT family unaffected; builds on+off the feature.
- **Kernel-touching (forward_logic + the model storage) ⇒ run the pre-merge `workflow_dispatch` QEMU gate green (all 4 shards)** (e.g. the `ai_generate`/`arthur_converse`-class scenario with the feature toggled). `cargo clippy --workspace --all-targets -- -D warnings` green via Docker.

## 4. Scope guard
Wire 9.1's op into the forward pass + the kill-switch + dual storage **only**. **Do NOT** add AVX2 (9.3), do the latency measurement (9.3), drop to raw-only storage (after 9.3), add a new OpKind (9.4+), target the GPU (9.5+), or change the op math (9.1's, frozen). No new `unsafe`/invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; both gates green (off ≡ 8.3 bit-for-bit; on → top-1 match); **pre-merge QEMU gate green**; clippy `--workspace --all-targets` green via Docker; push. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger. Next: **9.3** — AVX2 (`_mm256_maddubs`, bit-identical-vs-scalar gate) + the **latency-oracle measurement** that finally moves the §3 row (per-layer matmul latency + weight-bytes-read, Ryzen) and turns the ~7× *traffic* claim into a measured *speedup*.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed (never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-phase-9-build-9.2-wire-quantized-forward-jarvis.md`. **Branch:** `codex/phase-9-build-9.2-wire-quantized-forward-jarvis` off `origin/main`.
- **One-line:** Build **Phase 9.2 — wire your 9.1 scalar Q4_K×Q8_K op into the forward pass** (ratified D1=A: a `matmul_vec_quantized()` alongside the existing `matmul_vec()` in `forward_logic.rs`, selected by the kill-switch). **Dual weight storage** (D2 — keep both f32 + raw q4_K so the f32 fallback stays live; raw-only is post-9.3; note the transient footprint bump). Two gates: **(a) kill-switch OFF ≡ the 8.3 f32 path bit-for-bit** — the 8.5 KATs (cached==uncached, top-1 oracle) pass **unchanged** (the I-2 regression); **(b) feature ON → the quantized forward generates and its top-1 matches the f32 path / `llama.cpp`** on the "Hello"→271 bring-up prompt (honest tolerance: **same top-1**, not bit-identical logits — quantization error diverges the logits; a top-1 miss is diagnosed per-layer, not passed). Still **scalar** — AVX2 + the latency measurement are 9.3. Kill-switch default-off ≡ today; no new `unsafe`/invariant. **Kernel-touching ⇒ pre-merge QEMU gate green + `cargo clippy --workspace --all-targets -- -D warnings` via Docker (kernel clippy is BLOCKING).** **Scope: wire + kill-switch + dual storage only — NO AVX2/measure (9.3), NO raw-only, NO OpKind, NO GPU.**
