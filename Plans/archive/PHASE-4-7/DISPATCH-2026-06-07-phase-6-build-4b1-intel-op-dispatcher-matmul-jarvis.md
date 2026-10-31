<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4b-1 — generalize the Intel op dispatcher + land matmul → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `341097b`: `codex/phase-6-build-4b1-intel-op-dispatcher-jarvis`)
**Status:** DISPATCHED.
**Builds on:** BUILD-4a (`ab8fa0d`, Intel UHD 630 GPGPU `[4,6]` on silicon) + the THINK `Plans/THINK-2026-06-07-phase-6-build-4b-op-set-equivalence.md`.

## 0. Design decisions (resolved — THINK Q1/Q2/Q5)
The THINK left three open. Resolutions, with the load-bearing one called out:
- **Q1 (coverage):** representative GPU ops + `accel_cpu` fallback (Article 6.5), **not** full per-op GPU coverage. BUILD-4b-1 lands the dispatcher infra + **one workhorse op (matmul)**; the rest fall back to CPU and produce the oracle result. Further ops are later 4b slices.
- **Q2 (FP tolerance — the one real decision):** adopt the **Phase-5 precedent**, not a new policy: integer / data-movement ops **byte-exact (0 ULP)**; transcendental / accumulating ops (matmul included) within **≤ 2⁻¹² relative error vs host f32** — and the reference is **host f32**, not `accel_cpu` soft-float (hardware f32 tracks host f32 more closely than soft-float). This is continuity with how Phase 5 gated its CPU transcendental backend, so it is not a fresh assumption. *(PL: if you want tighter/looser bounds, say so and I'll amend the gate — but the build proceeds on the precedent.)*
- **Q3:** stay **stateless A64** (proven in 4a). **Q4:** keep the driver DRM-expressible (interface constraint), build no DRM interface. **Q5:** this is **4b-1** (dispatcher + matmul, Stage-A + a Stage-B matmul proof); the full-op-set forward-pass-equivalence + migration close is **4b-2**.

## 1. What exists today (the real starting point)
`IntelAccelDriver` already `impl AccelDriver` and self-registers (`accel_intel.rs:482`, `:219`); `copy_to/from_device`, `poll`, `wait` exist. But `dispatch` is **hard-wired to the Add** (`submit_add`, `:537/:593`) — it handles exactly one op. The proven dispatch substrate (legacy ring, GGTT, STATE_BASE_ADDRESS, the NEO indirect-data payload with per-thread local_id r1–r6 / cross-thread r7–r9, fault-address capture) is all there from 4a and is **op-agnostic** — only the kernel blob + the cross-thread arg payload change per op.

## 2. Scope — generalize the dispatcher, add matmul
Turn `dispatch` from "Add-only" into a **kernel-table op dispatcher**:
- An opcode → `{ EU-kernel blob, zeinfo arg ABI, payload builder }` table.
- For a **covered** opcode: build the cross-thread payload from the `OpBatch` operands (the op's input/output `DeviceStorage` A64 pointers + scalar dims), dispatch on the 4a path, read back, apply the Q2 tolerance.
- For an **uncovered** opcode: return the "not handled here" signal so the executor uses `accel_cpu` fallback (confirm the existing fallback mechanism in `ai::exec` / the registry; if none exists, the minimal hook is part of this build — a driver returning `AccelError::Unsupported` must route the op to `accel_cpu`, not fail the batch).
- **Covered op this slice: matmul** (`ai::ops` matmul opcode). The Add stays covered (regression).

## 3. ISCs
- **ISC-1 — Author + finalize the matmul EU kernel.** Add `intel_gen9_kernels/matmul_f32.cl` (a straightforward `C[m,n] = Σ_k A[m,k]·B[k,n]`, stateless A64 via `-cl-intel-greater-than-4GB-buffer-required`). Run the **existing** `tools/intel-gen9-finalizer` pipeline (pinned IGC/ocloc — it's checked in and reproducible) to emit the `.text` blob; commit the blob + the `ocloc disasm` (`matmul_f32.gen9.asm`) and `matmul_f32.gen9.zeinfo` byte oracles + the provenance row in `intel_gen9_kernels/README.md`, exactly as the Add did.
- **ISC-2 — Extract the matmul cross-thread ABI from its zeinfo.** The arg layout (A/B/C A64 pointers + M/N/K dims + the SIMD/GRF execution env) is *this kernel's* contract — read it from `matmul_f32.gen9.zeinfo`, do not assume the Add's offsets. Record the offsets/sizes in ADR-032.
- **ISC-3 — The kernel table + per-op payload builder.** Replace `submit_add`'s hard-wiring with a table keyed by opcode; the matmul payload builder lays the A/B/C pointers + dims per ISC-2. Keep the Add entry. The walker dims come from the op's work size (matmul: M·N elements; pick the tiling that the kernel was compiled for — read it from the zeinfo `simd_size`/walk order, don't guess).
- **ISC-4 — Fallback wiring.** Uncovered opcode → `accel_cpu`. Verify/implement the executor path so a single unsupported op in a forward pass transparently runs on CPU and the batch still completes (the BUILD-1 transparency oracles must stay byte-identical for any op that isn't GPU-covered).
- **ISC-5 — Tolerance check (Q2).** matmul result vs **host f32** within ≤ 2⁻¹² relative; the Add stays byte-exact. Encode the bound as a named constant with the ADR-032 justification (not a magic number).
- **ISC-6 — Independent byte oracles (CLAUDE.md rule).** matmul's cross-thread payload is a new on-wire layout → a **hand-laid byte fixture** in `tools/ai-ops-tests` asserting A/B/C pointers + dims at their zeinfo offsets, independent of the encoder. Re-lay the command-byte oracle for any batch-shape change. Cross-check against `matmul_f32.gen9.zeinfo`.
- **ISC-7 — Standards.** Default-off (`accel-intel`); SPDX; SAFETY + zeinfo/PRM citations; `cargo fmt`; host suite (new matmul payload oracle + the Add regression oracles + the fallback-transparency oracles) green; `shard-coverage 4`; `clippy -D warnings`; kernel builds feature-on (ISO) + feature-off. ADR-032 gets a BUILD-4b-1 section (the matmul ABI + tolerance + the dispatcher generalization).

## 4. Failure-mode + discriminator section (REQUIRED — CLAUDE.md blind-hardware rule)
The 4a fault-address capture stays in place, so the Stage-B matmul run is self-diagnosing. Expected modes if matmul doesn't land within tolerance:

| Mode | Likely cause | Discriminator |
|---|---|---|
| **A. Fault (RING_FAULT≠0)** | matmul payload pointers in wrong GRF (its ABI ≠ the Add's) | fault addr: garbage → payload/ABI wrong (re-check ISC-2 offsets); a real `0x0013_xxxx` → buffer mapping/size |
| **B. Retires, result wrong-but-structured** | tiling / M-N-K dim wiring or walker dims wrong | dump a small known A·B; compare element [0,0] and a corner; mismatch pattern localizes dim/stride vs accumulation |
| **C. Retires, off only in low bits** | hardware-FP vs host f32 within-but-near tolerance | report max relative error; if ≤2⁻¹² → PASS; if just over → Q2 bound revisit (PL), not a code bug |
| **D. Hang (no retire)** | walker thread-group sizing for the larger matmul payload | last breadcrumb + INSTDONE (the 4a EU-idle vs stuck signature) |

Capture-before-you-need-it: if discriminating A from B needs the assembled matmul payload dumped as the GPU sees it, add that dump in this build.

## 5. Gate (two-stage)
- **Stage A (you):** builds (on+off); matmul payload byte oracle + Add regression + fallback-transparency oracles green; host/CI green feature-off; clippy; fmt.
- **Stage B (PL, OptiPlex):** the matmul scenario retires and matches host f32 within ≤2⁻¹²; the Add still passes; a forward pass with matmul GPU-routed + other ops CPU-fallback matches the oracle. Report the result + max relative error + (on any miss) the fault addr + discriminator branch.
  - **Refinement (isolate the unknowns — iter7→iter8 lesson):** the **first** Stage-B matmul case uses **exactly-representable inputs** (small integers, e.g. 2×2·2×2 with exact f32 products/sums) so it is **byte-exact (0 ULP)** — a mismatch is then unambiguously a dim/index-wiring bug (discriminator mode B), not FP tolerance (mode C). Add a **second** case with non-exact values to exercise the ≤2⁻¹² bound. Don't conflate the two on the first blind run.

## 6. Scope guard
Dispatcher generalization + matmul only (`accel_intel.rs` dispatch table + payload builder, the matmul kernel via the existing finalizer, `ai-ops-tests` oracles, the `accel_cpu` fallback hook, ADR-032). **Do NOT** change the 4a dispatch substrate (STATE_BASE_ADDRESS, ring, GGTT, the indirect-data mechanism — all proven), introduce stateful BTI, or add other ops (later 4b slices). No DRM interface, no ELSP, no display.

## 7. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM runs `scripts/pm-merge-check.sh` → `--ff-only` to `main`. BUILD-4b-1 lands; 4b-2 (remaining ops + forward-pass-equivalence + migration close) follows and closes Phase 6.
