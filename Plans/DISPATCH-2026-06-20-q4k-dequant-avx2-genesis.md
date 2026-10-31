<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Q4_K dequantize AVX2 — bit-identical to scalar → Genesis (GLM-5.2)

**Date:** 2026-06-20
**From:** PM (Claude)
**To:** Genesis (GLM-5.2). New branch off `origin/main` HEAD: `codex/q4k-dequant-avx2-genesis`.
**Status:** DISPATCHED. **A host-only AVX2 bridge — zero QEMU/funnel cost while the kernel runners drain (OT-23, M1, then E1b).** Sibling of your landed q8_K quantizer: the **Q4_K block → f32 dequantizer** (`q4_k.rs:21 dequantize_q4_k`) has no AVX2 path yet. Same integer-depack + native-f32 discipline, same no-FMA trap you already nailed. Bit-identical-KAT gated, `ai/ops`-local, **host-only (no QEMU)**. (E1b resumes when a QEMU runner frees.)

## 0. The decision this build makes
Add an **AVX2 `dequantize_q4_k`** bit-for-bit identical to the scalar oracle (`q4_k.rs:21`), wired through a CPUID+XCR0 runtime gate (the exact pattern your q8_K quantizer + the 9.3 dot use). Scalar stays the default + oracle. **No new op, no math change — a SIMD reimplementation of an existing, frozen, native-f32 dequant, proven equal.**

## 1. References
```
git fetch origin
git show origin/main:kernel/src/ai/ops/q4_k.rs           # scalar dequantize_q4_k @ :21 (THE oracle) + get_scale_min_k4 @ :63 (the 6-bit depack — keep scalar) + f16_bits_to_f32
git show origin/main:kernel/src/ai/ops/qk_matmul.rs       # YOUR q8_K quantizer (the avx2 mod, CPUID/XCR0 gate runtime_available, quantize_row_q8_k_with_kernel dispatch) + the 9.3 dot — mirror these exactly
git show origin/main:tools/ai-ops-tests/src/lib.rs        # the Phase-9 KAT pattern (hand-laid + random sweep, bit-pattern compare) — add yours alongside
```
This is structurally your own q8_K build in reverse (unpack instead of pack). Reuse the discipline verbatim.

## 2. ISCs
- **ISC-1 — `avx2::dequantize_q4_k`, bit-identical to the scalar.** Reproduce `q4_k.rs:21` exactly: `d`/`dmin` from `f16_bits_to_f32` (scalar — once per block), `get_scale_min_k4` for the 6-bit `(sc,m)` pairs (**keep scalar** — 8 indices/block, like you kept signed max-abs scalar in the quantizer), then per 64-value sub-block: `d1 = d*sc1`, `min1 = dmin*m1` (native f32), and vectorize the 32 low-nibble lanes `out = d1*(qs&0x0f) - min1` and the 32 high-nibble lanes `out = d2*(qs>>4) - min2`. Output `[f32;256]` must equal the scalar's **bit-for-bit** for every block.
- **ISC-2 — The traps (pre-seeded — you hit the analogous ones in q8_K).** **(a) NO FMA:** the scalar is `d1 * q - min1` = a separate multiply *then* subtract; `_mm256_fmsub_ps` fuses and **diverges** — use `_mm256_mul_ps` then `_mm256_sub_ps`. **(b) nibble→f32 is exact** (`(u8 & 0x0f) as f32`, 0..15 → `_mm256_cvtepi32_ps`); preserve the **lane order** (32 low-nibble lanes then 32 high-nibble, per the scalar's two loops). **(c) the `f16→f32` of `d`/`dmin` must be identical** — reuse the existing `f16_bits_to_f32`, do not re-derive. **(d) the scalar/min path** (`d*sc as f32`, `dmin*m as f32`) is native f32 — compute it the same way (scalar-or-broadcast, but the product must match).
- **ISC-3 — Wire the dispatch.** Add a kernel-selectable entry (e.g. `dequantize_q4_k_with_kernel` or a row variant) that calls `avx2::dequantize_q4_k` when `phase9-avx2-matmul` + runtime AVX2 (reuse `runtime_available()`/`q4_k_q8_k_avx2_available()`), else the scalar. The **default `dequantize_q4_k` stays scalar** (the frozen oracle) so the off-build is byte-identical.
- **ISC-4 — KAT (the gate) in `tools/ai-ops-tests`.** Hand-laid blocks (all-zero quants, all-`0xff` nibbles → max, alternating, a real/representative block, a block with distinct sc/m per sub-block) + a randomized sweep (many blocks), asserting AVX2 output == scalar **by `f32::to_bits()`** (exact, not tolerance), element-by-element. Exercise the AVX2 path on the host (it has AVX2) — not skipped.
- **ISC-5 — Standards + kill-switch.** **`phase9-avx2-matmul` default-OFF ⇒ scalar only ≡ today bit-for-bit** (the feature exists — you're adding a dequant arm). **New `unsafe` = the AVX2 intrinsics** (`/// SAFETY`-doc'd, behind the CPUID+XCR0 gate). No new invariant; `ai/ops`/`q4_k`-local; **host-only (no QEMU — 9.1/9.2/9.3/your-q8K precedent)**. SPDX. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). **Do NOT wire AVX2 into the in-kernel forward pass** (OT-23-blocked) — the host op + KAT only.

## 3. Gate (host-only — no QEMU)
- **Host:** the hand-laid bit-identical KAT + the random sweep, green; builds **on AND off** `phase9-avx2-matmul`; off-build byte-identical to today.
- `cargo clippy --workspace --all-targets -D warnings` green via Docker.
- **No QEMU shard** (host-only — state it, citing your q8_K precedent).

## 4. Scope guard
The `avx2::dequantize_q4_k` + its dispatch arm + the KAT **only**. **Do NOT** change the scalar (frozen oracle), `get_scale_min_k4`, the 9.3 dot or your q8_K quantizer, wire AVX2 into the kernel forward pass (OT-23-blocked), or touch anything outside `kernel/src/ai/ops/{q4_k.rs,qk_matmul.rs}` + `tools/ai-ops-tests`. No new invariant. New `unsafe` = the documented AVX2 intrinsics only.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness — **base moves fast, rebase LAST**; the exact CI host-checks; SPDX; self-stat). Two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL — host-only, no QEMU, say so), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger. 

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Genesis (GLM-5.2)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-20-q4k-dequant-avx2-genesis.md` (+ your own q8_K quantizer in `qk_matmul.rs` as the template, and the scalar `q4_k.rs:21`). **Branch:** `codex/q4k-dequant-avx2-genesis` off `origin/main`.
- **One-line:** Build the **AVX2 `dequantize_q4_k` bit-for-bit identical to the scalar** at `q4_k.rs:21` — the dequant sibling of your q8_K quantizer (same q4_K depack, same no-FMA trap). Per 64-value sub-block: `d`/`dmin` via `f16_bits_to_f32` + `get_scale_min_k4` **kept scalar**, then vectorize `out = d1*(qs&0x0f) - min1` (32 low lanes) and `out = d2*(qs>>4) - min2` (32 high lanes) — **`_mm256_mul_ps` then `_mm256_sub_ps`, NEVER `_mm256_fmsub_ps`** (fusion diverges), nibble→f32 via `cvtepi32_ps`, preserve lane order, reuse `f16_bits_to_f32` exactly. Wire a CPUID+XCR0-gated dispatch arm (mirror `quantize_row_q8_k_with_kernel`); default stays scalar (oracle). **Gate = bit-identical KAT** in `tools/ai-ops-tests` (hand-laid: all-zero / all-0xff / alternating / distinct-sc-m / real block + random sweep; compare by `f32::to_bits()`). Kill-switch `phase9-avx2-matmul` off ⇒ scalar ≡ today. New `unsafe` = AVX2 intrinsics only. **Host-only — no QEMU.** **Scope: the AVX2 dequant arm + dispatch + KAT ONLY — don't touch the scalar oracle, `get_scale_min_k4`, the 9.3 dot/your quantizer, or the kernel forward pass.** **Close-gate: `scripts/worker-close-gate.sh` (rebase LAST); RETURN pastes the green push-CI URL** (host-only, no QEMU).
