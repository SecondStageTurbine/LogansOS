# Phase 3 BUILD-3 Close Evidence

Date: 2026-05-15
Agent: Jarvis
Status: VERIFICATION PASSED

This is close evidence only. It is not the BUILD-3 close report, not BUILD-4,
not BUILD-3F, not a merge, and not a push.

## Substrate

HEAD verified:

```text
747c3a6afc495d775719ce7152fb7d89f9691bb8
```

HEAD short log at verification start:

```text
747c3a6 docs: fix BUILD-3E timeout resolution formatting
3779491 docs: resolve BUILD-3E SMP timeout investigation
1da2d9a docs: fix BUILD-3E blocker formatting
6b53e49 docs: file BUILD-3E full SMP sweep blocker
60319e3 test: tighten BUILD-3E fsync evidence
9ef3599 docs: record Phase 3 BUILD-3E verification
```

## Build And Check Gates

All required build/check gates passed:

```text
git diff --check
exit=0
```

```text
docker compose run --rm neo cargo check -p xtask
exit=0
log=target/verify-phase3-build3-close-cargo-check-xtask.log
result=Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s
```

```text
docker compose run --rm neo cargo check -p neofs-mkfs
exit=0
log=target/verify-phase3-build3-close-cargo-check-neofs-mkfs.log
result=Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s
```

```text
docker compose run --rm neo cargo k
exit=0
log=target/verify-phase3-build3-close-cargo-k.log
warning_summary=`neoplutarch-kernel` (bin "kernel") generated 74 warnings
result=Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.70s
```

```text
docker compose run --rm neo cargo k --no-default-features
exit=0
log=target/verify-phase3-build3-close-cargo-k-no-default.log
warning_summary=`neoplutarch-kernel` (bin "kernel") generated 74 warnings
result=Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.59s
```

## AArch64 Runtime Barrier Lane

Command:

```text
docker compose run --rm neo cargo neo-test phase3_aarch64_virtio_barrier_lane
exit=0
log=target/verify-phase3-build3-close-aarch64-barrier.log
serial=target/neo-test/phase3_aarch64_virtio_barrier_lane.serial.log
```

Runtime PASS frame:

```text
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

## Final Full SMP Sweep

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
log=target/verify-phase3-build3-close-smp-sweep.log
```

Timestamps and exit:

```text
start=2026-05-15T19:48:47.2939626-04:00
end=2026-05-15T20:32:43.9674784-04:00
exit=0
```

Observed transient retry evidence:

```text
[xtask] neo-test: `phase3_neofs_mount_selects_highest_generation` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_mount_selects_highest_generation` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_reboot_persistence_small_file` two-boot attempt 1/2 failed: QEMU timed out after 180s
[xtask] neo-test: `phase3_neofs_reboot_persistence_small_file` two-boot PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
```

BUILD-3E fsync scenario evidence from the full SMP sweep:

```text
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` two-boot persistence PASS (disk:
[xtask] neo-test: `phase3_neofs_unfsynced_dirty_may_be_lost` PASS
[xtask] neo-test: `phase3_neofs_unfsynced_dirty_may_be_lost` PASS
[xtask] neo-test: `phase3_neofs_unfsynced_dirty_may_be_lost` two-boot persistence PASS (disk:
[xtask] neo-test: `phase3_neofs_fsync_issues_flush` PASS
[xtask] neo-test: `phase3_neofs_fsync_clean_no_extra_flush` PASS
[xtask] neo-test: `phase3_neofs_fsync_coordinates_with_writeback_gc` PASS
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_aarch64_virtio_barrier_lane` PASS
```

No `failed after`, `event=FAIL`, `KERNEL PANIC`, or `BUG:` was observed in the
full SMP sweep extraction.

## Final Cached Diff Gate

The final staged diff check was run after this evidence artifact was staged:

```text
git diff --cached --check
exit=0
```

## Scope Statement

No BUILD-4, BUILD-3F, CAS/dedup, AHCI/NVMe, foreign filesystem support,
rootfs pivot, mmap/exec, broad POSIX expansion, merge, or push occurred during
this verification-only close-evidence lane.

## Caveats To Carry Forward

The eventual BUILD-3 close report must carry the PM disposition caveats
explicitly:

- C1: 3E durable fsync uses mount-wide drain, not per-file dirty attribution.
- C2: GC remains quiescent.
- C3: no background GC worker landed.
- C4: `VIRTIO_BLK_F_CONFIG_WCE` remains not negotiated.
- C5: transient QEMU timeouts occurred under SMP and were resolved by clean rerun evidence.
- C6: fatal kernel panic/no unwind behavior remains.
- C7: single-thread-per-process invariant remains.
- C8: legacy BUILD-2 image compatibility caveat remains.
