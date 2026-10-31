# PROCEDURE — VMware Workstation hypervisor-VM verification of Phase 2 + Phase 3

**Date filed:** 2026-05-20
**Author:** PM
**Type:** Manual procedure document for PL direct execution. Not a dispatch — no agent involvement. PL executes at convenient cadence.
**Scope:** Combined hypervisor-VM verification of Phase 2 BUILD-3 SMP correctness AND Phase 3 substrate (NeoFS read-write, VFS + mount table, FAT32/ext4 foreign filesystems, AHCI driver, NVMe driver) under VMware Workstation Pro.
**Supersedes (in scope):** `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` (Phase 2 BUILD-3 only). This document covers Phase 2 + Phase 3 in one execution. The May 11 procedure remains available for Phase 2-only re-runs if ever wanted.
**Status:** Procedure ready for execution. Estimated 4-6 hours wall-clock total including build + Phase 2 sweep + Phase 3 sweep + composition stress + report.
**Bible mandate:** Article 16.3 bare-metal verification stays formally open. Hypervisor-VM is intermediate validation, not formal Phase 3 closure.

---

## 1. Why hypervisor-VM verification (Phase 2 + Phase 3)

QEMU TCG is software emulation. VMware Workstation Pro uses hardware virtualization (VT-x / AMD-V). The two environments differ in ways that matter for both Phase 2 SMP correctness AND Phase 3 substrate:

**Phase 2 surfaces (per May 11 procedure):**

- **CPU instruction execution.** QEMU TCG dynamically translates; VMware runs guest instructions directly on physical CPU. Instruction-sequence-specific bugs (memory ordering edges, FPU/SSE state transitions, exotic interrupt timings) surface here.
- **Memory model.** QEMU TCG memory model is permissive (effectively sequential consistency); VMware exposes real x86_64 TSO. SMP correctness issues QEMU masks become observable.
- **APIC and timing.** VMware passes through real LAPIC/TSC/APIC-timer behavior; QEMU emulates.
- **Performance.** VMware runs kernel speed; QEMU TCG is ~10-50x slower. Timing-sensitive race surfaces shift.

**Phase 3 substrate surfaces (new in this procedure):**

- **AHCI emulation differs.** VMware's AHCI controller is closer to real ICH9 silicon than QEMU's ICH9. The dual-controller quirk Jarvis worked around in BUILD-5a-AHCI may behave differently. PCI BAR enumeration, port command list management, ATA completion timing all may surface edges QEMU masked.
- **NVMe emulation differs.** VMware's NVMe controller has different interrupt-timing characteristics. **The MSI-X-before-APIC ordering bug Jarvis fixed in BUILD-5a-NVMe is exactly the kind of issue VMware tests most aggressively.** Real hardware (BUILD-5b) is expected to be even less tolerant than QEMU; VMware sits between them.
- **No virtio devices by default.** VMware does not natively expose virtio-blk. The VM must be configured with AHCI or NVMe virtual disks, so the storage path under VMware exclusively exercises BUILD-5a-AHCI/NVMe drivers (not the virtio-blk legacy path). This is intentional and useful: it isolates the new substrate.
- **Storage completion timing.** Under VMware's real-CPU timing, the synchronous-polling completion paths (intentionally landed in BUILD-5a-AHCI/NVMe with MSI configured-but-unused) behave under realistic load. Any latent ordering assumption that depends on QEMU's slow emulation may surface.
- **Real TSO under storage I/O.** Phase 3's lock hygiene (SwitchMutex sites with IF discipline) is exercised under real memory ordering, not TCG permissiveness.

This is NOT bare-metal verification. Bible Article 16.3 bare-metal mandate stays open. Hypervisor-VM verification is a valuable additional gate that catches different bugs than both QEMU TCG and bare-metal.

## 2. Prerequisites

PL host system requirements:

- VMware Workstation Pro (or Player; Pro is preferred for serial-output-to-file convenience) installed.
- VT-x or AMD-V enabled in host BIOS.
- ~4 GB free disk space (kernel ISO + VM config + virtual disks + serial logs).
- NeoPlutarch source tree at `E:\AI Automation\AltimitNeo\` accessible.

No additional software needed; the existing `cargo iso` build target produces a bootable ISO.

Substrate state required: BUILD-5a-NVMe commits (`eba73ac` + close-evidence) pushed to local main. Per
`Plans/PM-DISPOSITION-2026-05-20-phase-3-build-5a-NVMe-close-consumption.md`, push is authorized. Confirm
local main is at or after `eba73ac` before building the ISO.

## 3. Procedure

### Step 1 — Verify substrate HEAD

```powershell
cd 'E:\AI Automation\AltimitNeo'
git log --oneline main -5
git rev-parse main
```

Expected HEAD: at or after `eba73ac` (BUILD-5a-NVMe implementation). Note the actual HEAD for the verification report.

### Step 2 — Build the kernel ISOs

Two ISO variants are needed for full coverage:

**2.a — Production ISO** (no test harness; verifies clean boot):

```powershell
docker compose run --rm neo cargo iso
```

Output: `target/iso/NeoPlutarch.iso` (or similar; verify path in build output).

**2.b — Test-harness ISO** (scenario sweep):

```powershell
docker compose run --rm neo cargo iso --features neo-test
```

Output: `target/iso/NeoPlutarch-neo-test.iso` (or similar path).

Expected build times: 2-5 minutes each. Note any warnings or non-fatal output.

### Step 3 — Create VMware VM (production-ISO boot first)

**VM specifications:**

- **Type:** Custom VM.
- **OS Type:** Other → Other 64-bit.
- **CPU count:** 4 (matches `NEO_SMP=4` baseline; additional 1-CPU and 8-CPU triangulation per Step 8).
- **Memory:** 1024 MB (baseline; can be increased to 2048 MB without behavioral change).
- **Storage configuration (CRITICAL):**
  - **Do NOT use VMware's default SCSI controller (LSI Logic / paravirtual).** NeoPlutarch does not have a SCSI driver. Default VM creation will produce a SCSI disk that is invisible to the kernel.
  - **Add a SATA controller** and attach a SATA virtual disk (1 GB; raw or sparse).
  - **Optionally add an NVMe controller** (VMware Workstation supports this since version 14+) with an NVMe virtual disk (1 GB).
  - Both controllers can coexist. The kernel enumerates whichever it finds at boot.
- **Network:** Not needed (Phase 4+).
- **CD/DVD:** Mount the kernel ISO from Step 2 as bootable.
- **Display:** Standard, no acceleration needed.
- **Sound:** Disable.
- **USB controller:** Disable.
- **Serial port (CRITICAL):** Add an "output to file" serial port pointing at, e.g., `vmware-serial.log`. Kernel println goes to serial; the verification depends on capturing this output.

**Boot order:** CD-ROM first, then disk. Once the production-ISO smoke test passes, the same VM can be reconfigured to boot the test-harness ISO.

### Step 4 — Production-ISO boot (Phase 2 + Phase 3 smoke)

Mount the production ISO (Step 2.a output). Start the VM. Observe serial output:

**Expected boot sequence:**

- Limine boot banner + kernel handoff.
- `[ kernel ] NeoPlutarch v...` banner.
- `[ kernel ] arch: BSP CPU online`.
- `[ kernel ] mm: heap...` allocator init.
- `[ kernel ] traps: BSP IDT loaded`.
- `[ kernel ] per_cpu: cpu_id=0 gs:[24]=0` (BSP GS-base sentinel).
- AP discovery + bring-up: `[ kernel ] sched: AP cpu_id=N online` for each of the 4 vCPUs.
- `[ kernel ] sched: smp_test PASS` or equivalent.
- **`[ kernel ] pci: enumerating devices...`**.
- **`[ kernel ] ahci: controller detected bdf=...`** (if SATA controller was added).
- **`[ kernel ] nvme: controller detected bdf=...`** (if NVMe controller was added).
- **`[ kernel ] block: registered device_N=...`** for each block device discovered.
- `[ neo-init ] ...` userspace init.
- `[ neo-init ] neo-init exit code 0`.
- `[ kernel ] frame: leak audit clean`.

**Phase 3-specific indicators to capture for the verification report:**

- AHCI controller detection: which PCI bdf, which BAR, which port count.
- NVMe controller detection: which PCI bdf, which BAR, queue setup completion.
- MSI/MSI-X configuration outcome (msi_configured / msix_configured booleans, if logged).
- Block device registry final count.

**Indicators of failure:**

- `KERNEL PANIC` (immediate fail; capture full panic + backtrace).
- `BUG:` assertion failure (capture context).
- Hang for >60 seconds with no progress on serial.
- AP bring-up failure (only BSP reaches `sched:` lines).
- **AHCI controller not detected** when SATA disk was configured (Phase 3 substrate failure).
- **NVMe controller not detected** when NVMe disk was configured (Phase 3 substrate failure).
- **APIC-base assertion** during PCI enumeration or storage init — would indicate MSI-X-before-APIC ordering regression (the bug Jarvis fixed in BUILD-5a-NVMe).
- **GS-base sentinel mismatch** panic (Phase 2 substrate regression).

### Step 5 — Test-harness ISO boot + scenario sweep

Replace the boot ISO with the test-harness variant (Step 2.b output). Boot the VM again.

Scenarios run automatically. Tally PASS/FAIL per scenario for the report.

**Phase 2 scenarios (per May 11 procedure §3 Step 5):**

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| smp_per_cpu_scheduler_smoke | | | |
| smp_same_cpu_ipc | | | |
| smp_cross_cpu_ipc_smoke | | | |
| smp_cross_cpu_kill_blocked | | | |
| smp_cross_cpu_rendezvous_stress | | | |
| q7b_cr3_cache_skip_count | | | |
| q7c_concurrent_munmap_smp | | | |
| q7c_process_exit_during_thread_run | | | |
| q7c_missing_shootdown_detection | | | |
| q8a_concurrent_install_revoke | | | |
| q8b_grant_vs_exit_race | | | |
| q8b_exit_state_cas | | | |
| q8c_blocked_kill_walk | | | |
| q8c_close_peer_my_side_extract | | | |
| q8d_phase_a_coverage | | | |
| ipc_heap_kill_smoke | | | |
| cap_revoke_local | | | |

**Phase 3 scenarios (new; added in this procedure):**

NeoFS:

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| phase3_mount_neofs_device0_readonly | | | requires SATA or NVMe disk at device0 |
| phase3_neofs_readdir_root | | | |
| phase3_neofs_filecap_generation_stale | | | |
| phase3_neofs_fsync_filecap_stale_regression | | | |

VFS / mount table:

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| phase3_vfs_directory_read_is_eisdir | | | |
| phase3_mount_requires_mount_right | | | |
| phase3_umount_busy_is_ebusy | | | |
| phase3_sched_wake_token_abandon | | | |

FAT32:

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| phase3_fat32_mount_readonly | | | |
| phase3_fat32_readdir_root | | | |
| phase3_fat32_open_read_regular_file | | | |
| phase3_fat32_lfn_lookup | | | |
| phase3_fat32_subdir_walk | | | |
| phase3_fat32_write_is_rejected | | | |
| phase3_fat32_exec_blocked_by_default | | | |
| phase3_fat32_exec_with_opt_in | | | |
| phase3_fat32_type_import_is_regular | | | |

ext4:

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| phase3_ext4_mount_readonly | | | |
| phase3_ext4_readdir_root | | | |
| phase3_ext4_open_read_regular_file | | | |
| phase3_ext4_subdir_walk | | | |
| phase3_ext4_extent_chain_read | | | 50-extent fixture |
| phase3_ext4_write_is_rejected | | | |
| phase3_ext4_exec_blocked_by_default | | | |
| phase3_ext4_exec_with_opt_in | | | |
| phase3_ext4_type_import_is_regular | | | xattr-ignore |

AHCI:

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| phase3_ahci_controller_detected | | | requires SATA virtual disk |
| phase3_ahci_port_identify | | | |
| phase3_ahci_read_sector_pattern | | | |
| phase3_ahci_write_sector_pattern | | | |
| phase3_ahci_multi_sector_io | | | |
| phase3_ahci_block_device_registered | | | |
| phase3_ahci_fat32_mount_and_read | | | composition test |
| phase3_ahci_neofs_mount_and_readwrite | | | composition test, read+write durability |

NVMe:

| Scenario | PASS | FAIL | Notes |
|---|---|---|---|
| phase3_nvme_controller_detected | | | requires NVMe virtual disk |
| phase3_nvme_namespace_identify | | | |
| phase3_nvme_read_sector_pattern | | | |
| phase3_nvme_write_sector_pattern | | | |
| phase3_nvme_multi_sector_io | | | PRP chain |
| phase3_nvme_block_device_registered | | | |
| phase3_nvme_fat32_mount_and_read | | | composition test |
| phase3_nvme_neofs_mount_and_readwrite | | | composition test, MSI-X ordering surface |

**Note on Phase 3 scenario PASS criteria:**

The composition tests (`phase3_ahci_neofs_mount_and_readwrite` and `phase3_nvme_neofs_mount_and_readwrite`) are the load-bearing scenarios for hypervisor-VM verification of Phase 3 substrate. Both exercise:

- Real-hardware-ish PCI enumeration
- Real-CPU storage command timing
- MSI/MSI-X configuration paths
- End-to-end durability through `create_write_fsync_readback=ok`

If these PASS under VMware, the substrate-agnosticism result (NeoFS-on-AHCI, NeoFS-on-NVMe) extends to hypervisor-VM environment. If they FAIL, that's significant pre-BUILD-5b signal.

### Step 6 — Stress runs

In addition to the standard scenario sweep:

**6.a — 100x `ipc_heap_kill_smoke`:** Phase 2 sub-commit 2 gate originally surfaced the Q5 deferred-quantum race under QEMU stress. Re-running under VMware's TSO memory model may surface different behavior. Expected: 100/100 PASS.

**6.b — 200x `smp_cross_cpu_rendezvous_stress`:** Phase 2 SC-3B / SC-3C closure stress gate doubled. Expected: 200/200 PASS.

**6.c — 100x `phase3_ahci_neofs_mount_and_readwrite`:** Phase 3 composition stress on AHCI substrate. The first hypervisor-VM stress run of the AHCI completion path. Expected: 100/100 PASS. Watch specifically for any retry-eligible flakes (the `phase3_storage_completion_cross_cpu_wake` scenario has a known retry pattern; this composition test does not).

**6.d — 100x `phase3_nvme_neofs_mount_and_readwrite`:** Phase 3 composition stress on NVMe substrate. **This is the highest-value stress run for hypervisor-VM verification.** It exercises the MSI-X-before-APIC ordering fix's runtime behavior under realistic timing pressure. Expected: 100/100 PASS. Any flake is significant data.

**6.e — D14 + D16 negative-feature scenarios:** Run with `neo-test-missing-shootdown-negative` and `neo-test-missing-cap-lock-negative` features. Expected: both DETECT their failure modes (gates PASS by detection).

### Step 7 — Frame audit + crash residue checks

After each scenario run completes, observe in serial:

- `frame: leak audit clean` line.
- No `KERNEL PANIC` after init lines.
- No `BUG:` lines after init.
- For test-harness runs: scenario-specific PASS markers.

### Step 8 — Optional CPU-count triangulation

Re-run Steps 4-7 with VM CPU count set to 1 (verifies non-SMP regression) and 8 (verifies scaling beyond `NEO_SMP=4`). Optional but useful if any FAIL surfaces in the 4-CPU run.

### Step 9 — File the verification report

Create `Plans/PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-<date>.md` matching the template in §4.

## 4. Verification report template

```markdown
# Phase 2 + Phase 3 Hypervisor-VM Verification (VMware Workstation)

**Date:** YYYY-MM-DD
**Executor:** PL
**Substrate HEAD at verification:** <SHA>
**Verification environment:** VMware Workstation <version>, host OS <Windows version>, host CPU <model>, host RAM <GB>, VT-x/AMD-V enabled.
**VM config:** 4 vCPU, 1024 MB RAM, SATA controller + virtual disk, NVMe controller + virtual disk, serial output to file.
**Status:** PASS | PASS-WITH-INCIDENTAL | FAIL (with summary)

## 1. Build

- `cargo iso` (production): <PASS / FAIL>. Build time: <minutes>. Warnings: <list>.
- `cargo iso --features neo-test`: <PASS / FAIL>. Build time: <minutes>. Warnings: <list>.

## 2. Production-ISO boot (no test harness)

Observed serial highlights:
- Limine handoff: <yes/no/timestamps>
- Kernel banner: <observed>
- BSP online: <observed>
- AP bring-up: <count of APs online>
- PCI enumeration: <devices found>
- AHCI controller: <detected / not-detected / BDF / BAR / ports>
- NVMe controller: <detected / not-detected / BDF / BAR / namespace info>
- Block devices registered: <count and list>
- neo-init exit code: <code>
- frame: leak audit: <clean / leaks>
- KERNEL PANIC: <none / list>
- BUG:: <none / list>

## 3. Phase 2 scenario sweep (test harness)

(Table per §3 Step 5 — Phase 2 scenarios)

## 4. Phase 3 scenario sweep (test harness)

(Tables per §3 Step 5 — NeoFS, VFS/mount, FAT32, ext4, AHCI, NVMe)

## 5. Stress runs

- 100x `ipc_heap_kill_smoke`: <count PASS / count FAIL>.
- 200x `smp_cross_cpu_rendezvous_stress`: <count PASS / count FAIL>.
- 100x `phase3_ahci_neofs_mount_and_readwrite`: <count PASS / count FAIL>.
- 100x `phase3_nvme_neofs_mount_and_readwrite`: <count PASS / count FAIL>.
- D14 / D16 negative-feature detection: <PASS by detection / FAIL>.

## 6. CPU-count triangulation (optional)

- 1-CPU run: <PASS / FAIL>.
- 8-CPU run: <PASS / FAIL>.

## 7. Incidental observations

(Anything noted during verification that's not strictly PASS/FAIL: log line oddities, timing differences vs QEMU, performance observations, VMware-specific quirks, AHCI/NVMe behavioral differences from QEMU emulation, etc.)

## 8. Conclusion

(One paragraph: overall verdict; comparison to QEMU TCG baseline; any items needing PM triage.)
```

## 5. What constitutes a PASS

Hypervisor-VM verification PASSes if:

- Both ISO builds succeed.
- Production-ISO boot completes cleanly (frame audit clean; no panic; no BUG; no hang; AHCI + NVMe controllers detected when configured).
- All Phase 2 scenarios PASS (per May 11 procedure §5).
- All Phase 3 scenarios PASS, including both composition tests (`phase3_ahci_neofs_mount_and_readwrite` and `phase3_nvme_neofs_mount_and_readwrite`).
- Stress runs 100/100 + 200/200 + 100/100 + 100/100 PASS (or within 1 flake per series, which gets surfaced for analysis).
- D14 + D16 negative-feature gates detect their failure modes.

If all hold, Phase 2 + Phase 3 hypervisor-VM verification is complete. **Bible Article 16.3 bare-metal mandate remains formally open**; the hypervisor-VM pass is intermediate validation, not Phase 3 formal closure.

## 6. What constitutes a FAIL

Any of:

- Build failure.
- Production-ISO boot hang / panic / BUG.
- AHCI controller not detected when SATA disk configured.
- NVMe controller not detected when NVMe disk configured.
- Standard scenario sweep showing FAIL not seen on QEMU TCG.
- Stress run flake rate >1% on any series.
- Negative-feature gate failing to detect its failure mode.
- APIC-base assertion or any indication of MSI-X-before-APIC ordering regression.

On FAIL: PL surfaces the verification report. PM triages.

## 7. Likely failure modes and triage hints

**Phase 2 failure modes (per May 11 procedure §7):**

- AP bring-up hang → VMware APIC/SIPI differs from QEMU; check serial for which AP didn't come online.
- First SMP scenario hang → possible memory-model issue; `context_switch_with_release` asm wrapper at `kernel/src/sched/scheduler.rs` is first suspect.
- Sporadic stress FAIL → race-condition surface QEMU TCG masked.
- `ipc_heap_kill_smoke` flake → would re-surface Q5 deferred-quantum issue.

**Phase 3 failure modes (new in this procedure):**

- **AHCI controller not detected** → likely PCI enumeration class-triplet mismatch under VMware's specific AHCI device. Surface PCI device list from production-ISO boot logs.
- **NVMe controller not detected** → same, for NVMe class-triplet 0x01/0x08/0x02. Note: VMware's NVMe emulation appears as Vendor 0x15ad (VMware) rather than QEMU's typical 0x1b36; PCI enumeration should be vendor-agnostic per BUILD-5a-NVMe dispatch §3.
- **AHCI port enumeration fails** → BUILD-5a-AHCI q35 dual-controller quirk fix may not transfer; VMware may expose AHCI differently. Triage with port-implemented register dump.
- **NVMe admin queue creation hangs** → could be MSI-X timing or queue-pair memory allocation issue. The MSI-X-before-APIC ordering fix should hold, but VMware may surface a related issue.
- **Composition test FAIL** (NeoFS-on-AHCI or NeoFS-on-NVMe) → would indicate substrate-agnosticism regression under hypervisor-VM environment. Highly significant; surface full scenario evidence for PM triage.
- **APIC-base assertion during PCI enum / storage init** → MSI-X-before-APIC ordering regression. Direct refutation of BUILD-5a-NVMe §"Interrupts And Ordering" fix. Highly significant.
- **`phase3_nvme_neofs_mount_and_readwrite` stress flake** → would suggest residual ordering issue in NVMe completion path that QEMU masked. PM dispatches Cerebro for memory-ordering review.

## 8. What PASS unblocks

Hypervisor-VM PASS does NOT formally close Phase 3 (Bible Article 16.3 bare-metal mandate stays open). But it does:

- Validate the substrate-agnosticism result (3 filesystems × 3 backends) extends to a non-QEMU hypervisor.
- Demonstrate the MSI-X-before-APIC ordering fix holds under real-CPU timing.
- Reduce BUILD-5b risk: real-bare-metal will surface fewer surprises if hypervisor-VM has already shaken out the hardware-virtualization issues.
- Surface VMware-specific quirks that BUILD-5b's bare-metal dispatch should plan for.

This makes hypervisor-VM PASS a **strong pre-BUILD-5b signal**, even though it isn't BUILD-5b itself.

## 9. Cross-references

**Phase 2 BUILD-3 closure context:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md`
- `Plans/CLOSE-REPORT-2026-05-11-sc-4.md`
- `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` (Phase 2 antecedent)

**Phase 3 substrate context:**
- `Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md`
- `Plans/PHASE-3-BUILD-3-CLOSE-EVIDENCE-2026-05-15.md`
- `Plans/PHASE-3-BUILD-4A-CLOSE-EVIDENCE-2026-05-16.md`
- `Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md`
- `Plans/PHASE-3-BUILD-4C-CLOSE-EVIDENCE-2026-05-20.md`
- `Plans/PHASE-3-BUILD-4D-CLOSE-EVIDENCE-2026-05-20.md`
- `Plans/PHASE-3-BUILD-5a-AHCI-CLOSE-EVIDENCE-2026-05-20.md`
- `Plans/PHASE-3-BUILD-5a-NVMe-CLOSE-EVIDENCE-2026-05-20.md`

**Bible mandate:**
- Article 16.3: bare-metal verification gate for phase closure. Hypervisor-VM is intermediate; bare-metal stays open.

**Forward markers carried in:**
- BUILD-5a-AHCI §3.5: q35 dual-AHCI controller quirk (single-controller scope).
- BUILD-5a-NVMe §"Interrupts And Ordering": MSI-X-before-APIC ordering rule.

Both forward markers are tested under VMware by this procedure.

---

**End of procedure document.**

*Manual procedure for PL direct execution. Hypervisor-VM verification of Phase 2 BUILD-3 SMP correctness + Phase 3 substrate. Bare-metal mandate stays open. Estimated 4-6 hours wall-clock. Verification report template provided. PM triages any FAIL.*
