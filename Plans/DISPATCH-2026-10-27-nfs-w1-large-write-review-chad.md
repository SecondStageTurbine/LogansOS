<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #110): post-merge review of NFS-W1, kernel NeoFS large-file writes (#167) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. It adds a bounded-memory large-file create to the kernel's NeoFS, which
defines how files over 32 KiB are laid out on disk. It takes the existing single namespace lock and has no SMP-core
change. Claude-authored (S3), so it gets a Codex-family review. **Do this after relay #107.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `fa99f551`, #167's squash on main (PR head `b38788a1`). Gate: run
`40493398124`, all jobs green.

**What it does** (feature `neofs-large-write`; off ⇒ kernel byte-identical):
- It creates a regular file of any size up to 2 GiB in one piece. Data and pointer blocks are allocated as one
  contiguous run, so each pointer follows from its position (pure `tree_logic`).
- It writes in 64 KiB runs straight to the mount's device, so through the write guard or the installer's claim, and
  bypasses the block cache.
- The content BLAKE3 is streamed. The device is flushed, then a single commit (inode, directory, superblock) runs. A
  cut before the commit leaves the old tree.
- Memory is 135,168 bytes, whatever the file size.
- `kernel write_at` stays direct-only (32 KiB). The format spec §4.3 records the large path.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/nfs-w1-review-chad
          git fetch origin && git checkout -B codex/nfs-w1-review-chad fa99f551

git show origin/main:Plans/DISPATCH-2026-10-27-nfs-w1-large-write-review-chad.md   # this file
git show origin/main:docs/specs/neofs-format.md                                     # §4.3 and the indirect-tree layout
git diff fa99f551^ fa99f551
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='neofs indirect tree large write tree_logic commit'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input.

1. **The on-disk tree matches the spec.**
   - The single- and double-indirect byte oracles are hand-laid from `neofs-format.md`, not produced by the writer.
   - The KAT that resolves every logical block up to 2 GiB uses a walk written from the kernel reader, not from
     `tree_logic`.
   - Say whether triple-indirect is reachable below 2 GiB, and tested if so.
2. **Allocation.**
   - The contiguous run is taken from the free map atomically with respect to other allocators, under the namespace
     lock.
   - A failed allocation (disk full, or no run long enough) fails cleanly, with nothing written and nothing leaked.
   - Say whether a fragmented disk with enough total free space, but no single long run, is refused. That's acceptable
     for the installer's fresh disk, but should be stated.
3. **Crash consistency.**
   - Nothing is reachable from the committed tree until the single commit.
   - A cut before the commit leaves the old tree and a consistent free map (the scenario's cut leg).
   - Say what happens to the run's blocks after a cut: they're free on remount, because the free map was never
     committed.
   - The flush precedes the commit.
4. **Through the guard.** Every data and pointer write goes to the mount's device (`GuardedDevice`), or to the
   installer's `ClaimedWriter` when the installer calls it. No path writes the inner device directly or bypasses the
   cache's coherence for blocks the cache already holds.
5. **Memory and stack.**
   - The 135,168-byte bound holds for any size.
   - The pointer blocks live in a heap buffer, not on the kernel stack (CP-74).
   - The `kernel/src/mm/heap.rs` change is explained and safe for every other heap user.
6. **The hash.** The streamed BLAKE3 equals the verified content hash the reader computes. A write whose source bytes
   change mid-stream can't commit a hash that doesn't match the disk.
7. **Host cross-check.** xtask reads the kernel-written disk with `neofs-inspect.py` and `neofs-mkfs verify`, and both
   agree with the kernel on the content.
8. **Kill-switch.** With `neofs-large-write` off, `.text` is identical to main for desktop, product and image A, and
   the 32 KiB refusal is unchanged.
9. **Prior art.** ext2/4, FreeBSD FFS, OpenZFS and RedoxFS, with licenses and dates. GPL code is read for principle
   only; nothing copied.

## Close condition

- Commit `Plans/RETURN-2026-10-27-nfs-w1-review-chad.md` on your branch and **push it**, with PASS/FAIL per item and an
  overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
