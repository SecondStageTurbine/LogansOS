<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-87, installing LogansOS onto a disk or a VM (Try or Install)

**Date:** 2026-10-27 · **Author:** S3 · **Dispatch:** `Plans/DISPATCH-2026-10-27-ot87-installer-think-s3.md` ·
**Status:** design only, no code. Revision 2 answers the cross-family read (Chad, relay #86,
`Plans/RETURN-2026-10-27-ot87-installer-think-review-chad.md`: items 1, 3, 4 and 6 passed; 2, 5, 7 and 8 failed)
and folds in J.c.'s §9 decisions. Revision 3 answers the narrow re-check (relay #87,
`Plans/RETURN-2026-10-27-ot87-installer-think-recheck-chad.md`). That added durability barriers, identity on every
write submission, the full audit list, and destructive reinstall of a completed disk. Revision 4 answers the
second re-check (relay #88, `Plans/RETURN-2026-10-27-ot87-installer-think-recheck2-chad.md`): array-before-header
barriers for every GPT copy, an uncertain outcome when a flush fails, an honestly scoped unreported-swap bound,
and the prior-art survey (§11). Revision 5 answers relay #90: the `fatfs` borrow moves to the host ESP-template
builder (the kernel still never formats FAT), and §11 records the deferred encryption and Secure Boot scope.

**The ask (J.c., 2026-10-27).**
- "In the real world, I want this not to run off of a USB, but to be installed on an actual computer or VM."
- "Just like an Ubuntu ISO has the option to try first or install, I want that for our OS."

So the goal is one USB image that offers both **Try LogansOS** (today's live session) and **Install LogansOS** (onto
a disk the user picks), and an installed system that boots from its own disk with its model and its user data
on it.

**Related.** `docs/THINK-2026-10-19-arthur-first-boot-model.md:79-80` names "OT-65 (installer and first-boot
provisioning)" as 1.0 scope with zero code. **OT-65 is superseded by OT-87** (PM, 2026-10-27): this THINK is that
work, first-boot provisioning included (§4.8).

**Code paths below are cited at `origin/main` `0b39255b`.** `tools/xtask/src/main.rs` is shortened to `xtask:`.

## 0. Summary

1. **"Installed on a VM" needs almost no new kernel code.** Two facts make it cheap:
   - The image A ISO is already a hybrid image with a GPT and an EFI system partition built by xorriso
     (`xtask:6676-6704`). Written to a disk, UEFI firmware boots it; the OptiPlex has run that way for months
     (`docs/runbook-baremetal-arthur-optiplex.md:40`).
   - Image A's kernel already mounts a whole-disk NeoFS volume read-write at `/neofs` (`boot-neofs`, through
     `desktop` → `product`, `kernel/Cargo.toml:110-116,192-193`; mount at `kernel/src/main.rs:839-840`).

   So **rung 1 is an `xtask` VM image**: a boot disk holding the image A bytes, plus a NeoFS data disk. The model
   stays a boot module; persistence is real. This is the OptiPlex layout, built by a tool instead of by hand.
2. **One-disk installs need two real pieces of new kernel work,** both on the safety boundary:
   - NeoFS on a **GPT partition**. Today NeoFS is whole-device only: absolute superblock offsets, no partition
     view anywhere (`kernel/src/fs/neofs/format.rs:26,336-340`; `inode.rs:1802-1893`).
   - **A write-guard rule for that partition.** Today a NeoFS inside a partition can never arm: the guard's NEOFS
     claim always starts at byte 0, and any claim overlapping a GPT partition is dropped
     (`kernel/src/drivers/block/write_guard_logic.rs:577-603,656-659`).

   The second widens what the internal-disk write guard arms. The guard is a CLAUDE.md standing rule (#85/#88),
   not a row in the invariant ledger (`docs/complexity-governance.md`, I-1..I-16). Widening it is still done
   relax-and-re-prove, with the Windows-GPT KATs kept green. Whether the guard should become an I-17 row is a PM
   call (§9); the ledger is at its page-fit alarm threshold.
3. **The on-device installer writes prebuilt partition payloads**, not a whole-disk copy. There is no
   partitioner and no FAT writer in the kernel (§4).
   - The kernel writes a fresh GPT computed for the target's own sector size and capacity: a pure, KAT'd
     function with a byte oracle (§4.4). It never copies a GPT across sector geometries.
   - It writes a host-built ESP template matched to that sector size, and a host-built NeoFS seed.
   - It fills them from the bytes the running system booted from: the kernel file and modules Limine loaded,
     and the model and vocab boot modules. The payload arrives in RAM as Limine modules, because the kernel can't
     read files from the USB (it has no ISO9660 or USB mass-storage driver). So pulling the USB mid-install
     changes nothing.
   - An install is a recoverable state machine (§4.7). A partly written disk is recognisable as "LogansOS
     (installation incomplete)" and can be finished.
   - No EFI variable is set. Boot uses the UEFI removable-media path `\EFI\BOOT\BOOTX64.EFI`.
4. **Try vs Install is a Limine boot menu,** with Try as the default:
   - two entries, Try (default) and Install, with a short timeout instead of `timeout: 0`;
   - the kernel reads the entry's command line (a Limine request it doesn't make today);
   - the desktop gains an "Install LogansOS…" item that starts the same installer.
   - Today a Try boot can write an installed LogansOS disk it finds (§6.3); J.c.'s fix is to mount it read-only.
     §6.2 lists every disk-writing path in the Try image that was checked. The claim "Try writes nothing" is
     limited to those paths.
5. **Model from disk** is a kernel pin check on the NeoFS path, which has none today. Arthur keeps its boot-module
   path for Try, and adds the NeoFS path for an installed system. The `QWEN25_05B` pins stay the single
   authority (`kernel/src/boot_model_logic.rs:28-35,61-83`).
6. **A/B (OT-51): deployments stay in NeoFS, as OT-51 says** (§7, option (a)).
   - Limine can't read NeoFS. So the ESP holds a **boot copy** of each slot's kernel and modules, written by
     activation into preallocated slot files.
   - The model and user data live on NeoFS, outside every slot.
   - This adds three requirements OT-51 didn't state: the ESP boot copy, a slot writer (not a FAT writer), and
     single-sector activation. They are proposed as an OT-51 clarification for ratification
     (`docs/THINK-2026-07-10-ot51-update-rollback-model.md:38-43,105,109,125`).
   - Measured budget: one image A deployment is 133.7 MB as shipped today. Two fit a 512 MiB ESP with about
     256 MiB to spare (§7.2).
7. **The Zenbook's internal NVMe stays unwritten** (§2). "Erase disk and install" is in scope (J.c.), so "blank
   disks only" is no longer a layer on its own. What stops it today is Intel VMD hiding the disk. If a future
   kernel drives VMD, what stops it is a **protected-disk refusal** that the VMD lane must ship in the same change
   (§2, layer 5). The typed confirmation alone is not treated as enough.

## 1. What exists today, and what's missing

| Piece | Today | Citation |
|---|---|---|
| Image build | `xtask iso [--with-model]` → hybrid ISO (xorriso, Limine BIOS-CD + UEFI-CD, protective MBR, `-efi-boot-part`). No `limine bios-install`, so hard-disk BIOS boot isn't set up; UEFI boot from the image's ESP is | `xtask:6212-6221,6484-6715`, `6676-6704`, `6710-6712` |
| Model on the image | `/models/model-qwen2.5-0.5b.neom` + `/vocab/…vocb` as Limine boot modules, size and BLAKE3 pinned before anything is written | `xtask:6249-6289,6586-6645`; pins `kernel/src/boot_model_logic.rs:28-35,61-83` |
| NeoFS images | `tools/neofs-mkfs`: **whole-disk** image file, 4 KiB blocks, superblocks at 0 / 1 MiB / 64 MiB, ≥ 64 MiB, places models and files at the root (`--extra-model`, `--extra-file`) | `tools/neofs-mkfs/src/format.rs:19,26,848-853,1242-1345`; `main.rs:161-206` |
| Disk images for VMs | Whole-disk raw files only (NeoFS, scratch, log-sink). No GPT, no qcow2. `qemu-img` only converts raw → VMDK | `xtask:10487-10535,11698-11798,16166-16210,7909-7943,8296-8323` |
| QEMU boot | Always `-cdrom` + `-boot d`, SeaBIOS (no OVMF anywhere in xtask or CI) | `xtask:8788-8796,8830-8842` |
| NeoFS driver | Read-write, files ≤ 2 GiB, **whole device only**. The mount requires root entries `hello.txt` (Regular) and `model.neom` (Model) | `kernel/src/fs/neofs/mount.rs:49-55,320-350`; `format.rs:22-23` |
| Boot mount | `boot-neofs` mounts the **first** block device whose probe passes, read-write, at `/neofs`; failures only degrade | `kernel/src/fs/mod.rs:774-811,840-901` |
| Block writes | AHCI, NVMe, virtio-blk all write; every device is wrapped in `GuardedDevice` | `kernel/src/drivers/block/mod.rs:102-397,439-460` |
| Write guard | Arms only: a NEOFSv1 superblock (claim from byte 0), a sealed log-sink header at 256 MiB, or a sealed scratch sector. A valid GPT drops any claim overlapping a partition; an invalid or protective table makes the disk opaque | `write_guard_logic.rs:40-71,127-131,571-670,286-373,445-545` |
| GPT | Parsed read-only, and only in the guard | `write_guard_logic.rs:259-284,445-545` |
| Partition-table writing | None anywhere (kernel or tools). The ISO's GPT is xorriso's | — |
| UEFI variables | None. The kernel makes no EFI system-table request. Boot entries were made by hand with `efibootmgr` from Linux | `kernel/src/main.rs:116-152`; `docs/runbook-baremetal-m710q-gen9-eu.md:52-62` |
| Model from NeoFS | `sys_model_load` opens any non-`/boot/models/` path as a VFS file and builds the model. **No pin check on that path**; integrity is NeoFS's per-inode content hash | `kernel/src/arch/x86_64/syscall.rs:2514-2565`; `kernel/src/fs/neofs/inode.rs:1208-1219,1440-1478` |
| Arthur's model path | Product build (`arthur-boot-module`): `/boot/models/model-qwen2.5-0.5b.neom` and `/boot/vocab/…vocb`; other builds: `/neofs/model-qwen2.5-0.5b.neom` | `userland/neo-arthur/src/main.rs:180-208,275-283` |
| No model | Absent → "Boot the LogansOS USB it came with." | `userland/neo-arthur/src/model_state_logic.rs:45-58` |
| First-boot state | All on `/neofs`: grants, memory, the intro flag, accessibility flags. The `.arthur` home is made only by host `neofs-mkfs` | `userland/libneo/src/grant_store.rs:175-215`; `userland/neo-init/src/desktop_session.rs:3531-3560`; `tools/neofs-mkfs/src/format.rs:584-590` |
| A/B updates | Design only (OT-51). Nothing implemented; one Limine entry | `docs/THINK-2026-07-10-ot51-update-rollback-model.md:8,124-129`; `boot/limine.conf:8-9,17` |

**Missing, in dependency order:**
- a VM disk image and a QEMU boot-from-disk scenario, under UEFI;
- a pin check for the model on NeoFS, and Arthur's NeoFS model path in the product build;
- a partition view under NeoFS, and the guard rule for a LogansOS partition;
- a GPT writer (pure, with a byte oracle), the ESP templates and slot writer, and the install bundle;
- driver-reported identity and a registry generation (§2a);
- the installer, the Try/Install selection, and the Try-writes-nothing fix;
- the A/B slot layout.

## 2. Targets, and how the Zenbook's internal disk stays unwritten

Targets, in order:
- **(a)** a VM (QEMU, virtio-blk first, then NVMe), which also makes every rung testable in CI;
- **(b)** the Arthur OptiPlex, whose NVMe is ours (`docs/runbook-baremetal-arthur-optiplex.md:38-82`);
- **(c)** other PCs.

**The Zenbook's internal NVMe is never a target.** That is today's rule (CLAUDE.md; write guard #85/#88).
**"Erase disk and install" is in scope** (J.c., 2026-10-27), so the installer can write a disk that holds someone
else's data. The write guard's default therefore no longer protects a disk the user chooses to erase, and the
Zenbook needs its own layers:

1. **It is invisible today.** On the Zenbook the NVMe sits behind Intel VMD, which the kernel does not drive
   (`kernel/src/drivers/pci.rs:258-275`). It is never registered as a block device, so the installer cannot list
   it.
2. **The guard's default is unchanged.** A Windows GPT is opaque and nothing on it arms (host KAT
   `windows_gpt_disk_never_arms`). The new partition rule (§4) arms only a partition whose **type GUID is
   LogansOS's NeoFS GUID** and whose first block is a valid NEOFSv1 superblock. Only the install claim (point 4)
   lets a write past the guard onto a foreign table, and only for one disk, once.
3. **The installer never offers a disk it didn't enumerate,** and refuses outright the disk the running system was
   booted from. Limine's executable-file response names the GPT disk GUID of the boot medium. The installer
   refuses any disk whose GPT (read through the guard's parser) carries that GUID, so Install can't overwrite the
   USB it runs from.
4. **Erasing needs a typed, identity-bound confirmation** (R4b).
   - The installer lists each disk with model, serial, size and what it recognised on it, using the guard's own
     read: "blank", "LogansOS", "Windows (NTFS)", "Linux (ext4)", "BitLocker", "unknown partition table".
   - Erasing a non-blank disk needs the user to type the disk's size, or a short code shown beside it.
   - The kernel then grants a **one-shot install claim** to that exact device object (§2a). The claim is
     revalidated before every write, and it fails closed.
   - The claim is the only path past the guard to a disk with a foreign table. It expires after one install.
5. **A protected-disk refusal, for the day a kernel drives VMD.**
   - **Plainly: if a future kernel drives VMD, the Zenbook's NVMe becomes a disk the installer can list.** Layers
     2–4 then permit erasing it if a user types the confirmation. Layer 3's boot-disk refusal doesn't help, since
     the system booted from the USB.
   - So, as a standing rule, **any VMD driver dispatch must ship a protected-disk refusal in the same change:** the
     installer refuses, with no override, any disk listed in the image's protected-disk table. Entries are an NVMe
     serial plus a model/capacity pair, recorded by the PL from Windows for the Zenbook.
   - The refusal can be backed by the Zenbook platform quirk identity the kernel already matches
     (`kernel/src/drivers/i2c_hid/quirk_logic.rs`: DSDT OEM pair + PCI IDs). On that platform the installer offers
     no internal disk at all.
   - Until VMD is driven, layer 1 is what stops it, and layers 4–5 are the backstop.

The bare-metal installer rungs never run on the Zenbook. Its runbook already forbids writing the internal disk, and
the Kingston stick is the only medium written there.

## 2a. Device identity and the install claim (review items 2 and 5)

**Today.** `BlockDeviceIdentity` is only `{driver, name, pci_bdf}` (`kernel/src/drivers/block/mod.rs:62-67`). No
driver reports a model, a serial or a removal, and devices are registered once at boot (`:439-460`). A
`(model, serial, capacity)` tuple, even with those added, is not proof of identity: virtual disks often report an
empty or a duplicated serial, and a replacement disk can report the same tuple.

**Design: the claim names a kernel-held device object, not a description.** All of the following is new work:

1. **A registry generation.**
   - Each registered device gets a slot with a `generation: u64`. The generation is bumped whenever the slot's
     device is replaced, re-registered, or declared dead.
   - "Dead" means a driver reports a fatal state: NVMe controller fatal status, an AHCI port that lost its
     device, a virtio-blk reset or removal. That driver reporting is also new work.
   - The installer's enumeration returns opaque **device tokens** `(slot, generation)`, never bare indices.
2. **Identity reported by the driver, where it exists.**
   - New fields: model, serial, capacity, logical sector size.
   - Sources: NVMe Identify Controller (SN, MN) plus the namespace size; ATA IDENTIFY words 10–19 and 27–46;
     virtio-blk `GET_ID`.
   - An empty, blank or non-printable serial counts as **"identity unavailable"**.
3. **A content fingerprint, kept current by the installer.**
   - At enumeration: BLAKE3 of the first and the last MiB.
   - The installer keeps an **expected** fingerprint: the enumerated bytes overlaid with every byte it has since
     written to those two ranges (the protective MBR, both GPT headers and arrays). It knows those bytes exactly,
     because it computed them (§4.4).
   - A re-read is compared with the expected fingerprint, never with the enumeration digest, so the installer's
     own writes don't fail it.
   - The live claim holds the expected bytes (2 MiB) and compares a re-read 64 KiB at a time; enumeration rows
     keep only the digests, because the default kernel heap is 4 MiB.
   - The confirmation screen shows what was recognised from those reads.
4. **The claim holds the device object, not a slot.**
   - Shape: `{device: Arc<GuardedDevice>, slot, generation, identity snapshot, expected fingerprint, kind}`.
   - `kind` is `fresh` (blank or incomplete, §4.5) or `destructive` (erase, or reinstall over a completed
     LogansOS disk). It is one-shot.
   - Every install write goes through a `ClaimedWriter` that owns that `Arc`. A registry change can't redirect a
     write to another device object.
5. **The synchronization rule, and what recurs.**
   - **The rule.** Every driver event that can mean a different medium bumps the device's own
     `media_generation` (an atomic on the device object): removal, a link or port reset, namespace-change or
     reset notification, or a fatal status. That event reporting is new work.
   - **Per submission (every write request):** the claim is alive; the device is alive; `media_generation` is
     read **before** submission and again **after** completion, and must equal the claim's both times.
     - A mismatch before ⇒ not submitted.
     - A mismatch after ⇒ the write may have reached another medium. The installer stops at once, writes nothing
       more, and reports the disk's last durable state (§4.7).
   - **Per step boundary (W1…W6, after that step's flush):** the identity is re-read (Identify / IDENTIFY /
     `GET_ID`) and must equal the snapshot. The first and last MiB are re-read and must equal the expected
     fingerprint.
   - **What each check guarantees, and what it doesn't.**
     - **A reported swap has a guaranteed bound.** A medium swapped behind the same device object bumps
       `media_generation`. The next submission's pre-check or the current request's completion check catches it.
       Exposure is at most the requests in flight between the swap and that check. Software can't stop a disk
       being physically swapped while one command is in flight.
     - **That guarantee needs reliable media-change reporting from the driver.** It is a prerequisite, not
       something the installer can supply. Each driver's reporting (NVMe namespace or reset notices, AHCI
       PhyRdy and port-reset status, virtio config change) is part of the R4 work, listed per driver.
     - **An unreported swap has no guaranteed bound.** The step-boundary identity and fingerprint checks are
       change detectors, not proof of sameness.
       - They catch a replacement whose identity differs, or whose first or last MiB differs from the expected
         bytes at a boundary.
       - They can't catch a replacement that reports the same identity and whose distinguishing bytes in the
         sampled windows the current step overwrites. Example (relay #88): disk B has A's serial, model and
         size and A's last MiB, and differs in the first MiB only in the sectors W2 writes. After W2 both windows
         match, and every later boundary passes too.
       - Such a replacement can stay undetected for the rest of the install. Its interior is overwritten from W3
         on.
     - **What this means in practice.** The case needs two disks with identical identity, swapped mid-install
       with no reset or link event. On the targets in scope (§2), any removal or reinsertion resets the link, so
       a working driver reports it. The design relies on that reporting and says so. It doesn't claim more.
6. **Unavailable identity fails closed for every destructive install.**
   - A disk with no stable identity can't be erased, and can't be reinstalled over a completed LogansOS install
     (§4.5): "this disk doesn't identify itself, so LogansOS won't overwrite it".
   - It can take a `fresh` install (blank or incomplete). The step-boundary fingerprint check still applies.
     - A wrong blank disk loses nothing.
     - An incomplete disk never held user data, because `boot-neofs` never mounts an INCOMPLETE partition (§4.2).
   - A QEMU virtio disk without `serial=` is in this class. The VM scenarios set `serial=` where they test
     destructive installs.

**Tests** (host KATs on the claim logic with a model registry, plus QEMU where a device can actually be presented):

| Case | Expected | Where |
|---|---|---|
| Reorder: the slot behind a token now holds another device | `generation` mismatch → refused, no write | host KAT |
| Removal and reinsert of the same disk | dead → new generation → the old token is refused; re-enumerate and re-confirm | host KAT (no block hotplug in the kernel today) |
| Identity unavailable (virtio, no `serial=`) | erase refused with the reason; blank install allowed | QEMU |
| Replacement with the same tuple: two virtio disks with the same `serial=` and size and different contents; the token is swapped to the other through a test hook | fingerprint mismatch → refused before the first write | QEMU + host KAT |
| Replacement **after the first write** (between steps; the hook keeps the slot and generation) | the step-boundary fingerprint (expected bytes include our GPT) fails → stop, report | host KAT + QEMU |
| Replacement **between validation and submission** (a reported swap: `media_generation` bumps after the pre-check) | the post-completion check fails → stop after that request, nothing more written | host KAT (a model device that bumps mid-request) |
| Replacement the driver doesn't report, mid-step, with a differing identity or differing sampled bytes | caught at that step's boundary check | host KAT |
| Replacement the driver doesn't report, **masked by the current step** (relay #88: same identity, same last MiB, the first MiB differs only in sectors W2 writes; swapped before W2) | **not detected**; the install completes on B. The KAT asserts this outcome, documenting the limit, so a later claim of a stronger bound would fail it | host KAT |
| Completed LogansOS disk with no identity (virtio, no `serial=`, with an `.arthur` memory file) | reinstall refused; the disk is bit-identical afterwards | QEMU |
| Completed LogansOS disk with identity | reinstall only after the destructive typed confirmation | QEMU |
| The boot disk | refused at enumeration (GPT disk GUID equals the boot medium's) | QEMU |
| A protected-disk table entry | refused, no override | host KAT |

The Windows-GPT guard KAT stays. It tests the guard; these test the claim.

## 3. The shortest path to "installed on a VM": rung 1 is an `xtask` image

**The design.** `cargo xtask vm-image [--with-model <neom>]` writes two raw disks:
- `boot.raw`: the image A ISO's bytes, unchanged. Its GPT and ESP are xorriso's; UEFI boots `\EFI\BOOT\BOOTX64.EFI`
  (Limine), which loads the kernel and modules from the image (`boot():`).
- `data.raw`: a whole-disk NeoFS volume from `neofs-mkfs`. It holds:
  - the mount's required `hello.txt` and `model.neom` stubs (`mount.rs:320-350`);
  - the `arthur-memory` variant's `.arthur` home (`tools/neofs-mkfs/src/format.rs:584-590`).

  It holds no model in rung 1.

QEMU boots it with OVMF, `-drive if=virtio,file=boot.raw,format=raw` (index 0) and `-drive if=virtio,file=data.raw`,
and **no `-cdrom`**. The kernel mounts `data.raw` at `/neofs` (the first device whose NeoFS probe passes,
`kernel/src/fs/mod.rs:774-811`). The intro flag, grants and memory then persist across reboots: the persistence
the intro card says a USB can't give (`userland/neo-atrium/src/intro.rs:37,101-102`).

**What it proves and what it doesn't.**
- **Proves:** booting from a disk instead of an optical drive, under UEFI; the product image unchanged; the
  installed system keeping state across a reboot.
- **Doesn't prove:** one-disk installs; the model off the boot disk; the installer.
- The model is still a 485 MB boot module on the boot disk. That is acceptable for a VM, and it is exactly the
  OptiPlex's proven layout.

**Why rung 1, not an installer first.**
- It needs no kernel change and no new write path.
- It gives every later rung a CI-testable target: the same QEMU line with the installer's output disk in place
  of `boot.raw`.
- The cost is mostly CI: OVMF on the runners (the QEMU ACPI corpus sets were extracted under OVMF, so it is
  available on the PL's side; the runners need the `ovmf` package or a pinned `OVMF_CODE.fd`) and a scenario
  that boots twice.

**Alternative considered: a single disk with `limine bios-install` and SeaBIOS.** It avoids OVMF, but NeoFS still
can't share that disk (no partition view), so it still needs a second disk. It also tests a BIOS path no target
uses. Rejected for rung 1; kept as a later CI convenience.

## 4. The on-device installer (Install LogansOS)

### 4.1 Layout of an installed disk (single disk, GPT)

| # | Partition | Size | Contents |
|---|---|---|---|
| 1 | ESP (FAT32, type `C12A7328-F81F-11D2-BA4B-00A0C93EC93B`) | 512 MiB | `\EFI\BOOT\BOOTX64.EFI` (Limine), `\limine.conf`, the preallocated slot files `\slot\A\…` and `\slot\B\…` (§7) |
| 2 | LogansOS NeoFS (a new LogansOS type GUID, minted once and recorded in the pure GPT module) | to the end, 1 MiB-aligned | `hello.txt` and `model.neom` stubs; `/deploy/A/…` (the authoritative deployment, §7); `model-qwen2.5-0.5b.neom` + vocab; `.arthur/…`; the intro and accessibility flags |

- **Model and user data share one NeoFS volume.** That keeps the existing risk of a shared free map
  (`docs/THINK-2026-10-19-arthur-first-boot-model.md:76-78`). A read-mostly model partition is the clean fix
  later.
- **No log sink on an installed disk.** The sink is a test artifact.

### 4.2 Kernel prerequisites (rung 3)

1. **A partition view.**
   - `PartitionDevice { inner, first_lba, lba_count }` offsets and bounds every request.
   - It is built from a validated GPT entry, using the guard's parser (`write_guard_logic.rs:445-545`) moved to a
     shared pure module.
   - NeoFS mounts it unchanged, because its offsets are already device-relative.
   - `boot-neofs` probes LogansOS-typed partitions before whole devices.
   - It skips a partition marked INCOMPLETE (§4.7), and mounts a found one read-only in Try (§6.3).
2. **The guard's partition claim (relax-and-re-prove).** It arms exactly `[first_lba, last_lba]` of a GPT entry
   when all of these hold:
   - the GPT validates;
   - the type GUID is the LogansOS NeoFS GUID;
   - the partition's first 4 KiB is a valid NEOFSv1 superblock;
   - `total_blocks × 4096` is no larger than the partition.

   Everything else is as today. Proof:
   - every existing guard KAT, `windows_gpt_disk_never_arms` included;
   - the §4.4 byte oracle arms partition 2 only;
   - a disk with only the type GUID right, or only the superblock, arms nothing;
   - a superblock that claims past its partition arms nothing.

### 4.3 What the Try medium carries, and how the installer reads it

The kernel can't read files from the USB. It has no ISO9660 and no USB mass-storage driver (`kernel/src/drivers/usb/`
holds HID, CDC-ECM and mt7601u only). Every byte the installer writes therefore arrives as a **Limine module, in RAM,
loaded only by the Install entry** (§6.1), so Try boots don't pay for it.

1. **The install bundle module** (`/install/bundle`, one module, which keeps under `MAX_BOOT_MODULES = 8`,
   `xtask:5308`). It is a new format, `NEOINST1`, and ships with an I-12 byte oracle. It holds:
   - `esp-512.fat` and `esp-4096.fat`: two ESP templates. Each is a FAT32 image with the matching
     bytes-per-sector. xtask builds it **on the host** with vendored `fatfs` (§11.2; `std` features are fine
     there), replacing today's `mformat` call (`xtask:11834`). It holds Limine's `BOOTX64.EFI` (the pinned
     `LIMINE_VERSION`), an initial `limine.conf`, and the preallocated contiguous slot files. The template build verifies each slot file's
     cluster chain is contiguous, reading back the image `fatfs` wrote, and generates the **slot map** from that
     output: file → first sector, length in sectors, directory-entry sector and offset. A non-contiguous chain
     fails the build;
   - `neofs-seed.img`: a small `neofs-mkfs` image with the stubs and the `.arthur` home;
   - the slot maps and a manifest of each part's BLAKE3.
2. **Its integrity pin.**
   - xtask builds the bundle **before** the kernel, and the kernel is compiled with the bundle's BLAKE3 as a
     constant (`option_env!`).
   - xtask refuses to assemble an image whose staged bundle doesn't hash to it, the same discipline as the
     model's pin check (`xtask:6249-6289`).
   - The kernel checks the bundle's hash, then each part against the manifest, before the first write.
3. **The deployment is the running system.** The kernel, `neo-init`, `neo-worker`, `neo-shell` and `neo-arthur`
   written to the disk are the exact bytes Limine loaded:
   - the kernel through Limine's executable-file request (new; the kernel makes no such request today,
     `kernel/src/main.rs:116-152`);
   - the userland through the module response the kernel already holds.

   So the bundle never carries a second copy of the system.
4. **The model and vocab** are the pinned boot modules (`kernel/src/boot_model_logic.rs:28-35,61-83`). They are
   **consumable**: taking one for the model build reclaims its pages (`kernel/src/boot_modules.rs:385`). So in
   Install mode `boot-model-preload` is skipped. The installer streams the untaken module to NeoFS, re-checks
   the pin through NeoFS's content hash (§5), and only then reclaims it.

### 4.4 The GPT the installer writes (a byte spec)

**Geometry.** `S` is the target's logical sector size (512 or 4096, from `BlockGeometry`), and `L` is the
target's last LBA. Any other `S` is refused. The ESP template is chosen by `S`. NeoFS is sector-size neutral (byte
offsets in 4 KiB blocks). Nothing is copied across geometries.

**The layout function** `gpt_layout(S, L, disk_guid, esp_guid, neofs_guid) -> Layout` is pure. With
`A = 16384 / S` (sectors in a 128-entry × 128-byte entry array: 32 for 512 B, 4 for 4 KiB) and
`M = 1 MiB / S` (2048 or 256):

| Field | Primary (LBA 1) | Backup (LBA `L`) |
|---|---|---|
| Signature @0 (8) | `45 46 49 20 50 41 52 54` ("EFI PART") | same |
| Revision @8 (4) | `00 00 01 00` | same |
| HeaderSize @12 (4) | 92 | 92 |
| HeaderCRC32 @16 (4) | **CRC-H1**: CRC32 of bytes 0..92 of this header with @16..20 zeroed | **CRC-H2**: the same over the backup header |
| Reserved @20 (4) | 0 | 0 |
| MyLBA @24 (8) | 1 | `L` |
| AlternateLBA @32 (8) | `L` | 1 |
| FirstUsableLBA @40 (8) | `2 + A` | `2 + A` |
| LastUsableLBA @48 (8) | `L − 1 − A` | `L − 1 − A` |
| DiskGUID @56 (16) | `disk_guid` (mixed-endian on disk) | same |
| PartitionEntryLBA @72 (8) | 2 | `L − A` |
| NumberOfPartitionEntries @80 (4) | 128 | 128 |
| SizeOfPartitionEntry @84 (4) | 128 | 128 |
| PartitionEntryArrayCRC32 @88 (4) | **CRC-E**: CRC32 of the 16384-byte array | **CRC-E** (the same array, the same value) |
| @92..S | zero | zero |

**The entries** (all 128 are written; entries 3..128 are all-zero). Each has the type GUID @0, unique GUID @16,
StartingLBA @32, EndingLBA @40 (inclusive), attributes @48 and a UTF-16LE name @56 (72 bytes):
- **Entry 1, the ESP:** start `M`, end `M + 512·M − 1`, attributes 0, name "EFI System".
- **Entry 2, LogansOS NeoFS:** start `513·M`, end `E = ⌊(L − A) / M⌋ · M − 1` (the last 1 MiB boundary that ends
  at or before LastUsableLBA). Attributes: **bit 48 = INCOMPLETE** while installing (§4.7). Name "LogansOS".
  - NeoFS `total_blocks = (E + 1 − 513·M) · S / 4096`, which is a whole number because `M·S` is 1 MiB.
  - This proves the partition ends before the backup entry array: `E ≤ L − A − 1 < L − A`.

**Protective MBR (LBA 0).** Bytes 0..446 are zero (no boot code). One partition record @446: status 0x00, CHS
start `00 02 00`, type 0xEE, CHS end `FF FF FF`, starting LBA 1, size `min(L, 0xFFFFFFFF)`. Records 2–4 are zero,
and the signature @510 is `55 AA`. It is the same for both sector sizes; on 4 KiB, the rest of LBA 0 is zero.

**Three CRCs, named separately.** CRC-E is over the entry array. CRC-H1 and CRC-H2 are over each header, with its
own CRC field zeroed. They differ, because MyLBA, AlternateLBA and PartitionEntryLBA differ. All three are
CRC-32/ISO-HDLC (polynomial 0x04C11DB7, implemented reflected as 0xEDB88320; init and xorout 0xFFFFFFFF), the
same as the guard's (`write_guard_logic.rs:259-284`, the reflected constant at `:268`). Changing an entry (clearing INCOMPLETE) changes CRC-E and therefore both header
CRCs.

**The byte oracle (I-12; the R4 deliverable).** Two hand-laid fixtures for a 4 GiB disk:
- `S = 512`: `L = 8,388,607`, `A = 32`, `M = 2048`, FirstUsable 34, LastUsable 8,388,574, backup array at
  8,388,575; ESP 2048..1,050,623; NeoFS 1,050,624..8,386,559, which is 916,992 NeoFS blocks.
- `S = 4096`: `L = 1,048,575`, `A = 4`, `M = 256`, FirstUsable 6, LastUsable 1,048,570, backup array at
  1,048,571; ESP 256..131,327; NeoFS 131,328..1,048,319, also 916,992 blocks.

Each fixture lists every non-zero byte of LBA 0, both headers and entries 1–2, with fixed GUIDs. The CRCs are
computed when the fixture is laid out, by Python's `zlib.crc32`, independently of the writer. Three checks:
- the KAT asserts the writer's output equals the fixture;
- the guard's own `validated_gpt` accepts it;
- `sgdisk -v` accepts it (host-side, when the fixture is made).

### 4.5 Choosing a disk (keyboard, touchpad, console)

The installer is an Atrium app (`neo-installer`) with a console fallback (`neo-shell install`) for when the
desktop isn't running.

> **R4.4 decision (PM, 2026-10-29):** the installer UI is a WM view in neo-init; the cap holders are neo-init and
> neo-shell only; B is deferred until a second input-receiving client exists for another reason. (So there is no
> `neo-installer` app: the view is `userland/neo-init/src/installer_view.rs`. Option B was an installer role of
> `neo-worker` with pointer forwarding to it.) R4.4 opens the view when `install` is typed at Home, as `help` opens
> the help card; the Home Spine chip comes in R4.4b, after #185's app icons, and shows only on the Try boot.

> **R4.4b (PM decision (a), 2026-10-29): when the Install chip shows, until R5.** The Try boot doesn't exist yet
> (R5 adds its flag), so the chip's interim predicate is: neo-init holds the Installer cap **and** one
> `SYS_INSTALL_LIST` returns at least one row not refused `MEDIUM` (no bundle, no pin, pin mismatch, a bad or
> incomplete bundle, no model, slot too small). The chip exists only under the `installer` feature; with it off,
> Home's pixels and neo-init's `.text` are unchanged.
> - **The R5 rewire point** is `chip_shown()` in `userland/neo-init/src/installer_view.rs`, the only place that
>   decides. R5 replaces its body with the Try flag, and makes activating the chip on a non-Try boot an
>   explain-and-restart action instead of opening the list.
> - **When LIST runs, and what it costs.** Once per session, from `installer_view::start` at desktop session
>   start, never per frame. It runs one `installer::preflight` per enumerated disk. Each preflight first loads
>   and checks the bundle against its pin (a medium refusal stops there, before any claim). Otherwise it takes
>   that disk's install claim, reads the disk to classify it, and drops the claim with no write, so the kernel
>   logs `claimed_after=0` after the call. Nothing is written. Without the cap, LIST fails and the chip stays
>   hidden.
> - **No re-run trigger.** A disk attached after session start doesn't change the chip for that session. Opening
>   the installer view runs its own LIST, so the list itself is always fresh.

- **The list** shows one row per enumerated disk:
  - model and serial, or "identity unavailable";
  - size, and the logical sector size;
  - what the guard recognised, in three LogansOS-relevant classes plus foreign ones:
    - **blank** (no table, first MiB zero);
    - **LogansOS (installation incomplete)** (our GPT with INCOMPLETE set, never mounted, so no user data);
    - **LogansOS (installed)** (our GPT, complete, may hold user data);
    - Windows (NTFS) / Linux (ext4) / BitLocker / unknown partition table;
  - and either the action or the refusal reason:
    - actions: "Install" (blank), "Finish installing" (incomplete), "Reinstall (erases your LogansOS data)"
      (installed), "Erase and install" (foreign);
    - refusals: "this is the disk LogansOS is running from", "too small: needs 2 GiB", "protected", "can't be
      identified, so LogansOS won't overwrite it", "this disk can't confirm its writes are saved, so LogansOS won't
      install to it" (the device has no flush; §4.7's durability rules need one; added 2026-10-29, Jarvis #116).
  - Refused rows are shown, but can't be selected.
- **Keyboard:** Up/Down or Tab to move, Enter to choose, Esc to go back. **Touchpad or mouse:** click a row, then
  click the button. Both go through the existing Atrium input paths, the USB HID and i8042 keyboards and the USB
  and I2C-HID pointers.
- **Confirmation:**
  - For a blank or incomplete disk (`fresh`): one screen naming the disk, then Install or Finish installing.
  - For a **destructive** install (erase a foreign disk, or reinstall over an installed LogansOS):
    - the screen names the disk and what's on it ("Windows (NTFS), 3 partitions", or "LogansOS, installed, with
      your memories and settings");
    - the user types the 4-character code shown beside it (a size is ambiguous to type, so the code is used);
    - then Erase and install / Reinstall.
    - It requires stable identity (§2a, item 6).
  - **A data-preserving reinstall** (keep NeoFS, replace only the deployment) is an update. It is OT-51's
    activation, not the installer's, and is out of scope here.
- **Console fallback:** a numbered list. The user types the number, then the code.

### 4.6 Minimum target size

The installer refuses, **before any write**, a disk smaller than: the GPT overhead (`2 + 2A` sectors) + the
1 MiB alignment + a 512 MiB ESP + a NeoFS of at least 1 GiB. The NeoFS minimum covers the 64 MiB+ seed (the third
superblock copy sits at 64 MiB), the model's 485,489,152 bytes, the vocab's 7,990,862 bytes and room for
`.arthur`. **The minimum is 2 GiB**, checked against the reported capacity. The R4 scenarios include a 1 GiB disk
(refused, zero writes) and a disk one sector under the minimum.

### 4.7 The install state machine (power loss, USB removal, target removal)

**Two separate assumptions, each with its own mechanism:**
- **Atomicity:** a single logical-sector write is atomic (NVMe atomic write unit ≥ 1 LB; SATA sector writes).
  It says nothing about order or persistence.
- **Persistence and order** come only from **flush barriers**: the block API's `flush` (`kernel/src/drivers/block/mod.rs:77-83`),
  implemented by NVMe and AHCI (`nvme.rs:414`, `ahci.rs:336`) and required of virtio-blk (F_FLUSH).
  - A read-back can be served from a volatile cache, so it proves content, never persistence.

**The durability rules:**
1. **No flush, no install.** A target whose `supports_flush()` is false is refused before any write: "this disk
   can't confirm writes reached it".
2. **Each step is: write → flush → read back and verify → flush (if the verify wrote nothing, this is a no-op) →
   step-boundary identity check (§2a) → the next step.** A step is complete only when its flush returned success.
3. **Commit points are written only after everything they vouch for is durable.**
   - **Every GPT copy (W1, W2, both halves of W6) is written array first:** the entry array, flush, read back;
     only then the header sector that makes that array recognisable, flush, read back. A **newly submitted** header
     therefore never precedes its matching durable array (the relay #88 header-only trace). An *old* header (a
     foreign primary during W2, or our INCOMPLETE copy during W6) can briefly mismatch the array that replaces
     its own; the other GPT copy, already valid and durable, is what makes that interval recoverable.
   - W1's backup GPT header (INCOMPLETE) is flushed before W2.
   - W2's primary GPT is flushed **before W3 changes any byte of a partition**. For an erase or reinstall, the
     recoverable INCOMPLETE marker is durable before any destructive payload write.
   - All of W3–W5 is flushed before W6 starts.
   - W6 flushes after the backup copy and again after the primary copy. "Installed" is reported only after the
     second flush.
4. **A flush error fails the step, and its outcome is uncertain.** A failed flush doesn't mean nothing it covered
   persisted: any subset of its writes may have reached the media, in any order.
   - **Two different facts, kept apart:**
     - **No further write is submitted** after a failed flush, so no *later* marker can advance. This is
       certain.
     - **The writes the failed flush covered, including a marker it covered, may or may not have persisted.**
       This is uncertain. Example: W6's backup copy is written and its flush fails. The backup may already say
       complete.
   - **Why the uncertainty is safe.** A marker is submitted only after a successful flush of everything it vouches
     for (rule 3). So whichever subset of an uncertain marker persisted, the disk classifies as one of the §4.7
     outcomes with its prerequisites durable. The worst case is an "installed" disk whose payload is fully
     durable.
   - **What the installer shows:** "The disk didn't confirm the write at step Wn, so LogansOS can't tell whether
     that step took effect. Restart, and the installer will show the disk's state." It never reports success
     after a failed flush. If W6 was the step, it says the disk may already be installed.
   - Finish installing may be retried, re-running from W3 with all barriers.
   - Two flush failures in one session end the session, with the advice that the disk may be failing.

Each step's writes are deterministic, so a step can be re-run.

| Step | Writes | A cut here leaves the disk... | Next boot's installer shows |
|---|---|---|---|
| W0 | Nothing (enumerate, confirm, revalidate the claim) | unchanged | as before |
| W1 | The backup entry array, **flush**, then the backup header (one sector) with entry 2 **INCOMPLETE**, flush | blank (no signature yet; a persisted array alone is unrecognised data in the last MiB) or valid-via-backup INCOMPLETE. For an erase target: still foreign until W2 | "blank", "incomplete", or the old contents (re-confirm) |
| W2 | The protective MBR and the primary entry array, **flush**, then the primary header, flush | valid INCOMPLETE (primary, or via the durable backup) | "LogansOS (installation incomplete)" → Finish installing |
| W3 | The ESP template into partition 1, then the slot A files from the running system, read back and verified | INCOMPLETE | Finish installing |
| W4 | The NeoFS seed into partition 2, then grown to `total_blocks` (R4's NeoFS grow) | INCOMPLETE | Finish installing |
| W5 | Model and vocab to NeoFS (the pin is checked through the content hash), the deployment's authoritative copy at `/deploy/A/`, and the `limine.conf` slot A entry | INCOMPLETE | Finish installing |
| W6 | **Clear INCOMPLETE**: the backup array, flush, the backup header, flush; then the primary array, flush, the primary header, flush (CRC-E, CRC-H2, then CRC-H1) | either copy may still say INCOMPLETE; the parser prefers a valid primary, else the valid backup | done, or Finish installing |

- **Finish installing always re-runs W3–W6,** with every barrier. INCOMPLETE records only "not finished", not how
  far an install got, so no shortcut to W6 is taken. W3–W6 are idempotent.
- It needs no destructive confirmation: the disk is ours (a valid GPT, our type GUID, INCOMPLETE set), and it
  never held user data.
- **A disk is bootable-and-mountable only after W6.** `boot-neofs` skips an INCOMPLETE partition, and Arthur's line
  says "LogansOS didn't finish installing here; start the USB and choose Install to finish".
- **USB removed mid-install.** The payload is in RAM (§4.3), so the install continues to W6 unaffected. Proof: the
  install scenario asserts **zero reads from the boot medium's block device** after enumeration (a guard read
  counter).
- **Target removed or failing mid-install.** The next write fails revalidation or returns an error. The installer
  stops and names the last completed step. On reinsertion the disk classifies per the table, never as stranded:
  W1's array-then-header order is what keeps "blank" or "valid INCOMPLETE" the only outcomes on a blank disk: the
  header that makes the backup recognisable is submitted only after its array is durable.
- **Power loss.** The same as target removal, observed at the next boot.
- **Tests.**
  - A host KAT of the state machine against a **write-back-cache disk model**. Unflushed writes are held in a
    volatile cache. At a power cut, any subset of them in any order may have reached the media (the model
    enumerates small cases exhaustively, plus seeded random subsets). Flushed writes always persist.
    - The cut is taken at every submission and every flush.
    - After every cut, the disk must classify as blank, unchanged-foreign, INCOMPLETE or installed.
    - **"Installed" must imply every payload hash matches.**
    - A flush that returns an error is modelled as persisting **any subset** of the writes it covered, in any
      order, and the model then stops submitting. The invariant above must hold for every such subset.
    - **Named traces** (each a fixed case besides the enumeration):
      - relay #88 *header-only survives*: W1's array is volatile and only the backup header persists. With the
        array-first barrier this can't occur; the model asserts the header is never submitted before the
        array's flush returned success. A model without that barrier must fail this trace with
        `opaque-gpt-invalid`, proving the trace bites.
      - *W6 backup flush fails after its sectors persisted*: the installer stops (rule 4) and the disk classifies
        as INCOMPLETE via the primary. Payload hashes match.
      - A **separate** trace, the restarted Finish installing that then reaches the primary update: if that update
        is cut, the disk classifies as INCOMPLETE or installed, payload hashes matching in both. It is a new
        attempt, never a continuation after the failed flush.
  - A QEMU scenario with a test-only `install-stop-after=Wn`: power off at each step, reboot, Finish installing,
    and the disk then boots. The 4-shard gate carries it.

**Why prebuilt payloads and not a copied disk image.**
- A GPT is in logical sectors, so a copied image is wrong on the other geometry.
- A fixed image would also need a resize, which is three CRC edits on someone else's layout.
- Computing a fresh GPT for the target is one pure function with a byte oracle.
- The FAT side is a host-built template, so the kernel still never formats or rewrites FAT structures. It writes
  sectors inside known slot extents and two directory-entry size fields (§7).

### 4.8 First-boot provisioning

- On the installed system's first boot, the intro runs and is recorded. The flag is already on NeoFS
  (`userland/neo-init/src/desktop_session.rs:3531,3675-3715`).
- The grants file starts at the D7 default (`userland/libneo/src/grant_store.rs:175-215`).
- The `.arthur` home comes from the NeoFS seed.
- Nothing else is new. A machine-name or user-name step is product work for later.

## 5. Model from the installed NeoFS volume (rung 2)

**Today's gap.**
- The model is pinned only on the boot-module path (`kernel/src/boot_modules.rs:326-402`).
- A model opened from `/neofs` is checked against its NeoFS content hash, not against `QWEN25_05B`
  (`syscall.rs:2514-2565`).
- That makes "the model on disk" integrity-checked but not provenance-checked. A model file of the right name but
  different bytes would load.

**Design.**
1. **A kernel pin check for NeoFS models.**
   - For a path under `/neofs/` whose name matches a pinned model (`model-qwen2.5-0.5b.neom`), the kernel compares
     the inode's verified BLAKE3 content hash with the pin.
   - They are the same quantity by construction (`kernel/src/boot_model_logic.rs:12`), so no second 485 MB hash is
     needed.
   - Mismatch ⇒ `EBADMSG` (Arthur's "Damaged"), the same as the boot-module path. The vocab gets the same rule
     against `QWEN25_05B_VOCAB` / `_VOCB`.
   - Unpinned names keep today's behaviour (other models, test fixtures).
2. **A preload from NeoFS** under `boot-model-preload`.
   - `boot_modules::preload` (`:521-580`) gains a second source: after the boot modules, `/neofs/<pinned name>`.
   - The result is cached under the boot-module path, so `sys_model_load` hands out the same handle (`:2524-2529`).
   - The model is built at boot, at IF=1 on the inference stack, as for the boot module: CP-74's fix carries over.
3. **Arthur's source order** (`neo-arthur`): try `/boot/models/…` (the USB, or a boot disk that carries it), then
   `/neofs/…`.
   - The model-state line gains a source tag (`ARTHUR model source=boot-module|neofs`, `main.rs:2655-2672`).
   - The Absent line changes for an installed system to say the model is missing from the disk, not "boot the USB".
4. **Provenance.** `Model::from_neom_bytes` already takes a `FileProvenance` (`syscall.rs:2546`). The installer
   writes the model's pin name and hash into NeoFS. The console line names the source.

**Kill-switch:** feature `model-from-neofs`. Off ⇒ the boot-module path only, bit-for-bit today (`.text` identity for
`desktop`, `product` and image A).

## 6. Try or Install

### 6.1 Where the choice is offered

1. **The boot menu (primary).** `limine.conf` gains a second entry:
   - `/Try LogansOS` (default, unchanged modules);
   - `/Install LogansOS` (the same modules plus `cmdline: install`).
   - `timeout: 0` becomes `timeout: 5`, so the menu shows for 5 s.
   - The kernel adds Limine's executable-command-line request, which it doesn't make today (`kernel/src/main.rs:116-152`).
   - `install` sets a boot flag that neo-init reads to start the installer instead of the desktop.
   - The Install entry also loads `/install/bundle` (§4.3) and skips the model preload.
2. **From the desktop while trying.** An "Install LogansOS…" item, in the shelf or Arthur ("install LogansOS on this
   computer").
   - It can't install from inside this Try session. The Try boot has already taken (and reclaimed) the consumable
     model module (§4.3, item 4), and the install bundle isn't loaded.
   - So it explains that and offers "Restart into the installer": a restart with a one-line instruction to choose
     **Install LogansOS** at the menu. Try writes no disk, so it can't set Limine's default itself.
   - A seamless in-session install would mean keeping a second 485 MB model copy in RAM. That is left for a later
     rung, if J.c. wants it.
3. **A first screen: no.** It would add a choice to every boot, Try users included. The boot menu plus the desktop
   item cover both kinds of user; the menu default is Try.
4. **An installed system shows no Install entry.** Its `limine.conf` is the installer's (deployment A, later B).

### 6.2 What each path writes, and what was checked

| Path | Writes |
|---|---|
| Try LogansOS (USB) | **No disk writes through any path in the audit below**, after the §6.3 fix. User state lives in memory for the session; the intro says so (`intro.rs:37`) |
| Install LogansOS | Only the disk the user picked and confirmed, under the install claim (§2a): the GPT, the ESP template and slot files, the NeoFS seed and its contents. Nothing on the USB (refused, §2, layer 3) and nothing on any other disk |
| Installed system | Only its own NeoFS partition (the guard's partition claim). The ESP slot files are written only by OT-51 activation |

**The Try write-surface audit** (review item 3), at `origin/main` `0b39255b`:
- **The block funnel.** Every device is wrapped in `GuardedDevice` (`kernel/src/drivers/block/mod.rs:439-460`);
  a write needs an armed extent (`:372-396`).
- **Every non-driver caller of `write_block` / `BlockOp::Write`** in the kernel, in three groups:
  - **Production:** NeoFS (`kernel/src/fs/neofs/inode.rs:935,1398,1908,2102,2115,2512`, `cache.rs:222`). It is
    reached only through the `boot-neofs` mount, which becomes read-only in Try (§6.3). NeoFS already has
    `mount_readonly`, suppresses its writeback worker when read-only, and checks write permission on mutations
    (`mount.rs:49`; `inode.rs:114,187`).
  - **Test-only, installed from the harness:** the block log sink (`kernel/src/log/block_sink.rs:384,444`). It is
    installed only by `kernel/src/test_harness.rs:1785,19303`.
  - **Test-only, direct harness writes** (cited at `origin/main` at relay #87's reviewed SHA):
    `kernel/src/test_harness.rs:19299, 19338, 27669, 27933, 28008, 28139, 28312, 31485, 33629, 34046`. The whole
    harness module exists only under `#[cfg(feature = "neo-test")]` (`kernel/src/main.rs:87`), and image A's
    feature list has no `neo-test` (`xtask:4972`). These are safe in Try because they aren't compiled, not
    because they go through the sink.
- **One driver-level exception:** `virtio_blk::debug_submit_abandoned_write` (`kernel/src/drivers/block/virtio_blk.rs:544`)
  submits below the `BlockDevice` trait. It is gated `all(neo-test, not(neo-test-bare-metal-mode))` and asks the
  guard directly.
- **The other filesystems** are read-only: FAT32 and ext4 (`kernel/src/fs/fat32.rs:78`, `ext4.rs:100`).
- **Firmware variables:** none. There is no EFI runtime-services request (`kernel/src/main.rs:116-152`).
- **Not audited, and outside "disk" in this claim:** CMOS/RTC, NIC and USB device state, and PCI config. None of
  them is a disk.

The R5 gate turns this audit into checks:
- a CI check that lists every `write_block` / `BlockOp::Write` caller and compares it with an allowlist: NeoFS,
  the sink, the drivers' own `submit`, and the two test-only groups above. Each test-only group is accepted only
  where its `#[cfg]` includes `neo-test`. A new caller, or a test-only caller losing its gate, fails CI until this
  list is updated;
- the scenario: Try with an installed disk attached leaves it bit-identical (BLAKE3 of the whole disk, before and
  after).

### 6.3 A defect in "Try writes nothing" today

`boot-neofs` mounts the **first** valid NeoFS volume it finds, read-write (`kernel/src/fs/mod.rs:774-811,867-901`).
The guard arms it, because a NEOFSv1 superblock claims its own volume. So on a machine that has an installed LogansOS
disk, or an OptiPlex-style model volume, booting the Try USB:
- mounts that disk read-write;
- records the intro flag and grants on it;
- and the user's installed data is changed by "just trying".

**Fix (rung 5, J.c.: read-only):** in Try mode, `boot-neofs` mounts a found LogansOS volume **read-only**. The Try
session can then say "LogansOS is installed on this machine" without changing it. On a machine with no NeoFS (the
Zenbook, a fresh PC) nothing changes.

### 6.4 Kill-switch

Feature `installer` (kernel + userland), and the image built without it:
- `limine.conf` is today's, byte for byte (one entry, `timeout: 0`);
- the kernel's `.text` is identical (no command-line request);
- there is no installer module.

The xtask KAT asserts the generated `limine.conf` for both settings.

## 7. Updates: fit with OT-51's A/B model (review item 7)

### 7.1 The conflict, and the choice

OT-51 defines a deployment as `{kernel ELF + verified userland root + required base config}`, stored as **whole
images in NeoFS**. It has two slots, two Limine entries, and activation by flipping `default_entry`
(`docs/THINK-2026-07-10-ot51-update-rollback-model.md:38-43,105,109,125`). Revision 1 put the slots on the ESP,
which changes OT-51's storage model. Limine reads FAT and ISO9660, not NeoFS, so a deployment in NeoFS can't be
booted directly. Something has to reconcile the two. There are two options:
- **(a)** The slots stay in NeoFS, per OT-51. Activation copies the chosen deployment's kernel and modules to the
  ESP.
- **(b)** An OT-51 amendment that makes the ESP the slot store.

**Chosen: (a).** Reasons:
- NeoFS stays the single authority. That is where per-file BLAKE3 content hashes already live, and where OT-51's
  verification and signatures (slice 2's verified base) belong.
- The ESP becomes a **derived boot copy** that can always be regenerated from NeoFS. That gives a repair path, the
  same one the installer's W3 uses.
- Option (b) would make a FAT partition the thing that is verified. It would also need verified-base logic over FAT,
  and an ESP writer able to create and replace files.

**What (a) adds to OT-51** (a clarification for ratification, not a change to its storage model):
1. **A boot copy per slot on the ESP:** `\slot\A\` and `\slot\B\`, each holding the kernel and the four
   userland module files that slot's Limine entry loads.
2. **A slot writer, not a FAT writer.** The slot files are preallocated, contiguous, maximum-size files created by
   the host-built template (§4.3). Activation writes only:
   - sectors inside a slot file's recorded extent;
   - that file's 32-byte directory entry `FileSize` field (one sector);
   - the `limine.conf` sector holding `default_entry`.

   The FAT and the cluster chains are never touched. The slot writer is a pure, KAT'd module against a FAT byte
   oracle.
3. **Activation that is atomic, durable and reversible.**
   - Refuse activation on a device without flush (§4.7, rule 1).
   - Verify the new deployment in NeoFS (OT-51's verification).
   - Copy it into the **inactive** slot's files, write their directory-entry `FileSize` fields, **flush**, then
     read back and verify. Everything the commit sector will point at is durable first.
   - Flip `default_entry` with **one single-sector write**, then **flush**. This is the commit point. Activation is
     reported only after this flush.
   - A flush error before the flip aborts activation with the old slot untouched. A flush error after the flip
     reports "may not have taken effect"; the next boot shows which slot Limine chose.
   - Power loss before the flip leaves the old slot booting. Power loss during it lands on old or new, never a
     torn mix, because a single sector is atomic. Power loss after it but before its flush may also land on
     either. Both are complete, because the slot data was flushed first.
   - The slot writer's KAT uses the same write-back-cache model as §4.7.
   - Rollback is the Limine menu (both entries are always present) or `neo> rollback`, which is the same
     single-sector flip.

The installer writes slot A only. Slot B's files exist at their maximum size, are empty, and are listed in
`limine.conf` only once OT-51 slice 1 ships. The model, the vocab and user data are in no slot, so an update never
copies 485 MB.

### 7.2 ESP capacity budget (measured)

Measured on `origin/main` `0b39255b`. Image A's kernel was built with `PRODUCT_IMAGE_KERNEL_FEATURES`
(`xtask:4972`), in the dev profile xtask ships (`kernel_path`, `xtask:16218`: `target/x86_64-neoplutarch/debug/kernel`). The userland
came from `cargo xtask build` (default features: image A's `desktop-with-model` userland adds the intro and clock,
and R4's KAT re-measures the exact image A set).

| File | Bytes |
|---|---|
| kernel (image A features, unstripped, as shipped today) | 39,589,264 |
| `neo-init` | 23,221,552 |
| `neo-worker` | 23,206,992 |
| `neo-shell` | 23,322,560 |
| `neo-arthur` | 24,403,664 |
| **one deployment** | **133,744,032 (127.5 MiB)** |
| `.vocb` (if it were a module; with §5 it is on NeoFS) | 5,010,512 |

- **Two deployments:** 255.1 MiB, or 264.6 MiB if the vocab were a boot module.
- **Fixed overhead:** Limine's `BOOTX64.EFI` and `limine.conf`, under 1 MiB.
- **A 512 MiB ESP leaves about 256 MiB**, so each deployment can roughly double before the ESP must grow.
- **The model can't be a module on an installed disk:** two deployments plus two 485,489,152-byte models are about
  1.16 GiB. That is why §5 (model from NeoFS) comes before R4.
- **Stripping debug info** would shrink the kernel to 7,421,696 bytes, measured. It is a separate decision, out of
  scope here.

**Slot sizes (R4.2, PM-accepted).** Each slot preallocates one file per deployment file: `kernel` 72 MiB and
`neo-init`, `neo-worker`, `neo-shell` and `neo-arthur` 44 MiB each, 248 MiB per slot and 496 MiB for A and B
(`tools/xtask/src/esp_template.rs`, `SLOT_FILES`). An image whose kernel or module is larger than its slot file
fails to build (`install_bundle::check_slot_sizes`, R4.3); it is never truncated. The installer's preflight
refuses the same case again on the running system (`Refused::SlotTooSmall`).

### 7.3 What stays open for OT-51

- Signature and verified-base logic (slice 2) runs over the NeoFS deployment, not the ESP.
- Config history stays OT-50/OT-51 §5's.

## 8. The rung ladder

Each rung lists:
- its dependencies;
- its iteration cost: cycles, kernel/QEMU gates, bare-metal runs;
- its close gate;
- its kill-switch.

**Direct dependencies:**
- R1, R2 and R3 are independent and can run in parallel.
- R4 depends on R2 and R3.
- R4b depends on R4.
- R5 depends on R4, and so transitively on R2 and R3.
- R6 depends on R5, and on R4b if the OptiPlex disk isn't blank.
- R7 depends on R4 and on PL ratification of OT-51, with §7.1's clarification.

| Rung | What | Depends on | Cost (cycles / gates) | Close gate | Kill-switch |
|---|---|---|---|---|---|
| **R0** | This THINK + a cross-family read | — | 1 doc, 1 review | PM merge after the review | — |
| **R1** | `xtask vm-image` (boot.raw = image A, data.raw = NeoFS). Scenario `vm_installed_two_boot`: boot from disk under OVMF, no `-cdrom`; first boot writes the intro flag and a memory file; second boot reads them | A pinned OVMF baked into the dev container (Dockerfile; version and checksum recorded) | Few cycles. Host-only plus one new QEMU scenario (no kernel code) | 4-shard gate green with the new scenario; xtask KATs for the image layout | Not a policy rung (a tool and a scenario); the product image is unchanged |
| **R2** | Model from NeoFS: the kernel pin check on `/neofs` models, preload from NeoFS, Arthur's source order, the installed Absent line. R1's `data.raw` gains the model; a variant boot disk without model modules proves the disk source | — | Moderate. Kernel + userland; an IF=1 preload path (CP-74's class) | 4-shard gate; scenarios: model from NeoFS loads and pins; a wrong-byte model on NeoFS ⇒ Damaged; with the boot module present, it wins. Cross-family review (kernel AI path) | `model-from-neofs` |
| **R3** | Partition view + the guard's partition rule + the shared pure GPT module; `boot-neofs` probes LogansOS partitions. xtask builds a one-disk GPT image (ESP + NeoFS partition) for the VM | — | High. A safety-invariant change: relax-and-re-prove, many guard KATs, a hand-laid GPT byte oracle | 4-shard gate; every guard KAT green including `windows_gpt_disk_never_arms`; new KATs (§4.2); a one-disk VM scenario; the write guard gets ledger row **I-17**, with a consolidation pass in the same change so the ledger stays on one page; a **cross-family security review** (Muse, per CLAUDE.md, for the guard change) | `neofs-partition` (off ⇒ guard and mount bit-for-bit today) |
| **R4** | The installer for `fresh` targets (blank or incomplete), with flush barriers; the host ESP-template builder in xtask uses vendored `fatfs` under `third_party/` (§11.2; never in the kernel):<br>- the `NEOINST1` bundle and its pin<br>- the registry generation, driver identity and install claim (§2a)<br>- the GPT writer (§4.4)<br>- the ESP template and slot writer<br>- the NeoFS seed and grow<br>- the state machine (§4.7)<br>- the minimum size<br>- the UI (§4.5)<br>VM only | R2, R3 | High. New capability, new write syscall, three byte oracles (GPT, `NEOINST1`, slot map/FAT). Several cycles of scenario iteration | 4-shard gate. Scenarios:<br>- install to a blank virtio disk at `S=512` **and** `S=4096`, then that disk boots alone and the model loads from it<br>- `install-stop-after=Wn` for every step, then Finish installing<br>- zero reads from the boot medium during install<br>- 1 GiB and one-sector-short disks refused with zero writes<br>- foreign GPT or filesystem refused without R4b<br>- the §2a identity table<br>- a no-flush device refused<br>- a completed LogansOS disk refused in R4 (reinstall is R4b)<br>KATs: the §4.4 oracle, the write-back-cache state machine. Security review (Muse) | `installer` |
| **R4b** | Destructive installs: "Erase disk and install" over a foreign table, **and "Reinstall" over a completed LogansOS disk**. Both need the typed, identity-bound confirmation and stable identity. Also: the boot-disk refusal and the protected-disk table (empty until a VMD lane) | R4 | Moderate. A guard bypass path, so it is review-heavy | 4-shard gate; scenarios: erase-and-install over a disk with a foreign GPT, and reinstall over a completed LogansOS disk, succeed only after the typed confirmation. A wrong confirmation, a changed identity, an unidentified disk (incl. a completed LogansOS with an `.arthur` file), the boot disk, or a protected disk is refused, and the disk stays bit-identical (hash before/after). Muse security review | `installer-erase` (off ⇒ R4's blank-or-incomplete rule) |
| **R5** | Try/Install:<br>- the two Limine entries (Install loads the bundle and skips the preload)<br>- the kernel command line<br>- neo-init's install mode<br>- the desktop item ("Restart into the installer")<br>- Try mounting found volumes read-only<br>- the §6.2 audit grep | R4 | Moderate | 4-shard gate. xtask KAT on both `limine.conf`s. Scenario: Try with an installed disk attached leaves it bit-identical (whole-disk BLAKE3 before and after) and says "LogansOS is installed on this machine" | `installer` (the same switch) |
| **R6** | Bare metal: install onto the **OptiPlex** NVMe from the Try USB, boot it, Arthur answers from the disk | R5 | Blind hardware: expect several runs | The bank below; the acceptance milestone in the ledger (a merge was already gated on R4/R5) | — |
| **R7** | OT-51 slice 1 on this layout: slot B, the second entry, the slot writer's activation flip (§7.1) | R4, PL ratification of OT-51 with §7.1's clarification | OT-51's | OT-51's | OT-51's |

**R6's failure-mode and discriminator bank** (OptiPlex; serial log + screen, per its runbook):

| Expected failure | Discriminator |
|---|---|
| Firmware doesn't boot the installed disk (no NVRAM entry, removable-media path ignored) | Serial stops before Limine. `efibootmgr -v` from Ubuntu shows no entry for the disk. Fallback: add one by hand with `efibootmgr -c`. Rung "UEFI boot entry" moves up |
| Limine starts but can't find `\slot\A\kernel` | Limine's own error on serial (`serial: yes`). Check the installed `limine.conf` and the slot file's size from Ubuntu |
| The installer refuses the OptiPlex NVMe | The installer's verdict line names the guard's reason. Expected under R4 while the burned ISO's GPT is on it (R4 writes only blank or LogansOS disks). Under R4b, the erase confirmation names it "unknown partition table" (the xorriso GPT) and then installs |
| Write or verify fails mid-install | `install: step=Wn write lba=… err=…` / `verify mismatch at lba=…`. The disk is left in a §4.7 state: blank, or "LogansOS (installation incomplete)". The next USB boot offers Finish installing |
| GPT wrong on the real disk's geometry | The installer prints `S`, `L`, both headers' LBAs and CRC-E, CRC-H1 and CRC-H2. `sgdisk -v` from Ubuntu cross-checks. The R4 oracle covers both sector sizes |
| NeoFS grow wrong | The `boot-neofs` mount line (`total_blocks`, free count); `tools/neofs-inspect.py` on the partition (after an offset option is added) |
| Model copied but pin fails | `ARTHUR model source=neofs state=damaged` plus the kernel's hash line |
| Booting the installed disk mounts the wrong volume (the USB's, or the old `sda` model volume) | The `boot-neofs` line names the device and partition. The partition probe runs before whole devices (§4.2, item 1) |
| Try session, with the installed disk attached, changes it | R5's hash-before/after; on the OptiPlex, `sha256sum` of the partition from Ubuntu before and after a Try boot |

## 9. Decisions (PM, 2026-10-27) and the questions still open

| # | Question | Answer |
|---|---|---|
| 1 | OVMF on the CI runners | **Yes.** A pinned OVMF is baked into the dev container (Dockerfile), not installed as a host package, so the same image is used everywhere. The version and checksum are recorded beside the pin. R1 carries this |
| 2 | Grow NeoFS or a per-size image | **Fixed images for R1–R3** (the VM and the OptiPlex). NeoFS grow arrives in R4 |
| 3 | Erasing a non-blank disk | **In scope (J.c.)**, like Ubuntu's "Erase disk and install". It is **R4b**: a typed, identity-bound confirmation; the one-shot install claim as the only path past the guard to a foreign table; a Muse review. The Zenbook layers are re-stated in §2 |
| 4 | A named UEFI boot entry | **The removable-media path only, for R4–R6.** A named entry comes later, and only if a target needs it |
| 5 | Try with an installed disk attached: read-only or skip | **Read-only (J.c.).** Try can say "LogansOS is installed on this machine" (§6.3) |
| 6 | The write guard in the ledger | **Yes: an I-17 row when R3 lands.** The same change runs a consolidation pass to keep the ledger on one page, rather than dropping a row |

## 10. What this THINK doesn't do

- **It decides no product UI** beyond where the choice appears. The installer app's screens are the Atrium lane's.
- **It doesn't change OT-51's storage model.** Deployments stay in NeoFS. It proposes one clarification for
  ratification: the ESP boot copy, the slot writer, and single-sector activation (§7.1).
- **It doesn't make the Zenbook a target**, now or later. Installing onto the Zenbook would need Intel VMD support
  and a deliberate PL decision.

## 11. Prior-art survey (CLAUDE.md, "Unfamiliar features", 2026-10-27)

An installer is new to this codebase, so this section surveys what exists before anything is written. Every
row's license and last commit were read from a `git clone --depth 1` of the project's default branch on
2026-10-27. GPL, LGPL and AGPL projects are read for the principle only. Nothing is vendored by this THINK: a
build that vendors a crate does it under `third_party/<name>/` with the upstream license file, URL and commit.

### 11.1 Installers and their disk logic

| Project | Approach | License | Last commit (default branch) | What we'd take |
|---|---|---|---|---|
| **Calamares** (`calamares/calamares`) | A distro-neutral Qt installer driven by a module sequence (partition, unpack, bootloader…). Partitioning goes through KPMcore. "Erase disk", "Replace a partition" and "Manual" are separate choices, and erase is confirmed with a summary page | GPL-3.0-or-later (SPDX in each file, e.g. `src/modules/partition/PartitionViewStep.cpp`) | `95aa33fd` 2025-05-06 (branch `calamares`; latest tag v3.3.9). Alive but slowing, inside the two-year line | **Design only.** The separate erase / alongside / manual choices and the summary-before-write page match §4.5. We keep only erase and install; alongside and manual stay out of scope |
| **Subiquity** (`canonical/subiquity`) | Ubuntu Server's installer. The UI collects choices into an "autoinstall" config, then hands the actual disk work to curtin | `subiquity/`: AGPL-3.0-or-later; the rest GPL-3.0-only (`LICENSE`) | `5aad030a` 2026-10-23. Active | **Design only.** The split between UI and a separate disk executor matches §4.5 / §4.7: the UI produces an install claim, and one writer executes it. "Try or Install" at boot is the same Ubuntu shape J.c. asked for |
| **curtin** (`canonical/curtin`; upstream Launchpad, which this container can't reach; GitHub mirror read) | Executes a declarative storage config (disk → partition → format → mount, by id), staged in a fixed order: partitioning, then extract, then curthooks, then the bootloader | AGPL-3.0-only (`LICENSE`) | `93c36fb6` 2026-10-09 (mirror). Active | **Design only.** Our ordered W-steps with a durable marker play the same role as its fixed stage order. Its wipe-before-partition step (`wipe: superblock`) is the precedent for clearing stale filesystem signatures before reuse. Ours happens in W4, when the NeoFS seed overwrites partition 2's superblock copies, and W3 replaces the ESP whole from the template |
| **systemd-repart** (`systemd/systemd`, `src/repart/repart.c`) | Declarative and idempotent. `repart.d` files describe the wanted partitions. It only adds or grows partitions, never shrinks or moves existing ones, and type and partition UUIDs are derived from a machine-id hash so a re-run is repeatable. `--empty=require/force` decides whether it may touch a non-empty disk | LGPL-2.1-or-later (file SPDX) | `a09e5f55` 2026-10-26. Active | **Design only.** (1) Idempotence: running again yields the same table, the same as our Finish installing re-running W3–W6. (2) The explicit `--empty=` policy is the same idea as our `fresh` vs `destructive` claim kinds (§2a item 4). (3) Deterministic partition GUIDs: worth considering for reproducible images, but the installer keeps random disk and partition GUIDs, so two installs never collide |
| **Redox installer** (`redox-os/installer`; upstream GitLab, which this container can't reach; GitHub mirror read) | Whole-disk only. A GPT with a BIOS-boot partition in the first MiB, an EFI partition (`fatfs`), and RedoxFS in the rest, built with the `gpt` crate. A TUI picks the disk. There's no identity binding and no foreign-disk confirmation beyond picking from a list | MIT (`LICENSE`) | `b5a11310` 2026-09-01 (mirror). Active | **Design, plus a limit.** It is the closest shape to ours (whole disk, Rust, `gpt` + `fatfs`, a custom FS). Limit: it refuses any logical sector size but 512 ("block size … not supported", `src/installer.rs:577-583`), and its layout arithmetic is written for 512 (`gpt_reserved = 34 * 512`, `:586`). A 4Kn NVMe can't take a Redox install. Our §4.4 computes the layout per S, with an S=4096 oracle, because 4Kn NVMe exists in the field |
| **Haiku Installer** (`haiku/haiku`, `src/apps/installer`) | Copies the running system onto a partition chosen in DriveSetup, then makes it bootable. Like our §4.3 "the running system's own bytes", the source is the live system | MIT for the Haiku-written code (`License.md`; per-file headers rule) | `a12d4cd1` 2026-10-26. Active | **Design only.** It copies the running system rather than a separate payload, the same choice as §4.3. No code: it is C++ against Haiku's Storage Kit |
| **Limine** (`limine-bootloader/limine`) | Our bootloader. Its install step only puts the EFI binary on the ESP (`BOOTX64.EFI`) and, for BIOS, `limine bios-install` | BSD-2-Clause (`COPYING`) | `8ad14430` 2026-10-19 (branch `v12.x`). Active | **Nothing new to vendor.** The ESP layout and `limine.conf` follow its docs, as R1 already does |

### 11.2 Rust GPT and FAT crates

| Crate | What it does | Fit | License | Last commit | What we'd take |
|---|---|---|---|---|---|
| **`gpt`** (`Quyzi/gpt`, v4.1.0) | Read and write GPT headers and arrays, a protective MBR, and type GUIDs, over `Read + Seek + Write` | `std`-only (`std::io`); deps `crc`, `uuid` (v4, which needs an RNG), `bitflags`, `simple-bytes` | MIT | `73086def` 2026-08-19 | **Not vendored.** The kernel would need a `no_std` port, and the GPT writer here is about 200 lines once the byte spec (§4.4) is fixed. Use: its tests as a **second, independent reader** in the host KAT. A GPT our writer produces must parse in `gpt` with the same partitions. That is an independent check, alongside the hand-laid byte oracle |
| **`gptman`** (`rust-disk-partition-management/gptman`, v3.1.1) | The same scope as `gpt`, with a CLI; aligns partitions and handles 4Kn | `std`-only (`std::io`); default feature `nix` (Linux ioctl) | MIT OR Apache-2.0 | `4fc144e1` 2026-04-28 | **Not vendored**, for the same reason. It's a second host-side cross-check reader if `gpt` disagrees with our oracle on 4Kn |
| **`fatfs`** (`rafalh/rust-fatfs`) | A FAT12/16/32 filesystem: read, write, and `format_volume` (`src/fs.rs:1200`) | `no_std` when `std` is off (`src/lib.rs:47`), with `alloc` optional; deps `bitflags`, `log` | MIT | `38b3a795` 2026-10-16 on `master` (0.4 unreleased; the last tag is v0.3.6) | **The one borrow, in the host ESP-template builder only.** §4.3's two ESP templates are built on the host. `fatfs` (with its `std` features, inside xtask) replaces today's `mformat` call (`xtask:11834`): it formats the image, creates the preallocated slot files, and the builder reads the image back to verify each chain is contiguous and to generate the slot map. That is exactly the code that shouldn't be rewritten: a correct FAT32 formatter (FSInfo, reserved sectors, cluster sizing). **The kernel never formats FAT.** W3 copies the template whole, W4 is NeoFS, and activation keeps the narrow slot writer (§7.1), whose durability argument depends on knowing exactly which sectors are written and in what order. The bundle's BLAKE3 pin covers the templates, so a `fatfs` change shows as a pin change. Whether to pin `master` or v0.3.6 is decided when xtask adopts it (R4) |

### 11.3 How other hobby OSes install

- **Redox** (above) is the only one with a maintained, whole-disk Rust installer. It is the closest prior art, and
  its 512-only sector support is the limit §4.4's per-S layout avoids.
- **SerenityOS** and most other hobby kernels (OSDev-style) have no installer: images are written with `dd` or run
  in a VM. That is the state LogansOS is in today (§1). Nothing to borrow.
- **Haiku** (above) copies the running system, the same as our §4.3.

### 11.4 Outcome

- §4–§7's design doesn't change; §4.3 now names the template's formatter. The survey confirms the shapes: whole-disk erase-and-install (Redox), the running
  system as the payload (Haiku), a UI separated from an ordered, idempotent executor (Subiquity + curtin,
  systemd-repart), and the explicit fresh-vs-destructive policy (systemd-repart's `--empty=`).
- **One borrow:** vendor `fatfs` (MIT) under `third_party/fatfs/` for the **host** ESP-template builder in xtask,
  replacing `mformat`, in R4. The kernel never links it.
- **One test addition:** `gpt` (MIT) as an independent second reader in the §4.4 host KAT, as a host
  dev-dependency only, never in the kernel.
- **One limit recorded:** Redox installs only to 512-byte-sector disks. Our S=4096 oracle keeps 4Kn in scope.

### 11.5 Deferred: encrypted installs and Secure Boot

The surveyed installers cover two things this THINK deliberately doesn't. They are recorded here so the shapes
above aren't read as covering them.
- **Encrypted installs and recovery credentials.** Subiquity, Calamares and Windows Setup offer full-disk encryption
  (LUKS, BitLocker), with an unlock secret at boot and a recovery key. **Deferred.** It needs a passphrase entry
  before NeoFS mounts, a key-derivation and cipher layer under NeoFS, and a recovery-key flow. Each of those is
  its own design with its own byte oracle and security review. Nothing in R0–R7 precludes adding it later: the
  partition layout leaves NeoFS as one partition that a later layer can wrap.
- **Secure Boot.** Distros boot under Secure Boot through a Microsoft-signed shim. **Deferred.** LogansOS has no
  signed shim, and Limine isn't signed for the Microsoft UEFI CA, so an installed LogansOS boots only where Secure
  Boot is off or the user enrolls their own key. Signing, a shim, and enrollment are a separate lane.
- **The Zenbook's "Secure Boot off" is a test prerequisite, not a support statement.** The Zenbook runbook
  (`docs/runbook-baremetal-zenbook-s14.md:22`) turns Secure Boot off so the USB image can boot there for testing.
  It doesn't mean LogansOS supports general PCs with Secure Boot on. Until the deferred lane lands, the
  installer's documentation says the target PC needs Secure Boot off.
