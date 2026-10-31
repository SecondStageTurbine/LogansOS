<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Forward-path F3 — real-vocab tokenizer + Qwen chat template (userland) → Prometheus

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Prometheus (free; bounded userland + KAT shape — its proven strength). New branch off `origin/main` HEAD (`bfd2f3d` or later): `prometheus/forward-path-f3-real-vocab`.
**Status:** DISPATCHED — **PL-ratified D3 (forward-path THINK `cf50ffb`).** The third forward-path slice, **fully independent of F1/F2** (userland only — start immediately). Arthur tokenizes with a **hardcoded 7-token fixture vocab** (`BPE_VOCAB`/`BPE_MERGES` in `neo-arthur/src/main.rs`); the real Qwen2.5 vocab is **151 936 tokens / 151 387 merges** (the L1 sidecar). F3 loads the real sidecar at Arthur startup, builds the BPE tokenizer over it, wires it into the conversation backend, and frames turns with the **Qwen chat template** so real text tokenizes/decodes correctly.

## 0. The decision this build makes
Turn "Arthur can only tokenize ~7 fixture tokens" into "**Arthur tokenizes and detokenizes real natural-language text with the real Qwen2.5 vocab + chat framing**" — the userland half of running the real brain. Kill-switched so the fixture path stays bit-for-bit identical when off.

## 1. References (read before code)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-21-forward-path-real-model-integration.md  # D3 (ratified) + §4
git show origin/main:userland/neo-arthur/src/main.rs               # BPE_VOCAB / BPE_MERGES fixture (~7 tokens); SyscallBackend::tokenize()/decode() — the call sites to rewire
git show origin/main:userland/neo-arthur/src/conversation_logic.rs # ConversationBackend trait (tokenize/generate/decode) — the seam the real vocab plugs into
git show origin/main:kernel/src/ai/tokenizer_logic.rs              # BpeTokenizer::from_slices() — accepts raw-source vocab + merge slices (built for the real tables); reuse it
git show origin/main:tools/neomc/src/gguf_import.rs                # what L1 EMITS: the vocab sidecar byte format (151 936 tokens / 151 387 merges) + the bos/eos/pad special-token IDs — F3's parser must match this exactly
# L2 (merged) lets NeoFS serve the sidecar; the .arthur/ FileRead cap (E1b, merged) lets Arthur read its own dir
```
**Reuse `tokenizer_logic::BpeTokenizer` — do not write a new tokenizer.** F3 is *wiring the real tables in*, not building BPE.

## 2. ISCs
- **ISC-1 — Parse the vocab sidecar at startup.** Read the L1 vocab sidecar from NeoFS at Arthur startup; parse it into **owned** `Vec<BpeVocabEntry>` + `Vec<BpeMergeEntry>` (151 936 / 151 387). The parser must match L1's emitted byte format **exactly** (read it from `gguf_import.rs` — raw bytes, not a guess). Immutable after construction.
- **ISC-2 — Build the tokenizer once + rewire the backend.** Build `BpeTokenizer::from_slices(...)` once at startup (not per-call). Replace the fixture references in `SyscallBackend::tokenize()`/`decode()` so the conversation backend tokenizes/detokenizes over the real tables.
- **ISC-3 — Qwen chat template framing.** The sidecar carries the Qwen special-token IDs (`<|im_start|>`, `<|im_end|>`, bos/eos/pad). Wrap each conversation turn with the Qwen chat template before tokenizing (these are *single tokens in the vocab*, not byte-level composition — see the THINK prior-art note). Decode strips/handles them.
- **ISC-4 — kill-switch `arthur-real-vocab` default-OFF ≡ fixture bit-for-bit.** Off ⇒ the existing 7-token fixture path, the `arthur_boots_thinking`/fixture scenario green **bit-for-bit** (the equality anchor). On ⇒ load the sidecar; **if the sidecar is unreadable/malformed, fall back to the fixture** (never crash). No new `unsafe`. No new invariant.
- **ISC-5 — byte-oracle + gate (the format rule).** A **host KAT independent of round-trip**: assert a handful of **known Qwen token ↔ id pairs** (raw-sourced from the Qwen tokenizer — e.g. IWD-style raw source or the HF `tokenizer.json`, **never a summarizing fetch for the exact ids**) tokenize/detokenize to the exact expected ids/bytes through F3's parser + `BpeTokenizer`. Plus a round-trip (encode→decode == input) as the complementary check. **QEMU scenario `arthur_real_vocab`:** feature OFF ⇒ fixture green bit-for-bit; feature ON ⇒ Arthur loads a real-format vocab sidecar (a representative fixture sidecar staged via NeoFS is fine for CI — the full 151 936-token sidecar is the Stage-B proof; mirror F1's mini-fixture decoupling) and tokenizes a real sentence → correct ids, chat template frames with bos/eos present. At `NEO_SMP=4`.
- **ISC-6 — standards.** SPDX on new files. `cargo fmt --all --check` (**run it — the L1 fmt bounce was here**), `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --all-targets -- -D warnings`, kernel clippy — all via Docker. Note the **BPE merge perf** (O(n²)-ish on long inputs) as a known limitation deferred to a perf slice (acceptable for Stage-B short turns) — don't optimize here.

## 3. Scope guard
Real-vocab sidecar load + tokenizer wiring + chat template + the oracle/scenario **only**. **Do NOT**: write a new BPE tokenizer (reuse `tokenizer_logic`), change the `.neom`/sidecar format (L1 frozen — F3 *consumes* it), touch the kernel forward path (F1/F2), or change `conversation_logic`'s trait shape beyond wiring the real tables. No new invariant/`unsafe`.

## 4. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; **the full host-check set incl. `cargo fmt --all --check`** — do not skip it; SPDX; self-stat). Two hard rules: **green CI URL** (push-CI + the 4-shard QEMU `workflow_dispatch` URL — kernel test_harness gains a scenario ⇒ QEMU gate) **— paste the URL, "tests pass locally" is not the gate**; PM re-inspects `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-28: F3 done — Arthur tokenizes real text; F1+F2+F3 = the real brain end-to-end**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-forward-path-f3-real-vocab-prometheus.md` (+ the THINK D3, `neo-arthur/src/{main,conversation_logic}.rs`, `tokenizer_logic.rs`, L1's `gguf_import.rs` sidecar format). **Branch:** `prometheus/forward-path-f3-real-vocab` off `origin/main`.
- **One-line:** Build **OT-28 F3 — real-vocab tokenizer (userland, independent of F1/F2, start now)**: Arthur uses a **7-token fixture vocab**; load the real **151 936-token / 151 387-merge** L1 sidecar from NeoFS at startup into owned tables, build `BpeTokenizer::from_slices()` **once** (reuse `tokenizer_logic` — don't write a tokenizer), rewire `SyscallBackend::tokenize()/decode()`, and frame turns with the **Qwen chat template** (`<|im_start|>`/`<|im_end|>`/bos/eos from the sidecar — single vocab tokens). **Kill-switch `arthur-real-vocab` default-OFF ≡ the fixture path bit-for-bit** (`arthur_boots_thinking` green off; unreadable sidecar ⇒ fall back to fixture, never crash). **Byte-oracle:** known Qwen token↔id pairs (raw-sourced, NOT a summarizing fetch) through F3's parser + `BpeTokenizer`, independent of round-trip. **Parser must match L1's emitted sidecar bytes exactly** (read `gguf_import.rs`). QEMU `arthur_real_vocab` @ NEO_SMP=4 (off ≡ fixture; on loads a representative sidecar fixture + tokenizes a real sentence → correct ids — full 151 936 sidecar is the Stage-B proof). **RUN `cargo fmt --all --check`** (the L1 bounce was a skipped fmt). No new tokenizer/invariant/`unsafe`; don't touch the kernel forward path or the sidecar format. **Scope: sidecar load + wiring + chat template + oracle/scenario ONLY.** **Close-gate: `scripts/worker-close-gate.sh` (full host set incl. fmt; rebase LAST); RETURN the green push-CI + QEMU URLs — paste the URL.**
