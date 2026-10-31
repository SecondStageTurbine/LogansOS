<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur v1 — model selection + the intent/tool-call strategy (deep research) → Prometheus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Prometheus (deep-research/design lane). **Fresh** branch off `origin/main` HEAD: `prometheus/arthur-model-and-intent-research`.
**Status:** DISPATCHED. **Deep research + design only — NO production code, NO `ci.yml`.** One FINDINGS doc under `Plans/`. On the Arthur critical path: it resolves the one hand-wave in the Arthur design and feeds **Slice 4** (intent parsing → capability-bounded action).

## 0. The decision this sets up
The Arthur design (`THINK-2026-06-16-arthur-userland-arc-...` §4C, ratified OD-3=A "delimited tool-call tokens") assumes Arthur's model emits parseable action intents — but **never resolved which model, or how it emits them without us training one** (LogansOS does not train models — Article 23.3, model-agnostic kernel; we *run* off-the-shelf GGUF models). Two unresolved, on-path questions:
1. **What is Arthur's brain?** Qwen2.5-0.5B (proven to run, but tiny) is the *bring-up* model — what's the *useful* v1 model, given on-device CPU fit + the Phase-9 quantized-matmul speedup?
2. **How does an off-the-shelf instruct model emit our action intents** (the Slice-2 catalog: read_file/write_file/list_dir/spawn/.../net_*/infer/load_model) reliably enough for Slice 4 to parse — *without fine-tuning*?

This research produces the answer + a recommendation, so Slice 4 is built against a decided model + intent format.

## 1. What to work out (raw-source from authoritative model cards / docs — cite, don't recall)
- **Model-selection survey.** Candidate on-device instruct models available as **GGUF** (our importer's format): the Qwen2.5-Instruct family (0.5B/1.5B/3B), Llama-3.2-Instruct (1B/3B), Phi-3.5-mini, SmolLM2, etc. For each, the load-bearing facts: parameter count, **q4_K on-disk + resident footprint**, **license** (must be locally-redistributable / on-device-legal — the sovereignty constraint, Article 23), context length, and **native tool-calling / chat-template support**. Map size → expected CPU speed (today's f32-dequant path is slow; **Phase-9 quantized matmul is the speed unlock** — reference the Phase-9 THINK). Recommend a **v1 model** (smallest that's *actually useful* as Arthur — the VISION "useful, not a demo") + a **"when Phase-9 lands" upgrade target**.
- **The intent/tool-call realization (the crux) — WITHOUT fine-tuning.** How to get a chosen off-the-shelf instruct model to emit the OD-3 delimited tool-call tokens reliably. Weigh: (a) the model's **native tool-calling chat template** (Qwen2.5/Llama-3.2 ship tool-calling formats — typically JSON-in-a-delimited-block) + our parser; (b) **special tokens** registered via our BPE tokenizer (can the 8.2 importer add/honor special tokens? check); (c) **constrained / grammar-guided decoding** (force valid action syntax at sample time — the most robust, and it fits our in-kernel sampler); (d) a **prompt-convention + few-shot** approach. Recommend one (or a primary + fallback, mirroring the THINK's "A primary, B interim"). Reconcile with OD-3=A: does "delimited tool-call tokens" mean the native template, special tokens, or grammar-constrained output? **Pin it.**
- **The Slice-2 action mapping.** Confirm the chosen model + format can express the 12 v1 actions (Slice 2's catalog) — the intent schema (action name + typed args) and how it round-trips to `execute_action()`. Flag any action that's hard to express.
- **Constraints honored:** sovereignty (locally-runnable, redistributable license, no cloud/telemetry — Article 23); GGUF (the importer); the in-kernel sampler (8.1) as the decoding substrate (esp. if grammar-constrained decoding is recommended — what would 8.1 need?).

## 2. Deliverable
`Plans/RESEARCH-2026-06-17-arthur-model-and-intent-FINDINGS.md`:
- the model-selection table (param/footprint/license/context/tool-calling, cited) + a **v1 recommendation** + the Phase-9 upgrade target;
- the **intent realization** decision (native template / special tokens / grammar-constrained / prompt-convention) with a recommendation, reconciled with OD-3=A, and what (if anything) the sampler (8.1) / tokenizer (8.2) needs to support it;
- the action-intent schema mapping to Slice-2's `execute_action()`;
- the sovereignty/license check per candidate;
- **open decisions for PM/PL ratification** before Slice 4 is dispatched (esp.: the v1 model, and the intent-format mechanism).

## 3. Scope guard
Research + `Plans/` FINDINGS **only**. **Do NOT** write production code, touch `ci.yml`/kernel/`ai-ops-tests`, pre-empt Slice 3 (the relay, in flight), or assume we will fine-tune a model (we run off-the-shelf GGUF). Raw-source model facts from authoritative model cards / docs (cite); flag anything second-hand. **⚠ PUSH the branch before reporting done.**

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-arthur-model-and-intent-research-prometheus.md` (`git show origin/main:<path>`).
- **Branch:** `prometheus/arthur-model-and-intent-research` off `origin/main` (fresh).
- **One-line:** Deep-research **Arthur's brain + how it emits intents** — the one hand-wave in the Arthur design, on the Slice-4 critical path. (1) **Model selection:** survey on-device **GGUF** instruct models (Qwen2.5-Instruct 0.5B/1.5B/3B, Llama-3.2 1B/3B, Phi-3.5-mini, SmolLM2, …) — param count, q4_K footprint, **license (must be locally-redistributable — sovereignty, Article 23)**, context, **native tool-calling support**; map size→speed (Phase-9 quantized matmul is the unlock); recommend a **v1 model** (smallest that's genuinely useful) + a post-Phase-9 upgrade. (2) **The crux — intent emission WITHOUT fine-tuning** (we run off-the-shelf models, don't train): how to get the model to emit the OD-3 delimited tool-call tokens reliably — native chat-template tool-calling vs tokenizer special-tokens vs **grammar-constrained decoding** (likely most robust, fits our in-kernel sampler) vs prompt-convention; recommend one, reconcile with OD-3=A, say what the 8.1 sampler / 8.2 tokenizer would need. (3) Confirm the 12 Slice-2 actions are expressible + the intent→`execute_action()` schema. Deliver `Plans/RESEARCH-2026-06-17-arthur-model-and-intent-FINDINGS.md` with a recommendation + open decisions to ratify before Slice 4. Raw-source model facts (cite the cards), no fine-tuning assumed. **Research/FINDINGS only — no code.** Push before reporting done.
