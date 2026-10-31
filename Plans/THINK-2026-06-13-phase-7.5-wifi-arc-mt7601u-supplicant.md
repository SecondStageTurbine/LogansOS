<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7.5 WiFi arc — MT7601U SoftMAC + the host-tier WPA2/3 supplicant

**Date:** 2026-06-13
**Seat:** PM/Cerebro (arc-opening design pass, grounded in the ratified research; not a dispatch)
**Context:** The connectivity lane's governance work is done (0/A/B silicon; K digest; V = I-14 fully proven). WiFi is the last big arc. The target is **MediaTek MT7601U** (Article 24.1 amendment, `PL-DECISION-2026-06-13`), chosen because the research (`RESEARCH-2026-06-13-…-FINDINGS.md`) proved **no chip eliminates the host-side WPA supplicant** — so the win is the smallest USB SoftMAC with BSD firmware, not a FullMAC fantasy.
**Related:** the FINDINGS doc (MT7601U specifics + the Q5 trap bank); ADR-017 Amendment; the CLAUDE.md trap-mining + blind-hardware + I-14 standing rules (all three apply to this arc).

## 1. What we are actually building (and NOT building)
MT7601U is **SoftMAC** — the host runs the 802.11 MAC. In Linux that's `mac80211` (a huge framework). **We are not porting mac80211.** Our goal is the Article 9.10 goal: **a station (STA) associating to one AP with WPA2/WPA3 and passing IP traffic.** That needs a *bounded* MLME, not the full 802.11 surface:
- **In scope:** STA scan (receive beacons/probe-responses, or directed probe of a known SSID), open + SAE authentication, association, the WPA2 4-way handshake / WPA3 SAE, CCMP data encryption, and the `NetDevice` glue so the Phase-4 stack runs over it (DHCP/DNS/TCP — same as USB-Ethernet).
- **Explicitly NOT in scope:** AP mode, mesh, P2P, roaming, 802.11n/ac aggregation tuning, WPA-Enterprise/802.1X, band-steering. Those are follow-ons (ADR-017's "follow-up ADRs" clause). Minimal-sufficient-mechanism: the smallest MLME that connects.

## 2. The slice — and the dongle-independence that lets us start NOW
The PL-DECISION sketched W1 (driver) / W2 (supplicant). The research sharpens that: the **supplicant's crypto is fully host-testable without a radio**, so it should be pulled out as the *first, dongle-independent* build. Refined three-way slice:

- **7.5-W0 — the WPA2/3 crypto + EAPOL library (pure host-tier; NO dongle; dispatchable now).** PBKDF2-HMAC-SHA1 (WPA2 PSK→PMK), the 802.11 PRF + 4-way-handshake key derivation (PTK/GTK), EAPOL-Key frame construct/parse, AES-CCMP encrypt/decrypt, and WPA3 **SAE** (the dragonfly handshake: hunting-and-pecking / hash-to-element, commit/confirm). **Gate = published IEEE 802.11 / RFC test vectors** (the byte-oracle discipline, I-12, applied to crypto — assert against known-answer vectors, not round-trip). This is the hardest *software* in WiFi, it's self-contained, and it composes into W2's on-air state machine when the dongle arrives. **This is the build we start while the dongle ships.**
- **7.5-W1 — the MT7601U USB driver (blind hardware; needs the dongle).** Rides 7.5-A/B: vendor-class interface (`0xff`), 3 bulk endpoints (EP1 OUT TX, EP2 IN RX, EP3 IN interrupt/events); ~44 KB initcode firmware chunk-uploaded on EP1; MCU command interface (USB vendor control-OUT, responses via the interrupt EP; the clean `MT_H2M_*`/`MT_M2H_*` enum); eFUSE/EEPROM read; MAC/PHY init; the bulk data path (raw 802.11 frame TX/RX); **scan** (we can *see* APs). Gate: enumerate + firmware-load + a scan that lists real BSSIDs on the OptiPlex.
- **7.5-W2 — the MLME + on-air supplicant.** The STA state machine (auth → assoc → 4-way/SAE → key-install via MCU cmd, firmware does data-plane CCMP) driving the W0 crypto over the air via W1; `NetDevice` glue; the Phase-4 stack lights up over WiFi. Gate: associate to a real WPA2 (then WPA3) AP and pull a DHCP lease + DNS round-trip on the OptiPlex — the wire (air) is the test, exactly like USB-Ethernet's.

## 3. I-14 fault-containment analysis (the standing rule — and it's *already satisfied*)
The research's Q4 is decisive: **USB fundamentally eliminates device-initiated DMA.** The xHCI controller mediates all memory movement; the MT7601U can only move data through transfer descriptors the driver explicitly maps — **even fully-compromised firmware cannot DMA arbitrary host memory.** So:
- **(a) lock isolation:** the W1 bulk-completion/event path takes no AI-governance lock (same posture as the USB-Ethernet driver; reuses 7.5-A/B's bounded ISR drain).
- **(b) liveness:** RX/event flood is capped per service pass (the 7.5-B `EP_QUEUE_CAP` pattern, reused).
- **(c) memory isolation:** **handled by the USB architecture itself** — the firmware blob (the adversary) is contained by xHCI's host-controlled DMA, and the xHCI controller already sits in VT-d's fence domain (7.5-V). **No new (c) vector, no new guard needed.** This is the whole reason the MT7601U/USB target was strictly superior to a PCIe `iwlwifi` (which would have needed its own VT-d domain). State it in each W dispatch; it's a one-line confirmation, not new work.

## 4. Trap bank — pre-seeded from the research (the trap-mining rule, for W1)
The FINDINGS Q5 already mined the MT7601U field gotchas; they go in W1's bank on day zero, each with its discriminator:
1. **Firmware load** — `mt7601u.bin` (~44 KB initcode); verify the `mt7601u_fw_header` magic; chunk to endpoint maxpacket. Disc: "firmware request failed" vs a generic USB error.
2. **eFUSE calibration** — cheap clones ship unprogrammed eFUSE (zero MAC, no RF cal → associates but terrible range / regulatory-non-compliant). Disc: `MT_EFUSE_CTRL` no-valid-data; fall back to initcode default cal + a random MAC.
3. **USB transfer-size limits** — MCU cmds ≤192 B in-band; firmware chunks ≤ EP maxpacket; TX ring ~32 descriptors. Disc: TX hangs with `actual_length < transfer_length` (short URB).
4. **Regulatory lockout** — no reg domain → "world" (country 00) hides channels. Disc: scan returns too few BSSIDs / channels 12-13 invisible. Set a default reg domain at boot.
5. **Deep-sleep hang** — cheap clones' wake pin isn't wired; deep sleep bricks until replug. Disc: device vanishes from the bus after suspend. **Disable deep-sleep**, use USB remote-wake.
6. **TX ring overflow under load** — ~32 descriptors; submitting faster than firmware TXes → `-EBUSY`. Disc: tx_dropped climbs. Implement stop/wake thresholds (~80%).
7. **Band-specific initcode** — (FINDINGS Trap 7) the firmware initcode is band-specific; load the right table.
Plus the blind-hardware discriminator discipline for W1's bare-metal scan run (each failure self-localizing — firmware-load vs eFUSE vs reg-lockout vs no-AP-in-range are distinct breadcrumbs).

## 5. The kill-switch + standards (per build)
`wifi`/`mt7601u` feature off ⇒ system bit-for-bit today (purely additive, like the CDC-ECM driver). The W0 crypto lib lives in the pure host-testable tier; W1's `unsafe` is the USB-transfer/MMIO idiom (justified, like 7.5-A/B); each build opens its ANSPEC §10.x subsection + an ADR (W0 → ADR-051, W1 → ADR-052, W2 → ADR-053, tentatively).

## 6. Design questions for the PL
- **Q1 — Adopt the three-way slice (W0 crypto-lib host-tier → W1 MT7601U driver+scan → W2 MLME+on-air supplicant), and dispatch W0 first (dongle-independent, critical path)?** *(Recommended — §2. Makes real progress on the hardest software while the dongle ships; W1 waits on the `148f:7601` adapter.)*
- **Q2 — Minimal STA MLME, not a mac80211 port (no AP/mesh/roaming/Enterprise; those are follow-up ADRs)?** *(Recommended — §1, minimal-sufficient.)*
- **Q3 — WPA2 first, WPA3/SAE in the same W0 crypto lib but gated on its own RFC vectors (SAE is the harder crypto; build it host-tested before it's ever on-air)?** *(Recommended — de-risks SAE entirely in the pure tier.)*
- **Q4 — Confirm I-14(c) needs no new guard for this arc (USB architecture + the 7.5-V xHCI fence already contain the firmware), recorded as a one-line confirmation per W dispatch?** *(Recommended — §3.)*

## 7. If ratified — the first dispatch
**BUILD-7.5-W0 (the WPA2/3 crypto + EAPOL library):** PBKDF2/PRF/PTK-GTK, EAPOL-Key construct/parse, AES-CCMP, SAE — pure host-tier, asserted against IEEE/RFC known-answer vectors (the I-12 byte-oracle discipline applied to crypto). No `unsafe`, no hardware, no kernel glue yet (it's a library the W2 state machine will call). `[skip ci]`-adjacent (host tests only). Opens ANSPEC §10.9 (the WiFi security/supplicant model) + ADR-051. This is the dongle-independent critical-path build; W1 (the driver, blind hardware) follows when the adapter is in hand.
