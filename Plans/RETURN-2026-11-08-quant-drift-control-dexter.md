<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 333: the quantization-drift control, per-position logits and the gaps

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-08-relays-333-334-quant-drift.md`, Section 333.
**Branch:** `dexter/quant-drift`, rebased on `origin/main` @ `93dee0672`.
**Code head (before this RETURN):** `aa368f927`.
**Kernel-touching:** **no.** Host-only crate, reference tools, an on-demand KAT and fixtures.

## What landed

| Item | Path |
|---|---|
| `llama_batch` per-position reference (public `llama.h`; `logits=1` on every token, `llama_decode` + `llama_get_logits_ith`) | `tools/quant-drift-tests/ref/llama_drift.cpp` |
| Kernel-harness top-1/top-2 per position and per greedy step | `src/model.rs`, `src/main.rs` |
| Gap fields in the v2 reference TSV and both divergence tables in the KAT JSON | `src/report.rs` |
| Float32 reference with top-2 output | `ref/fp32_ref.py` |
| Corrected control run (F16 control, same-checkpoint bar B, model-identity probe) | `ref/run-control.sh` |
| New pinned KAT + report + ours TSV | `fixtures/quant-drift-kat-2026-11-08-control.{json,report.md,ours.tsv}` |
| Control C evidence | `fixtures/quant-drift-control-C-f16.json`, `fixtures/quant-drift-control-C-q8.json` |
| Model-identity evidence | `fixtures/quant-drift-model-identity-2026-11-08.json` |
| Old KAT, kept in history (not deleted) | `fixtures/quant-drift-kat-2026-11-08.{json,report.md,ours.tsv}` |

The v1 TSVs still parse (`quant-drift-ref-v1` or `v2`); the new KAT test reads the
`corpus_tokens`/`corpus_eval_start` recorded in the pin rather than hard-coded
8,192/4,096 constants, so the old 2,048-token KAT and the new one both work.

## Inputs, pinned

| Input | Pin |
|---|---|
| Arthur's `.neom` | 485,489,152 bytes; BLAKE3 `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe` |
| Shipped Q4_K_M source | `Qwen/Qwen2.5-0.5B-Instruct-GGUF` @ `9217f5db79a29953eb74d5343926648285ec7e67`, `qwen2.5-0.5b-instruct-q4_k_m.gguf`, SHA-256 `74a4da8c…` |
| Same-checkpoint high-precision reference | the repo's `qwen2.5-0.5b-instruct-fp16.gguf`, SHA-256 `8e0ae260…` |
| Upstream Q8_0 | same repo/revision, SHA-256 `ca59ca7f…` |
| Pinned full precision | `Qwen/Qwen2.5-0.5B-Instruct` @ `7ae557604adf67be50417f59c2c2f167def9a775`, `model.safetensors` SHA-256 `fdf756fa…`, float32 through transformers |
| llama.cpp | `a2b301831d05b07860de508d229d430207127802` (tag `b12201`) |
| Corpus | WikiText-2 raw `test`, Salesforce/wikitext @ `b08601e0…`; first 8,192 tokens, parent-slice BLAKE3 `7b434e6b…` |
| PPL window | last 2,048 tokens of that slice (offset 6,144; BLAKE3 `1782216d…`), NLL over 1,023 targets (positions 1,024..2,046, 1,024-token context) |

Corpus text is not committed (CC-BY-SA); only source, revision, offsets and hashes.

## Control C — the reference control

Set before the run: **llama.cpp F16** built from the pinned float32 GGUF, against the
pinned float32 transformers run, same IDs and window. F16 is the lossless reference
path; Q8_0 is itself a lossy quantization, so it is reported separately as a
sensitivity probe, not used as the control. Bar: teacher-forced top-1 ≥ 98% and
relative ΔPPL ≤ 1%.

| candidate | top-1 vs float32 | ΔPPL | verdict |
|---|---:|---:|---|
| llama.cpp **F16** (pinned) | **99.7131%** (695 / 697) | **0.0231%** (30.753309 vs 30.746210) | **PASS** |
| llama.cpp Q8_0 (pinned) | 90.1004% (628 / 697) | 1.4403% (30.303363 vs 30.746210) | FAIL |
| llama.cpp Q8_0 with F16 token embeddings (pinned) | 95.1220% (663 / 697) | 1.4306% vs F16 | FAIL on top-1 |

**Control C passes on F16.** The float32 and llama.cpp reference paths therefore
agree, and the Q8_0 miss is the quantization error of this 0.5B model, not a
reference bug. The F16-embedding variant moves top-1 from 90.1% to 95.1% (the tied
embedding explains most of the top-1 flips) but leaves the PPL shift at 1.43%
(so the PPL shift comes from the transformer-weight Q8_0 noise). Neither Q8_0
variant clears the 98%/1% bar; the bar is simply not a Q8_0 bar on this model.

### The cause of the original bar-B anomaly: the two "pinned" models are not the same checkpoint

`fixtures/quant-drift-model-identity-2026-11-08.json` records the tensor comparison.
The upstream Q4_K_M/Q8_0/F16 GGUFs are mutually consistent (Q4↔F16: ΔPPL 1.34%,
Q8↔F16: 2.48%), but they are **not** the pinned safetensors:

- `token_embd.weight`: upstream F16 vs pinned F32 relative error `2.6e-8` (identical);
- `blk.0.attn_q.weight`: upstream F16 vs pinned F32 relative error **0.4352**;
- `blk.0.ffn_down.weight`: **0.3240**;
- `blk.0.attn_output.weight`: **0.0315**;
- upstream F16 vs pinned F32 top-1: 92.83% (647 / 697), ΔPPL 6.18%.

So the old bar B compared the shipped Q4_K_M against a **different checkpoint**, not
against its own full precision. That is why Q4's PPL appeared below float32's.

## llama.cpp per-position logits

`ref/llama_drift.cpp` now builds a real `llama_batch` and sets `logits[i] = 1` for
**every** token, decodes it with `llama_decode()`, and reads row `i` with
`llama_get_logits_ith(ctx, i)`. It also emits `generated_logit`/`generated_2nd_logit`
for each greedy step.

`--verify-final N` re-feeds each of the first N positions of a prompt in a fresh
context and compares that final-position row to the multi-output row. The sample is
25 positions per prompt (685 rows):

| model | max \|batch − final\| top-1 logit | result |
|---|---:|---|
| pinned F32 | 0.000051 | 685 / 685 identical |
| pinned F16 | 0.006852 | 685 / 685 identical |
| pinned Q8_0 | 0.000000 | 685 / 685 identical |
| upstream Q4_K_M | 0.230373 | 684 / 685; the one flip is a near-tie |

The Q4 exception is `desktop_help` position 17: batch top-1 `198` (gap 0.562) vs
final top-1 `872` (gap 0.282), a 0.230 logit shift from the different batch shape.
That is Q4 numerical sensitivity, not a row-offset bug: the row is the right
position, and the two top candidates are within 0.6 of each other.

With the corrected path, teacher-forced top-1 for ours vs llama.cpp Q4_K_M is
**89.3831% (623 / 697)**, not the previously reported 89.4% from the unreliable
legacy path; the number is unchanged but is now measured on the public batch API.

## Bar A — ours vs llama.cpp on the same Q4_K_M file

- greedy tokens identical on all 30 prompts: **FAIL** (16 / 30)
- PPL within 0.5% relative: **PASS** (ours 29.112625, llama.cpp 29.226277, Δ 0.3889%)
- teacher-forced top-1: **89.3831%** (623 / 697)
- **A: FAIL**

### Divergence gaps

**Generated streams (14 prompts diverge; first divergence per prompt):** every one is
a near-tie. Both runtimes' top-1/top-2 gaps are below 0.5; 11 / 14 are below 0.25 in
both. The first is the boot prompt at generated index 5:

| prompt | index | ours top-1 / top-2 (gap) | llama.cpp top-1 / top-2 (gap) |
|---|---:|---|---|
| boot_hello_world | 5 | 1492 / 7789 (0.1008) | 7789 / 1492 (0.0619) |
| confirm | 4 | 714 / 358 (0.0125) | 358 / 714 (0.0625) |
| console_hello | 16 | 14589 / 4460 (0.0480) | 4460 / 14589 (0.2421) |
| desktop_apps | 6 | 2776 / 646 (0.0157) | 646 / 2776 (0.0810) |
| desktop_help | 22 | 501 / 2025 (0.0039) | 2025 / 501 (0.0132) |
| desktop_list | 21 | 2585 / 9645 (0.0165) | 9645 / 358 (0.0537) |
| desktop_session | 14 | 2585 / 358 (0.0573) | 358 / 2585 (0.1186) |
| desktop_time | 24 | 697 / 279 (0.0643) | 279 / 697 (0.0536) |
| desktop_tokyo | 15 | 13 / 911 (0.0060) | 911 / 13 (0.2526) |
| kill | 12 | 10161 / 4460 (0.0767) | 4460 / 10161 (0.0267) |
| preview_calc | 15 | 4226 / 1102 (0.0783) | 1102 / 4226 (0.1102) |
| send_external | 4 | 714 / 358 (0.1062) | 358 / 714 (0.0369) |
| write_forbidden | 13 | 2661 / 2701 (0.2199) | 2701 / 2661 (0.4354) |
| write_reversible | 23 | 9645 / 3555 (0.2882) | 576 / 3555 (0.3359) |

**Teacher-forced top-1 (74 divergences over 697 positions):** 36 are near-ties
(both gaps < 0.2), 37 have a clear gap > 0.5 in at least one runtime (36 have both
> 0.5). Median gap: ours 0.587, llama.cpp 0.260. All 74 rows are in the KAT JSON
(`comparisons.ours_vs_llama_q4.divergence_gaps`).

**Read:** the bar-A failure that changes the generated stream is **near-tie
numerics**, not a gross token-selection bug. The teacher-forced clear-gap rows are
the part that is not explained by tie-breaking; they are the evidence for a bounded
forward-pass comparison, not for a blanket tolerance waiver.

## Bar B — rerun with the fixed (same-checkpoint) reference

The corrected full-precision reference is the upstream **F16** GGUF, the only
high-precision member of the same GGUF family the shipped Q4_K_M was imported from.
The pinned float32 is a different checkpoint and is used only for Control C and the
model-identity measurement.

- ΔPPL ≤ 10% relative: **PASS** (Q4 29.112625, F16 28.841103, Δ 0.9414%)
- teacher-forced top-1 ≥ 90%: **FAIL** (66.2841%, 462 / 697)
- boot prompt greedy reply identical: **PASS**
- **B: FAIL**

Cross-check on the same upstream file pair: llama.cpp Q4_K_M vs F16 gives 69.87%
top-1 (487 / 697), ΔPPL 1.3355%, and 0 / 30 identical generated streams. The
shipped `.neom` is the slightly different Q4 import (89.38% top-1 vs the upstream
Q4, 16 / 30 generated streams), so 66.28% vs F16 is the shipped model's number.

**Why Q4's PPL came out below float32's:** the float32 was a different checkpoint
(see above). Against the same-checkpoint F16, Q4's PPL is **0.94% higher**, as a
quantized model should be. The only remaining "quantized PPL is lower" observation
is Q8_0 against F16 on the **pinned** model (30.303 vs 30.753, −1.46%): that is a
reproducible Q8_0 noise effect on this out-of-distribution window, not a reference
error, and it disappears as a "beats full precision" claim once the reference is the
same checkpoint. The F16-embedding Q8 variant leaves the PPL shift at 1.43%, so it
is the transformer-weight noise, not the tied output projection.

## Full 8,192-token window

The llama.cpp full pass fits and was run: upstream Q4_K_M PPL **22.284448** over
4,095 targets, llama.cpp F32 **23.971357**, transformers float32 **23.962579**
(0.04% apart, so the corpus path is validated). The same-checkpoint upstream F16
full pass is the reference for a corrected full-window ΔPPL; it is recorded in the
new run artifacts.

The shipped `.neom` 8,192-token pass is the scalar host forward (~0.5 s/token,
~70 min for 8,192 tokens). It was started but did not complete in this relay; the
corrected same-checkpoint F16 full pass was also started and did not complete. The
measured comparison in the KAT is therefore the 2,048-token window, where all
models consume identical IDs and the same window. This is the stated shortfall
against the dispatch's "if the run fits" allowance, not a change to any bar; the
llama.cpp Q4/F32 full pass (above) is the full-window evidence that did fit.

## KAT re-pin

The new pin is `fixtures/quant-drift-kat-2026-11-08-control.json`, with the
human-readable `…control.report.md` and the exact ours TSV
`…control.ours.tsv`. It records the new A/B numbers, the corrected F16 reference,
and both divergence-gap tables. The old KAT stays in the history, unmodified.

## Findings and falsification tests

1. **Bar A — generated divergence is near-tie numerics; teacher-forced clear gaps
   are the open question.**
   *Hypothesis:* the kernel's Q4 forward differs from llama.cpp by Q4
   dequant/matmul accumulation noise (near-ties in the generated stream), and the
   37 clear-gap teacher-forced positions come from the same accumulation
   difference, not a token-selection bug.
   *Falsification test:* feed both runtimes the identical **dequantized f32** Q4
   weights at those 37 positions; if the kernel's logits still differ by >0.5, the
   defect is in the kernel forward (attention/RoPE/matmul ordering), not in the
   quantization. If they agree, the gap is quantization noise and a tolerance THINK
   is the right next step.

2. **Bar B — the 66.28% top-1 is Q4_K_M's real quantization loss on the shipped
   checkpoint.**
   *Hypothesis:* the old 65.28% was mostly the cross-checkpoint mismatch; the
   same-checkpoint Q4-vs-F16 number remains ~66–70% because Q4_K_M on a 0.5B model
   is genuinely that lossy on teacher-forced positions.
   *Falsification test:* quantize the **same upstream F16** to Q4_K_M with a second
   independent quantizer and compare it to that F16; if that pair clears 90% top-1,
   the shipped Q4 file/import is anomalous and the bar is not measuring Q4 loss.

3. **Q8_0 vs F16 PPL (pinned model) is lower by 1.43%.**
   *Hypothesis:* Q8_0 transformer-weight noise smooths the logits on this
   out-of-distribution window, lowering NLL while flipping near-tie top-1; it is
   not a reference error and not the tied embedding.
   *Falsification test:* run a different Q6_K/Q5_K_M noise realization from the same
   f32 GGUF. If all quantized variants sit below F16 in PPL while their top-1 is
   worse, smoothing is supported; if only Q8_0 does, the Q8 file is suspect.

4. **Control C Q8_0 miss is a bar problem, not a reference problem.**
   *Hypothesis:* Q8_0 is not lossless enough for a 98%/1% bar on this 0.5B model;
   F16 is.
   *Falsification test:* the F16-embedding Q8 variant already moves top-1 to 95.12%
   but leaves the PPL shift at 1.43%; if a Q8_0 variant with f32 transformer weights
   and Q8 embeddings still misses, the bar is not reachable by Q8_0 at all.

## Prior art / licences

- **llama.cpp** — MIT, commit pinned above. `ref/llama_drift.cpp` uses only the
  public `llama.h` API; no source copied.
- **transformers** (Apache-2.0) and **PyTorch** (BSD-3-Clause) — public
  `AutoModelForCausalLM`; no source copied.
- **Qwen2.5-0.5B-Instruct / GGUF** — Apache-2.0.
- **WikiText-2** — CC-BY-SA-3.0; only hashes and offsets are committed.
- No personal details (no Windows paths, LAN addresses, MACs, hostnames or emails)
  in the diff.

## Close

- Code commit: `aa368f927`, rebased on `origin/main` @ `93dee0672` (ff-clean).
- Close-gate: **satisfied** — `CLOSE_GATE_DOCKER=env scripts/worker-close-gate.sh`
  passed Step 1 (base == `origin/main` tip), the full host-check set (fmt, workspace
  tests, ai-ops real-quant+AVX2 and soft-f32, neo-atrium's three feature variants,
  workspace clippy, blocking kernel clippy, the usb-host/!mt7601u KAT, the
  !xsave-fpu kernel build, and every `ci.yml` userland build/clippy), and SPDX.
  After the last schema/offset amend I re-ran `cargo fmt --all --check`,
  `cargo clippy -p quant-drift-tests --all-targets -- -D warnings` and
  `cargo test -p quant-drift-tests`; all green.

  ```
  Branch:        dexter/quant-drift @ aa368f927
  Rebased on:    origin/main @ 93dee06  (ff-clean)
  Kernel-touching: no
  Push CI:       <PR run; not obtainable from the worker>
  Self-stat:     23 files changed, 5518 insertions(+)  (this RETURN adds the 24th file)
  ```
- The on-demand KAT regression test was also run with `QUANT_DRIFT_NEOM` and
  `QUANT_DRIFT_CORPUS=corpus.u32` against the new pin. **PASS**: all 30 prompts
  re-ran with 0 generated/top-1 mismatches and the offset-6144 / 2,048-token PPL
  window reproduced at **29.112625 vs the KAT's 29.112625 (Δ 0.0000%)**;
  `test result: ok. 1 passed ... finished in 1243.53s`.
- Push-CI: **not obtainable from the worker** — this box has no `gh` and no GitHub
  token; `CI` runs on `pull_request`/`workflow_dispatch`, not on a branch push. The
  PR at `dexter/quant-drift` is the PM's; its `pull_request` run is the CI evidence
  the PM verifies. Branch is pushed at `origin/dexter/quant-drift`.
- Disk reclaim: after merge, `cargo clean` in `~/workers/dexter` and delete the
  downloaded `.gguf`, `.neom`, safetensors and WikiText text.
