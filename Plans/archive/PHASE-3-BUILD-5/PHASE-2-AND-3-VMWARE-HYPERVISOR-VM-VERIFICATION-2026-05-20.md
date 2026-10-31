# Phase 2 + Phase 3 Hypervisor-VM Verification (VMware Workstation)

**Date:** 2026-05-20
**Executor:** PL
**Recorded by:** PM
**Substrate HEAD at verification:** `aa52579f51f9a7abaa6de972e658b63db6fc5047`
**Verification environment:** VMware Workstation Pro `25.0.1.25219725`; host OS Microsoft Windows 11 Pro `10.0.26200`; host CPU Intel(R) Core(TM) i9-9900K CPU @ 3.60GHz; host RAM 127.9 GiB.
**VM config:** BIOS firmware, CD/DVD boot from `production-aa52579.iso`, 1 GiB SATA virtual disk, 1 GiB NVMe virtual disk, serial output to `target/vmware-logs/*.log`. Primary run used 4 vCPU and 2048 MiB RAM; CPU triangulation also covered 1 vCPU and 8 vCPU.
**Status:** PASS-WITH-SCOPE.

Scope note: this report records the production-ISO VMware smoke after `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification-ADDENDUM.md`. The full test-harness scenario/stress sweep is intentionally not claimed here. A follow-on harness lane added VMware scenario-bundle export support, and the AHCI/NVMe controller-specific exemplars now have manual VMware PASS evidence.

---

## 1. Build

- `docker compose run --rm neo cargo iso`: PASS.
- ISO used for the successful run: `target/vmware-isos/production-aa52579.iso`.
- Build warnings: existing unused/dead-code warnings only; no build failure.

## 2. Production-ISO boot

Observed serial highlights:

- Limine handoff: PASS. Kernel and both modules loaded.
- Kernel banner: PASS.
- BSP online / x86 setup: PASS. SSE/SSE2 ready; GDT, IDT, SYSCALL/IRETQ initialized.
- AP bring-up: PASS. Serial observed `smp: started 3 AP(s)` under 4-vCPU VM.
- Scheduler smoke: PASS. Serial observed `sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=10)`.
- Userspace smoke: PASS. `neo-init` spawned `worker`, IPC heap ping/pong completed, blocked-worker kill path completed.
- `neo-init` exit code: PASS. Serial observed `proc: neo-init exit code 0`.
- Frame audit: PASS. Serial observed `frame: leak audit clean (baseline=1037 final=1037)`.
- Framebuffer render: PASS. Serial observed `boot sequence rendered (1024x768)`.
- KERNEL PANIC: none observed.
- BUG: none observed.

## 3. Storage Detection

AHCI:

- PASS. VMware exposed AHCI controller `02:01.0`, vendor `0x15ad`, device `0x07e0`, class `0x010601`.
- PASS. Kernel attached AHCI port 0: `sectors=2097152`, `sector_size=512`, `msi_configured=true`.
- PASS. Block registry registered `device index=0`, `driver=ahci`, `name=ahci0p0`, `pci_bdf=Some((2, 1, 0))`.
- PASS. PCI scan summary, with AHCI and NVMe both configured: `functions=43 virtio_blk=0 ahci=1 ahci_ok=1 nvme=1 nvme_ok=1 attached_devices=2`.

NVMe:

- PASS. VMware exposed NVMe controller `03:00.0`, vendor `0x15ad`, device `0x07f0`, class `0x010802`.
- PASS. Block registry registered `device index=1`, `driver=nvme`, `name=nvme0n1`, `pci_bdf=Some((3, 0, 0))`.
- PASS. Kernel attached NVMe namespace 1: `sectors=2097152`, `sector_size=512`, `doorbell_stride=4`, `msix=true`, `msi=false`.
- PASS. The scan summary recorded `nvme=1 nvme_ok=1`.

Virtio:

- NOT APPLICABLE. VMware Workstation does not expose virtio-blk by default; this smoke intentionally exercised AHCI and NVMe instead.

## 4. Scenario Sweep

Full sweep deferred per `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification-ADDENDUM.md`.

Reason at production-smoke time: the `cargo neo-test` path built one scenario-specific ISO at a time and prepared QEMU fixture disks. Several Phase 3 scenarios also expected QEMU device-index layouts using virtio filler disks (`device3` for AHCI, `device4` for NVMe). VMware does not reproduce that layout through ordinary UI-created SATA/NVMe disks.

Follow-on update: `Plans/PHASE-3-VMWARE-SCENARIO-HARNESS-CLOSE-EVIDENCE-2026-05-20.md` records the harness fix and two manual VMware scenario exemplars.

AHCI exemplar:

- ISO: `target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/phase3_ahci_neofs_mount_and_readwrite.iso`
- Scenario VMDK: `target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/device3.vmdk`
- Serial: `target/vmware-logs/phase3-ahci-neofs-serial.log`
- PASS frame: `NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30`

NVMe exemplar:

- ISO: `target/neo-test/vmware/phase3_nvme_neofs_mount_and_readwrite/phase3_nvme_neofs_mount_and_readwrite.iso`
- Scenario VMDK: `target/neo-test/vmware/phase3_nvme_neofs_mount_and_readwrite/device4.vmdk`
- Serial: `target/vmware-logs/phase3-nvme-neofs-serial.log`
- PASS frame: `NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30`

Both exemplar runs intentionally omitted qemu-filler disks. The same AHCI/NVMe
scenarios still PASS under QEMU as `device3`/`device4`; under VMware they PASS as
`device0`, proving dynamic driver-identity lookup and mount-source construction
for the controller-specific path.

## 5. Stress Runs

Deferred for the same reason as the scenario sweep.

## 6. CPU-Count Triangulation

- 1-vCPU production smoke: PASS. Serial observed `smp: Limine reports 1 cpu(s)`, `smp: started 0 AP(s)`, `ahci_ok=1 nvme_ok=1 attached_devices=2`, `sched: smoke test passed`, `proc: neo-init exit code 0`, and `frame: leak audit clean (baseline=1037 final=1037)`.
- 4-vCPU production smoke: PASS. Serial observed `smp: started 3 AP(s)`, `ahci_ok=1 nvme_ok=1 attached_devices=2`, `sched: smoke test passed`, `proc: neo-init exit code 0`, and `frame: leak audit clean`.
- 8-vCPU production smoke: PASS. Serial observed `smp: Limine reports 8 cpu(s)`, `smp: started 7 AP(s)`, `ahci_ok=1 nvme_ok=1 attached_devices=2`, `sched: smoke test passed`, `proc: neo-init exit code 0`, and `frame: leak audit clean (baseline=1037 final=1037)`.

## 7. Incidental Observations

- The first VMware production boot used an older remote-main-derived ISO and only reported an `AHCI forward marker`. PM corrected the Git integration state, pushed Phase 3 BUILD-4B through BUILD-5a storage code to remote main at `aa52579`, rebuilt the ISO, and reran.
- The first corrected run configured SATA only and validated AHCI. PL then added a VMware NVMe virtual disk and reran the same production ISO; the second corrected run validated both AHCI and NVMe.
- VMware's display went black after boot during the successful run, but serial output continued and reached clean halt after framebuffer render. Serial is authoritative for this verification.
- The successful run validates that the real AHCI and NVMe drivers attach under VMware Workstation's emulated devices.

## 8. Conclusion

VMware Workstation production-ISO smoke PASSes with scope across 1, 4, and 8 vCPU configurations: Phase 2 SMP boot/scheduler/userspace smoke remained clean, and Phase 3 AHCI plus NVMe block-device detection and registration succeeded under VMware hardware virtualization. Follow-on manual scenario exemplars also PASSed for AHCI-backed NeoFS read/write and NVMe-backed NeoFS read/write. This is a strong intermediate signal for both BUILD-5a storage paths, but it does not formally close Phase 3. Bible Article 16.3 bare-metal validation remains open, and the full scenario/stress sweep remains separate follow-on work.
