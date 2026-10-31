<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK (stub): Next-brain candidate — Qwen 3.5 2B (Gated DeltaNet hybrid)

**Date:** 2026-07-03
**Status:** STUB — research parked, not scheduled. Records the PL evaluation so the candidate is tracked (OT-38), not lost. **Not a build; not the next step.** Owner when opened: research/design lane (Prometheus when back, or a design pass).

## 0. The candidate (as described, PL 2026-07-03)
**Qwen 3.5 2B** (Alibaba Qwen, ~Mar 2026; part of the small series alongside 0.8B / 4B / 9B):
- Dense **2B vision-language model** (VLM).
- **Gated DeltaNet *hybrid*** architecture — linear-attention (DeltaNet delta-rule state + gating) interleaved with full-attention layers; the same mechanism the 9B uses to keep the KV/state small at long context.
- **262K native context, extensible to >1M** — on a 2B.
- **Thinking / non-thinking** modes (non-thinking default for the small variants).
- **Tool calling** called out as a strength.

## 1. Why it's genuinely attractive for Arthur
- **Tool-calling strength** is the highest-value trait: the entire agentic stack (A1/A2/A3 + the yolo-within-guardrails posture) rests on the model emitting *valid* tool-calls under constrained decode. A tool-call-strong base lifts that reliability from a higher floor.
- **262K–1M context** pairs with Arthur's persistent memory (OT-17) — vast in-context recall.
- **2B is in our proven size band** (between the landed 1.5B and the OT-38-targeted 3B).
- **Constant-size linear-attention state** is a *better* memory profile than our growing standard-attention KV cache (`Qwen2KvCacheSet`) — relevant to the heap budget we already fight (768 MiB → 1.75 GiB).

## 2. Why it is NOT a config-swap (the core issue)
The 1.5B/3B are trivial for us because they are the **same architecture** as the 0.5B (RoPE / GQA / SwiGLU / RMSNorm) — a `.neom` config swap, forward math frozen. **Gated DeltaNet is a different forward:** a linear-recurrent state with a delta-update rule + gating, interleaved with full-attention layers. Running it means **implementing a new attention/state mechanism in ring 0** — precisely the **OT-16** trigger ("a shift to an SSM/Mamba, MoE, or new attention scheme is a *kernel* change; do NOT let it metastasize into `if qwen / if llama` branches"). So this is "build a second forward," not "run a bigger Qwen."

## 3. The decisive gate (the Gemma-4 lesson — UNANSWERED)
What killed Gemma 4 (OT-38) was not size — it was that **llama.cpp did not implement its forward correctly**, so the OT-30 top-1 parity oracle would have anchored to a **known-wrong reference** (match it → ship degraded output; fix it → diverge from the anchor). **The same question decides Qwen 3.5 2B:**

> **Q1 — Is there a *correct*, trustworthy reference implementation of Gated DeltaNet to anchor our parity oracle against?**

Linear-attention / DeltaNet support in llama.cpp (and similar) is exactly the class that is partial or subtly-wrong early. **This must be verified from raw source before the candidate is viable** (byte-oracle-acquisition discipline — do not trust a summarizing read). If no correct reference exists, it is a non-starter until one does, same as Gemma 4.

## 4. Open questions to resolve before any build
- **Q1 (above):** correct parity reference for Gated DeltaNet. **Gating.**
- **Q2 — OT-16 appetite:** do we do the *data-driven forward-graph generalization* (a `.neom`-described layer schedule) rather than hardcode a second forward? Adopting DeltaNet is the natural forcing function for OT-16; doing it any other way is the anti-pattern OT-16 warns against.
- **Q3 — VLM scope:** run text-only (ignore the vision tower) for Arthur, or is multimodal in scope? (Multimodal is a separate, larger axis.)
- **Q4 — new-op byte-oracles:** each new op (DeltaNet recurrence, gating, delta rule, the hybrid layer schedule) needs its own independent byte-oracle + KAT — more surface than the F1 quant kernels.
- **Q5 — quant/import:** does a trustworthy GGUF (or other) quantization exist that round-trips to `.neom` with a verifiable byte-oracle, given the non-standard layers?

## 5. Verdict + recommendation
**Strong *eventual* candidate — its tool-calling + huge context are exactly Arthur's needs — but NOT the next step and NOT a slot-in.**
- **Now:** stay the **OT-38 course** — Qwen2.5 1.5B/3B config-swap (proven forward, zero architecture risk) for the immediate capability step.
- **Q1 RESOLVED ✅ YES (2026-07-04, `RESEARCH-2026-07-04-gated-deltanet-reference-FINDINGS.md`):** a correct reference exists (HF `torch_recurrent_gated_delta_rule` — best parity anchor; llama.cpp `GGML_OP_GATED_DELTA_NET` — full CPU/CUDA, not the Gemma-4-PLE trap; vLLM; GGUF import viable). The feasibility blocker is cleared.
- **Q2 RESOLVED ✅ RATIFIED (PL 2026-07-05, `THINK-2026-07-05-ot16-forward-graph-design.md`):** the OT-16 build is ratified as a Phase-9 slice (B1→B3, after the 1.5B/3B config-swap) — the candidate is now a *scheduled pipeline*, no longer gated. Original framing: with a correct anchor in hand, this becomes a **Phase-9-scale build** — the OT-16 data-driven forward-graph generalization + a verified Gated DeltaNet forward + the parity/byte-oracle discipline. It's now a **prioritization** decision (do we spend a Phase-9 slice on a second, non-Llama forward for the tool-calling + long-context payoff?), not a feasibility question. Pairs naturally with the agentic/tool-calling direction.

*Tracked under OT-38 (next-brain strategy). Reopen when Q1 (correct reference) is answerable.*
