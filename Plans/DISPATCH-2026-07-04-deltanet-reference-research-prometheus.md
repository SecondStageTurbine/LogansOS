<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Gated-DeltaNet reference research — the Q1 gate for the Qwen 3.5 2B candidate → Prometheus

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Prometheus (research / prior-art / host-only lane). New branch off `origin/main` HEAD (`e406a83`): `prometheus/deltanet-reference-research`. **(Queued task 1 of 2.)**
**Status:** DISPATCHED — **answer the ONE question that gates the Qwen 3.5 2B next-brain candidate.** The candidate (OT-38; `THINK-2026-07-03-next-brain-qwen35-deltanet-candidate.md`) is a strong eventual fit (tool-calling + huge context) but its architecture is **Gated DeltaNet (linear attention)** — a *new ring-0 forward*, not a config-swap. Its viability hinges on **Q1**: *is there a **correct** reference implementation to anchor our OT-30 llama.cpp top-1 parity oracle against?* This is the exact question that killed Gemma 4 (llama.cpp implemented its PLE wrong → matching it would ship degraded output). **Scope: research/docs, host-only — NO code, no QEMU. Output is a FINDINGS doc.**

## 0. Why this is the decisive question
Our entire correctness discipline for a brain is the **parity oracle**: the kernel forward must match a *correct* reference (OT-30). If no trustworthy reference implements Gated DeltaNet's forward correctly, the candidate is a **non-starter** — matching a wrong reference ships wrong output; implementing it "right" diverges from the anchor (the Gemma-4 trap). Answering Q1 converts OT-38's DeltaNet candidate from "unknown" to a clean **yes/no**.

## 1. What to determine (from REAL source — never a summarizing fetch; byte-oracle-acquisition rule)
- **Does llama.cpp implement Gated DeltaNet correctly?** Read the actual source: is there a Gated-DeltaNet / linear-attention / DeltaNet op in `llama.cpp`/`ggml` (and the Qwen-3.5-small graph)? Is the **delta-rule state recurrence + gating** implemented in the *forward graph*, or stubbed / approximated / absent (the Gemma-4-PLE failure mode — "runs but the mechanism is never applied → subtly degraded")? Cite the exact code / issue / PR.
- **Is there ANOTHER trustworthy reference?** The **official Qwen 3.5 repo / HF `transformers` modeling code** (the authoritative forward), **vLLM**, or a reference from the DeltaNet/Gated-DeltaNet papers (Yang et al. and the gated variant). Which of these implements the forward *correctly and completely*, and could serve as our parity anchor (even if slower/Python — the anchor need only be correct, not fast)?
- **Is a GGUF (or other quant) path even available**, and does the quantization round-trip the non-standard layers, or does GGUF go text-only / drop the linear-attention state (as Gemma-4 GGUFs did)?
- **The verdict:** for each candidate reference — correct / partial-wrong / absent, with the evidence. Net: **is there a correct reference we could anchor OT-30 parity against — YES or NO?**

## 2. ISCs (all ANALYSIS)
- **ISC-1 — The Q1 verdict.** A clear YES/NO on "a correct Gated-DeltaNet reference exists to anchor our parity oracle," with the per-reference evidence (llama.cpp, transformers, vLLM, paper code).
- **ISC-2 — If NO:** what's missing and would it be fixable by us (implement-and-self-verify against the paper math), or is it Gemma-4-class (no correct anchor → park). If YES: which reference, and the rough shape of the import/parity path.
- **ISC-3 — Update the recommendation.** Confirm or revise the candidate THINK's verdict (strong-eventual-candidate vs park-until-reference-exists) based on the Q1 answer.

## 3. Scope guard
**Host-only + a FINDINGS doc.** No code, no QEMU, no Cargo feature. Exact claims (which reference implements what) from **reading the real source / issues / PRs**, not summaries. Output: `Plans/RESEARCH-2026-07-04-gated-deltanet-reference-FINDINGS.md`.

## 4. Gate + close
- **Host only:** no code ships; `cargo fmt`/SPDX clean on the doc; **no QEMU** (docs, paths-ignored).
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). RETURN the FINDINGS path + the **Q1 YES/NO verdict** + the per-reference evidence. PM updates OT-38 + the candidate THINK.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-deltanet-reference-research-prometheus.md` (+ `docs/THINK-2026-07-03-next-brain-qwen35-deltanet-candidate.md` for Q1–Q5, OT-38, OT-30). **Branch:** `prometheus/deltanet-reference-research` off `origin/main` HEAD (`e406a83`).
- **One-line:** **Answer the gating question for the Qwen 3.5 2B brain: is there a CORRECT reference implementation of Gated DeltaNet to anchor our OT-30 parity oracle against?** Read real source — does `llama.cpp`/`ggml` implement the delta-rule-state-recurrence + gating in the *forward graph* correctly, or is it stubbed/absent (the Gemma-4-PLE trap)? Check the official Qwen/HF-transformers forward, vLLM, and the paper code as alternative anchors. Produce `Plans/RESEARCH-2026-07-04-gated-deltanet-reference-FINDINGS.md` with a clean **YES/NO Q1 verdict** + per-reference evidence + a confirm/revise on the candidate's park-vs-pursue status. **Real-source reads, never a summarizing fetch. Host-only, no code.** Close-gate; RETURN the FINDINGS + the Q1 verdict.
