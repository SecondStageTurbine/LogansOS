<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-053: Phase 8 BUILD-8.1 — autoregressive decode loop and sampler

**Status:** Accepted — BUILD-8.1 implementation.
**Date:** 2026-06-14
**Design:** `Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md`
**Dispatch:** `Plans/DISPATCH-2026-06-14-phase-8-build-8.1-decode-loop-janus.md`
**Builds on:** ADR-028 (`.neom` v1), ANSPEC-001 §9, the existing `exec::run` governed forward pass.
**Ledger:** no new invariant, no new lock, no new `unsafe`; feature-gated by `ai-generate`.

## The decision

Add a generation layer on top of the existing single-pass executor. The new
`ai-generate` feature compiles:

- `kernel/src/ai/generate_logic.rs`: host-testable sampler, PCG32 PRNG,
  logits slicing, and a pure decode-loop oracle.
- `kernel/src/ai/generate.rs`: kernel glue that builds token-id tensors,
  calls `exec::run` once per generated token, samples the next token from the
  current last-position logits, appends it, and repeats until EOS or
  `max_new_tokens`.

The executor and governance hot-path functions are not modified. Generation is a
consumer of `exec::run`, not a restructuring of the residency path.

## The `.neom` v1 IO contract

BUILD-8.1 confirmed the actual model contract: `.neom` v1 IO shapes are static
and `exec::run` validates tensors exactly against those shapes. A dynamically
growing `[seq]` input tensor would be rejected once its length differed from the
model's declared input length.

The generation contract is therefore bounded-static-context:

- The model has exactly one `I32` rank-1 token-id input.
- The input length is the context window for BUILD-8.1.
- Each decode step fills the prefix with the current tokens and pads the
  remaining slots with the configured `pad_token_id`.
- Rank-2 logits are read from row `current_context_len - 1`; rank-1 logits are
  accepted as an already-selected vocab vector.
- If generation would exceed the declared context window, the call fails closed
  with `ContextFull`.

This is still a full-context recompute every step. It is deliberately O(n^2).
KV-cache state, sliding windows, position-state APIs, and dynamic-shape model IO
are deferred.

## Sampler and PRNG

The sampler supports:

- Greedy argmax, with lowest token id winning ties.
- Temperature scaling.
- Top-k filtering.
- Top-p nucleus filtering.
- Categorical sampling from the renormalized filtered distribution.

The order is temperature, then top-k, then top-p, then categorical sampling.
Sampling uses PCG32 XSH-RR from Melissa O'Neill's minimal C PCG reference
implementation. It is documented as non-cryptographic and deterministic. The
host KAT pins seed `(42, 54)` against the published `pcg32-demo` first-round
sequence:

`a15c02b7 7b47f409 ba1d3330 83d2f293 bfa4784b cbed606e`

References:

- `https://www.pcg-random.org/using-pcg-c-basic.html`
- `https://github.com/imneme/pcg-c-basic`

## Proof discipline

The gate is I-12 known-answer testing, not round-trip testing:

- Host KATs assert the PCG32 published sequence.
- Host KATs assert greedy, temperature, top-k, and top-p sampler outputs from
  hand-computed logits and the seeded draw.
- Host KATs assert the decode loop sees the full growing context at each step
  and stops on EOS.
- Host KATs validate the tiny hand-laid `.neom` oracle's bytes parse as a
  single-input, single-output Embedding model.
- The QEMU scenario `ai_generate_decode` loads that `.neom` and proves the
  kernel generation path returns `[2, 3, 4]` from prompt `[1]`, with `4` as EOS.

The tiny `.neom` is intentionally simple: an `Embedding` table is also the
logit table. Row 1's argmax is token 2, row 2's argmax is token 3, and row 3's
argmax is EOS token 4. The oracle is human-computable byte-for-byte.

## Anti-scope

No tokenizer, no KV cache, no quantization, no beam search, no speculative
decode, no model-format expansion, and no executor/governance hot-path edits.
The path operates on token IDs and f32 logits only.

## Consequences

LogansOS now has the first serving primitive: a model can be driven
autoregressively rather than only run for one forward pass. The result is
correctness-first and intentionally unoptimized, giving BUILD-8.2 a stable
reference when KV-cache state becomes governed tensor state.
