# Phase 3 VMware Clean-Copy Rerun Evidence

**Date:** 2026-05-21
**Recorder:** PM
**Executor:** PL
**Repository:** `E:\AI Automation\NeoPlutarch-clean`
**HEAD at bundle generation:** `dc47050407d0`
**Status:** PASS-WITH-SCOPE

## 1. Scope

This artifact records a clean-working-copy rerun of the VMware scenario-bundle
path for the two controller-specific NeoFS read/write scenarios:

- `phase3_ahci_neofs_mount_and_readwrite`
- `phase3_nvme_neofs_mount_and_readwrite`

This rerun confirms that the current clean repository can generate VMware-ready
AHCI and NVMe scenario bundles and that both pass under VMware Workstation Pro
when booted manually by PL.

This is intermediate hypervisor evidence. It does not close Phase 3 BUILD-5b;
Bible Article 16.3 bare-metal validation remains open.

## 2. Bundle Generation

AHCI bundle command:

```bash
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite
```

Observed exported paths:

```text
target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/phase3_ahci_neofs_mount_and_readwrite.iso
target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/device3.vmdk
```

NVMe bundle command:

```bash
docker compose run --rm neo cargo neo-test --build-only phase3_nvme_neofs_mount_and_readwrite
```

Observed exported paths:

```text
target/neo-test/vmware/phase3_nvme_neofs_mount_and_readwrite/phase3_nvme_neofs_mount_and_readwrite.iso
target/neo-test/vmware/phase3_nvme_neofs_mount_and_readwrite/device4.vmdk
```

Both commands completed successfully and generated ISO, manifest, raw disk, and
VMDK outputs under `target/neo-test/vmware/`.

## 3. AHCI VMware Result

PL booted the AHCI scenario ISO with the scenario VMDK attached as a VMware SATA
disk. The qemu-filler disks were not attached.

Serial output was captured to an older VM-configured path:

```text
E:\AI Automation\AltimitNeo-vmware-verify\target\vmware-logs\phase3-nvme-neofs-serial.log
```

The filename is stale, but the serial payload is authoritative. Observed PASS
frame:

```text
NEO_TEST/1 event=BEGIN name=phase3_ahci_neofs_mount_and_readwrite phase=dispatch
NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30
```

## 4. NVMe VMware Result

PL then booted the NVMe scenario ISO with the scenario VMDK attached as a VMware
NVMe disk.

Serial output path:

```text
E:\AI Automation\NeoPlutarch-clean\target\neo-test\vmware\phase3_nvme_neofs_mount_and_readwrite\serial-nvme.log
```

Observed PASS frame:

```text
NEO_TEST/1 event=BEGIN name=phase3_nvme_neofs_mount_and_readwrite phase=dispatch
NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30
```

VMware log also confirmed the corrected NVMe setup:

```text
ide1:0.fileName = "E:\AI Automation\NeoPlutarch-clean\target\neo-test\vmware\phase3_nvme_neofs_mount_and_readwrite\phase3_nvme_neofs_mount_and_readwrite.iso"
nvme0:0.fileName = "E:\AI Automation\NeoPlutarch-clean\target\neo-test\vmware\phase3_nvme_neofs_mount_and_readwrite\device4.vmdk"
serial0.fileName = "E:\AI Automation\NeoPlutarch-clean\target\neo-test\vmware\phase3_nvme_neofs_mount_and_readwrite\serial-nvme.log"
NVME-CORE:Successfully created device: nvme0:0
```

## 5. Notes

- The first NVMe attempt booted the AHCI ISO by mistake and failed as expected
  with `reason=ahci_probe_not_ok`; PL corrected the CD/DVD ISO and reran.
- The NVMe VM configuration still had the previous SATA AHCI disk present, but
  the NVMe scenario passed through driver-identity lookup and mounted the NVMe
  backend as `device0`.
- Generated ISOs, VMDKs, and serial logs remain under `target/` and are not
  tracked source artifacts.

## 6. Conclusion

The clean repository's VMware scenario-bundle path is verified for the two
Phase 3 controller-specific NeoFS read/write exemplars. AHCI and NVMe both pass
under VMware Workstation Pro with scenario disks attached through their intended
controller classes.

Phase 3 remains substantively complete in QEMU plus VMware hypervisor evidence.
Formal Phase 3 closure still waits for BUILD-5b bare-metal validation.
