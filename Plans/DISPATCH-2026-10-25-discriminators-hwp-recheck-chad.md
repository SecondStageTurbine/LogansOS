<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #61): narrow re-check of #124 (your R6 and tick-sleep notes, plus HWP in image A) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own earlier findings: R6 (relay #53, the
steal-grace timing discriminator) and the tick-sleep selftest notes (relay #56). #124 also moves `hwp-performance`
into image A by default, so that gets one short item. **Do this after relay #59.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `0ffc4e22` (main; the squash of PR #124, gated as `07c639c1`, run `40208394936`,
22/22).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/discriminators-hwp-recheck-chad
          git fetch origin && git checkout -B codex/discriminators-hwp-recheck-chad 0ffc4e227c55ca2248ebf9848337bbb33c04e5aa

git show origin/main:Plans/DISPATCH-2026-10-25-discriminators-hwp-recheck-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-steal-grace-recheck-chad.md            # your R6
git show origin/main:Plans/RETURN-2026-10-25-tick-sleep-review-chad.md              # your #56 notes
git diff 31624896 0ffc4e22 -- kernel tools .github
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='MIN_STEAL_DECISION_CYCLES past_steal_grace tick_sleep selftest DEADLINE'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input.

1. **R6, decision time.** In the grace scenario, the elapsed value recorded for a successful steal is the same value
   `past_steal_grace` decided on. It's read before the claim's CAS and before the stolen compute
   (`kernel/src/ai/par_matmul.rs`, `MIN_STEAL_DECISION_CYCLES`). The scenario fails if the smallest recorded value is
   below the expected grace.
2. **R6, independent threshold.** The scenario compares against a written-out `EXPECTED_GRACE_CYCLES` (1 GHz ×
   500 ms), not the production helper's output. It also fails `grace_threshold_mismatch` when the helper disagrees.
3. **R6, control.** `neo-test-threaded-matmul-parity-grace-time-control` makes the grace 0, so the scenario must FAIL
   `stole_before_grace`. The control is not in the gate list.
4. **Production path unchanged.** With `matmul-steal-grace-time` on and the test feature off, the steal decision is
   the same code as before #124. The test-only branch is compiled out.
5. **Tick-sleep early check.** "Early" now compares the tick the thread recorded on waking with its own published
   deadline, so a slow observer can't make it look early. The early-control (wake one tick early) must FAIL it.
6. **Tick-sleep never-woken path.** A thread that's never woken is detached instead of joined, so the selftest fails
   at once with `tick_sleep_not_parked` instead of hanging to the QEMU timeout. Check that detaching a parked thread
   leaves no dangling reference to the selftest's statics or stack, and that the no-wake control fails within the
   200-tick wait.
7. **HWP in image A.** `hwp-performance` is now in `PRODUCT_IMAGE_KERNEL_FEATURES`. The `--hwp-performance` flag is
   accepted as a no-op. CI's `product-real-lm` arm builds the same feature set as the image. Every MSR write is still
   behind its CPUID check (`kernel/src/arch/x86_64/cpufreq.rs`).

## Close condition

- Commit `Plans/RETURN-2026-10-25-discriminators-hwp-recheck-chad.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
