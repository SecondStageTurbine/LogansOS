<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-37 — Stage-B coherence: kill the degenerate `<|im_end|>` reply, make Arthur actually answer → Oracle

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Oracle (owns the Stage-B lane — picked it up from Jarvis, landed `a31a056`). New branch off `origin/main` HEAD: `codex/stage-b-coherence-ot37-oracle`.
**Status:** DISPATCHED — **the headline blocker (OT-37).** Stage-B cleared the #GP and the real 485 MB brain now boots + runs the forward to completion in-kernel — **but the reply is `<|im_end|>`** (the EOS token, no natural-language output). Make Arthur produce a **coherent** reply on the real brain. **This is a diagnosis-first iteration, not a from-scratch build** — most of the work is localizing *which boot-time layer diverges from the proven host harness*, then a targeted fix.

## 0. The decision this build makes
Turn "the pipeline runs but Arthur emits immediate EOS" into "**the booted real Qwen2.5-0.5B answers a fixed prompt with a coherent, finite, deterministic natural-language reply, top-1 consistent with the Stage-A/llama.cpp baseline for the first K tokens.**" That is the ISC-3 bar the original Stage-B gate aimed at but met only on the hermetic fixture — now meet it on the *real brain*.

## 1. The anchor: we already have a correct reference
**OT-30 proved the identical real brain hits 16-token llama.cpp top-1 parity in the Stage-A *host harness* (`tools/ai-ops-tests` / `tools/arthur-logic-tests`).** So the forward math is correct. The `<|im_end|>` therefore comes from something the **in-kernel boot does differently from that host harness** — not from the matmul. The whole triage is: *diff the boot against the host harness, layer by layer, until the divergence appears.*

## 2. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                                # OT-37 (the full statement + suspects)
git show origin/main:tools/ai-ops-tests/src/lib.rs                       # Stage-A host parity harness — the KNOWN-GOOD baseline (prompt→tokens→logits→tokens)
git show origin/main:tools/arthur-logic-tests/src/lib.rs                 # Arthur host-side logic tests
git show origin/main:userland/neo-arthur/src/conversation_logic.rs       # boot-time prompt assembly / Qwen chat-template wrapping (suspect #1)
git show origin/main:userland/neo-arthur/src/vocab_sidecar.rs            # boot-time tokenizer / vocab-sidecar mapping (suspect #2)
git show origin/main:userland/neo-arthur/src/main.rs                     # Arthur boot: prompt → tokenize → generate → reply
git show origin/main:kernel/src/ai/generate_logic.rs                     # sampling / stop / EOS logic (suspect #3)
git show origin/main:kernel/src/ai/generate.rs                           # generation entry / first-forward
git show origin/main:kernel/src/ai/qwen_inventory.rs                     # in-kernel Qwen config (suspect #4 — config divergence vs host)
```

## 3. ISCs

### ISC-1 — Instrument the divergence (capture-before-you-fix)
Emit, at boot, the data needed to diff against the host harness for the **identical fixed prompt**:
- **(a)** the **post-chat-template prompt string** + the **tokenized prompt IDs** (after Qwen-template wrapping) — breadcrumb `prompt_tokens ids=[…] n=…`.
- **(b)** the **first-forward logits top-k** (token IDs + values) before sampling — breadcrumb `forward0 top5=[(id,logit)…]`.
- **(c)** the **sampled token + stop decision** — breadcrumb `sample0 tok=… is_eos=…`.
Run the **same fixed prompt** through `tools/ai-ops-tests` host harness and capture the same three. The first place the two disagree **names the layer**.

### ISC-2 — The discriminator truth-table (each suspect → its tell)
- **Chat-template (suspect #1, most likely):** if (a)'s token IDs lack the `<|im_start|>…<|im_end|>\n<|im_start|>assistant\n` framing the host harness uses → the prompt isn't wrapped; the model "completes" by ending the (mis-formatted) turn → immediate EOS. **Fix:** wrap the prompt in the Qwen chat template at boot exactly as the host harness does.
- **Tokenizer / vocab-sidecar (#2):** if (a)'s template *string* is right but the IDs differ from the host's for the same string → the vocab-sidecar mapping / special-token IDs are off. **Fix:** reconcile the boot tokenizer with the host.
- **Sampling / stop (#3):** if (b)'s top-1 is a sensible token but (c) still emits/forces EOS → the sampling or stop logic mis-fires (e.g. EOS not masked at position 0, or greedy over a wrong logit slice). **Fix:** the stop/sampling path.
- **Forward/config (#4, least likely — OT-30 froze the math):** if (b)'s logits themselves diverge from the host for an identical token sequence → an in-kernel config/`qwen_inventory` difference. **Fix:** reconcile config; do NOT touch the OT-30 matmul kernels.

### ISC-3 — Targeted fix + the real-brain coherence proof
Apply the localized fix. Then prove on the **real 485 MB brain** (local/bare-metal — CI has no model): a fixed prompt → a **coherent, finite, deterministic** natural-language reply, **top-1 consistent with the Stage-A baseline for the first K tokens** (reuse the OT-30 parity tokens). Capture the reply + the per-token IDs. *This is the OT-37 close evidence.*

### ISC-4 — Regression-proof the fixture gate
Strengthen the hermetic QEMU fixture so an **immediate-EOS regression is caught in CI**: the fixture's correct behavior must assert a **non-EOS first token** (today's fixture passed *through* the bug). Keep the existing scenarios green.

### ISC-5 — standards / scope.
SPDX; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker. **No kill-switch needed** (this is a correctness fix, not a new policy substrate) — but the existing fixture scenarios stay **bit-for-bit** green. No new `unsafe`/invariant.

## 4. Fast-loop discipline (don't pay the 485 MB tax to find a config bug)
The likely cause (template / tokenizer / sampling) **reproduces on a small real-quant model** — the bug is config/format, not size. **Iterate ISC-1/2 on a small real-quant model (or the host harness directly), localize + fix fast, then confirm ISC-3 ONCE on the full 485 MB.** Do not cold-load 485 MB every iteration.

## 5. Scope guard
Coherence **only**. **Do NOT**: change the OT-30 forward/matmul kernels (parity-frozen — the math is proven), the `.neom`/vocab format (L1), NeoFS (L2), or the #GP/syscall-stack fix (Stage-B, just landed); add a driver; add a new invariant. The fix is expected in **boot-time prompt assembly / tokenizer wiring / sampling-stop**, not the compute.

## 6. Gate + close (PM)
- **Real-brain coherence proof (ISC-3)** — the local/bare-metal full-485 MB coherent reply + first-K top-1 parity vs Stage-A, captured. **This is what closes OT-37.**
- **Hermetic QEMU fixture green (now regression-armed, ISC-4)** at `NEO_SMP=4`. **Kernel-touching ⇒ 4-shard QEMU `workflow_dispatch` mandatory.**
- **Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat). RETURN the **green push-CI + 4-shard QEMU URLs**, the **real-brain reply text + the prompt/forward0/sample0 diff vs Stage-A that localized the bug**, and which suspect it was.
- **Disk reclaim (after PM confirms merge):** delete the build tree once `codex/stage-b-coherence-ot37-oracle` is on `origin/main`; no blob in git. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-stage-b-coherence-ot37-oracle.md` (+ OT-37 in `OPEN-THREADS.md`, the `ai-ops-tests` host harness, `conversation_logic.rs`/`vocab_sidecar.rs`/`generate_logic.rs`). **Branch:** `codex/stage-b-coherence-ot37-oracle` off `origin/main` HEAD.
- **One-line:** **The headline blocker — make Arthur actually answer (kill the `<|im_end|>`).** The real brain boots + runs the forward to completion, but replies with the EOS token = no text. **The forward math is PROVEN (OT-30: 16-token llama.cpp top-1 parity in the Stage-A host harness)** — so the bug is in what the *boot* does differently. **Instrument + diff against the host harness for one fixed prompt:** (a) post-template prompt token IDs, (b) first-forward top-5 logits, (c) sampled tok + EOS decision; the first disagreement names the layer. **Suspects, in order:** Qwen **chat-template** not wrapping the prompt (most likely — produces exactly this symptom) → **tokenizer/vocab-sidecar** IDs → **sampling/stop** (EOS not masked at pos 0) → in-kernel **config** (`qwen_inventory`; do NOT touch the matmul). **Fast-loop on a small real-quant model** (the bug is config/format, size-independent) — confirm ONCE on the full 485 MB. **Prove:** real-brain coherent + finite + deterministic reply, first-K top-1 parity vs Stage-A — that closes OT-37. **Arm the fixture** to catch immediate-EOS (it passed through the bug). **Scope: coherence ONLY** — no forward/`.neom`/vocab/NeoFS/#GP-fix changes, no driver, no new invariant. **Close-gate `scripts/worker-close-gate.sh`; RETURN green push-CI + 4-shard QEMU URLs + the real reply text + the diff that localized it.**

---
## BOUNCE 2026-06-24 (PM, AAG) — the fix is right; the PROOF of the headline has an unverified, rule-violating joint

The RETURN (`2a4843c`) is **excellent work and the three boot-layer fixes are correct + verified** (generate-stub→`sys_generate` real loop; atomic chat-template control tokens; context 32→512). The forward math + greedy sampler are confirmed **shared code** between kernel and host (no divergence). **But the adversarial-audit gate (lens 5, high-confidence) bounces the merge on the load-bearing claim** "the booted real Qwen answers coherently":

**The blocker (point 3 of the completeness lens).** The host-harness test that produces the coherent reply runs on a **hand-transcribed `const PROMPT: [u32;21]`** (`tools/ai-ops-tests/src/lib.rs:4775`, comment "captured verbatim from the QEMU serial"). The kernel builds its real prompt at boot via the real BPE tokenizer (`frame_turn_ids`, `vocab_sidecar.rs:227`). **No gate asserts the kernel's boot prompt_ids == that host PROMPT const.** The framing byte-oracle only proves control-token *placement* with a *mock* byte-identity encoder — it never runs the real 151936-entry BPE on the text segments. So the entire "booted brain answers coherently" composition hangs on a **human transcription of a serial log** — the exact failure mode the standing **byte-oracle acquisition rule** forbids (CLAUDE.md: "exact bytes from raw source, never a summarizing/manual read"; it bit us in BUILD-W0). Escalated to blocker because the composition IS the headline (load-bearing).

**The fix (one assertion — cheap, decisive).**
- **ISC-B1 (required):** add an automated, `NEO_REAL_NEOM`-gated host assertion **`frame_turn_ids(real_vocab, "hello world") == PROMPT`** — i.e. run the *real* BPE tokenizer through the *real* `frame_turn_ids` and assert it equals the exact id vector the host harness decodes. This ties the proof's two ends with a gate instead of a human. **Regenerate `PROMPT` from a raw captured source** (dump the kernel's `prompt_tokens ids=[…]` breadcrumb to a file and read it literally), not by re-typing — the const must come from raw bytes, per the rule.
- **ISC-B2 (fold in, closes lens-3's note):** the same assertion uses the real encoder, so it simultaneously removes the mock-encoder gap in the framing oracle.
- **ISC-B3 (optional, lens-4 note):** add a `prompt.len() > max_context` guard on the `sys_generate` path (positions beyond trained RoPE range) — minor, may defer to a follow-on.
- **Not required here:** the IF=0 inference-preemption finding is tracked separately as **OT-40** (pre-existing, non-blocking).

**Re-gate:** the change is host-test-only (kernel unchanged ⇒ the `2a4843c` QEMU gate still covers the kernel); RETURN the green push-CI with the new `frame_turn_ids==PROMPT` assertion passing under `NEO_REAL_NEOM`, and the raw-capture provenance of `PROMPT`. Then the headline is load-bearing-proven (modulo the honestly-disclosed full-in-kernel-485MB / bare-metal gold-standard run, which stays the PL's).

### Relay — what to tell Oracle (BOUNCE)
- **One-line:** **The coherence fix is right and merges as soon as the proof is closed — one assertion.** The AAG bounced it on a single seam: the host test that prints "Hello! How can I help you today?" runs on a **hand-typed `PROMPT[21]` const** ("captured from QEMU serial"), and **nothing asserts the kernel's real-tokenizer boot prompt actually equals it** — that violates our byte-oracle rule (no manual transcription of exact bytes). **Fix:** add a `NEO_REAL_NEOM`-gated `frame_turn_ids(real_vocab,"hello world") == PROMPT` assertion (runs the *real* BPE, also closes the mock-encoder gap in the framing oracle), and **regenerate `PROMPT` from the kernel's raw `prompt_tokens` serial dump, not by retyping**. Optional: a `prompt.len()>max_context` guard on `sys_generate`. Re-gate is host-CI only (kernel unchanged). RETURN the green assertion + the raw-capture provenance. (The IF=0 inference-preemption item is OT-40, separate, not blocking.)
