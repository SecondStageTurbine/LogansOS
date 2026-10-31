<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: driver coverage for the PL's two machines, and the order to build in

**Date:** 2026-10-22 · **Author:** PM · **Inputs:** Windows `Get-PnpDevice -PresentOnly` exports from both machines
(the inventory files are kept in the private repo, not in this tree).

**Why this exists.** The PL asked about a "driver pack" like Windows ships. Windows `.sys` drivers cannot run on our
kernel (a different driver model). Linux drivers can't be imported either: they are welded to Linux internals, and
they are GPL against our MPL kernel. We may read them for the principle and the traps, but we write our own code. Our
equivalent of a driver pack is **class drivers**: one driver per published standard, built into every image and bound
by PCI/ACPI ID at boot. Vendor-specific chips (Wi-Fi, GPU) are the exception and are costed separately below.

**Today's baseline** (main 4e9ea28c): xHCI (first controller only; #93 fixes that), USB HID boot keyboard/mouse,
USB CDC-ECM, mt7601u USB Wi-Fi, NVMe, AHCI, virtio-blk/net, the GOP framebuffer, a Gen9 GPU compute path (M710q only),
HPET/ACPI-PM/LAPIC timers, the CMOS RTC, VT-d. **There is no PS/2 (i8042) keyboard driver and no ACPI AML
interpreter.** Only static tables are parsed (FADT/HPET/DMAR/MADT).

## The two machines

| | Zenbook S14 UX5406AA (test target) | Main PC, MSI MS-7E57 (Ryzen 9 9900X) |
|---|---|---|
| USB | Intel xHCI `8086:E37D` (PCH, USB-A + USB2 pins) and `8086:E331` (Type-C/TCSS); USB4 routers `E333/E334` | four AMD xHCI (`1022:15B6/15B7/15B8`, chipset `1022:43EF`) |
| Storage | NVMe (Sandisk SN5100S) **behind Intel VMD** `8086:B06F` (hidden from us, which suits the never-write rule) | 4x NVMe (Samsung 970 EVO+, WD Blue SN5000, WD Black SN750), AHCI `1022:43EB` with SATA SSDs |
| Built-in keyboard | **PS/2 via the EC: `ACPI\ATK3001` "PC/AT Enhanced PS/2 Keyboard"**, plus ASUS hotkeys (`ASUH2024`) and Intel HID buttons (`INTC816`) | n/a (USB keyboards: Razer `1532:0067`, `0C45:7603`) |
| Pointing | Touchpad `ASCE1206` (**I2C-HID**) on Intel Serial-IO I2C `8086:E350/E351`; touchscreen + pen `ELAN9008` (**I2C-HID**); GPIO `INTC10BB` | USB mice |
| Network | Intel Wi-Fi 7 BE211 `8086:E340` (CNVi); no Ethernet | **Realtek RTL8125 2.5GbE `10EC:8125`**; MediaTek Wi-Fi 7 MT7925 `14C3:7925` |
| Audio | Intel SST + **SoundWire**, Cirrus CS35L56 amps + CS42L43 codec (SDCA) | **HD Audio**: AMD `1022:15E3` + Realtek ALC897; NVIDIA/AMD HDMI audio |
| Graphics | Intel Xe3 `8086:B0A0` (GOP only) | RTX 5090 + AMD iGPU (GOP only) |
| Other | Intel NPU `8086:B03E`; ISH sensors; UVC + IR camera `3277:0059`; battery/AC/lid/thermal via ACPI | a **COM1 16550** (`PNP0501`): a real serial port |

## Coverage tiers and the build order

Each step is costed in dependency depth and iteration cycles, per CLAUDE.md, not in calendar time.

1. **USB on every controller: #93, in flight.** This unblocks external keyboards and mice on both machines. The main
   PC has four xHCIs, so #93's census matters there too.
2. **PS/2 / i8042 keyboard driver.** It is small and self-contained, with one legacy controller. It covers the
   Zenbook's **built-in keyboard** (`ATK3001` is an EC-emulated PS/2 keyboard) and most laptops ever made. Traps to
   mine: the FADT `IAPC_BOOT_ARCH` "8042 present" bit (often wrong, so probe), the controller self-test, IRQ1 through
   the IOAPIC (MADT override), and scancode set 2 translation. It needs no AML if ports 0x60/0x64 and IRQ1 are assumed
   and probed. **Recommended next driver after #93.**
3. **ACPI AML interpreter (foundation).** It is required for anything known only through the DSDT: the I2C-HID
   touchpad and touchscreen (`_CRS` I2cSerialBus + GpioInt), battery, AC, lid, thermal and fan, the power button,
   proper shutdown (`_S5`) and sleep. This is a genuinely deep, multi-cycle piece. **Decision for the PL:**
   - write our own interpreter, or
   - vendor a permissively licensed one: the rust-osdev `acpi`/`aml` crates are MIT/Apache-2.0, and ACPICA is
     dual-licensed BSD/GPL.

   Either way it gets a cross-family review and a kill-switch.
4. **I2C-HID over Intel Serial-IO (DesignWare I2C) + GPIO interrupts.** This builds on 3. It is one class driver for
   **the touchpad and touchscreen on most modern laptops**, the Zenbook included. HID report parsing exists already
   (USB HID), and it needs the general report-descriptor parser for precision-touchpad reports.
5. **Wired Ethernet: Realtek RTL8125/8169 family.** It gives the main PC (and most desktops) real networking. It is a
   PCI NIC with descriptor rings, the same shape as virtio-net, plus PHY bring-up. The I-14 DMA fence applies (CLAUDE.md
   connectivity rule).
6. **HD Audio** (controller + codec). It covers the main PC's Realtek ALC897 and most desktops. The Zenbook does NOT
   use HDA: its SoundWire/SDCA audio is a separate, harder stack (tier 8).
7. **Hard, vendor-specific (each is its own program):**
   - Intel Wi-Fi 7 BE211: a firmware-driven MVM-class driver. Firmware blobs ship under Intel's redistributable
     licence.
   - MediaTek MT7925: CONNAC3. Our mt7601u work is a different generation.
   - Intel NPU: interesting for an AI OS; a firmware-driven, documented accelerator.
8. **Not soon:** SoundWire/SDCA audio, the ISH sensor hub, UVC cameras, discrete GPU acceleration, Bluetooth, USB4
   tunnelling (USB2 pins work without it).

## Notes

- **The Zenbook's internal NVMe sits behind VMD,** and we have no VMD driver. That means the kernel does not even see
  the Windows disk, which is a second wall beside the write guard. Do not add a VMD driver without revisiting the
  never-write rule.
- **The main PC has a real COM1.** If it is ever used as a bare-metal target, it gives us serial logs again. Today it is
  the PL's daily machine and is not a test target.
- **Inventories are snapshots.** Re-export after hardware changes. The commands are in the PM chat from 2026-10-22
  (`Get-PnpDevice -PresentOnly | Select Class, FriendlyName, InstanceId | Export-Csv`).
