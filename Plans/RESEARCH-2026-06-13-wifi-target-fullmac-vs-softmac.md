<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH LANE: the WiFi target — FullMAC-USB vs SoftMAC-iwlwifi (the easier-path question) → Prometheus

**Date:** 2026-06-13
**From:** PM/Cerebro
**To:** Prometheus (deep-research lane; cross-validated + adversarially-verified cited report; no kernel code)
**Output:** `Plans/RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac-FINDINGS.md` (committed to `main`; `[skip ci]`, Plans/ path-ignored)
**Why now:** Phase 7.5 commits to WiFi (Article 9.10 / ADR-017 names Intel `iwlwifi`). A prior-art pass found `iwlwifi` is the **maximum-difficulty** target on every axis (PCIe **SoftMAC** — the host writes the entire 802.11 MLME + a from-scratch WPA2/3 supplicant — plus a large firmware + complex host-command interface). A **FullMAC** chip moves MLME + the WPA handshake into firmware; a **USB** FullMAC dongle *additionally* rides the just-built USB stack (7.5-A/B). Before we commit ~4–5 blind builds to `iwlwifi`, settle whether a USB-FullMAC path delivers the **goal** (wireless connectivity) at a fraction of the from-scratch software — an **Article 24.1 scope decision** the PL will make from this report.
**Related:** the lane THINK (`THINK-2026-06-11-phase-7.5-opening`); 7.5-V VT-d (`THINK/DISPATCH-2026-06-13-...-vtd`) — the (c2) guard that hardware-contains a FullMAC firmware blob's DMA; ADR-017 (WiFi strategy); the CLAUDE.md trap-mining standing rule (this lane also *produces* the trap bank for the chosen target).

## The decisive question
**What WiFi target minimizes from-scratch host software while being (a) documentable enough to write a driver for, (b) DMA-containable under our I-14 posture, and (c) actually buyable?** Rank the options; recommend one; be honest about what each does and does not move.

## Questions to answer (each cross-validated, cited, adversarially checked)

### Q1 — The MLME/supplicant split per option (the load-bearing axis)
For each candidate, state **precisely** what the *firmware* does vs what the *host* must write:
- **Scan / auth / associate (MLME):** firmware or host?
- **The WPA2/WPA3 4-way handshake + EAPOL + key derivation (PBKDF2/SAE) + AES-CCMP/GCMP:** firmware or host? *(This is the crux — if firmware does the handshake given an SSID+passphrase, the from-scratch supplicant nearly vanishes; if the host must still run EAPOL, it only shrinks. Pin this for each chip — do not assume "FullMAC ⇒ no supplicant.")*
- **Regulatory/channel management:** firmware-default or host?
Candidates to cover: Intel `iwlwifi` (PCIe SoftMAC — the baseline), Atheros `ath9k` (PCIe SoftMAC, **no firmware blob** — host does everything but no closed firmware), Broadcom `brcmfmac` (FullMAC; USB/SDIO/PCIe variants), Marvell `mwifiex` (FullMAC; USB variant), Realtek USB (`rtw88`/`rtl8xxxu` — mixed; identify which are FullMAC), and any standout **USB FullMAC** dongle with a documented host protocol.

### Q2 — Host-interface documentation quality (can we actually write it from scratch?)
"FullMAC" is only easier if the **host command/event protocol is documented or cleanly reverse-engineered.** For each FullMAC candidate, assess: is the bus/command protocol (e.g. Broadcom `bcdc`/`msgbuf`, Marvell's command structs) documented in the vendor spec, the open driver source, or a clean RE writeup? How big is the host driver in Linux (LOC, as a difficulty proxy)? Firmware: openly distributable, closed-but-redistributable, or RE'd-only? Flag any that need a firmware blob we can't legally bundle (Bible Article 26 / licensing).

### Q3 — Does it ride the USB stack we built? (the compounding win)
Which candidates are **USB-attached** (ride 7.5-A/B's xHCI + USB core + the class-driver seam — possibly even reusing the CDC-ECM-style framing), vs PCIe (a new bus path)? A USB-FullMAC dongle = no new bus bring-up **and** firmware-side MLME/WPA. Confirm the USB host-interface specifics (bulk/interrupt endpoints, vendor control requests).

### Q4 — DMA containment fit (I-14(c2))
A FullMAC firmware blob is the I-14(c2) adversary. Confirm the recommended path is contained by our design: a **USB** FullMAC dongle sits behind the USB controller's VT-d fence domain (7.5-V) — the firmware's DMA is hardware-bounded to the arena. A **PCIe** FullMAC chip needs its own fence domain. State which, and that 7.5-V (sequenced first) covers it.

### Q5 — The trap bank (the CLAUDE.md trap-mining deliverable)
For the **recommended** target, pre-seed the failure-mode/discriminator bank from documented field gotchas (the equivalent of the CDC-ECM alt-1 / AX88179-vendor-only / DHCP-chaddr traps that cost us the USB day): firmware-load sequencing, the command/event ring protocol, endpoint/alt-setting traps, the credential-handoff format, regulatory lockouts, the "works-on-Linux-because-of-quirk-X" cases. Each trap → its discriminator. This bank seeds the eventual build dispatch.

### Q6 — The recommendation (Article-24.1-ready)
A clear ranked recommendation with the honest trade:
- **Option A — honor the literal commitment:** build `iwlwifi` (SoftMAC + supplicant from scratch). Cost: the heaviest arc; the full 802.11/WPA stack is ours.
- **Option B — honor the goal:** a specific USB-FullMAC dongle (name the exact chip + a buyable adapter). Cost: a firmware blob (contained by 7.5-V) + the vendor host protocol; benefit: deletes the from-scratch supplicant **and** the PCIe bring-up.
- **Option C — `ath9k`-class (no firmware blob, SoftMAC):** the "purist" path — no closed blob, but the full MLME/WPA stack is still ours.
Give the **single recommended target + a specific buyable part**, the estimated build-count delta vs `iwlwifi`, the residual risks, and — explicitly — whether choosing it requires an **Article 24.1 re-deferral/amendment** of the ADR-017 `iwlwifi` commitment (it does; frame the PL decision).

## Method / standards
Cross-validate every load-bearing claim against ≥2 independent sources (vendor spec + driver source preferred). Adversarially check the "FullMAC ⇒ no supplicant" assumption per chip (it is NOT uniformly true — Q1). Cite everything. No kernel code; the output is the decision report. **Do NOT pick a target that needs a firmware blob we can't legally redistribute without flagging the Article-26 licensing consequence.** RETURN the FINDINGS doc + a one-paragraph PL-facing executive recommendation.
