# Phase 3 BUILD-3 Sub-Commit 3E Verification

Date: 2026-05-15
Implementation commit: `8cd35490863514c9875a72704f8378d718eec908`
Subject: `kernel/fs: make NeoFS fsync durable`

## Scope

BUILD-3E changes `sys_fsync(fd)` from the BUILD-3D validation-only no-op into a durable NeoFS Regular-file operation.

Changed files:

- `kernel/Cargo.toml`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/inode.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`

No CAS/dedup, AHCI/NVMe, foreign filesystem, rootfs pivot, mmap/exec, broad POSIX, BUILD-4, or BUILD-3F+ work landed.

## Contract Summary

`sys_fsync(fd)` now looks up an existing file handle and calls `OpenFile::fsync()`. `OpenFile::fsync()` first validates the captured FileCap generation, so stale NeoFS FileCaps fail before any durability work can mutate cache, metadata, or block-device state.

NeoFS Regular inodes implement `fsync()` by serializing against namespace mutation and draining the mount-owned write-back cache through the existing ordered drain path. This is intentionally conservative for BUILD-3E: mount-wide drain, not per-file dirty attribution.

Dirty fsync waits for cache drain and observes virtio FLUSH command/completion on FLUSH-capable devices. The dirty FLUSH scenario asserts `device.supports_flush() == true` before using FLUSH counters as evidence. Clean fsync remains cheap and deterministic: the clean scenario starts from a fully-clean mount, then observes no dirty drain and no spurious FLUSH.

The `pause_worker()` / already-started `drain_pass()` race is acceptable for BUILD-3E because foreground `drain_all()` loops until the cache is fully clean (`flush_required == false` and no dirty cached blocks remain). A worker pass already in progress may finish, but fsync still owns the foreground wait-to-clean condition before returning.

The stale FileCap check remains anchored to the current reachable inode generation: stale fsync returns `ESTALE` before durability mutation, while the refreshed handle observes the advanced current generation and can fsync successfully.

## Build Gates

- `docker compose run --rm neo rustfmt --edition 2021 kernel/src/arch/x86_64/syscall.rs kernel/src/fs/mod.rs kernel/src/fs/neofs/inode.rs kernel/src/test_harness.rs tools/xtask/src/main.rs` -> PASS.
- `git diff --check` -> PASS, with only CRLF conversion notices from Git.
- `docker compose run --rm neo cargo check -p xtask` -> PASS.
- `docker compose run --rm neo cargo check -p neofs-mkfs` -> PASS.
- `docker compose run --rm neo cargo k` -> PASS. Kernel warning summary: `neoplutarch-kernel` generated 74 warnings.
- `docker compose run --rm neo cargo k --no-default-features` -> PASS. Kernel warning summary: `neoplutarch-kernel` generated 74 warnings.

## New 3E Scenario Evidence

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash phase3_neofs_unfsynced_dirty_may_be_lost phase3_neofs_fsync_issues_flush phase3_neofs_fsync_clean_no_extra_flush phase3_neofs_fsync_coordinates_with_writeback_gc phase3_neofs_fsync_filecap_stale_regression
```

Result: exit 0.

Serial PASS frames:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=write neofs_fsync_persist=prepared path=/neofs/fsync-3e-persist.txt bytes=56 dirty_blocks_before_fsync=10 dirty_blocks_after_fsync=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=readback neofs_fsync_persist=ok path=/neofs/fsync-3e-persist.txt bytes=56 selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_unfsynced_dirty_may_be_lost stage=write neofs_unfsynced_dirty=prepared marker=/neofs/fsync-3e-marker.txt target=/neofs/fsync-3e-unfsynced.txt marker_dirty_blocks_after_fsync=0 dirty_blocks_before_exit=10 writeback_hold=true no_target_fsync=true selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_unfsynced_dirty_may_be_lost stage=readback neofs_unfsynced_dirty=lost_without_corruption marker=/neofs/fsync-3e-marker.txt target=/neofs/fsync-3e-unfsynced.txt dirty_target_absent=true selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_issues_flush neofs_fsync=issues_flush path=/neofs/fsync-3e-flush.txt device_supports_flush=true dirty_blocks_before=10 dirty_blocks_after=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 flush_completions_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_clean_no_extra_flush neofs_fsync=clean_no_extra_flush path=/neofs/hello.txt dirty_blocks_before=0 dirty_blocks_after=0 drained_blocks_delta=0 cache_flushes_delta=0 flush_commands_delta=0 flush_completions_delta=0 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_coordinates_with_writeback_gc neofs_fsync_gc=coordinated path=/neofs/fsync-3e-gc.txt dirty_blocks_before_fsync=14 dirty_blocks_after_fsync=0 dirty_blocks_after_gc=4 dirty_blocks_after_gc_fsync=0 dirty_blocks_after_release=0 drained_blocks_delta=14 gc_fsync_drained_blocks_delta=4 reclaimed_blocks=16390 cache_flushes=2 writeback_hold_during_fsync=true drain_scope=mount_wide selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
```

Evidence mapping:

- Crash/reboot persistence: `phase3_neofs_fsync_dirty_persists_after_crash` wrote, fsynced, exited without mount teardown reliance, then read the same content after reboot.
- Unfsynced dirty behavior: `phase3_neofs_unfsynced_dirty_may_be_lost` preserved the committed marker and observed the held, unfsynced target absent after reboot.
- FLUSH command/completion: `phase3_neofs_fsync_issues_flush` asserted `device.supports_flush() == true`, then observed `flush_commands_delta=1` and `flush_completions_delta=1`.
- Clean fsync: `phase3_neofs_fsync_clean_no_extra_flush` started from a fully-clean mount and observed zero dirty/drain/flush deltas.
- GC/writeback coordination: `phase3_neofs_fsync_coordinates_with_writeback_gc` drained pre-GC dirty state, then drained GC-published metadata with a second fsync.
- Stale FileCap regression: `phase3_neofs_fsync_filecap_stale_regression` returned `ESTALE` (`-116`) before durability mutation; dirty/flush state was unchanged after stale fsync.

## Targeted Regressions

Command:

```text
docker compose run --rm neo cargo neo-test phase3_virtio_blk_flush_command_smoke phase3_virtio_blk_abandoned_io_no_leak phase3_block_vfs_smoke phase3_neofs_writeback_dirty_drain phase3_neofs_writeback_worker_stop_join phase3_neofs_gc_waits_for_clean_cache phase3_neofs_gc_reboot_persistence phase3_neofs_gc_corrupt_free_map_rejected phase3_neofs_filecap_generation_stale phase3_aarch64_virtio_barrier_lane
```

Result: exit 0.

Observed PASS scenarios:

- `phase3_virtio_blk_flush_command_smoke`
- `phase3_virtio_blk_abandoned_io_no_leak`
- `phase3_block_vfs_smoke`
- `phase3_neofs_writeback_dirty_drain`
- `phase3_neofs_writeback_worker_stop_join`
- `phase3_neofs_gc_waits_for_clean_cache`
- `phase3_neofs_gc_reboot_persistence`
- `phase3_neofs_gc_corrupt_free_map_rejected`
- `phase3_neofs_filecap_generation_stale`
- `phase3_aarch64_virtio_barrier_lane`

## Full SMP Sweep

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Captured log: `target/verify-phase3-build3e-neo-test-no-meta-smp4-rerun.log`.

Process status observed by PowerShell: exit 1.

Log classification: no `failed after`, no `event=FAIL`, no `KERNEL PANIC`, and no `BUG:` were observed. The run completed through the final listed scenario, `phase3_aarch64_virtio_barrier_lane`, with PASS. The log contains 122 scenario PASS lines. Seven attempt-level QEMU timeouts occurred, and all seven had matching retry PASS lines.

Relevant retry evidence:

```text
[xtask] neo-test: `phase3_neofs_writeback_reboot_after_drain` two-boot attempt 1/2 failed: QEMU timed out after 180s
[xtask] neo-test: `phase3_neofs_writeback_reboot_after_drain` two-boot PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_clean_no_extra_flush` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_clean_no_extra_flush` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_aarch64_virtio_barrier_lane` PASS
```

Follow-up check:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_clean_no_extra_flush phase3_neofs_fsync_filecap_stale_regression
```

Result: exit 0.

This records the SMP sweep honestly: scenario-level evidence is green after retry, but the wrapper process status was not clean in the captured full-sweep run.

## Final Diff Checks

Before implementation commit:

- `git diff --check` -> PASS, with only CRLF conversion notices from Git.
- `git diff --cached --check` -> PASS.
