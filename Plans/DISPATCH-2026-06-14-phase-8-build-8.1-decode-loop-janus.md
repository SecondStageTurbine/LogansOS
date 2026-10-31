<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.1 — the autoregressive decode loop + sampling → Janus

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Janus (new branch off `origin/main` `1bf1327`: `codex/phase-8-build-8.1-decode-loop-janus`)
**Status:** DISPATCHED. **Host + QEMU — NO silicon, NO dongle.** The first build of Phase 8 (inference maturity → the JARVIS L1 layer), PL-ratified 2026-06-14 (`Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md`, Q1–Q4 ratified as-is).
**Welcome to the project, Janus.** You're opening a new front — turning the substrate from "computes one forward pass" into "serves a model" — in parallel with Jarvis (the latency oracle) and the Phase 7.5 WiFi tail. Read the THINK first; it has the verified substrate map you're building on.

## 0. The decision this build makes
LogansOS today computes a **single transformer forward pass, governed** — `exec::run(model, inputs)` returns the output once. BUILD-8.1 makes it **generate**: a loop that samples the next token from the model's logits, feeds it back as the next input, and repeats — the autoregressive spine of every LLM. **Correctness-first and deliberately un-optimized:** it recomputes the full growing context each step (O(n²)); the KV cache that makes it O(n) is **8.2's** job, not yours. Token-IDs in, token-IDs out — **no tokenizer (8.3), no KV cache (8.2), no quantization (8.4)** in this build.

## 1. What you build ON (verified symbols — do NOT re-derive)
- **The forward pass:** `kernel/src/ai/exec.rs:44` `pub fn run(model: &Model, inputs: &[Arc<Tensor>]) -> Result<Vec<Arc<Tensor>>, InferenceError>` — call this **once per generated token**. It already mints weights, places/evicts, dispatches every op, recency-touches — the whole governed hot path. You wrap it; you do **not** modify it.
- **The op set (f32):** `kernel/src/ai/ops/mod.rs` — Matmul/Softmax/LayerNorm/RmsNorm/Gelu/Relu/Embedding(op 9, i32 indices→f32 table)/… All you need to run a small LM exists.
- **The model + IO contract:** `kernel/src/ai/model.rs`, `model_logic.rs` (.neom v1, ADR-028). **Confirm the IO contract first:** the model input is the token-id sequence (the Embedding op gathers it); the output is the logits tensor. Establish exactly what shape `run` returns (logits `[seq, vocab]` vs `[vocab]`) and read the **last position's** logits to sample — pin this from `model.rs`/`exec.rs`, don't assume.
- **Host oracle tier:** `tools/ai-ops-tests/src/lib.rs` (the `*_logic`/known-answer pattern). Your sampler + loop logic live in the host-testable tier where possible.

## 2. ISCs
- **ISC-1 — The decode loop.** A new `generate` module: given a prompt (`&[u32]` token-ids), the model, and a sampling config + stop condition (`max_new_tokens` + an `eos_token_id`), loop: build the input tensor from the current token sequence → `exec::run` → take the **last-position logits** → sample the next token → append → repeat until EOS or max. Returns the generated token-ids. **Recompute the full context each step** (no cache — correctness-first). Pin the input-tensor construction and the logits-extraction from the real IO contract (ISC scope: confirm, then implement).
- **ISC-2 — The sampler.** `argmax` (greedy, deterministic), plus `temperature` (scale logits by 1/T before softmax), `top_k` (keep the k highest logits), and `top_p` (nucleus: smallest set whose cumulative softmax prob ≥ p), composing in the standard order (temperature → top-k → top-p → sample from the renormalized categorical). Each mode host-KAT'd against a **hand-computed** expected token (I-12 — published/hand-computed answer, **not** round-trip).
- **ISC-3 — The seeded PRNG (net-new; non-crypto).** Sampling needs a deterministic RNG so the KATs are known-answer. Add a small, **documented, well-specified non-crypto PRNG** (e.g. a 64-bit `xorshift*` or PCG-XSH-RR — pick one with a **published reference sequence**). KAT its output stream against that algorithm's published values (the raw-source-KAT rule — pull the reference vector from the raw source, never a summarizing fetch). **Do NOT** use a crypto RNG or hand-roll one without a reference; sampling RNG is non-crypto, but it must be *deterministic and referenced* so the sampler KATs are reproducible.
- **ISC-4 — The whole-loop byte-oracle (the integration KAT).** Build a **tiny generation-test `.neom`** — a minimal LM (Embedding → a small block → an output projection to a small vocab, e.g. vocab=8) whose **greedy-decode token sequence is hand-computable**. Assert the decode loop reproduces that exact sequence from a fixed prompt. This is the I-12 oracle for the *whole loop* (not just the sampler, and not round-trip): a hand-laid model + a hand-computed greedy sequence the loop must match byte-for-byte (token-for-token).
- **ISC-5 — Standards.** Additive + feature/scenario-gated (off ⇒ bit-for-bit today — the kill-switch); host KAT suite green (ISC-2/3/4); a **QEMU scenario** generating N tokens from the test `.neom` end-to-end (the family stays green; NEO_SMP=4 unaffected); `cargo fmt`; `clippy -D warnings`; **no new invariant, no new lock, no `unsafe`**. Open **ANSPEC §11** (the inference-serving model — generation, sampling, the autoregressive contract; §11 is currently unopened) + **ADR-053** (the decode loop + sampler; record the recompute-context-correctness-first decision and that the KV cache is 8.2).

## 3. Gate (host + QEMU — no silicon)
- **Host:** ISC-2 sampler KATs (greedy deterministic + seeded temp/top-k/top-p → known tokens), ISC-3 PRNG sequence vs the published reference, ISC-4 the tiny-`.neom` greedy sequence reproduced exactly; builds on+off the feature; clippy/fmt.
- **QEMU:** the N-token generation scenario runs end-to-end on the test model, family green, kill-switch ≡ today.
- **No Stage B / no silicon.**

## 4. Scope guard
The decode loop + sampler only. **Do NOT** build the **tokenizer** (8.3 — operate on token-IDs), the **KV cache** (8.2 — recompute context, correctness-first; do not optimize here), or **quantization** (8.4 — f32 only). **Do NOT** modify `exec::run` or any governance hot-path function (you wrap, you don't restructure — Jarvis's BUILD-LO is benchmarking those exact functions in parallel; keep your surface disjoint). **Do NOT** hand-roll a crypto RNG or an unreferenced PRNG. **Do NOT** assert round-trip — every gate is a hand-computed/published known answer. If the model IO contract can't express a vocab-logits output without restructuring the executor, BLOCKER-to-PM (we'll decide the minimal `.neom` shape together) rather than churning `exec.rs`.

## 5. Why correctness-first (the design rationale, so it's not mistaken for an omission)
The O(n²) recompute is **deliberate**. Each Phase-8 build keeps a clean correctness gate; 8.2's KV cache is the optimization, proven against 8.1's correct output as its reference. The latency this exposes is exactly the workload Jarvis's latency oracle (BUILD-LO) will measure — 8.1 is the oracle's first real per-token consumer. Correct first, fast second — the CPU-fallback thesis.

## 6. Merge (PM, on gate PASS)
Close-gate: rebase onto current `main`, re-run the host KATs + the QEMU generation scenario + the family, push. PM `pm-merge-check` → `--ff-only` → `main`; ledger the audit log + ANSPEC §11 + ADR-053. Then **8.2 — the KV cache wired to residency governance** (the showcase: KV cache *is* governed tensors — the thesis made concrete), driving 8.1's proven loop.

---

## Relay — what to tell Janus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-phase-8-build-8.1-decode-loop-janus.md`
- **The phase THINK (the substrate map + the build sequence):** `Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md`
- **Read them from `main`** (your tree is on a feature branch — a local file-open won't find them):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-phase-8-build-8.1-decode-loop-janus.md
  git show origin/main:Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md
  ```
- **Branch:** `codex/phase-8-build-8.1-decode-loop-janus` off `origin/main` `1bf1327`.
- **One-line:** Turn the single-pass executor into **generation**. A new `generate` module that calls `exec::run` (@ `exec.rs:44`) **autoregressively** — sample next token from the last-position logits, append, repeat to EOS/max — **recomputing the full context each step** (no KV cache; that's 8.2). Add a sampler (argmax + temperature/top-k/top-p) driven by a **documented seeded non-crypto PRNG** (referenced, so the KATs are deterministic). **Gate = hand-computed/published known-answer KATs (I-12, not round-trip):** the sampler modes, the PRNG sequence vs its published reference, and — the integration oracle — a **tiny hand-laid `.neom` whose greedy-decode token sequence is hand-computable**, reproduced exactly + a QEMU N-token run. Token-IDs in/out (no tokenizer, 8.3). f32 (no quant, 8.4). **Additive only — do NOT touch `exec::run` or the governance hot-path functions (Jarvis is benchmarking those in BUILD-LO right now — keep surfaces disjoint).** Feature-gated, kill-switch ≡ today, no new invariant/lock/`unsafe`. Open ANSPEC §11 + ADR-053. Next: 8.2 (KV cache).
