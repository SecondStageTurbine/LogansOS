# Phase 3 BUILD-3 Sub-Commit 3B Verification - Virtio-blk FLUSH

Date: 2026-05-14
Agent: Jarvis
Branch: codex/phase-3-build-3-3b-virtio-flush
Implementation commit SHA: a3023a0259be4b1d1694442ddfa370fde7099b99
Implementation commit message: kernel/storage: negotiate virtio-blk flush

## Scope

BUILD-3B adds the kernel-internal block FLUSH API and negotiates/issues real virtio-blk FLUSH requests. It does not change NeoFS write-back, GC, free-space format, sys_fsync behavior, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broader POSIX behavior.

## Changed Files

Implementation commit:
- kernel/Cargo.toml
- kernel/src/arch/x86_64/syscall.rs
- kernel/src/drivers/block/mod.rs
- kernel/src/drivers/block/virtio_blk.rs
- kernel/src/test_harness.rs
- tools/xtask/src/main.rs

Verification artifact:
- Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md

## Block FLUSH API Summary

- `BlockOp::Flush` is added for the block layer.
- `BlockDevice::supports_flush()` exposes the capability query.
- `BlockDevice::flush()` exposes the synchronous kernel-internal flush call.
- Virtio-blk implements `flush()` as a real hardware request through the existing queue, completion, and wake-token path.

## Virtio Feature Policy

- Required attach gate remains: QEMU/device must offer `VIRTIO_BLK_F_FLUSH`.
- Driver features now negotiate `VIRTIO_F_VERSION_1 | VIRTIO_BLK_F_FLUSH`.
- `VIRTIO_BLK_F_CONFIG_WCE` remains not negotiated.
- Probe/runtime logs explicitly report:
  - `F_FLUSH_offered=true`
  - `F_FLUSH_negotiated=true`
  - `F_CONFIG_WCE_negotiated=false`

## Request Path Summary

- `VIRTIO_BLK_T_FLUSH = 4` is issued for block flush.
- Read/write requests retain the existing header + data + status descriptor chain.
- FLUSH requests use header + status descriptors only; no fake data buffer is allocated.
- Header/status DMA buffers are retained until completion through the same `IoRendezvous` lifetime model as read/write.
- Completion wakes the blocked caller through the existing `ThreadWakeToken` storage rendezvous.
- Descriptor-publish and notification-suppression barriers remain on the existing shared submit path.

## Runtime Evidence

New 3B targeted scenarios:

```text
docker compose run --rm neo cargo neo-test phase3_virtio_blk_flush_negotiated_gate phase3_virtio_blk_flush_command_smoke --no-meta
```

PASS frames:

```text
target/neo-test/phase3_virtio_blk_flush_negotiated_gate.serial.log:97: NEO_TEST/1 event=PASS name=phase3_virtio_blk_flush_negotiated_gate bdf=0x000300 offered_features=0x0000010130007e54 driver_features=0x0000000100000200 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false block_flush_supported=true block_count=32768 block_size=512 attach=rw
target/neo-test/phase3_virtio_blk_flush_command_smoke.serial.log:101: NEO_TEST/1 event=PASS name=phase3_virtio_blk_flush_command_smoke bdf=0x000300 request_type=4 VIRTIO_BLK_T_FLUSH=true completions_delta=1 flush_commands_delta=1 flush_completions_delta=1 wake_result=1 retained_dma_buffers=2 has_data_buffer=false readback=ok descriptor_publish_barriers_delta=1 notification_suppression_barriers_delta=1 live_before=6 live_after=6 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
```

Regression scenarios:

```text
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke phase3_virtio_blk_abandoned_io_no_leak phase3_block_vfs_smoke phase3_neofs_fsync_noop_write_through phase3_aarch64_virtio_barrier_lane --no-meta
```

PASS frames:

```text
target/neo-test/phase3_virtio_blk_raw_rw_smoke.serial.log:100: NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke bdf=0x000300 block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
target/neo-test/phase3_virtio_blk_abandoned_io_no_leak.serial.log:104: NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
target/neo-test/phase3_block_vfs_smoke.serial.log:118: NEO_TEST/1 event=PASS name=phase3_block_vfs_smoke bdf=0x000300 block_vfs_same_boot=true raw_rw=ok vfs_syscalls=ok stdout_namespace=fd1_serial lba=3 block_size=512 completions_delta=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
target/neo-test/phase3_neofs_fsync_noop_write_through.serial.log:412: NEO_TEST/1 event=PASS name=phase3_neofs_fsync_noop_write_through neofs_fsync=noop path=/neofs/fsync-2d.txt bytes=53 fsync_rc=0 generation_before=3 generation_after=3 write_through=true flush_commands_before=0 flush_commands_after=0 no_flush_command=true no_intent_log=true selected_superblock=0 generation=1
target/neo-test/phase3_aarch64_virtio_barrier_lane.serial.log:9: NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

Full SMP sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result:

```text
PASS
serial_logs=95
pass_frames=93
```

## Compile and Diff Verification

```text
cargo k
PASS

cargo k --no-default-features
PASS

cargo check -p xtask
PASS

rustfmt --edition 2024 kernel/src/drivers/block/mod.rs kernel/src/drivers/block/virtio_blk.rs kernel/src/test_harness.rs kernel/src/arch/x86_64/syscall.rs tools/xtask/src/main.rs
PASS

git diff --check
PASS

git diff --cached --check
PASS before implementation commit a3023a0259be4b1d1694442ddfa370fde7099b99
```

## Explicit Non-Transition Statement

No NeoFS write-back cache, mark-sweep GC, free-space bitmap or format changes, sys_fsync behavior transition, durable fsync contract, CAS/dedup, AHCI/NVMe, foreign filesystem, rootfs pivot, file-backed mmap/exec, or broad POSIX work landed in BUILD-3B.
