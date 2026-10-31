<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE RETURN: OT-16 B3 Qwen3.5 Gated-DeltaNet bring-up

**From:** Jarvis

**Branch:** `codex/ot16-b3-qwen35-deltanet-bringup-jarvis`

**Status:** CLOSE RE-SPIN — OT-59 cleared; B3 AAG byte-oracle findings addressed

## Deliverable

B3 imports and executes the real Qwen3.5-0.8B Q4_K_M hybrid through the
`.neom` v4 data-driven walker: 18 Gated-DeltaNet layers, six full-attention
layers, preserved raw Q5_K projections, persistent owned recurrent/KV state,
and fail-closed in-kernel generation checking.

The OT-59 shootdown-readiness and virtio-blk I/O-gate fixes are on `main`.
The previously parked gate was re-run green on `debc038c`:

- push CI: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/29657914271
- workflow-dispatch QEMU: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/29657922861

The AAG-bounce re-spin is re-gated on its final head; the exact-head URLs are
reported in the PM RETURN evidence block.

## AAG bounce findings closed

### F1 — independent 32-token oracle anchor

`fixtures/qwen35_llama_cpp_hello_golden.rs` is the single golden consumed by
the kernel syscall oracle, `neo-arthur`, and xtask's serial discriminator. Its
provenance is committed beside the bytes:

- GGUF SHA-256:
  `f5b14da98939b60bbe1019a964eba656407e1e0b64f1fe3003ff6d650e93bfec`
- llama.cpp commit:
  `5f04dc7ac3f40271f60105574be0f617887195bb` (version 9638)
- raw greedy `/completion`, prompt `Hello`, `n_predict=32`, `temperature=0`,
  `seed=0`, with token IDs taken directly from the 32 raw JSON
  `completion_probabilities[].id` entries.

```text
11,353,1044,264,5286,506,264,11759,13,353,1044,4237,264,5392,383,279,
8282,314,328,760,14955,314,279,3130,303,279,10499,314,279,36317,3158,353
```

The non-environment-gated host KAT
`qwen35_llama_cpp_greedy_32_golden_is_the_kernel_oracle` makes drift red in
ordinary CI.

### F2 — malformed v4 RoPE config

The kernel model boundary now rejects `rope_dimension_count == 0` before
forward execution, matching the tool validator and ADR-072. The negative host
KAT `neom_config_qwen35_rejects_zero_rope_dimension_count` asserts
`ModelFormatError::InvalidConfig`.

### F3 — real-brain coverage is explicit

When the model artifact is absent, xtask emits a machine-greppable protocol
record instead of an easy-to-misread green omission:

```text
NEO_TEST/1 event=SKIP name=arthur_real_brain_boot_qwen35 reason=model_artifact_unavailable coverage=real_brain_not_executed required_env=NEO_STAGE_B_NEOM_or_NEO_REAL_NEOM
```

The model-present scenario was also re-run once on the re-spun source tree with
`NEO_SMP=4` and `NEO_TEST_ATTEMPTS=1`. Its archived serial evidence is under
`Plans/evidence/`; it proves 32 tokens, 18 GDN/six attention layers, recurrent
state carry, exact golden equality, and terminal PASS.

## AAG inputs

- **Completeness:** real model-present ring-0 generation PASS plus formal CI
  skip coverage marker when the external artifact is unavailable.
- **ABI:** `.neom` v4, Q5_K preservation, committed independent greedy golden,
  and zero-RoPE malformed input rejected at the kernel boundary.
- **Memory safety:** owned `Vec<f32>` recurrent/conv/KV state; zero new `unsafe`;
  checked file-derived lengths before slicing.
- **I-14 / interrupts:** no governance lock, DMA, ISR, EOI, or APIC work in the
  GDN forward/decode path.
- **Concurrency:** sequential per-token decode; stack-local
  `InventoryDecodeState`; immutable shared model weights.

The shipped GDN reduction is safe scalar FMA-compatible math
(`recurrent_state_dot` plus safe `fused_mul_add_f32` widening), not the rejected
AVX2 `unsafe` alternative.
