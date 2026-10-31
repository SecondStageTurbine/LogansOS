# Phase 3 VMware Scenario Harness Procedure

**Date:** 2026-05-20

This procedure covers the first repeatable path for building a named `neo-test`
scenario ISO and matching fixture disks for manual VMware execution. It does not
turn VMware into an automated CI runner.

## Build A Scenario Bundle

Run from the repository root inside the canonical dev container:

```bash
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite
```

Equivalent xtask spelling:

```bash
docker compose run --rm neo cargo xtask neo-test-build phase3_ahci_neofs_mount_and_readwrite
```

The command prints a stable summary frame:

```text
NEO_VMWARE_SCENARIO/1 scenario=<scenario> iso=<path> manifest=<path>
NEO_VMWARE_DISK/1 scenario=<scenario> qemu_index=<n> raw=<path> vmdk=<path> controller=<controller>
```

Outputs are under `target/neo-test/vmware/<scenario>/`. Raw disks remain under
`target/neo-test/`; when `qemu-img` is available, VMDKs are written next to the
manifest.

When built through `docker compose`, absolute paths in the summary and manifest
use the container's `/workspace/...` view. On the Windows VMware host, resolve
the manifest's `*_relative` fields under the repository root.

## AHCI Manual VMware Run

Use `phase3_ahci_neofs_mount_and_readwrite` as the first exemplar.

1. Create or reuse a BIOS VMware VM with a serial port configured to output to a file.
2. Attach the generated scenario ISO as the CD/DVD boot device.
3. Open the manifest and use the `disk.N.role=scenario` entry.
4. Attach that scenario disk's VMDK as a SATA/AHCI disk. On Windows, prefer the
   `disk.N.vmdk_relative` path resolved under the repository root.
5. Do not attach `role=qemu-filler` disks for the VMware exemplar.
6. Boot and inspect the serial log for:

```text
NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite
```

The scenario discovers the registered `ahci` block device and constructs the
mount source as `deviceN` at runtime, so VMware does not need to reproduce QEMU's
filler-disk index.

## NVMe Manual VMware Run

Use `phase3_nvme_neofs_mount_and_readwrite`.

1. Attach the generated ISO as the CD/DVD boot device.
2. Attach the manifest's `role=scenario` disk as an NVMe disk. On Windows,
   prefer the `disk.N.vmdk_relative` path resolved under the repository root.
3. Omit `role=qemu-filler` disks for the VMware exemplar.
4. Boot and inspect the serial log for:

```text
NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite
```

The scenario discovers the registered `nvme` block device and mounts the matching
`deviceN` source dynamically.

## Scope Notes

- AHCI/NVMe scenario bundles are the intended VMware path in this step.
- Non-AHCI/NVMe scenario classes may still export a bundle, but their manifest is
  marked `qemu-layout`; VMware attachment order for those classes is not validated here.
- Generated ISOs, disks, VMDKs, and serial logs stay under `target/` and are not
  source artifacts.
