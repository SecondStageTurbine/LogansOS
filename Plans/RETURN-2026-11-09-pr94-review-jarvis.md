<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #94 Q8_0 quant-drift review

**Relay:** 372 · **Reviewer:** Jarvis · **Reviewed head:** `907cc447d9dd8e06f930bff610d57fed0fd2bd70`

## Findings

- **FAIL — same prompts, IDs, window and llama.cpp commit; runner refuses unpinned inputs.** The committed Q8 and Q4 KATs agree on all ordered prompt IDs, corpus text SHA-256, window BLAKE3, window size, eval start, offset and llama.cpp commit (`tools/quant-drift-tests/fixtures/quant-drift-kat-2026-11-09-q8.json` `inputs` / `ours_q8_0_neom.prompts`; `quant-drift-kat-2026-11-08-control.json` equivalents). The runner does refuse a wrong Q8 `.neom`, upstream Q8 GGUF, or tokenized window (`ref/run-q8.sh:76-87`). But it does not refuse changed prompt IDs/text or a different llama.cpp commit: prompts are regenerated from the supplied `.vocab` at `ref/run-q8.sh:51-53`, and `LLAMA_COMMIT` defaults to the checkout's current HEAD without a comparison to the dispatch pin `a2b30183` (`:39`, passed into run metadata at `:102`). Thus the recorded run matches the pins, but the runner does not enforce all of them.
- **PASS, with evidence limitation — four-process split and merge.** The README reports three 10-prompt runs plus a window run, identical merged row order, and identical boot rows from two independent processes (`tools/quant-drift-tests/README.md:217-220`). The committed Q8 KAT has the expected 30 prompts in the Q4 order and all three Q8 result blocks share the same boot prompt IDs. The separate pre-merge TSVs are not committed, so the duplicate-process comparison itself is recorded as a report, not independently reproducible from this branch.
- **PASS — bars remain unchanged; distinct-prefix figure is supplemental.** The Q8 report keeps the existing A/B decisions: A token identity FAIL (21/30), A PPL PASS (0.2590%), B ΔPPL PASS (2.2131%), B top-1 FAIL (627/697), and boot reply PASS (`fixtures/quant-drift-kat-2026-11-09-q8.report.md:9-14,32-37`). The THINK explicitly says the additional distinct-prefix statistic does not replace the bar and reports ours at 96.09% alongside the original 89.96% result (`docs/THINK-2026-11-08-arthur-quantization-choice.md:107-126`).
- **PASS — Q8 row correction.** The addendum says the original 99.4% clear-position result was llama.cpp Q8_0's; ours was 93.86%, with both at 98.54% when counted once (`docs/THINK-2026-11-08-arthur-quantization-choice.md:128-130`).
- **PASS — speed note is a host measurement.** The 3.7 vs 9.1 s/token measurements are explicitly attributed to the same 4-core cloud host and the host harness, and are distinguished from Zenbook performance (`docs/THINK-2026-11-08-arthur-quantization-choice.md:132-136`).

## Conclusion

The Q8 analysis and reported verdicts are internally consistent with the committed KATs. The reproduction script still needs explicit prompt/ID and llama.cpp commit pin checks before its “same inputs” guarantee is enforceable for future runs.
