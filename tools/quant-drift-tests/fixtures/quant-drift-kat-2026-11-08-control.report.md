# Quantization drift (Q4_K_M vs full-precision reference)

| model | perplexity | corpus tokens | eval targets |
|---|---:|---:|---:|
| ours_q4_k_m_neom | 29.112625 | 2048 | 1023 |
| llama_q4_k_m | 29.226277 | 2048 | 1023 |
| llama_f16_upstream | 28.841103 | 2048 | 1023 |

## Bar A — ours vs llama.cpp on the same Q4_K_M file

- greedy tokens identical on 100% of prompts: **FAIL** (16 / 30 prompts)
- perplexity within 0.5% relative: **PASS** (ours 29.112625, llama.cpp 29.226277, Δ 0.3889%)
- first divergence: **boot_hello_world** at generated index 5
- **A: FAIL**

## Bar-A divergence gaps

- Generated-stream first divergences: 14 prompt(s); every one is a near-tie (both runtimes' top-1/top-2 gaps below 0.5).
- Teacher-forced top-1 divergences: 74; 36 are near-ties (both gaps < 0.2), 37 have a clear gap > 0.5 in at least one runtime.
| prompt | gen index | ours top-1 / top-2 gap | llama.cpp top-1 / top-2 gap |
|---|---:|---:|---:|
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

## Bar B — the shipped Q4_K_M vs the full-precision reference

- ΔPPL ≤ 10% relative: **PASS** (Q4 29.112625, llama_f16_upstream 28.841103, Δ 0.9414%)
- top-1 agreement ≥ 90%: **FAIL** (66.2841%, 462 / 697)
- boot prompt greedy reply identical: **PASS**
- **B: FAIL**

## Cross-check — llama.cpp Q4_K_M vs llama_f16_upstream

- ΔPPL 1.3355% (29.226277 vs 28.841103), boot reply identical: false
## Per-prompt divergence (shipped Q4_K_M vs llama_f16_upstream)

| prompt | prompt tokens | first divergence | top-1 agreement |
|---|---:|---:|---:|
| act_read | 25 | 6 | 64.00% |
| act_write | 23 | 0 | 65.22% |
| boot_hello_world | 21 | 14 | 66.67% |
| confirm | 20 | 2 | 65.00% |
| console_hello | 22 | 14 | 68.18% |
| console_model | 26 | 6 | 69.23% |
| delete | 25 | 16 | 68.00% |
| desktop_again | 20 | 30 | 60.00% |
| desktop_apps | 22 | 17 | 68.18% |
| desktop_date | 25 | 15 | 72.00% |
| desktop_help | 20 | 4 | 65.00% |
| desktop_list | 22 | 6 | 63.64% |
| desktop_session | 21 | none | 66.67% |
| desktop_time | 24 | 24 | 70.83% |
| desktop_tokyo | 28 | 15 | 75.00% |
| kill | 27 | none | 66.67% |
| login_hi | 20 | 14 | 60.00% |
| multi_clean | 22 | 6 | 63.64% |
| multi_runaway | 21 | 6 | 61.90% |
| posture | 22 | 5 | 63.64% |
| posture_off | 23 | 6 | 65.22% |
| posture_yolo | 24 | 6 | 62.50% |
| preview_calc | 27 | 3 | 70.37% |
| preview_time | 23 | 15 | 69.57% |
| read_scratch | 23 | 10 | 65.22% |
| send_external | 23 | 4 | 69.57% |
| undo | 21 | 16 | 61.90% |
| write_danger | 29 | 0 | 68.97% |
| write_forbidden | 24 | 0 | 62.50% |
| write_reversible | 24 | 0 | 62.50% |
