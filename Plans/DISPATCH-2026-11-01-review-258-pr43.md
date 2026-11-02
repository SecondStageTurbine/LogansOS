<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 258: Chad, review of #43 (the kernel CSPRNG's RDRAND carry flag) — priority

**MODEL** GPT-6.1-Sol, **EFFORT** high: kernel entropy/CSPRNG code; an escaped defect that panics the product image
at boot.

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. `git fetch origin +pull/43/head:pr43 && git diff
origin/main...pr43` (head `718163a6` when queued; name the head). Read the PR body for the diagnosis.

1. RDRAND (and RDSEED, if used) semantics per the Intel SDM: CF=1 means a valid value; the fixed loop accepts only
   CF=1 values, retries CF=0 a bounded number of times, and never returns the zero from a failed draw.
2. The asm: the carry is read correctly (`setc` or equivalent) right after the instruction, with no instruction in
   between that changes flags; register constraints and clobbers are right.
3. The fail-closed behaviour (D6) is unchanged: when RDRAND really never succeeds, init still refuses to seed from
   jitter alone if that's the stated policy.
4. Any other RDRAND/RDSEED call site in the kernel or userland with the same inverted check (grep for them).
5. A test that would have caught it: a gate scenario booting a non-`neo-test` kernel through `Csprng::init`, or a
   host KAT of the carry-handling logic. Say whether the PR has one, and if not, what it should be.

RETURN: `Plans/RETURN-2026-11-01-pr43-rdrand-review-chad.md`, branch `codex/pr43-review-chad`.
