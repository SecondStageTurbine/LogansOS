<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #91): post-merge review of A9 finding A, the WM-liveness overlay and guard (#151) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. Contained and test-heavy: an overlay field and a new QEMU guard with its
control. It doesn't change the SMP core. Claude-authored (S1), so it gets a Codex-family review.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `839b893e`, #151's squash on main (PR head `caed817f`). Gate: batch 3,
run `40440467911`, all 22 jobs green.

**What it answers.** A9's long answer showed `presents=0` for 12 s. S1's reading: the WM had nothing to draw. It
presents on pointer input and client commits, Arthur commits the reply once at the end, and J.c. didn't move the
pointer. This PR adds:
- `ptr in=… rx=…` to the overlay's `time` line;
- `generate_corunner_liveness`: a co-runner on CPU 0 must never wait more than 250 ms while the generate core forks
  attention across 4 CPUs;
- `generate_corunner_liveness_control`: the same run held at IF=0, which must starve CPU 0.
Gaps are measured on the TSC, not TICKS.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/a9-presents-liveness-review-janus
          git fetch origin && git checkout -B codex/a9-presents-liveness-review-janus 839b893e

git show origin/main:Plans/DISPATCH-2026-10-27-a9-presents-liveness-review-janus.md   # this file
git diff 839b893e^ 839b893e -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete schedule or input and the wrong result.

1. **The counters.** `ptr in` counts pointer reports the drivers read during a generation, and `rx` counts the
   records the WM took. Check the window: they reset at the generation's start and are read at its end. No count is
   lost or double-counted across CPUs. Taking them adds no lock the ISR path holds (I-14(a)).
2. **The diagnosis it enables.** The three readings (`in=0`; `in>0 rx=0`; `in>0 rx>0 presents=0`) map to "nothing
   moved", "the WM starved" and "the WM ran but drew nothing", with no fourth case that reads as one of them.
3. **The guard measures what it claims.**
   - The co-runner's worst gap is measured on the TSC.
   - The gate passes at 39 ms against a 250 ms limit.
   - The control (IF=0) must fail with a starvation gap.
   - Removing the IF-on preemption from the generate core would fail the guard.
   - The guard really forks attention on every step across every CPU.
4. **Production unchanged** except the overlay field. Everything new is behind `prefill-perf` or `neo-test`
   features, and image A's other behaviour is unchanged.
5. **Prior art.** The PR states which prior-art rule applies: the concurrency pitfall bank, for the IF and preemption
   classes it measures. Check that its claims hold.

## Close condition

- Commit `Plans/RETURN-2026-10-27-a9-presents-liveness-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
