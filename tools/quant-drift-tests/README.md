<!-- SPDX-License-Identifier: MPL-2.0 -->
# quant-drift-tests — Q4_K_M vs full precision

Host-only quantization-drift check for Arthur's shipped model. It measures the
one accuracy question the per-op KATs cannot: **accumulated drift across full
forward passes**, and whether that drift moves the tokens Arthur actually
chooses.

The acceptance bars are fixed in
`Plans/DISPATCH-2026-11-08-relay-323-quant-drift-dexter.md`, before any run. A
run reports **pass or fail** against them, never a number with no decision.

## The three models, one tokenizer

| # | model | how |
|---|-------|-----|
| 1 | **ours**: Arthur's shipped `model-qwen2.5-0.5b.neom` (Q4_K_M, BLAKE3 `a32e4a1b…`) | the kernel's own `forward_logic` files, path-`mod`ed by this crate (same mount as `tools/ai-ops-tests`) |
| 2 | **llama.cpp Q4_K_M** | the upstream `qwen2.5-0.5b-instruct-q4_k_m.gguf` (SHA-256 `74a4da8c…`, revision `9217f5d…`), run by the pinned llama.cpp |
| 3 | **full precision** | `Qwen/Qwen2.5-0.5B-Instruct` safetensors (revision `7ae5576…`) in **float32** through transformers (`ref/fp32_ref.py`) |

All three receive **identical token IDs**, produced once by the shipped
`.vocab` sidecar through Arthur's `QwenTemplate` (`<|im_start|>` and
`<|im_end|>` emitted as atomic IDs). llama.cpp never tokenizes here, so the
"one tokenizer" claim is true by construction, not by hoping two BPE
implementations agree.

## Eval set

`src/prompts.rs` lists **30 prompts**, all exact typed text from where Arthur is
actually used:

- the 21-token boot prompt (`hello world`, `arthur_real_brain_boot`);
- the desktop scenarios in `tools/xtask/src/main.rs` (`hello desktop`,
  `again`, `open probe app`, `list /`, `help`, the clock questions, `hi`);
- the Arthur console/agentic scenarios (the `read`/`write`/`posture`/`multi`
  commands);
- J.c.'s preview questions (`what's 2+2?`, `what time is it`).

Each is framed as one user turn with Arthur's chat template (system:
`You are a helpful assistant.`). Greedy decode, **32 new tokens** per prompt.

The perplexity corpus is the **first 8,192 tokens** of WikiText-2 (raw) `test`,
pinned by source revision and hashes. The text is downloaded, never committed
(CC-BY-SA). NLL is summed over positions 4,096..8,190 predicting the next
token (4,095 targets) — the same last-half window `llama-perplexity` evaluates
at `-c 8192`.

## Bars

**A. Implementation fidelity — ours vs llama.cpp on the same Q4_K_M file**
- greedy tokens identical on 100% of prompts, all 32 tokens;
- perplexity within 0.5% relative.
- A miss is a **bug in our forward pass**.

**B. Quantization loss — Q4_K_M vs full precision**
- ΔPPL ≤ 10% relative;
- top-1 next-token agreement ≥ 90% over all prompt positions (teacher-forced:
  every model sees the same prompt IDs, so the comparison is well defined);
- the boot prompt's greedy reply identical to full precision's.
- A miss is a **decision, not a bug**: open a THINK on Q5_K_M/Q6_K/Q8_0 for J.c.

## Run it

The test is hermetic: without `QUANT_DRIFT_NEOM` it skips, so CI stays green
with no model. To reproduce the KAT:

```sh
export QUANT_DRIFT_NEOM=/path/model-qwen2.5-0.5b.neom       # + .vocab beside it
export QUANT_DRIFT_GGUF_Q4=/path/qwen2.5-0.5b-instruct-q4_k_m.gguf
export QUANT_DRIFT_GGUF_F32=/path/qwen2.5-0.5b-f32.gguf
export QUANT_DRIFT_CORPUS_TEXT=/path/wikitext-2-raw-test.txt
export QUANT_DRIFT_LLAMA_DIR=/path/llama.cpp               # built
tools/quant-drift-tests/ref/run-reference.sh
```

Then the on-demand regression test:

```sh
QUANT_DRIFT_NEOM=... QUANT_DRIFT_CORPUS=.../corpus.u32 \
  cargo test --release -p quant-drift-tests --test quant_drift -- --nocapture
```

It re-runs the prompts and compares them with the committed
`fixtures/quant-drift-kat-2026-11-08.ours.tsv`; a forward-pass change that
moves a token fails there instead of being re-blessed by review. The JSON KAT
is the full pinned artifact: input hashes, every model's token IDs and
perplexity, the per-prompt divergences, and the bar verdicts.

## Prior art / licences

- **llama.cpp** — MIT. Pinned commit recorded in the KAT. `ref/llama_drift.cpp`
  uses only the public `llama.h` API; it walks logits and greedy decode itself.
  No llama.cpp source is copied.
- **transformers** (Apache-2.0) and **PyTorch** (BSD-3-Clause) — the float32
  full-precision runner. `ref/fp32_ref.py` calls the public `AutoModelForCausalLM`
  API; no library source is copied.
- **llama.cpp `convert_hf_to_gguf.py`** — MIT, same checkout; used verbatim to
  build the F32 GGUF from the pinned safetensors.
- **Qwen2.5-0.5B-Instruct / GGUF** — Apache-2.0 (the model card's licence).
- **Tokenizer note.** Arthur's shipped `.vocab` and the GGUF's own tokenizer
  agree on the boot prompt and the chat controls, but segment some prose
  differently (e.g. `" ="` is `[220, 28]` vs the GGUF's `[284]`). Every model in
  this KAT therefore consumes the SAME IDs produced by Arthur's `.vocab`: that
  is the production path and keeps the three-way comparison exact.
- **WikiText-2** — CC-BY-SA-3.0; only its source URL, revision, slice offsets
  and hashes are committed, never the text.
