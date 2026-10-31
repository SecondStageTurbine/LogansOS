# Phase 3 BUILD-3E SMP Timeout Resolution

Date: 2026-05-15
HEAD: `1da2d9a963ec7034ca1b511436d8dafae2adaef9`
Accepted implementation HEAD: `60319e32d348082fa1e5f90ee98288cfcc5bcf42`

## Context

PM accepted BUILD-3E implementation but blocked closure on a clean full SMP sweep after:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

failed with terminal scenario:

```text
phase3_neofs_fsync_dirty_persists_after_crash
```

The failure was documented in:

```text
Plans/BLOCKER-2026-05-15-phase-3-build-3-3e-full-smp-sweep-jarvis.md
```

## Investigation

No kernel code changes were made in this investigation lane.

### Targeted Scenario

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
```

Result: exit 0.

Evidence:

```text
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` two-boot persistence PASS
```

### Immediate Predecessor Mini-Suite

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_writeback_dirty_drain phase3_neofs_writeback_worker_stop_join phase3_neofs_writeback_reboot_after_drain phase3_neofs_writeback_crash_before_drain phase3_neofs_gc_waits_for_clean_cache phase3_neofs_fsync_dirty_persists_after_crash
```

Timestamps:

```text
start=2026-05-15T15:20:58.1466581-04:00
end=2026-05-15T15:28:31.2512985-04:00
exit=0
```

Result: exit 0.

Notes:

- `phase3_neofs_writeback_reboot_after_drain` timed out on attempt 1, then passed on retry.
- `phase3_neofs_fsync_dirty_persists_after_crash` passed both boots.

### NeoFS 3C/3D Cluster

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_cow_write_after_write phase3_neofs_crash_mid_overwrite_recovery phase3_neofs_superblock_rotation_recovery phase3_neofs_crash_mid_create_recovery phase3_neofs_typed_inode_exec_refusal phase3_neofs_regular_exec_policy_stub phase3_neofs_type_vs_rights_policy_errors phase3_neofs_filecap_generation_stale phase3_neofs_reserved_type_refusal phase3_neofs_filerights_u32_forward_bits phase3_neofs_gc_mark_live_set phase3_neofs_gc_reclaims_orphan_after_overwrite phase3_neofs_gc_reuses_reclaimed_block phase3_neofs_gc_reboot_persistence phase3_neofs_gc_crash_before_publish phase3_neofs_gc_corrupt_free_map_rejected phase3_neofs_cache_hit_miss_smoke phase3_neofs_writeback_dirty_drain phase3_neofs_writeback_worker_stop_join phase3_neofs_writeback_reboot_after_drain phase3_neofs_writeback_crash_before_drain phase3_neofs_gc_waits_for_clean_cache phase3_neofs_fsync_dirty_persists_after_crash
```

Timestamps:

```text
start=2026-05-15T15:28:54.9382223-04:00
end=2026-05-15T15:54:34.1143019-04:00
exit=0
```

Result: exit 0.

Notes:

- Four attempt-level QEMU timeouts occurred and retried to PASS.
- `phase3_neofs_fsync_dirty_persists_after_crash` passed both boots.

## Clean Full SMP Sweep

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Captured log:

```text
target/verify-phase3-build3e-timeout-full-sweep-rerun2.log
```

Timestamps:

```text
start=2026-05-15T15:54:59.0905889-04:00
end=2026-05-15T16:41:32.6993164-04:00
exit=0
```

Result: exit 0.

The full SMP hard gate is now clean.

Transient retry evidence in the clean run:

```text
75526: [xtask] neo-test: `phase3_neofs_crash_mid_overwrite_recovery` two-boot attempt 1/2 failed: QEMU timed out after 180s
76291: [xtask] neo-test: `phase3_neofs_crash_mid_overwrite_recovery` two-boot PASS after retry attempt 2/2
87015: [xtask] neo-test: `phase3_neofs_gc_crash_before_publish` two-boot attempt 1/2 failed: QEMU timed out after 180s for
87786: [xtask] neo-test: `phase3_neofs_gc_crash_before_publish` two-boot PASS after retry attempt 2/2
90354: [xtask] neo-test: `phase3_neofs_writeback_dirty_drain` attempt 1/2 failed: QEMU timed out after 180s for
91140: [xtask] neo-test: `phase3_neofs_writeback_dirty_drain` PASS after retry attempt 2/2
99478: [xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
100351: [xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
```

Final 3E scenario evidence from the clean full sweep:

```text
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` two-boot persistence PASS
[xtask] neo-test: `phase3_neofs_unfsynced_dirty_may_be_lost` PASS
[xtask] neo-test: `phase3_neofs_unfsynced_dirty_may_be_lost` PASS
[xtask] neo-test: `phase3_neofs_unfsynced_dirty_may_be_lost` two-boot persistence PASS
[xtask] neo-test: `phase3_neofs_fsync_issues_flush` PASS
[xtask] neo-test: `phase3_neofs_fsync_clean_no_extra_flush` PASS
[xtask] neo-test: `phase3_neofs_fsync_coordinates_with_writeback_gc` PASS
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_aarch64_virtio_barrier_lane` PASS
```

Serial PASS frames:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=write neofs_fsync_persist=prepared path=/neofs/fsync-3e-persist.txt bytes=56 dirty_blocks_before_fsync=10 dirty_blocks_after_fsync=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=readback neofs_fsync_persist=ok path=/neofs/fsync-3e-persist.txt bytes=56 selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_issues_flush neofs_fsync=issues_flush path=/neofs/fsync-3e-flush.txt device_supports_flush=true dirty_blocks_before=10 dirty_blocks_after=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 flush_completions_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1
```

## Classification

The timeout was not reproduced by:

- direct `phase3_neofs_fsync_dirty_persists_after_crash` under `NEO_SMP=4`;
- the immediate predecessor mini-suite;
- the broader NeoFS 3C/3D cluster.

The later full `NEO_SMP=4 --no-meta` sweep passed. Current best classification is full-suite/QEMU timeout pressure with successful retry behavior, not a deterministic BUILD-3E fsync correctness failure.

One harness weakness remains: xtask uses fixed per-scenario serial paths, so failed retry-attempt serial logs can be overwritten by a later retry or targeted rerun. If this class of issue recurs, preserve per-attempt serial logs before deeper kernel work.
