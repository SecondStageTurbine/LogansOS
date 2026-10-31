<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-058: Phase 9 quantized Q4_K x Q8_K matmul

**Status:** Accepted
**Date:** 2026-06-17
**Dispatch:** `Plans/DISPATCH-2026-06-17-phase-9-build-9.1-quantized-matmul-scalar-jarvis.md`
**Design:** `Plans/THINK-2026-06-17-phase-9-quantized-matmul-design.md`
**Scope:** Phase 9 scalar op, default-off forward wiring, and host-tier AVX2
equality gate; no GPU, no raw-only storage, no `.neom` v2, no new invariant.

## Decision

Add the Phase 9.1 scalar quantized-matmul helpers under
`kernel/src/ai/ops/`:

- `block_q8_K` activation quantization from f32 using llama.cpp's
  `iscale = -127 / max` rule and per-16 `bsums`.
- Scalar `block_q4_K x block_q8_K` integer dot using the existing raw
  Q4_K 144-byte block layout, 6-bit scale/min depacking, i32 accumulators,
  and the `bsums` min-correction.
- A small row-by-row matrix helper for future 9.2 use, still standalone and
  not called by the forward path in this build.

The helpers are compiled by the kernel only with the default-off
`phase9-quantized-matmul` feature. The host KAT crate path-mods the same files
under test cfg. With the feature off, no forward path, executor, importer, or
decode-loop behavior changes.

## Source Discipline

The byte layouts and scalar math are pinned to raw source:

- ggml-org/llama.cpp `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3`
  - `ggml/src/ggml-common.h`: `QK_K`, `K_SCALE_SIZE`, `block_q4_K`,
    and `block_q8_K`.
  - `ggml/src/ggml-quants.c`: `quantize_row_q8_K_ref`.
  - `ggml/src/ggml-cpu/quants.c`: `ggml_vec_dot_q4_K_q8_K_generic`.
  - `tests/test-quantize-fns.cpp`: cosine input generator and
    `MAX_DOT_PRODUCT_ERROR = 0.02`.

No WebFetch-derived values are authoritative.

## Proof

The gate is I-12 known-answer testing:

- A hand-laid `block_q8_K` byte oracle checks exact `d`, 256 quant bytes, and
  16 little-endian `bsums`.
- A scalar Q4_K x Q8_K KAT uses the llama.cpp cosine-data generator shape
  (`0.1 + 2*cos(i + offset)`, N=4096), a deterministic raw `block_q4_K`
  fixture, and a source-transcribed llama.cpp scalar dot reference. The
  normalized delta is checked against the ratified `<= 0.02` tolerance.
- The matrix helper is exercised as a 1x1 wrapper around the same dot to
  prove shape/block plumbing without wiring it into generation.

## Phase 9.2 Wiring Note

BUILD-9.2 wires the 9.1 helper into `forward_logic` behind the same
default-off `phase9-quantized-matmul` feature. The f32 `matmul_vec()` path
remains present and is selected whenever the feature is off or a tensor has no
raw Q4_K sidecar. When the feature is on and a raw Q4_K RHS is present,
`matmul_vec_quantized()` quantizes the f32 activation to Q8_K blocks and calls
the scalar Q4_K x Q8_K matrix helper.

Qwen2 layer weights carry dual storage during this slice: the existing
dequantized f32 slices plus optional raw Q4_K bytes for the quantized path.
This intentionally raises transient host memory footprint so the f32 fallback
and kill-switch regression remain live until 9.3 proves the accelerated path.

The 9.2 gate is: feature off keeps the existing 8.5 f32 KATs unchanged, and
feature on generates the same top-1 token as the f32/llama.cpp `Hello` oracle.
The quantized logits are not required to be bit-identical to f32 logits.

## Phase 9.3 AVX2 Note

BUILD-9.3 adds a host-tier AVX2 selector for the same Q4_K x Q8_K dot:
`_mm256_maddubs_epi16`, widen-accumulate through `_mm256_madd_epi16` into i32
lanes, and a horizontal i32 reduction. The scalar kernel remains the default
entry point and the oracle. The AVX2 path is selected only through
`QKMatmulKernel::AutoAvx2`, and only when runtime CPUID/XCR0 checks prove both
AVX2 support and OS-enabled XMM/YMM state.

The 9.3 gate is exact equality against the 9.1 scalar result: integer math is
identical through the scaled-dot and min-correction accumulators, so the final
float result must be bit-identical rather than tolerance-bounded.

The new `unsafe` in 9.3 is limited to the AVX2 intrinsic bodies and XCR0 read,
with `/// SAFETY` preconditions on each target-feature entry. It is
host-tier-only in this slice. The in-kernel forward path is not swapped to AVX2
because the current kernel does not preserve YMM high halves across context
switches/IRQs/syscalls. The syscall/timer paths use FXSAVE/FXRSTOR and the
context switch saves only GP callee-saved state, so in-kernel YMM use is blocked
until the separate OT-23 XSAVE/XRSTOR state-preservation slice lands.

## Anti-Scope

Phase 9.1 did not modify `forward_logic`, store raw GGUF Q4_K beside the f32
fallback, add tensor ABI dtypes or an executor `OpKind`, add AVX2, or target
the GPU. Phase 9.2 only wires the default-off forward selector and dual
storage; it still does not add AVX2, latency measurement, raw-only storage, a
new `OpKind`, GPU kernels, new invariants, or new `unsafe`. Phase 9.3 adds the
host AVX2 kernel and its equality gate only; it does not add latency
measurement, raw-only storage, a GPU path, an in-kernel AVX2 forward swap, or
the YMM-state preservation fix.
