<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: which quantization Arthur ships (a decision for J.c.)

**Date:** 2026-11-08. **Decision owner:** J.c. **Decided: Q8_0** (J.c., 2026-11-08: "Q8, always go for the better option to make it a better experience"). **Revised 2026-11-09 (J.c., option 2):** measure Q4_K_M vs Q8_0 reply speed on the Zenbook first; Arthur stays on Q4_K_M and #90 stays held until J.c. decides on the measured numbers (host scalar measured 9.1 vs 3.7 s/token, #94). Build: `Plans/DISPATCH-2026-11-08-arthur-q8.md`. **Evidence:** PR #84 (`tools/quant-drift-tests/`), the RETURNs for
relays 323, 333 and 342 on `dexter/quant-drift`, and the KATs under `tools/quant-drift-tests/fixtures/`.

## What was asked

J.c. asked whether Arthur's 4-bit model (Qwen2.5-0.5B-Instruct, Q4_K_M) drifts from full precision, with the
acceptance bars set before running. Bar A: our forward pass matches llama.cpp on the same file. Bar B: Q4_K_M vs full
precision, ΔPPL ≤ 10%, top-1 agreement ≥ 90%, and an identical boot reply.

## What we found

1. **Our forward pass is correct.**
   - On one dequantized f32 copy of the weights, the kernel agrees with an independent float64 forward to within
     0.054 logits, with identical top-1 at every checked position.
   - llama.cpp is the one that drifts (up to 0.30), because it keeps the attention cache in 16-bit floats.
   - Bar A fails as written (16 of 30 replies identical), but every difference in the generated replies is a near-tie
     (the top two words within 0.5 logits). It's numerics, not a bug.
2. **Arthur's file is exactly upstream's Q4_K_M.** All 291 tensors are byte-identical to
   `Qwen/Qwen2.5-0.5B-Instruct-GGUF` @ `9217f5db`.
3. **Q4_K_M does lose accuracy (bar B fails on top-1).**
   - Against its own F16, perplexity is only 0.9–1.3% worse, which passes.
   - But the next word matches F16's only 66–70% of the time, against a bar of 90%.
   - A first run looked better than full precision. That was because the pinned "full precision" was a different
     checkpoint; the comparison now uses the matching F16.

## The options

Same upstream repo and revision, same 2,048-token WikiText-2 window, compared against the same-checkpoint F16
(llama.cpp). "Clear positions" means positions where F16 itself is confident (its top-two gap is above 0.5).

| Option | File size | vs today | Next-word match | On clear positions | ΔPPL | Boot reply | Replies identical (of 30) |
|---|---:|---:|---:|---:|---:|---|---:|
| **Q4_K_M** (today) | 491 MB | — | 69.9% | 80.2% | +1.3% | differs* | 0 |
| Q5_K_M | 522 MB | +6% | 79.6% | 86.8% | +0.9% | same | 5 |
| Q6_K | 650 MB | +32% | 90.2% | 93.5% | +1.3% | same | 10 |
| **Q8_0** | 676 MB | +38% | 94.0% | 99.4% | +2.5% | same | 14 |

\* On llama.cpp. On our kernel, today's Q4_K_M gives the same boot reply as F16 ("Hello! How can I help you?").

How to read the table:
- **Perplexity and next-word match disagree.** Q8_0 has the best match but the worst perplexity on this window. The
  window is encyclopedia prose, not Arthur's kind of text, so perplexity here is a weak guide. For an assistant that
  answers short commands, "picks the same word when the model is sure" (the clear-positions column) matters most.
- **Q6_K is dominated by Q8_0.** Q8_0 is only 26 MB bigger and is much closer to F16 on clear positions (99.4% vs
  93.5%). It's also the simplest format to decode.
- So the real choice is **Q4_K_M (today) vs Q8_0**, with Q5_K_M as a small-step middle.

## The costs that aren't in the table

- **Speed.** Generating each word reads every weight once, so it's limited by memory bandwidth. Q8_0 reads about 38%
  more bytes per word, so expect replies roughly a third slower. That's an estimate: it hasn't been measured on the
  Zenbook. The kernel already decodes Q8_0, Q6_K, Q5_K and Q4_K (`kernel/src/ai/gguf_dequant_logic.rs`).
- **Memory.** About 185 MB more resident. That's fine on the Zenbook; check against the smallest target we support.
- **Disk and image.** The model lives on the model volume, not in the ISO, so the size budget in #3 is unaffected.
  The model volume and the installer's copy grow by 185 MB.
- **Work.** Re-import one file (`.neom` from the upstream Q8_0), re-pin it in `kernel/src/boot_model_logic.rs`,
  re-run the boot reply and the scenarios that pin model output, and re-pin the drift KAT. That's one build and one
  gate, not a redesign.

## Recommendation

**Q8_0**, if a third slower is acceptable; otherwise keep Q4_K_M and accept the measured loss. Q8_0 gives the same
answer as full precision in 99.4% of the places where full precision is sure, against 80% for Q4_K_M.

Two ways to make the decision cheaper, if J.c. wants them first:
- a reply-speed measurement of Q4_K_M vs Q8_0 on the Zenbook's next bare-metal run (no extra run needed; it rides along);
- the same ladder on Arthur's own prompts only (the 30 in the KAT), which is closer to real use than WikiText.

## Open, either way

- The remaining difference between our Q4 reader and llama.cpp's on identical blocks is accumulation order.
  - A tolerance THINK follows: it states the bound, and pins the KAT so a future change that moves it beyond the
    bound fails.
  - Falsification test: run both readers with one tensor class at a time left quantized (Q4_K, Q5_0, Q6_K, Q8_0); the
    first class that moves a clear-gap row is the source.
- PR #84 (the harness) waits on its review. Codex is down, so Prometheus has it as relay 343.
