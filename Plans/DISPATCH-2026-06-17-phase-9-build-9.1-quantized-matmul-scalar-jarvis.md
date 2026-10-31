<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 9 BUILD-9.1 — scalar quantized-matmul kernel + KAT → Jarvis

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Jarvis (free after the benchmark + consolidation; the perf/measurement fit). New branch off `origin/main` HEAD: `codex/phase-9-build-9.1-quantized-matmul-scalar-jarvis`.
**Status:** DISPATCHED. **Host-tier — published-reference KAT (I-12).** The first slice of the ratified Phase-9 arc: the **scalar** q4_K×q8_K integer-dot matmul op + the q8_K activation quantizer, host-proven against `llama.cpp`. The speed unlock that makes the real Qwen2.5-1.5B-Instruct run at interactive speed — *"useful, not a demo."*

## 0. The decision this build makes
Build the **scalar** quantized-matmul kernel (D4: prove the riskiest code — scale depacking — on scalar first, with full KAT coverage; AVX2 is 9.3 as an optimization with its own bit-identical-vs-scalar gate). Standalone op + KAT only — **no wiring into the forward pass** (that's 9.2), no SIMD, no GPU (9.5+). ~7× less weight traffic than the f32-dequant path (the §3 *traffic* arithmetic; the realized *speedup* is measured at 9.3).

## 1. Authoritative design — RATIFIED (read first)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-17-phase-9-quantized-matmul-design.md   # §10 = ratified D1-D7; §2 op math; §4 KAT plan; §7 raw-source llama.cpp ref
```
Reuse the **8.3a `block_q4_K` layout** (144 B / 256). RustCrypto-style discipline doesn't apply (this is integer arithmetic, not crypto) — but the op *math* is the deliverable, validated against the published reference.

## 2. ISCs
- **ISC-1 — q8_K activation quantizer.** f32 activation vector → `block_q8_K` (256-elem) on the fly: the −127/max per-block scaling + the `bsums` (per-16-group sums for the q4_K min-correction). Host-KAT'd against a **hand-laid `block_q8_K` byte oracle** (I-12, exact bytes, not round-trip).
- **ISC-2 — Scalar q4_K×q8_K integer dot.** Per 256-elem super-block: depack the q4_K 6-bit scales/mins, the integer multiply-accumulate (`i32` accumulator — analytically verify no overflow: 32×(15×127)×63 ≈ 3.8M ≪ 2³¹), the min-correction via `bsums`, then dequant the accumulator (weight-scale × activation-scale). **Scalar only** — no `_mm256_maddubs` (9.3). The scale-depacking is the trap; pin it.
- **ISC-3 — The KAT (D7, I-12).** Match **`llama.cpp`'s `ggml_vec_dot_q4_K_q8_K`** output on synthetic cosine data, **tolerance ≤ 0.02 normalized** (D3 — llama.cpp's own; fp16-scale + horizontal-sum rounding is platform-dependent). **Raw-source the reference** (`git`/`curl` the llama.cpp `test-quantize-fns.cpp` / the kernel; cite SHA — **never WebFetch**). The q8_K quantizer (ISC-1) is KAT'd independently.
- **ISC-4 — Standards.** **Kill-switch:** behind a feature, default-off ⇒ the op isn't built/used, kernel **bit-for-bit today**. `ai/ops/`-local + `ai-ops-tests` (host-only, like 8.4a/8.5a — no QEMU scenario). **No new `unsafe`** (scalar, no intrinsics); **no new invariant**; no AI-hot-path/governance touch. ANSPEC §11.x; fold into ADR-055 or a short 9.1 note.

## 3. Gate
- **Host:** the q4_K×q8_K KAT vs `llama.cpp` (≤0.02) + the q8_K-quant byte oracle green; the existing op/KAT family unaffected; builds on+off the feature.
- **`cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings` GREEN via the Docker toolchain** — note **kernel clippy is now BLOCKING** (the consolidation flipped it); your op must be clippy-clean, and run the *exact* CI command (not `-p`).
- Host-only (no kernel boot path changes) ⇒ **no QEMU scenario / no pre-merge QEMU gate needed** (consistent with 8.4a/8.5a). If you add any boot-path/scenario, the gate applies.

## 4. Scope guard
The scalar op + the q8_K quantizer + the KATs **only**. **Do NOT** wire into `forward_logic` (9.2), add AVX2 (9.3), target the GPU (9.5+), or change the f32 path. Reuse 8.3a's block layout. No new `unsafe`/invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; the KATs + family green; `clippy --workspace` (incl. now-blocking kernel) green via Docker; push (CI). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (ADR + §3 row note). Next: 9.2 (wire it into the forward pass behind the kill-switch, dual f32+raw storage per D2) → 9.3 (AVX2 + the latency-oracle measurement — the row that finally moves §3).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed (never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-phase-9-build-9.1-quantized-matmul-scalar-jarvis.md`. **Branch:** `codex/phase-9-build-9.1-quantized-matmul-scalar-jarvis` off `origin/main`.
- **One-line:** Build **Phase 9.1 — the scalar q4_K×q8_K integer-dot matmul + the q8_K activation quantizer** (ratified Phase-9 THINK §10; reuse 8.3a's `block_q4_K`). Scalar only (D4 — prove scale-depacking first; AVX2 is 9.3). The q8_K quantizer (−127/max scaling + `bsums`); the integer MAC (`i32` accumulator, no overflow) + min-correction + dequant. **KAT against `llama.cpp`'s `ggml_vec_dot_q4_K_q8_K`, ≤0.02 normalized (D3), raw-source never WebFetch**, + a hand-laid q8_K byte oracle. Kill-switch feature default-off ≡ today; `ai/ops/`-local, host-only (no QEMU, like 8.4a); **no new `unsafe`/invariant**. Gate: KATs green + **`cargo clippy --workspace --all-targets -- -D warnings` green via Docker (kernel clippy is now BLOCKING — run the exact command, not `-p`)**. **Scope: scalar op + KAT only — NO forward wiring (9.2), NO AVX2 (9.3), NO GPU.**
