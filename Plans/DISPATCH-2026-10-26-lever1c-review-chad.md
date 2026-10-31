<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #79): post-merge review of lever 1c, attention reads the KV cache in place (#140) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. Contained numeric code (one op and its call site), no SMP. Claude-authored
(S1), so it gets a Codex-family review. You reviewed lever 1b (relay #68), the same function. **Do this after relay
#75.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `4f80766e` (main; the squash of PR #140, merged from the batch-1 gate `2da79dd3`,
run `40363142380`, green).

**What it does.** The cached decode step used to copy the KV cache's active prefix out and build a full query before
calling `gqa_causal_attention_last`. It now calls `gqa_causal_attention_last_strided` on the cache's own storage
(`[kv_heads][max_context][head_dim]`, `head_stride` apart). Both entries share one core (`gqa_last_core`) over a
`Lanes` trait, so the arithmetic and summation orders are the same.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/lever1c-review-chad
          git fetch origin && git checkout -B codex/lever1c-review-chad 4f80766eaab0dac50f2403d963f712739cf43032

git show origin/main:Plans/DISPATCH-2026-10-26-lever1c-review-chad.md          # this file
git show origin/main:Plans/NOTE-2026-10-26-arthur-lever1c-kv-in-place.md      # the design
git show origin/main:Plans/RETURN-2026-10-26-attn-lever1b-review-chad.md      # your relay #68
git diff 39f561ab 4f80766e -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='gqa_causal_attention_last_strided gqa_last_core Lanes attention_last KvCache'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete shape or input and the wrong result.

1. **Same bits.** For every cache state, `attention_last` gives the same output bits as the old path (copy the active
   prefix, build the query, call `gqa_causal_attention_last`). `gqa_last_core` is one body for both entries, and the
   two `Lanes` impls read the same IEEE bits for the same stored value.
2. **Only the active prefix is read.** The strided entry reads positions `0..seq_len` of each KV head and nothing past
   `used_len`. The KAT (`tools/ai-ops-tests/src/attention_lever1c_kat.rs`) fills the unused positions with a poison
   value and checks the output doesn't change. Removing the `head_len` bound would fail it.
3. **Shape checks.** `head_len > head_stride`, a short `key` or `value`, `q_last` or `out` of the wrong length, zero
   heads, and `n_q_heads` not a multiple of `n_kv_heads` are rejected before any read. The `needed` arithmetic can't
   overflow or under-count. Give an input that reads out of bounds if you find one.
4. **The byte-path entry is unchanged.** `gqa_causal_attention_last` (used by prefill and the forward scenarios) gives
   the same bits as before #140.
5. **MXCSR.** The strided entry checks MXCSR at entry, and `mxcsr_entry_check_tests.rs` covers it (it panics under
   0x5F80, not under 0x1F80).
6. **Kill-switch.** There's no feature to turn lever 1c off; the claim is that it's a pure refactor with the same bits.
   Say whether the KAT proves that well enough to stand in for a kill-switch, or name what's missing.
7. **The perf line.** The `at=` share in `prefill_perf_logic.rs` still times the attention step.

## Close condition

- Commit `Plans/RETURN-2026-10-26-lever1c-review-chad.md` on your branch and **push it**, with PASS/FAIL per item and an
  overall verdict. Paste the SHA to the PL.
- You may run `cargo test -p ai-ops-tests` if you want. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
