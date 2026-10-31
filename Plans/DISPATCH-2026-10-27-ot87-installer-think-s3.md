<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-87 (new), install LogansOS onto a real disk or a VM: design THINK → S3

**Why.** J.c. (2026-10-27): "in the real world, I want this not to run off of a USB, but to be installed on an actual
computer or VM." The release ledger (`Plans/LEDGER-2026-07-29-logansos-release-ledger.md`, journey 1 "Install") has
this at **zero code**. Deployment today means `dd` the ISO, and the model loads from the USB's boot modules (the Absent
line says "Boot the LogansOS USB it came with"). This opens **OT-87** with a design THINK. No code yet.

**Deliverable:** `docs/THINK-2026-10-27-ot87-installer.md` (CC-BY-4.0 SPDX header), pushed on a branch
`claude/ot87-installer-think` with a PR, or straight to a PR. It's docs only, so it needs no QEMU gate.

## Questions the THINK answers

**The product shape (J.c., 2026-10-27): "just like an Ubuntu ISO has the option to try first or install, I want that
for our OS."** One USB image, two choices:
- **Try LogansOS:** the live session as it runs today, nothing written to any disk;
- **Install LogansOS:** onto a disk the user picks.

Say where the choice is offered:
- the boot menu (Limine entries with different kernel command lines or features);
- a first screen in the live session;
- an "Install" item reachable from the desktop while trying, as Ubuntu has.
Also say what each path writes (Try: nothing), and how Try stays exactly today's behaviour (the kill-switch: the Install
path absent ≡ today's image).


1. **Targets, in order:**
   - (a) a **VM**: QEMU with a virtio-blk or NVMe disk, which is also what makes it testable in CI;
   - (b) the **Arthur OptiPlex**, whose NVMe is ours to write (`docs/runbook-baremetal-arthur-optiplex.md`);
   - (c) later, other PCs.
   The **Zenbook's internal NVMe is never written** (CLAUDE.md, write guard #85/#88). Say how the installer keeps it
   that way: the guard, an explicit target selection, refusing any disk it didn't enumerate as the chosen target.
2. **The shortest path to "installed on a VM".** Could `xtask` produce a bootable **disk image** (raw or qcow2: GPT,
   an ESP with Limine and the kernel, a NeoFS volume with userland, the model and the vocab) that a VM boots
   directly, with no installer program at all? That may be rung 1.
3. **The on-device installer** (boot the USB, pick a disk, install):
   - partitioning (GPT, ESP and NeoFS root);
   - Limine onto the ESP and the UEFI boot entry;
   - copying the system;
   - putting the model and vocab on the disk's NeoFS volume;
   - first-boot provisioning.
   Say what exists today (`tools/neofs-mkfs`, the NeoFS driver, AHCI/NVMe write paths, the write guard, the boot-neofs
   flow) and what's missing.
4. **Model from disk instead of boot modules.** What Arthur and the kernel need so the model loads from the installed
   NeoFS volume. Today that's the boot-module path plus the `QWEN25_05B_*` pins. Keep provenance and the pins.
5. **Updates.** How this fits OT-51's A/B update model (`docs/THINK-2026-07-10-ot51-update-rollback-model.md`), so the
   install layout doesn't block A/B later.
6. **A rung ladder** with a close gate per rung, scoped in dependencies, cycles and gates (no calendar estimates). For
   any rung with a bare-metal run, a failure-mode and discriminator bank. Name the kill-switch for each policy rung.

RAG isn't required. Cite code by `file:line`. You have a standing yes for a branch and PR on this lane. When it's
pushed, send the PM the PR number. A cross-family read of the THINK follows before rung 1 is dispatched.
