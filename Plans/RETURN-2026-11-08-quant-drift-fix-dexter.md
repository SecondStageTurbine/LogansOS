<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 358: #84 fix, the `.neom` control-run corpus window

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-08-relays-355-358.md` §358.
**Branch:** `dexter/quant-drift`. **Fix commit:** `2ca1ef3b4`, rebased onto `origin/main` @ `1fb76e014`.
**Reviewed FAIL:** `Plans/RETURN-2026-11-08-quant-drift-review-jarvis.md`, against the then-head `8f13f13d3`.
**Kernel-touching:** no — host-only harness/tooling.

## The FAIL

`ref/run-control.sh:87-94` gave the `.neom` run the full `corpus.u32` with `--corpus-tokens 2048` and the
default offset 0 (`src/main.rs:248-256`), while llama.cpp and transformers read `corpus-window.u32` from
offset 6,144 (`:58-62`). The committed `.ours.tsv` records offset 6,144, so the checked-in script could not
reproduce it.

## Fix

| Item | What changed |
|---|---|
| `.neom` run | `ref/run-control.sh:90-97` passes `--corpus-offset 6144`; the run slices `corpus.u32[6144..8192]`, the same window as the committed artifact. |
| Reference runners | `ref/llama_drift.cpp:232-242,300-318` and `ref/fp32_ref.py:54-56,70-83` accept `--corpus-offset`, slice at it, and write `corpus_window_offset` into each TSV. `run-control.sh:105-130` gives every runner the full `corpus.u32` plus the same offset, so no runner can read a different slice. |
| Guard | `src/report.rs:368-402`: `compare` returns `Err` when the runs record different `corpus_window_offset`, `corpus_tokens`, `corpus_eval_start`, `corpus_nll_count`, or a different non-empty `corpus_slice_blake3`. Unit tests at `:1132-1160`. |
| Licence | `README.md:124-129` records the pinned Salesforce/wikitext metadata's two licences, `cc-by-sa-3.0` and `gfdl` (revision `b08601e04326c79dfdd32d625aee71d232d685c3`), and notes that one prose paragraph in the same card says CC BY-SA 4.0. |

**Second finding exposed by the re-run.** The checked-in control script also passed `--verify-final 25` to the
llama.cpp runs. That diagnostic re-feeds each prompt through a fresh context and aborts on a top-1 disagreement:
on this build it stops at `desktop_help` position 17 (`batch=198`, the value the KAT pins, `final=872`). The
committed control KAT was generated without that diagnostic (its TSVs carry no `verify_final_*` rows), so the
script could not complete even after the offset fix. `run-control.sh:99-103` now leaves the diagnostic off and
records why; `llama_drift.cpp` still accepts the flag for a manual investigation.

## Re-run — it reproduces the committed numbers

Ran the fixed script end to end on the same pinned inputs, with `QUANT_DRIFT_DATE=2026-11-08T12:00:00Z` so the
KAT date matches and host `Linux 6.8.0-147-generic x86_64`. Output was kept outside the repo at
`/home/logans/quant-drift-artifacts/verify-358b/` (log `.../verify-358b.log`). The `.neom` run consumed
`corpus.u32[6144..8192]` — 2,048 tokens, 1,023 NLL targets.

- `quant-drift-kat-2026-11-08-control.ours.tsv`: **byte-identical** to the committed file (`diff -q` exit 0).
- `quant-drift-kat-2026-11-08-control.report.md`: **byte-identical**.
- KAT JSON: every numeric/verdict field equal — all three models' `perplexity`/`nll_sum`/`nll_count` and every
  `prompt_ids`/`generated_ids`/`top1_ids` row; all three `comparisons` (PPL, top-1 agreement/matches/positions,
  divergence gaps); all `bars`. The only differences are descriptive labels (`upstream Q4_K_M` vs the full HF
  path, `llama_drift.cpp` vs `ref/llama_drift.cpp`) and the `full_precision_reference_source` string.
- Control C: F16 99.7131% top-1 (695/697), ΔPPL 0.0231%, **PASS**; Q8_0 90.1004% (628/697), ΔPPL 1.4403%,
  **FAIL** (the expected quantization probe). Same numbers and verdicts; only the descriptive `candidate` label
  differs (`llama_f16_gguf` → `llama_f16_pinned`, `llama_q8_0` → `llama_q8_pinned`).
- The archived KAT predates the `generated_divergence_gaps` arrays for `llama_q4_vs_fp32` (30 rows) and
  `ours_vs_fp32` (28 rows); the re-run emits them. That is pre-existing artifact staleness, not a number change;
  `ours_vs_llama_q4`'s 14 gap rows are equal.

Key numbers unchanged: ours Q4_K_M PPL **29.112625039**, llama.cpp Q4_K_M **29.226276631** (Δ 0.3889%), F16
reference **28.841102973** (Δ 0.9414%). Bars A **FAIL** (16/30 exact streams) and B **FAIL** (66.2841% top-1,
462/697), exactly as the committed artifact records.

## Checks

- `cargo fmt --all --check`: green.
- `cargo test -p quant-drift-tests`: all unit tests pass (10/10); the model-gated KAT test skips cleanly without
  `QUANT_DRIFT_NEOM`.
- `cargo clippy -p quant-drift-tests --all-targets -- -D warnings`: green.
- `CLOSE_GATE_DOCKER=env scripts/worker-close-gate.sh`: **satisfied** on the rebased tree (`2ca1ef3b4`,
  base `1fb76e0`); full log at `/home/logans/quant-drift-artifacts/close-gate-358.log`.
- Host-only: no kernel, userland, CI-workflow or QEMU path changes.
- Push CI: not obtainable from the worker (no `gh`/token); the branch push is the evidence to open a PR against.

## Close

The `.neom` control run now consumes the same 2,048-token window as the references, `compare` rejects a
mismatched window instead of silently emitting a ΔPPL, and the fixed script reproduces the committed numbers.
Ready for the PM's merge check.
