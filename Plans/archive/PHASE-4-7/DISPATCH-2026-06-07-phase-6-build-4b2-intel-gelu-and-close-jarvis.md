<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4b-2 — GPU GELU + forward-pass-equivalence + migration (closes Phase 6) → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `c7af58a`: `codex/phase-6-build-4b2-intel-gelu-and-close-jarvis`)
**Status:** DISPATCHED. **This build closes Phase 6.**
**Builds on:** BUILD-4b-1 (`dc27459`, op dispatcher + matmul on silicon) + the THINK `Plans/THINK-2026-06-07-phase-6-build-4b-op-set-equivalence.md`.

## 0. The scoping decision (PM, deliberate)
4b-2 lands **one transcendental on the GPU — GELU — and** does the integration close (forward-pass-equivalence + migration). It does **not** take the lean matmul-only close. Two reasons, both load-bearing for the project's heart:
1. **4b-1 left the tolerance machinery unvalidated.** matmul came out bit-exact (`max_rel=0`) because the Gen9 EU has no FMA — so `≤2⁻¹²` / the host-f32 reference has never been exercised where it bites. Closing Phase 6 with correctness apparatus that's never done its job violates Article 15.7 (a byte must buy *validated* correctness) and this project's verification discipline. **GELU is the first op that genuinely diverges** (Gen9 hardware-math `tanh`/`exp` ≠ soft-float ≠ host libm), so it makes the tolerance non-vacuous.
2. **"Intelligence as a kernel primitive" means the nonlinearity runs on the accelerator.** A transformer is matmuls *and* nonlinearities; closing the AI-accelerator phase with every activation bouncing to CPU would keep the AI-characteristic math off the silicon. GELU is the MLP activation.

This is one op, chosen because it makes the phase's claim *true* — not op-coverage expansion.

## 1. What 4b-1 proved (the starting point)
`IntelAccelDriver::dispatch` is a kernel-table `OpKind` router (Add|Matmul) on the proven 4a substrate; uncovered/oversize ops route `Unsupported` → `accel_cpu` (completes, never fails). The GPU tracks host f32 bit-for-bit on linear ops (no FMA). The 4a dispatch substrate (legacy ring, GGTT, STATE_BASE_ADDRESS, NEO indirect-data payload, fault-address capture) is settled and op-agnostic.

## 2. Scope
1. **Land GELU on the GPU** (pointwise, opcode 5) via the existing finalizer; a third dispatcher entry.
2. **Fix the tolerance-reference model** for ops that actually diverge (the real intellectual content — §ISC-3).
3. **Forward-pass-equivalence** — the model-disk transparency oracle 4b-1 deferred.
4. **Tensor migration** cpu↔intel — the last unproven `AccelDriver` capability on real GPU.
Then Phase 6 closes.

## 3. ISCs

- **ISC-1 — Author + finalize the GELU EU kernel.** `intel_gen9_kernels/gelu_f32.cl` computing the **same tanh-approx form** as `ai::ops::gelu` (`x·0.5·(1+tanh(√(2/π)·(x+0.044715·x³)))`), stateless A64. Use OpenCL `tanh()` so it compiles to the Gen9 **hardware extended-math** path (verify the disasm shows a `math` instruction, not a polynomial — that's the whole point). Run the pinned finalizer; commit the blob + `gelu_f32.gen9.asm`/`.zeinfo` oracles + README provenance row. Extract its **own** zeinfo ABI (pointwise: in-ptr, out-ptr, count) — don't assume matmul's offsets.

- **ISC-2 — Dispatcher entry for `Gelu`.** Add the pointwise payload builder (in/out A64 ptrs + element count) and the `OpKind::Gelu` arm; small-shape constraint as in 4b-1 (N ≤ 32 work items on the single SIMD32 thread; larger → `Unsupported` → fallback). Add/Matmul stay byte-identical.

- **ISC-3 — The tolerance-reference model (REQUIRED design resolution; ANSPEC §9.5 refinement).** GELU exposes that **GPU-gelu ≠ cpu-gelu (soft-float) ≠ host-gelu (libm)** — three independent approximations. So "GPU == CPU" is the wrong gate. Resolve and record in ADR-032 / ANSPEC §9.5:
  - **Op-level:** GPU GELU validated against a **host-f32 reference** (the closest-to-true value) within the per-op ≤2⁻¹² relative bound — **not** against soft-float `accel_cpu`.
  - **Forward-pass-level:** the assertion is **not** "GPU-mixed forward == CPU-pure forward" (impossible — both diverge from truth independently). It is: **both** the pure-`accel_cpu` forward **and** the mixed-GPU/CPU forward land within an **accumulated** relative bound `B` of the **host-f32 reference**, where `B` is justified from op-depth × per-op ≤2⁻¹² (state the arithmetic, don't pick a magic number). The soft-float regression (`accel_cpu` vs host) stays its Phase-5 bound unchanged.

- **ISC-4 — Forward-pass-equivalence (the deferred transparency oracle).** Add a `model.neom` disk to the `accel_intel` scenario bundle. Run a real forward pass via `exec::run_with_driver` with `accel_intel` as the driver: **matmul + add + gelu route to the GPU; softmax/layernorm/rmsnorm/embedding fall back to `accel_cpu`**; assert the output lands within `B` (ISC-3) of the host-f32 reference, and that the pure-`accel_cpu` run still matches its Phase-5 bound (BUILD-1 transparency regression intact). The fallback for the non-GPU ops must be transparent (completes, never fails).

- **ISC-5 — Tensor migration on real GPU.** Prove `copy_to_device`/`copy_from_device` are real non-identity transfers between host and the GGTT-mapped device buffer; a tensor migrated cpu→intel→cpu round-trips, a migrated forward matches the reference, no buffer leak — mirroring `accel_sim_migrate_roundtrip` / `virtio_accel_migrate_roundtrip`, now on silicon.

- **ISC-6 — Independent byte oracles (CLAUDE.md rule).** GELU payload hand-laid fixture (in/out ptrs + count at the zeinfo offsets), kernel-EOT scan, the GELU tolerance checker (vs host-f32). Re-lay the command oracle if the batch shape changes. Add/Matmul oracles stay green.

- **ISC-7 — Standards.** Default-off; SPDX; SAFETY + zeinfo/PRM citations; `cargo fmt`; host suite (GELU oracle + Add/Matmul regressions + the forward-pass-equivalence + migration oracles) green; `shard-coverage 4`; `clippy -D warnings`; builds feature-on (ISO) + feature-off. ADR-032 gets the BUILD-4b-2 section (GELU ABI + the ISC-3 tolerance-reference model + the close); ANSPEC §9.5 amended for the mixed-precision reference rule.

## 4. Failure-mode + discriminator section (REQUIRED — blind-hardware rule)
Fault-address capture stays live. Two Stage-B surfaces (GELU op, then the forward pass):

| Mode | Likely cause | Discriminator |
|---|---|---|
| **A. GELU faults** | pointwise payload ABI wrong (≠ matmul's) | fault addr garbage → ISC-1 offsets; `0x0013_xxxx` → buffer mapping |
| **B. GELU retires, off > 2⁻¹²** | wrong reference (compared vs soft-float not host-f32), or the GPU `math` path genuinely exceeds 2⁻¹² | report `max_rel`; if it's a reference-choice bug → ISC-3; if the silicon `math` unit truly exceeds 2⁻¹² → **PL escalation**: that's a real finding, the bound (not the code) is what's wrong |
| **C. Forward pass off > B** | accumulated-bound `B` mis-derived, or a fallback op not actually falling back | per-op intermediate dump: localize which op's output first exceeds host-ref |
| **D. Migration corrupts** | copy_to/from_device aliasing or size error | the round-trip diff + leak check |

Capture-before-you-need-it: if discriminating B (reference choice vs silicon reality) needs the host-f32 per-element reference logged alongside the GPU output, log it.

## 5. Gate (two-stage)
- **Stage A (you):** builds (on+off); GELU oracle + Add/Matmul regressions + forward-pass-equivalence + migration oracles green; host/CI green feature-off; clippy; fmt.
- **Stage B (PL, OptiPlex):** GELU retires within ≤2⁻¹² of host f32 (the tolerance fires non-vacuously — report `max_rel`); the mixed-GPU/CPU `.neom` forward pass lands within `B` of host-f32 with the pure-CPU regression intact; cpu↔intel migration round-trips. Report results + `max_rel` + (on any miss) fault addr + discriminator branch.

## 6. Scope guard
GELU + the integration close only (`accel_intel.rs` dispatch entry + pointwise payload, the GELU kernel via the existing finalizer, the model-disk scenario, migration, `ai-ops-tests`/scenario oracles, ADR-032 + ANSPEC §9.5). **Do NOT** change the 4a/4b-1 substrate or add ops beyond GELU; **do NOT** introduce stateful BTI or a DRM interface (post-Phase-6 hardening). No ELSP, no display.

## 7. Merge + Phase-6 close (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM runs `pm-merge-check` → `--ff-only` → `main`. Then the **Phase-6 close**: bible Article 13 marks Phase 6 **Complete** (accel driver class CPU/sim/virtio + real Intel GPU running matmul/add/gelu with mixed-precision fallback + migration, bare-metal verified); README roadmap flips Phase 6 → Complete, Phase 7 → next; a `Plans/CLOSE-REPORT` records it. The stateful-BTI / DRM-expressibility path stays a documented post-Phase-6 hardening marker (ADR-023).
