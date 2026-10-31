<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-90 (new), clean shutdown and restart: design THINK → S5

**Why.** The MVP tier (`Plans/DECISION-2026-10-27-release-tiers-mvp.md`, J.c. 2026-10-27) needs **clean shutdown and
restart**. Today LogansOS has no power-off or reboot path (there's no system shutdown syscall). You did the ACPI/AML
work (OT-86, #96, Fix 1), so you know the parts this depends on.

**Deliverable:** `docs/THINK-2026-10-27-ot90-shutdown-restart.md` (CC-BY-4.0 SPDX header), as a PR on
`claude/ot90-power-think`. It's docs only, so it needs no QEMU gate. You have a standing yes for the branch and PR.
Send the PM the PR number.

## Questions the THINK answers

1. **Power-off.** ACPI S5 needs `\_S5` from the DSDT (through the AML interpreter), `SLP_TYPa`/`SLP_TYPb` and
   `PM1a/PM1b_CNT` from the FADT. Also cover hardware-reduced ACPI (the Zenbook may be one) and the reset register.
   Say what exists today and what's missing.
2. **Restart.** Use the FADT reset register, with fallbacks (the 0xCF9 port, the keyboard controller, a triple
   fault), in a stated order.
3. **"Clean."** Stop userland in order, give apps a chance to save, then flush and unmount NeoFS (fsync, superblock).
   Stop the APs, quiesce DMA and IOMMU, then power off. What happens if something hangs (a bounded wait, then a
   forced power-off)?
4. **Who can ask.** A capability for shutdown and restart: the desktop's power menu has it, ordinary apps don't.
   Say how it fits the capability model.
5. **The user's side.** A power menu on the desktop (shut down, restart). The power button is optional for the MVP;
   say what it would need (an ACPI fixed event or a GPE).
6. **Testing.** QEMU scenarios: power-off ends QEMU with a defined exit and the NeoFS volume is clean on the next
   boot; restart reboots into a second boot. A bare-metal check on the OptiPlex, and a Zenbook check using photos per
   its runbook. Include a failure-mode and discriminator bank for the bare-metal runs.
7. **Sleep and wake (S3 or s2idle)** are 1.0. Name what OT-90's design must not block.
8. **A rung ladder** in dependencies, cycles and gates (no calendar estimates), with a kill-switch per policy rung.

Cite code by `file:line`. A cross-family read of the THINK comes before the first build rung.
