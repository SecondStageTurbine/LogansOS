# ADR-017: WiFi support strategy and phase-binding

**Status:** Accepted (2026-05-10) · **Amended 2026-06-13 (Article 24.1)** — the chipset target is changed from Intel `iwlwifi` to **MediaTek MT7601U (`mt7601u`)**; see the Amendment block below, `Plans/PL-DECISION-2026-06-13-wifi-target-mt7601u.md`, and `Plans/RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac-FINDINGS.md`.
**Read first:** the live decision is the **Article 24.1 / Bible Article 9.10 amendment** (`iwlwifi` → MT7601U). Read the Amendment block before the original D1/D3/D5 text, which is retained only as superseded history.
**Supersedes:** None
**Superseded by:** None
**Related:** ADR-013 (Phase 2 SMP retrofit; WiFi depends on SMP-correct substrate). Bible Articles 9.10, 13, 17, 28.
**Decision background:** `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md` (full options analysis with cost estimates, preserved for historical reference).

---

## Amendment (2026-06-13, Article 24.1) — chipset target: iwlwifi → MT7601U

A Phase-7.5 deep-research lane (`RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac-FINDINGS.md`, six candidates, cross-validated) overturned the premise behind D1. **D1's "Intel iwlwifi only" target is replaced by MediaTek MT7601U (`mt7601u`).** What changed:

- **The deciding finding:** "FullMAC offloads the supplicant" is **false for every candidate** — FullMAC firmware handles only the MLME (auth/assoc) state machine; the **WPA handshake, EAPOL, and CCMP key management remain host-side regardless** (the one exception, brcmfmac FWSUP, is SDIO-embedded, not USB). So no WiFi target avoids a host-side supplicant — which collapses iwlwifi's only would-be peer and reduces the choice to *driver size, firmware licensing, bus, and DMA containment*.
- **MT7601U wins every remaining axis:** USB 2.0 **SoftMAC**, ~5,500 LOC (~40% of iwlwifi's surface); **BSD-3-Clause firmware** (freely redistributable — strictly cleaner than D4/D5's "iwlwifi redistributable-but-not-modifiable" posture, and unaffected by the ADR-026 license change that post-dates this ADR); **rides the already-built Phase-7.5-A/B USB stack** (zero new bus bring-up); **inherently DMA-contained by the USB path** (the xHCI controller's 7.5-0 arena fence, and VT-d's USB fence domain later) — it introduces no new PCIe DMA master. Buyable generic dongle, `148f:7601`, ~$5–8 (verify with `lsusb`; avoid the mislabeled TL-WN725N v1 / EW-7811Un).
- **Consequence — the commitment shrinks dramatically.** D2/D5's "~2–3 engineer-year iwlwifi deliverable" was sized for the maximum-difficulty path. MT7601U is a much smaller build: a USB SoftMAC driver (rides 7.5-A/B) + the host-tier WPA2/3 supplicant. The supplicant (EAPOL/SAE/key-derivation/CCMP) is the real remaining work and belongs in the pure host-testable tier against RFC vectors. The phase-binding (D2: Phase 7.5) and the firmware-blob discipline (D4, now `boot/firmware/mediatek/mt7601u/`) stand; the *chipset and the cost estimate* are what this amendment revises.
- **Identity note:** MT7601U being SoftMAC means LogansOS *does* run the 802.11 MLME in-kernel — the "AI-native wireless" shape — but the choice is made on minimal-sufficient-mechanism grounds (it is the *smallest* path once FullMAC's supplicant-elimination proved illusory), not for purity. SoftMAC additionally keeps the airtime-as-governed-resource door open natively.
- **D1's deferral inverts:** "Realtek/Qualcomm/MediaTek deferred to follow-up ADRs" is superseded for MediaTek (now the primary target); other families remain follow-on.

---

## Context

Bible v1.1.0's Article 9.10 placed WiFi at "Phase 8+ at earliest. Requires substantial driver work (firmware blob handling, full 802.11 stack); ADR if pursued." Bible v1.1.0's Article 28 (Product Accessibility) committed NeoPlutarch to being usable by real people on hardware they already own. Most consumer laptops sold since ~2015 ship without Ethernet ports; "real people on common hardware" without WiFi support is a partial accessibility goal at best.

The project lead identified WiFi as a must-have for the accessibility commitment to be credible. This ADR responds to Article 9.10's "ADR if pursued" trigger, scopes the work, and binds it to a concrete phase.

---

## Decision

**⚠ AMENDMENT FIRST:** This ADR was amended 2026-06-13 (Article 24.1). The original D1 target (Intel iwlwifi) is **superseded** by MediaTek MT7601U. Read the Amendment block above before the decisions below — D1, D3, and D5 are revised by that amendment.

The following options from the decision-background proposal are accepted:

### D1 — Initial chipset family: Intel iwlwifi only ~~(superseded by MT7601U per Amendment)~~

The first WiFi deliverable targets the Intel iwlwifi chipset family (AX200, AX201, AX210, AX211, BE200, etc.), covering approximately 60–70% of x86_64 laptops sold since ~2018. Adding additional chipset families (Realtek, Qualcomm, MediaTek) is deferred to follow-up ADRs after the iwlwifi deliverable proves out the discipline.

Rationale: Intel iwlwifi has the best documentation and most predictable firmware-blob handling; single-family scope keeps the initial deliverable bounded; the ~60–70% coverage hits the largest plausible user base for a single-family investment. Multi-family expansion is naturally additive, not architecturally different.

### D2 — Phase-binding: Phase 7.5

WiFi is added to Article 13's phase ledger as **Phase 7.5 — WiFi support: Intel iwlwifi family, WPA2/WPA3 client mode, hardware-default regulatory, kernel-side connection management surface (userspace tooling deferred to Phase 8+).** Phase 7.5 sits between Phase 7 (inference scheduler, multi-tenant model serving) and Phase 8+ (userland).

Rationale: Phases 5–7 are the AI-native phases that justify the project's existence per Article 13's framing. Pulling WiFi forward of those phases would delay the project's core thesis by 2–3 years for a single subsystem, which fails the Article 1.4 "no parity-chasing" test indirectly (by trading thesis-delivery time for hardware-coverage broadening). Phase 7.5 places WiFi after the AI-native deliverable lands but before the userland phase that needs WiFi most. Stepping stones (D3) bridge the gap for users who need networking before Phase 7.5 lands.

This is a real multi-year commitment, not a placeholder. PM-estimated cost is 2–3 focused engineer-years for the iwlwifi deliverable alone.

### D3 — Stepping stones: all three accepted

The following Phase 4 additions are accepted:

- **D3-A:** USB Ethernet adapter support added as a Phase 4 deliverable. Targets generic CDC-ECM and CDC-NCM class drivers (covers most cheap USB-Ethernet dongles plus phone tethering). Estimated ~1–2 engineer-months on top of the existing Phase 4 USB and network-stack work.
- **D3-B:** USB tethering from phone shares the same CDC-NCM/RNDIS plumbing as D3-A; landing D3-A effectively delivers D3-B.
- **D3-C:** NeoPlutarch distribution documentation includes a "wired-network install posture" — the supported install path for users without WiFi-capable NeoPlutarch is to install via wired Ethernet (built-in port on desktops; USB Ethernet dongle on laptops; phone tether as fallback). Documentation-only deliverable; no engineering cost.

Rationale: USB Ethernet support is cheap on top of the Phase 4 USB+network work, and bridges the years between Phase 4 (network stack lands) and Phase 7.5 (WiFi lands) so accessibility doesn't have to wait. Stepping stones are universal — they don't conflict with the WiFi work or with anything else.

### D4 — Firmware blob discipline: bundled, license-preserved

Vendor firmware files are bundled with NeoPlutarch distributions in `boot/firmware/<vendor>/<chipset>/<file>`, with vendor licenses preserved alongside each firmware blob. Distribution README enumerates included firmware and lists their vendor licenses.

Concretely:

- **File-tree location:** `boot/firmware/intel/iwlwifi/<chipset-firmware-file>` for Intel; vendor name and chipset family directory structure for any future additions.
- **Loading mechanism:** Limine boot modules; the kernel locates firmware via a defined path convention. Specific load mechanism design deferred to a follow-up ADR at Phase 7.5 implementation time.
- **License posture:** NeoPlutarch project-authored code remains MIT OR Apache-2.0 per Article 26. Vendor firmware retains its own license (Intel iwlwifi firmware is redistributable but not modifiable — acceptable for redistribution as a separate licensed component).
- **Non-redistributable firmware:** Out of scope for this ADR. If a future chipset family requires non-redistributable firmware, that's a separate ADR with separate license discipline.
- **Update mechanism:** Vendor firmware updates ship as part of NeoPlutarch distribution releases; users do not separately update firmware.

Rationale: bundled firmware delivers a working out-of-the-box experience; the license-preserved posture is honest about what's project-authored and what's vendor-supplied; redistributable-only scope keeps Article 26's clean MIT OR Apache-2.0 posture intact for project-authored code.

### D5 — Bible amendment scope: Article 9.10 only

Bible v1.2.0 amendment scope:

- **Article 9.10:** revised to commit WiFi to Phase 7.5 with Intel iwlwifi family, replacing the current "Phase 8+ at earliest, ADR if pursued" framing.
- **Article 13:** phase ledger updated to add Phase 7.5 between Phase 7 and Phase 8+.
- **Article 17:** source layout updated to include `boot/firmware/<vendor>/<chipset>/` as a recognized location for vendor firmware blobs.
- **Article 26:** NOT amended in this ADR. The current language ("Third-party code retains its own license") is interpreted to cover bundled vendor firmware. If the project later acquires multiple firmware-blob categories or non-redistributable firmware, a separate amendment may be appropriate.

Rationale: minimum-scope amendment that captures the load-bearing decisions; Article 26's existing third-party-license language is sufficient for the bundled-redistributable-firmware case.

---

## Consequences

### What this ADR commits NeoPlutarch to

- Phase 4 grows by approximately 1–2 engineer-months for the USB Ethernet stepping stones.
- Phase 7.5 is a new ~2–3 engineer-year phase.
- The accessibility goal in Article 28 has a credible delivery path for laptop users.
- Vendor firmware bundling becomes part of the NeoPlutarch distribution discipline.
- Future chipset-family additions (Realtek, Qualcomm, MediaTek) are scoped as follow-up ADRs, not as part of Phase 7.5.

### What this ADR explicitly does not commit NeoPlutarch to

- WiFi power management (suspend/resume) — gated on a future PM subsystem; not in any phase plan.
- Bluetooth support — separate stack; not addressed in this ADR.
- WiFi AP mode, mesh, or IBSS — client-mode only.
- WPA Enterprise (802.1X / EAP) — corporate/enterprise authentication is a follow-up.
- Driver coverage parity with Linux — the goal is "common modern laptops with Intel WiFi work," not "every device that has ever existed."
- Userspace WiFi management GUI — kernel exposes the surface; full graphical connection management is a Phase 8+ userland concern.

### Downstream artifacts

- **Bible v1.2.0 amendment** — landing concurrently with this ADR's acceptance.
- **Phase 4 plan** — when Phase 4 opens, its plan document includes USB Ethernet stepping stones as deliverables alongside the network stack work.
- **Phase 7.5 plan** — drafted when Phase 7.5 opens, references this ADR as scoping authority.
- **Future ADRs** as the WiFi work matures: specific design ADRs for the 802.11 protocol-stack architecture, the WPA crypto layer, the iwlwifi firmware-load mechanism, the regulatory-domain approach, and any second-chipset additions.

### Implementation discipline

The WiFi work obeys all standard project disciplines:

- **Bible Article 1.3** (every commit ships) — Phase 7.5's commits are real ship-quality commits, not experimental scaffolding.
- **Bible Article 8.5** (critical-section discipline) — WiFi RX handlers run in interrupt context and obey 8.5.1–8.5.4. Frame buffers use pre-allocated pools per 8.5.2.
- **Bible Article 12.1** (capability-based security) — WiFi network access is capability-gated; processes don't get ambient network access via WiFi.
- **Bible Article 15.2** (SAFETY comments) — WiFi-related unsafe blocks (DMA, MMIO, firmware loading) carry SAFETY comments.
- **Bible Article 18.1** (ADRs for second-guessable choices) — major sub-decisions inside the WiFi work get their own ADRs.

---

## References

- Bible v1.2.0 (concurrent amendment landing this ADR's phase-binding).
- Bible Article 9.10 (revised by this ADR's acceptance).
- Bible Article 13 (phase ledger updated to include Phase 7.5).
- Bible Article 17 (source layout updated to include `boot/firmware/<vendor>/<chipset>/`).
- Bible Article 28 (Product Accessibility — the principle this ADR helps deliver).
- Bible Article 26 (license — existing third-party-license language covers bundled vendor firmware; no amendment needed).
- ADR-013 (Phase 2 SMP retrofit — WiFi RX/TX paths assume SMP-correct substrate).
- `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md` — full options analysis with cost estimates, preserved for historical reference.
- Linux mac80211 / cfg80211 / nl80211 architecture (prior art for in-kernel 802.11 stack).
- Linux iwlwifi driver (prior art for Intel chipset support).
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` — Asterinas framekernel-architecture discipline relevant to WiFi driver code's unsafe-boundary handling.

---

**End of ADR.**
