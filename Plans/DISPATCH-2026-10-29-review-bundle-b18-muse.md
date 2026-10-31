<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #162–#163) → Muse: the NeoFS supersede-free fix and C2.2b's containment

Two security reviews of Claude-authored code. Both merged after batch 18 (run `40867249786`, green at `bcab0517`).
**Read the files at each squash SHA from the repo, not a summary.** The PL pastes your RETURNs back, and the PM banks
them. Write each as a plain checklist: pass or fail, the enforcing `file:line`, and a concrete input for any fail.

## #162: #206 `78991d10`, NeoFS: every commit frees what it supersedes. Banked as `Plans/RETURN-2026-10-29-neofs-frees-review-muse.md`

Product storage durability. Every commit leaked the file's old blocks, the old directory chain and the old free map,
permanently. It now frees each superseded block in its own publish, with reuse deferred under #180's rule. A
pre-merge Claude AAG found and fixed two more problems:
- C1, a non-atomic free reachable from a crafted image: blocks are now checked first, and mount refuses duplicate
  owners;
- U1, an ABA two-writer race: the generation is now checked.

Files: `kernel/src/fs/neofs/{inode.rs,alloc_logic.rs,mount.rs,cache.rs}`.
1. Can any block the last durable generation references be reallocated before a newer superblock is durable,
   including directory blocks and the old free map?
2. Can a crafted image, which mount accepts, make a commit free a live block? Check `free_superseded`'s pre-check and
   mount's duplicate-owner refusal.
3. Can a failed publish leave a live block free, or a stale `DeferredFrees` entry?
4. Is a stale writer always refused (the generation check in `commit_child_update`)?
5. Stated limits: is U2 (a read re-check) and U3 (a refused commit's reservations) honest? Both are fixed in #213,
   which is pending.

## #163: #205 `3bcc2f99`, OT-61 C2.2b, the per-account object budget: containment. Banked as `Plans/RETURN-2026-10-29-ot61-c2.2b-review-muse.md`

Behind `obj-quota` (the browser lane, in no product image). It adds a byte budget for kernel objects (handles,
channels, spawns, threads, exit codes, pins), `SYS_SPAWN_BUDGETED2` and `SYS_MEM_INFO` v2.
1. Can a budgeted process create kernel objects past its budget by any path?
2. Can a child's object budget exceed its parent's, or be created out of nothing?
3. Is each charge released exactly once (exit, kill, failed spawn, revoke)?
4. Can one process read or change another's usage through `SYS_MEM_INFO` v2?
