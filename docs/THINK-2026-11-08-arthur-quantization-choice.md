<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: which quantization Arthur ships (a decision for J.c.)

**Date:** 2026-11-08. **Decision owner:** J.c. **Decided: Q8_0** (J.c., 2026-11-08: "Q8, always go for the better option to make it a better experience"). **Revised 2026-11-09 (J.c., option 2):** measure Q4_K_M vs Q8_0 reply speed on the Zenbook first; Arthur stays on Q4_K_M and #90 stays held until J.c. decides on the measured numbers (host scalar measured 9.1 vs 3.7 s/token, #94). **Measured 2026-11-09 on the Zenbook** (AVX2, 16 CPUs, release `zenbook-timing-2026-11-09`, all 10 runs clean): Q4_K_M median 16.5 ms/token and 375.9 ms to first token; Q8_0 median 13.0 ms/token and 279.2 ms. Q8_0 is faster on the target, so J.c.'s original Q8_0 decision stands and #90 is un-held. Build: `Plans/DISPATCH-2026-11-08-arthur-q8.md`. **Evidence:** PR #84 (`tools/quant-drift-tests/`), the RETURNs for
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

## Addendum, 2026-11-09: the bars on the shipped Q8_0 (Seat C)

Arthur now ships the Q8_0 import (#90, `.neom` BLAKE3 `1cfd06dc…`, from upstream `qwen2.5-0.5b-instruct-q8_0.gguf`
@ `9217f5db`, SHA-256 `ca59ca7f…`). This re-runs bars A and B on that file. The bars, prompts, token IDs, window
(offset 6,144, 2,048 tokens, eval from 1,024, BLAKE3 `1782216d…`) and llama.cpp commit (`a2b30183`) are unchanged.
Control C doesn't depend on Arthur's file, so it isn't re-run. The KAT is
`tools/quant-drift-tests/fixtures/quant-drift-kat-2026-11-09-q8.json`; `ref/run-q8.sh` reproduces it.

**Reproduction checks.**
- Our `neomc` import of the pinned Q8_0 GGUF gives #90's pins exactly: `.neom` `1cfd06dc…`, vocab `e8974121…`.
- The F16 reference here has PPL 28.841103, exactly #84's figure.
- Our committed Q4_K_M run scores 66.28% top-1 against this F16 run, exactly the Q4 KAT's figure.

**The bars, as written.**

| Bar | Q4_K_M (2026-11-08 KAT) | Q8_0 (this run) |
|---|---|---|
| A: replies identical to llama.cpp on the same file | FAIL, 16 / 30 | **FAIL, 21 / 30** |
| A: PPL within 0.5% of llama.cpp | PASS, 0.39% | **PASS, 0.26%** |
| B: ΔPPL vs F16 ≤ 10% | PASS, 0.94% | **PASS, 2.21%** |
| B: top-1 vs F16 ≥ 90% | FAIL, 66.28% | **FAIL, 89.96% (627 / 697)** |
| B: boot reply identical to F16 | PASS | **PASS** |

Bar A fails the same way it did on Q4. All nine generated divergences are near-ties (both runtimes' top-two gaps below
0.5), and 33 of the 34 teacher-forced divergences are below 0.2 in both runtimes.

Bar B misses by **one position**: 627 of 697, against 628 needed.

**Why bar B misses: one shared-prefix position, counted 30 times.**
- Every prompt starts with the same chat-template prefix, and the bar counts every position of every prompt.
- At prefix index 10, F16's top two are 0.558 apart, and ours are 0.080 apart. Ours picks F16's second choice, so
  that one position is a miss in all 30 prompts.
- In all 32 of our misses where F16 is clear (gap > 0.5), F16's pick is our second choice. 30 of those 32 are this
  one position.
- Relay 342 found the same thing on Q4: 30 of its 37 rows were one shared-prefix position.

Counting each distinct prefix once (256 positions):

| | all positions | clear (F16 gap > 0.5) | distinct | distinct and clear |
|---|---:|---:|---:|---:|
| ours, Q4_K_M | 66.28% | 81.57% | 89.06% | 95.63% |
| ours, Q8_0 | 89.96% | 93.86% | **96.09%** | **98.54%** |
| llama.cpp, Q8_0 | 93.97% | 99.42% | 95.70% | 98.54% |

These extra columns explain the miss; they don't replace the bar. On distinct positions our Q8_0 is as close to F16
as llama.cpp's Q8_0 is. The two differ only where the model is near a tie.

**A correction to the table above.** Its Q8_0 row ("99.4% on clear positions") was measured with **llama.cpp**'s
Q8_0, as were all of its rows. Our kernel's Q8_0 gives 93.86% on clear positions. The gap is the prefix position
above. Counted once, both runtimes are at 98.54%.

**Speed: measured slower than the estimate.** On the same 4-core cloud host, our host forward pass took 194.8 s on
Q4_K_M and about 484 s on Q8_0 for the 21-token boot prompt with 32 generated tokens. That's 3.7 vs 9.1 s/token,
**2.5× slower for 1.38× the bytes**. The section above estimated about a third slower, from bytes alone. This is the
host harness, not the kernel on the Zenbook, so it doesn't settle reply speed there. The Zenbook measurement suggested
above is now worth taking before the next release, and the Q8_0 path (`matmul_q8_0_q8_0_with_kernel`) is the first
place to look.

**Falsification test.** The claim is that the miss is reference numerics at a near-tie, not a Q8_0 reading error.
Re-run the F16 reference with an **f32** attention cache, the cache precision relay 342 found llama.cpp differs on
(llama.cpp's `type_k`/`type_v`). If the claim holds, the reference's pick at prefix index 10 becomes ours, or its gap
there falls below 0.5, and our all-position top-1 rises above 90%. If index 10 doesn't move, the difference is in our
Q8_0 reader, and the per-class test in "Open, either way" finds it.
