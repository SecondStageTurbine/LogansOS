<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-87 installer THINK cross-family review

**Relay:** #86 · **Reviewer:** Codex-family (GPT-6-Luna, high effort) · **Reviewed:** `f1cec9c2d305be67a54b0d87c6ec30df7719cf58` (`docs/THINK-2026-10-27-ot87-installer.md`)

**Overall verdict: FAIL — revise before rung 1.** The THINK has a sound R1 direction and correctly identifies today's Try-session write defect, but its A/B layout conflicts with OT-51, its disk-change and GPT-resize guarantees are underspecified, and the installer has no defined interrupted-install recovery path. The change to the guard is an invariant expansion and should remain gated on the R3 byte-oracle and Muse review described in the THINK.

## 1. Claims about today's code — PASS, with one qualification

The spot-checks support the cited current-state claims:

- Hybrid ISO build uses xorriso's UEFI EFI boot partition and protective MBR (`THINK:70`, `tools/xtask/src/main.rs:6675`, `:6692`, `:6710`). It is therefore reasonable to use the image as an OVMF boot disk, though R1 must demonstrate that exact disk path.
- `boot-neofs` scans block devices in index order, selects the first valid NeoFS, and mounts it read-write (`THINK:87`, `kernel/src/fs/mod.rs:787`, `:806`, `:840`, `:843`).
- NeoFS stores byte offsets in 4 KiB filesystem blocks and uses device-relative offsets (`THINK:85`, `kernel/src/fs/neofs/format.rs:336`, `kernel/src/fs/neofs/inode.rs:1802`). There is no partition offset/view in these paths, supporting the whole-device-only claim.
- The write guard claims NeoFS from byte zero and drops claims overlapping GPT allocations (`THINK:86`, `kernel/src/drivers/block/write_guard_logic.rs:595`, `:599`, `:656`).
- The VMD path explicitly says the hidden NVMe is not probed or registered (`THINK:107`, `kernel/src/drivers/pci.rs:259`, `:269`).
- The product model pin is `QWEN25_05B` with the stated digest (`THINK:72`, `kernel/src/boot_model_logic.rs:32`, `:34`).
- The THINK accurately calls out GPT parsing as read-only today (`THINK:95`; the guard parser reads/validates headers and entries at `kernel/src/drivers/block/write_guard_logic.rs:286`, `:450`).

Qualification: the new partition rule can preserve the Windows-GPT KAT only if it recognizes the exact new partition type GUID and validates the partition-relative NeoFS extent against that partition's bounds. The current whole-disk NeoFS rule cannot itself establish that property.

## 2. Zenbook stays unwritten — FAIL as a complete guarantee

The four stated layers are useful, but layer 4 does not close the changed-device case. The current `BlockDeviceIdentity` contains only driver, static name, and optional PCI BDF (`kernel/src/drivers/block/mod.rs:62`); serial/model support is new work. Even if added, model + serial + capacity can be missing or duplicated, particularly for virtual disks, and the same tuple can describe a replacement disk. Binding an index plus that tuple is not proof that the device object being written is the one selected.

Specify a kernel-held reference/generation for the enumerated device, revalidate it immediately before each write, and fail closed where the driver cannot supply a stable identity. Include tests for reorder, removal/reinsert, identity unavailable, and a replacement with the same reported tuple. The Windows-GPT KAT remains necessary and is not a substitute for install-claim tests.

## 3. §6.3 defect — PASS

Confirmed. The first valid device is selected (`kernel/src/fs/mod.rs:787-808`) and mounted read-write (`:840-844`). A Try boot with an installed NeoFS volume earlier in device order can therefore persist intro/grant/data writes onto that volume. The proposed read-only/skip fix addresses this path; the R5 hash-before/after scenario is appropriate.

Narrow the claim until the full write surface is audited: read-only `boot-neofs` prevents writes through that mount, but “Try writes nothing to any disk” also needs to cover every other disk-writing path enabled in the Try image, including logging or diagnostics.

## 4. R1 needs no kernel change — PASS, as a proposed rung

The described two-disk setup is consistent with current code: the ISO boot disk has no valid NeoFS volume, the second virtio disk can be the first valid NeoFS, and the existing mount can write it. Nothing in the guard's GPT overlap rule blocks a separate whole-disk NeoFS data disk. The xorriso output exposes an EFI system partition (`tools/xtask/src/main.rs:6692-6712`).

This is a sound host-tool/QEMU rung and requires no kernel change. Keep the boot-neofs ordering assumption explicit in the scenario and prove persistence over two OVMF boots. The code review does not establish that the proposed OVMF runner or exact raw-ISO-as-virtio-disk invocation already works.

## 5. Image installer and GPT resize — FAIL pending a byte-level specification

The image-based approach is a reasonable way to avoid adding a kernel FAT writer, but the description does not yet prove correct resize behavior for either sector size (`THINK:226`). State the logical sector size used to build `install.img` and how the image is installed when target sectors are 512 bytes versus 4096 bytes. GPT LBAs are in logical sectors; copying one sector geometry's GPT block-for-block onto the other is invalid. The existing guard itself derives GPT LBAs and byte offsets from the device sector size (`kernel/src/drivers/block/write_guard_logic.rs:294`, `:331`, `:457`).

Specify the exact edit algorithm and byte oracle: relocate the backup entry array and header to the target's last LBA; update primary and backup header current/alternate LBAs, usable bounds, partition entry, entry-array CRC, and both header CRCs; preserve the protective MBR semantics; and prove the NeoFS partition ends before the relocated backup structures. “Both CRCs” is ambiguous because there are header CRCs and an entry-array CRC.

The install claim also does not close changed-device identity as written; see item 2. Add explicit tests for a device replacement between enumeration, confirmation, and write, as well as duplicate/unavailable identity. Until these are specified, do not claim wrong-disk and changed-disk cases are closed.

## 6. Rung ladder and kill-switches — PASS with a dependency correction

R1/R2 can proceed in parallel; R3 has no R2 dependency; R4 follows R2 and R3. R5 is correctly downstream of R4. The policy switches for model-from-NeoFS, partition support, and installer/Try behavior are stated (`THINK:277`, `:203`, `:317`), and the R3 switch explicitly returns guard/mount behavior to today's path.

Correct the dependency text: the prose says “rungs 4–5 depend on 2 and 3” (`THINK:63`), while the table gives R5 only R4 (`:363`). Either is transitively equivalent, but write the direct dependency clearly. R6 is a hardware acceptance rung rather than a policy rung, so no feature kill-switch is needed there. Preserve the Muse review before enabling the guard relaxation.

## 7. A/B fit with OT-51 — FAIL

The claimed fit is contradicted by OT-51. This THINK puts deployment A/B files on the ESP (`THINK:171`, `:332`), while OT-51 defines deployments as whole images stored in NeoFS (`docs/THINK-2026-07-10-ot51-update-rollback-model.md:38`, `:43`) and says slice 1 needs two files/regions (`:105`, `:125`). Therefore the current layout does not simply reserve space for OT-51 slice 1; it changes the deployment storage design and adds an ESP-writing/update mechanism that OT-51 did not specify.

Either keep deployment slots in NeoFS in line with OT-51, or explicitly propose an OT-51 amendment and reconcile its verification, atomic activation, rollback, and ESP-write requirements before saying the layout matches. A 512 MiB ESP also needs a measured capacity budget for both complete deployments, not only a rough estimate for one kernel and four modules.

## 8. Missing installer requirements — FAIL

Add these before rung 4 is dispatched:

- A recoverable install state machine. If power, the USB, or the target disappears after GPT/image writes begin, the disk may be neither blank nor a valid LogansOS install. The failure bank currently says retry is allowed only for “LogansOS” or “blank” (`THINK:374`), leaving a partially written target potentially unrecoverable by the installer. Define resume/repair behavior and verify each transition before proceeding.
- A concrete source path for `install.img`. The USB is said to carry it as a file, compressed image, or sparse extent list (`THINK:211`), but the design does not identify how the running kernel/installer opens and streams that file from the Try medium, nor how integrity is pinned before writing.
- UI and input fallback for selecting a disk with keyboard and touchpad, including inaccessible/unknown identity and confirmation text; the prompt itself names disk picking but the design only says an Atrium app/console fallback (`THINK:216`).
- Minimum-target-size behavior before any write. Reject disks smaller than the image plus GPT/NeoFS growth requirements, and include the smaller-than-image case in the R4 scenario bank.
- A defined policy for removal of the source USB during install and a test that verifies the target is left in the documented recoverable state.

The existing R6 failures cover useful bare-metal discriminators, but they do not replace these R4 safety and recovery cases.

**Disposition:** return to S3 for revision; no rung 1 dispatch until the design resolves the A/B storage conflict and adds the missing install and identity/GPT guarantees.
