# Phase 3 BUILD-3 sub-commit 3A verification

Date: 2026-05-14
Branch: codex/phase-3-build-3-3a-kthread-lifecycle
Base: 096414c4da9f2294c35bcfc9e0c459a99c50078b
Commit SHA: reported after commit; this artifact is committed with the implementation.

## Scope

Implemented BUILD-3A only: scheduler-owned kthread ownership/lifecycle substrate.

No NeoFS write-back cache, virtio FLUSH negotiation, GC, free-space format
change, `sys_fsync` behavior change, CAS/dedup, AHCI/NVMe, foreign FS, rootfs
pivot, mmap/exec, or broad POSIX work landed.

## Changed files

- `kernel/src/sched/kthread.rs`
- `kernel/src/sched/mod.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/test_harness.rs`
- `kernel/Cargo.toml`
- `tools/xtask/src/main.rs`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md`

## API summary

- Added `sched::kthread` with `KthreadId`, `KthreadHandle`,
  `KthreadStopToken`, `KthreadExitState`, and `KthreadJoinResult`.
- `KthreadId` uses registry slot plus generation identity.
- `spawn_owned` creates a kernel worker under the normal scheduler-owned
  `Box<Thread>` model and returns a subsystem-owned handle.
- Handles own lifecycle authority only. They do not contain `Arc<Thread>`,
  `Box<Thread>`, or raw `Thread` pointers.
- Worker entry receives a `KthreadStopToken`; stop is cooperative, visible via
  `is_stop_requested`, and can wake a worker parked through
  `park_until_stop`.
- `join` parks the joining owner through the scheduler wake-token path and
  observes normal worker completion.
- `detach` supports detached/global-worker semantics. Dropping an unjoined
  handle marks the worker detached so completed workers can unregister.
- Worker-visible failures are returned as `KthreadExitState::Failed(code)`.
  Kernel panic remains fatal because the kernel does not unwind.

## Runtime evidence

New BUILD-3A scenarios were run in the Docker dev container:

```text
docker compose run --rm neo cargo neo-test phase3_kthread_spawn_join_smoke phase3_kthread_stop_join_smoke phase3_kthread_stop_wakes_parked_worker phase3_kthread_detached_worker_smoke phase3_kthread_no_thread_arc_lifetime --no-meta
```

PASS frames from the final full sweep serial logs:

```text
target/neo-test/phase3_kthread_spawn_join_smoke.serial.log:99:
NEO_TEST/1 event=PASS name=phase3_kthread_spawn_join_smoke kthread_slot=0 generation=1 runs=1 joined=true reaped=true

target/neo-test/phase3_kthread_stop_join_smoke.serial.log:99:
NEO_TEST/1 event=PASS name=phase3_kthread_stop_join_smoke kthread_slot=0 generation=1 stop_observed=true joined=true

target/neo-test/phase3_kthread_stop_wakes_parked_worker.serial.log:99:
NEO_TEST/1 event=PASS name=phase3_kthread_stop_wakes_parked_worker kthread_slot=0 generation=1 parked=true stop_wake=true joined=true

target/neo-test/phase3_kthread_detached_worker_smoke.serial.log:99:
NEO_TEST/1 event=PASS name=phase3_kthread_detached_worker_smoke kthread_slot=0 generation=1 detached=true reaped=true

target/neo-test/phase3_kthread_no_thread_arc_lifetime.serial.log:99:
NEO_TEST/1 event=PASS name=phase3_kthread_no_thread_arc_lifetime kthread_slot=0 generation=1 handle_control_arc=true thread_owner=scheduler thread_arc=false raw_thread=false
```

## Regression evidence

Build/check gates:

```text
rustfmt --edition 2024 kernel/src/sched/kthread.rs kernel/src/sched/mod.rs kernel/src/sched/scheduler.rs kernel/src/test_harness.rs tools/xtask/src/main.rs
cargo k
cargo k --no-default-features
cargo check -p xtask
```

Results: PASS. `cargo k` and `cargo k --no-default-features` still emit the
pre-existing kernel warning set; no kthread dead-code warning remains in the
normal production build.

Targeted regression gates were run in Docker:

```text
docker compose run --rm neo cargo neo-test phase3_virtio_blk_abandoned_io_no_leak phase3_block_vfs_smoke phase3_virtio_blk_raw_rw_smoke phase3_aarch64_virtio_barrier_lane --no-meta
```

PASS frames from the final full sweep serial logs:

```text
target/neo-test/phase3_virtio_blk_abandoned_io_no_leak.serial.log:104:
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false

target/neo-test/phase3_block_vfs_smoke.serial.log:118:
NEO_TEST/1 event=PASS name=phase3_block_vfs_smoke bdf=0x000300 block_vfs_same_boot=true raw_rw=ok vfs_syscalls=ok stdout_namespace=fd1_serial lba=3 block_size=512 completions_delta=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false

target/neo-test/phase3_virtio_blk_raw_rw_smoke.serial.log:100:
NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke bdf=0x000300 block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false

target/neo-test/phase3_aarch64_virtio_barrier_lane.serial.log:9:
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

Full scheduler-regression sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: PASS. Final `target/neo-test` collection contained 93 serial logs and
91 PASS frames, including the five new kthread scenarios and the aarch64
barrier lane. The two-count difference is from multi-boot scenario logging.

## Environment notes

Runtime QEMU evidence was collected through the established Docker dev
container path. Host-only QEMU was not used; Docker provided the Limine,
xorriso, QEMU x86_64, and QEMU aarch64 runtime environment.

Scoped rustfmt was run on the changed Rust files. Global `cargo fmt --check`
was not claimed.

## Final checks

Results:

```text
git diff --check                 PASS
git diff --cached --check        PASS
```
