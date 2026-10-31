<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #71): post-merge review of image A with UB checks off, and re-check of your #68 items 4–5 (#134) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. #134 has two parts:
- the product image's kernel is now built with core's UB precondition checks off (`-Zub-checks=no`), J.c.'s decision
  of 2026-10-26;
- the fix-forward of your relay #68 items 4 and 5 (MXCSR entry checks on the GDN ops and sampling, and a bounded
  perf line).

It's contained build and numeric code, Claude-authored, so it gets a Codex-family review. **Do this after relay #70.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `2c4aeb31` (main; the squash of PR #134, gated as `f5bc3bf6`, run `40262637050`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ub-checks-off-review-chad
          git fetch origin && git checkout -B codex/ub-checks-off-review-chad 2c4aeb3195b53acc6df7108b47501d5934f05937

git show origin/main:Plans/DISPATCH-2026-10-26-ub-checks-off-review-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-26-attn-lever1b-review-chad.md      # your relay #68
git diff 2818b954 2c4aeb31 -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='PRODUCT_IMAGE_UB_CHECKS_OFF NEO_IMAGE_UB_CHECKS build_product_image_kernel ub-checks banner'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete command or input and its result.

1. **Product image only.** `-Zub-checks=no` reaches only `build_product_image_kernel` (`iso --with-model` and
   `image-kernel-build`) through `CARGO_TARGET_X86_64_NEOPLUTARCH_RUSTFLAGS`. Check three things:
   - every neo-test scenario kernel, the host builds and the CI feature arms keep UB checks on;
   - the flag also reaches build-std's `core`/`alloc` (the point of the change);
   - the existing target rustflags (link args) still apply.
2. **What stays on.** `debug-assertions` and `overflow-checks` are unchanged in the image. Only core's library UB
   precondition checks go.
3. **Visible and fingerprinted.** The xtask log prints `ub-checks=off`. The kernel's boot banner prints
   `debug-assertions=… overflow-checks=… ub-checks=…` from what was actually compiled (`kernel/src/main.rs`). Check that
   the banner can't claim "off" for a kernel built with checks on, or the reverse.
4. **Kill-switch.** `NEO_IMAGE_UB_CHECKS=on` builds the image with the checks back on. Nothing else reads that variable.
5. **Your #68 item 4.** Every public GDN op (`kernel/src/ai/ops/gdn.rs`) and sampling's `fill_weights`
   (`generate_logic.rs`) check MXCSR at entry. `tools/ai-ops-tests/src/mxcsr_entry_check_tests.rs` proves it by
   behaviour, with the per-element check off as the kernel runs: 20 entry ops each panic under 0x5F80, and none do
   under 0x1F80. Removing one check fails the test. Replay your `gdn_rmsnorm_gated` probe.
6. **Your #68 item 5.** `write_perf_line` clamps shares to 0..=100 and saturates counts at 9,999,999, so any input
   gives ≤ 141 bytes. The boundary test covers the widest line, your counterexample, and every field at `u64::MAX`.

## Close condition

- Commit `Plans/RETURN-2026-10-26-ub-checks-off-review-chad.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- You may run the host tests (`cargo test -p ai-ops-tests`) and the xtask image build if you want. A FAIL is reported
  only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
