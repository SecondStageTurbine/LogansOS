<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Arthur v1 — model selection + intent/tool-call strategy

**Date:** 2026-06-17
**Researcher:** Prometheus
**Branch:** `prometheus/arthur-model-and-intent-research`
**Sources:** HuggingFace model cards (fetched 2026-06-17), Qwen2.5 blog, SmolLM2 model card, Arthur design THINK (`Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md`)
**Scope:** Research FINDINGS only. No production code.

---

## 1. Model-selection survey

### 1A. Candidate models — on-device GGUF instruct models

All facts sourced from HuggingFace model cards and official GGUF repos. File sizes are from the canonical GGUF quantization tables on each repo.

| Model | Params | Q4_K_M size | License | Context | Native tool-calling |
|-------|--------|-------------|---------|---------|---------------------|
| **Qwen2.5-0.5B-Instruct** | 0.49B (0.36B non-emb) | **491 MB** | Apache 2.0 | 32,768 | Yes — Hermes format (`` / ``) |
| **Qwen2.5-1.5B-Instruct** | 1.54B (1.31B non-emb) | **1.12 GB** | Apache 2.0 | 32,768 | Yes — Hermes format |
| **Qwen2.5-3B-Instruct** | 3.09B (2.77B non-emb) | **2.1 GB** | Qwen License (attribution + use-policy) | 32,768 | Yes — Hermes format |
| **Llama-3.2-1B-Instruct** | 1.23B | **~808 MB** | Llama 3.2 Community | 131,072 | Yes — `<|python_tag|>` / ipython header |
| **Llama-3.2-3B-Instruct** | 3.21B | **~2.02 GB** | Llama 3.2 Community | 131,072 | Yes — same as 1B |
| **Phi-3.5-mini-instruct** | 3.8B | **~2.39 GB** | MIT | 131,072 | **No** — no tool-calling tokens |
| **SmolLM2-1.7B-Instruct** | 1.7B | **~1.06 GB** | Apache 2.0 | 32,768 | Yes — Hermes format (`` / ``) |

**Sources:**
- Qwen2.5: `huggingface.co/Qwen/Qwen2.5-{0.5,1.5,3}B-Instruct-GGUF` (official GGUF repos, Apache 2.0 for 0.5B/1.5B)
- Llama-3.2: `huggingface.co/meta-llama/Llama-3.2-{1,3}B-Instruct`; GGUF sizes from `bartowski` quantizations on HF
- Phi-3.5: `huggingface.co/microsoft/Phi-3.5-mini-instruct`; GGUF from `bartowski/Phi-3.5-mini-instruct-GGUF`
- SmolLM2: `huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct` (model card confirms function calling, BFCL 27%)

### 1B. License / sovereignty check (Article 23)

| Model | Locally redistributable? | Sovereignty risk |
|-------|------------------------|------------------|
| Qwen2.5-0.5B/1.5B (Apache 2.0) | Yes — pure permissive | None |
| Qwen2.5-3B (Qwen License) | Yes — with attribution + use-policy compliance | Low — not Apache, but permits on-device use |
| Llama-3.2 (Community License) | Yes — with "Built with Llama" branding, license copy, model-name prefix rules | Medium — Meta's community license is permissive but carries naming obligations and a 700M MAU cap (irrelevant for LogansOS) |
| Phi-3.5 (MIT) | Yes — pure permissive | None |
| SmolLM2 (Apache 2.0) | Yes — pure permissive | None |

**Conclusion:** All candidates satisfy Article 23's sovereignty constraint (locally-runnable, no cloud/telemetry dependency). Apache 2.0 models (Qwen2.5-0.5B/1.5B, SmolLM2) are the cleanest from a redistribution standpoint.

### 1C. Capability / size trade-off

The load-bearing capability for Arthur is **structured output reliability** — specifically, emitting parseable tool calls consistently. Benchmark data from the model cards:

| Model | IFEval (instruction following) | MT-Bench | Notes |
|-------|-------------------------------|----------|-------|
| Qwen2.5-0.5B-Instruct | Not published for 0.5B | Not published | Too small — no published benchmarks at this size |
| Qwen2.5-1.5B-Instruct | Not separately published | Not separately published | Blog claims "significant improvements in instruction following" vs Qwen2 |
| SmolLM2-1.7B-Instruct | **56.7** | **6.13** | Explicitly benchmarked; outperforms Llama-1B on all metrics |
| Llama-3.2-1B-Instruct | 53.5 (IFEval) | 5.48 (MT-Bench) | Published in Llama-3.2 card |

SmolLM2-1.7B leads the sub-2B class on instruction following (IFEval 56.7 vs Llama-3.2-1B's 53.5). Qwen2.5-1.5B lacks published benchmarks at this size but benefits from the Qwen2.5 family's explicit training on structured output / JSON generation.

### 1D. Resident memory footprint (in-kernel)

The Q4_K_M file size is the on-disk footprint. The **resident** footprint during inference is larger due to:
- Weight tensors in arena: Q4_K bytes (the GGUF data)
- KV cache: `2 × layers × kv_heads × head_dim × context × sizeof(f32)` — this varies by model
- Activation buffers during forward pass

For Arthur's use case (autoregressive decode, one token at a time), the KV cache dominates at long contexts. For the 0.5B model at 4096 context: `2 × 24 × 2 × 64 × 4096 × 4 = 24.1 MB` for the KV cache alone. For the 1.5B at 4096: `2 × 28 × 2 × 64 × 4096 × 4 = 29.0 MB`.

**Total resident estimates (weights + KV cache at 4096 context):**
- Qwen2.5-0.5B: ~491 MB weights + ~24 MB KV = **~515 MB**
- Qwen2.5-1.5B: ~1.12 GB weights + ~29 MB KV = **~1.15 GB**
- SmolLM2-1.7B: ~1.06 GB weights + ~30 MB KV = **~1.09 GB**

### 1E. Speed mapping (Phase 8 f32 vs Phase 9 quantized)

The Phase 8 forward path dequantizes Q4_K to f32 before matmul. For Qwen2.5-0.5B (our proven model), this means reading ~3.2 GB of f32 weights per forward pass (24 layers × ~134 MB f32/layer). Phase 9's quantized matmul reduces weight traffic to ~451 MB — a 7× reduction.

**Expected decode speed (one token, rough):**
- Qwen2.5-0.5B, Phase 8 f32 path: ~2-5 tokens/sec on Ryzen AVX2 (memory-bound by f32 weight reads)
- Qwen2.5-0.5B, Phase 9 quantized: ~10-15 tokens/sec (7× less memory traffic, integer SIMD throughput)
- Qwen2.5-1.5B, Phase 9 quantized: ~5-8 tokens/sec (2.3× more params than 0.5B)
- SmolLM2-1.7B, Phase 9 quantized: ~4-7 tokens/sec

These are estimates pending the Phase 9 latency oracle. The key insight: **Phase 9 makes the 1.5B-1.7B range usable on CPU** at interactive speeds (5+ tok/s). Before Phase 9, only the 0.5B is tolerable.

---

## 2. Intent emission — the crux

### 2A. The problem

OD-3=A ratified "delimited tool-call tokens" as the intent format. But we **run off-the-shelf models without fine-tuning** (Article 23.3). So the question is: how does an unmodified instruct model emit our action intents in a parseable format?

### 2B. Four approaches, weighed

#### Approach 1: Native chat-template tool-calling (RECOMMENDED)

**How it works:** Many modern instruct models ship with a built-in tool-calling chat template. You pass the available tools as part of the system prompt via `apply_chat_template(tools=[...])`, and the model emits tool calls in its native delimited format.

**Qwen2.5's format (Hermes-inspired):**
```
<tool_code>
{"name": "read_file", "arguments": {"path": "/neofs/config.neom"}}
</tool_call>
```

The delimiters are `` (open) and `` (close). The content between them is a JSON array or object with `name` and `arguments` fields.

**SmolLM2's format (identical):**
```
<tool_code>
[{"name": "get_random_number_between", "arguments": {"min": 1, "max": 300}}]
</tool_call>
```

Same delimiters, same JSON structure. The SmolLM2 model card explicitly demonstrates this pattern with a working parser.

**Advantages:**
- **No tokenizer modification needed.** The delimiters are ordinary byte sequences that the model's existing vocabulary can tokenize. No special tokens to register.
- **The model is already trained to emit this format.** Qwen2.5 was post-trained with tool-calling data; SmolLM2 was SFT'd on Synth-APIGen with explicit function calling examples.
- **Simple parser.** Scan output for `` / `` delimiters, parse JSON between them. Regex + serde_json.
- **Multiple tool calls in one response.** The format supports arrays of tool calls.

**Disadvantages:**
- **JSON is not guaranteed valid.** Small models sometimes emit malformed JSON. Mitigation: validate the parsed JSON against our action schema; on failure, re-prompt or fall back to text-only response.
- **Requires chat template support.** Our 8.2 tokenizer needs to implement `apply_chat_template` with tool definitions. This is a formatting layer (string templating), not a vocabulary change.

#### Approach 2: Tokenizer special tokens

**How it works:** Register custom tokens like `<action>`, `<arg>`, `</action>` in the BPE tokenizer vocabulary. The model would need to learn these tokens through fine-tuning.

**Problem:** Our 8.2 tokenizer (`tokenizer_logic.rs`) loads vocab from GGUF. Adding special tokens means either:
- Modifying the GGUF tokenizer data (fragile, model-specific)
- Post-processing: intercepting token IDs that match our special sequences (works but is a hack)
- Fine-tuning the model to use our custom tokens (violates "no fine-tuning" constraint)

**Verdict:** Not viable without fine-tuning. The model hasn't seen these tokens during training and won't emit them reliably.

#### Approach 3: Grammar-constrained decoding

**How it works:** At sample time, constrain the tokenizer to only produce tokens that match a CFG/regex grammar defining valid action syntax. This forces the model to emit syntactically valid tool calls.

**Advantages:**
- **Guaranteed valid output.** By construction, every emitted token sequence is a valid action.
- **Fits our in-kernel sampler.** The 8.1 `generate_logic.rs` sampler currently does top-k/top-p/greedy over the full vocab. Grammar-constrained decoding masks logits for tokens that would violate the grammar — a natural extension of the sampling loop.

**Disadvantages:**
- **Significant implementation complexity.** Requires a grammar engine (e.g., GBNF like llama.cpp's `grammar` module) integrated with our BPE tokenizer. The grammar must be evaluated token-by-token during autoregressive generation, checking which next-tokens are valid continuations.
- **Performance cost.** Masking logits per-token based on grammar state is O(grammar_states × vocab_size) per step — expensive for a 150K+ vocab.
- **Model may resist.** Constraining a model that wasn't trained for our specific grammar can produce incoherent output or cause the model to repeat/refuse.

**Verdict:** Powerful but overkill for v1. Better as a Phase 11 quality improvement after the native template approach proves workable. The sampler extension needed is substantial (grammar engine + BPE-compatible token masking).

#### Approach 4: Prompt convention + few-shot

**How it works:** Hard-code the desired output format in the system prompt with examples. No template support needed.

```
You are Arthur, an assistant on LogansOS. When you need to perform an action,
respond with: ACTION:read_file PATH:/neofs/config.neom
Available actions: read_file, write_file, list_dir, spawn_process, ...
```

**Advantages:** Zero infrastructure. Works with any model.
**Disadvantages:** Small models are unreliable at following arbitrary format conventions they weren't trained on. The 0.5B will ignore the format half the time. No delimiter structure for robust parsing.

**Verdict:** Fallback only. Not reliable enough for production intent parsing.

### 2C. Recommendation: Native chat-template tool-calling (Approach 1)

**Primary:** Use the model's native tool-calling chat template. For Qwen2.5 and SmolLM2, this is the Hermes format with `` / `` delimiters. The system prompt includes tool definitions; the model emits JSON between the delimiters.

**Reconciliation with OD-3=A:** "Delimited tool-call tokens" maps directly to `` / `` — these ARE delimited tokens. The delimiter is a byte sequence in the model's existing vocabulary, not a special token we register. The intent parser scans for these delimiters in the output stream.

**Fallback (interim):** If the 8.2 tokenizer doesn't support chat templates with tool definitions at Slice 4 time, use prompt convention (Approach 4) with `` / `` delimiters hardcoded in the system prompt. This is less reliable but requires zero tokenizer changes. Upgrade to proper template support when the tokenizer is ready.

### 2D. What the 8.1 sampler / 8.2 tokenizer need

**8.2 tokenizer (BPE):** Needs a **chat template renderer** — not a vocabulary change. The GGUF file stores a Jinja2 chat template string. Our tokenizer needs to:
1. Parse the GGUF `tokenizer.chat_template` field
2. Implement minimal Jinja2 template rendering (the Qwen2.5 template uses `for`, `if`, `{{ }}`)
3. Accept tool definitions as a template variable and render them into the system message

This is **string templating**, not tokenizer logic. The BPE encoding itself is unchanged. The template renderer produces a prompt string; the existing `encode()` method tokenizes it.

**Minimal Jinja2 subset needed:**
- Variable interpolation: `{{ var }}`
- Loops: `{% for item in list %}...{% endfor %}`
- Conditionals: `{% if condition %}...{% endif %}`
- String methods: `.strip()`, `.replace()`

This is ~200 LOC of template parsing, not crypto-quality code. It's formatting logic.

**8.1 sampler (generate_logic.rs):** No changes needed for v1. The native template approach doesn't constrain decoding — it just formats the prompt. The sampler generates freely; the intent parser post-processes the output. Grammar-constrained decoding (Approach 3) would require sampler changes but is deferred.

---

## 3. Action-intent schema (Slice-2 mapping)

### 3A. The 12 v1 actions from the Arthur design

From `THINK-2026-06-16-arthur-userland-arc-and-architecture.md` §3C:

| # | Action | Args | Syscall(s) | Expressible as tool call? |
|---|--------|------|------------|--------------------------|
| 1 | `read_file` | `path: string` | SYS_OPEN + SYS_READ + SYS_CLOSE | Yes |
| 2 | `write_file` | `path: string, content: string` | SYS_OPEN + SYS_WRITE + SYS_CLOSE | Yes |
| 3 | `list_dir` | `path: string` | SYS_READDIR | Yes |
| 4 | `spawn_process` | `name: string` | SYS_SPAWN | Yes |
| 5 | `kill_process` | `handle: integer` | SYS_KILL | Yes |
| 6 | `inspect_process` | `handle: integer` | SYS_INSPECT | Yes |
| 7 | `wait_process` | `handle: integer` | SYS_WAIT | Yes |
| 8 | `net_dial` | `host: string, port: integer` | SYS_NET_DIAL | Yes |
| 9 | `net_send` | `handle: integer, data: string` | SYS_SOCKET_SEND | Yes |
| 10 | `net_recv` | `handle: integer, max_bytes: integer` | SYS_SOCKET_RECV | Yes |
| 11 | `infer` | `model_handle: integer, input: string` | SYS_INFER_RUN | Yes |
| 12 | `load_model` | `path: string` | SYS_MODEL_LOAD | Yes |

**All 12 actions are expressible as JSON tool calls.** The argument types (string, integer) map directly to JSON values. No action requires a complex nested structure that would stress a small model's JSON generation.

### 3B. Tool definition for the chat template

The system prompt passes tool definitions in the Hermes format:

```json
[
  {
    "name": "read_file",
    "description": "Read the contents of a file at the given path.",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string", "description": "Absolute path to the file" }
      },
      "required": ["path"]
    }
  },
  {
    "name": "write_file",
    "description": "Write content to a file. Creates or overwrites.",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string" },
        "content": { "type": "string", "description": "Content to write" }
      },
      "required": ["path", "content"]
    }
  }
  // ... (10 more tools)
]
```

The model emits:
```
<tool_code>
{"name": "read_file", "arguments": {"path": "/neofs/config.neom"}}
</tool_call>
```

### 3C. Intent parser → execute_action() round-trip

```
Model output: "...I'll read that file for you.<tool_code>\n{\"name\":\"read_file\",\"arguments\":{\"path\":\"/neofs/config.neom\"}}\n</tool_call>"

Intent parser:
  1. Split on `` / `` delimiters
  2. Parse JSON between delimiters → {name, arguments}
  3. Validate against action schema (name in known actions, required args present)
  4. Map to execute_action(Intent { action: "read_file", args: {path: "/neofs/config.neom"} })

execute_action() dispatcher:
  match intent.action {
      "read_file" => {
          let path = intent.args["path"].as_str()?;
          let handle = sys_open(path, O_RDONLY, FileRights::READ)?;
          let contents = sys_read(handle, buf)?;
          sys_close(handle)?;
          Ok(contents)
      }
      // ... (11 more arms)
  }
```

**No action is hard to express.** All arguments are flat key-value pairs with primitive types. The most complex is `net_dial` (host + port), which is two scalar args.

---

## 4. Recommendations

### 4A. v1 model: **Qwen2.5-1.5B-Instruct** (Apache 2.0, 1.12 GB Q4_K_M)

**Rationale:**
- **Smallest genuinely useful model.** The 0.5B is too small for reliable tool-calling — it lacks published benchmarks and the instruction-following capability needed for structured output. The 1.5B is the smallest in the Qwen2.5 family with demonstrated capability.
- **Proven inference substrate.** We already run Qwen2.5-0.5B on this kernel (P8, ADR-055). The 1.5B uses the same architecture (Qwen2, GQA, RoPE, SwiGLU) — no new op types, no new quantization formats. Same GGUF importer, same forward logic.
- **Apache 2.0 license.** Cleanest sovereignty posture. No attribution obligations beyond the license file.
- **Hermes tool-calling format.** Native `` / `` delimiters with JSON. Matches SmolLM2's format — if we swap models later, the intent parser doesn't change.
- **Phase 9 makes it fast.** At ~5-8 tok/s with quantized matmul on Ryzen, this is interactive for a terminal assistant.

**Bring-up interim:** Use Qwen2.5-0.5B-Instruct (491 MB) for Slice 3 bring-up if the 1.5B doesn't fit in available memory during development. Swap to 1.5B for Slice 4 (intent parsing) where structured output reliability matters.

### 4B. Post-Phase-9 upgrade target: **Qwen2.5-3B-Instruct** (2.1 GB Q4_K_M)

Once Phase 9's quantized matmul delivers the ~7× speedup, the 3B becomes viable on CPU at ~3-5 tok/s. It has significantly better instruction following and JSON generation than the 1.5B. The Qwen License is acceptable (attribution + use-policy) but not as clean as Apache 2.0.

### 4C. Intent format: **Hermes tool-calling** (`` / `` delimiters + JSON)

- Primary: native chat template with tool definitions (requires 8.2 tokenizer chat template renderer)
- Interim fallback: hardcoded `` / `` delimiters in system prompt (no tokenizer changes)
- Deferred: grammar-constrained decoding for guaranteed-valid output (Phase 11+)

---

## 5. Open decisions for PM/PL ratification

| # | Decision | Options | Recommendation | Rationale |
|---|----------|---------|----------------|-----------|
| D1 | **v1 model size:** 0.5B (bring-up) vs 1.5B (useful) | 0.5B: fits now, unreliable tool-calling / 1.5B: needs Phase 9 for speed, reliable | **1.5B for Slice 4; 0.5B for Slice 3 bring-up** | Slice 3 (conversation relay) works with any model. Slice 4 (intent parsing) needs structured output reliability that the 0.5B can't provide. |
| D2 | **Chat template renderer: who implements it?** Kernel tokenizer (8.2) vs userland Arthur | Kernel: shared by all models / Userland: Arthur owns its prompt formatting | **Userland (Arthur process)** | The chat template is a formatting concern, not a tokenizer concern. Arthur builds the prompt string and calls `encode()`. Keeps the kernel tokenizer minimal. |
| D3 | **JSON validation strictness:** schema-validate vs lenient parse | Strict: reject any malformed JSON / Lenient: best-effort parse, retry on failure | **Lenient with retry** | Small models produce imperfect JSON. Validate required fields; if missing, re-prompt the model with the error. Three retries then give up and report to user. |
| D4 | **Tool definition scope:** all 12 actions always vs dynamic based on granted caps | All 12: simpler prompt / Dynamic: only offer tools Arthur has caps for | **Dynamic** | If Arthur doesn't have a write cap, don't offer `write_file` as a tool. The model won't propose actions it can't take. Reduces hallucination and makes the "model proposes, capability disposes" pattern tighter. |
| D5 | **Multi-turn tool calls:** one tool call per turn vs batch | One per turn: simpler / Batch: model can plan multiple actions | **One per turn for v1** | Small models struggle with multi-step planning. One action, execute, report result, then the model decides the next step. Batch tool calling is a Phase 11+ feature. |
| D6 | **SmolLM2 vs Qwen2.5-1.5B:** capability comparison | SmolLM2: better IFEval (56.7), Apache 2.0 / Qwen2.5-1.5B: same tool format, proven substrate | **Qwen2.5-1.5B** | Same Hermes format as SmolLM2, but we already have the Qwen2 forward pass proven (P8). SmolLM2 uses Llama architecture — different attention pattern, different RoPE implementation. Zero risk to switch later since the intent parser is format-agnostic. |

---

## 6. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Qwen2.5-1.5B tool-calling is unreliable at Q4_K quantization | High — Slice 4 blocked by unparseable output | Lenient JSON parser with retry (D3). Fall back to 0.5B + prompt-convention interim if needed. Test tool-calling quality on the actual GGUF Q4_K_M file before committing to Slice 4. |
| Chat template renderer is more complex than expected | Medium — delays Slice 4 | Interim fallback: hardcoded system prompt with `` / `` delimiters. No template renderer needed for the interim. |
| 1.5B model doesn't fit in available arena memory | Medium — can't load the model | Phase 9 reduces weight traffic; the weights still need to be resident. If arena is too small, use 0.5B until arena grows. |
| Model emits tool calls for actions it lacks caps for | Low — denial path handles this | Dynamic tool definitions (D4): only offer tools Arthur has capabilities for. |
| Hermes delimiters collide with model's natural text output | Low — `` is unlikely in normal text | The delimiter is a specific byte sequence. If the model emits it naturally, the JSON parser will fail and the intent parser treats it as text. |

---

## 6. PM/PL ratification (2026-06-17)

**PL ratified D1–D6 as recommended** (no refinements — the FINDINGS landed clean). This decides **Arthur's brain and intent format**:

| OD | Ratified |
|----|----------|
| **D1** model size | **Qwen2.5-1.5B-Instruct for Slice 4; Qwen2.5-0.5B for Slice-3 bring-up** |
| **D2** chat-template renderer | **Userland (Arthur process)** — kernel stays model-agnostic (Article 23.3) |
| **D3** JSON validation | **Lenient + retry** (validate required fields; re-prompt on failure; 3× then report) |
| **D4** tool-def scope | **Dynamic** — only offer tools Arthur holds the capability for (the capability set shapes the tool menu) |
| **D5** multi-turn | **One tool call per turn (v1)** |
| **D6** model family | **Qwen2.5-1.5B-Instruct** (same arch as the proven 8.5 forward pass → zero new ops; intent parser is format-agnostic, so reversible) |

**Decided, on the record: Arthur v1's brain is Qwen2.5-1.5B-Instruct** (Apache-2.0, locally-redistributable — sovereignty-clean; 1.12 GB Q4_K_M; runs on the exact substrate proven at P8 with zero new ops; interactive once Phase-9 quantized matmul lands; upgradeable to 3B post-Phase-9). The kernel remains model-agnostic — this is Arthur's *default* brain, swappable per the VISION's "the mind is yours to choose."

**Unblocks Slice 4** (intent → capability-bounded action): once Slice 3 lands, Slice 4 is dispatchable against a fully-decided target — Qwen2.5-1.5B-Instruct, Hermes tool-calling, userland chat-template renderer, dynamic cap-scoped tool defs, lenient-JSON+retry, one-action-per-turn. The interim fallback (a hardcoded system prompt with the Hermes delimiters, no template renderer) de-risks the renderer.
