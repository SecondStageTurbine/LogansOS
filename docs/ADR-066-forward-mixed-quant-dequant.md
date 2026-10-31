<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-066: Mixed-Quant Forward Matmul via Native Quant Dots

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-06-22 |
| Related | OT-28; OT-30; ADR-063; ADR-064; `Plans/THINK-2026-06-21-forward-path-real-model-integration.md`; `Plans/DISPATCH-2026-06-22-forward-path-f1-mixed-quant-matmul-maincoder.md`; `Plans/DISPATCH-2026-06-22-forward-path-perf-profile-fix-jarvis.md`; `Plans/DISPATCH-2026-06-22-forward-path-f4-native-quant-kernels-jarvis.md` |

## Context

The Qwen2 forward path had one quantized fast path: q4_K weights multiplied by q8_K activations. The real Qwen2.5 model is mixed-quant, dominated by q5_0 with additional q6_K, q8_0, q4_K, and f32 tensors. That made the native q4_K path insufficient as the only quantized matmul route.

## Decision

F1 initially routed every supported quantized forward weight type through the
landed `gguf_dequant_logic` dequant-to-f32 primitives, transposed the GGUF
column-major tensor payload into the existing f32 matmul RHS layout, and
dropped the scratch buffer after that matmul. OT-30 removed the full RHS
materialization and dotted each quantized block directly against the f32
activation vector.

F4 keeps the same default-off `phase9-real-quant` boundary but changes the hot
path again: supported quantized weights now dot against quantized activations
using native block kernels. K-quant weights use q8_K activations (`q4_K x q8_K`,
`q6_K x q8_K`). The 32-lane legacy formats use q8_0 activations
(`q5_0 x q8_0`, `q8_0 x q8_0`), matching llama.cpp's raw-source contracts and
keeping Qwen's 896-wide hidden vectors valid without padding.

When `phase9-real-quant` is disabled, the fixture path remains the previous
behavior: the existing q4_K native path is available under
`phase9-quantized-matmul`, and the f32 fallback remains unchanged.

## Consequences

This adds no new invariant. Correctness composes the existing dequant byte
oracles with two F4 gates: hand-laid native quant dot byte-oracles for
q4_K/q5_0/q6_K/q8_0, and native-vs-dequant-reference equivalence KATs at
Qwen-shaped widths. The dequant-to-f32 path stays available as the test
reference; it is no longer the real-quant hot path.

The `.neom` format is not changed in F1. Current imported quant tensors remain opaque U8 payloads, so F1 carries forward-layer dtype metadata at the Qwen2 weight role boundary. F2 remains responsible for broader data-driven model configuration.

## OT-30 Amendment: Profile-First Fix

The OT-30 profile pass ruled out the build profile as the primary cause: the
`ai-ops-tests` harness compiles the path-modded kernel AI modules inside the
release test crate, and `cargo test --release` reports the optimized release
profile. The release failure mode was still effectively the same as debug:
the Stage-A harness did not produce a first token after roughly 11 minutes in
release, versus more than 15 minutes in debug, so optimization inheritance was
not the smoking gun.

The localized root cause was the combination of allocation and compute shape:
the F1 path dequantized each quantized GGUF tensor into a full f32 tensor,
transposed it through `gguf_col_major_to_ops_rhs`, and then entered the generic
`ops::matmul` path, whose inner loop uses `soft_f32`. The old real-model
harness also replayed the whole context for every generated token, turning a
16-token decode into 1 + 2 + ... + 16 = 136 forward passes. Allocation evidence
matched that diagnosis: zero-filled f32 placeholders for quantized layer
weights accounted for 1,431,306,240 bytes per model layer-buffer sweep, and
the old output head materialized 136,134,656 f32 elements
(544,538,624 bytes) before also building the transposed RHS.

The accepted OT-30 correction keeps the feature default-off and changes only
the real-quant forward path. Quantized RHS storage is now accepted without a
f32 placeholder, quantized GGUF payloads are borrowed by the host harness
instead of cloned, and supported q4_K/q5_0/q6_K/q8_0 blocks are dotted directly
against the activation vector. The generic f32 matmul and fixture behavior are
unchanged. The Stage-A harness now performs a prompt prefill once, keeps one
KV cache, and feeds generated tokens one at a time.

Validation after the fix:

- F1 byte-oracle KAT remains green under `phase9-real-quant`.
- Stage-A real `.neom` release validation completes 16 tokens with top-1
  parity against llama.cpp:
  `generated=[271, 40, 1079, 4460, 311, 1855, 264, 4285, 2025, 429, 686, 1896, 264, 914, 323, 264]`,
  `first_divergence=None`, `finite=ok`, `deterministic=ok`.
- The measured first decode is 21.807 seconds for 16 tokens, or
  1362.980 ms/token, inside the D5 0.5-2.0 s/token target band.

This amendment adds no new unsafe block and no new invariant. Native
in-register quant kernels were left for F4 after OT-30 made the model runnable
and showed direct blockwise scalar dotting was the next floor.

## F4 Amendment: Native Quant Kernels

F4 adds native q5_0/q6_K/q8_0 dot kernels beside the existing q4_K x q8_K
implementation. Each has a scalar entry point and an explicit
`QKMatmulKernel::AutoAvx2` selector. The selector remains hermetic: it checks
CPUID, OSXSAVE, and XCR0 before entering `#[target_feature(enable = "avx2")]`
bodies, and otherwise falls back to scalar. The new q5_0/q8_0 AVX2 bodies use
signed i8 lane dot products; q6_K keeps the native scalar integer depacker under
the same selector while avoiding a riskier SIMD depack in this slice. The q5_0
matmul wrapper probes AVX2 once per matrix and then stays in the selected loop;
it does not re-run CPUID/XCR0 for every output-column dot.

The q8 activation split is intentional. llama.cpp's raw-source CPU table pairs
q5_0 and q8_0 with q8_0 activations, and q6_K with q8_K activations. Forcing
q8_K onto q5_0/q8_0 would require 256-wide activation blocks and would reject
Qwen2.5's 896-wide hidden vectors. The implementation therefore quantizes
activations once per row into the matching q8 block class and dispatches by
weight dtype.

F4 validation pins both correctness anchors:

- `forward_f4_native_quant_dot_byte_oracles_scalar_and_auto`: hand-laid
  q4_K/q5_0/q6_K/q8_0 blocks, exact 127-hot q8 activation, scalar and AutoAvx2
  bit-identical.
- `forward_f4_native_vs_dequant_reference_qwen_shape_kat`: native output equals
  the retained dequant-to-f32 reference at Qwen hidden/intermediate widths when
  the q8 activation quantization is exact.
- `forward_f1_mixed_quant_direct_matmul_path_kat`: the earlier mixed-quant path
  KAT remains green under `phase9-real-quant`.
- QEMU `forward_mixed_quant`: the kernel scenario exercises the same dtype
  spread with an exact 127-hot activation so native q8 quantization cannot hide
  scale-rounding drift behind the fixture.
- Post-F2 config compatibility: `.neom` v2 imports now keep table offsets
  8-byte aligned, the tool validator mirrors the kernel's table alignment
  check, and `derive_model_config` handles the real importer shape where
  quantized tensors are stored as raw `U8` block payloads.
- `forward_f4_q5_0_native_perf_probe_when_requested` on the real
  `blk.0.attn_q.weight` q5_0 tensor reports q5_0 native faster than the
  retained dequant reference: 0.191 ms native vs 0.773 ms reference per matmul
  over 20 release iterations, speedup 4.038.
- Stage-A real `.neom` release validation remains top-1 identical to llama.cpp:
  `generated=[271, 40, 1079, 4460, 311, 1855, 264, 4285, 2025, 429, 686, 1896, 264, 914, 323, 264]`,
  `first_divergence=None`, `finite=ok`, `deterministic=ok`. Measured latency is
  22.966 seconds for 16 tokens, or 1435.432 ms/token, with F2
  `phase9-data-driven-config` and F4 native quant both enabled.

The F4 native quant kernels reuse the existing AVX2 target-feature entry-point
and CPUID/XCR0 guard discipline. No new lock, invariant, syscall, graph,
tokenizer behavior, or `unsafe` surface is introduced.

## OT-16 B3 Amendment: Q5_K Preservation and Recurrent Parity

The Qwen3.5 0.8B Q4_K_M reference stores every GDN QKV projection as Q5_K.
Expanding those 18 tensors to F32 changed llama.cpp's quantized activation/dot
rounding and failed strict recurrent parity. The importer now preserves the raw
176-byte `block_q5_K` payloads and the shared Q8_K matmul dispatch evaluates
them directly.

Q5_0 and Q5_K both occupy 5.5 bits per value, so matrix geometry plus byte
length cannot distinguish them. Tensor-name inference would move architecture
knowledge into ring 0 and is rejected. The `.neom` tensor dtype ABI therefore
adds one explicit storage tag:

| ABI | dtype | storage/shape rule |
|---:|---|---|
| 7 | `Q5K` | raw GGML `block_q5_K` bytes; rank-1 opaque byte-payload shape; one storage byte per declared shape element |

Existing ABI values 0..6 and existing U8 quant payloads are unchanged. The
model loader materializes dtype-7 weights as byte tensors while retaining the
Q5_K metadata tag for data-driven projection dispatch. Writer and reader tests
pin ABI value 7 and a hand-laid Q5_K payload byte-for-byte.

The fused GDN recurrence also follows llama.cpp's 128-lane reduction order:
four eight-lane accumulators, the same pairwise reduction tree, state updates
with fused multiply-add rounding, and Q scaling after the completed recurrent
dot. The no-unsafe correctness implementation widens the multiply-add to F64
and rounds once to F32; a faster bit-identical implementation remains required
before the kernel scenario is considered performance-ready.

With the preserved Q5_K model, the host greedy oracle for raw `Hello` is strict
for 32 tokens and deterministic on a second run:

```text
11,353,1044,264,5286,506,264,11759,13,353,1044,4237,264,5392,383,279,8282,314,328,760,14955,314,279,3130,303,279,10499,314,279,36317,3158,353
```

The correctness reference measured 215,688 ms for the first 32-token decode
(6,740.271 ms/token). This is parity evidence, not a performance acceptance.
