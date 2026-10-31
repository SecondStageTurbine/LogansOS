# Phase 3 BUILD-1 Sub-Commit 1D Verification

Date: 2026-05-12
Owner: Jarvis
Branch: `codex/phase-3-build-1-jarvis`
Commit: working tree at artifact creation; final sub-commit contains this file.

## Verdict

`x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane`

BUILD-1 is not closed by this artifact. The x86_64 block+VFS integration gates pass, but the required aarch64 barrier-exposure lane cannot be honestly run in the current repo. See `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`.

## Files Changed

- `kernel/Cargo.toml`
- `kernel/src/drivers/block/virtio_blk.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `userland/neo-init/Cargo.toml`
- `userland/neo-init/src/main.rs`
- `userland/neo-worker/Cargo.toml`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md`
- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`

## Scenario Evidence

New 1D scenarios:

- `docker compose run --rm neo cargo neo-test phase3_block_vfs_smoke`: PASS.
  - Serial: `target/neo-test/phase3_block_vfs_smoke.serial.log`
  - Evidence: `block_vfs_same_boot=true raw_rw=ok vfs_syscalls=ok stdout_namespace=fd1_serial lba=3 block_size=512 completions_delta=2 descriptor_publish_barriers=2 notification_suppression_barriers=2`.
  - Feature policy: `F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false`.
  - VFS stdout namespace marker: `NEO_STDOUT_COMPAT/1 namespace=serial fd=1 source=vfs-block-smoke`.

- `docker compose run --rm neo cargo neo-test phase3_storage_completion_cross_cpu_wake`: PASS.
  - Serial: `target/neo-test/phase3_storage_completion_cross_cpu_wake.serial.log`
  - Evidence: `completions_delta=1 wake_result=1 completion_cpu=0 token_cpu=0 token_slot=0 actual_cross_cpu=false ready_cpus=4 instrumented_completion_path=virtio_used_ring instrumented_wake_token_path=ThreadWakeToken irq_routing_forced=false live_before=3 live_after=3`.
  - The test uses the real virtio used-ring completion path and `ThreadWakeToken` wake path. QEMU IRQ routing was not forced to a different CPU in this lane, so cross-CPU composition is supported by instrumentation here and by the existing SMP rendezvous stress lane below.

- `docker compose run --rm neo cargo neo-test phase3_build1_reboot_persistence_smoke`: PASS.
  - Disk image: `target/neo-test/phase3_build1_reboot_persistence_smoke.raw`
  - Boot 1 serial: `target/neo-test/phase3_build1_reboot_persistence_smoke.boot1.serial.log`
  - Boot 2 serial: `target/neo-test/phase3_build1_reboot_persistence_smoke.boot2.serial.log`
  - Evidence: boot 1 `stage=write image=reused lba=5 block_size=512 completions_delta=3 same_boot_verify=ok`; boot 2 `stage=readback image=reused lba=5 block_size=512 completions_delta=1`.

Required rerun/reference scenarios:

- `docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke`: PASS.
  - Serial: `target/neo-test/phase3_virtio_blk_raw_rw_smoke.serial.log`
  - Evidence: `block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false`.

- `docker compose run --rm neo cargo neo-test phase3_vfs_testfs_open_read_close`: PASS.
  - Serial: `target/neo-test/phase3_vfs_testfs_open_read_close.serial.log`

- `docker compose run --rm neo cargo neo-test phase3_vfs_testfs_write_readback`: PASS.
  - Serial: `target/neo-test/phase3_vfs_testfs_write_readback.serial.log`

- `docker compose run --rm neo cargo neo-test phase3_vfs_stdout_compat`: PASS during the broad SMP non-explicit suite.
  - Serial: `target/neo-test/phase3_vfs_stdout_compat.serial.log`
  - Evidence: `NEO_STDOUT_COMPAT/1 fd=1 route=serial`; `sys_open_skips_fd1=true`.

- `docker compose run --rm neo cargo neo-test phase3_vfs_type_vs_rights_errors`: PASS during the broad SMP non-explicit suite.
  - Serial: `target/neo-test/phase3_vfs_type_vs_rights_errors.serial.log`
  - This preserves the 1C distinction between type errors and rights errors.

- `docker compose run --rm neo cargo neo-test phase3_virtio_blk_abandoned_io_no_leak`: PASS.
  - Serial: `target/neo-test/phase3_virtio_blk_abandoned_io_no_leak.serial.log`
  - Evidence: `retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false`.

- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress`: PASS.
  - Serial: `target/neo-test/smp_cross_cpu_rendezvous_stress.serial.log`
  - Evidence: `iterations=1000 send_first=500 recv_first=500 cpus=4`.

- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_block_vfs_smoke`: PASS after retry attempt 2/2.
  - Serial: `target/neo-test/phase3_block_vfs_smoke.serial.log`

- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all`: stopped at the known explicit `irqsafe_mutex_bench` threshold miss.
  - Serial: `target/neo-test/irqsafe_mutex_bench.serial.log`
  - Evidence: `reason=overhead_threshold overhead_ns=11017 threshold_ns=6000`.
  - This matches the dispatch's known isolated benchmark timing miss and is not treated as a storage/VFS regression.

- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta`: PASS.
  - This ran the non-explicit SMP integration suite without the host-load-sensitive explicit microbenchmark.

## Build and Style Checks

- `cargo k`: PASS.
- `cargo k --no-default-features`: PASS.
- `git diff --check`: PASS.
- `cargo fmt --check`: BLOCKED by host environment.
  - Exact error: `'cargo-fmt.exe' is not installed for the toolchain 'nightly-2026-04-16-x86_64-pc-windows-msvc'`.
- `docker compose run --rm neo cargo fmt --check`: BLOCKED by Docker toolchain environment.
  - Exact error: `'cargo-fmt' is not installed for the toolchain 'nightly-2026-04-16-x86_64-unknown-linux-gnu'`.

## aarch64 Gate

Attempted compile-only probe:

```powershell
cargo build -p neoplutarch-kernel --target aarch64-unknown-none '-Zbuild-std=core,alloc' '-Zbuild-std-features=compiler-builtins-mem' --no-default-features
```

Result: FAIL. The repo still compiles x86_64-only arch modules for the aarch64 target, including `x86_64::instructions` imports, `extern "x86-interrupt"` handlers, x86 inline asm/registers, x86-only `uart_16550::SerialPort`, x86 page-table/TLB paths, and x86 Limine MP fields. This is not a barrier-exposure runtime lane.

Blocker artifact: `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`.

## Final Status

The BUILD-1 x86_64 integration gates pass. BUILD-1 closure remains blocked because the aarch64 barrier-exposure lane is missing.
