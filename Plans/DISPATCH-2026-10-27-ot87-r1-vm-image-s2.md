<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-87 R1, `xtask vm-image`: LogansOS installed on a VM disk, booted under UEFI → S2

**Why.** J.c. (2026-10-27) wants LogansOS installed on a real computer or a VM, not run off the USB. The OT-87 THINK
(PR #149, S3) is still in revision, but its **rung 1 was judged sound in both cross-family reads** (Chad, relays #86
and #87: "the independent R1 design remains sound"). R1 needs no kernel change and is independent of the later write
paths, so it starts now.

**Spec:** `docs/THINK-2026-10-27-ot87-installer.md` §3 at `1f313ac6`
(`git fetch origin && git show 1f313ac6:docs/THINK-2026-10-27-ot87-installer.md`).

## Scope

1. **`cargo xtask vm-image [--with-model <neom>]`** writes two raw disks under `target/vm/`:
   - `boot.raw`: image A's ISO bytes, unchanged (xorriso's GPT and ESP);
   - `data.raw`: a whole-disk NeoFS volume from `neofs-mkfs`, with the mount's required stubs and the `.arthur` home.
     It holds no model in R1.
   Optionally, `xtask vm-run` boots them with OVMF and no `-cdrom`, for J.c. to run locally.
2. **OVMF, hermetic.** Bake a pinned OVMF (code and vars) into the dev container image, and record the version and
   checksum in the Dockerfile and the runbook. No host package. The writable vars are a per-run copy.
3. **A QEMU scenario, `vm_installed_two_boot`,** in the 4-shard gate. It boots from `boot.raw` + `data.raw` under OVMF,
   with no `-cdrom`:
   - the first boot writes the intro flag and a memory file;
   - a **second boot of the same disks** reads them back.
   It must fail when `data.raw` is replaced by a fresh one between the boots: a control that proves persistence is
   what passes it.
   Keep the boot-neofs device-order assumption explicit: the boot disk has no valid NeoFS, and the data disk is the
   first one that does.
4. **Product image unchanged.** No kernel or userland change. Image A's ISO bytes in `boot.raw` match `xtask iso`'s.
   Add an xtask KAT for the vm-image layout: sizes, NeoFS superblock, and boot.raw equal to the ISO.

## Rules

- The Zenbook is never a target, and nothing here writes a real disk. This is host files and QEMU only.
- Scenario hermeticity: no host-variable input (CLAUDE.md, hermeticity is a gate invariant).
- `.github`/Dockerfile changes go through the 4-shard gate. Run the close-gate (`scripts/worker-close-gate.sh`). No
  `[skip ci]`.
- You have a standing yes for branch `claude/ot87-r1-vm-image` and a PR.
- Add a short "Try LogansOS in a VM" section to the runbook or README with the two commands, so J.c. can run it.

## Close

Send the PM the PR number and head SHA. I gate it. Once it merges, J.c. can boot LogansOS from a VM disk and keep his
state across reboots.
