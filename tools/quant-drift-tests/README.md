<!-- SPDX-License-Identifier: MPL-2.0 -->
# quant-drift-tests — Q4_K_M vs full precision

> **Relay 333 correction (2026-11-08).** The upstream
> `Qwen/Qwen2.5-0.5B-Instruct-GGUF` Q4_K_M/Q8_0 files are **not** the same
> weights as `Qwen/Qwen2.5-0.5B-Instruct@7ae5576`: their token embeddings
> match exactly, but `blk.0.attn_q.weight` differs by ~44% relative and
> `blk.0.ffn_down.weight` by ~32%. The reliable same-checkpoint references are
> the upstream **F16** GGUF (for the shipped Q4_K_M) and the pinned float32
> safetensors (for Control C). The old cross-model bar-B numbers (65.3% top-1,
> Q4 PPL below float32) are therefore **not** quantization loss. The corrected
> artifact is `fixtures/quant-drift-kat-2026-11-08-control.json`; the original
> KAT stays in the history.

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
(CC-BY-SA-3.0 and GFDL; see §Prior art / licences).

**PPL window.** NLL is summed on the last **2,048-token window** of that slice
(tokens 6,144..8,191, BLAKE3 `1782216d…`), evaluating positions 1,024..2,046
with 1,024 tokens of context. The 8,192-token slice is still the pinned parent
(the KAT records both), but the full 8,192-token sequential pass is ~0.5 s/token
in the scalar harness (≈70 min) and did not finish in this relay; the window is
the honest measurement that did. All three models consume the same window, so
the ΔPPL comparison is exact: `ref/run-control.sh` gives every runner the full
pinned corpus plus `--corpus-offset 6144`, and `src/report.rs` refuses a
comparison whose runs record different offsets, token counts or eval starts.

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
- **WikiText-2** — the pinned Salesforce/wikitext repository
  (`b08601e04326c79dfdd32d625aee71d232d685c3`) declares **both `cc-by-sa-3.0`
  and `gfdl`** in its dataset-card metadata. One prose paragraph in the same
  card says "CC BY-SA 4.0"; we record both metadata licences rather than
  treating the version-specific wording as definitive. Only its source URL,
  revision, slice offsets and hashes are committed, never the text.

## Relay 333 — Control C and the model-identity check

Control C is **llama.cpp F16** built from the pinned float32 GGUF, against the
pinned float32 transformers run, on the same IDs and the 2,048-token window.
F16 is the lossless reference path; Q8_0 is itself a lossy quantization and is
reported separately as a quantization probe. The bar is fixed before the run:
teacher-forced top-1 >= 98% and relative PPL delta <= 1%.

| candidate | top-1 vs float32 | ΔPPL | verdict |
|---|---:|---:|---|
| llama.cpp F16 (pinned) | 99.71% (695/697) | 0.023% | **PASS** |
| llama.cpp Q8_0 (pinned) | 90.10% (628/697) | 1.44% | FAIL (quantization probe) |

F16 passing proves the float32 and llama.cpp reference paths agree; the Q8_0
miss is the quantization error of a 0.5B model, not a reference bug. A Q8_0
variant with the tied token embedding kept at F16 moves top-1 to 95.12% while
leaving PPL at 30.313 (vs F16 30.753), which separates the embedding's top-1
effect from the transformer weights' small PPL shift.

`ref/run-control.sh` reproduces the whole corrected run, including the
same-checkpoint F16 reference for bar B.

## Relay 342 — the dequantized-f32 control and the quant ladder

**One dequantization, read by both.** `llama-quantize --allow-requantize
qwen2.5-0.5b-instruct-q4_k_m.gguf dequant.gguf F32` writes the f32 GGUF once.
The kernel reads that GGUF directly with `--gguf-f32` (the `.neom` container
caps at 2 GiB, below this model's 2.5 GiB f32 payload) while llama.cpp reads the
same file; there is no second dequantizer.

```sh
# kernel: full 151,936-wide logit rows at named (prompt, index) positions
quant-drift-tests logits --neom M.neom --gguf-f32 dequant.gguf \
    --prompts prompts.tsv --positions positions.tsv --out kernel.qdlg

# kernel: residual-stream hidden state after every layer
quant-drift-tests hidden --neom M.neom --gguf-f32 dequant.gguf \
    --prompts prompts.tsv --positions positions.tsv --out kernel.qdlh

# llama.cpp: the same two dumps, plus its per-layer inputs (needs the
# `llama-ext.h` layer-input API and the one-line qwen2.cpp diagnostic patch)
llama_drift dequant.gguf prompts.tsv corpus.u32 out.tsv LABEL SRC SHA TOOL \
    --dump-positions positions.tsv --dump-logits llama.qdlg \
    --dump-layer-inputs llama.qdlh
```

`QDLG` is `magic,version,n_vocab,rows` then per row `name,position,f32×n_vocab`;
`QDLH` is the same with `hidden_size,layers` and every layer's f32 row. Both are
little-endian and are compared by a plain array subtraction.

The 37 clear-gap rows are the `ours_vs_llama_q4` divergences with a > 0.5 gap
in either runtime; 30 of them are the same shared-prefix position 7, so the
control measures the 8 unique positions and replicates the shared-prefix row.

Result: the kernel stays within 0.054 of an independent float64 reference at
all 37 rows (top-1 identical at all 37); llama.cpp's f32 run drifts up to 0.299.
The first layer where the two runtimes part is layer 0, in the K/V cache: layer
0's input is bit-identical, and a numpy layer-0 computed with **f16 K/V**
matches llama.cpp to 3.4e-4 while the f32-K/V computation matches the kernel to
6e-11. The kernel is the more accurate side, not the outlier. The committed
artifacts are `fixtures/quant-drift-kat-2026-11-08-f32-control.json` (control,
per-layer localization, and the byte-identity check) and
`fixtures/quant-drift-kat-2026-11-08-quant-ladder.json`.

