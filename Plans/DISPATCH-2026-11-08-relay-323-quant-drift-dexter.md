<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 323: Dexter builds the quantization-drift check (Arthur's Q4_K_M model vs full precision)

J.c. (2026-11-08): the one real accuracy unknown is the model's quantization. The per-op KATs pin quantized math
against reference values (`ai-ops-tests`, real-quant), and `real_model_stage_a_forward_validation_when_available`
compares our forward pass with llama.cpp's greedy tokens for one prompt. Nothing yet measures **accumulated drift
across full forward passes** shifting token choice, against the full-precision model. Pin it. **The acceptance bars
below are set now, before any run; a run reports pass or fail against them, not a number with no decision.**

**MODEL** DeepSeek, **EFFORT** high: a bounded host-only build (no kernel change), your lane.
- WORKTREE: `~/workers/dexter` on the always-on box. BRANCH: `dexter/quant-drift` from `origin/main`.
- RAG is not available there; use grep and direct reads.

## What to build
A host-only crate `tools/quant-drift-tests/` (next to `ai-ops-tests`; reuse its loaders and our forward pass), run on
demand like the existing `*_when_available` tests (env vars point at the artifacts; skips cleanly without them, so CI
stays hermetic), plus a small script to produce the reference outputs.

**Three models, one tokenizer.** The vocabulary is lossless (binary = text, proven), so token IDs are directly
comparable.
1. **Ours:** Arthur's shipped `.neom` (Qwen2.5-0.5B-Instruct Q4_K_M, pinned in `kernel/src/boot_model_logic.rs`),
   run through **our** forward pass on the host.
2. **llama.cpp Q4_K_M:** the same GGUF the `.neom` was imported from, run by llama.cpp (pin its release/commit).
3. **Full precision:** `Qwen/Qwen2.5-0.5B-Instruct` safetensors at a pinned revision, run host-side (transformers in
   float32, or llama.cpp on an F16/F32 conversion — say which).

**Eval set.**
- **Perplexity corpus:** a fixed WikiText-2 (raw) test slice, at least 8,192 tokens, fetched raw with `git`/`curl`
  and pinned by BLAKE3. **Don't commit the corpus** (CC-BY-SA, not our docs licence): commit its source, offsets and
  hash, and the results.
- **Arthur's prompts:** LogansOS has no telemetry, so there is no usage sample. Use the prompts Arthur is actually
  given: the boot prompt (the exact 21-token prompt, prompt #1), every prompt in the Arthur and desktop scenarios
  (`tools/xtask`), and J.c.'s preview questions (`Plans/FEEDBACK-2026-11-02-preview.md`: "what's 2+2", "what time is
  it", …). At least 24 prompts, each wrapped in the chat template Arthur uses. List them in the crate.

**Method.** Greedy decode, 32 new tokens per prompt, identical token IDs in. Per prompt: the generated IDs, the
first position where two models diverge, and per-position top-1 agreement. Per model: perplexity on the corpus.
Commit the reference outputs (token IDs, ΔPPL, agreement, divergence positions) as a pinned KAT artifact with the
model, corpus and tool hashes, so any future re-quantization or forward-pass change is regression-checked against
it instead of re-blessed by review.

## Acceptance bars (fixed before running)
**A. Implementation fidelity: ours vs llama.cpp on the same Q4_K_M file.** This must be essentially exact, since it's
the same quantized weights.
- Greedy tokens identical on **100%** of prompts for all 32 tokens.
- Perplexity within **0.5% relative** of llama.cpp's on the corpus.
- A miss here is a **bug in our forward pass**: bounce with the first diverging prompt and position.

**B. Quantization loss: Q4_K_M vs full precision.** This is inherent to the format, so the bar asks whether it's
acceptable to serve.
- ΔPPL **≤ 10% relative**.
- Top-1 next-token agreement **≥ 90%** over all prompt positions.
- The boot prompt's greedy reply **identical** to full precision's.
- A miss here is a **decision, not a bug**: the PM opens a THINK on serving Q5_K_M, Q6_K or Q8_0 instead (size,
  load time and Zenbook latency against the drift), for J.c.

## Close
- Show the three bars' results in the RETURN, with the pinned hashes.
- `scripts/worker-close-gate.sh`. Push, open a PR to `main`, and paste the push-CI URL.
- Before pushing, check your diff for personal details: Windows user paths, LAN addresses, MACs, hostnames, emails.
- Prior art: name what you use (llama.cpp's `llama-perplexity`, transformers), each with licence and pinned
  version/commit; copy no GPL code.
- RETURN: `Plans/RETURN-2026-11-08-quant-drift-dexter.md` on your branch, naming the head SHA.
- Review: Jarvis (Codex), after the PR is up.
- Disk reclaim: after the merge, `cargo clean` in your worktree and delete the downloaded weights.
