<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 342: the dequantized-f32 control and the Q4–Q8 ladder

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-08-relays-342-343-quant-drift.md`, Section 342.
**Branch:** `dexter/quant-drift`, rebased on `origin/main` @ `18c247bbf`.
**Code head (before this RETURN):** `682969fed`.
**Kernel-touching:** **no.** Host-only crate, reference tool, fixtures.

## Part 1 — the 37 clear-gap positions on dequantized f32 weights

### One dequantization, read by both

`llama-quantize --allow-requantize qwen2.5-0.5b-instruct-q4_k_m.gguf q4km-dequant-f32.gguf F32`
writes the F32 GGUF once. The kernel now reads **that GGUF** directly
(`quant-drift-tests logits --gguf-f32 …`); the `.neom` container caps at 2 GiB,
below this model's 2.5 GiB F32 payload, so the loader keeps the GGUF bytes and
points the same kernel `forward_logic` at them. llama.cpp reads the same file.
There is no second dequantizer.

| input | pin |
|---|---|
| shipped `.neom` | BLAKE3 `a32e4a1b…`, SHA-256 `b934a714…` |
| upstream Q4_K_M (same tensors as the `.neom`) | `Qwen/Qwen2.5-0.5B-Instruct-GGUF` @ `9217f5db…`, SHA-256 `74a4da8c…`, 491,400,032 B |
| dequantized F32 GGUF | SHA-256 `89ac6242…`, 2,526,617,440 B, written by the pinned llama.cpp (`a2b30183…`) |
| reference implementation | `quant-drift-tests` + the pinned llama.cpp commit `a2b30183…`, corpus window 6,144..8,191, BLAKE3 `1782216d…` |

The 37 rows are the `ours_vs_llama_q4` divergences with a > 0.5 gap in either
runtime. **30 of them are the same position** (index 7 of a prompt prefix all 30
prompts share byte-for-byte), so the control measures the 8 unique positions and
replicates the shared-prefix row. This is exact, not an approximation: a causal
forward at index 7 sees only `ids[0..=7]`, which is identical for all 30 prompts.

### The bar, as written

| reading | max \|Δlogit\| | top-1 |
|---|---:|---|
| kernel **vs llama.cpp** at the 37 rows | **0.2656** | 37 / 37 identical |
| kernel vs exact float64 reference | **0.0539** | 37 / 37 identical |
| llama.cpp vs exact float64 reference | **0.2991** | 37 / 37 identical |

The literal bar (`max |Δlogit| ≤ 0.05 **kernel vs llama.cpp**`) **FAILS**: the
largest row is `send_external` index 20 at 0.2656. But the failure is not the
kernel's. An independent float64 forward over the same F32 GGUF is 5–10× closer
to the kernel than to llama.cpp at every one of the 37 rows. The kernel is the
accurate side; the llama.cpp reference is the one that moved.

### The first layer where the hidden states part

Per-layer residual dumps (`quant-drift-tests hidden` and a diagnostic
`--dump-layer-inputs` in `llama_drift`) at `preview_time` index 20:

| hidden state | llama vs f64 | kernel vs f64 | llama vs kernel |
|---|---:|---:|---:|
| layer 0 input (token embedding) | **0.0** | **0.0** | **0.0** |
| layer 0 output / layer 1 input | **0.0304** | **1.1e-4** | 0.0303 |
| layer 1 output / layer 2 input | 0.0337 | 1.5e-4 | 0.0336 |
| … layer 23 output | 0.0491 | 5.2e-3 | 0.0453 |
| layer 24 output (final residual) | — | 1.1e-2 | — |

**First divergence: layer 0 (the first transformer block), position
`preview_time` index 20.** The layer-0 input is the token embedding and is
bit-identical. Inside layer 0, the component is the **K/V cache precision**:

- a float64 layer-0 computed with **f32 K/V** matches the kernel to **6e-11**;
- the same layer-0 computed with **f16 K/V** matches llama.cpp to **3.4e-4**;
- the f32/f64-K/V computation differs from llama.cpp by **0.0304**.

So llama.cpp is running an **f16 K/V cache** in this comparison (F32 weights,
f16 K/V), while the kernel runs f32 K/V. That is the 0.03-per-layer gap that
accumulates to 0.2–0.3 logits by layer 24. It is a reference-precision
difference, not a kernel forward-pass bug. `--kv-f32 1` on this llama.cpp build
produced a byte-identical dump, so the flag did not take effect; the f16-K/V
match above identifies the cache type from the numbers.

### Why the shipped `.neom` "differs" from the upstream Q4_K_M

It does not differ in its weights. Every one of the **291 tensor payloads in the
shipped `.neom` is byte-for-byte identical** to the upstream
`qwen2.5-0.5b-instruct-q4_k_m.gguf` (`fixtures/quant-drift-kat-2026-11-08-f32-control.json`,
`block_identity`). The mix is the Q4_K_M recipe: Q5_0 ×133, F32 ×121, Q8_0 ×13,
Q6_K ×12, Q4_K ×12. Block-by-block dequantization of representative tensors
gives the same f32 from the `.neom` block and the GGUF block, and a small
relative RMS against the same-checkpoint F16:

| tensor | block type | neom block = GGUF block | max dequant Δ | rel RMS vs F16 |
|---|---|---:|---:|---:|
| `blk.0.attn_q.weight` | Q5_0 | yes | 0.0 | 4.99% |
| `blk.0.ffn_down.weight` | Q6_K | yes | 0.0 | 2.33% |
| `blk.2.ffn_down.weight` | Q4_K | yes | 0.0 | 7.13% |
| `output.weight` | Q8_0 | yes | 0.0 | 0.65% |

**It is the same quantization of the same F16, read differently by the two Q4
runtimes.** The 89.4% top-1 / 16-of-30 streams is the kernel's Q4 reader vs
llama.cpp's Q4 reader on identical blocks — not a different file and not a
re-quantization. The f32 control above confirms the structural forward is right;
the remaining Q4 difference is the quantized
matmul/dequant accumulation path (plus llama.cpp's f16 K/V reference gap measured
above), which is the tolerance-THINK follow-up.

## Part 2 — the quant ladder for the serving decision

All four candidates are from the **same upstream repo and revision**
(`Qwen/Qwen2.5-0.5B-Instruct-GGUF` @ `9217f5db79a29953eb74d5343926648285ec7e67`),
pinned by SHA-256, and compared against the **upstream F16** at the same
revision (`8e0ae260…`, 1,266,425,696 B, PPL 28.841103). Same prompt IDs and the
same 2,048-token WikiText-2 window for every model; llama.cpp only.

| model | SHA-256 | file size | teacher-forced top-1 | ΔPPL | boot reply | identical 32-token streams | top-1 where F16 gap > 0.5 |
|---|---|---:|---:|---:|---|---:|---:|
| Q4_K_M | `74a4da8c…` | 491,400,032 B | 69.87% (487/697) | +1.34% | **no** | 0 / 30 | 80.23% (418/521) |
| Q5_K_M | `04147455…` | 522,186,592 B | 79.63% (555/697) | +0.89% | yes | 5 / 30 | 86.76% (452/521) |
| Q6_K | `2f822336…` | 650,379,104 B | 90.24% (629/697) | +1.31% | yes | 10 / 30 | 93.47% (487/521) |
| Q8_0 | `ca59ca7f…` | 675,710,816 B | 93.97% (655/697) | +2.48% | yes | 14 / 30 | 99.42% (518/521) |

All four files exist upstream at that revision; nothing was self-quantized. The
F16 reference's own top-1/top-2 gap exceeds 0.5 at **521 of 697** positions. On
that subset the top-1 misses drop sharply (Q4 69.87→80.23, Q5 79.63→86.76, Q6
90.24→93.47, Q8 93.97→99.42), so a large share of the raw agreement number is
near-ties on flat WikiText distributions.

Two non-monotonicities are real and worth the serving decision:
- **Q6_K's PPL (+1.31%) is worse than Q5_K's (+0.89%)** even though Q6's top-1 is
  10.6 points better. PPL and teacher-forced top-1 are measuring different
  things on this window; Q6 is not a free upgrade on the PPL axis.
- **Q8_0 has the worst PPL (+2.48%)** while having the best top-1. This is the
  same Q8 noise effect the relay-333 control saw on this out-of-distribution
  window; it is not a reference error and not monotonic in bits-per-weight.

**Read for J.c.:** Q8_0 is the only candidate that is close to F16 on
teacher-forced top-1 (94%) and nearly exact on clear-gap positions (99.4%), at
1.35× Q4's file size. Q5_K_M is the best PPL value but leaves 20% of
teacher-forced positions wrong. Q4_K_M is the boot-reply failure and the
weakest top-1; it is a size decision, not an accuracy-equivalent one.

## Findings and falsification tests

1. **The 37 bar-A gaps are quantization, not a kernel forward-pass bug.**
   *Evidence:* on one dequantized F32 file the kernel agrees with an independent
   float64 forward to ≤ 0.054 and identical top-1 at all 37; the Q4 top-1 gaps
   vanish when the quantized matmul is removed. *Hypothesis:* the residual Q4-vs-llama
   gap is the kernel's native Q4 dequant/accumulation order. *Falsification:* feed
   both runtimes one tensor at a time as f32 except the Q4_K/Q6_K/ffn_down
   tensors; the first tensor class whose reintroduction moves a clear-gap row is
   the source. (Named follow-up; not run here.)

2. **The f32 bar's literal failure belongs to llama.cpp, not the kernel.**
   *Evidence:* kernel-vs-f64 max 0.0539, llama-vs-f64 max 0.2991; layer 0 input
   bit-identical; an f16-K/V layer 0 matches llama.cpp to 3.4e-4 while f32/f64
   K/V matches the kernel to 6e-11. *Falsification:* rebuild/configure llama.cpp
   so its K/V cache is genuinely f32 and re-run the 37; the llama-vs-kernel max
   should fall to the kernel-vs-f64 scale (~0.05). A remaining >0.05 would then
   be a real runtime difference.

3. **The `.neom` is not a different quantization of the upstream Q4_K_M.**
   *Evidence:* all 291 tensor payloads byte-identical; block dequant identical.
   *Falsification:* hash any tensor block from both files; a mismatch would
   reopen the import path. None was found.

## Evidence and close

- Fixtures:
  `fixtures/quant-drift-kat-2026-11-08-f32-control.json` (37-row control,
  per-layer localization, f16-K/V proof, 291-tensor identity) and
  `fixtures/quant-drift-kat-2026-11-08-quant-ladder.json` (Part 2 table + pins).
- Harness: `--gguf-f32` loader, `logits`, `hidden`, `debug-layer`,
  `llama_drift --dump-logits/--dump-layer-inputs`; documented in
  `tools/quant-drift-tests/README.md`.
- Diagnostic-only llama.cpp change (outside this repo):
  `src/models/qwen2.cpp` gained `res->t_layer_inp[il] = inpL;` to expose layer
  inputs. It changes no arithmetic; the unpatched build produced the same
  logits. The layer-input API is `llama-ext.h` from the pinned checkout.
- Code commit: `682969fed`, rebased on `origin/main` @ `18c247bbf` (ff-clean).
- Close-gate: **satisfied** — `CLOSE_GATE_DOCKER=env scripts/worker-close-gate.sh`
  passed Step 1 (base == `origin/main` tip), the full host-check set (fmt,
  workspace tests, the ai-ops real-quant+AVX2 and soft-f32 variants, all three
  neo-atrium feature variants, workspace clippy, blocking kernel clippy, the
  usb/!mt7601u KATs, the !xsave-fpu kernel build, and every `ci.yml` userland
  build/clippy), and SPDX.

  ```
  Branch:        dexter/quant-drift @ 682969fed
  Rebased on:    origin/main @ 18c247b  (ff-clean)
  Kernel-touching: no
  Push CI:       <not obtainable from the worker; the PR run is the PM's>
  Self-stat:     26 files changed, 7052 insertions(+)  (this RETURN adds the 27th)
  ```
- Push CI: **not obtainable from the worker** (no `gh`, no token; `CI` runs on
  `pull_request`/`workflow_dispatch`, not a branch push). The PR on
  `dexter/quant-drift` is the PM's; its `pull_request` run is the CI evidence.
- Disk reclaim: after merge, `cargo clean` in `~/workers/dexter` and delete the
  downloaded `.gguf`/`.neom`/safetensors/WikiText artifacts.
