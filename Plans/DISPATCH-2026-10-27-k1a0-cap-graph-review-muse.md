<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #108): security review of K1a-0, closing two revoke escapes in the capability graph (#166) → Muse (Gwen)

**MODEL:** Muse (Gwen). **Why:** this fixes two pre-existing ways a revoked capability could survive, and amends
ADR-010, the capability system's foundation. Revoke is what logout and the browser's file sandbox will rely on.
Claude-authored (S4); security reviews of Claude-authored code go to Muse (CLAUDE.md).

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** a RETURN. Muse can't push, so the PL pastes or uploads it and the PM saves it as
`Plans/RETURN-2026-10-27-k1a0-cap-graph-review-muse.md`.

**Reviewed SHA:** `149d4a19` on `main`, #166's squash (PR head `7e15450c` plus a merge of main). Gate: batch 9, run
`40484657565`, all jobs green. Read the files at that SHA from the repo, not a summary:
- `kernel/src/sched/process.rs`: `revoke_handle`, `install_cap`, close and teardown, and re-parenting;
- `kernel/src/sched/cap_graph_logic.rs` and `kernel/src/sync/ticket_mutex.rs` (the `CAP_GRAPH` lock);
- `kernel/src/arch/x86_64/syscall.rs`: derive, grant, close, share, accept and fence;
- `docs/ADR-010-capability-system-foundation.md` (the amendment);
- `tools/cap-graph-logic-tests/src/lib.rs` (the edge gate);
- `userland/neo-init/src/main.rs` and `userland/neo-worker/src/main.rs` (the `cap_graph_revoke` scenario);
- `docs/concurrency-pitfalls.md` (the new CP entries).

**The two escapes it closes.**
1. **A race.** Revoke walked descendants, then removed them. A derive, grant or spawn-inherit in that gap installed a
   child that survived.
2. **Deterministic laundering.** The child→parent edge lives in the holder's own `derivations` map. Closing the
   intermediate cap, or its holder exiting, deleted that edge, so a grandchild escaped the revoke. Example: init gives
   Arthur D′ of D; Arthur derives D″; Arthur closes D′; revoking D misses D″.

**The fix.**
- One global `CAP_GRAPH` lock: a FIFO ticket lock, interrupts off.
  - Revoke holds it for the walk and the removal.
  - Every install with a parent takes it and re-checks that the parent is live, including a parent in another
    process.
  - Every close and teardown takes it, and re-parents a closing cap's children to its parent.
- Spawn registers the child before it runs.
- A source-level edge gate fails if any kernel site edits `derivations` outside an allowlisted `CAP_GRAPH` path.

## Checklist

Answer each item PASS or FAIL, with the `file:line` that enforces it. For a FAIL, give a concrete sequence of calls.

1. **The race is closed.** No derive, grant, spawn-inherit, share, accept or fence can install a child of a cap that a
   concurrent revoke will remove, on any interleaving.
2. **Laundering is closed.** Revoking an ancestor reaches every live descendant after any sequence of derive, grant,
   close and exit by intermediate holders, in any order. Re-parenting happens before the closing cap's edge is
   dropped, for both close and exit.
3. **Rights never widen.** A re-parented child's rights stay a subset of its new parent's. No path lets a holder gain a
   right it didn't have. Check the KAT for a chain of three or more closed in either order.
4. **A dead parent refuses the install.** An install whose parent was revoked, or whose holder is exiting, is refused,
   and the recipient gets nothing. Spawn either refuses or starts the child without the revoked cap; say which, and
   that it's safe.
5. **The edge gate is real.** It finds every insert, remove, take or rewrite of `derivations` in the kernel. A planted
   rogue edit fails it. Name any site it would miss: a macro, an alias, a helper in another module.
6. **The ADR-010 amendment** states the re-parenting rule precisely and matches the code. Say whether it changes any
   existing guarantee, such as survive-exit (Q4).
7. **No new denial of service.** One process's derive or close loop can't starve another's revoke indefinitely (FIFO
   ticket). A held `CAP_GRAPH` is never held across a park or an IPC wait.
8. **Test-only items stay out of the product.** The control feature `neo-test-cap-graph-unlocked-control` and its
   hooks compile only under `neo-test-*` and are absent from `PRODUCT_IMAGE_KERNEL_FEATURES`.
9. **Coverage.** All 7 `cap_graph_revoke` legs pass with the fix and all 7 leak under the control. Accept and fence are
   covered only through the shared `install_cap` path (leg 7). Say whether that's sufficient.

## Close condition

Give the RETURN to the PL with PASS/FAIL per item, `file:line`, and an overall verdict (PASS or FAIL). The PM saves it
under `Plans/`.
