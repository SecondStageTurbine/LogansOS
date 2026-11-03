<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 300: Jarvis reviews the NeoFS cache lookup fix (Seat E, from #57's shard-4 timeout)

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. Seat E opens this as its own PR from branch
`claude/neofs-cache-lookup-loop` (head `4e8e6aa4` when queued; review the PR's head and name it). If the PR isn't
open yet, review the branch: `git fetch origin claude/neofs-cache-lookup-loop && git diff origin/main...FETCH_HEAD`.

## Section 300: Jarvis, the `find_cached_block` indexed loop
**MODEL** GPT-6-Luna, **EFFORT** high: one contained kernel function (NeoFS block cache), no SMP core.
1. The indexed loop returns exactly what `iter().position(...)` returned: the first match, `None` for a missing
   key, `None` for an empty cache; no off-by-one at the last entry.
2. No lock, borrow or ordering changed: the function is called under the same locks as before, and nothing else in
   the cache moved.
3. A host KAT pins items 1's cases against the old behaviour.
4. The PR body's numbers (main vs branch vs fix on `install_fresh_4k`) are consistent with the runs it cites, and it
   names the root cause (an unbounded linear scan with no eviction) as a follow-up rather than claiming it fixed.
RETURN: `Plans/RETURN-2026-11-03-neofs-cache-loop-review-jarvis.md`, branch `codex/neofs-cache-loop-review-jarvis`.
