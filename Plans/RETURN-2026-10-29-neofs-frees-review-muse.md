<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #162: #206 `78991d10`, NeoFS: every commit frees what it supersedes

Reviewed SHA: `78991d10`. Reviewer model: Muse (Gwen), reviewing Claude-authored code. Pasted back by the PL on
2026-10-29 and banked by the PM verbatim.

Scope: product storage durability, `kernel/src/fs/neofs/{inode.rs,alloc_logic.rs,mount.rs,cache.rs}` plus
`commit_logic.rs` and `format.rs`, which the listed files call for the free, the drain and the mount checks. The
pre-merge Claude AAG's C1 and U1 fixes were verified from the code, not taken on the AAG's word. All paths below are
`kernel/src/fs/neofs/` unless noted.

1. Can any block the last durable generation references be reallocated before a newer superblock is durable,
   including directory blocks and the old free map? — PASS.
   * A freed block is deferred, not freed for reuse: `free_superseded` (`inode.rs:2501`) marks the bits and calls
     `deferred.defer(block, generation)` for the publishing generation, and `DeferredFrees::release`
     (`commit_logic.rs:163`) drops only entries with `generation <= durable_generation`.
   * `durable_generation` is the lowest generation across every present superblock copy (`commit_logic.rs:127`),
     stored from the cache only on a successful flush (`cache.rs:669`), and the allocator consults it on every
     free-map allocation: `allocate_from_free_map` (`alloc_logic.rs:69`) calls `release_deferred` first and then
     refuses any block where `is_deferred` holds (`commit_logic.rs:169`).
   * The bump allocator can only hand out blocks at or past `next_free_block`, so it can never hand out a freed block
     (all freed blocks lie below it).
   * Directory blocks: `collect_directory_blocks` (`inode.rs:2476`, inode block plus direct data blocks) runs for every
     frame, root included, in `commit_directory_frames` (`inode.rs:802`), and the collected blocks are freed by
     `commit_root_entries` (`inode.rs:1253`) in the same publish.
   * The old free map: `publish_superblock` (`inode.rs:1331`) frees the previous map's blocks through
     `free_superseded` with the publishing generation.
   * The drain barrier holds in the real drain, not just the pure function: `drain_pass` (`cache.rs:619`) follows
     `next_drain_step` (`commit_logic.rs:68`) — a superblock copy is never written while a non-superblock write may
     still be volatile; a flush comes first, and a flush error stops the pass with the superblocks unwritten.
   * `neofs-leak-superseded` is test-only (`kernel/Cargo.toml:1414-1418`, implied by `neo-test-phase3-neofs-3c`); when
     it is on, nothing is freed at all — a leak, not a premature reuse.
2. Can a crafted image, which mount accepts, make a commit free a live block? — PASS. Both halves of C1 hold, verified
   independently:
   * Pre-check: `free_superseded` checks the whole list first through `check_frees` (`alloc_logic.rs:173`) — a
     duplicate block, block 0, a block past `total_blocks`, or an already-free block refuses the commit with no bit
     changed.
   * Mount duplicate-owner refusal: `collect_live_blocks` (`inode.rs:2845`) marks the superblock copy blocks and the
     free-map blocks live first, then walks the whole tree from the root (inode blocks, direct/indirect/data blocks,
     directory entries recursively, depth capped at 16) with `mark_live_once` (`alloc_logic.rs:190`), which refuses
     any block reached a second time — two inodes sharing a block, one inode naming a block twice, a directory cycle,
     or a tree block equal to a free-map or superblock block.
   * `validate_free_map_against_live_set` (`inode.rs:2799`) additionally refuses a free bit on any live block and a
     free bit at or past `next_free_block`.
   * Integrity, not just structure: the superblock carries a BLAKE3 over its first 480 bytes, checked in
     `parse_superblock` (`format.rs:123`); the root inode's hash is checked against the superblock (`mount.rs`,
     `validate_mount_candidate`); directory contents are hash-verified at mount; the highest-generation valid
     superblock wins (`mount.rs:323`, `select_superblock`).
   * The one apparent hole is closed: a volume with no free map skips the live-set walk
     (`validate_free_map_against_live_set` returns early on `None`), but `free_superseded` also frees nothing without
     a free map, and allocation there is bump-only — so the skipped walk cannot cause a free.
3. Can a failed publish leave a live block free, or a stale `DeferredFrees` entry? — PASS.
   * `commit_root_entries` (`inode.rs:1253`): on any `publish_superblock` error it calls `unfree` (`inode.rs:2530`)
     over the superseded list — bits cleared back to used and each deferred entry cancelled (`commit_logic.rs:158`) —
     then returns the error.
   * `publish_superblock` (`inode.rs:1331`): on error in the dirty-map path it unfrees the old-map blocks; the outer
     path then unfrees the superseded list as well, so both lists are undone.
   * A failed publish's cached new-map and superblock bytes may still drain, but they form a consistent newer
     generation (map written before superblock, and the barrier orders superblocks after the data they name) or
     nothing at all — never a mix. The in-memory divergence after a failure (bumped generation, new root pointer,
     `superblock.free_map` naming an unpublished location) is forward-only: the next publish overwrites it before
     writing any copy, and `on_disk` still names the old map. What a failed publish leaks (a first-attempt map
     location, reserved blocks) stays marked used — a leak, not a live block freed.
   * The create path additionally marks the whole mount failed on a commit failure (`inode.rs:394`), so a
     half-published mount commits nothing more.
4. Is a stale writer always refused (the generation check in `commit_child_update`)? — PASS.
   * `commit_child_update` (`inode.rs:768`) checks both the entry's child block and the generation read from the inode
     at that block before mutating; either mismatch returns `NotFound` and nothing is freed or published.
   * The ABA case holds: generations come from `reserve_block_and_generation` (`alloc_logic.rs:134`), a monotonic
     counter advanced with `checked_add` — exhaustion refuses the reservation (`Invalid`) instead of wrapping — so a
     later version reusing the same inode block always carries a different generation, and the stale writer's check
     fails on the generation even when the block matches.
   * All writers serialize on `entries.lock()`, held across the whole commit, and every mutation path (regular write
     at `inode.rs:564`, truncate at `:629`, create at `:213`, large-write create at `:282`) goes through
     `commit_child_update` or `commit_directory_frames`. Create needs no expected version because it refuses an
     existing name under the same lock.
5. Stated limits: is U2 (a read re-check) and U3 (a refused commit's reservations) honest? — PASS (both limits are
   real, bounded as stated, and neither is already fixed in this tree).
   * U2: `read_at` (`inode.rs:1626`) clones the record and reads ranges with no generation re-check afterwards. The
     only guard is `ensure_verified_content` (`inode.rs:1858`), a one-time-per-handle hash verification cached by
     record version — so a stale handle whose blocks were reallocated after the freeing generation went durable
     returns the new owner's bytes on a later read. That is a genuine fail-open for a racing stale reader, but it
     cannot corrupt the disk (a first read after reallocation fails closed on the content-hash mismatch), and it is
     exactly the "read re-check" gap stated. It is not fixed here.
   * U3: a refused commit's reservations leak. The write and truncate paths propagate `commit_child_update`'s `?`
     without releasing the newly reserved data blocks or inode block, and the create path's commit failure marks the
     mount failed with reservations unreleased. The leaked blocks stay marked used — never freed while live — and
     persist through the next published map; recovery needs the test-only GC sweep. Repeated refused commits can
     therefore exhaust space (a capacity DoS, not an integrity violation). That is exactly the stated limit, and it
     is not fixed here.
   * Both are stated as fixed in #213, which is pending; nothing in this tree fixes either, so the statements are
     honest about #206's scope.

Overall verdict: PASS. All 5 items pass. The U2 stale-reader window and the U3 reservation leak are real, bounded,
and honestly stated — observations carried forward to #213, not failures of this change.
