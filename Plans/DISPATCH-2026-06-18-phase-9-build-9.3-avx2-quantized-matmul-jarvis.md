<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 9 BUILD-9.3 — AVX2 quantized-matmul op + bit-identical-vs-scalar gate → Jarvis

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Jarvis (built 9.1 scalar + 9.2 wiring — full Phase-9 context). New branch off `origin/main` HEAD: `codex/phase-9-build-9.3-avx2-quantized-matmul-jarvis`.
**Status:** DISPATCHED. **Host-tier — the AVX2 q4_K×q8_K op, gated bit-identical against 9.1's scalar oracle.** The third Phase-9 slice (ratified THINK D4: prove scale-depacking scalar-first at 9.1, then AVX2 as an optimization *with its own bit-identical-vs-scalar gate*). The op only — **the latency *measurement* that moves the §3 row is the hardware-gated half (OT-7) and defers to the PL's Ryzen/OptiPlex window.**

## 0. The decision this build makes
Add an AVX2 implementation of the q4_K×q8_K integer dot (`_mm256_maddubs_epi16` + horizontal reduce) that is **bit-identical to 9.1's scalar op** on the same inputs — integer arithmetic, so the gate is *exact equality*, not a tolerance. Behind a feature, selectable; scalar stays the default and the oracle. **No forward-pass swap to AVX2 in-kernel yet** (that needs the YMM-state prerequisite below proven first — ISC-3), **no GPU (9.5+), no latency measurement (hardware-gated, OT-7).**

## 1. Authoritative design — RATIFIED (read first)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-17-phase-9-quantized-matmul-design.md   # §10 D4 (scalar-first then AVX2), §2 op math
git show origin/main:docs/ADR-058-phase-9-scalar-quantized-matmul.md              # the 9.1 scalar op = the bit-identical ORACLE
```
9.1's `ai/ops/qk_matmul.rs` scalar `matmul_q4_k_q8_k` is the **reference**: the AVX2 path must reproduce its output bit-for-bit.

## 2. ISCs
- **ISC-1 — AVX2 q4_K×q8_K integer dot.** `_mm256_maddubs_epi16` (u8×i8→i16 with saturation — verify the q8_K sign convention doesn't saturate; analytically bound as in 9.1's i32-accumulator note) + `_mm256_madd_epi16` widen-accumulate + horizontal sum; the same 6-bit-scale depack + `bsums` min-correction + dequant as scalar. Behind feature `phase9-avx2-matmul`.
- **ISC-2 — The bit-identical gate (the kill-switch equality, I-2 shape).** A host KAT asserting **`avx2_dot(x) == scalar_dot(x)` exactly** over the 9.1 KAT vectors + a randomized sweep. Integer math ⇒ **bit-identical, not ≤tol** — any divergence is a bug, diagnosed (saturation? horizontal-sum order? depack lane mismatch?), not passed. Scalar remains the oracle; AVX2 is validated *against* it (not against `llama.cpp` directly — 9.1 already anchored scalar to `llama.cpp`).
- **ISC-3 — New `unsafe` (AVX2 intrinsics) + the in-kernel YMM-state prerequisite (verify, don't assume).** AVX2 intrinsics are **new `unsafe`** — each `/// SAFETY`-documented, gated behind a **runtime CPUID `avx2` detection** (never execute AVX2 on a CPU without it; scalar fallback). **Blind-spot to check before any in-kernel use:** does the kernel preserve **YMM/AVX state across context switches and in interrupt handlers**? If the kernel only saves x87/SSE (XMM) state, using YMM in the forward path would corrupt across a preemption — a real prerequisite. **This build is host-tier (the KAT runs on the host CPU), so it does NOT require the kernel YMM fix** — but state the finding: if YMM-state preservation is absent, wiring AVX2 into the in-kernel forward pass is BLOCKED until it's added (surface as BLOCKER-to-PM; that becomes its own slice). 9.3 ships the op + the host gate; the in-kernel swap waits on the prerequisite.
- **ISC-4 — Standards.** Kill-switch `phase9-avx2-matmul` default-off ⇒ op not built, kernel bit-for-bit today. `ai/ops/`-local + `ai-ops-tests` host-only (like 9.1/9.2 — **no QEMU scenario** unless you wire it in-kernel, which ISC-3 defers). `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy **blocking**). No new invariant; the op math is 9.1's, frozen — AVX2 reproduces, never changes it.

## 3. Gate
- **Host:** the bit-identical KAT (`avx2 == scalar`, exact) + the randomized sweep green; the 9.1/9.2 op + KAT family unaffected; builds on+off the feature; runtime feature-detect verified (forced-scalar path on a non-AVX2 path tested).
- **`cargo clippy --workspace --all-targets -- -D warnings` GREEN via Docker** (incl. blocking kernel clippy).
- **Host-only ⇒ no pre-merge QEMU gate needed** (consistent with 9.1/9.2's host KATs). If you wire AVX2 in-kernel (you should NOT — ISC-3 defers it), the QEMU gate + the YMM prerequisite apply.

## 4. Scope guard
The AVX2 op + the bit-identical gate **only**. **Do NOT** swap the in-kernel forward pass to AVX2 (needs the ISC-3 YMM prerequisite — separate slice), do the latency measurement (hardware-gated, OT-7 — the PL's rig window), change the op math (9.1's, frozen), target the GPU (9.5+), or add a new OpKind. New `unsafe` limited to the documented, CPUID-gated AVX2 intrinsics.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (the one-command ritual — rebase-freshness vs current `main`, the exact CI host-check set on the rebased tree, SPDX, self-stat). Then the two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL in your RETURN; host-only ⇒ no QEMU URL needed), and the PM re-inspects your commit's own `git show --stat` before merge. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (ADR-058 amendment + §3 note). Next: the **latency-oracle measurement** (per-layer matmul latency + weight-bytes-read on the Ryzen/OptiPlex — the run that finally turns the ~7× *traffic* claim into a measured *speedup* and moves §3), bundled with the next silicon trip; and, if the YMM prerequisite is met, the in-kernel AVX2 forward swap.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-phase-9-build-9.3-avx2-quantized-matmul-jarvis.md`. **Branch:** `codex/phase-9-build-9.3-avx2-quantized-matmul-jarvis` off `origin/main`.
- **One-line:** Build **Phase 9.3 — the AVX2 q4_K×q8_K integer-dot op** (`_mm256_maddubs_epi16` + widen-accumulate + horizontal reduce; same depack/`bsums`/dequant as 9.1), gated **bit-identical against 9.1's scalar op** (integer math ⇒ exact equality, not tolerance — scalar is the oracle). New `unsafe` = the AVX2 intrinsics, each `/// SAFETY`-doc'd + **CPUID runtime-detected** (scalar fallback; never run AVX2 unguarded). **Verify + report** whether the kernel preserves **YMM state across context switches/IRQs** — if not, the in-kernel AVX2 swap is BLOCKED (its own later slice); **this build is host-tier so it doesn't need the fix.** Kill-switch `phase9-avx2-matmul` default-off ≡ today; `ai/ops/`-local, host-only (no QEMU, like 9.1/9.2); no new invariant; op math frozen. **Scope: the AVX2 op + bit-identical KAT ONLY — NO in-kernel swap, NO latency measurement (hardware-gated), NO GPU.** **Close-gate: run `scripts/worker-close-gate.sh` before declaring done; RETURN must paste the green push-CI URL.**
