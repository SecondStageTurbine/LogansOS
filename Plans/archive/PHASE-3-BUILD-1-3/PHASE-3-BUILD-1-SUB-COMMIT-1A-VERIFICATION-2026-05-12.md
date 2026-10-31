# Phase 3 BUILD-1 Sub-Commit 1A Verification

Date: 2026-05-12  
Lane: Jarvis / Codex  
Branch: `codex/phase-3-build-1-jarvis`

## Commit / working-tree note

Working-tree note prepared before the 1A commit, per dispatch allowance for a not-yet-committed verification artifact.

Suggested title:

```text
kernel/storage: scaffold block substrate, DMA buffers, and virtio PCI probe
```

## Files changed

- `kernel/src/drivers/block/mod.rs`
- `kernel/src/drivers/block/dma.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `kernel/src/drivers/pci.rs`
- `kernel/src/drivers/mod.rs`
- `kernel/src/main.rs`
- `kernel/src/test_harness.rs`
- `kernel/Cargo.toml`
- `tools/xtask/src/main.rs`

## Summary

- Added the Phase 3 block substrate with `BlockDevice`, request/result types, and a registry.
- Added `DmaBuffer`, an identity-DMA abstraction returned and submitted as `Arc<DmaBuffer>`.
- Added a PCI config-space walker using CF8/CFC that recognizes modern virtio-blk by `(0x1af4, 0x1042)`, refuses legacy virtio-blk `(0x1af4, 0x1001)`, and logs AHCI `0x010601` / NVMe `0x010802` forward markers.
- Added modern virtio PCI vendor-capability parsing and BAR MMIO mapping through `mm::map_kernel_mmio_page`.
- Added a probe-only virtio-blk feature log. The 1A path does not submit virtqueue requests or claim read/write success.
- Added xtask raw deterministic disk creation and QEMU attachment for Phase 3 storage scenarios.
- Added `phase3_pci_virtio_blk_probe_smoke`.

## Verification commands

| Command | Result | Notes |
|---|---|---|
| `cargo k` | PASS | Kernel/userspace build completed. Existing warnings remain; new scaffold APIs are intentionally unused until 1B/1C. |
| `cargo k --no-default-features` | PASS | Kernel/userspace build completed. |
| `cargo neo-test phase3_pci_virtio_blk_probe_smoke` | BLOCKED | Host cannot assemble ISO: `Limine directory /home/neo/limine does not exist. Set $LIMINE_DIR or build the dev container.` |
| `cargo neo-test all` | BLOCKED | Same host Limine dependency; first scenario fails before QEMU launch. |
| `cargo fmt --check` | BLOCKED | `cargo-fmt.exe` is not installed for `nightly-2026-04-16-x86_64-pc-windows-msvc`. |

## Serial logs

- Expected new scenario log: `target/neo-test/phase3_pci_virtio_blk_probe_smoke.serial.log`
- Not produced on this host because ISO assembly blocked before QEMU launch.

## F_FLUSH feature log

Runtime log is blocked by missing Limine. The probe-only driver emits this exact format when QEMU runs:

```text
[ kernel ] virtio-blk: feature-policy bdf=BB:DD.F offered=0x... F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false attach=probe-only
```

## Abandoned-I/O leak result

Not applicable to 1A. No virtqueue requests are submitted in this sub-commit.

## aarch64 lane

Not applicable to 1A. The aarch64 barrier lane remains a BUILD-1 closure gate.

## Deviations

- QEMU verification could not run on the Windows host because the Limine ISO assembly dependency is absent. This matches earlier local verification posture in this repo; Docker or a host with `LIMINE_DIR` and `xorriso` is required for the QEMU gates.
