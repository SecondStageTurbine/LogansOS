<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #79): lever 1c, in-place KV-cache attention

Reviewed SHA: `4f80766eaab0dac50f2403d963f712739cf43032`.
Branch: `codex/lever1c-review-chad`.
Overall verdict: **PASS**. No confirmed failure; review only, with no code changes.
RAG health: healthy (`ollama: true`, `qdrant: true`).

| Checklist | Verdict | Evidence at reviewed SHA |
|---|---|---|
| 1. Same bits | PASS | `kernel/src/ai/ops/mod.rs:421-427` and `:469-475` call the same `gqa_last_core`. The byte lane reads via `bits_at` (`:486-494`); the f32 lane uses `to_bits` (`:497-505`). The prior cache path serialized these f32 values with `to_le_bytes`, so on this x86 target both paths deliver the same IEEE u32 bits. The core keeps the same score, denominator, and value summation order (`:536-570`). The cache path supplies its original query row and cache values (`kernel/src/ai/forward_logic.rs:408-427`). The KAT agrees at the tested 0.5B shapes and contexts (`tools/ai-ops-tests/src/attention_lever1c_kat.rs:90-115`). |
| 2. Only the active prefix is read | PASS | The shared core sets `q_pos = seq_len - 1` and reads key rows, weights, and values only while `k_pos <= q_pos` (`kernel/src/ai/ops/mod.rs:526-539`, `:552-569`). The KAT fills unused cache storage with `1.0e4` and compares output to the copied-prefix path (`tools/ai-ops-tests/src/attention_lever1c_kat.rs:86-115`). Its wrong-stride falsification also passes (`:119-179`). |
| 3. Shape checks and bounds | PASS | `gqa_causal_attention_last_strided` checks nonzero parameters, head divisibility, checked `u64` conversions, checked query/head products, exact query/output lengths, and `head_len <= head_stride` before slicing (`kernel/src/ai/ops/mod.rs:447-459`). It checks `(n_kv_heads - 1) * head_stride + head_len` with checked arithmetic and verifies both key/value lengths before entering the core (`:461-468`). These bounds cover every later row offset and index: `kv_head < n_kv_heads`, each row lies within `head_len`, and the last head extent is at most `needed`. Zero dimensions and incompatible head counts return before any data read. The KAT directly exercises short K/V, wrong query size, and a short stride (`tools/ai-ops-tests/src/attention_lever1c_kat.rs:182-216`); output-size, zero-head, and nonmultiple cases are validated by the entry checks but are not separate KAT cases. I found no overflow or under-count that permits an out-of-bounds read. |
| 4. Byte-path entry | PASS | The byte entry still passes its validated query/key/value byte rows into the same core (`kernel/src/ai/ops/mod.rs:398-427`). Compared with the pre-#140 body at base `39f561ab`, the arithmetic and loop/summation order are preserved; the refactor changes row access and shares the core. |
| 5. MXCSR | PASS | The strided entry calls `debug_assert_kernel_mxcsr()` at entry (`kernel/src/ai/ops/mod.rs:439-448`). `mxcsr_entry_check_tests` includes this operator and asserts success under `0x1f80` and an MXCSR panic under `0x5f80` (`tools/ai-ops-tests/src/mxcsr_entry_check_tests.rs:174-187`, `:270-289`). Host test passed. |
| 6. Kill-switch | PASS | No runtime kill-switch is needed for this pure refactor: both entries share one arithmetic body and deliver the same stored bits to it. The KAT compares the new path against the preserved byte path in one binary. Its randomized cases do not enumerate every possible f32 bit pattern, but source-level identity of the core and lane conversions covers those patterns on x86. |
| 7. Perf line | PASS | The `at=` formatter field remains `pct(l.attn_cycles, forward)` (`kernel/src/ai/prefill_perf_logic.rs:58-74`). The counter starts before `append_rope_kv` and is recorded after `cache.attention_last` (`kernel/src/ai/forward_logic.rs:803-809`), so it still times the cached attention step. The four perf-line tests passed. |

## Validation

- `cargo test -p ai-ops-tests attention_lever1c`: 3 passed.
- `cargo test -p ai-ops-tests mxcsr_entry_check_tests`: 1 passed.
- `cargo test -p ai-ops-tests prefill_perf_line_tests`: 4 passed.
- The full `ai-ops-tests` package and end-to-end model generation regressions were not run; the review-specific host KATs and MXCSR/perf tests passed.
