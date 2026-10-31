<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #68): post-merge review of lever 1b, the sw=/at= counters and the per-op MXCSR check (#130) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. This is contained numeric code: attention computes each score once, with an
overlay line and an MXCSR check moved to op entry. There's no SMP core and no new syscall. It's Claude-authored, so it
gets a Codex-family review. You reviewed lever 1 (#128, relay #66), and this builds on it. The main evidence is the byte
oracle. **Do this after relay #66** (done).

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `0e66eb59` (main; the squash of PR #130, gated as `7d250761`, run `40248528328`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/attn-lever1b-review-chad
          git fetch origin && git checkout -B codex/attn-lever1b-review-chad 0e66eb59dc81adabfbc4c5198bf8a2043b7a13ea

git show origin/main:Plans/DISPATCH-2026-10-26-attn-lever1b-review-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-soft-f32-hw-review-chad.md      # your relay #66
git show origin/main:Plans/NOTE-2026-10-25-arthur-lever2-design.md
git diff e3e6e770 0e66eb59 -- kernel tools
```

The PR body (PR #130 on GitHub) has the KAT falsification, the stack/heap census and the host split.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='gqa_causal_attention_last weights scores debug_assert_kernel_mxcsr write_perf_line'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input (shapes, position, values) and both
outputs.

1. **Attention byte identity.** `gqa_causal_attention_last` computes each Q·K score once, keeps it in the existing
   `weights` Vec, and uses it for both the max and the softmax. The V sum's indexing is hoisted. Check that no
   summation, max, or exp is reordered against the old body (`gqa_causal_attention_last_reference`, cfg(test)), at
   any position, head count or GQA grouping.
2. **The KAT is real.** `tools/ai-ops-tests/src/attention_lever1b_kat.rs` compares old and new at 0.5B shapes and
   positions 1/2/3/17/64/255/1024/2304 × 3 scales, other GQA groupings, and short-input errors. Reversing the dot's
   dim order fails it (the PR's falsification). Run it if you want: `cargo test -p ai-ops-tests attention_lever1b`.
3. **Bounds.** The `weights` Vec is sized `seq_len` (≤ 2304 here). Check that every index into it and into the K/V rows
   is in bounds at the maximum context, and that the short-input/error paths return before any slice.
4. **MXCSR at op entry.** `debug_assert_kernel_mxcsr` is called once at the entry of every op that loops soft f32 (the
   PR lists add/mul/silu/swiglu/rope_neox_yarn/both GQA attentions/matmul/softmax/gelu/layer and RMS norm, each
   forward layer and the output logits). Grep for any op that calls `soft_f32::add`/`mul`/`sub` in a loop without an
   entry check. The per-element check remains only under cfg(test).
5. **Counters.** `sw=` and `at=` are shares of the forward. `prefill_perf_logic::write_perf_line` is pure, host-tested,
   and stays within the overlay line's 192 bytes at the widest values (`prefill_perf_line_tests.rs`). The counters add
   no lock on the forward path.
6. **Kill-switch.** `soft-f32-only` still gives the integer path everywhere, and the per-op check doesn't change
   behaviour with it on.

## Close condition

- Commit `Plans/RETURN-2026-10-26-attn-lever1b-review-chad.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- No kernel build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
