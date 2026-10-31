<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PL-DECISION: Phase 7.5 WiFi target — replace iwlwifi with MediaTek MT7601U (Article 24.1)

**Date:** 2026-06-13
**From:** PL (decision recorded by PM/Cerebro)
**Type:** Article 24.1 amendment of a committed deliverable (ADR-017 D1 chipset target)
**Status:** DECIDED
**References:**
- `docs/ADR-017-wifi-support-strategy.md` (Amended 2026-06-13 — the authoritative amendment)
- `Plans/RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac-FINDINGS.md` (the cross-validated research)
- Bible Article 9.10 (WiFi commitment), Article 13 (Phase 7.5 ledger)

## Context
ADR-017 (D1) committed the Phase-7.5 WiFi deliverable to the **Intel `iwlwifi`** family. A PM-dispatched deep-research lane (Prometheus, six candidates, cross-validated) tested the load-bearing assumption that a FullMAC chip would eliminate the from-scratch WPA supplicant. **It does not:** FullMAC firmware offloads only the 802.11 MLME (auth/assoc) state machine; the WPA handshake / EAPOL / CCMP key management remain host-side in every candidate (the lone exception, brcmfmac FWSUP, is SDIO-embedded, not USB). With the supplicant unavoidable on any target, the choice reduces to **driver size, firmware licensing, bus, and DMA containment** — and on every one of those, `iwlwifi` is the maximum-difficulty option (80K+ LOC, per-generation closed blobs, PCIe bring-up).

## Decision
**The Phase-7.5 WiFi chipset target is changed from Intel `iwlwifi` to MediaTek MT7601U (`mt7601u`)**, per Article 24.1 (amendment of a committed deliverable requires explicit PL decision).

- **MT7601U** — USB 2.0 **SoftMAC**, ~5,500 LOC driver (~40% of iwlwifi); **BSD-3-Clause firmware** (freely redistributable); rides the already-built 7.5-A/B USB stack (no new bus bring-up); USB-DMA-contained (no new PCIe master). Buyable generic dongle `148f:7601` (~$5–8; verify with `lsusb`; avoid the mislabeled TL-WN725N v1 / EW-7811Un).
- **Article 9.10's goal is unchanged** — wireless connectivity for the accessibility commitment. Only the *means* changes, to the strictly-superior target on cost, risk, licensing, and security containment.

## Rationale
- **No supplicant savings anywhere** ⇒ pick the smallest, cleanest-licensed, USB-attached chip. MT7601U is it.
- **BSD-3-Clause firmware** removes the redistribution friction `iwlwifi`'s "redistributable-but-not-modifiable" blobs carried (and is unaffected by the ADR-026 license change that post-dates ADR-017's D5).
- **USB-attached** ⇒ the WiFi firmware's DMA is already contained by the xHCI path (7.5-0 arena; VT-d's USB fence later) — so **WiFi no longer hard-blocks on 7.5-V (VT-d)**; the VT-d-before-iwlwifi sequencing rationale (a PCIe blob with its own DMA) does not apply to a USB dongle. VT-d still earns its place as the I-14(c2) build; it is simply no longer a prerequisite for the radio.
- **Cost:** the deliverable shrinks from D2/D5's "~2–3 engineer-years" to a USB SoftMAC driver (rides 7.5-A/B) + a host-tier WPA2/3 supplicant — the supplicant being the real remaining work, to be built in the pure host-testable tier against RFC test vectors.

## Consequences / forward
- ADR-017 amended (chipset + cost estimate); D2 phase-binding (Phase 7.5) and D4 firmware discipline (now `boot/firmware/mediatek/mt7601u/`) stand.
- Bible Article 9.10 + the Article 13 Phase-7.5 ledger line update `iwlwifi` → MT7601U.
- The WiFi build re-slices: **7.5-W1** (MT7601U USB SoftMAC driver bring-up — enumeration on the USB stack, firmware load, 802.11 data path + MLME scan/auth/assoc; blind hardware) → **7.5-W2** (the WPA2/3 supplicant — EAPOL/SAE/key-derivation/CCMP in the host-testable tier, then on-air). Each carries the trap-mining + (for W1) blind-hardware discriminator + I-14 fault-containment sections.
- LogansOS will run the 802.11 MLME in-kernel (SoftMAC) — chosen on minimal-sufficient-mechanism grounds, and it keeps airtime-as-governed-resource open natively.

## De-escalation rule
Any further change of the WiFi target requires a fresh Article 24.1 decision. MT7601U is the committed target unless re-amended.
