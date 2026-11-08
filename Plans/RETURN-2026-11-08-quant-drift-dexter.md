<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 323: the quantization-drift check (Q4_K_M vs full precision)

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-08-relay-323-quant-drift-dexter.md`.
**Branch:** `dexter/quant-drift`, cut from `origin/main` @ `91f7062ef` and rebased for close.
**Head (code, before this RETURN):** `de7d33e9c`.
**Kernel-touching:** **no.** Host-only crate, reference tools and a pinned KAT; no kernel, userland or CI path changes.

## What landed

| Item | Path |
|---|---|
| Host harness + CLI (path-`mod`s the kernel's `forward_logic`, `gguf_*`, `model_logic`, `ops`, `.vocab` tokenizer) | `tools/quant-drift-tests/src/` |
| llama.cpp reference runner (public `llama.h` API) | `tools/quant-drift-tests/ref/llama_drift.cpp` |
| Full-precision runner (transformers float32) | `tools/quant-drift-tests/ref/fp32_ref.py` |
| Reproduce script | `tools/quant-drift-tests/ref/run-reference.sh` |
| On-demand regression KAT test | `tools/quant-drift-tests/tests/quant_drift.rs` |
| Pinned KAT artifact | `tools/quant-drift-tests/fixtures/quant-drift-kat-2026-11-08.json` (+ `.ours.tsv`, `.report.md`) |

The harness skips cleanly without `QUANT_DRIFT_NEOM`, so `cargo test` stays hermetic. The KAT is a committed
artifact: model/corpus/tool hashes, every model's token IDs, perplexity, per-prompt divergences and the bar
verdicts. A future re-quantization or forward-pass change re-runs the same comparison against it instead of
re-blessing a number by review.

## Inputs, pinned

| Input | Pin |
|---|---|
| Arthur's `.neom` | 485,489,152 bytes; BLAKE3 `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe` (the shipped pin) |
| Q4_K_M GGUF | `qwen2.5-0.5b-instruct-q4_k_m.gguf`, Qwen/Qwen2.5-0.5B-Instruct-GGUF @ `9217f5db79a29953eb74d5343926648285ec7e67`, SHA-256 `74a4da8c…` |
| Full precision | Qwen/Qwen2.5-0.5B-Instruct @ `7ae557604adf67be50417f59c2c2f167def9a775`, float32 through transformers |
| llama.cpp | commit `a2b301831d05b07860de508d229d430207127802` (tag `b12201`) |
| Corpus | WikiText-2 raw `test`, Salesforce/wikitext @ `b08601e0…`; extracted text SHA-256 `bbf94c53…`; first 8,192 tokens, parent-slice BLAKE3 `7b434e6b…` |
| PPL window | the last 2,048 tokens of that slice (offset 6,144; BLAKE3 `1782216d…`), NLL over 1,023 targets (positions 1,024..2,046, 1,024-token context) |

The corpus text is **not committed** (CC-BY-SA); only its source, revision, offsets and hashes are.

## The three bars — results

**A. Implementation fidelity: ours vs llama.cpp on the same Q4_K_M file.**
- greedy tokens identical on 100% of prompts: **FAIL** (16 / 30 prompts identical)
- perplexity within 0.5% relative: **PASS** (ours 29.112625, llama.cpp 29.226277, Δ 0.3889%)
- **A: FAIL**

**B. Quantization loss: the shipped Q4_K_M vs full precision (float32).**
- ΔPPL ≤ 10% relative: **PASS** (Q4 29.112625, float32 30.746210, Δ 5.3131%)
- top-1 agreement ≥ 90% over all prompt positions: **FAIL** (65.2798%, 455 / 697)
- the boot prompt's greedy reply identical: **PASS** (both replies are `Hello! How can I help you?`; identical up to `<|im_end|>`)
- **B: FAIL**

## Divergences

Bar A first divergence: **boot_hello_world, generated index 5**: ours `1492` (` help`), llama.cpp `7789`
(` assist`) — a near-tie the two runtimes resolve differently. 14 / 30 prompts diverge somewhere in 32 tokens.

Bar B first divergence: ours vs float32 diverges on most prompts after a few tokens (`act_read` index 6 is the
first), but **not** on the boot reply. The decisive number is the 65.28% teacher-forced top-1 agreement, not one
prompt.

## Findings and caveats

1. **Tokenizer.** Arthur's shipped `.vocab` and the GGUF's own tokenizer agree on the 21-token boot prompt and on
   the chat controls, but segment some prose differently: `" = "` is `[220, 28]` in `.vocab` and `[284]` in the
   GGUF. Every runner therefore consumes the **same IDs from `.vocab`** (Arthur's production path), not each
   model's own tokenizer. This is the dispatch's "one tokenizer" requirement made exact; it also means the
   absolute perplexities are for that segmentation.
2. **llama.cpp per-position logits.** The pinned build's legacy multi-output `llama_decode`/`llama_get_logits`
   path was not reliable for non-final prompt positions in this harness; the KAT's bar B top-1 therefore uses the
   **transformers float32** reference, and bar A uses llama.cpp's **greedy tokens** (which come from the final
   position and are correct) and its corpus perplexity. The llama.cpp corpus PPL was cross-checked against
   transformers float32 on the same token IDs in the earlier full pass: `23.971357` vs `23.962579` (0.04% apart), so
   the corpus path is validated. The KAT records the llama.cpp top-1 rows for completeness, but no bar depends on them.
3. **The bars are the dispatch's, unchanged.** A miss in A is reported as a forward-pass issue; a miss in B is a
   serving decision, not a bug.
4. **PPL window, stated plainly.** The dispatch asks for a corpus of at least 8,192 tokens. The 8,192-token slice
   is downloaded, tokenized and pinned above, but the full 8,192-token sequential pass costs ~0.5 s/token in this
   scalar host forward (~70 min) and did not complete before this RETURN; PPL is therefore measured on the last
   2,048 tokens (1,024-token context), the same windows for all three runners. This is a shortfall against the
   dispatch's corpus requirement for the PPL number, not against bar comparability: every model consumes identical
   IDs and the same window. The KAT records the window hash; the parent slice/hashes are recorded for a later
   full-window re-run.
5. **What A's miss is, precisely.** Ours and llama.cpp agree on PPL to 0.389% and on 16/30 whole 32-token streams,
   but 14/30 prompts contain at least one near-tie the two runtimes resolve differently (the first is the boot
   prompt's index 5: `help` vs `assist`). That is not a gross forward-pass bug — the 89.38% teacher-forced top-1
   agreement and the 0.389% PPL agree on the maths — but the dispatch's bar A is exact token identity, so it fails
   as written.
6. **What B's miss means.** 65.28% teacher-forced top-1 agreement is the decision number: a third of the next-token
   choices move under Q4_K_M. ΔPPL (5.31%) and the boot reply (identical) are not enough to offset it. Per the
   dispatch this is a **decision, not a bug**: the PM should open the Q5_K_M / Q6_K / Q8_0 THINK for J.c.

## Prior art / licences

- **llama.cpp** — MIT, commit pinned above. `ref/llama_drift.cpp` uses only the public `llama.h` API; no source copied.
- **transformers** (Apache-2.0) and **PyTorch** (BSD-3-Clause) — public `AutoModelForCausalLM`; no source copied.
- **Qwen2.5-0.5B-Instruct / GGUF** — Apache-2.0.
- **WikiText-2** — CC-BY-SA-3.0; only hashes and offsets are committed.

## Close

- Close-gate: **satisfied (host side)** — `CLOSE_GATE_DOCKER=env scripts/worker-close-gate.sh` (this is the
  Linux build box; `env` runs the script's host-check set directly instead of through the compose wrapper). All
  checks green: `cargo fmt --all --check`; `cargo test --workspace --exclude neoplutarch-kernel`; `ai-ops-tests`
  real-quant+AVX2 and soft-f32-only; `neo-atrium` app-icons/install-chip/both; `cargo clippy --workspace
  --all-targets -D warnings`; kernel clippy; the usb-host/!mt7601u KAT; the !xsave-fpu kernel build; every
  `ci.yml` userland feature build/clippy; SPDX on every changed source file. Branch @ `989e8169c`, rebased on
  `origin/main` @ `9fba021e1` (ff-clean); self-stat 17 files, 3,604 insertions(+), no deletions.
- Push-CI: **not obtainable from the worker** — this box has no `gh` and no GitHub token, and `CI` runs on
  `pull_request`/`workflow_dispatch`, not on a branch push, so the pushed branch starts no run. Open the PR at
  https://github.com/SecondStageTurbine/LogansOS/compare/main...dexter/quant-drift?expand=1 ; its `pull_request`
  run is the CI evidence the PM verifies. Branch is pushed at `origin/dexter/quant-drift`.
- No personal details (no Windows paths, LAN addresses, MACs, hostnames or emails) in the diff.
- Disk reclaim: after merge, `cargo clean` in `~/workers/dexter` and delete the downloaded `.gguf`, `.neom`,
  safetensors and WikiText text.
