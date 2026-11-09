<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat F, measure Arthur's reply speed on the Zenbook: Q4_K_M vs Q8_0 (2026-11-09)

**Seat:** Seat F (cloud). **Branch:** `claude/arthur-q8-zenbook-timing` off `origin/main`.
**Decision context:** `docs/THINK-2026-11-08-arthur-quantization-choice.md`. J.c. chose option 2 on 2026-11-09: measure
on the Zenbook first. #90 (the Q8 switch) **stays held**, and Arthur ships Q4_K_M until J.c. decides on these numbers.
**Runbook:** `docs/runbook-baremetal-zenbook-s14.md` (read it via `git show origin/main:...`).

## What J.c. needs

Two numbers, measured on the Zenbook with the shipped kernel: **seconds per generated token** and **seconds to first
token**, for Q4_K_M and for Q8_0. Both are measured on the same prompts in the same boot, so the only difference is
the weights file. The host scalar path measured 9.1 vs 3.7 s/token (#94). The question is what the Zenbook's real
path gives.

## What to build

1. **A timing mode for Arthur**, off by default behind a feature that can't be built into the product image. It runs
   a fixed prompt set: 5 short commands from the 30-prompt KAT in `tools/quant-drift-tests/fixtures/`, with greedy
   decoding and a cap of 32 tokens. Each prompt runs once on each model, and the order alternates (Q4, Q8, Q8, Q4,
   ...) so a thermal or cache drift shows up instead of being hidden. For every prompt it prints one line,
   `TIMING model=<q4|q8> prompt=<n> prefill_ms=<..> tokens=<..> decode_ms=<..> ms_per_token=<..>`, and one summary
   line per model (median ms/token, median time-to-first-token).
2. **Both models on the model volume.** Use #90's Q8_0 `.neom` import (same pins, same BLAKE3) next to today's
   Q4_K_M. If the volume layout holds only one model, add a second slot for this image only, and say how in the PR.
3. **Evidence on both channels.** The fb progress console shows the summary lines, since a photo is allowed evidence
   on the Zenbook. The log sink is on the **image stick only**, and it holds every line. Under the standing rule, the
   kernel never writes the internal Windows NVMe: every block write goes through `GuardedDevice`. If the bundle
   writes a disk sink, the image boots only with the guard on.
4. **Clock source.** Use the kernel's TSC with its calibrated frequency, and print the calibrated frequency in the
   header line. A wrong calibration would scale both models equally, but the absolute number matters to J.c.

## Expected failure modes and discriminators (blind-hardware rule)

| Mode | What you'd see | Discriminator |
|---|---|---|
| Q8 doesn't fit the arena or heap | allocation failure at load | print arena/heap need vs have before loading each model |
| Second model slot not found | Q8 lines missing | print each slot's header and BLAKE3 at boot |
| Thermal throttling skews later prompts | ms/token climbs with prompt index for both models | the alternating order; print per-prompt lines, not only medians |
| SIMD path differs from QEMU's (AVX2 present on silicon) | Zenbook far faster than host scalar for both | print which matmul path each model took (AVX2 / scalar) |
| TSC calibration off | absolute times implausible | print calibrated MHz; compare with the known CPU base clock |

## Gate

- A QEMU scenario runs the timing mode on a tiny fixture model, checking the line format and the alternating order,
  not speed. It goes in the 4-shard gate, and `scripts/worker-close-gate.sh` must pass.
- Kill-switch: with the feature off, the product image is unchanged (size check or hash).
- Once green, the PM schedules the Zenbook run with J.c. He boots the stick, and the photos and log are the RETURN
  evidence. Seat F then posts the two numbers on #90 in one plain table.

## Close

Post PM: on the PR with the run URL. Commit and push at every checkpoint. The timing image is a measurement tool and
not a product change, so it merges only if the PM asks. Otherwise the branch is kept for re-runs.
