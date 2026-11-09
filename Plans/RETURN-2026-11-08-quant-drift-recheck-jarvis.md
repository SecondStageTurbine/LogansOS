<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 364: #84 corpus-window re-check

**Seat:** Jarvis. **Dispatch:** `Plans/DISPATCH-2026-11-08-relays-359-362.md`, Section 364.
**Reviewed head:** `eb28b7fb7c701e7a44e0e1f67da81f901460a9d6` (`dexter/quant-drift`).
**Fix:** relay 358, `2ca1ef3b4a418d256b9dc67840fad03615145ddd`.
**RETURN branch:** `codex/quant-drift-recheck-jarvis`, based on that exact reviewed head.
**Scope:** only the relay-334 corpus-window FAIL and the four requested re-checks; no implementation changes.

| Requested check | Verdict and evidence at the reviewed head |
|---|---|
| `.neom` gets the same window as llama.cpp and transformers | **PASS.** `tools/quant-drift-tests/ref/run-control.sh:91-97` gives `.neom` the full corpus with `--corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024`. The llama.cpp invocations use the same file and flags at lines 105-128; transformers does so at lines 131-135. The consumers actually slice at those offsets: `src/main.rs:248-270`, `ref/llama_drift.cpp:300-307`, and `ref/fp32_ref.py:70-72` (paths relative to `tools/quant-drift-tests/`). |
| Report rejects differing corpus offsets or lengths | **PASS.** `tools/quant-drift-tests/src/report.rs:373-393` returns an error before comparison when offsets, token counts, evaluation starts, or NLL target counts differ. Lines 394-402 also reject unequal nonempty slice hashes. Existing regression tests at lines 1147-1158 pass. |
| Committed script reproduces committed `.ours.tsv` window | **PASS.** Ran the unchanged script on the pinned inputs. Its fresh `work-364/ours.tsv` is byte-identical (`cmp`, exit 0) to `tools/quant-drift-tests/fixtures/quant-drift-kat-2026-11-08-control.ours.tsv`; `run-control.sh:161` copies this file to the output fixture. The resulting offset is 6,144 and length is 2,048, matching committed TSV lines 6-11. |
| WikiText licence note matches pinned repository metadata | **PASS.** `tools/quant-drift-tests/README.md:124-129` records both `cc-by-sa-3.0` and `gfdl` at revision `b08601e04326c79dfdd32d625aee71d232d685c3`, and acknowledges the card's conflicting prose version. Independently read the [pinned dataset card](https://huggingface.co/datasets/Salesforce/wikitext/raw/b08601e04326c79dfdd32d625aee71d232d685c3/README.md): its YAML `license` entries are those two identifiers (lines 8-10, one-based); its licensing paragraph refers to CC BY-SA 4.0 (line 307). |

## Reproduction evidence

The script at the reviewed head was not edited (SHA-256
`1f21776de05828a6f619db236121b37cf0c5b38a8868e4345b957db537ff9197`).
Used the local pinned files under `/home/logans/quant-drift-artifacts/`, with
`QUANT_DRIFT_DATE=2026-11-08T12:00:00Z`, fresh scratch `work-364/`, and output
`verify-364/`; the exact environment invocation is saved in `/tmp/jarvis-relay-364-run.sh`.
Log: `/home/logans/quant-drift-artifacts/verify-364.log`.
The existing Python environment was empty; a separate temporary environment supplied
`blake3`, transformers 4.57.1, PyTorch 2.9.0+cpu, and accelerate 1.15.0.

Fresh tokenization gives the same 8,192 corpus IDs and prompt IDs as the archived relay-358 run.
Independently checked that `corpus-window.u32` is exactly `corpus.u32[6144*4:8192*4]`,
2,048 little-endian u32 IDs, with BLAKE3
`1782216d7805bdc404e80dbc34f5af707e290559711d6e8dc23b5173567cecdb`.
The fresh `.neom` run finished in 1201.3 seconds with NLL sum
`3448.70888462890752635`, 1,023 targets, and PPL `29.112625039`.
Both fresh and committed `.ours.tsv` have SHA-256
`4cf1f5d6d0eae3f58f47663f222b1454a336f11134d22ce61c1a05ad4ed01587`.

The unchanged script completed end to end with exit 0. Its final output
`verify-364/quant-drift-kat-2026-11-08-control.ours.tsv` and Markdown report are both
byte-identical to their committed fixtures (`cmp`, exit 0 for each).
The pre-existing Q8 probe misses its bar and is explicitly tolerated by the script;
Control C F16 passes. Neither alters the four re-check verdicts.

## Checks and conclusion

- `bash -n tools/quant-drift-tests/ref/run-control.sh`: PASS.
- `cargo test -p quant-drift-tests`: PASS, including all 10 library tests and the offset/length rejection regressions. The model-gated integration test skips without `QUANT_DRIFT_NEOM`; the separate script run supplies the real inputs.
- Independent CLI probes changed only `corpus_window_offset` (6144 to 0), then only `corpus_tokens` (2048 to 1024), in copies of the committed TSV. Both `control` invocations exited 1 with `corpus window differs`, before emitting a comparison.
- Input SHA-256 values for `.neom`, Q4, Q8, upstream F16, safetensors and corpus text match the committed control KAT/model-identity pins. llama.cpp HEAD is the pinned `a2b301831d05b07860de508d229d430207127802`.

All four Section-364 checks pass. The relay-334 corpus-window blocker and licence-note caveat are resolved at `eb28b7fb`. This narrow re-check does not change the recorded quantization bar verdicts.
