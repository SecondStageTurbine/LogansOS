<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 342–343 (2026-11-08): quantization drift, the open bar-A gaps, the quant ladder, and a review

Read with `git fetch origin && git show origin/main:<path>`; the branch is `dexter/quant-drift` (PR #84, head
`5d2cf1f8`). The relay-333 RETURN is `Plans/RETURN-2026-11-08-quant-drift-control-dexter.md` on that branch. Bars
A and B stay as written. Push RETURNs to the branch named.

**Where it stands.** Control C passes on F16 (99.71% top-1, ΔPPL 0.02%). The original anomaly was a checkpoint
mismatch: the pinned safetensors isn't the checkpoint the upstream GGUFs were made from. Against the same-checkpoint
F16:
- Bar B: ΔPPL 0.94% PASS, top-1 66.28% FAIL, boot reply PASS.
- Bar A: every generated divergence is a near-tie, but 37 teacher-forced positions show a clear gap (> 0.5).

## Section 342: Dexter, two measurements. DeepSeek, high.
**Part 1, the 37 clear-gap positions (your falsification test 1).** Do this first; it decides whether there's a
kernel forward-pass bug.
- [ ] Feed the kernel harness and llama.cpp the identical **dequantized f32** weights from the shipped `.neom`'s Q4
  tensors (one dequantization, written once, read by both), and compare the logits at the 37 clear-gap positions.
- [ ] Bar, set now: max |Δlogit| ≤ 0.05 at all 37, and identical top-1 at all 37.
  - PASS → the gaps come from Q4 dequant/accumulation order; a tolerance THINK follows.
  - FAIL → name the first layer where the hidden states part (attention, RoPE, norm, matmul) and give the position.
- [ ] Explain why the shipped `.neom` differs from the upstream Q4_K_M (89.4% top-1 between them, 16/30 streams
  identical). Is it a different quantization of the same F16, or the same blocks read differently? Compare a few
  tensors block by block.

**Part 2, the quant ladder for J.c.'s serving decision.** All against the same-checkpoint upstream F16, same IDs and
window, llama.cpp only (no new kernel work):
- [ ] Q4_K_M, Q5_K_M, Q6_K, Q8_0, all from the same upstream repo and revision, pinned by SHA-256. For each, report
  teacher-forced top-1, ΔPPL, the boot reply identical or not, the number of identical 32-token streams out of 30, and
  the file size.
- [ ] Also report each one's top-1 over only the positions where F16's own top-1/top-2 gap exceeds 0.5. That shows
  how much of the miss is near-ties on flat WikiText distributions.
- [ ] If any of Q5/Q6/Q8 is missing upstream at that revision, say so; don't quantize your own.

RETURN: `Plans/RETURN-2026-11-08-quant-drift-ladder-dexter.md` on `dexter/quant-drift`.
Close condition: a hypothesis plus a named falsification test for anything still open.

## Section 343: Prometheus, review of the quant-drift harness. Qwen. Head `5d2cf1f8`.
This was relay 334's review (Jarvis); it moved to Prometheus because Codex isn't running and Dexter wrote the code.
It's host-only tooling, which fits Prometheus's scope. Run Section 334 of
`Plans/DISPATCH-2026-11-08-relays-333-334-quant-drift.md` as written, plus:
- [ ] Control C is computed as the dispatch defines it (F16 vs float32, ≥ 98% top-1, ≤ 1% ΔPPL), and the F16 used is
  pinned by hash. Pass/fail plus line.
- [ ] Bar B now uses the same-checkpoint F16, and the KAT records which reference each bar used. Pass/fail plus line.
- [ ] The old KAT is kept unmodified. Pass/fail.
RETURN: `Plans/RETURN-2026-11-08-quant-drift-review-prometheus.md` on `prometheus/quant-drift-review`.
