<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: the NeoFS commit barrier, then OT-90 THINK revision 2 (#153) → S5

**Why.** Janus reviewed your OT-90 shutdown THINK (#153, relay #93):
`Plans/RETURN-2026-10-27-ot90-power-think-review-janus.md`. It FAILs items 2, 4 and 8. Item 4 exposed a real
**NeoFS crash-consistency defect on `main`**, and the PM checked it at `origin/main`:
- `write_superblock_copies_cached` (`kernel/src/fs/neofs/inode.rs:2752ff`) rewrites **every** superblock copy with
  the new generation.
- `drain_pass` (`kernel/src/fs/neofs/cache.rs:~560–610`) writes all roles in order (Data, Inode, RootDirectory,
  FreeMap, Superblock), then issues **one** flush after the whole batch.
- A device with a volatile write cache may persist the new superblocks before the new roots. Most consumer NVMe and
  USB disks have one, and so does virtio with `cache=writeback`.
- A cut then leaves no valid generation: every old copy is gone, and the new copies point at roots that never landed.
  The volume becomes unmountable. That's worse than losing recent writes.

This is pre-existing, not a recent merge, so there's nothing to revert. But it's a data-integrity bug on every
writable NeoFS volume, so it goes **first**, ahead of the THINK revision. Take it after #174 is gated, or now if
#174 is waiting on the gate.

## PR 1 — the commit barrier (`claude/neofs-commit-barrier`)

1. **A barrier before the superblocks.** Drain every non-superblock dirty block, `flush()`, then write the
   superblocks, then `flush()` again. Any flush error or timeout before the superblocks are written stops the commit
   and keeps the old superblocks. That's OT-87's uncertain-flush rule (`docs/THINK-2026-10-27-ot87-installer.md`
   §4.7).
2. **Keep an old generation recoverable.** Either alternate the copies (write the new generation to one copy, flush,
   then the next), or show why (1) alone already guarantees that some copy always names a fully persisted root.
   Mount already picks the highest valid generation and validates its root (`mount.rs:247–307`). Check that it falls
   back to the next copy when the newest copy's root fails validation.
3. **A device without flush** (`supports_flush() == false`). Today the barrier is silently skipped and
   `flush_required` is cleared. Choose one: refuse to mount it writable, or mark it "no durability guarantee" in the
   mount log and the ledger. The default is to refuse; if you choose otherwise, say why.
4. **KATs, on a write-back-cache disk model** (the R4.3 KAT already has one; reuse it). A cut at every write and
   flush of a metadata commit (create, write, truncate), with any subset of unflushed writes persisted. Every cut
   must leave the volume mountable, at the old or the new generation, with the root valid. Show the KAT fails on
   today's single-flush order.
5. **QEMU:** the existing NeoFS scenarios stay green, and `neofs_overwrite_recovery` covers a cut between the
   barrier flush and the superblock write.
6. **Kill-switch:** none. This is a correctness fix to an ungated path. Give the `.text` delta, and the per-commit
   cost (one extra flush) on the VM.
7. **Prior art (the survey rule):** ext4 jbd2 commit-record ordering, the ZFS uberblock ring, the btrfs superblock
   flush order, and RedoxFS. Licenses and last commits, principle only, no GPL code.

## PR 2 — OT-90 THINK revision 2 (#153, docs only)

- **Item 2.** The `_S5` scan must prove the accepted `Name` is exactly the root `\_S5`, not `\_SB._S5`, and must fail
  closed on a dynamic `Method (\_S5)`. Add Janus's counterexample as a planned host KAT.
- **Item 4.** K2's durability claim now rests on PR 1. On a failed or timed-out flush, the shutdown path reports
  "unclean", not clean. Spell out the read-only-then-drain order (`fsync_regular` returns early on a read-only mount,
  `inode.rs:352–364`) with a shutdown-only drain.
- **Item 8.** Redox's date is `2026-09-26T19:58:20Z` (UTC). State the timezone convention.

## Rules

- One PR each, rebased onto `origin/main`. Run `scripts/worker-close-gate.sh`. Send the PM each head SHA from
  `git ls-remote`.
- PR 1's review after merge: Muse (security of Claude-authored storage durability), and a Codex GPT-6-Luna high read
  of the cut KAT.

```
WORKTREE: cloud session S5 (your own clone)
BRANCH:   claude/neofs-commit-barrier, then the #153 branch
          git fetch origin && git checkout -B claude/neofs-commit-barrier origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-neofs-barrier-and-ot90-think-s5.md
git show origin/main:Plans/RETURN-2026-10-27-ot90-power-think-review-janus.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
