<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-16 data-driven forward-graph design THINK — make the DeltaNet appetite decision concrete → Prometheus

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Prometheus (research / design-THINK / host-only lane). **Branch:** `prometheus/ot16-forward-graph-design-think` off `origin/main` HEAD (fetch fresh — the OT-34/identity-sweep batch is landing; if `kernel/src/ai/par_matmul.rs` is not in your base yet, read the live forward code via `git show origin/staging-fix-sweep:kernel/src/ai/forward_logic.rs` until it merges).
**Status:** DISPATCHED — **the design pass that converts OT-38's Qwen 3.5 2B (Gated DeltaNet) candidate from "gated on OT-16 appetite" into a concrete, ISC-decomposed, costed plan the PL can ratify or defer.** Q1 is RESOLVED (a correct parity reference exists — your `RESEARCH-2026-07-04-gated-deltanet-reference-FINDINGS.md`); the remaining blocker is Q2: do we build the OT-16 data-driven forward-graph generalization? Nobody can answer an appetite question about an unscoped build. **Scope: a design THINK, host-only — NO code, no QEMU. Output: `docs/THINK-2026-07-05-ot16-forward-graph-design.md`.**

## 0. The problem (OT-16, verbatim intent)
`kernel/src/ai/forward_logic.rs` is Qwen2-specific: the op *primitives* (matmul/softmax/RoPE/RMSNorm/SwiGLU) are substrate-stable, but the *model-graph orchestration* is hardcoded in ring 0 — a new architecture (Gated DeltaNet's linear-attention hybrid) is today a kernel patch. Per ANSPEC §19.2's own logic, the forward *graph* should be **data-driven from the `.neom` description**: a new architecture becomes a new `.neom`, not new kernel branches. The named anti-pattern to avoid: `if qwen / if llama` metastasis (Article 5.7 applied to model graphs).

## 1. What the THINK must decide (D1–D6)
- **D1 — The graph-description shape.** What does a `.neom`-carried layer schedule concretely look like? (A typed op-sequence table? A per-layer enum config the kernel walks? Full graph vs "layer-kind schedule" — Qwen2 layers are homogeneous; DeltaNet hybrids interleave two layer kinds, which is the actual forcing function.) Decide the *minimum* expressiveness that covers {Qwen2 today, Gated DeltaNet hybrid tomorrow} without building a general NN compiler in ring 0. Name what stays compiled-in (the op primitives, ANSPEC §19.2) vs what becomes data.
- **D2 — Article 6.6 (no policy in kernel).** Argue explicitly why a data-driven graph walker is *mechanism* (the kernel executes a validated schedule the userland/`.neom` supplies) and where the validation boundary sits (a malformed/hostile schedule must be rejected at load, not crash ring 0 — enumerate the validation invariants). This is the same mechanism-not-policy line the A1 logits-vs-bitmask decision used.
- **D3 — The kill-switch + parity proof.** The generalization ships OFF ⇒ the hardcoded Qwen2 path runs **bit-for-bit** (the OT-30 top-1 parity oracle is the proof, plus the ledger's byte-identity guards). ON with a Qwen2 schedule ⇒ *also* bit-for-bit vs the hardcoded path (the real acceptance test: the data-driven walker reproducing today's forward exactly). Spell both regressions.
- **D4 — OT-34 compatibility.** The forward is now threaded (deterministic column-partition fork-join, `par_matmul.rs`, I-1 byte-identity under parallelism). The graph design must state how a data-driven schedule preserves that: which ops thread (the matmuls), which stay serial, and why a DeltaNet recurrence (inherently sequential in the state dimension) doesn't break the determinism story.
- **D5 — The DeltaNet op inventory (Q4/Q5 made concrete).** From your Q1 findings (HF `torch_recurrent_gated_delta_rule` as the anchor): enumerate the exact new ops a Gated DeltaNet hybrid needs (delta-rule state recurrence, gating, the hybrid attention/linear interleave, any norm/proj variants), and for each: its byte-oracle/KAT plan (independent hand-laid fixture per the standing rule), its parity anchor, and its rough size. Plus the import path: does the GGUF→`.neom` conversion round-trip the non-standard layers (Q5), and text-only scope (Q3 — park the vision tower, state it).
- **D6 — The build plan + verdict.** ISC-decompose the actual future build(s) (likely: B1 walker+Qwen2-schedule-parity, B2 DeltaNet ops+KATs, B3 model bring-up+OT-30-parity vs the HF anchor), with a rough effort/risk read per slice and which coder tier each needs. Then the recommendation: is this a Phase-9 slice worth the payoff (tool-calling strength + 262K context + constant-size state for Arthur), or defer? **The PL ratifies; your job is to make the decision cheap.**

## 2. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                                        # OT-16, OT-38
git show origin/main:docs/THINK-2026-07-03-next-brain-qwen35-deltanet-candidate.md
git show origin/main:Plans/RESEARCH-2026-07-04-gated-deltanet-reference-FINDINGS.md   # your Q1 work
kernel/src/ai/forward_logic.rs + par_matmul.rs (post-merge main or staging-fix-sweep)
ANSPEC §19.2; bible Article 5.7 / 6.6 / 1.2
```

## 3. Scope guard
**THINK only — no code, no `.neom` format change, no QEMU.** If D1 forces a `.neom` format-version question, spec it as an open item (a format change is ADR-028-family work with its own byte oracle — name it, don't do it). Surface genuine forks (e.g. "schedule table vs layer-kind enum") as PL ratification points with a recommendation.

## 4. Gate + close
- **Host only / docs:** SPDX clean; no QEMU (paths-ignored).
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). **Your last RETURN reported finished while the branch CI was red — run the close-gate and cite the actual green push-CI URL this time; a RETURN without the URL bounces on arrival.** RETURN: the THINK path + D1–D6 decisions + the ratification points.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-ot16-forward-graph-design-think-prometheus.md` (+ OT-16/OT-38 in OPEN-THREADS, the DeltaNet candidate THINK, your own Q1 FINDINGS). **Branch:** `prometheus/ot16-forward-graph-design-think` off `origin/main` HEAD.
- **One-line:** **Design the OT-16 data-driven forward graph — the THINK that makes the Qwen 3.5 DeltaNet appetite decision concrete.** Decide: the minimal `.neom`-carried layer-schedule shape covering {Qwen2, DeltaNet hybrid} without a ring-0 NN compiler, and what stays compiled-in (D1); why a graph walker is mechanism-not-policy + the load-time validation invariants (Article 6.6) (D2); the double kill-switch proof — OFF ≡ hardcoded bit-for-bit AND ON-with-Qwen2-schedule ≡ hardcoded bit-for-bit via OT-30 parity (D3); compatibility with the just-landed OT-34 threaded matmul determinism (D4); the exact DeltaNet op inventory with per-op byte-oracle/KAT plans + GGUF import + text-only scope (D5); and an ISC-decomposed build plan (walker-parity → ops+KATs → model bring-up) with effort/risk so the PL can ratify or defer cheaply (D6). **THINK only — no code, no format change; spec open items, don't resolve them.** Close-gate **with the green push-CI URL cited in the RETURN — last time the branch was red at "finished"; that bounces on arrival now.**
