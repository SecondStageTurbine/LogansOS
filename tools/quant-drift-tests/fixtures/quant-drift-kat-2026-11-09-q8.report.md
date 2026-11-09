# Quantization drift (Q8_0 vs full-precision reference)

| model | perplexity | corpus tokens | eval targets |
|---|---:|---:|---:|
| ours_q8_0_neom | 29.479371 | 2048 | 1023 |
| llama_q8_0 | 29.555919 | 2048 | 1023 |
| llama_f16_upstream | 28.841103 | 2048 | 1023 |

## Bar A — ours vs llama.cpp on the same Q8_0 file

- greedy tokens identical on 100% of prompts: **FAIL** (21 / 30 prompts)
- perplexity within 0.5% relative: **PASS** (ours 29.479371, llama.cpp 29.555919, Δ 0.2590%)
- first divergence: **act_write** at generated index 7
- **A: FAIL**

## Bar-A divergence gaps

- Generated-stream first divergences: 9 prompt(s); every one is a near-tie (both runtimes' top-1/top-2 gaps below 0.5).
- Teacher-forced top-1 divergences: 34; 33 are near-ties (both gaps < 0.2), 1 have a clear gap > 0.5 in at least one runtime.
| prompt | gen index | ours top-1 / top-2 gap | llama.cpp top-1 / top-2 gap |
|---|---:|---:|---:|
| act_write | 7 | 311 / 304 (0.0012) | 304 / 311 (0.0181) |
| console_hello | 26 | 25 / 198 (0.5790) | 198 / 25 (0.0081) |
| desktop_date | 19 | 1492 / 7789 (0.0045) | 7789 / 1492 (0.0059) |
| desktop_list | 20 | 729 / 2025 (0.1267) | 2025 / 729 (0.0021) |
| kill | 19 | 2266 / 1995 (0.0741) | 1995 / 2266 (0.0674) |
| posture_yolo | 13 | 369 / 13 (0.0442) | 13 / 369 (0.1925) |
| undo | 28 | 358 / 2585 (0.1100) | 2585 / 358 (0.0360) |
| write_danger | 0 | 40 / 785 (0.0177) | 785 / 40 (0.1153) |
| write_reversible | 12 | 10161 / 4460 (0.0910) | 4460 / 10161 (0.0595) |

## Bar B — the shipped Q8_0 vs the full-precision reference

- ΔPPL ≤ 10% relative: **PASS** (Q8_0 29.479371, llama_f16_upstream 28.841103, Δ 2.2131%)
- top-1 agreement ≥ 90%: **FAIL** (89.9570%, 627 / 697)
- boot prompt greedy reply identical: **PASS**
- **B: FAIL**

## Cross-check — llama.cpp Q8_0 vs llama_f16_upstream

- ΔPPL 2.4785% (29.555919 vs 28.841103), boot reply identical: false
## Per-prompt divergence (shipped Q8_0 vs llama_f16_upstream)

| prompt | prompt tokens | first divergence | top-1 agreement |
|---|---:|---:|---:|
| act_read | 25 | none | 92.00% |
| act_write | 23 | 7 | 91.30% |
| boot_hello_world | 21 | 29 | 90.48% |
| confirm | 20 | none | 85.00% |
| console_hello | 22 | none | 86.36% |
| console_model | 26 | none | 92.31% |
| delete | 25 | 23 | 92.00% |
| desktop_again | 20 | none | 80.00% |
| desktop_apps | 22 | 25 | 90.91% |
| desktop_date | 25 | 19 | 88.00% |
| desktop_help | 20 | 15 | 85.00% |
| desktop_list | 22 | none | 90.91% |
| desktop_session | 21 | none | 90.48% |
| desktop_time | 24 | 27 | 91.67% |
| desktop_tokyo | 28 | none | 89.29% |
| kill | 27 | 30 | 92.59% |
| login_hi | 20 | none | 90.00% |
| multi_clean | 22 | none | 90.91% |
| multi_runaway | 21 | none | 90.48% |
| posture | 22 | 21 | 86.36% |
| posture_off | 23 | 28 | 86.96% |
| posture_yolo | 24 | none | 87.50% |
| preview_calc | 27 | none | 92.59% |
| preview_time | 23 | 27 | 91.30% |
| read_scratch | 23 | 5 | 91.30% |
| send_external | 23 | 4 | 91.30% |
| undo | 21 | 16 | 90.48% |
| write_danger | 29 | none | 93.10% |
| write_forbidden | 24 | none | 91.67% |
| write_reversible | 24 | 12 | 91.67% |
