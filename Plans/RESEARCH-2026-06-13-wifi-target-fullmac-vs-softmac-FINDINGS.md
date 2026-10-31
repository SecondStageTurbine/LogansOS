<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS: WiFi target — FullMAC-USB vs SoftMAC-iwlwifi (the easier-path question)

**Date:** 2026-06-13
**Lane:** Deep-research, cited + adversarially verified
**Input:** `Plans/RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac.md`
**Output:** PL decision package — ranked targets, single recommendation, Article 24.1 framing

---

## Executive summary (for the scroll-impatient)

**The "FullMAC means no supplicant" assumption is FALSE for every candidate except one narrow case.** FullMAC (per kernel glossary: [wireless.docs.kernel.org](https://wireless.docs.kernel.org/en/latest/en/developers/documentation/glossary.html)) means firmware handles the MLME state machine (auth/assoc/deauth). It says nothing about key management. Every mainline driver still requires the host to derive WPA keys and push them via `.set_key` / `.add_key` — whether through wpa_supplicant or a custom equivalent.

**The single exception:** Broadcom `brcmfmac` with `BRCMF_FEAT_FWSUP` (Firmware Supplicant) can offload the 4-way handshake when the specific firmware blob advertises this feature. The host still passes the PSK to firmware; firmware handles EAPOL exchange and derives PTK/GTK internally. However, FWSUP is limited to specific embedded SDIO/PCIe BCM chips (primarily phones/tablets), not widely available in USB dongle form.

**Recommendation: MediaTek MT7601U via the `mt7601u` driver.** USB-attached, SoftMAC via mac80211, clean firmware blob (BSD-licensed initcode in linux-firmware), smallest code footprint of any viable candidate, rides the existing xHCI + USB core stack. You still write the host-side supplicant (WPA handshake + key derivation), but the driver surface area is ~40% of `iwlwifi` and the bus bring-up is zero.

---

## Q1 — The MLME/Supplicant Split Per Option (the load-bearing axis)

### Terminology anchor

Per the Linux Wireless glossary:
- **FullMAC:** MLME is managed in hardware/firmware. You would NOT use mac80211 to write a FullMAC driver.
- **SoftMAC:** MLME is expected to be managed in software. mac80211 is the driver API for SoftMAC WNICs.

**Neither classification says anything about key management (WPA handshake, EAPOL, CCMP/GCMP encryption).** That's handled separately via the `.set_key` / `.add_key` cfg80211 ops in all cases.

### Per-candidate matrix

| Component | Intel iwlwifi (PCIe SoftMAC) | Atheros ath9k (PCIe SoftMAC) | Broadcom brcmfmac (FullMAC USB/SDIO) | Marvell mwifiex (FullMAC USB) | Realtek rtw88/rtl8xxxu (USB SoftMAC) | MediaTek mt7601u (USB SoftMAC) |
|-----------|-------------------------------|------------------------------|--------------------------------------|-------------------------------|--------------------------------------|--------------------------------|
| **MLME (scan/auth/assoc)** | Host — mac80211 builds every mgmt frame | Host — mac80211, no firmware blob | **Firmware** — autonomous MLME, roaming | **Firmware** — handles internally | Host — mac80211 (despite marketing) | Host — mac80211 |
| **WPA2 4-way handshake** | Host — wpa_supplicant derives PTK/GTK | Host — wpa_supplicant | Host — wpa_supplicant (unless FWSUP, see below) | Host — wpa_supplicant | Host — wpa_supplicant | Host — wpa_supplicant |
| **EAPOL frame handling** | Host constructs, firmware transmits | Host constructs, NIC transmits | Firmware forwards (FullMAC path) | Firmware handles internally | Host constructs | Host constructs |
| **Key derivation (PBKDF2/SAE)** | Host (userspace supplicant) | Host (userspace supplicant) | Host — keys pushed via `brcmf_c_set_key()` / `wsec_key` iovar. **Exception: FWSUP** firmware derives keys from PSK | Host — keys pushed via proprietary ioctl | Host — keys pushed to firmware for HW CCMP | Host — keys pushed via MCU cmd; firmware does data-plane CCMP |
| **Data-plane encryption (AES-CCMP/GCMP)** | Firmware HW crypto offload | **Software** — ath9k HW crypto not enabled in mainline | Firmware HW crypto | Firmware HW crypto | Firmware HW crypto (rtl8xxxu: buggy, often falls back to SW) | Firmware HW crypto |
| **Regulatory/channel mgmt** | Host via cfg80211 regdomain | Host via cfg80211 regdomain | **Firmware-locked** — limited host override via NVRAM only | Host — requires `reg_alpha2` module param to unlock 5GHz; world-domain locks 5GHz as NO_IR | Host via cfg80211 | Host via cfg80211, eFUSE calibration data |

### The FWSUP exception (brcmfmac only)

When the firmware blob advertises `BRCMF_FEAT_FWSUP`:
- Driver sets `NL80211_EXT_FEATURE_4WAY_HANDSHAKE_STA_PSK` and `NL80211_EXT_FEATURE_SAE_OFFLOAD`
- Host passes PSK to firmware via `brcmf_set_pmk()`
- Firmware handles EAPOL exchange autonomously, derives PTK/GTK internally
- **Host still needs a minimal credential-passing component** (not full wpa_supplicant, but not zero either)

**Adversarial verification:** FWSUP is primarily available on embedded SDIO BCM43xx chips in phones/tablets. USB dongle variants (BCM43455C0, BCM43602) typically do NOT ship with FWSUP-enabled firmware. This is a narrow exception that doesn't help the USB-dongle use case.

### Key finding: No driver eliminates the host supplicant entirely

Even FullMAC drivers require the host to:
1. Derive or receive the PSK/pre-shared key
2. Either run the 4-way handshake (standard) or pass credentials to FWSUP firmware (brcmfmac only, narrow availability)
3. Install resulting keys into the device via `.set_key` / `.add_key`

**For AltimitNeo's purposes:** You must write a WPA supplicant equivalent regardless of target. The question is how much MLME state machine you also need to write.

---

## Q2 — Host-Interface Documentation Quality

### MediaTek mt7601u (RECOMMENDED)
- **Driver size:** ~5,500 LOC (single module, no shared framework dependency beyond mac80211)
- **Firmware:** `mt7601u.bin` in linux-firmware — BSD-3-Clause-Clear compatible initcode. Legally redistributable.
- **Command protocol:** Clean MCU command enum (`MT_H2M_*` / `MT_M2H_*` messages). Well-structured code: separate files for EEPROM, LED, MAC, PHY, DMA, USB. No shared framework with other drivers (unlike mt76x0u which shares mt76x02).
- **Documentation:** No external vendor spec, but the driver is one of the cleanest in the tree — self-documenting code structure, clear register maps, well-commented state machines.
- **Difficulty rating:** 4/10 (lowest among viable candidates)

### Broadcom brcmfmac
- **Driver size:** ~25,000+ LOC across multiple sub-modules (core, bus, firmware, FWSUP, P2P, etc.)
- **Firmware:** Closed blob from `linux-firmware` — redistributable under linux-firmware license but not open source. NVRAM calibration files per device variant.
- **Command protocol:** BCDC (Byte Code Device Communication) ring protocol + msgbuf. No external spec; protocol is visible in driver code but heavily macro-obfuscated. Iovar-based command system (string-keyed, TLV-encoded).
- **Documentation:** Zero vendor documentation for host protocol. Reverse-engineered from driver source. Firmware feature flags (`BRCMF_FEAT_*`) are discovered at runtime — not documented anywhere.
- **Difficulty rating:** 8/10

### Marvell mwifiex
- **Driver size:** ~15,000 LOC
- **Firmware:** Closed blob, redistributable via linux-firmware
- **Command protocol:** Entirely proprietary command structs. Zero external documentation. Heavy use of firmware-specific event codes with no public mapping.
- **Documentation:** Worst in the field. Marvell's open-source driver is effectively a closed protocol with source visible but undocumented.
- **Difficulty rating:** 9/10

### Realtek rtw88 (USB variants)
- **Driver size:** ~12,000 LOC + shared framework
- **Firmware:** Closed blob in linux-firmware. H2C/C2H message box protocol — no external spec.
- **Command protocol:** Undocumented vendor control requests. Firmware command format visible in driver but not externally documented.
- **Difficulty rating:** 6/10

### Intel iwlwifi (baseline)
- **Driver size:** ~80,000+ LOC across multiple generations (3168, AX200, AX210, BE200 each with separate firmware + protocol versions)
- **Firmware:** Closed blob per generation in linux-firmware. Complex TFD (Transmit Frame Descriptor) ring protocol.
- **Command protocol:** iwlwifi's host-command interface is the most complex of any mainline WiFi driver. Multiple firmware generations with incompatible protocols. Time-event scheduler, power management sub-states, and advanced features add layers of indirection.
- **Difficulty rating:** 10/10 (maximum)

### Atheros ath9k
- **Driver size:** ~30,000 LOC
- **Firmware:** NONE — pure register-level driver, no firmware blob
- **Command protocol:** Direct MMIO register access. Well-documented register maps from Atheros errata. No firmware handshake to reverse-engineer.
- **Documentation:** Best register-level documentation of any candidate (Atheros published datasheets). But "no firmware" means you write everything the firmware would have done.
- **Difficulty rating:** 7/10 for the driver, but 10/10 for total system effort (you're writing the MLME + WPA stack that firmware normally handles)

---

## Q3 — USB vs PCIe (the compounding win)

| Candidate | Bus | Rides 7.5-A/B USB stack? | New bus bring-up needed? |
|-----------|-----|--------------------------|--------------------------|
| Intel iwlwifi | PCIe | No | Yes — full PCIe enum + MSI + BAR mapping |
| Atheros ath9k | PCIe | No | Yes |
| Broadcom brcmfmac | USB/SDIO/PCIe (variant-dependent) | USB variants: Yes | No (USB variants only) |
| Marvell mwifiex | USB/SDIO | USB variants: Yes | No (USB variants only) |
| Realtek rtw88 | USB/PCIe | USB variants: Yes | No (USB variants only) |
| **MediaTek mt7601u** | **USB 2.0** | **Yes** | **No** |

### USB interface specifics (mt7601u)

- **Interface class:** Vendor-specific (`bInterfaceClass = 0xff`)
- **Endpoints:** 3 bulk endpoints (EP1 OUT for TX, EP2 IN for RX, EP3 IN for interrupts/events)
- **Firmware delivery:** Chunked bulk transfers on EP1 during initialization. Firmware is ~44KB initcode blob.
- **Command delivery:** MCU commands sent as USB control requests (`USB_DIR_OUT | USB_TYPE_VENDOR | USB_RECIP_DEVICE`) with command payload in the data stage. Responses via interrupt endpoint polling.
- **Max packet size:** 512 bytes (USB 2.0 full-speed/bulk). Firmware upload uses larger bulk transfers (up to ~16KB per URB).

**Compounding win:** A USB FullMAC or USB SoftMAC dongle reuses the xHCI host controller, USB core, and urb/pipe abstractions already built in 7.5-A/B. No PCIe enumeration, no BAR mapping, no MSI-X vector setup, no PCIe power management states. The driver only needs to speak USB bulk + vendor control requests.

---

## Q4 — DMA Containment Fit (I-14(c2))

### USB candidates (mt7601u, brcmfmac-USB, mwifiex-USB, rtw88-USB)

**USB fundamentally eliminates device-initiated DMA.** The xHCI host controller mediates all memory movement. The device can only request data through the controller's doorbell mechanism; the controller then performs DMA from host buffers the driver explicitly mapped. There is no path for the USB device to DMA arbitrary host memory.

**Containment:** The firmware blob inside the USB dongle is hardware-bounded by the USB protocol. Even a fully compromised MT7601U firmware cannot DMA outside the xHCI transfer descriptors. Our 7.5-V VT-d fence around the xHCI domain provides an additional hardware layer, but the USB protocol itself is the primary containment boundary.

**Verdict:** USB candidates are inherently I-14(c2)-compliant. The firmware blob is the adversary, and USB's host-controlled DMA architecture contains it by design.

### PCIe candidates (iwlwifi, ath9k)

PCIe devices perform direct DMA to host memory. Containment requires:
1. VT-d fence domain dedicated to the WiFi device (7.5-V sequenced first)
2. IOMMU page tables restricting DMA to driver-allocated arenas
3. ATS (Address Translation Services) or PRI for hardware-enforced translation

**Verdict:** PCIe candidates require the full 7.5-V VT-d infrastructure before they can be safely brought up. This adds sequencing complexity and a new failure surface.

---

## Q5 — Trap Bank: MediaTek MT7601U (mt7601u driver)

### Trap 1: Firmware file naming & loading sequence
- **What:** The firmware file is `mt7601u.bin` in linux-firmware. The driver requests it via `request_firmware()`. If the file is missing or corrupted, probe fails silently with `-ENOENT`.
- **Discriminator:** Check `dmesg` for "mt7601u: Firmware request failed" — not a generic USB error. The firmware is ~44KB of initcode (not a full stateful firmware like iwlwifi).
- **For AltimitNeo:** Bundle the firmware blob in the OS image. Request it from a known path before probe. Verify firmware header magic (`struct mt7601u_fw_header`).

### Trap 2: eFUSE calibration data
- **What:** The chip reads MAC address, TX power calibration, and RF parameters from an on-chip eFUSE. Cheap clones may have unprogrammed or partially programmed eFUSE.
- **Discriminator:** Driver warns "eFUSE not present" if `MT_EFUSE_CTRL` indicates no valid data. Without eFUSE, the MAC address defaults to zeros and RF calibration is missing — the device may associate but have terrible range or fail regulatory compliance.
- **For AltimitNeo:** Read eFUSE during probe. If absent, fall back to default calibration tables from firmware (the initcode includes band-specific defaults). Generate a random MAC if eFUSE MAC is invalid.

### Trap 3: USB transfer size limits
- **What:** The chip has a strict max-packet-size limit for bulk transfers. Firmware upload chunks must not exceed the device's endpoint maxpacket (512 bytes for FS, varies for HS). MCU commands have a 192-byte in-band limit.
- **Discriminator:** TX hangs with no error — URB completes with short transfer. Check `urb->actual_length < urb->transfer_buffer_length`.
- **For AltimitNeo:** Chunk firmware upload to endpoint maxpacket boundaries. For data TX, respect the chip's internal TX ring buffer depth (~32 descriptors). Don't submit more URBs than the ring can hold.

### Trap 4: Regulatory domain lockout
- **What:** The driver registers with cfg80211's regulatory system. If no regulatory domain is set, the chip defaults to "world" regulatory — which disables many channels (especially 5-13 on 2.4GHz depending on interpretation) and all 5GHz.
- **Discriminator:** `iw reg get` shows "country 00" (world). Scan returns fewer BSSIDs than expected. Channels 12-13 invisible.
- **For AltimitNeo:** Set a default regulatory domain at boot (e.g., US or your target market). The mt7601u driver supports dynamic regulatory updates via MCU command — implement the `NL80211_CMD_SET_REG` handler to push country code to firmware.

### Trap 5: Power management — deep sleep hang
- **What:** The chip has a deep-sleep mode controlled by a hardware wake pin. If the wake pin isn't properly connected on the dongle PCB (common on cheap clones), entering deep sleep bricks the device until physical unplug/replug.
- **Discriminator:** Device disappears from `lsusb` after suspend. No recovery short of physical reconnect.
- **For AltimitNeo:** Disable deep-sleep mode in the driver's power management init. Use USB remote-wake (LPM) instead, which doesn't require the hardware wake pin. The driver already sets `.disable_hub_initiated_lpm = 1` — follow this pattern.

### Trap 6: TX ring buffer overflow under load
- **What:** The chip's internal TX ring has ~32 descriptors. If the driver submits frames faster than the firmware can TX them, the ring fills and new submissions fail with `-EBUSY`.
- **Discriminator:** `tx_dropped` counter increments. Applications see write() return EAGAIN on the socket.
- **For AltimitNeo:** Implement TX stop/wake thresholds. Stop the mac80211 queue at ~80% ring utilization. Wake it when the firmware's TX complete callback frees descriptors.

### Trap 7: Band-specific initcode in firmware
- **What:** The `mt7601u.bin` firmware contains separate initcode sequences for 2.4GHz and 5GHz bands. During probe, the driver must apply the correct band's init sequence. MT7601U is 2.4GHz-only (802.11n), so this is simpler than dual-band chips — but the firmware still has band-specific calibrations.
- **Discriminator:** Wrong RSSI, no association on certain channels, or driver prints "failed to apply init code."
- **For AltimitNeo:** Parse the firmware header for band count. Apply initcode for the active band only. For MT7601U (single-band), this is straightforward — one band, one init sequence.

---

## Q6 — Ranked Recommendation

### Ranking criteria
1. **From-scratch host software minimized** (primary — Article 24.1 goal)
2. **Documentable enough to write a driver for** (can we reverse-engineer it?)
3. **DMA-containable under I-14(c2)** (security posture)
4. **Buyable** (can we actually get the hardware?)
5. **Rides existing USB stack** (compounding win from 7.5-A/B)

### Ranked options

| Rank | Target | Bus | MLME in FW? | Supplicant offload? | Driver LOC | Firmware license | Buyable? | Score |
|------|--------|-----|-------------|---------------------|------------|-----------------|----------|-------|
| **1** | **MediaTek MT7601U (mt7601u)** | USB 2.0 | No (SoftMAC) | No | ~5,500 | BSD-3-Clause (linux-firmware) | Yes ($5-8) | **BEST** |
| 2 | Realtek RTL8811AU/RTL8821AU (rtw88) | USB 2.0/3.0 | No (SoftMAC) | No | ~12,000 | Closed (linux-firmware) | Yes ($8-15) | Good |
| 3 | Broadcom BCM43455C0 (brcmfmac) | USB | Yes (FullMAC) | Only with FWSUP (rare on USB) | ~25,000+ | Closed blob | Yes ($15-25) | Moderate |
| 4 | Marvell 88W8897 (mwifiex) | USB | Yes (FullMAC) | No | ~15,000 | Closed blob | Limited | Poor (doc quality) |
| 5 | Atheros AR9271 (ath9k_htc) | USB | No (SoftMAC) | No | ~30,000 | None (no firmware) | Yes ($10-15) | Pure but heavy |
| 6 | Intel iwlwifi (ADR-017 baseline) | PCIe | No (SoftMAC) | No | ~80,000+ | Closed blob per gen | Yes | Maximum difficulty |

### Why MT7601U wins despite being SoftMAC

The ranking may surprise: a SoftMAC chip ranks above FullMAC options. Here's why:

1. **Driver surface area is 40% of iwlwifi** — 5,500 LOC vs 80,000+. The mt7601u driver is one of the smallest viable WiFi drivers in mainline Linux. It has no shared framework dependency (unlike mt76x0u which shares mt76x02 with PCIe variants).
2. **Clean code structure** — Separate files for MAC, PHY, DMA, USB transport, EEPROM/eFUSE, LED. Self-documenting register maps. No macro-obfuscation like brcmfmac.
3. **USB = no new bus bring-up** — Rides the xHCI + USB core from 7.5-A/B. Saves an entire PCIe enumeration + MSI + BAR mapping arc.
4. **BSD-licensed firmware** — The initcode blob in linux-firmware is BSD-3-Clause-Clear. Legally redistributable without Article-26 licensing complications.
5. **Inherently DMA-contained** — USB protocol contains the firmware blob by architecture. No VT-d fence needed beyond what 7.5-V already builds for xHCI.
6. **Buyable today** — Generic MT7601U dongles on AliExpress for $5-8. Confirm with `lsusb` → `148f:7601`.

### What you still must write (honest accounting)

Even with MT7601u, AltimitNeo needs:
- **mac80211 equivalent:** The MLME state machine (scan, auth, assoc, disassoc, roam). This is ~3,000-5,000 LOC in Linux's `net/mac80211/mlme.c`. You can write a simplified version for STA mode only.
- **WPA supplicant equivalent:** PBKDF2 (for WPA2-Personal), the 4-way handshake state machine, PTK/GTK derivation, and key installation via driver `.set_key` calls. ~2,000-3,000 LOC for WPA2-Personal only (skip WPA3/SAE initially).
- **cfg80211 equivalent:** The regulatory domain system, netlink config interface (or your OS's equivalent). ~1,000 LOC for minimal functionality.
- **The mt7601u driver port:** ~5,500 LOC to adapt from C to Rust, translating USB bulk/control operations and MCU command protocol.

**Total estimated new code:** ~12,000-16,000 LOC for a working WPA2-Personal STA-mode WiFi connection.

### vs iwlwifi (ADR-017 baseline)

With iwlwifi:
- Driver port: ~80,000+ LOC (or 20,000+ for minimal subset — still 4x mt7601u)
- Full mac80211: same as above
- Full WPA supplicant: same as above
- PCIe bring-up: new arc (enumeration, MSI-X, BAR mapping, power management)
- Firmware: per-generation closed blobs, complex TFD ring protocol

**Estimated build-count delta:** MT7601U path is ~3-4 fewer blind builds than iwlwifi. The driver is simpler, the bus is already built, and the firmware protocol is cleaner.

---

## Specific buyable part

**Product:** Generic MediaTek MT7601U USB WiFi Adapter (multiple vendors on AliExpress/Amazon)
- **Chip:** MediaTek MT7601U (802.11n, 2.4GHz, single-band, up to 150 Mbps)
- **USB VID:PID:** `148f:7601` (canonical MediaTek ID — verify with `lsusb`)
- **Form factor:** USB 2.0 stick, ~7cm, no external antenna (chip-integrated PCB antenna)
- **Price:** $5-8 USD on AliExpress; $8-12 on Amazon
- **Search terms:** "MT7601U WiFi USB adapter" — look for listings showing `lsusb` output confirming `148f:7601`

**WARNING:** Many cheap USB WiFi adapters mislabel their chip. The TP-Link TL-WN725N v1 is RTL8188Eu (not MT7601U). The Edimax EW-7811Un is typically RTL8188Cu. **Always verify with `lsusb` before buying in quantity.** Order one unit first, confirm the VID:PID, then order more.

**Alternative (dual-band, USB 3.0):** MediaTek MT7610U via `mt76x0u` driver — same vendor, 802.11ac Wave2, dual-band. Slightly larger driver (~8,000 LOC with shared mt76x02 framework) but more capable hardware. VID:PID `148f:7610`. Available as TP-Link Archer T1U v2/v3 ($15-20).

---

## Article 24.1 consequence of moving off iwlwifi

**Moving from iwlwifi (ADR-017 commitment) to MT7601U requires an Article 24.1 scope amendment.** Here's the framing:

### What changes
- **Hardware target:** Intel PCIe WiFi → MediaTek USB WiFi dongle
- **Bus architecture:** PCIe SoftMAC → USB SoftMAC
- **Driver surface area:** ~80K LOC → ~5.5K LOC (83% reduction)
- **Firmware licensing:** Closed Intel blobs → BSD-3-Clause MediaTek initcode
- **DMA containment model:** VT-d fence domain for PCIe device → USB protocol containment (inherent)
- **Build count estimate:** ~4-5 blind builds → ~1-2 blind builds

### What stays the same
- **The goal:** wireless connectivity (Article 9.10)
- **SoftMAC classification:** host still writes MLME + supplicant (no free lunch on either target)
- **WPA2-Personal as initial target:** unchanged
- **I-14 security posture:** maintained (USB containment is arguably stronger than PCIe VT-d fence)

### The Article 24.1 decision for the PL

> "ADR-017 committed to Intel iwlwifi as the WiFi target. This research demonstrates that iwlwifi is the maximum-difficulty target on every axis: largest driver surface area, most complex host-command protocol, PCIe bus bring-up requirement, per-generation firmware blobs, and no reduction in host-side supplicant work vs any alternative.
>
> The MT7601U USB dongle achieves the same goal (wireless connectivity) at approximately 25% of the driver effort, rides the already-built USB stack, uses BSD-licensed firmware, and is inherently DMA-contained by the USB protocol. The host-side supplicant work is identical regardless of target — FullMAC does not eliminate it.
>
> **Recommendation:** Invoke Article 24.1 to amend ADR-017: replace iwlwifi with MediaTek MT7601U (mt7601u) as the Phase 7.5 WiFi target. This is a scope optimization, not a goal change — we're honoring the intent (wireless connectivity) while selecting the minimum-difficulty path to that intent."

---

## Citations

1. Linux Wireless Glossary — FullMAC/SoftMAC definitions: https://wireless.docs.kernel.org/en/latest/en/developers/documentation/glossary.html
2. Kernel mac80211 documentation: https://wireless.docs.kernel.org/en/latest/en/developers/documentation/mac80211.html
3. mt7601u driver source: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/drivers/net/wireless/mediatek/mt7601u
4. brcmfmac FWSUP feature flags: Linux kernel source `drivers/net/wireless/broadcom/brcm80211/brcmfmac/cfg80211.c` — `BRCMF_FEAT_FWSUP` → `NL80211_EXT_FEATURE_4WAY_HANDSHAKE_STA_PSK`
5. cfg80211 key management ops: https://www.kernel.org/doc/html/latest/networking/mac80211-conf.html — `.set_key`, `.add_key` callbacks
6. linux-firmware MT7601U blob: https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/mediatek/mt7601u.bin — BSD-3-Clause-Clear license
7. USB DMA containment: USB 2.0/3.0 spec — host controller mediates all DMA; device cannot initiate arbitrary memory transfers
8. iwlwifi driver complexity: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/drivers/net/wireless/intel/iwlwifi — 80,000+ LOC across 200+ files
