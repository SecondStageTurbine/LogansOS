# Dispatch - Phase 3 BUILD-3C NeoFS Free-Space and Quiescent GC

**Date:** 2026-05-14
**Agent:** Jarvis
**Status:** AUTHORIZED
**Authority:**
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-3b-consumption-and-3c-authorization.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md`
- `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`
- `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`

---

## 1. Mission

Implement BUILD-3 sub-commit 3C only: NeoFS physical-block free-space metadata plus quiescent mark-sweep GC.

Base:

```text
64693dede3c56bf8177f9adc94d81ef077044ee7
```

Recommended branch:

```text
codex/phase-3-build-3-3c-neofs-gc-free-space
```

## 2. Required Outcome

BUILD-2 NeoFS uses only `superblock.next_free_block`; overwritten CoW blocks become orphaned forever. 3C must make physical-block reclamation real for NeoFS images that carry the BUILD-3 free-space metadata.

Required semantics:

- add the smallest persistent free-space metadata needed to reuse reclaimed physical blocks;
- preserve all BUILD-2 mount/read/write behavior;
- keep existing BUILD-2 images mountable if practical;
- run mark-sweep only at write quiescence;
- mark all reachable blocks from the selected root;
- sweep unreachable allocated blocks into the free-space allocator;
- prove reclaimed blocks can be reused without corrupting active files;
- publish GC/free-space state crash-safely through normal NeoFS superblock selection;
- do not change `sys_fsync` behavior.

## 3. Format Guidance

The current superblock format has `next_free_block` and zero-only reserved bytes. Actual reclamation needs a format extension.

Prefer a narrow, backward-compatible extension if practical:

- an all-zero extension means legacy BUILD-2 bump-only state;
- nonzero extension identifies free-space metadata, such as a free-bitmap block or small bitmap-block range;
- free-space metadata has checksum/hash coverage;
- free-space metadata is referenced by the selected superblock and protected by superblock generation/checksum selection;
- mkfs-created BUILD-3 images initialize the metadata deterministically.

If a `format_version` bump is required, keep old BUILD-2 mkfs images mountable or file a blocker explaining why compatibility cannot be preserved.

Do not use content hashes for address lookup. This is not CAS.

## 4. GC Reachability Rules

The mark phase must treat at least these blocks as live/reserved:

- all physical blocks containing the three superblock copies present on the device;
- free-space metadata blocks;
- selected `root_inode_block`;
- root directory direct blocks up to `block_count`;
- every directory entry's child inode block;
- every reachable Regular/Model inode's direct data blocks up to `block_count`;
- reachable Directory inode direct blocks if directory recursion is supported by current format helpers;
- any static mkfs metadata blocks that are not otherwise reachable but must never be allocated.

BUILD-3C may continue to reject unsupported nonzero indirect blocks. It must not implement indirect blocks unless a blocker proves GC cannot be correct without them.

## 5. Quiescence Rule

GC must not run concurrently with writes.

Acceptable 3C entry points:

- mount-time GC before writable operations begin;
- unmount-time GC after rejecting/draining new writable operations;
- a test-only explicit GC helper that freezes the NeoFS instance and runs to completion;
- a private kernel-internal GC method called only by QEMU scenarios at a known quiescent point.

Do not add a public `sys_gc` syscall unless there is no cleaner test path. If a syscall is added, stop and get PM confirmation first.

3A kthread handles are available, but 3C does not need a background worker. Do not start 3D write-back or periodic GC worker work.

## 6. Allocation Rules

After 3C, allocation must no longer be bump-only on BUILD-3 free-space images.

Required behavior:

- live allocations are not duplicated;
- reclaimed orphan blocks can be reused;
- free-space state survives reboot;
- `next_free_block` remains meaningful as a high-water cursor or compatibility field;
- no allocated block may overlap superblock-copy blocks, free-space metadata blocks, root/inode blocks, or live data blocks;
- generation and FileCap stale-handle invariants from BUILD-2 remain intact.

## 7. Commit Ordering

For GC/free-space publication:

1. compute live set from the currently selected root;
2. compute new free-space metadata;
3. write free-space metadata blocks;
4. write all valid superblock copies for the new generation;
5. issue one block-device FLUSH after all superblock copies are written, if the mounted device supports FLUSH;
6. only then treat the GC transaction as committed in memory.

If the current shared `write_superblock_copies` helper is extended, do not accidentally change `sys_fsync` semantics or start write-back behavior. A narrow GC publication path is acceptable.

## 8. Required Runtime Scenarios

Add focused QEMU scenarios and xtask wiring. Names may vary, but cover:

1. `phase3_neofs_gc_mark_live_set`
   - mount a NeoFS image;
   - mark reachable blocks;
   - prove root inode, root directory data, child inodes, direct data blocks, superblock-copy blocks, and free-space metadata blocks are live/reserved.

2. `phase3_neofs_gc_reclaims_orphan_after_overwrite`
   - create or use a Regular file;
   - overwrite it enough to orphan old CoW blocks;
   - run quiescent GC;
   - prove old inactive blocks become free and active file content remains readable.

3. `phase3_neofs_gc_reuses_reclaimed_block`
   - after GC, perform a later allocation;
   - prove at least one reclaimed block is reused;
   - prove no live block aliasing or data corruption.

4. `phase3_neofs_gc_reboot_persistence`
   - run GC and commit free-space state;
   - reboot using the same disk image;
   - mount selects a valid superblock and observes the post-GC free-space state.

5. `phase3_neofs_gc_crash_before_publish`
   - interrupt after free-space metadata write but before superblock publication;
   - reboot must select the old valid state and not allocate from unpublished metadata.

6. `phase3_neofs_gc_corrupt_free_map_rejected`
   - corrupt the free-space metadata referenced by the newest superblock;
   - mount must reject that candidate and select an older valid state, or fail safely if no valid candidate exists.

7. `phase3_neofs_fsync_noop_write_through`
   - remains a no-op: `flush_commands_before == flush_commands_after`, no generation mutation from fsync itself.

## 9. Required Regression Gates

Run and report:

```text
cargo k
cargo k --no-default-features
cargo check -p xtask
```

If `tools/neofs-mkfs` changes, also run:

```text
cargo check -p neofs-mkfs
cargo run -p neofs-mkfs -- sanity --output-dir target\neofs-mkfs-sanity
```

Run new 3C Docker/QEMU scenarios.

Run targeted regressions:

```text
cargo neo-test phase3_virtio_blk_flush_command_smoke
cargo neo-test phase3_virtio_blk_raw_rw_smoke
cargo neo-test phase3_block_vfs_smoke
cargo neo-test phase3_neofs_create_small_file
cargo neo-test phase3_neofs_write_readback_same_boot
cargo neo-test phase3_neofs_reboot_persistence_small_file
cargo neo-test phase3_neofs_cow_write_after_write
cargo neo-test phase3_neofs_superblock_rotation_recovery
cargo neo-test phase3_neofs_filecap_generation_stale
cargo neo-test phase3_aarch64_virtio_barrier_lane
```

Because 3C touches NeoFS allocation/recovery, run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Use the established Docker path if host QEMU/Limine is blocked.

Run:

```text
git diff --check
git diff --cached --check
```

Scoped rustfmt for changed Rust files is expected. Do not claim global `cargo fmt --check` PASS unless it actually passes globally.

## 10. Verification Artifact

Create:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md
```

Include:

- exact commit SHA;
- changed files;
- free-space format summary;
- mkfs/tooling changes, if any;
- GC mark/sweep algorithm summary;
- quiescence enforcement summary;
- PASS frames for new GC/free-space scenarios;
- evidence that reclaimed blocks are reused safely;
- evidence that `sys_fsync` behavior did not change;
- targeted regression and full SMP sweep results;
- explicit statement that no write-back cache, durable fsync contract, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work landed.

Do not leave a placeholder for the commit SHA in the committed verification artifact.

## 11. Stop Conditions

Stop and file a blocker instead of committing if:

- reclamation requires a broad NeoFS migration rather than a narrow free-space metadata extension;
- old BUILD-2 images cannot remain mountable and there is no small compatibility path;
- GC can race active writes;
- the mark phase cannot account for superblock-copy blocks or free-space metadata blocks;
- a corrupt free-space map can cause allocation over live data;
- reclaimed blocks can be double-allocated;
- GC publication can make a newer superblock point at invalid free-space metadata without mount rejecting it;
- `sys_fsync` behavior changes before the durable fsync dispatch;
- implementation starts write-back cache, background GC worker, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work.

If blocked, create:

```text
Plans/BLOCKER-2026-05-14-phase-3-build-3-3c-neofs-gc-free-space-jarvis.md
```

## 12. Commit Discipline

Commit only 3C implementation and its verification or blocker artifact.

Suggested commit message:

```text
kernel/fs: add NeoFS free-space GC
```

Do not merge to `main`. Do not push. Do not start 3D.
