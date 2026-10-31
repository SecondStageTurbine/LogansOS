# CLOSE-REPORT - BlockDeviceLogSink degradation visibility + boot-time self-test

**Date:** 2026-05-23
**Dispatch:** `Plans/DISPATCH-2026-05-23-block-sink-degradation-visibility-jarvis.md`
**Worker:** Jarvis (Codex)
**Status:** CLOSED - implementation and local verification complete

## Summary

Implemented durable block-log-sink degradation diagnostics, a bare-metal-only log-sink self-test, extractor reporting for degradation fields, and the distinct framebuffer self-test failure state.

Runtime verification of the actual degradation marker path and strict-halt framebuffer state still requires the next BUILD-5b real-hardware run on the OptiPlex 7080 Micro. QEMU does not reproduce the captured_bytes=0 hardware failure.

## Acceptance Criteria

| Criterion | Status | Notes |
|---|---:|---|
| C1 - degradation reason recorded out-of-band | PASS | `BlockDeviceLogSink` now records reason/op/LBA/cursor and writes a rebuilt 4096-byte header via direct `device.write_block`, with no read-modify-write or `_print` recursion. Runtime trigger awaits hardware. |
| C2 - boot-time self-test in bare-metal mode | PASS | Bare-metal path emits the required begin sentinel, flushes, rereads the header/payload through a fresh `DmaBuffer`, checks `write_cursor > 0`, degradation reason is zero, and payload contains the sentinel. Failure strict-halts with `log sink selftest failed`. Runtime trigger awaits hardware. |
| C3 - extractor prints degradation fields | PASS | `neo-extract-log` prints a degradation block only when `degradation_reason != 0`; successful/old headers remain quiet. |
| C4 - framebuffer self-test FAIL state | PASS | `phase_indicator::fail_text` maps `log sink selftest failed` to `FAIL LOG SELFTEST`. Runtime display verification awaits hardware. |
| C5 - no QEMU regression | PASS | AHCI and NVMe NeoFS read/write QEMU scenarios passed without `--bare-metal-mode`; self-test path is cfg-gated. |
| C6 - bare-metal-mode ISOs rebuild cleanly | PASS | AHCI and NVMe bare-metal build-only commands completed and produced ISOs/raws. The kernel build still emits the pre-existing warning set; no warnings were introduced by the new dispatch code after fixes. |
| C7 - close report | PASS | This file provides the required layout, changed files, framebuffer state, verification, and runtime caveat. |

## Header Layout

| Offset | Size | Field |
|---:|---:|---|
| `0x000` | 16 | `magic = b"NEO_LOG_v1\0"` NUL-padded |
| `0x010` | 8 | `region_size_bytes: u64` |
| `0x018` | 8 | `write_cursor: u64` |
| `0x020` | 32 | `kernel_build_id: [u8; 32]` |
| `0x040` | 64 | `scenario_name: [u8; 64]` |
| `0x080` | 8 | `start_timestamp_ns: u64` |
| `0x088` | 4 | `header_version: u32 = 2` |
| `0x08c` | 4 | `degradation_reason: u32` (`0 none`, `1 Io`, `2 InvalidRequest`, `3 Unsupported`, `4 Other`) |
| `0x090` | 4 | `degradation_op: u32` (`0 idle`, `1 read_block_for_rmw`, `2 write_block_payload`, `3 write_block_header_rewrite`, `4 device_flush_after_write`) |
| `0x094` | 4 | reserved zero / alignment |
| `0x098` | 8 | `degradation_lba: u64` |
| `0x0a0` | 8 | `degradation_cursor: u64` |
| `0x0a8` | rest | reserved zero |
| `0x1000` | variable | log payload |

## New Framebuffer State

`log sink selftest failed` renders as `FAIL LOG SELFTEST`.

## Files Changed

| File | Approx. delta |
|---|---:|
| `kernel/src/log/block_sink.rs` | +258 / -44 |
| `kernel/src/test_harness.rs` | +103 / -2 |
| `kernel/src/scenario/phase_indicator.rs` | +1 / -0 |
| `tools/neo-extract-log/src/main.rs` | +151 / -12 |

## Verification

Commands run:

```powershell
docker compose run --rm neo cargo test -p neo-extract-log
docker compose run --rm neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite
docker compose run --rm neo cargo neo-test phase3_nvme_neofs_mount_and_readwrite
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite --bare-metal-mode
docker compose run --rm neo cargo neo-test --build-only phase3_nvme_neofs_mount_and_readwrite --bare-metal-mode
```

Results:

- `neo-extract-log`: 6 tests passed.
- AHCI QEMU scenario: PASS.
- NVMe QEMU scenario: PASS.
- AHCI bare-metal build-only: PASS.
- NVMe bare-metal build-only: PASS.

Bare-metal artifacts:

| Artifact | Size |
|---|---:|
| `phase3_ahci_neofs_mount_and_readwrite.bare-metal.iso` | 24,750,080 bytes |
| `phase3_ahci_neofs_mount_and_readwrite.ahci.raw` | 134,217,728 bytes |
| `phase3_ahci_neofs_mount_and_readwrite.nvme-log.raw` | 301,989,888 bytes |
| `phase3_nvme_neofs_mount_and_readwrite.bare-metal.iso` | 24,750,080 bytes |
| `phase3_nvme_neofs_mount_and_readwrite.nvme.raw` | 134,217,728 bytes |
| `phase3_nvme_neofs_mount_and_readwrite.ahci-log.raw` | 301,989,888 bytes |

These match the expected prior-dispatch scale: about 24.7 MB ISO, 128 MiB scenario raw, and 288 MiB log-sink raw.

## Runtime Caveat

Codex cannot exercise the strict-halt self-test failure or the real degradation-marker path in QEMU because the observed failure only manifested on real OptiPlex hardware. BUILD-5b re-execution should confirm C1/C2/C4 at runtime and determine which failing primitive is captured in the degradation fields.
