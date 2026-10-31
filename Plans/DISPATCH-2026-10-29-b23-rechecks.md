<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RELAYS #181 → Jarvis and #182 → Janus: narrow pre-merge re-checks for batch 23

Batch 23 (`claude/pm-gate-2` @ `49828888`) gates #216, #224, #225 and #226. #216 and #226 touch concurrency code, so
each needs its original cross-family reviewer's re-check **before** merge. Fetch with
`git fetch origin +pull/216/head:pr216 +pull/226/head:pr226`.

## #181 → Jarvis: your #160 on #216 (`3ea3e987`). Slug `l2b-recheck`

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of your own findings.

Your review: `Plans/RETURN-2026-10-29-l2b-aag-jarvis.md`. Say CLOSED or STILL OPEN, with the enforcing `file:line`:
1. Items 6 and 1: is a record forwarded or received across the lock chord never delivered to the unlocked client
   (the lock filter under DELIVERY, the re-check after the pop, epoch-stamped queues emptied by `fresh()`)?
2. Item 7: does the drop clear ACTIVE before publishing LOCK=0, and does every reader load them in the matching order?
3. Item 10: does CP-97's text now match what the guard enforces?
4. Does each new scenario leg fail with its fix reverted?
5. Anything new. The known residual, the WM's blocking reply sends (#144, A1a's rework), is out of scope; note it
   only if #216 makes it worse.

Commit `Plans/RETURN-2026-10-29-l2b-recheck-jarvis.md`, push it, and paste the SHA.

## #182 → Janus: your #127 on #226 (`c29f6c28`). Slug `k1a0-grant-pin-recheck`

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of your own finding.

Your review: `Plans/RETURN-2026-10-29-k1a0-cap-graph-recheck-janus.md`. Say CLOSED or STILL OPEN, with the enforcing `file:line`:
1. Is a grant recipient's (and grantor's) last Process Arc never dropped under CAP_GRAPH (`get_pinned` lookups in
   `complete_grant` and both derives; the guard releases the lock before handing any last Arc to the deferred list)?
2. Does every reap drain the deferred drops, with interrupts on and no lock held? `reap()` is also called from the
   NeoFS block-I/O retry paths (`fs/neofs/inode.rs`, `cache.rs`): does a NeoFS lock ever cover that drain?
3. Can a pin leak, or the deferred-pending flag lose an update?
4. Does the F1b leg fail with the fix reverted?

Commit `Plans/RETURN-2026-10-29-k1a0-grant-pin-recheck-janus.md`, push it, and paste the SHA.
