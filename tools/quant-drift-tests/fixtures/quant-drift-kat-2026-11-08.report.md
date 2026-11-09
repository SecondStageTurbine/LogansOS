# Quantization drift (Q4_K_M vs full precision)

| model | perplexity | corpus tokens | eval targets |
|---|---:|---:|---:|
| ours_q4_k_m_neom | 29.112625 | 2048 | 1023 |
| llama_q4_k_m | 29.226277 | 2048 | 1023 |
| llama_f32_transformers | 30.746210 | 2048 | 1023 |

## Bar A — ours vs llama.cpp on the same Q4_K_M file

- greedy tokens identical on 100% of prompts: **FAIL** (16 / 30 prompts)
- perplexity within 0.5% relative: **PASS** (ours 29.112625, llama.cpp 29.226277, Δ 0.3889%)
- first divergence: **boot_hello_world** at generated index 5
- **A: FAIL**

## Bar B — the shipped Q4_K_M vs full precision (F32)

- ΔPPL ≤ 10% relative: **PASS** (Q4 29.112625, F32 30.746210, Δ 5.3131%)
- top-1 agreement ≥ 90%: **FAIL** (65.2798%, 455 / 697)
- boot prompt greedy reply identical: **PASS**
- **B: FAIL**

## Cross-check — llama.cpp Q4_K_M vs F32

- ΔPPL 4.9435% (29.226277 vs 30.746210), boot reply identical: false
## Per-prompt divergence (shipped Q4_K_M vs F32)

| prompt | prompt tokens | first divergence | top-1 agreement |
|---|---:|---:|---:|
| act_read | 25 | 6 | 64.00% |
| act_write | 23 | 0 | 65.22% |
| boot_hello_world | 21 | 31 | 66.67% |
| confirm | 20 | 4 | 60.00% |
| console_hello | 22 | 14 | 68.18% |
| console_model | 26 | 6 | 69.23% |
| delete | 25 | 16 | 68.00% |
| desktop_again | 20 | 30 | 65.00% |
| desktop_apps | 22 | 25 | 68.18% |
| desktop_date | 25 | 8 | 72.00% |
| desktop_help | 20 | 4 | 65.00% |
| desktop_list | 22 | 2 | 63.64% |
| desktop_session | 21 | 21 | 66.67% |
| desktop_time | 24 | 19 | 70.83% |
| desktop_tokyo | 28 | 15 | 75.00% |
| kill | 27 | 8 | 62.96% |
| login_hi | 20 | 14 | 55.00% |
| multi_clean | 22 | 6 | 63.64% |
| multi_runaway | 21 | 6 | 57.14% |
| posture | 22 | 5 | 63.64% |
| posture_off | 23 | 6 | 60.87% |
| posture_yolo | 24 | 0 | 58.33% |
| preview_calc | 27 | 3 | 66.67% |
| preview_time | 23 | 15 | 65.22% |
| read_scratch | 23 | 4 | 60.87% |
| send_external | 23 | 12 | 65.22% |
| undo | 21 | 4 | 66.67% |
| write_danger | 29 | 1 | 68.97% |
| write_forbidden | 24 | 0 | 66.67% |
| write_reversible | 24 | 0 | 62.50% |
