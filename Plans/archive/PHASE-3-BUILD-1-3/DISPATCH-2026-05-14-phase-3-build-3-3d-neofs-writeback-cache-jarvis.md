# Dispatch - Phase 3 BUILD-3D NeoFS Write-Back Cache

**Date:** 2026-05-14
**Agent:** Jarvis
**Status:** AUTHORIZED
**Authority:**
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-3c-consumption-and-3d-authorization.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md`
- `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`

---

## 1. Mission

Implement BUILD-3 sub-commit 3D only: a NeoFS write-back buffer cache owned by the mounted NeoFS instance and drained by a 3A `KthreadHandle` worker.

Base:

```text
67913b7595bdd8c2666b93261d5e64d78bce870e
```

Recommended branch:

```text
codex/phase-3-build-3-3d-neofs-writeback-cache
```

## 2. Required Outcome

3D introduces mount-owned cached FS-block state and a background write-back worker, without changing the `sys_fsync` contract.

Required semantics:

- cache NeoFS filesystem blocks at the 4 KiB FS-block layer;
- expose cache hit/miss and dirty/clean state for tests;
- dirty writes are tracked by block role where practical: data, inode, root-directory, free-map, and superblock publication;
- a NeoFS-owned write-back kthread drains dirty state at clean checkpoints;
- write-back uses existing 3B FLUSH and 3C free-map/superblock publication ordering where required;
- mount teardown/test drain must stop and join the worker before dropping mount-owned state;
- GC must run only when the cache is clean or after a full drain;
- existing BUILD-2/3C read/write/reboot behavior remains correct after background drain;
- crashes before drain may lose uncommitted dirty writes but must not create torn mount state.

## 3. Insertion Guidance

Likely insertion points:

- `kernel/src/fs/neofs/inode.rs` `read_fs_block`;
- `kernel/src/fs/neofs/inode.rs` `write_fs_block`;
- `NeoFsState` mount-owned state;
- `commit_regular_write` and free-map/superblock publication helpers;
- mount/test helper paths in `kernel/src/fs/neofs/mount.rs`;
- 3A `kernel/src/sched/kthread.rs` public API.

Prefer a small `kernel/src/fs/neofs/cache.rs` module if it keeps `inode.rs` from growing further. Follow local style.

Do not replace the entire CoW transaction model. The cache should sit under or beside the existing FS-block helpers and preserve the known ordering constraints.

## 4. Fsync Boundary

`sys_fsync` must remain the BUILD-2/3C no-op in 3D.

3D may add private/test-only drain helpers, but user-visible `sys_fsync(fd)` must still:

- validate the file handle;
- not force dirty cache drain;
- not issue a FLUSH command because of fsync itself;
- not mutate NeoFS generation because of fsync itself.

Durable per-file fsync is BUILD-3E.

## 5. Write-Back Ordering

Write-back must not publish a superblock that points at missing data or invalid metadata.

For any transaction that reaches superblock publication:

1. dirty data blocks required by the transaction are written;
2. dirty inode/root/free-map metadata required by the transaction are written;
3. all relevant superblock copies are written;
4. one block-device FLUSH is issued if the device supports FLUSH;
5. only then is the transaction considered clean/committed in memory.

If 3D cannot preserve this ordering while allowing `write()` to return before the background worker drains the transaction, stop and file a blocker. Do not weaken crash consistency to make the cache fit.

## 6. Required Runtime Scenarios

Add focused QEMU scenarios and xtask wiring. Names may vary, but cover:

1. `phase3_neofs_cache_hit_miss_smoke`
   - read a NeoFS block/file through the cache;
   - prove first access misses and later access hits;
   - no content corruption.

2. `phase3_neofs_writeback_dirty_drain`
   - perform a write that leaves dirty cache state visible;
   - write-back worker drains it;
   - dirty count reaches zero;
   - data remains readable.

3. `phase3_neofs_writeback_worker_stop_join`
   - mount owns a write-back worker handle;
   - stop request reaches the worker;
   - worker drains or reaches a clean checkpoint;
   - join completes before mount-owned cache state can be dropped.

4. `phase3_neofs_writeback_reboot_after_drain`
   - write data;
   - wait for background drain;
   - reboot with same disk image;
   - data and free-map state are present after remount.

5. `phase3_neofs_writeback_crash_before_drain`
   - dirty write is present in memory/cache;
   - crash before drain/publication;
   - reboot mounts old valid state;
   - no torn superblock, invalid free-map, or corrupt active file.

6. `phase3_neofs_gc_waits_for_clean_cache`
   - dirty cache exists;
   - GC entry either drains first or refuses until clean;
   - GC never sweeps dirty-unpublished blocks.

7. `phase3_neofs_fsync_noop_write_through`
   - still proves fsync itself did not force drain or FLUSH;
   - include dirty/flush counters precise enough to separate background write-back from fsync.

## 7. Required Regression Gates

Run and report:

```text
cargo k
cargo k --no-default-features
cargo check -p xtask
cargo check -p neofs-mkfs
```

Run new 3D Docker/QEMU scenarios.

Run targeted regressions:

```text
cargo neo-test phase3_virtio_blk_flush_command_smoke
cargo neo-test phase3_virtio_blk_abandoned_io_no_leak
cargo neo-test phase3_block_vfs_smoke
cargo neo-test phase3_neofs_gc_reuses_reclaimed_block
cargo neo-test phase3_neofs_gc_reboot_persistence
cargo neo-test phase3_neofs_gc_corrupt_free_map_rejected
cargo neo-test phase3_neofs_filecap_generation_stale
cargo neo-test phase3_aarch64_virtio_barrier_lane
```

Because 3D touches cache/writeback and kthreads, run:

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

## 8. Verification Artifact

Create:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md
```

Include:

- exact commit SHA;
- changed files;
- cache architecture summary;
- write-back worker lifecycle summary;
- dirty block role/accounting summary;
- ordering and FLUSH publication summary;
- PASS frames for new cache/write-back scenarios;
- evidence GC does not race dirty cache state;
- evidence `sys_fsync` behavior did not change;
- targeted regression and full SMP sweep results;
- explicit statement that no durable fsync contract, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work landed.

Do not leave a placeholder for the commit SHA in the committed verification artifact.

## 9. Stop Conditions

Stop and file a blocker instead of committing if:

- write-back requires changing `sys_fsync` semantics in 3D;
- write-back can publish a superblock before required data/metadata is durable;
- dirty cache state can be dropped while still referenced by a mount, file handle, or worker;
- the worker cannot be stopped/joined without dropping mount-owned state first;
- GC can mark/sweep while dirty unpublished cache blocks exist;
- crash-before-drain creates an unmountable filesystem or corrupts a previously committed file;
- FileCap generation/stale-handle behavior regresses;
- the implementation starts durable fsync, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, broad POSIX work, or background GC beyond the cache write-back worker.

If blocked, create:

```text
Plans/BLOCKER-2026-05-14-phase-3-build-3-3d-neofs-writeback-cache-jarvis.md
```

## 10. Commit Discipline

Commit only 3D implementation and its verification or blocker artifact.

Suggested commit message:

```text
kernel/fs: add NeoFS write-back cache
```

Do not merge to `main`. Do not push. Do not start 3E.
