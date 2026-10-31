# ADDENDUM - VMware Workstation Phase 2 + Phase 3 verification procedure

**Date filed:** 2026-05-20
**Author:** PM
**Type:** Operator addendum / erratum to `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md`
**Status:** Active correction for PL execution.

---

## 1. Correction

The base procedure remains valid for the **production-ISO VMware boot smoke**.

The base procedure is **not currently sufficient for the full test-harness scenario sweep** as written.

Two implementation facts in the current tree require this correction:

1. `cargo iso` always builds the production ISO at `boot/neoplutarch.iso`. The current `xtask iso` command does not accept `--features neo-test`.
2. The integration harness is scenario-specific. `cargo neo-test <scenario>` builds one scenario ISO, assembles it at `boot/neoplutarch.iso`, prepares QEMU fixture disks, and launches QEMU. There is no single all-scenarios VMware test ISO in the current tooling.

Additionally, the Phase 3 AHCI/NVMe composition scenarios are QEMU-layout-shaped:

- AHCI scenarios expect the AHCI-backed target at `device3`.
- NVMe scenarios expect the NVMe-backed target at `device4`.
- QEMU reaches those indices by attaching preceding virtio-blk fixture disks.
- VMware Workstation does not natively expose virtio-blk, so the same disk-index layout is not reproduced by simply adding SATA/NVMe disks in the VMware UI.

Therefore, a VMware scenario FAIL from the current manual procedure may reflect harness/device-layout mismatch rather than a kernel substrate failure.

## 2. Revised PL execution guidance

PL should execute the production-ISO VMware smoke now:

- build `boot/neoplutarch.iso`;
- create a BIOS VMware VM with serial-output-to-file;
- attach a SATA virtual disk and, if available, an NVMe virtual disk;
- confirm clean boot, AP bring-up, PCI enumeration, AHCI/NVMe detection, block-device registration, `neo-init` exit, and clean frame audit.

PL should **not** spend time attempting the full scenario/stress sweep in VMware until one of the following exists:

1. a VMware-specific harness path that can build named scenario ISOs and provide matching VMDK fixture disks with known registry indices; or
2. kernel test scenarios parameterized for VMware's available block-device ordering; or
3. an explicit PM/Jarvis dispatch that validates a repeatable raw-disk-to-VMDK conversion and controller ordering recipe.

## 3. PM disposition

This addendum narrows the immediate VMware procedure to a valuable production-boot/storage-detection smoke test. It does not invalidate the base motivation for hypervisor-VM validation, but it prevents the current harness mismatch from being misread as a kernel regression.

The full Phase 2 + Phase 3 VMware scenario sweep remains desirable, but it now requires a small tooling/harness lane before PL execution.

Bible Article 16.3 bare-metal validation remains unchanged and still gates formal Phase 3 closure.

