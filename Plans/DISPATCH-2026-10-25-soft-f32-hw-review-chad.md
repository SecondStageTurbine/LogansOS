<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #66): post-merge review of hardware add/sub/mul in soft_f32 (#128, lever 1) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. This is contained numeric code (one module plus host oracles), with no SMP core
and no new syscall. It's Claude-authored, so it gets a Codex-family review. The main evidence is the byte oracle. One
item touches the kernel FP environment (CP-41), so read that entry. **Do this after relay #61.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `d47e94c5` (main; the squash of PR #128, gated as `9a5d8caf`, run `40231364150`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/soft-f32-hw-review-chad
          git fetch origin && git checkout -B codex/soft-f32-hw-review-chad d47e94c55253f18fb0f3fa5bcc2a8f41e9955fa5

git show origin/main:Plans/DISPATCH-2026-10-25-soft-f32-hw-review-chad.md   # this file
git show origin/main:Plans/NOTE-2026-10-25-arthur-non-matvec-cost.md
git diff 4a6bd93a d47e94c5 -- kernel tools
```

Read `docs/concurrency-pitfalls.md` CP-41 (the kernel's MXCSR at entry) before item 4.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='soft_f32 add mul QUIET_NAN MXCSR load_kernel_fp_control fpu_state_ptr'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input (the two `u32` bit patterns and both
outputs) or a concrete context.

1. **Byte identity, add.** For every pair of `u32` inputs, `add` on the hardware path returns the same bits as
   `add_int`: NaN inputs (any payload, either sign), infinities of the same and opposite sign, zeros of both signs
   (the `+0 + -0` adjustment returns `b`; check `-0 + +0` and `-0 + -0` too), subnormals, and results that round to
   or overflow past `f32::MAX`. Check the oracle in `tools/ai-ops-tests/src/soft_f32_hw_kat.rs` covers each of these
   classes, and say which, if any, it doesn't.
2. **Byte identity, mul and sub.** The same for `mul` (including `0 × ∞`, `-0 × x`, subnormal × large) and for `sub`
   as `add(a, b ^ SIGN)`.
3. **The oracle is real.** The KAT runs both paths in one binary through `with_integer_ops`. With the `+0/-0`
   adjustment removed, the edge KAT fails, so the switch really changes paths. The exhaustive `exp` test
   (`#[ignore]`) and the sampled one in CI cover the inputs that reach `exp`'s polynomial.
4. **The FP environment.** Every kernel caller of `soft_f32::add`/`mul`/`sub` (`ai/ops/mod.rs`, `ai/ops/gdn.rs`,
   `ai/forward_logic.rs` and anything they're called from) runs where:
   - the kernel's SSE register state is saved and restored across preemption and interrupts (the same rule the AVX2
     matmul relies on);
   - MXCSR has the kernel's control value 0x1F80 (round-to-nearest, no FTZ/DAZ), per CP-41.
   Name any caller that could run in an ISR, during early boot before the FPU is set up, or with a user's MXCSR.
   The debug assertion checks MXCSR only in debug builds; say whether the product image (opt-level 3) has any other
   guard.
5. **Kill-switch.** With `soft-f32-only`, `add`/`mul` compile to exactly `add_int`/`mul_int` (the old bodies, renamed),
   bit-for-bit the previous layer.
6. **div and sqrt.** `div` and `sqrt` still use the integer path; nothing in #128 routes them to hardware.

## Close condition

- Commit `Plans/RETURN-2026-10-25-soft-f32-hw-review-chad.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- You may run the host KATs (`cargo test -p ai-ops-tests`) if you want; no kernel build is needed. A FAIL is reported
  only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
