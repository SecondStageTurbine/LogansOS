# Phase 3 VMware Scenario Harness Close Evidence

**Date:** 2026-05-20  
**Dispatch:** `Plans/DISPATCH-2026-05-20-phase-3-vmware-scenario-harness-jarvis.md`

## 1. Observed Harness Shape

- `tools/xtask/src/main.rs` owns `cargo neo-test`: scenario selection, userspace/kernel feature builds, ISO assembly, deterministic fixture disk generation, and QEMU launch.
- Existing scenario ISOs were always assembled at `boot/neoplutarch.iso`; there was no "build without QEMU" export path under `target/`.
- AHCI and NVMe QEMU scenarios intentionally used virtio filler disks so AHCI registered as `device3` and NVMe as `device4`.
- `kernel/src/test_harness.rs` encoded those QEMU indexes directly in AHCI/NVMe device lookup and mount sources.
- `kernel/src/fs/mod.rs` accepted only enumerated mount sources for NeoFS/FAT32/ext4, which made dynamic AHCI/NVMe registry indexes leaky at the mount boundary.
- `MountedFs` already accepted NeoFS, FAT32, and ext4 without trait modification. No filesystem trait change was needed.

## 2. Changes Made

- Added `cargo neo-test --build-only <scenario>` and `cargo xtask neo-test-build <scenario>`.
- The build-only path builds the scenario ISO, copies it to `target/neo-test/vmware/<scenario>/<scenario>.iso`, generates deterministic fixture disks, converts raw disks to VMDK when `qemu-img` is present, writes `manifest.txt`, and prints `NEO_VMWARE_SCENARIO/1` plus `NEO_VMWARE_DISK/1` frames.
- The manifest includes repo-relative ISO/raw/VMDK paths for Windows VMware use when the build is run inside Docker and absolute paths are reported as `/workspace/...`.
- Added `find_device_by_driver(driver)` to the block registry so AHCI/NVMe scenarios can locate their registered backend by identity.
- Updated AHCI/NVMe scenario helpers to discover the `ahci` or `nvme` block device dynamically and construct `deviceN` mount sources from the registry index.
- Replaced enumerated mount-source handling with bounded `deviceN` parsing in `fs/mod.rs`; NeoFS still preserves empty-source default to `device0`.
- Updated `phase3_mount_request_validation` to distinguish malformed source syntax (`EINVAL`) from a well-formed but missing `deviceN` backend (`ENOENT`).
- Added `Plans/PROCEDURE-2026-05-20-phase-3-vmware-scenario-harness.md`.
- Lock hygiene: the only new lock acquisition is inside `find_device_by_driver`, using the existing `REGISTRY: Lazy<SwitchMutex<Registry>>` declaration in `kernel/src/drivers/block/mod.rs`. No new lock type or declaration was added.

## 3. How To Use The New VMware Scenario Path

Build an AHCI exemplar bundle:

```bash
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite
```

Observed summary:

```text
NEO_VMWARE_SCENARIO/1 scenario=phase3_ahci_neofs_mount_and_readwrite iso=/workspace/target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/phase3_ahci_neofs_mount_and_readwrite.iso manifest=/workspace/target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/manifest.txt
NEO_VMWARE_DISK/1 scenario=phase3_ahci_neofs_mount_and_readwrite qemu_index=3 raw=/workspace/target/neo-test/phase3_ahci_neofs_mount_and_readwrite.ahci.raw vmdk=/workspace/target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/device3.vmdk controller=sata-ahci
```

For VMware, attach the printed ISO as the CD/DVD boot device, attach the printed scenario VMDK to the indicated controller, omit `role=qemu-filler` disks, and capture serial output to a file. The expected terminal frame is the scenario's `NEO_TEST/1 event=PASS`.

## 4. Verification Commands And Results

Container-canonical tool check:

```bash
docker compose run --rm neo bash -lc 'which qemu-img && qemu-img --version | head -n 1'
```

Result: `/usr/bin/qemu-img`, version `7.2.22`.

Build-only export:

```bash
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite
```

Result: PASS; ISO, manifest, raw disks, and VMDKs produced under `target/neo-test`.

Focused QEMU subset:

```bash
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_ahci_controller_detected --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_nvme_controller_detected --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_nvme_neofs_mount_and_readwrite --no-meta
```

Result: all four PASS. Serial PASS evidence included:

```text
phase3_ahci_controller_detected controller=ich9-ahci ... msi_configured=true
phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device3 mount=rw ... bytes=30
phase3_nvme_controller_detected controller=qemu-nvme ... msix_configured=true msi_configured=false
phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device4 mount=rw ... bytes=30
```

PM validation addendum:

```bash
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_mount_request_validation --no-meta
docker compose run --rm neo cargo check -p xtask
docker compose run --rm neo rustfmt --check --config skip_children=true tools/xtask/src/main.rs kernel/src/fs/mod.rs kernel/src/drivers/block/mod.rs
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite
docker compose run --rm neo cargo neo-test --build-only phase3_nvme_neofs_mount_and_readwrite
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite phase3_nvme_neofs_mount_and_readwrite --no-meta
```

Result: all PASS. PM validation caught and fixed one adjacent contract drift:
`phase3_mount_request_validation` had still treated `device7` as malformed
source syntax, but generic `deviceN` parsing now makes that a well-formed missing
backend. The scenario now asserts `devicex -> EINVAL` and `device7 -> ENOENT`.
Serial PASS evidence:

```text
phase3_mount_request_validation bad_source=EINVAL missing_source=ENOENT
phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device3 mount=rw ... bytes=30
phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device4 mount=rw ... bytes=30
```

Manual VMware exemplar addendum:

PL manually booted the generated bundles in VMware Workstation Pro using BIOS
firmware, 4 vCPU, 2048 MiB RAM, and file-backed serial output. For each run,
only the scenario VMDK was attached; qemu-filler disks were omitted.

AHCI bundle:

- ISO: `target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/phase3_ahci_neofs_mount_and_readwrite.iso`
- Scenario VMDK: `target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/device3.vmdk`
- Serial: `target/vmware-logs/phase3-ahci-neofs-serial.log`
- Result: PASS.

```text
[ kernel ] pci: AHCI controller 02:01.0 vendor=0x15ad device=0x07e0 class=0x010601
[ kernel ] block: registered device index=0 driver=ahci name=ahci0p0 pci_bdf=Some((2, 1, 0))
[ kernel ] block: pci scan complete functions=43 virtio_blk=0 ahci=1 ahci_ok=1 nvme=1 nvme_ok=0 attached_devices=1
NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30
```

NVMe bundle:

- ISO: `target/neo-test/vmware/phase3_nvme_neofs_mount_and_readwrite/phase3_nvme_neofs_mount_and_readwrite.iso`
- Scenario VMDK: `target/neo-test/vmware/phase3_nvme_neofs_mount_and_readwrite/device4.vmdk`
- Serial: `target/vmware-logs/phase3-nvme-neofs-serial.log`
- Result: PASS.

```text
[ kernel ] pci: NVMe controller 03:00.0 vendor=0x15ad device=0x07f0 class=0x010802
[ kernel ] block: registered device index=0 driver=nvme name=nvme0n1 pci_bdf=Some((3, 0, 0))
[ kernel ] nvme: attached bdf=03:00.0 nsid=1 sectors=262144 sector_size=512 doorbell_stride=4 msix=true msi=false
[ kernel ] block: pci scan complete functions=43 virtio_blk=0 ahci=1 ahci_ok=0 nvme=1 nvme_ok=1 attached_devices=1
NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30
```

These two VMware passes prove the scenario harness now handles VMware's
controller-specific one-disk layout dynamically: AHCI and NVMe both registered
as `device0` in their single-scenario VMware runs, while the same scenarios
continue to PASS under QEMU's filler-disk layout as `device3`/`device4`.

Formatting:

```bash
docker compose run --rm neo rustfmt --check --config skip_children=true tools/xtask/src/main.rs kernel/src/fs/mod.rs kernel/src/drivers/block/mod.rs
```

Result: PASS.

Xtask compile check:

```bash
docker compose run --rm neo cargo check -p xtask
```

Result: PASS.

`cargo fmt --check` and a scoped check including `kernel/src/test_harness.rs` are not clean because of pre-existing unrelated formatter drift near imports and older helper blocks. The modified AHCI/NVMe section compiled and passed the targeted QEMU scenarios.

Full `docker compose run --rm neo cargo neo-test --no-meta` was not run; the dispatch allowed the targeted subset when a full sweep was too slow for this lane.

## 5. Residual Gaps

- Full manual VMware scenario sweep is not complete; two controller-specific
  exemplars PASSed manually under VMware.
- No VMware automation or CI runner was introduced.
- Non-AHCI/NVMe scenario bundles are exported as `qemu-layout`; VMware attachment order is not validated for those classes in this change.
- The build-only path still uses the existing ISO assembly function, so `boot/neoplutarch.iso` is updated as a build side effect before the scenario ISO is copied under `target/`.
- Generated ISOs, raw disks, VMDKs, and serial logs remain untracked under `target/`.

## 6. Recommendation

Do not open the full VMware scenario matrix yet. The harness lane's goal is met:
build-only exports exist, AHCI/NVMe dynamic block-device discovery works under
both QEMU and VMware, and the manual controller exemplars have serial PASS
evidence. Broaden to additional VMware scenarios only when a specific risk or
release gate needs more hypervisor coverage.
