<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 8 BUILD-8.2 — GGUF importer + BPE tokenizer → the Phase-8 coder

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** the Phase-8 coder (new branch off `origin/main` HEAD: `codex/phase-8-build-8.2-gguf-importer`). *(Janus has Phase-8 context from 8.1; Jarvis is also free — PL assigns.)*
**Status:** DISPATCHED. **Host-tier — NO silicon, NO dongle. Gate = published/raw-source byte-oracles (I-12) + loading the real model.** The first build on the **grounded** Phase-8 slicing (gap-analysis FINDINGS) now that 8.1 (the decode loop) has landed.
**Format decision — RATIFIED (PL, 2026-06-14):** `.neom` stays the **native** model object (Articles 21/23.4); **GGUF enters through an *importer*** that loads into the existing `Model` object. **Do NOT** build a `.neom` v2 quant format and **do NOT** adopt GGUF as native — an importer, per *"we define our own; importers handle compatibility."*

## 0. The decision this build makes
Make LogansOS able to **read a real quantized model.** A GGUF parser (header + metadata + tensor table + quantized weight blocks) and a **BPE tokenizer**, so the target — **Qwen2.5-0.5B at Q4_K_M** (~491 MB; gap-analysis §1) — loads and tokenizes. It does **not** dequant (8.3), run RoPE/SwiGLU/attention (8.4/8.5), or execute a forward pass. It loads the bytes and turns text into token-ids.

## 1. Authoritative references (on `main` — read first)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §3 GGUF byte layouts, §4 tokenizer + KAT plan, §6 slicing
git show origin/main:Plans/THINK-2026-06-14-phase-8-opening-inference-maturity.md             # the amendment: grounded 8.2→8.5 + the GGUF-importer ratification
```
The FINDINGS is the byte-layout source of truth (raw-pulled from llama.cpp). **The corrected `block_q4_K` is 144 bytes / 256 values** (2 B `d` + 2 B `dmin` + 12 B `scales` + 128 B `qs`; the `ggml-common.h:328` static_assert) — *not* 72. `block_q8_0` = 34 B / 32 vals. Use the FINDINGS layouts, and **re-confirm any exact byte against the raw `ggml-quants.c`/`gguf.c` you cite — never a summarizing fetch** (the raw-source-KAT rule; the 72→144 catch is exactly why).

## 2. ISCs (each gated by a byte-oracle — I-12, raw-source, NOT round-trip)
- **ISC-1 — GGUF container parser.** Parse the **GGUF v3** header (magic `GGUF`, version, tensor-count, metadata-kv-count), the **KV metadata** block (the typed key→value entries — arch, hyperparams, tokenizer arrays), and the **tensor info table** (name, n_dims, shape, ggml dtype, offset). **Byte-oracle:** a **hand-laid GGUF fixture** (a tiny header + 1–2 tensor-info entries laid out byte-for-byte from the spec) asserted independently — the format's own worked example, not a writer/reader round-trip.
- **ISC-2 — Quantized block extraction (parse, don't dequant).** Read the tensor data region into **typed quantized-block buffers** — `block_q4_K` (144 B/256) and `block_q8_0` (34 B/32) — preserving the exact block structure for 8.3 to dequant. **Byte-oracle:** a hand-laid Q4_K + Q8_0 block asserted field-by-field (`d`/`dmin`/`scales`/`qs` offsets + values) against the FINDINGS/`ggml-quants.c` layout. **Do NOT dequant here** (that's 8.3) — just extract the blocks correctly.
- **ISC-3 — BPE tokenizer.** The vocab + merges (from the **GGUF tokenizer arrays**, or a `tokenizer.json` slice), byte-level pre-tokenization, the merge loop, special tokens. **KAT:** the published test string → known token-ids from the gap-analysis §4 tokenizer-KAT plan (raw-source vocab; **never WebFetch the vocab bytes**). Detokenize round-trips the ids back to the string.
- **ISC-4 — The importer → `Model`.** Wire the parser to load a GGUF into the existing `Model` object (the ratified importer path; `.neom` loading untouched). **Integration gate:** load the real `Qwen2.5-0.5B-Q4_K_M.gguf` and enumerate its tensors (names/shapes/dtypes) — proving the parser handles a real 491 MB model, not just the fixtures.
- **ISC-5 — Standards.** Host-testable tier (the parser + tokenizer are pure logic — `*_logic`/`ai-ops-tests`); a new default-off feature (e.g. `gguf`) gates it (kill-switch ≡ today); the RustCrypto-style "reuse vetted crates if any apply, else hand-write with byte-oracles" posture; `fmt`/`clippy -D warnings`; **no new `unsafe`**, no AI-hot-path touch, no new invariant. Open **ANSPEC §11.2** (the model-import path — confirm next free §11.x) + **ADR-055**. Note in the RETURN how the importer relates to ADR-028's `.neom` (coexist; `.neom` native, GGUF imported).

## 3. Gate (host — no silicon)
- Host **byte-oracles** green: the GGUF header/tensor-info fixture, the Q4_K + Q8_0 block fixtures (field-exact), the tokenizer encode→known-ids KAT.
- **Integration:** loading the real Qwen2.5-0.5B-Q4_K_M GGUF enumerates tensors correctly (if the 491 MB file is available in the test env; if not, BLOCKER-to-PM on sourcing it — and the fixtures still gate the layouts).
- Builds on+off the feature; family green; fmt/clippy clean. **Let CI run** on push (code change — no `[skip ci]`; the host gate runs cheap on the self-hosted runner).

## 4. Scope guard
GGUF parse + tokenizer + the importer-into-`Model` only. **Do NOT** dequant (8.3), implement RoPE/SiLU/SwiGLU/GQA-attention/KV-cache (8.4/8.5), or run a forward pass. **Do NOT** build a `.neom` v2 quant format or make GGUF native (importer only — Articles 21/23.4). **Do NOT** WebFetch exact GGUF/quant/vocab bytes (raw `git`/`curl`, byte-oracle independently). If a vetted `no_std` GGUF/BPE crate fits the dependency posture, reuse it (Article 15.7) — else hand-write with the byte-oracles; flag the choice.

## 5. Merge (PM)
Close-gate: rebase onto current `main`, re-run the host byte-oracles + tokenizer KAT + the real-GGUF tensor enumeration, push (CI runs). PM `pm-merge-check` → `--ff-only` → ledger (Article 13 + ANSPEC §11.2 + ADR-055). Then **8.3 (Q4_K dequant + the f32 inference path)** — route the extracted blocks → f32 through the existing matmul, the first forward pass on a real model's weights.

---

## Relay — what to tell the coder
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-phase-8-build-8.2-gguf-importer.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.2-gguf-importer.md
  git show origin/main:Plans/RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md   # §3/§4 byte layouts + tokenizer KAT
  ```
- **Branch:** `codex/phase-8-build-8.2-gguf-importer` off `origin/main` HEAD.
- **One-line:** Make LogansOS read a real quantized model. **GGUF importer** (v3 header + KV metadata + tensor table + extract `block_q4_K`=**144 B/256** and `block_q8_0`=34 B/32 into typed buffers — *parse, don't dequant*) + a **BPE tokenizer** (raw-source vocab; published test-string→token-ids KAT). Wire it to load into the existing `Model` object — **importer, not native; `.neom` stays native (Articles 21/23.4); no `.neom` v2.** Gate = **hand-laid byte-oracles** (GGUF header + Q4_K/Q8_0 blocks field-exact, I-12 not round-trip) + the tokenizer KAT + loading the real **Qwen2.5-0.5B-Q4_K_M** and enumerating its tensors. Host-tier, default-off feature, no `unsafe`/no new invariant. **Raw-source the exact bytes — never WebFetch** (the 72→144 q4_K catch is why). Open ANSPEC §11.2 + ADR-055. Let CI run (no `[skip ci]`). Next: 8.3 (dequant + f32 forward).
