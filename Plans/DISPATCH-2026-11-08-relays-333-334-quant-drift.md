<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 333–334 (2026-11-08): quantization drift, the reference control and the review

Read with `git fetch origin && git show origin/main:<path>`. Your relay-323 RETURN is
`Plans/RETURN-2026-11-08-quant-drift-dexter.md`; the build is PR `dexter/quant-drift` (opened by the PM).
Answer pass/fail with `file:line`; push the RETURN to the branch named.

## Why a control first

Relay 323 missed bar A (16/30 identical streams) and bar B (65.3% top-1 vs float32). **The bars stay as written.**
Two numbers don't fit together, though:
- the Q4_K_M model scores *lower* perplexity than float32 on the window (29.11 vs 30.75);
- your earlier full pass had llama.cpp Q4_K_M and float32 within 0.04% (23.971 vs 23.963).

A quantized model that beats full precision while disagreeing on a third of next tokens points at the reference
before it points at Q4_K_M. Bar B's verdict can only go to J.c. as a serving decision once the reference passes a
control. Bar A's teacher-forced number (89.4%) also rests on llama.cpp per-position rows, which you flagged as
unreliable.

## Section 333: Dexter, the control and the two fixes. DeepSeek, high: same harness, one more lens.
Branch: `dexter/quant-drift` (push on top; the PM keeps its PR open).
- [ ] **Control C (new, set now, before running):** llama.cpp on a **Q8_0** GGUF of the same pinned model (or F16),
  against float32, same IDs and window. Bar: teacher-forced top-1 ≥ 98% and ΔPPL ≤ 1%. **If C misses, the float32 or
  llama.cpp reference is wrong.** Find the cause (dtype, tied embeddings, rope base, attention mask, position IDs,
  BOS/chat template, last-position offset) before any verdict on B stands.
- [ ] **llama.cpp per-position logits:** use a `llama_batch` with `logits = true` on every position (the public API),
  and confirm the result against the final-position logits at a sample of positions. Then report ours-vs-llama.cpp
  teacher-forced top-1 again.
- [ ] **At each bar-A divergence**, report the top-1/top-2 logit gap in both runtimes. That says whether each one is a
  near-tie (numerics) or a clear gap (a forward-pass bug). Bar A stays a FAIL either way. The gaps decide whether the
  next step is a bug hunt or a tolerance THINK for J.c.
- [ ] **Rerun B** with the fixed reference, and record why Q4's perplexity came out below float32's.
- [ ] **The full 8,192-token window**, if the run fits; otherwise say why not.
- [ ] Re-pin the KAT with the new numbers. Leave the old KAT in the history, not deleted.
- **Close condition:** a hypothesis plus a named falsification test for each miss that's left.
RETURN: `Plans/RETURN-2026-11-08-quant-drift-control-dexter.md` on `dexter/quant-drift`, pushed.

## Section 334: Jarvis, review of the quant-drift harness. **MODEL** GPT-6-Luna, high: a contained host-only tool.
Run it after 333 lands (the PM will post the head). Read `tools/quant-drift-tests/`.
- [ ] The harness `mod`s the kernel's real forward-pass, GGUF, ops and tokenizer code (no copy), so it measures the
  shipped path. Pass/fail plus line.
- [ ] Every input is pinned by hash (`.neom`, GGUF, float32 revision, llama.cpp commit, corpus slice and window).
  Pass/fail.
- [ ] All three runners consume identical token IDs and the same window. Pass/fail plus line.
- [ ] Bars A, B and C are computed exactly as the dispatches define them, with no tolerance added. Pass/fail plus line.
- [ ] `cargo test` stays hermetic without `QUANT_DRIFT_NEOM`, and no corpus text is committed. Pass/fail.
- [ ] The licences listed (llama.cpp MIT, transformers Apache-2.0, PyTorch BSD-3, Qwen Apache-2.0, WikiText CC-BY-SA as
  hashes only) are right, and no source is copied. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-quant-drift-review-jarvis.md` on `codex/quant-drift-review-jarvis`.
