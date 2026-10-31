<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7.5 BUILD-W1 — the MT7601U USB SoftMAC driver (pre-stage)

**Date:** 2026-06-14
**Seat:** PM/Cerebro (arc design pass — **pre-staged while the dongle ships**; not yet a dispatch)
**Status:** **DISPATCH-READY** (the Prometheus raw-source mine landed 2026-06-14 — `Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md`, on `main`, 7 starter traps verified/corrected + 20 new (T-001…T-020), file:line-cited bring-up sequence). The governance scaffolding (I-14, the blind-hardware discriminator framework, the kill-switch, the slice, the gate) stands; the chip-specific facts are now in the FINDINGS, referenced below (not duplicated — the FINDINGS is the authoritative bring-up reference the dispatch's ISCs are built on). **The only remaining gate to promote this to a dispatch is the dongle in hand** (`lsusb` = `148f:7601` on the OptiPlex) — see §9. One PL-facing risk surfaced by the mine (the eFUSE HARD-FAIL) is flagged in §4/§6 and needs a decision before/at dispatch.
**Related:** the arc THINK (`THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` §2 the W-slice, §4 the starter trap bank); the W0/W0b crypto core (merged — the supplicant crypto is proven); ADR-046 (xHCI/USB core), ADR-047 (CDC-ECM/NetDevice precedent), ADR-050 (VT-d fence domain); the CLAUDE.md standing rules (trap-mining, blind-hardware discriminator, I-14 — all three bind this build).

## 1. What W1 is (and is NOT)
W1 is the **MT7601U USB SoftMAC driver up to and including scan** — enumerate the `148f:7601` dongle on the 7.5-A/B USB core, load its firmware, bring up MAC/PHY, and **list real BSSIDs on the OptiPlex**. It is the blind-hardware build of the WiFi arc.
- **In scope:** vendor-class interface claim; the 3 bulk endpoints; firmware chunk-upload; the MCU command interface; eFUSE/EEPROM read; MAC/PHY init + calibration; the raw-802.11 bulk data path (TX/RX descriptor handling); **scan** (receive beacons/probe-responses → enumerate BSSIDs). Gate: enumerate + firmware-load + a scan that lists real APs.
- **Explicitly NOT in scope:** the MLME auth/assoc/4-way state machine (that's W2), the `NetDevice` glue (W2), CCMP data path on-air (W2), AP/mesh/P2P/roaming (follow-ons). W1 proves *we can see the air*; W2 proves *we can join it*.

## 2. I-14 fault-containment analysis (the standing rule — already satisfied by the USB architecture)
The connectivity-dispatch rule requires this section before DISPATCHED. For W1 it is a **one-line confirmation, not new work**, because the MT7601U is USB:
- **(a) lock isolation:** the W1 bulk-completion/event path takes **no** AI-governance lock — it reuses 7.5-A/B's bounded ISR drain (`USB_*` locks order above all AI levels under the I-5 lockdep; the band ceiling `USB_CORE`=31 is unchanged — W1 adds a device-state leaf lock, not a new ordered level). Grep-assertable + the runtime no-AI-lock-in-IRQ assertion.
- **(b) liveness:** RX/event flood is capped per service pass (the 7.5-B `EP_QUEUE_CAP` pattern, reused) — a firmware event storm degrades to the backstop, never stalls governance.
- **(c) memory isolation:** **handled by the USB architecture itself** — the MT7601U firmware (the adversary) can only move data through transfer descriptors the driver explicitly maps; the xHCI controller mediates all DMA and **already sits in VT-d's fence domain (7.5-V, silicon-proven)**. Even fully-compromised firmware cannot DMA arbitrary host memory. **No new (c) vector, no new guard** — this is the whole reason the USB MT7601U target was strictly superior to a PCIe `iwlwifi` master.

## 3. The blind-hardware failure-mode + discriminator framework (the standing rule)
W1 is bare-metal blind bring-up — the archetype the discriminator rule was written for (and the AX88179 day is the cautionary case). The dispatch MUST design every run to be **conclusive whether it passes OR fails**. The base rate is "the next run reveals the next layer," so each candidate failure gets a self-localizing breadcrumb *before* the boot:

**Framework (the discriminator slots — populated from the trap bank in §4):**
| Stage | Pass breadcrumb | The distinct failure breadcrumbs that localize it |
|---|---|---|
| Enumerate | `148f:7601` claimed, 3 bulk EPs found | no-claim (interface/class mismatch) vs wrong-EP-layout — distinct logs |
| Firmware load | header-OK → chunks-acked → run/ready poll OK | three distinct error paths (FINDINGS §1.4/§4): `-ENOENT` firmware-missing vs upload-DMA `-ETIMEDOUT` vs FW-running-poll `-ETIMEDOUT`; ASIC-ready polls `MT_MAC_CSR0` (1 ms) |
| eFUSE read | valid cal + real MAC | `MT_EFUSE_CTRL` no-valid-data (clone with blank eFUSE) → the random-MAC + default-cal fallback path, logged as *taken* |
| MAC/PHY init | BBP/RF soak-loops converge | which soak-loop timed out — RF via `MT_RF_CSR_CFG` (1 ms), BBP/PHY tables (FINDINGS §1.8); log the register address |
| Scan | ≥1 real BSSID listed | reg-lockout (too few channels) vs no-AP-in-range vs RX-path-dead — distinct, so "saw nothing" is never ambiguous |

**Capture-before-you-need-it:** if a discriminator needs data the driver doesn't yet log (a firmware-load fault *code*, the failing soak register *address*), adding that capture is part of W1 — not a discovery on the failing run. The MCU/firmware fault codes to log from boot are enumerated in FINDINGS §1.4/§1.5 (the distinct `-ENOENT`/`-ETIMEDOUT`/`-EINVAL` paths).

## 4. Trap bank — VERIFIED (the mine landed; authoritative bank in the FINDINGS §2)
The trap bank is now `RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` **§2** (the 7 starter traps verified/corrected in §2.1 + the 20 raw-source traps T-001…T-020 in §2.2, each with its discriminator). The W1 dispatch carries that bank by reference; do not re-derive it. **The load-bearing corrections to our starter bank** (so they're not lost in the dispatch):
- **Firmware load (Trap 1):** chunks are `MCU_FW_URB_MAX_PAYLOAD = 0x3800` (14,336 B), **NOT** EP maxpacket; upload is FCE DMA via vendor req `0x42`, not direct bulk; the header has **no magic** — validation is `ilm_len > IVB_SIZE(64)` + total = header+ilm+dlm. Discriminators are three *distinct* error paths: `-ENOENT` (firmware missing) vs upload-DMA `-ETIMEDOUT` vs FW-running-poll `-ETIMEDOUT`.
- **⚠ eFUSE (Trap 2) — a HARD FAIL, not a soft fallback.** If the eFUSE **usage map < 5 words used**, the Linux driver returns **`-EINVAL` and aborts init** ("needs default EEPROM file") — there is **no** recovery for usage-map exhaustion (only the *MAC address* has a random fallback). **This is a hardware-procurement risk** (cheap clones may ship blank/minimal eFUSE) and a W1 design decision (§6 ISC-4): faithfully port the hard-fail, or be more lenient than Linux by shipping a **default EEPROM/cal blob** so a blank-eFUSE clone still associates. Discriminator: the "needs default EEPROM file" message vs a valid-cal path.
- **USB limits (Trap 3):** MCU in-band ≤ `INBAND_PACKET_MAX_LEN = 192`; TX ring is **not** ~32 (see FINDINGS T-008).
- **Endpoints:** **EP3 is BULK OUT, not interrupt** (corrects our §3/§5 assumption); MCU responses come back via the FCE/bulk path, not a separate interrupt EP.
- **Band (Trap 7):** MT7601U is **2.4 GHz-only** (14 channels 2412–2484 MHz) — single-band, so no wrong-initcode-table risk.
- **eFUSE read protocol:** `MT_EFUSE_CTRL` addr+mode+KICK, poll KICK→0 (10 ms), read 4× `MT_EFUSE_DATA`; all-ones `AOUT` ⇒ 0xFF fill.

## 5. The bring-up sequence — VERIFIED (authoritative reference: FINDINGS §1)
The ordered init is `FINDINGS §1` (file:line-cited from the raw `mt7601u` source), in nine steps the W1 ISCs map to directly: **§1.1** USB enumeration + endpoint discovery (EP1 OUT TX / EP2 IN RX / **EP3 BULK OUT** — corrected); **§1.2** ASIC ready poll (`MT_MAC_CSR0`, timeout **1 ms** — corrected from ASIC_CTRL); **§1.3** WLAN enable + PLL/XTAL check; **§1.4** firmware-load handshake (FCE DMA, 14,336 B chunks, vendor req `0x42`); **§1.5** MCU command interface (`INBAND_PACKET_MAX_LEN=192`, 300 ms × 5 retries); **§1.6** DMA init + MAC/BBP initvals; **§1.7** eFUSE/EEPROM read (the HARD-FAIL gate, §4); **§1.8** PHY init + calibration (RF via `MT_RF_CSR_CFG`, BBP soak-loops); **§1.9** MAC start (`MT_MAC_SYS_CTRL` ENABLE_TX/RX, WPDMA poll) + the TXWI/RXWI bulk data path (4 B TXINFO + `mt76_txwi`; RX strips 4 B DMA-hdr + 4 B RX-info + RXWI). The dispatch's ISCs cite FINDINGS §1.x for exact registers — raw-pulled literal bytes, never re-summarized.

## 6. The slice → ISCs (shape; ISC bodies finalized at dispatch once §4/§5 are folded in)
- **ISC-1 — Enumerate + claim** the vendor-class interface on the 7.5-A/B core; identify the 3 bulk EPs; register the MT7601U as a USB class driver (the 7.5-B registration seam).
- **ISC-2 — Firmware load:** parse/verify `mt7601u_fw_header`, chunk-upload to EP1, poll ready. (The §3 firmware discriminators.)
- **ISC-3 — MCU command interface:** the `MT_H2M`/`MT_M2H` request/response path over control-OUT + the interrupt EP.
- **ISC-4 — eFUSE/EEPROM read + MAC/PHY init + calibration** (FINDINGS §1.7/§1.8). **Decision point (the eFUSE HARD-FAIL, §4 Trap 2):** Linux aborts with `-EINVAL` if the usage map < 5 words. W1 must choose — port the hard-fail (and accept that a blank-eFUSE clone simply won't work, surfaced as a clear breadcrumb), or ship a **default EEPROM/cal blob** so such clones still associate (more lenient than Linux; more work; needs a sourced default-cal). Random-MAC fallback is kept regardless. Log which path is taken.
- **ISC-5 — The bulk data path + scan:** raw-802.11 RX (TXWI/RXWI handling), receive beacons/probe-responses, enumerate BSSIDs. The scan output is the gate.
- **ISC-6 — Standards:** the `wifi`/`mt7601u` feature (default-off ⇒ bit-for-bit today — the kill-switch); `unsafe` is the USB-transfer/MMIO idiom (justified, like 7.5-A/B); ANSPEC §10.x subsection + **ADR-052** ⟦confirm the next free ADR at dispatch — LO took 052; W1 likely 053⟧. I-14 §2 confirmation restated.

## 7. Gate (blind hardware)
- **Stage A (QEMU):** the MT7601U has no QEMU model — so Stage A is the **host-testable logic** (the firmware-header parse, the MCU framing, the descriptor/TXWI-RXWI layout as byte-oracles, I-12) + the family staying green with the feature off. State honestly that QEMU cannot exercise the radio.
- **Stage B (OptiPlex blind):** `lsusb` confirms `148f:7601`; the driver enumerates → loads firmware → reads eFUSE → inits MAC/PHY → **scans and lists real BSSIDs**, each stage emitting its §3 breadcrumb so a failure self-localizes in one run. This is the conclusive gate.

## 8. Kill-switch + complexity posture
`mt7601u` feature off ⇒ the driver doesn't compile ⇒ system bit-for-bit today (purely additive, like CDC-ECM). No new AI invariant; no new ordered lock (a device-state leaf only); the `unsafe` is the established USB idiom. Consolidation note: W1 is the 4th–5th feature build since BUILD-K — **a Rule-4 consolidation (INDEX refresh + ledger reconcile + the banked follow-ons) is due around W1's close.**

## 9. Open items to resolve before W1 dispatches
1. ✅ **DONE** — the verified bring-up sequence (§5) + trap bank (§4) landed (FINDINGS, 2026-06-14, on `main`).
2. **The dongle physically in the OptiPlex with `lsusb` = `148f:7601`** (W1's Stage B can't run without it) — **the sole remaining gate**. ~2 days out. **Procurement caveat (FINDINGS §4 Trap 2 + §5 honest-negatives):** a too-cheap clone with a blank/minimal eFUSE (usage map < 5 words) will **hard-fail** the Linux-faithful init — verify the ordered adapter's eFUSE is programmed, or plan the default-cal-blob mitigation (ISC-4).
3. **eFUSE hard-fail policy (ISC-4):** port-the-hard-fail vs ship-a-default-EEPROM — PL/PM decision at dispatch.
4. Confirm the next free ADR number at dispatch time (ADR-052 = LO, 053 = 8.1; W1 likely 054).
5. Decide scan-only (W1) vs scan+`NetDevice`-stub for the W1/W2 cut. (W2 MLME crib is from **Linux mac80211**, FINDINGS §3 — BSD `net80211` was unavailable, a partial clone; noted honestly.)
