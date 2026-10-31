<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: q8_K block-quantizer AVX2 — bit-identical to scalar → Genesis (GLM-5.2)

**Date:** 2026-06-19
**From:** PM (Claude)
**To:** Genesis (GLM-5.2, OpenCode CLI coder — new seat). New branch off `origin/main` HEAD: `codex/q8k-quantize-avx2-genesis`.
**Status:** DISPATCHED. **Genesis's trust-ramp build + a real Phase-9 gap.** Self-contained `ai/ops` primitive, **host-only (no QEMU)**, gated by a **bit-identical-to-scalar KAT** — a deterministic objective oracle, exactly the bounded shape a new seat earns trust on. It also closes the never-validly-completed A/B Slot-A task: the AVX2 path for the Q8_K *block quantizer* (the activation-side quantize the q4_K×q8_K matmul consumes).

## 0. The decision this build makes
Add an **AVX2-optimized `quantize_block_q8_k`** that is **bit-for-bit identical** to the existing scalar quantizer, wired into `quantize_row_q8_k`'s existing runtime dispatch (the same CPUID+XCR0-gated pattern 9.3 used for the matmul dot). Scalar stays the default + the oracle; AVX2 is an optimization that must prove exact equality. **No new op, no algorithm change — a SIMD reimplementation of an existing, frozen scalar function, proven equal.**

## 1. References
```
git fetch origin
git show origin/main:kernel/src/ai/ops/qk_matmul.rs        # scalar quantize_block_q8_k @ :202 (THE oracle); quantize_row_q8_k @ :64 (dispatch site); mod avx2 @ :291 (the existing 9.3 AVX2 + the CPUID/XCR0 gate to mirror); feature phase9-avx2-matmul
git show origin/main:tools/ai-ops-tests/src/lib.rs         # the Phase-9 KAT harness — the hand-laid + random-sweep bit-identical pattern 9.3 used; add yours alongside
git show origin/main:docs/bible.md                         # Article-13 BUILD-9.3 entry (the AVX2-equality-gate discipline, ratified D4: scalar-first oracle → AVX2 optimization with its own equality gate)
```
**Mirror 9.3 exactly:** `_mm256_*` intrinsics, `#[target_feature(enable="avx2")]`, `/// SAFETY`-documented unsafe, behind `phase9-avx2-matmul`, dispatched only when **CPUID + XCR0 runtime detection** confirms AVX2 (not just the CPUID bit — that the OS enabled YMM via XCR0, the rigorous check 9.3 already implements; reuse it).

## 2. ISCs
- **ISC-1 — `avx2::quantize_block_q8_k`, bit-identical to the scalar.** Reproduce the scalar (`qk_matmul.rs:202`) exactly in AVX2: find max-abs over the block, `iscale = -127.0 / max`, quantize each element via the scalar's **exact** `nearest_int` rounding, **clamp to [-127, 127]**, compute `bsums`. The output `BlockQ8K` (`d`, `qs`, `bsums`) must equal the scalar's for **every** input — this is integer/exact-float equality, **not** a tolerance.
- **ISC-2 — The degenerate/tie traps (the discriminator — pre-seeded from the A/B).** The A/B's earlier q8_K attempt tripped a **sign/rounding-tie** bug. Pin these down and prove them in the KAT: **(a) the all-zero / max==0 block** — `iscale` would divide by zero; match whatever the scalar does (almost certainly `d=0`, `qs=0`, `bsums=0`) bit-for-bit. **(b) round-half ties** — `nearest_int` at exact `.5` boundaries must round the *same direction* as the scalar (round-half-to-even vs away-from-zero — read the scalar, do not assume). **(c) clamp boundaries** — values mapping to exactly ±127 and just past. **(d) negative-max** — max-abs from a negative element. If the scalar and AVX2 disagree on any of these, the AVX2 is wrong, not the scalar.
- **ISC-3 — Wire the dispatch.** Extend `quantize_row_q8_k` (`:64`) to call `avx2::quantize_block_q8_k` when `phase9-avx2-matmul` is enabled AND runtime AVX2 is detected (the existing gate), else the scalar. Identical control-flow shape to the 9.3 matmul dispatch.
- **ISC-4 — KAT (the gate) in `tools/ai-ops-tests`.** Two tests, the 9.3 pattern: **(1) hand-laid cases** — sequential `0..255`, all-zeros, a uniform value, alternating `±` extremes, a negative-max case — asserting AVX2 `(d, qs, bsums)` == scalar element-by-element. **(2) randomized sweep** — many random blocks (1–8 blocks each), AVX2 == scalar bit-for-bit. (Both halves run on host; the scalar IS the oracle — no external KAT vector to mangle, so no acquisition hazard.)
- **ISC-5 — Standards + kill-switch.** **Kill-switch `phase9-avx2-matmul` default-OFF ⇒ scalar path only ≡ today bit-for-bit** (the feature already exists — you're adding a quantize arm under it). **New `unsafe` = the AVX2 intrinsics**, `/// SAFETY`-documented, behind the CPUID+XCR0 gate. No new invariant; `ai/ops`-local; **host-only (no QEMU — like 9.1/9.2/9.3)**. SPDX intact. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). **Do NOT wire AVX2 into the in-kernel forward pass** — that's blocked on OT-23 (YMM/XSAVE); this is the host/`ai/ops` op + its KAT only.

## 3. Gate
- **Host:** the hand-laid bit-identical KAT + the randomized sweep, green; **builds on AND off** `phase9-avx2-matmul`; the off-build is byte-identical to today.
- `cargo clippy --workspace --all-targets -D warnings` green via Docker.
- **No QEMU shard** (host-only op — state this in the RETURN, citing the 9.1/9.2/9.3 precedent).

## 4. Scope guard
The `avx2::quantize_block_q8_k` + its dispatch arm + the KAT **only**. **Do NOT** change the scalar (it's the frozen oracle), change the matmul dot (9.3, frozen), wire AVX2 into the kernel forward pass (OT-23-blocked), add a new op/feature, or touch anything outside `kernel/src/ai/ops/qk_matmul.rs` + `tools/ai-ops-tests`. No new invariant. New `unsafe` limited to the documented AVX2 intrinsics.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-checks, SPDX, self-stat). Two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL — host-only, so the push CI is the gate; no QEMU needed, say so), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (the Q8_K quantize AVX2 path, bit-identical-gated; **Genesis's first landed build**). 

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Genesis (GLM-5.2)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-19-q8k-quantize-avx2-genesis.md`. **Branch:** `codex/q8k-quantize-avx2-genesis` off `origin/main`.
- **One-line:** Build the **AVX2 `quantize_block_q8_k`, bit-for-bit identical to the scalar** at `kernel/src/ai/ops/qk_matmul.rs:202` (max-abs → `iscale = -127/max` → exact `nearest_int` → clamp `[-127,127]` → `bsums`), wired into `quantize_row_q8_k`'s existing CPUID+XCR0-gated dispatch (mirror the 9.3 matmul AVX2 — `#[target_feature(enable="avx2")]`, `/// SAFETY`, behind `phase9-avx2-matmul`). **The gate is a bit-identical KAT** in `tools/ai-ops-tests` (hand-laid cases + random sweep; `(d,qs,bsums)` == scalar, exact not tolerance). **Nail the traps the earlier attempt missed:** the all-zero/`max==0` block (no div-by-zero — match the scalar), round-half ties (round the SAME direction as the scalar — read it, don't assume), clamp boundaries, negative-max. **Kill-switch `phase9-avx2-matmul` default-off ⇒ scalar only ≡ today.** New `unsafe` = the AVX2 intrinsics only. **Host-only — no QEMU** (9.1/9.2/9.3 precedent). **Scope: the AVX2 quantize arm + dispatch + KAT ONLY — do NOT touch the scalar (the oracle), the matmul dot (9.3), or wire AVX2 into the kernel forward pass (OT-23-blocked).** **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI URL** (host-only — no QEMU).
