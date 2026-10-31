# Phase 3 Bare-Metal OptiPlex 7080 Boot Smoke Evidence

**Date:** 2026-05-22
**Recorder:** PM
**Executor:** PL
**Repository:** `E:\AI Automation\AltimitNeo`
**HEAD at ISO generation:** `d425d02692f56828f3504980ceec165e2d5988c0`
**Status:** PASS-WITH-SCOPE

## 1. Scope

This artifact records the first reported bare-metal UEFI USB boot smoke of the
NeoPlutarch production ISO on a Dell OptiPlex 7080.

This is a visual boot-smoke result. It verifies that the current production ISO
can be presented as UEFI-removable media, accepted by the OptiPlex firmware
when Secure Boot is disabled, and reach the NeoPlutarch framebuffer boot screen
on physical hardware.

This artifact does not claim:

- Secure Boot support;
- AHCI or NVMe real-disk read/write validation;
- a full Phase 3 BUILD-5b storage validation sweep;
- serial-log capture from the OptiPlex run.

## 2. ISO Under Test

Production ISO path:

```text
E:\AI Automation\AltimitNeo\boot\neoplutarch.iso
```

ISO build command:

```powershell
docker compose run --rm neo cargo iso
```

Local ISO metadata after the UEFI-capable rebuild:

```text
Path: E:\AI Automation\AltimitNeo\boot\neoplutarch.iso
Size: 24,616,960 bytes
LastWriteTime: 2026-05-22 18:54:31 America/New_York
```

The ISO was generated after commit `d425d02` updated `xtask` to emit a
BIOS+UEFI Limine image. Local inspection confirmed:

```text
El Torito boot img :   1  BIOS  y   none  ... /boot/limine-bios-cd.bin
El Torito boot img :   2  UEFI  y   none  ... /boot/limine-uefi-cd.bin
'/EFI/BOOT/BOOTX64.EFI'
```

## 3. USB Media Preparation

PL prepared USB media with Rufus x64 v4.14.2377 on Windows 11 Pro.

Observed Rufus signals from the operator log:

```text
UEFI bootloaders analysis:
  - /EFI/BOOT/BOOTX64.EFI
  (Unsigned Bootloader)
Using image: neoplutarch.iso
```

Rufus enforced DD image mode for the ISOHybrid image. The USB device was a
Generic Flash Disk USB device (`058F:6387`).

## 4. OptiPlex 7080 Result

Target hardware:

```text
Dell OptiPlex 7080
```

Boot path:

```text
USB removable media
UEFI boot path
Secure Boot disabled
```

First attempt:

```text
Firmware reached the USB boot path but stopped with signature verification.
The Limine UEFI bootloader is unsigned, so this is expected under Secure Boot.
```

Corrected attempt:

```text
PL disabled Secure Boot, selected the UEFI USB boot entry, and reported that
NeoPlutarch loaded successfully.
```

Observed milestone:

```text
NeoPlutarch framebuffer boot screen reached on physical OptiPlex 7080 hardware.
```

Because the current production boot renders the framebuffer boot screen after
the kernel production path reaches the final boot-frame render, this is useful
bare-metal smoke evidence for the Limine UEFI path, framebuffer initialization,
and physical-machine boot viability.

## 5. Caveats And Follow-Ups

- No serial output was captured from the OptiPlex run.
- No internal disk write or mount operation is claimed by this artifact.
- Secure Boot remains unsupported until the project adds a signing/enrollment
  story.
- Phase 3's stronger BUILD-5b storage-validation target still needs explicit
  real-device AHCI/NVMe evidence if the project wants to claim storage-driver
  bare-metal closure rather than boot-smoke closure.

## 6. Conclusion

NeoPlutarch's production ISO successfully booted from UEFI USB on a Dell
OptiPlex 7080 after Secure Boot was disabled. This discharges the immediate
bare-metal boot-smoke question for the OptiPlex target and creates a concrete
hardware baseline for future BUILD-5b storage validation.
