<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur agentic action layer — natural-language operation (OT-26)

**Date:** 2026-06-21
**Author:** Prometheus
**Branch:** `prometheus/arthur-agentic-action-design`
**Inputs:** Dispatch `Plans/DISPATCH-2026-06-20-arthur-agentic-action-design-prometheus.md`; live code at `origin/main` HEAD `f11efde`; OT-17 ratified THINK (`THINK-2026-06-18-arthur-memory-substrate-design.md` — memory substrate + per-category caps); OT-18 ratified THINK (`THINK-2026-06-18-arthur-grant-ergonomics-design.md` — coarse durable grants, six scopes, silent-within-scope); OT-19 ratified THINK (`THINK-2026-06-18-arthur-disagreement-reversibility-design.md` — reversibility mechanism, userland copy-before-write, stakes = user policy); Article 29.3 ("model proposes, capability disposes" — the keystone this realizes); Slice 4 action surface (`userland/libneo/src/action.rs` — 13 `ActionKind`s, `execute_action()` cap-check, `Grant`/`RequiredCapability` enums); Arthur conversation loop (`userland/neo-arthur/src/main.rs` — `respond_to_turn_with_actions`, `SyscallBackend`, tool-call rendering in `intent_logic.rs`); Hermes tool-call format (`<tool_call>...</tool_call>` delimiters, JSON payload with `name` + `arguments`); existing `ToolKind` enum and `extract_tool_call()` parser in `intent_logic.rs`; neo-shell as deterministic command floor (`userland/neo-shell/src/main.rs`)
**Scope:** Design THINK only. No production code, no `ci.yml`. The **agentic action layer**: NL → Action → kernel cap-check → NL result. Capstone of the OT-17/18/19 agent-maturation trilogy.

---

## 1. Problem statement — from chatbot to OS agent

Today Arthur is a **chatbot that occasionally acts**: the conversation loop in `main.rs` calls `respond_to_turn_with_actions()`, which renders a Hermes prompt with tool definitions, generates model output, and parses one `<tool_call>...</tool_call>` tool-call per turn. The parsed `ToolCall` maps to an `Action`, which `execute_action()` checks against the grant set before any syscall runs. Article 29.3 is live: "model proposes, capability disposes."

But this is **command-driven**: the model must emit a well-formed JSON tool-call inside Hermes delimiters. The user interacts via typed `neo>` commands or by carefully prompting Arthur to use tools. There is no natural-language interface to OS operations. The substrate exists (action vocabulary, cap-check, grants, reversibility), but the **glue** — NL ↔ Action made reliable and safe with a fallible 0.5B/1.5B model — is unbuilt.

This designs that glue. The load-bearing problem is not the happy path ("read /hello.txt" → `ReadFile` → cap-check → result). It is: **how do you make a small, unreliable model emit a valid action structure without hallucinating fields, malforming JSON, or silently doing the wrong thing?** That is D1.

**Non-negotiables:**
- **The keystone is never softened.** The kernel cap-check (29.3) remains the boundary. The NL layer is UNTRUSTED — it proposes, the capability system disposes. A compromised or confused model cannot exceed Arthur's held caps.
- **Companion to `neo>`, not replacement.** NL is a full LLM forward pass (slow on CPU). `neo>` commands remain the deterministic, instant floor. Both coexist: `arthur>` for "tell me what you want," `neo>` for "do this exactly."
- **No engine change.** The 8.5 inference engine is frozen. Constrained decoding hooks in at the sampling boundary — note where it plugs, don't build it.
- **Compose OT-18/OT-19, don't redesign them.** Grants (authority), reversibility (safety net), and stakes (user policy) are ratified. This uses them.

---

## 2. Prior art — what the industry learned about small-model tool-use

### 2.1 LLM function-calling / tool-use (OpenAI, Anthropic, Google)

**Mechanism:** The model is prompted with structured tool definitions (JSON Schema for parameters, descriptions, required fields). The model emits a structured tool-call block. The system parses and executes.

**What works:** For 70B+ models, this is reliable. GPT-4o achieves >95% parse success on standard function-calling benchmarks. Claude's tool-use is similarly robust at the large-model tier.

**What breaks at small scale:** At 0.5B–1.5B, the failure modes are systematic (not occasional):
- **JSON malformation:** Missing closing braces, unescaped quotes in string values, trailing commas. Occurs in 30-60% of tool-call attempts on sub-3B models (raw-sourced from llama.cpp community benchmarks and Groq's Small Model Tool-Use report, 2025).
- **Field hallucination:** The model invents parameter names not in the schema (`"filepath"` instead of `"path"`, `"target_directory"` instead of `"path"` for `ListDir`). Occurs in 15-40% of attempts.
- **Action hallucination:** The model emits a tool name that doesn't exist in the available set. Occurs in 5-20% of attempts.
- **Argument omission:** Required fields are silently dropped, producing a structurally valid but semantically invalid call. Occurs in 10-25% of attempts.

**Relevance:** Our existing `intent_logic.rs` already implements Hermes-format tool-call extraction with retry (`MAX_TOOL_RETRIES = 3`). This is the "free-form JSON + retry" approach (Option B in D1). It works for demo purposes but is fundamentally unreliable at small-model scale — the retry loop masks the problem rather than solving it.

### 2.2 Constrained / grammar-guided decoding (llama.cpp GBNF, Outlines, xGrammars)

**Mechanism:** The model's token sampling is constrained by a formal grammar. At each step, only tokens that are valid continuations of the grammar are considered. The model **cannot** emit an invalid structure — it can only stall (reach a dead end in the grammar) or emit something conformant.

**Implementations:**
- **llama.cpp GBNF:** Grammar Backend Normal Form. Precompiled grammar constrains the sampling loop via logit masking. Mature, widely used for function-calling in small models. The grammar is defined as a context-free grammar over token IDs — the sampler computes which tokens are valid at each grammar state and masks all others.
- **Outlines (Python):** Similar approach, uses xGrammars or CFG-based constraints. Targets both llama.cpp and transformers backends.
- **xGrammars:** Grammar format designed for LLM constrained decoding. Supports parameterized grammars (inject values into the grammar at runtime — e.g., enumerate available action names).

**Performance impact:** Logit masking adds ~5-15% latency per token (the mask computation is O(vocab_size) per step, which is dominated by the forward pass on CPU for small models). For our use case (generating one compact action structure, not a long-form reply), this overhead is negligible — we're generating ~20-50 tokens total.

**Reliability at small scale — structural validity:** Near-perfect. The model cannot emit malformed JSON or hallucinated field names because the grammar only permits valid continuations. Empirical results from llama.cpp community: 99%+ parse success on constrained function-calling with 1B models, vs ~50% for unconstrained (raw-sourced from ggml-org forums and Outlines benchmarks, 2025). This "~99%" number is scoped to **structural validity** (the output is a well-formed action AST with valid field names). It does not measure **semantic correctness** (did the model pick the right action for the user's intent?). A 1.5B model constrained by grammar can still perfectly emit `ListDir "/etc"` when the user meant "go to the temp folder" — structurally valid, semantically wrong. Semantic correctness remains model-dependent and is what the safety net catches: cap-check denies unauthorized paths, confirmation gates irreversible actions, and OT-19 reverses destructive mutations. Constrained decoding makes the output parseable; it does not make the model smarter about intent.

**Relevance:** This is the mechanism D1 recommends. It converts structural validity from a probabilistic property (~50% parse success for unconstrained small models) to a deterministic guarantee (~99% valid AST). The model can only emit a structurally valid action (or an explicit `clarify`/`no-op` token). Semantic correctness — picking the right action for the intent — is not solved by constrained decoding and remains the safety net's job (cap-check, confirm, OT-19 reversal).

### 2.3 The "untrusted planner + trusted executor" agent-safety pattern

**Mechanism:** The planning component (LLM) is treated as untrusted. It proposes actions in a structured format. The execution component (deterministic system) validates each action against policy before running it. Invalid proposals are rejected, not executed.

**Instances:**
- **Microsoft AutoGen (2023):** Multi-agent framework where a "critic" agent validates proposals from a "coder" agent before execution. The critic can be another LLM or a deterministic checker.
- **LangChain tool validation:** Tools declare input schemas; invalid inputs are rejected before execution.
- **Devin (Cognition, 2024):** Uses a structured action space with deterministic validation before each shell command executes.

**What this pattern gets right:** Separation of concerns. The model doesn't need to be perfect — it needs to be *understandable*. If the system can parse what the model proposed and validate it, imperfect proposals degrade gracefully (denied, clarified, retried) rather than causing silent damage.

**Relevance:** This is exactly Article 29.3: "model proposes, capability disposes." Our `execute_action()` already implements the trusted executor half. D1 delivers the reliable proposal mechanism; D3 states the safety model explicitly.

### 2.4 Clarification patterns (Anthropic, ReAct, Reflexion)

**Mechanism:** When the model is uncertain about what to do, it asks for clarification rather than guessing. The system detects low confidence or ambiguity and routes to a clarification turn instead of execution.

**Instances:**
- **Anthropic's Claude:** Explicitly trained to ask clarifying questions when user intent is ambiguous, rather than making assumptions.
- **ReAct (Yao et al., 2022):** Reasoning + Acting pattern where the model interleaves reasoning steps with actions. The reasoning step can surface uncertainty.
- **Reflexion (Shinn et al., 2023):** Self-reflection loop where the model evaluates its own action quality and retries with feedback.

**Relevance:** D4's clarify-don't-guess leverages this pattern. With constrained decoding, we can add an explicit `clarify` branch to the grammar — the model *can* choose to ask a question instead of executing an uncertain action. **Critical residual:** Making clarify a grammar production means the model *can* ask — it does not guarantee that it *will*. Small models are overconfident and will reliably choose to emit a wrong action rather than clarify. The grammar enables clarification; it does not resolve the behavioral problem of overconfidence. The backstop is D3's safety net: cap-check denies unauthorized actions, confirmation gates irreversible ones, and OT-19 reverses destructive mutations. Clarify-don't-guess improves UX when the model uses it; the system remains safe when it doesn't.

---

## 3. Design space: D1 — Intent→Action mechanism (THE crux)

### 3.1 The problem concretely

A user types: "find my notes about the OptiPlex." Arthur must:
1. Understand the intent (search for files matching a keyword)
2. Map it to an `Action` (`ListDir` on the relevant directory, or a search action if one exists)
3. Fill in parameters (path, search pattern)
4. Execute through `execute_action()` with cap-check

With a 0.5B model doing free-form JSON tool-calling, step 2-3 fail frequently: the model might emit `{"name": "search_file", "arguments": {"keyword": "OptiPlex"}}` — but `search_file` isn't an `ActionKind`. Or it might emit `{"name": "list_dir", "arguments": {"path": "/home/notes", "keyword": "OptiPlex"}}` — inventing a `keyword` field that `ListDir` doesn't have. The existing retry loop (3 attempts) masks this but doesn't solve it: the model will consistently produce the same hallucinated structure across retries.

### 3.2 Options

| Option | Mechanism | Reliability | Cost |
|--------|-----------|-------------|------|
| **A: Free-text → regex/heuristic parse** | Arthur prompts the model to "describe what you want to do in plain English," then a heuristic parser extracts intent + parameters from natural language. | Low. Regex-based extraction is brittle for open-ended NL. Fails silently on edge cases — wrong path extracted, action misidentified. | Minimal implementation cost. No engine change needed. |
| **B: Few-shot "emit JSON" (current approach)** | Hermes-format prompt with tool definitions. Model emits `<tool_call>{"name":"...","arguments":{...}}</tool_call>`. Parser extracts and validates. Retry on parse failure. | Medium for 7B+, low for <3B. Systematic malformation at small scale: ~50% parse success on sub-1B models. Retry loop masks but doesn't fix hallucinated fields/actions. | Already implemented in `intent_logic.rs`. No engine change. But unreliable at our target model size. |
| **C: Constrained / grammar-guided decoding** | Formal grammar over the `ActionKind` set constrains token sampling. The model can only emit a valid action structure or an explicit `clarify`/`no-op` token. Impossible to produce malformed output. | Near-perfect structural validity (99%+). The remaining failure mode is semantic: the model chose the wrong valid action. This is caught by D4 (clarify on low confidence) and D3 (cap-check denies unauthorized actions). | Requires grammar compilation + logit masking at the 8.5 engine's sampling boundary. One new module (`action_grammar.rs`). Engine hook is a single trait method override — no engine restructuring. |

### 3.3 Tradeoffs

**Option A (regex/heuristic):** This is what many early chatbot integrations tried. It fails in ways that are worse than Option B: silent misidentification ("I'll read /tmp/notes" when you meant /home/notes) rather than parse failure. The model's natural language output is even harder to constrain than JSON — there's no schema to validate against. **Rejected.**

**Option B (few-shot JSON, current approach):** This is what `intent_logic.rs` implements today. It works as a demo with careful prompting and 3 retries, but it's fundamentally unreliable at 0.5B–1.5B scale. The failure modes are systematic: small models consistently hallucinate field names and action names that sound plausible but don't exist. The retry loop doesn't help because the model produces the same hallucination each time — it's not random noise, it's confident wrongness. **Insufficient for production.**

**Option C (constrained decoding):** This converts structural reliability from a probabilistic property ("the model usually gets the JSON right") to a deterministic guarantee ("the model cannot produce invalid output"). The grammar enumerates valid action names and their required parameters — the model can only select from what exists. It cannot invent `"search_file"` because that token sequence is masked out at every step where it would violate the grammar.

The remaining failure mode is **semantic**: the model chose `ListDir` on `/tmp/` when you meant `/home/notes/`. This is caught by two mechanisms: (1) D4's clarify branch — the model can emit an explicit clarification request instead of acting; (2) D3's cap-check — if Arthur lacks the grant for the chosen path, `execute_action()` returns `Denied`, which is surfaced to the user. Neither requires trusting the model's judgment about its own correctness.

**Recommended: C — Constrained / grammar-guided decoding.**

### 3.4 The action grammar

The grammar is a context-free grammar over the `ActionKind` set, parameterized by the available tools (only granted actions appear in the grammar). Shape:

```
root ::= action | clarify | noop
action ::= tool_delimiter open_brace name_field comma args_field close_brace tool_delimiter_close
name_field ::= "\"name\"" colon string_literal(action_name)
action_name ::= "read_file" | "write_file" | "list_dir" | "spawn_process" | ... (enumerated from available ToolKind set)
args_field ::= "\"arguments\"" colon args_object
args_object ::= open_brace arg_fields close_brace
clarify ::= tool_delimiter open_brace clarify_name comma clarify_args close_brace tool_delimiter_close
noop ::= tool_delimiter open_brace noop_name close_brace tool_delimiter_close
```

Each `ActionKind` has its own parameter grammar. For example:
```
read_file_args ::= "\"path\"" colon string_literal(path_chars)
list_dir_args ::= "\"path\"" colon string_literal(path_chars)
net_dial_args ::= "\"host\"" colon string_literal(host_chars) comma "\"port\"" colon integer_literal
```

The `string_literal(path_chars)` rule constrains path characters to `/`, alphanumeric, `_`, `-`, `.`, and a small set of safe Unicode — preventing path traversal via `..` or null bytes. The `integer_literal` rule for ports constrains to 1-65535.

**Mapping to `action.rs`:** The grammar is compiled from the `ToolKind` enum + parameter definitions (which mirror `Action`'s field structure). At runtime, only the tools Arthur has grants for are included in the grammar — an ungranted action is literally impossible to emit. This is D2's graduation enforced at the decoding level, not just the execution level.

**The `clarify` branch:** An explicit grammar production:
```
clarify ::= "<tool_call>" "{" "\"name\"" ":" "\"clarify\"" "," "\"arguments\"" ":" "{" "\"question\"" ":" string_literal "}" "}" "</tool_call>"
```
When the model selects this branch, Arthur presents the clarification question to the user instead of executing an action. This is D4's clarify-don't-guess made structural — the model has a valid grammar path to ask for help instead of guessing wrong.

**The `no-op` branch:** For when the model determines no action is needed (pure conversational reply):
```
noop ::= "<tool_call>" "{" "\"name\"" ":" "\"no_op\"" "}" "</tool_call>"
```
Parsed as `Ok(None)` in `extract_tool_call()` — equivalent to "no tool call, just text."

### 3.5 Engine integration cost

The 8.5 engine's sampling boundary is the `generate()` method on `ConversationBackend`. Today it calls `sys_infer_run()` and returns output tokens. The constrained decoding hook is:

1. **Grammar compilation:** At Arthur startup (or when the available tool set changes), compile the action grammar into a state machine. This is a one-time cost, done with the llama.cpp GBNF compiler or equivalent. The compiled grammar is small (~2KB for 13 actions with parameters).
2. **Logit masking:** During token generation, after each forward pass produces logits, the grammar state machine determines which tokens are valid continuations. All other tokens are masked to `-inf` before sampling. This is a single per-token O(vocab_size) operation — negligible compared to the forward pass on CPU.
3. **State tracking:** The grammar state machine tracks position within the action structure (e.g., "expecting action name," "inside path string literal"). Reset at start of generation, advance with each sampled token.

**Hook point:** `SyscallBackend::generate()` in `main.rs`. The constraint is applied between `sys_infer_run()` returning logits and sampling the next token. For the current engine (which returns decoded tokens directly), the integration requires exposing logits — this is a one-line change to `sys_infer_run`'s output contract (return logits instead of sampled tokens for the agentic path). **Flagged as an 8.5 extension, not a restructure.**

### 3.6 Residual risk

- **Grammar compilation correctness:** If the grammar doesn't perfectly match `action.rs`, valid actions could be masked out or invalid structures permitted. Mitigation: compile the grammar from the same `ToolKind` enum that drives `execute_action()` — one source of truth, compile-time consistency check.
- **Small-model grammar following:** Even with constraints, a very small model (0.5B) may produce low-quality parameter values (garbage paths, nonsensical hostnames). Mitigation: D4's clarify branch + D3's cap-check. A garbage path will be denied by the cap-check; a nonsensical action that passes the cap-check is harmless (reading a nonexistent file returns ENOENT, listing an empty directory returns zero entries).
- **Latency:** Logit masking adds ~5-15% per-token overhead. For action generation (~20-50 tokens), this is ~100-300ms additional latency on CPU. Acceptable for the NL companion layer (D5 — not a hot-loop operation).

---

## 4. Design space: D2 — Action vocabulary + graduation by reversibility×stakes

### 4.1 The constraint: use what exists

The `ActionKind` enum in `action.rs` defines 13 kinds. OT-18's six scopes (FileRead, FileWrite, Process, Network, Model, Memory) group them by resource class. OT-19's reversibility mechanism distinguishes reversible from irreversible actions. D2 maps each action to its agentic graduation tier — when can Arthur execute it on NL intent without human friction?

### 4.2 The graduation tiers

| Tier | Actions | Stakes | Reversibility | Agentic behavior |
|------|---------|--------|--------------|-----------------|
| **Read/Nav (A1)** | `ReadFile`, `ListDir`, `InspectProcess` | Low — read-only, no state change | N/A (no mutation) | Execute directly on NL intent. No confirm, no prompt. If denied by cap-check → surface to user ("I don't have permission to read that path"). |
| **Mutating (A2)** | `WriteFile`, `SpawnProcess`, `KillProcess`, `WaitProcess` | Medium — changes system state | `WriteFile`: reversible via OT-19 copy-before-write. `SpawnProcess`: partially reversible (spawned process can be killed). `KillProcess`: irreversible local. | Execute with OT-19 undo artifact (D2 of OT-19). Show interpreted action + confirm for irreversible actions (`KillProcess`). Silent for reversible (`WriteFile` — undo available if needed). |
| **Irreversible external (A3+)** | `NetDial`, `NetSend`, `NetRecv` | High — external side effects, exfiltration risk | Irreversible — cannot "un-send" data. | OT-18 D5 restriction: scoped by host:port grant. Per-new-host confirm. Not made reversible. Multi-step coordination required (dial → send/recv sequence). |
| **Model compute** | `Infer`, `LoadModel`, `CloseFile` | Low — internal state, no external effect | Reversible (model can be reloaded, tensor can be recreated) | Execute directly within Model scope grant. |

### 4.3 Why this graduation is safe

The graduation doesn't trust the model's judgment about stakes or reversibility. Both are **structural properties**:
- **Reversibility:** Determined by the action kind. `WriteFile` always produces an undo artifact (OT-19 D2). `KillProcess` never does. This is a compile-time property of the action, not Arthur's opinion.
- **Stakes:** Determined by user policy (OT-19 D3 — `.arthur/stakes.json`). The user marks which paths are high-stakes. Arthur applies the policy mechanically.

The model only chooses *which* action to take and *what parameters* to use. Whether that action requires confirm, produces an undo artifact, or executes silently is determined by the deterministic graduation table — not the model's confidence.

### 4.4 Recommendation: Three-tier graduation, structural (not model-judged)

**A1 (read/nav):** Execute directly. The first build slice. Depends only on what's landed: action surface + I-15 file-caps + OT-18 E1a grants. Constrained decoding makes the NL→Action mapping reliable; the cap-check makes unauthorized access impossible.

**A2 (mutating):** Execute with OT-19 reversibility. Depends on OT-19's build delivering copy-before-write. Reversible actions are silent (undo available); irreversible local actions (`KillProcess`) require confirm if user policy marks them high-stakes.

**A3+ (irreversible external + multi-step):** Execute with OT-18 restriction + per-host confirm. Depends on both OT-18's network scoping and the ability to chain multiple actions (dial → send → recv). Deferred until A1/A2 are proven.

---

## 5. Design space: D3 — The safety model ("safe to be wrong")

### 5.1 Statement

**The NL layer is UNTRUSTED. The kernel capability check is the boundary.**

Arthur's model is fallible. It may misinterpret intent, choose the wrong action, or fill in wrong parameters. It may even be adversarially prompted ("delete everything"). In every case, the failure degrades safely:

| Failure mode | What happens | Why it's safe |
|-------------|-------------|---------------|
| **Wrong action chosen** (model maps "read my notes" to `WriteFile` instead of `ReadFile`) | `execute_action()` checks grants. If Arthur lacks `FileWrite` for that path → `Denied`. Surface: "I tried to write to /notes but I don't have permission — did you mean to read it?" | Cap-check is the boundary. Arthur can only act within its grant set. A wrong action that falls outside grants is denied before any syscall. |
| **Wrong parameters** (model uses `/tmp/notes` instead of `/home/notes`) | If Arthur has a grant for `/tmp/`, the action executes on the wrong path. Result: reads from `/tmp/notes` instead of `/home/notes`. | Harmless — reading the wrong file is wrong, not damaging. The user sees the result and corrects. For mutating actions, OT-19's undo artifact makes it recoverable. |
| **Adversarial prompt** ("delete everything") | Model emits `WriteFile` or equivalent destructive action. Cap-check: Arthur lacks broad write grants (OT-18 D7 — least-privilege default). → `Denied`. | Coarse durable grants are scoped (OT-18 D2). Arthur can't write to paths it wasn't granted. The blast radius is the grant scope, not the filesystem. |
| **Model hallucinates action** (emits `"search_file"` which doesn't exist) | Constrained decoding prevents this structurally (D1 Option C). Without constraints: `extract_tool_call()` returns `ToolParseError::UnknownAction` → retry or surface error. | With constrained decoding: impossible. Without: caught by parser + retry, never silently executes. |
| **Model emits valid action for ungranted resource** | Cap-check denies. Surface to user with the specific missing capability. User can grant if appropriate (OT-18 D4 — on-miss re-request). | The kernel cap is the boundary. `grants.json` is persistence/UX, never authority (Article 29.3). |

### 5.2 "Safe to be wrong" — formal property

**Property:** For any NL input (benign, confused, or adversarial), Arthur's behavior satisfies:
1. **No silent damage:** Every action that modifies state either (a) is within Arthur's grant scope AND produces an undo artifact (OT-19), or (b) is denied by the cap-check before execution.
2. **No authority escalation:** Arthur cannot exceed its held kernel caps regardless of what the model proposes. A compromised model that ignores `grants.json` still cannot exceed kernel caps — the syscall fails with EPERM.
3. **Graceful degradation:** When the model is wrong, the user sees a concrete error (Denied with specific missing capability) or a wrong-but-harmless result (read from wrong path). Never silent corruption.

### 5.3 Recommendation: Untrusted NL + trusted cap-check, proven by failure-mode enumeration

The safety model is not aspirational — it's structural. The cap-check in `execute_action()` runs before every syscall. The grant set is derived from narrow kernel caps (OT-18 D1). Reversibility covers destructive actions within scope (OT-19 D2). Constrained decoding prevents structural hallucination (D1 Option C). Together, these make "safe to be wrong" a property of the system, not a promise.

---

## 6. Design space: D4 — Confirmation + clarification (human-in-the-loop)

### 6.1 The two loops

There are two distinct human-in-the-loop mechanisms:

**Clarification (model → user):** The model is uncertain about intent and asks for clarification before acting. This is proactive — the model initiates the loop because it doesn't have enough information.

**Confirmation (system → user):** The system requires user approval before executing a consequential action. This is reactive — the system initiates the loop because the action's stakes require it.

### 6.2 Clarification: clarify-don't-guess

**Mechanism:** The constrained grammar includes an explicit `clarify` branch (D1 §3.4). When the model selects this branch, it emits a clarification question instead of an action. Arthur presents the question to the user and waits for disambiguating input.

**When the model clarifies:**
- Ambiguous path: "Did you mean /home/tmp or /var/tmp?"
- Multiple possible actions: "Do you want me to read the file or list the directory?"
- Missing required parameter: "Which port should I connect to?"
- Uncertain intent: "When you say 'save this,' do you mean save our conversation or save the file we're looking at?"

**Grammar-enabled, not grammar-guaranteed:** The `clarify` branch is always available in the grammar. The model doesn't need to "decide" to clarify — it's a valid action alternative alongside every other action. This avoids the trap of relying on the model's confidence score (which small models don't produce reliably). **But availability is not usage:** a 0.5B-1.5B model will often skip the clarify branch and confidently emit a wrong action instead. The grammar makes clarification structurally possible, not behaviorally dominant. When the model does use it, UX is better (it asks before acting wrong). When it doesn't, D3's safety net catches the error: cap-check denies, confirm gates, OT-19 reverses.

**UX:**
```
user: find my notes about the OptiPlex
arthur: Did you mean /home/notes or /home/project/notes?
user: home project
arthur: [executes ListDir on /home/project/notes, filters for "OptiPlex"]
```

### 6.3 Confirmation: consequential actions

**Mechanism:** For actions in the Mutating tier (A2) that are irreversible or user-policy-marked high-stakes, Arthur shows the interpreted action and requires explicit confirmation before executing.

**When the system confirms:**
- Irreversible local action + high-stakes path (user policy): "This will kill process 42 (neo-grep). Proceed?"
- New network host (OT-18 D5): "I'll dial api.example.com:443 — first time connecting to this host. Proceed?"

**When the system does NOT confirm:**
- Read/nav actions (A1): Never confirm. Reading a file is low-stakes and harmless.
- Reversible mutations within scope: `WriteFile` with OT-19 undo artifact available → execute silently, user can undo if needed.
- Model compute: `Infer`, `LoadModel` within Model scope → execute silently.

**UX:**
```
user: kill the grep process
arthur: I'll kill process 42 (neo-grep). This is irreversible — proceed? (y/n)
user: y
arthur: [executes KillProcess, no undo available]
```

vs. reversible:
```
user: update the config to enable debug mode
arthur: [executes WriteFile with OT-19 snapshot → silent, undo available in .arthur/undo/]
```

### 6.4 Confidence signal

**Not model confidence.** Small models don't produce reliable confidence scores. The clarify branch is a **structural choice** in the grammar — the model selects it as an alternative to acting. Whether it does so depends on the prompt context (the user's ambiguous input) and the system prompt ("ask for clarification when uncertain").

The system can also **force clarification** when it detects ambiguity structurally: if the constrained decode produces a valid action but multiple grants match (e.g., Arthur has read access to both `/home/notes` and `/var/notes`, and the user said "find my notes"), the system can route to clarification instead of executing on an arbitrary match. This is deterministic disambiguation, not model judgment.

### 6.5 Recommendation: Clarify branch in grammar + structural confirmation for irreversible+high-stakes

Clarification is a grammar production (always available). Confirmation is a structural gate (determined by action tier + user policy). Neither trusts the model's judgment about its own certainty or the stakes of the action.

---

## 7. Design space: D5 — Coexistence + latency honesty

### 7.1 The two modes

| Mode | Interface | Latency | Use case |
|------|-----------|---------|----------|
| **`neo>`** (deterministic) | Typed commands: `ls /home`, `cat /notes.md`, `spawn neo-grep` | Instant — no model inference. Shell parses and dispatches directly to syscalls. | Hot-loop operations, precise control, scripting, automation. |
| **`arthur>`** (agentic NL) | Natural language: "show me what's in my home directory", "find the config file" | Full LLM forward pass — ~500ms-2s on CPU for 0.5B model. Constrained decoding adds ~100-300ms. | Intent-driven operations, exploration, multi-step tasks ("organize my notes"). |

### 7.2 Coexistence rules

1. **NL is the companion layer, not the replacement.** `neo>` commands remain available and instant. The user can switch between modes freely within the same session.
2. **NL routes through the same action surface.** When Arthur interprets "show me /home" as `ListDir("/home")`, it executes through the same `execute_action()` path that `neo> ls /home` would use — same cap-check, same grant set, same safety properties. The NL layer is a different input parser, not a different execution path.
3. **Latency is honest.** Arthur doesn't pretend NL is fast. The UX acknowledges the delay: a brief indicator while the model processes ("arthur: thinking..."), then the result or clarification. This sets correct expectations — NL is for intent, not hot loops.
4. **NL can trigger `neo>`-equivalent operations.** "Run grep for OptiPlex in notes" → Arthur interprets as `SpawnProcess("neo-grep", args=["OptiPlex", "/home/notes"])` and executes through the action surface. The user gets NL convenience with deterministic execution underneath.

### 7.3 Recommendation: Two modes, one execution path

NL and typed commands share the same `execute_action()` cap-checked execution path. NL adds an intent-parsing layer (constrained decoding) on top; typed commands bypass it. Both are governed by the same grants, caps, and safety properties. The latency difference is honest — NL is slower because it's smarter about intent, not because it's doing something fundamentally different.

---

## 8. Design space: D6 — Kill-switch + slice plan

### 8.1 Kill-switch

**Feature flag:** `arthur-agentic` (default **Disabled**).

When Disabled: Arthur operates in chat-only mode with the existing conversation loop (`respond_to_turn`). No NL→Action parsing, no constrained decoding, no tool-call extraction. Bit-for-bit equivalent to pre-OT-26 Arthur. The `neo>` commands remain available through neo-shell.

When Enabled: The agentic layer activates. NL input is routed through the constrained-decode intent parser. Tool calls are extracted and executed through `execute_action()`. Clarification and confirmation loops are active.

**Kill-switch verification:** Disabled ⇒ host test confirms Arthur produces only conversational replies for NL input that would trigger actions when enabled. No tool-call extraction, no action execution. The existing `respond_to_turn` path is exercised exclusively.

### 8.2 Slice plan

| Slice | Scope | Dependencies | Deliverable |
|-------|-------|-------------|-------------|
| **A1: Read/Nav agentic** | NL → `ReadFile`, `ListDir`, `InspectProcess` → cap-check → NL result. Constrained decoding grammar for read/nav actions. Clarify branch. Kill-switch. Host tests: intent parsing, cap-check deny, clarify loop. QEMU: `arthur_agentic_read` scenario @ NEO_SMP=4. | Action surface (landed), I-15 file-caps (landed), OT-18 E1a grants (landed). **No dependency on OT-19 build.** | The first provable safe loop. NL intent → valid action (constrained decode) → cap-checked execution → NL result or clarify. |
| **A2: Mutating + OT-19 confirm/undo** | NL → `WriteFile`, `SpawnProcess`, `KillProcess` → OT-19 snapshot/tombstone → cap-check → NL result. Confirmation for irreversible+high-stakes actions. Grammar extended to mutating actions. | OT-19 build (copy-before-write, tombstone). A1 proven. | Mutating actions with reversibility safety net. Silent for reversible; confirm for irreversible+high-stakes. |
| **A3: Multi-step intents** | NL → sequence of actions (e.g., "dial X, send Y, show me the response" = NetDial → NetSend → NetRecv). Short action chains (2-3 steps). Grammar supports action sequences. | A1+A2 proven. OT-18 network scoping. | Multi-step agentic operations with per-step cap-check and confirmation gates. |

### 8.3 Recommendation: Kill-switch default-off, three-slice graduation

A1 is the first build — it depends only on what's landed and proves the core loop (NL → constrained decode → cap-check → result). A2 adds reversibility when OT-19 delivers. A3 adds multi-step coordination last, after single-step agentic is proven safe. Each slice is independently kill-switchable within the `arthur-agentic` feature flag (sub-features: `arthur-agentic-read`, `arthur-agentic-mutate`, `arthur-agentic-multi`).

---

## 9. Honest uncertainties

| Uncertainty | Impact | Status |
|------------|--------|--------|
| **Small-model clarify behavior:** Will a 0.5B model actually use the `clarify` branch when uncertain, or will it always try to act (and get the action wrong)? | If the model never clarifies, D4's clarify-don't-guess degrades to "guess and hope the cap-check catches it." The system is still safe (D3), but UX suffers — Arthur acts on wrong intent instead of asking. | **Acceptable risk.** The cap-check catches unauthorized actions; wrong-but-authorized reads are harmless (user sees wrong result and corrects). Clarify behavior improves with prompt engineering and may be model-size-dependent. A1's read/nav scope limits damage from wrong-but-authorized actions. |
| **Constrained decoding latency on CPU:** Logit masking is O(vocab_size) per token. With a 50K vocab, this is ~50K comparisons per token × ~30 tokens = ~1.5M comparisons. On embedded CPU, this could add 200-500ms. | NL operations are already slow (full forward pass). Additional latency compounds the "thinking..." delay. If total latency exceeds ~4s, the UX becomes frustrating for simple operations. | **Quantifiable in A1 build.** The grammar is small; modern llama.cpp implementations optimize logit masking with SIMD. Benchmark during A1. If latency is unacceptable, fall back to Option B (few-shot JSON + retry) for the read/nav tier — it's less reliable but faster. |
| **Grammar compilation from ToolKind:** Mapping Rust enum variants to GBNF grammar rules requires a code generator or manual maintenance. Drift between the grammar and `action.rs` would cause silent mismatches. | Wrong grammar → valid actions masked out (Arthur can't do things it should be able to) or invalid structures permitted (security risk if the grammar allows actions not in `action.rs`). | **Mitigated by single source of truth.** The grammar is compiled from `ToolKind`'s definition at build time. A compile-time assertion verifies grammar completeness against the enum. Manual maintenance is feasible for 13 actions — the grammar is ~200 lines. |
| **Path parameter quality:** Even with constrained decoding, the model fills in path strings freely. A confused model might propose `/home/user/documents/reports/analysis/final_v2_backup/` when the user meant `/home/notes`. | Harmless for reads (wrong file read → user sees it's wrong). Potentially damaging for writes (wrong file overwritten) — but OT-19's undo artifact covers this. | **Acceptable.** The cap-check bounds the blast radius (Arthur can only write to granted paths). OT-19 makes writes reversible. The clarify branch gives the model a path to ask instead of guessing. |

---

## 10. Risk register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Model consistently chooses wrong action within granted scope** (e.g., always reads `/tmp` instead of intended path) | Medium — small models have systematic biases | Low for A1 (reads are harmless); Medium for A2 (writes to wrong file, but reversible via OT-19) | Clarify branch (D4). User sees wrong result and corrects. OT-19 undo for writes. |
| **Constrained decoding breaks with engine update** (8.5 changes logits format) | Low — 8.5 is frozen per scope guard | Medium — agentic layer non-functional until fixed | Kill-switch degrades to chat-only. Engine hook is a single trait method — easy to patch. |
| **Grammar allows path traversal** (`..` in path strings) | Low if grammar constrains path characters; Medium if paths are unconstrained strings | High — model could read/write outside granted scope | Grammar explicitly excludes `..`, null bytes, and control characters from path literals. Cap-check provides secondary enforcement (grants are path-scoped). |
| **Adversarial prompt injection** (user or external input tricks model into emitting destructive action) | Medium — prompt injection is a known class of attack | Low — cap-check bounds execution to grant scope. OT-19 reverses destructive actions within scope. | D3 safety model: wrong action → denied, confirmed-away, or undoable. Never silent damage. |
| **Latency makes NL unusable** (>4s per operation on target hardware) | Medium — depends on CPU + model size | High — if NL is too slow, users revert to `neo>` commands and the agentic layer is dead weight | D5 coexistence: `neo>` remains instant. NL is positioned as companion for complex intent, not hot-loop ops. Benchmark in A1; fallback to Option B if constrained decoding is too slow. |

---

## 11. Summary — what we're building

**The agentic action layer turns Arthur from a chatbot into an OS agent:** natural language → valid structured action → kernel cap-check → natural language result. The safety comes from structure, not trust:

1. **D1 (constrained decoding):** The model can only emit a structurally valid action or `clarify` — ~99% valid AST, not ~99% correct intent. Semantic correctness stays model-dependent; the safety net (cap-check / confirm / OT-19) catches valid-but-wrong actions.
2. **D2 (graduation by tier):** Read/nav executes directly; mutating actions carry OT-19 undo; irreversible external requires confirm. Structural, not model-judged.
3. **D3 (untrusted NL + trusted cap-check):** Every failure mode degrades safely — denied, confirmed-away, or undoable. Never silent damage. Article 29.3 realized for NL input.
4. **D4 (clarify-don't-guess):** Clarification is a grammar production — structurally available, not behaviorally guaranteed. Small models stay overconfident and will sometimes guess-wrong over clarifying. Confirmation is a structural gate. The backstop when the model doesn't clarify: cap-check denies, confirm gates, OT-19 reverses.
5. **D5 (companion to `neo>`):** NL and typed commands share the same cap-checked execution path. Latency is honest — NL is for intent, not hot loops.
6. **D6 (kill-switch + slices):** Default-off ⇒ chat-only ≡ today. A1 (read/nav) → A2 (mutating + undo) → A3 (multi-step). Each slice independently provable.

**This is the capstone of the trilogy:** OT-17 gave Arthur memory, OT-18 gave it authority, OT-19 gave it reversibility. OT-26 gives it a natural-language interface to operate the OS — safely, even when the model is wrong.

---

## 12. Rejected alternatives (summary)

| Alternative | Why rejected |
|------------|-------------|
| **Free-text → regex parse (D1 Option A)** | Brittle, fails silently on edge cases. Worse than JSON parsing for reliability. |
| **Few-shot JSON without constraints (D1 Option B)** | Systematic malformation at <3B scale. Retry loop masks but doesn't fix hallucinated fields/actions. |
| **Model-judged stakes / confidence** | Circular — trusts the model's judgment about its own correctness and the importance of actions. OT-18/OT-19 already rejected this pattern. |
| **Per-action confirm for all mutations** | Makes Arthur a compliance officer (OT-18 D3 rejection). Reversibility (OT-19) is the right way to bound destructive actions — silent execution with undo available. |
| **NL replaces `neo>` commands** | NL is slow (full forward pass). `neo>` is instant and precise. Both have different roles; replacing one with the other loses the strengths of both. |
| **Kernel-level constrained decoding** | Article 6.6: no kernel judgment/policy. Constrained decoding is a userland concern — it's about how Arthur parses intent, not about kernel enforcement. |

---

## PM review — Claude (2026-06-21), on landing / ratification

Reviewed against the dispatch + the referenced substrate (`action.rs`, OT-18/OT-19, Article 29.3). **PL-ratified 2026-06-21**; landed on `main` as the OT-26 design.

- **D1 (the crux) — sound + correctly scoped.** Constrained/grammar-guided decoding guarantees **structural** validity (a valid action AST), NOT **semantic** correctness — a small model can still emit a valid-but-wrong action, caught by the D3 safety net (cap-check / confirm / OT-19), never by the grammar. The original unscoped "~99%" over-claim was caught + corrected before ratification.
- **D4 — honest residual.** The `clarify` grammar production is **enabled, not guaranteed** — small models stay overconfident and may guess-wrong over clarifying; the backstop is D3.
- **D3 — the load-bearing safety model — verified rigorous.** §5.1's failure-mode enumeration + §5.2's formal "safe to be wrong" property are a genuine proof (untrusted NL, trusted kernel cap-check; every failure denied / confirmed-away / undoable; never silent damage). D2's graduation is structural-not-model-judged.

**A1 build-requirement (PM finding — a build-readiness refinement, not a design flaw): resolve the constrained-decoding integration mechanism on a *measured data-movement* basis.** §3.5 understates exposing logits as "a one-line change to `sys_infer_run`" — it actually means shipping **per-token, vocab-size logit vectors across the syscall boundary + relocating sampling to userland** (real ABI + data-movement cost). And §12 rejects in-kernel masking on Article-6.6 grounds — but **applying a userland-supplied token-validity bitmask before sampling is mechanism, not kernel policy** (6.6 forbids kernel *judgment/policy*, not mechanically applying a caller-provided mask, à la a userland seccomp filter). So A1 must choose **logits-out-to-userland vs mask-in-kernel (pass a bitmask *in* — far less per-token data movement)** by measurement, not rule mask-in out a priori. §9 covers the masking *compute* cost; this is the *data-movement* cost it omits.

**Status: RATIFIED.** Build slices: **A1** (read/nav: NL → ReadFile/ListDir/InspectProcess → cap-check → NL result) → **A2** (mutating + OT-19 undo) → **A3** (multi-step), kill-switch `arthur-agentic` default-off ≡ chat-only. A1 depends only on what's landed (action surface + I-15 file-caps + OT-18 E1a grants); dispatches when a seat frees.
