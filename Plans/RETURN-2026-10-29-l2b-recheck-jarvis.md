<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: L2b lock-route pre-merge re-check, relay #181 / #216

Reviewer: Jarvis. Branch: `codex/review-bundle-b23-b24-jarvis`.
Fetched `pull/216/head` as `pr216`; reviewed
`3ea3e987a4bf7670c1a0c1bc969b3c5bb6972747`, including fix `2b3e34b1`
and fmt. All implementation references below are **at that PR head**.
**CLOSED for the dispatched #160 findings**, under the clarified
linearization contract and with the coverage limits below.

1. **CLOSED — #160 items 1 and 6, under the clarified linearization
   contract.** `kernel/src/atrium/focus.rs:627–636` takes DELIVERY before
   filtering the forward target and capturing the lock epoch. Release/drop
   cannot run between that filter and enqueue. A lock engagement can still
   race the enqueue because engagement intentionally takes no lock. Such
   input cannot be received while locked: `try_recv` checks the route under
   DELIVERY (:275–278) and again after popping (:290–299). Nor can the queued
   input survive unlock: release increments `delivery.lock_epoch` (:899),
   locked-focus drop increments it (:938), and `fresh` (:955–964) drains a
   stale client queue before reading or writing it. Read paths call `fresh`
   at :287 and :330; route/flush/forward writes at :380, :470 and :658.
   DELIVERY serializes the stamp, clearing and enqueue. The WM's pointer
   route is deliberately exempt.

   This does not promise that an overlapping receive's *return instruction*
   precedes the latch: its last route check may precede the latch, and it
   then linearizes before it. `wait_ready` can similarly return a stale
   readiness result, but transfers no event and the subsequent receive is
   gated again. That is the documented resolution of the original absolute
   completion claim, not a claim to have serialized engagement with DELIVERY.
2. **CLOSED — #160 item 7 (also the composed-drop issue in item 4).**
   `focus.rs:936` clears ACTIVE with Release before the successful AcqRel
   LOCK compare-exchange (:942–944) publishes zero. An engagement racing
   an initially unlocked drop makes the CAS fail and the loop retries the
   engaged state, clearing ACTIVE before retrying publication. `closed`
   prevents repeated epoch increments. The combined reader `input_route`
   (:821–825) loads LOCK with Acquire **first**, then ACTIVE with Acquire;
   observing the clearing CAS therefore orders the prior ACTIVE clear.
   `is_active`/`receivable` and routing use this reader. The remaining direct
   LOCK readers inspect registration/admission only, not an ACTIVE fallback.
3. **CLOSED — #160 item 10.** `docs/concurrency-pitfalls.md:4249–4263`
   now distinguishes enqueue from receipt, describes the post-pop check,
   states the pre-latch linearization point, permits stale readiness, and
   explains why a queue spanning the lock is emptied. The summary row at
   :142 agrees. The guard enforces these revised claims.
4. **CLOSED — each new leg fails with its protecting fix disabled.**
   Independently ran `NEO_SMP=4 cargo neo-test lock_route_races --no-meta`
   in Docker, QEMU 10.2.3. The unmodified head passed all four legs.
   The legs and hit-count assertions start at
   `kernel/src/test_harness.rs:25988`. Then executed these isolated controls:

   | Leg / mutation | Observed failure |
   |---|---|
   | Receive: remove post-pop route re-check | `lock_race_recv_returned_key_after_latch` (:26030) |
   | Forward: make `fresh` skip stale-queue clearing | `lock_race_forward_record_crossed_lock` (:26073) |
   | Queued key: same disabled clearing; bypass only the earlier forward assertion after it consumes the stale record, to reach this leg | `lock_race_queued_key_crossed_lock` (:26089) |
   | Drop: publish LOCK=0 before clearing ACTIVE, with the route-read hook between those stores | `lock_race_drop_reopened_client_route` (:26107) |

   All booted four CPUs and failed via the kernel's explicit FAIL/debug-exit
   path. The receive control encountered the runner's default four attempts
   (all failed at the same witness); the remaining controls used
   `NEO_TEST_ATTEMPTS=1`. Original source bytes were restored after each
   control, and the isolated worktree is clean. The existing hooks do not
   separately discriminate moving the forward filter back outside DELIVERY
   while keeping the epoch fix: the hook is already inside the lock. That
   ordering was source-reviewed; the scenario is not evidence for that
   individual mutation.
5. **No new blocking implementation finding in the fix diff.** The WM's
   blocking reply sends (#144 / A1a) are unchanged; #216 removes their
   control over the input lock and does not make that residual worse.

Executed host validation: `cargo test -p atrium-logic-tests --locked`,
144 passed. The QEMU race scenario deliberately injects the relevant
interleavings on one CPU even with four CPUs booted; it is not a stress
test proving every real-CPU ordering. RAG was healthy; semantic discovery
returned no useful match for the later scheduler query, so pinned source
was read directly. No implementation changes are included in this RETURN.
