<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #80): post-merge review of lever 2, SwiGLU and cached attention forked across cores (#142, #143) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. Lever 2 adds two new partition kinds to the cross-CPU `par_matmul` executor
(the BSP publishes partitions, APs claim them, the BSP steals unclaimed ones). That's SMP-core code, Claude-authored
(S4), so it needs a cross-family review. **Do this after relay #79** (lever 1c; lever 2's attention runs lever 1c's
strided core per q-head range).

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `cb980631` (main; the squashes of PR #142 `55f0ed50` and PR #143 `cb980631`, merged
from the batch-1 gate `2da79dd3`, run `40363142380`, green; main's code trees equal the gated tree).

**What it does.** Under the feature `par-elementwise` (on in image A):
- SwiGLU's element-wise step forks by contiguous element range past `should_parallelize`;
- the cached decode step's attention forks by q-head range when `n_q × seq × head_dim ≥ 16,384`
  (`PAR_MIN_ATTN_WORK`). Each partition gets its own `seq_len` slice of a scratch buffer the BSP allocates up front,
  indexed by partition number.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/lever2-review-chad
          git fetch origin && git checkout -B codex/lever2-review-chad cb98063192c6db7309c0c1b163ac719c432fdf03

git show origin/main:Plans/DISPATCH-2026-10-26-lever2-review-chad.md     # this file
git show origin/main:Plans/NOTE-2026-10-25-arthur-lever2-design.md      # the design
git show origin/main:docs/concurrency-pitfalls.md                        # the pitfall bank
git diff 4f80766e cb980631 -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='par_matmul PartitionJob AttnHeads Swiglu run_job steal scratch partition'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete schedule (which CPU claims or steals which
partition, in what order) or input, and the wrong result.

1. **Disjoint outputs.** Every partition writes only its own units (`col_start * out_stride` onward, `len` values). For
   `AttnHeads`, `out_stride` is `head_dim` and the output length is `n_q_heads * head_dim`. No two partitions in flight
   write the same output value, for any part count and head count, including counts that don't divide evenly.
2. **Scratch per partition, under stealing.** Partition `t` uses scratch `t * seq_len .. (t+1) * seq_len`. A partition
   runs exactly once, whether an AP claims it or the BSP steals it, so no two CPUs use the same scratch slice at once.
   The bounds check runs before the slice is made.
3. **Lifetimes.** The query, the cache, the gate/up rows, the output and the scratch stay alive and unmoved until
   every partition is done (the BSP blocks on the barrier). No partition reads after the BSP returns, on the success
   path or on an error path (one partition returns `Err` while others are still running).
4. **Read-only inputs.** No partition writes the KV cache, the query or the gate/up rows. Nothing appends to the cache
   while a job is in flight.
5. **No allocation on an AP.** Partitions allocate nothing. The scratch is allocated by the BSP before the job is
   published.
6. **Same bits at any count.** SwiGLU and attention give the same bytes as the single-core call for thread counts 1, 2
   and 4. The kernel scenario (`neo-test-threaded-matmul-parity`), the host KATs (`swiglu_split_kat.rs`,
   `attention_heads_split_kat.rs`), and the negative control (a boundary-keyed perturbation makes a multi-partition run
   diverge) prove it. With `neo-test-par-attention-every-step`, the forward scenarios actually take the forked path
   (`par_attention_jobs > 0`).
7. **MXCSR on the APs.** The partition entry points check MXCSR (`swiglu_f32`, `gqa_causal_attention_last_strided_heads`).
   Every AP that runs a partition has the kernel's MXCSR at that point.
8. **Kill-switch.** With `par-elementwise` off, SwiGLU and attention take exactly the pre-#142 paths (the byte-view
   `ops::swiglu` and the single-core strided call). Image A lists `par-elementwise`; say whether anything else needs it.
9. **The threshold.** `should_parallelize_attention` can't overflow (`saturating_mul`), returns false when only one part
   is available, and the 16,384 figure is only a performance knob (any split gives the same bytes).
10. **Pitfall bank.** For each `docs/concurrency-pitfalls.md` class the change touches (publish/claim ordering, barrier
    completion, steal-back), say whether it's guarded and by what.

## Close condition

- Commit `Plans/RETURN-2026-10-26-lever2-review-chad.md` on your branch and **push it**, with PASS/FAIL per item and an
  overall verdict. Paste the SHA to the PL.
- You may run `cargo test -p ai-ops-tests` and a QEMU scenario if you want. A FAIL is reported only after you've checked
  it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
