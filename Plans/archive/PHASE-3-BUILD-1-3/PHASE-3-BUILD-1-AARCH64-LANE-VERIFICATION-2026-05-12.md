# Phase 3 BUILD-1 aarch64 Barrier Lane Verification

Date: 2026-05-12
Branch: `codex/phase-3-build-1-jarvis`
Dispatch: `Plans/DISPATCH-2026-05-12-phase-3-build-1-aarch64-barrier-lane-jarvis.md`
Disposition: PASS for the dedicated aarch64 barrier-exposure lane. This is not a BUILD-1 close report.

## Scope

This lane adds a narrow aarch64 runtime gate for the virtio split-ring ordering points that blocked BUILD-1 closure:

- descriptor publish ordering before the available-ring index is exposed
- notification-suppression ordering before reading `used.avail_event`
- real `qemu-system-aarch64` execution, not compile-only evidence

It does not start BUILD-2 and does not attempt a general aarch64 kernel port.

## Implementation Evidence

- Docker image now installs `qemu-system-arm`, which provides `/usr/bin/qemu-system-aarch64`.
- `rust-toolchain.toml` now includes `aarch64-unknown-none` and `rustfmt` for the pinned nightly.
- Production virtio-blk calls shared wrappers in `kernel/src/drivers/block/virtio_barrier.rs`:
  - `publish_descriptors()` uses `fence(Ordering::Release)`.
  - `before_notification_check()` uses `fence(Ordering::SeqCst)`.
- The aarch64 harness includes the same wrapper source with:
  - `#[path = "../../../kernel/src/drivers/block/virtio_barrier.rs"] mod virtio_barrier;`
- `cargo neo-test phase3_aarch64_virtio_barrier_lane` builds a standalone no_std aarch64 ELF and runs it under QEMU `virt` with a modern virtio-mmio block device.

QEMU command shape emitted by xtask:

```text
qemu-system-aarch64 -M virt -cpu cortex-a57 -m 128M -display none
  -serial file:target/neo-test/phase3_aarch64_virtio_barrier_lane.serial.log
  -monitor none
  -kernel target/aarch64-virtio-barrier-lane/aarch64-unknown-none/release/phase3-aarch64-virtio-barrier-lane
  -drive if=none,format=raw,file=target/neo-test/phase3_aarch64_virtio_barrier_lane.raw,id=hd0,cache=none
  -device virtio-blk-device,drive=hd0
  -global virtio-mmio.force-legacy=false
  -no-reboot -d guest_errors
  -semihosting-config enable=on,target=native
```

The explicit `virtio-mmio.force-legacy=false` is required on Debian QEMU 7.2 because the default `virt` transports are legacy unless forced modern.

## Runtime Evidence

QEMU availability:

```text
/usr/bin/qemu-system-aarch64
QEMU emulator version 7.2.22 (Debian 1:7.2+dfsg-7+deb12u18)
```

Final aarch64 command:

```text
docker compose run --rm neo cargo neo-test phase3_aarch64_virtio_barrier_lane
```

Result: PASS

Serial log:

```text
target/neo-test/phase3_aarch64_virtio_barrier_lane.serial.log
```

PASS frame:

```text
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

This proves runtime execution because the harness:

- boots under QEMU aarch64 and emits PL011 serial frames
- discovers virtio-mmio block at `0x000000000a003e00`, version 2
- negotiates `VERSION_1` and `EVENT_IDX`
- writes sector 8, reads it back, and validates data integrity
- exits through semihosting only after the PASS frame
- reports `compile_only=false sentinel=runtime_qemu_aarch64`

Barrier exposure counts:

- `publish_descriptors=2`
- `before_notification_check=2`
- `notification_decisions=2`
- `event_idx_negotiated=true`

## Regression Gates

Build and runner:

- `docker compose build neo`: PASS
- `cargo check -p xtask`: PASS
- `cargo k`: PASS
- `cargo k --no-default-features`: PASS
- `git diff --check`: PASS, with Windows LF/CRLF warnings only
- Scoped rustfmt for new aarch64 harness and barrier wrapper: PASS

BUILD-1 x86 integration gates:

```text
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke phase3_block_vfs_smoke phase3_storage_completion_cross_cpu_wake phase3_build1_reboot_persistence_smoke
```

Result: PASS

Serial paths:

- `target/neo-test/phase3_virtio_blk_raw_rw_smoke.serial.log`
- `target/neo-test/phase3_block_vfs_smoke.serial.log`
- `target/neo-test/phase3_storage_completion_cross_cpu_wake.serial.log`
- `target/neo-test/phase3_build1_reboot_persistence_smoke.boot1.serial.log`
- `target/neo-test/phase3_build1_reboot_persistence_smoke.boot2.serial.log`

Key x86 PASS evidence:

- `F_FLUSH_offered=true`
- `F_FLUSH_negotiated=false`
- `F_CONFIG_WCE_negotiated=false`
- `descriptor_publish_barriers=2`
- `notification_suppression_barriers=2`
- reboot persistence boot 1 `stage=write same_boot_verify=ok`
- reboot persistence boot 2 `stage=readback`

Full default non-meta sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: PASS

## Formatting Status

`cargo fmt --check` and Docker `cargo fmt --check` now run after adding `rustfmt` to the pinned toolchain, but the repo has broad pre-existing format drift outside this lane. Representative first paths reported by Docker:

- `kernel/src/arch/x86_64/apic.rs`
- `kernel/src/arch/x86_64/gdt.rs`
- `kernel/src/arch/x86_64/pic.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `kernel/src/drivers/fb/font.rs`
- `kernel/src/loader/elf.rs`
- `kernel/src/mm/mod.rs`

The new standalone harness and new shared barrier wrapper were checked with scoped rustfmt and pass.

## Closure Note

This artifact supplies the requested aarch64 verification. It does not declare BUILD-1 closed and no BUILD-2 NeoFS work is included.
