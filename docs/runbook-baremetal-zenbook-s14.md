<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook — bare-metal runs on the ASUS Zenbook S14 (UX5406AA)

**Status:** Operational runbook (PL/PM-maintained). **Created:** 2026-10-21 (PL decision: this is the
bare-metal test target from now on, replacing the OptiPlex 7080).

This is the access, boot and evidence procedure for every bare-metal run on the Zenbook. A bare-metal
dispatch on this target references it, the same way OptiPlex dispatches reference
`docs/runbook-baremetal-arthur-optiplex.md`. The target-specific risks and their discriminators are in
`Plans/NOTE-2026-10-21-zenbook-target-s2.md`.

## The target

| | |
|---|---|
| Machine | ASUS Zenbook S14 UX5406AA |
| CPU | Intel Core Ultra 9 386H (Panther Lake, 2026); hybrid P / E / LP-E cores |
| Memory | 32 GB LPDDR5X-8533 |
| Graphics | Intel Xe3 (GOP framebuffer only; we do not drive the GPU) |
| Display | High-resolution panel; **10-point touchscreen (unsupported)** |
| Internal disk | 954 GB NVMe, **Windows 11 Home: the PL's work laptop** |
| Firmware | Secure Boot **off**, BitLocker **off** |
| Serial | **None.** There is no COM port. |
| Boot medium | The **Kingston DataTraveler USB stick** (it overwrote the old Ubuntu control stick) |

## HARD RULE: nothing is ever written to the internal disk

The Windows NVMe is the PL's work machine. It is never written: not by a run, not by provisioning, not by
a log sink. This is enforced twice.

1. **In the kernel.** Every block write passes the write guard
   (`kernel/src/drivers/block/write_guard_logic.rs`). A write reaches a disk only inside an extent that a
   **sealed, structurally valid** marker read from that disk claims:
   - a NEOFSv1 superblock (BLAKE3 checksum and format fields valid) claims its own volume;
   - a sealed v3 log-sink header at 256 MiB claims only its 16 MiB region;
   - a sealed scratch sector naming the disk's exact capacity (xtask's QEMU disks) claims the disk.

   On a disk with a partition table, a claim counts only if it lies in space the table leaves
   unallocated: any claim overlapping LBA 0, the table's own sectors or a partition is dropped. On the
   Windows NVMe's GPT every probe offset (256 MiB included) lies inside a Windows partition, so
   **nothing on it can arm, even a file holding a valid-looking marker at a probe offset** (host KAT
   `windows_gpt_disk_never_arms`). Its first write attempt logs `block: WRITE REFUSED device=N ...
   table=gpt ... this disk is never written`, and no write reaches the driver. The GPT is believed only
   when a copy of it validates: header and entry-array CRC32s, and structural bounds. The primary is tried
   first, then the backup in the last LBA. A GPT with no valid copy protects the whole disk
   (`table=opaque-gpt-invalid`). Without a partition table the disk is still protected if it holds a boot
   signature or a recognisable whole-disk volume (NTFS, FAT, exFAT, BitLocker, LUKS, LVM, ext, ISO 9660:
   `table=opaque-filesystem` / `opaque-boot-record`), or a malformed MBR (`opaque-mbr-invalid`); a hybrid
   MBR's partitions count beside the GPT's. The bare-metal log sink
   also refuses a disk without its sealed header (`block-log-sink: REFUSED`) and the run continues
   without it. If Intel VMD/RST is on, the NVMe is not even visible to us (`pci: Intel RAID/VMD
   controller ... no internal disk will be probed`).
2. **In procedure.**
   - Never provision a log-sink header, a NEOFS volume or anything else onto the internal disk.
   - Never run a disk-writing tool against it from any OS.
   - The only medium we write is the Kingston stick, from the PL's PC.

Which images may boot here:
- **Image A**, the product ISO (`cargo xtask iso --with-model <neom>`): **allowed now.** It has no log
  sink, and its only writes are to NEOFS volumes it finds; none exists on this machine.
- **Bare-metal bundles** (`cargo xtask neo-test --bare-metal-mode --build-only <scenario>`): allowed only
  once the write guard is on `main`, i.e. the `claude/zenbook-target` PR is merged. Before that, a bundle's
  log sink would write the first NVMe it finds.

## Evidence: photos of the screen (allowed on this target)

There is no serial port, so **the screen is the log**. Both images build with `fb-progress-console`:
- every kernel log line is drawn on screen from early boot, at 2x glyphs on this panel, about 180 x 50
  characters;
- the text wraps to the top instead of scrolling, so **the newest line is the one directly above the
  blank row**;
- the mirror stops once anything else draws (the desktop compositor, a splash, a panic screen).

What to photograph:
1. **The screen when the run stops**, whether it passes, fails or hangs. Take a sharp, straight-on photo
   of the whole panel. If the text is small in the photo, take a second, closer photo of the area around
   the blank row.
2. **Any panic or fault screen.**
3. For Image A, **the desktop** once it is up (the compositor owns the screen from then on; the boot log
   above was mirrored until that moment).

Bundles also print their `NEO_TEST/1 event=PASS|FAIL ...` verdict through the same path, so it appears
on screen. With no sink on this machine, a photo of that line is the verdict.

## Preparing the stick (on the PL's PC)

1. Build the image on the build box: `cargo xtask iso --with-model <neom>` produces
   `boot/neoplutarch.iso`. For a bundle, use the bundle command above; it names its ISO.
2. Write it to the **Kingston stick** as a raw image. The ISO is a Limine hybrid image, so a raw write
   boots on UEFI. Use Rufus in **DD image mode**, or `dd if=neoplutarch.iso of=/dev/sdX bs=4M
   conv=fsync` on Linux. **Double-check the target is the Kingston stick**, never the laptop's disk:
   compare the size, about 30 GB, against 954 GB.

## Booting the run (the PL, at the laptop)

1. Power the laptop **off** (a full shutdown; with Windows Fast Startup a "shut down" is hibernation, so
   hold Shift while choosing Shut down, or use Restart).
2. Insert the Kingston stick, and **a USB keyboard and mouse if the run needs input** (see *USB keyboard
   and mouse* below). The internal keyboard, the touchpad and the touchscreen are not supported
   (NOTE §4b).
3. Power on and press **Esc** repeatedly at the ASUS logo to get the boot device menu; pick the Kingston
   (UEFI) entry. Alternatively press **F2** for setup and use *Boot Override*.
4. Let the run go to completion. For Image A that is the desktop; for a bundle it is its PASS/FAIL line.
   Do not interrupt it early.
5. Photograph the screen as above.
6. Power off, **remove the stick**, and power on: the laptop boots Windows as before. Nothing on its disk
   has changed.

## USB keyboard and mouse (image A)

The internal keyboard (i8042, PR #98) and touchpad (I2C-HID, the section after this one) are not USB.
Until both are proven on this machine, input comes from a **USB keyboard and mouse**. On Intel client parts since Tiger Lake the USB-C ports' USB 2 pins and the USB-A ports are on
the south xHCI (PCI device `0x14`), not on the Type-C subsystem's (`0x0d`); the next run's summary
confirms it for this board. The kernel now chooses the controller a USB 2 device is on
(`Plans/NOTE-2026-10-22-zenbook-usb-input.md`).

- Plug the keyboard and mouse in **before power-on** where possible; the kernel also picks up devices
  plugged in later (it re-reads the ports every 250 ms), including at the desktop.
- If a run with the USB-C port and adapter shows no input, **repeat it with the USB-A port**. That
  separates "the controller or the stack" from "the USB-C path".
- Two devices on one adapter usually means a hub (or a keyboard with a pass-through port). If input fails,
  also try **one device plugged in directly**.

**The USB summary.** Just before the desktop starts, image A prints a block of `usb-summary:` lines: every
xHCI found (its PCI address, which ports had a device, which controller was chosen and why), the firmware
handoff, the connected ports, every enumerated device with the driver that claimed it, and
`keyboard=… mouse=…`. If no keyboard or mouse has bound after 5 s, it **holds the screen for up to 30 s**:
- **Photograph the whole `usb-summary` block** (it is the newest text; the blank row is below it). Read it
  against the failure table in the NOTE (§3).
- Plugging a device in during the hold ends it early and prints the summary again; photograph that too.
- If a keyboard or mouse is bound, the screen shows **`INPUT CHECK - press a key and move the mouse now
  (6 s)`**: press a key and move the mouse. The next line says `RECEIVED` or `NOTHING RECEIVED` with the
  report counts. `RECEIVED` continues to the desktop; `NOTHING RECEIVED` holds the screen up to 30 s:
  photograph it.

## The built-in touchpad (image A, rung 1: mouse mode)

The touchpad is HID over I2C on Serial-IO I2C5 (`Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad.md`,
findings `docs/RESEARCH-2026-10-23-zenbook-i2c-hid-touchpad-FINDINGS.md`). At boot the kernel prints one
`i2c-hid:` line per bring-up step. After the USB summary comes the last block on the boot console:

- **`i2c-hid: TOUCHPAD CHECK - move a finger on the touchpad and click now (6 s)`**: move a finger on the
  touchpad and click.
- The next line says `RECEIVED` or `NOTHING RECEIVED`, with the counters (`reports= id= parsed= … irq=gsi105
  count= … mode=`). `NOTHING RECEIVED` holds the screen up to 30 s (touching the touchpad ends the hold).
- **Photograph the `i2c-hid:` lines.** After the USB summary, the check replays every bring-up line under
  `i2c-hid: ---- touchpad (again) ----`, so one photo holds them. A non-match is replayed too: its single
  `i2c-hid: no match (...)` line, with no TOUCHPAD CHECK after it. Read them against the failure table in
  the findings (§6).
- At the desktop, move the pointer with the touchpad and click; note whether it moves and clicks. A USB mouse
  may stay plugged in: both drive the same pointer.

A machine that is not this Zenbook prints only `i2c-hid: no match (...)` and the touchpad lane touches
nothing.

## Boot time (L7.1)

A run that measures boot time uses image A built with **`--boot-time`**:
`cargo xtask iso --with-model <neom> --boot-time`. It adds the boot-time numbers to image A:
- the kernel's `boot-time`;
- a row on Home's intro card and on the sign-in card;
- a line in Arthur's transcript.

Without the flag, image A is unchanged. Boot it the usual way, and don't interrupt it: the times run from
power-on.

**What to photograph.**
1. **Home's intro card, its second row**, as soon as Home appears. It's the evidence, and it needs no
   typing:

   `From power-on: firmware 4.12 s, model ready 6.50 s, desktop 7.95 s, Home 8.40 s.`

   The four times:
   - **Firmware** is the firmware's own exit-boot-services time, its handoff to the bootloader, from the
     ACPI FPDT.
   - **Model ready** is when the kernel finished building the boot model Arthur answers with
     (`boot-model-preload`, before userland).
   - **Desktop** is when the kernel handed the screen to the desktop.
   - **Home** is when Home first showed.

   To fit the card, this image drops one of the card's middle rows.
2. If a sign-in screen shows (a machine with an installed `.system/`; not the Zenbook today), **its card's
   last row**: `Sign-in 9.87 s after power-on (firmware 4.12 s).`
3. Optionally, **Arthur's first transcript line**, once you have asked it something:

   `From power-on: firmware 4.12 s, model ready 6.50 s, desktop 7.95 s, Arthur ready 31.70 s.`

   Image A starts Arthur only when you type on Home, so its time includes how long you took. The boot
   number is Home's row.
4. Optionally, **the boot console's last line**, just before the desktop takes the screen. It flashes by:

   `[ boot ] time: firmware handoff 4.12 s, kernel entry 4.30 s, model ready 6.50 s, desktop start 7.95 s (from power-on; TSC … Hz)`

   The code makes the cross-check itself: the TSC times are called **from power-on** only when the
   firmware recorded its handoff and the kernel entry comes after it. Otherwise every line says **since
   TSC reset** (`(since TSC reset; …)` here, `Since TSC reset: …` on Home) and leaves the firmware time
   out, since it is on another clock. Record such a run, but note that its times aren't from power-on.

**Recording a run.** Add a row to the table below in the same PR as the photo's reading. A run is a cold
boot: power off fully first (step 1 above).

| Date | Image (commit) | Firmware | Model ready | Desktop | Home | Notes |
|---|---|---|---|---|---|---|
| | | | | | | |

**If a time is missing.**
- `firmware … unknown (<reason>)` or no firmware part: the FPDT was not found or not readable. The reason
  names which (`no-fpdt`, `bad-record`, `address-out-of-range`, …). Without the firmware's handoff, the
  times read `since TSC reset`.
- `firmware handoff not recorded`: the FPDT is there, but its handoff time is 0 (not recorded). It is
  never shown as `0.00 s`; the times read `since TSC reset`.
- No `model ready` part: the image has no boot model, or its build failed (the boot console says which).
- The kernel line ends with `rate=`, where the TSC rate came from:
  - `calibrated` (the LAPIC timer's, the normal case);
  - `cpuid-15`;
  - `cpuid-16-estimate`, the processor's base frequency, which Intel says need not be the TSC's: the times
    are then estimates. Home's row says `(estimated clock)`, and the kernel line ends `…Hz, estimated)`;
    say so in the table's notes.

  `at TSC … (no TSC rate)` means none of them was available; divide by a rate from the `timer:` lines
  below.

## If it does not boot

- **Nothing at all after the boot menu** (black screen, no Limine menu): the stick is not a raw image,
  Secure Boot was re-enabled, or the firmware does not list it as UEFI. Re-write it in DD mode and check
  the firmware settings.
- **Limine panics before the kernel**, for example `Kernel does not support x2APIC`: that is a
  bootloader-stage refusal (CP-68 fixed this one). Photograph it.
- **The log stops early:** NOTE §4 maps the last visible line to its likely cause.
- **The timer** (CP-69). Image A and the bare-metal bundles tick from the **LAPIC timer**, so a gated
  8254 or a missing 8259 route no longer freezes the boot. The screen names the outcome:
  - `timer: tsc rate=… source=hpet checked=acpi-pm…` and `timer: LAPIC tick mode=… first_tick=…`: the
    LAPIC tick is running;
  - `timer: LAPIC tick mode=… gave NO TICK … demoting`, then `PIT/PIC fallback tick alive (lapic=…)`: the
    LAPIC mode did not tick, and the verified PIT/PIC tick took over;
  - `timer: NO TICK SOURCE (lapic=<reason>, pit=gated) - halting`: nothing ticks. Photograph the
    `timer:` lines above it; they say why the LAPIC was not used (for example `uncalibrated`,
    `pit-only-clock`, `lapic-rate-mismatch`).
