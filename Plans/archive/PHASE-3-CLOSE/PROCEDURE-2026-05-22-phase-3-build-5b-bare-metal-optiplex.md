# Phase 3 BUILD-5b Bare-Metal Storage Validation Procedure (OptiPlex 7080)

**Date:** 2026-05-22
**Recorder:** PM
**Executor:** PL (on bare metal); dev-container build step run by PL or worker per §3
**Repository:** `E:\AI Automation\AltimitNeo`
**HEAD at filing:** `55e42f3`
**Status:** procedure, not yet executed
**Authority chain:**
- Bible Article 13 (bare-metal verification required for phase close)
- `Plans/PM-NOTE-2026-05-21-phase-4-authority-packet-rehydration.md` (Phase 3 forward direction)
- `Plans/PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22.md` (predecessor evidence: target boots)

---

## 1. Scope

This procedure specifies the bare-metal storage validation run for Phase 3
BUILD-5b on the Dell OptiPlex 7080 hardware target proven viable by today's
boot-smoke evidence. The goal is to execute the
`phase3_ahci_neofs_mount_and_readwrite` and
`phase3_nvme_neofs_mount_and_readwrite` scenarios on real OptiPlex 7080
internal storage and produce close-evidence of equivalent rigor to
`Plans/PHASE-3-VMWARE-CLEAN-COPY-RERUN-EVIDENCE-2026-05-21.md`.

The terminal artifact of this procedure is
`Plans/PHASE-3-BUILD-5B-BARE-METAL-OPTIPLEX-EVIDENCE-<date>.md`. Once that
lands, Phase 3 formal close becomes actionable per Bible Article 13.

Out of scope:

- Secure Boot enablement (separate ADR territory).
- Cross-firmware testing on additional bare-metal targets. One OptiPlex run is
  sufficient for Phase 3 close.
- Performance benchmarking, power-management testing, network stack, AI
  subsystem — none of these are Phase 3 concerns.

---

## 2. Hardware Prerequisites

### 2.1 Target machine

Dell OptiPlex 7080. Per the boot-smoke evidence, viable for UEFI USB boot with
Secure Boot disabled.

PL pre-run inventory items, answer before §3:

- Chassis variant (SFF, MT, MFF, micro)
- BIOS revision
- Internal SATA storage: present? device spec?
- M.2 NVMe storage: present? device spec?
- M.2 SATA option (some variants): present?

The presence of each internal storage type determines which scenarios are
executable. AHCI scenario requires a SATA disk on the AHCI controller; NVMe
scenario requires an NVMe disk in the M.2 slot.

If neither internal AHCI nor internal NVMe is present, BUILD-5b cannot
complete on this target and a different hardware target would be required for
Phase 3 close.

### 2.2 Serial capture path (CRITICAL)

The boot-smoke run captured no serial log. BUILD-5b cannot proceed under the
same constraint: scenario PASS signatures live in serial output as
`NEO_TEST/1 event=PASS name=<scenario>` per
`PROCEDURE-2026-05-20-phase-3-vmware-scenario-harness.md`. Without serial
capture, PASS cannot be rigorously distinguished from "kernel hung after some
visible milestone."

Serial paths in priority order:

1. **Internal motherboard COM header → DB9 bracket.** Most OptiPlex 7080
   variants ship with the bracket option. Capture with a USB-to-serial cable
   from a second machine running PuTTY / screen / minicom at 115200 8N1.
2. **PCIe serial expansion card.** The 7080 has at least one PCIe slot on most
   variants. A standard serial card exposes COM1 to the kernel.
3. **Visual fallback.** If neither 1 nor 2 is achievable, BUILD-5b downgrades
   to visual-only evidence — insufficient for storage validation. Escalate:
   acquire a serial path, or PL judgment on whether visual-only counts for
   Phase 3 close under Bible Article 13.

PL answer needed: which serial path is reachable on his unit.

### 2.3 USB media

The UEFI-capable USB device used for the boot-smoke run today is sufficient.
Rufus DD mode, UEFI partition scheme. Each scenario re-flashes the same stick
with its scenario-specific ISO.

### 2.4 Provisioning USB (separate)

A second USB device with a Linux live ISO (Ubuntu / Debian / any modern live)
is needed for §4 to `dd` scenario raw disk files onto real OptiPlex storage.
This is separate from the BUILD-5b ISO USB.

---

## 3. Scenario Bundle Builds

Build each scenario bundle from the canonical dev container:

```bash
docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite
docker compose run --rm neo cargo neo-test --build-only phase3_nvme_neofs_mount_and_readwrite
```

Each command emits to `target/neo-test/vmware/<scenario>/`:

- A scenario ISO (the UEFI-bootable image to flash to USB)
- A scenario raw disk file (`disk.0.role=scenario` per the manifest)
- A manifest file describing disk layout

For bare metal, only the **scenario ISO** and the **scenario raw disk** are
used. `role=qemu-filler` entries are ignored.

The scenario ISO is the same UEFI-capable image format proven by today's
boot-smoke run (`d425d02` added UEFI emission to `xtask`).

---

## 4. Scenario Disk Provisioning

The scenario raw disk must be present on the OptiPlex's real storage when the
BUILD-5b kernel boots. On VMware this was VMDK attachment; on bare metal the
raw bytes need to land on a real device.

### 4.1 AHCI scenario

Target: the internal SATA disk attached to the OptiPlex's AHCI controller.

1. Boot the OptiPlex from the provisioning USB (Linux live).
2. Identify the SATA disk via `lsblk` (typically `/dev/sda` when present alone).
3. Write the scenario raw to it:
   ```
   sudo dd \
     if=<repo>/target/neo-test/vmware/phase3_ahci_neofs_mount_and_readwrite/disk.0.raw \
     of=/dev/sda \
     bs=1M conv=fsync status=progress
   ```
4. Shut down the live USB.
5. Reflash the BUILD-5b USB stick with the AHCI scenario ISO (Rufus, DD mode).
6. Proceed to §5.

This is destructive to any existing data on the SATA disk. PL acknowledges the
wipe in §11 before executing.

### 4.2 NVMe scenario

Target: the internal M.2 NVMe device. Procedure identical to §4.1 with the
NVMe device path (typically `/dev/nvme0n1` on the live USB).

### 4.3 Scenario disk sizes

Scenario raw disks are small (single-digit GB at most). Both internal SATA and
internal NVMe accommodate them with substantial headroom.

---

## 5. Boot Configuration

Per the boot-smoke evidence:

- UEFI mode (legacy/CSM disabled).
- Secure Boot disabled.
- F12 boot menu → UEFI USB device.

No additional BIOS configuration is required from the boot-smoke baseline.

---

## 6. Execution Per Scenario

For each scenario (AHCI then NVMe):

1. Scenario raw disk written to target storage per §4.
2. Scenario ISO flashed to USB per §3.
3. Serial capture active on the second machine per §2.2.
4. Power on OptiPlex; F12 → UEFI USB.
5. Watch serial log for one of:
   - `NEO_TEST/1 event=PASS name=<scenario_name>` → PASS for that scenario.
   - `NEO_TEST/1 event=FAIL name=<scenario_name> reason=<reason>` → FAIL;
     capture context.
   - Kernel panic / unrecoverable hang → capture full serial up to last output.
6. Power off. Archive serial log to
   `Plans/PHASE-3-BUILD-5B-BARE-METAL-OPTIPLEX-EVIDENCE-<date>-serial-<scenario>.log`.
7. Photograph framebuffer at end-state, especially on FAIL or hang.

---

## 7. Evidence Capture

Per scenario, capture:

- Full serial log file.
- Photo of framebuffer at scenario completion or hang point.
- Optional: post-scenario disk state via re-boot to Linux live USB
  (`fdisk -l`, `nvme list`).

File the BUILD-5b close evidence as
`Plans/PHASE-3-BUILD-5B-BARE-METAL-OPTIPLEX-EVIDENCE-<date>.md`, mirroring the
structure of `Plans/PHASE-3-VMWARE-CLEAN-COPY-RERUN-EVIDENCE-2026-05-21.md`.
Reference the per-scenario log files as relative paths under `Plans/`.

---

## 8. Success Criteria

BUILD-5b PASS requires all of:

- **(S1)** Serial capture demonstrably working — any kernel output observed in
  the serial log.
- **(S2)** `phase3_ahci_neofs_mount_and_readwrite` emits
  `NEO_TEST/1 event=PASS` in serial output, against real SATA disk on the AHCI
  controller. Skip iff no SATA storage is present on the unit — note as
  *deferral*, not pass.
- **(S3)** `phase3_nvme_neofs_mount_and_readwrite` emits
  `NEO_TEST/1 event=PASS` in serial output, against real NVMe disk. Skip iff
  no NVMe storage is present on the unit — note as *deferral*, not pass.
- **(S4)** No kernel panic, no unrecoverable hang, no late-stage assert during
  either scenario.
- **(S5)** Evidence artifact filed and committed to `Plans/`.

If either S2 or S3 is deferred for hardware reasons, Phase 3 formal close
becomes a PL judgment call. Bible Article 13 does not explicitly require both
AHCI and NVMe on bare metal; the spirit is that bare-metal validation must
cover the storage code paths in scope. PL decides.

### 8.1 Closure linkage to Jarvis log-sink dispatch

BUILD-5b execution implicitly discharges acceptance criteria C2 and C4 from
`Plans/DISPATCH-2026-05-22-block-device-log-sink-and-phase-indicator-jarvis.md`,
closed substantially on 2026-05-22 per
`Plans/CLOSE-REPORT-2026-05-22-block-device-log-sink-and-phase-indicator.md` and
`Plans/PM-DISPOSITION-2026-05-22-block-device-log-sink-and-phase-indicator-close-consumption.md`:

- **C2 (framebuffer phase indicator renders observable in QEMU/VMware) discharged-on-BUILD-5b-execution.** S1–S5 observation of the OptiPlex's framebuffer during scenario execution constitutes the runtime rendering verification.
- **C4 (both scenarios run cleanly with sink + extract retrieves PASS log) discharged-on-BUILD-5b-execution.** S2/S3 PASS-sentinel observation in the extracted log via `tools/neo-extract-log` constitutes the runtime exercise verification.

No additional BUILD-5b success criteria are introduced by this linkage. The
linkage is documentary; BUILD-5b's existing S1–S5 do the actual verification
work.

---

## 9. Anti-Scope

Per §1, BUILD-5b explicitly excludes:

- Secure Boot
- Network stack
- AI subsystem
- Additional bare-metal targets beyond the OptiPlex 7080
- Performance benchmarking
- Power-management (S-state) testing
- Hot-plug or USB-attached storage device drivers

---

## 10. Risk Register

| Risk | Mitigation |
|------|-----------|
| Real-IPI timing exposes a race the BUILD-4 fixes did not actually close | Capture full serial; if a stall pattern appears, file as ADR-territory issue and dispatch to investigator agent |
| Real OEM ACPI tables cause boot regression past framebuffer | Already disproven by boot-smoke for the initial path; beyond framebuffer, serial captures kernel progress markers |
| Intel chipset AHCI controller has command-ordering quirks VMware did not emulate | Captured in serial; analyze post-run |
| NVMe controller has async event notification differences from VMware | Same — captured in serial |
| C-state-induced TSC drift affects timer-based code | Phase 3 storage code is not heavily timer-dependent; low risk |
| PL has no serial capture hardware | §2.2 escalation path required before execution |
| Wipe of internal disk loses data PL wants | PL pre-commits to the wipe in writing per §11 before §4 runs |
| Kernel runs past scenario without halting (no shutdown path) | Treat hang-at-end as PASS iff PASS sentinel is in log; treat hang-without-sentinel as FAIL |
| Scenario raw written by `dd` is misaligned to disk sector size | Both SATA SSDs and NVMe devices use 512B or 4KB logical sectors; `bs=1M` is a multiple of both. Low risk |
| OptiPlex SATA controller in RAID mode rather than AHCI | PL verifies BIOS storage mode = AHCI before §4.1 |

---

## 11. Open Questions For PL

Answer these once before scheduling the BUILD-5b execution session. PL may
answer inline by amending this artifact or via a sibling response artifact.

- **Q1.** OptiPlex 7080 chassis variant (SFF / MT / MFF / micro)?
- **Q2.** Internal SATA disk present? If yes, model and capacity?
- **Q3.** Internal NVMe disk present? If yes, model and capacity?
- **Q4.** BIOS storage mode for SATA: AHCI confirmed (not RAID/Intel RST)?
- **Q5.** Serial capture path reachable per §2.2? Which option (1/2/3)?
- **Q6.** Second machine available for serial terminal capture?
- **Q7.** Spare Linux live USB available for the provisioning step?
- **Q8.** PL acceptance: internal storage on this OptiPlex may be wiped during
  the run? (Required before §4 executes.)

---

## 12. Suggested Phasing

| Step | Owner | Estimated time | Deliverable |
|------|-------|----------------|-------------|
| Answer Q1–Q8 | PL | ~15 min | Inline amendment to this artifact |
| Build scenario bundles per §3 | PL or worker dispatch | ~15 min | `target/neo-test/vmware/<scenario>/` populated |
| Provision AHCI disk per §4.1 | PL | ~20 min | SATA disk holds scenario raw |
| Execute AHCI scenario per §5–§7 | PL | ~30 min | Serial log + framebuffer photo |
| Provision NVMe disk per §4.2 | PL | ~20 min | NVMe disk holds scenario raw |
| Execute NVMe scenario per §5–§7 | PL | ~30 min | Serial log + framebuffer photo |
| File BUILD-5b evidence artifact | PM | ~30 min | `PHASE-3-BUILD-5B-BARE-METAL-OPTIPLEX-EVIDENCE-<date>.md` |
| File Phase 3 close PM-DISPOSITION | PM | ~20 min | `PM-DISPOSITION-<date>-phase-3-formal-close.md` |

The two execution segments may run back-to-back in a single PL hardware
window. Estimate ~2.5 hours wall-clock for the full execution if hardware
prerequisites are already satisfied.

---

## 13. References

- Bible Article 13 (verification requirements)
- `Plans/PROCEDURE-2026-05-20-phase-3-vmware-scenario-harness.md` — VMware analog this mirrors
- `Plans/PHASE-3-VMWARE-CLEAN-COPY-RERUN-EVIDENCE-2026-05-21.md` — evidence structure to mirror
- `Plans/PHASE-3-VMWARE-SCENARIO-HARNESS-CLOSE-EVIDENCE-2026-05-20.md` — prior VMware close
- `Plans/PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22.md` — predecessor evidence
- `Plans/PM-NOTE-2026-05-21-phase-4-authority-packet-rehydration.md` — Phase 3 forward direction context
- `tools/xtask/src/main.rs` — scenario bundle build mechanics; `d425d02` added UEFI ISO emission

---

**End of procedure.**
