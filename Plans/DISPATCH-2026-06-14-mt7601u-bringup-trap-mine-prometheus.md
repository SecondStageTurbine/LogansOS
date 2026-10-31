<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: MT7601U deep bring-up trap-mine (raw driver source) → Prometheus

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Prometheus (research lane — deep prior-art mining; NO kernel code)
**Status:** DISPATCHED. **The trap-mining-rule pass for BUILD-7.5-W1, run *before* the dongle lands.** Pure research/acquisition; the deliverable is a verified bring-up reference + an extended trap bank, copied/derived from **raw driver source** (`git clone`, never WebFetch for exact sequences — the raw-source rule, which you just honored cleanly on the SAE lane).
**De-risks:** BUILD-7.5-W1 (the MT7601U USB SoftMAC driver — blind bare-metal bring-up), which fires the moment the `148f:7601` dongle is in the OptiPlex (~2 days). Running this now means W1's trap bank is **complete and verified before the first boot** — the AX88179-day insurance.

## 0. Why this lane (the standing rule)
CLAUDE.md mandates a **deep prior-art trap-mining pass before any driver/blind-hardware build** — enumerate the documented field gotchas for *the exact chip*, from the vendor spec + Linux/BSD driver source + kernel lists + OSDev, and pre-seed the failure-mode/discriminator bank *before* the first run. The AX88179 day (eight blind iterations on what was our own bug, all three real blockers already documented in prior art) is the case study for why. We have a **7-trap starter bank** (below) from the research *summary*; this lane turns that into a **deep, raw-source-verified bring-up reference**: the actual register sequence, the firmware handshake, the MCU protocol, and every additional trap the summary didn't reach — so W1 is a checklist, not a mystery.

## 1. The starter bank (already on `main` — VERIFY and EXTEND, don't re-derive)
From `Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` §4 (read it: `git show origin/main:Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md`):
1. **Firmware load** — `mt7601u.bin` (~44 KB initcode); `mt7601u_fw_header` magic; chunk to EP maxpacket.
2. **eFUSE calibration** — clones ship unprogrammed eFUSE (zero MAC, no RF cal); `MT_EFUSE_CTRL` no-valid-data → fallback.
3. **USB transfer-size limits** — MCU cmds ≤192 B; firmware chunks ≤ EP maxpacket; TX ring ~32 desc.
4. **Regulatory lockout** — no reg domain → country 00 hides channels.
5. **Deep-sleep hang** — clones' wake pin unwired; deep sleep bricks until replug.
6. **TX ring overflow** — ~32 desc; over-submission → `-EBUSY`.
7. **Band-specific initcode** — load the right table.
**Your job: confirm each against the raw source (is it real, is the discriminator right?), correct anything the summary got wrong, and add every trap it missed.**

## 2. Sources (raw `git clone` / `curl` — NEVER WebFetch for exact register/byte sequences)
- **Linux `mt7601u`** — `git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git`, `drivers/net/wireless/mediatek/mt7601u/` (`init.c`, `usb.c`, `mcu.c`, `eeprom.c`, `phy.c`, `dma.c`, `tx.c`, `mac.c`, `regs.h`, `mt7601u.h`). This is the authoritative bring-up. Read the literal register writes and sequencing.
- **BSD `net80211` + USB WiFi drivers** — OpenBSD/FreeBSD `sys/dev/usb/if_run.c` / `sys/net80211/` (the `run`/`mt76xx` family where present). BSD's `net80211` STA state machine is a **far smaller minimal-STA reference than Linux mac80211** — mine it for the bounded auth→assoc→4-way→data path W2 will need (a crib to *port*, not a from-scratch MLME).
- **linux-firmware** — `git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git`, `mediatek/mt7601u.bin` + its `WHENCE` license entry; the `mt7601u_fw_header` struct in the driver. Capture the exact header layout + chunk-upload protocol + the BSD-3-Clause-Clear redistribution terms (we need to vendor the blob — confirm `boot/firmware/mediatek/mt7601u/` is license-clean).
- **Field reports** — OpenWrt (`mac80211`/MT7601U), the linux-wireless list, OSDev, and Vendor reset/quirk threads — for the bring-up failures real people hit (each as symptom→cause→fix).

## 3. Deliverable
`Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` containing:
1. **The bring-up sequence reference** — the ordered init from the raw driver: USB enumeration/interface + the 3 bulk endpoints (EP1 OUT TX, EP2 IN RX, EP3 IN interrupt/events — confirm the numbers), the **MCU command protocol** (`MT_H2M_*`/`MT_M2H_*` — the control-OUT request + the interrupt-EP response shape), the **firmware-load handshake** (header check → chunked upload → run/ready poll), the **eFUSE/EEPROM read** (`MT_EFUSE_CTRL`), the **MAC/PHY init + calibration**, and the **bulk data path** (raw 802.11 frame TX/RX descriptor format + the RXWI/TXWI prepend). Cite file:line for each step. Exact register *values* must be the literal source bytes (raw-pull, not summarized).
2. **The extended, verified trap bank** — the 7 above each marked confirmed/corrected, **plus every additional trap** (reset quirks, USB SET_INTERFACE/alt-setting needs, endpoint-stall recovery, the WMM/EDCA init, the BBP/RF register soak-loops with their timeouts, the ASIC version gating, vendor-request idiosyncrasies), **each with a discriminator** (the breadcrumb/observation that self-localizes it on a blind run — the discriminator discipline).
3. **The W2 minimal-STA crib** — the bounded MLME path from BSD `net80211`: the STA state machine (scan→auth→assoc→4-way→run), which mgmt frames must be built, and the key-install seam (keys pushed via MCU cmd; firmware does data-plane CCMP) — sized as "what's the smallest thing that associates," not the full surface.
4. **Firmware provenance** — the blob's source, the `mt7601u_fw_header` layout (verbatim struct), the chunk protocol, and the confirmed redistribution license (so W1 can vendor it cleanly).
5. **Provenance + how-obtained** for every source (repo URL, commit, file:line, `git clone`/`curl` — explicit; a WebFetch-sourced register sequence is disqualified and must be re-pulled raw).
6. **The honest negatives** — anything the raw source *contradicts* in our starter bank (say so), and any trap you can't resolve from source (flag it for a blind discriminator instead).

## 4. Scope guard
Research/acquisition only. **Do NOT** write Rust, implement the driver, or touch `kernel/`. **Do NOT** WebFetch exact register/firmware-byte sequences (raw `git`/`curl` only — the rule). **Do NOT** "tidy" copied register values or struct layouts. **Do NOT** design the W1 architecture (that's the W1 THINK/dispatch — you supply the mined facts it's built on).

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-mt7601u-bringup-trap-mine-prometheus.md`
- **Read it + the starter bank from `main`** (your tree is on a feature branch — a local file-open won't find them):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-mt7601u-bringup-trap-mine-prometheus.md
  git show origin/main:Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md   # §4 starter bank
  ```
- **One-line:** The trap-mining pass for W1, run before the dongle lands. **`git clone` the raw `mt7601u` driver** (Linux `drivers/net/wireless/mediatek/mt7601u/`) + **BSD `net80211`/`if_run`** + **linux-firmware** (`mediatek/mt7601u.bin` + WHENCE) — **never WebFetch exact register/firmware sequences** (the raw-source rule you nailed on SAE). Deliver `Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` with: (1) the ordered **bring-up sequence** (endpoints, MCU `MT_H2M`/`MT_M2H` protocol, firmware-load handshake + `mt7601u_fw_header`, eFUSE read, MAC/PHY init + cal, TXWI/RXWI data path) cited file:line; (2) the **7-trap starter bank verified/corrected + every trap it missed**, each with a blind-run discriminator; (3) a **W2 minimal-STA MLME crib** from BSD `net80211` (the smallest associate path); (4) firmware provenance + license. Verify-and-extend the bank, don't re-derive it. Pure research — no code, no architecture. This makes W1's blind bring-up a checklist instead of a mystery.
