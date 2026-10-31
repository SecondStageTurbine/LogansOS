# ADR-XXX: WiFi support strategy and phase-binding

**Status:** Proposed. Pending project-lead ratification. ADR number to be assigned at ratification (next available in `docs/ADR-NNN-*.md` sequence).
**Date:** 2026-05-10
**Author:** PM (drafting)
**Bible articles affected on ratification:** Article 9.10 (will be amended to reflect new phase-binding); Article 11 (may need clarification on WiFi-stack relationship to the in-kernel network stack); Article 26 (may need a clause about third-party firmware blob licensing).

---

## Context

The bible at v1.1.0 includes Article 28 (Product Accessibility), committing AltimitNeo to being usable by real people on hardware they already own. The amendment landed on 2026-05-10 honestly bounds Article 9.10 with "WiFi — out of scope until Phase 8+ at earliest. Requires substantial driver work (firmware blob handling, full 802.11 stack); ADR if pursued."

The project lead has identified WiFi support as a must-have for the accessibility goal to be credible. Most consumer laptops sold since ~2015 have shipped without Ethernet ports; "real people on common hardware" without WiFi support is a partial accessibility goal at best. This ADR responds to Article 9.10's "ADR if pursued" trigger.

This ADR does NOT yet ratify a delivery date. It proposes a scope, target hardware, firmware-blob discipline, and a phase-binding option set. The project lead picks; Article 9.10 then gets a follow-up bible amendment to reflect the chosen phase.

---

## Decision context: what WiFi support actually entails

A credibly-shippable WiFi stack requires the following components, each with realistic cost estimates. Numbers are PM estimates based on Linux/FreeBSD/Asterinas precedent and assume a focused minimal scope, not feature-parity with mainline Linux.

### Component 1 — 802.11 protocol stack (client-mode only)

Frames, association/deauth, beaconing, power-save, basic QoS. Targets WPA2 (CCMP) and WPA3 (GCMP-256, SAE) only — no WEP, no WPA1, no AP mode, no mesh, no IBSS.

- Estimated LOC: 20,000–50,000 of careful Rust.
- People-time: ~9–18 months for one focused engineer.
- Risk: state-machine complexity. The 802.11 spec is large; a correct minimal implementation is harder than the LOC suggests.

### Component 2 — Crypto + WPA supplicant

AES-CCMP, GCMP-256, SAE handshake, 4-way handshake. Article 11.3 already commits to in-kernel crypto being "audited and minimal."

- Estimated LOC: 5,000–10,000.
- People-time: ~3–6 months.
- Risk: SAE (the WPA3 dragonfly handshake) has non-trivial constant-time discipline requirements. Side-channel resistance is real engineering work.

### Component 3 — Chipset driver(s)

Realistic options for initial chipset target, in priority order:

- **Intel iwlwifi family** — AX200, AX201, AX210, AX211, BE200, etc. Coverage: ~60–70% of x86_64 laptops sold since ~2018. Requires Intel firmware blobs. Linux driver is ~150,000 LOC; a focused port targeting AX2xx generation is smaller (~30,000–60,000 LOC) but still substantial. Best documented and most mainline-supported family.
- **Qualcomm Atheros (ath10k/ath11k)** — common in mid-range Asus, Acer, Lenovo. Firmware-loading required. Driver complexity comparable to iwlwifi.
- **Realtek** — common in cheap hardware. Many chipset variants (RTL8821CE, RTL8822CE, RTL8852AE, etc.). Driver quality is mixed in the broader FOSS ecosystem.
- **Broadcom** — Apple hardware, some Dell/HP. Historically hostile to FOSS drivers; reverse-engineered support varies. Out of scope for the initial deliverable.
- **MediaTek (mt76)** — increasingly common. Reasonable mainline Linux support.

PM lean: **Intel iwlwifi as the initial target**, single-chipset deliverable. Intel coverage alone hits the "I can boot AltimitNeo on my Thinkpad/XPS/Framework/etc." case for the largest plausible user base. Adding a second family is a follow-up after the iwlwifi deliverable proves out the discipline.

Per-driver-family people-time: ~12–24 months.

### Component 4 — Firmware blob handling discipline

Vendor firmware files loaded into chips at initialization. They retain vendor licenses (Intel firmware is redistributable but not modifiable; not compatible with MIT OR Apache-2.0 per Article 26).

Decisions required:

- **File tree location.** Proposal: `boot/firmware/<vendor>/<chipset>/<file>`. Loaded by Limine as boot modules or located by the kernel via a known path convention.
- **License posture.** Proposal: project-authored code remains MIT OR Apache-2.0 per Article 26; vendor firmware retains its own license; license file shipped alongside each firmware blob; AltimitNeo distribution README enumerates included firmware and lists their vendor licenses.
- **Non-redistributable firmware.** Some chipsets require firmware that cannot be redistributed by AltimitNeo. Proposal: AltimitNeo ships with redistributable firmware only (Intel iwlwifi qualifies); non-redistributable firmware is the user's responsibility to obtain and place.
- **Update mechanism.** Vendor firmware periodically gets security updates. Proposal: firmware updates are part of AltimitNeo distribution releases; users do not separately update firmware.

This is the area where Article 26 may need a small amendment to acknowledge third-party firmware as a distinct license category.

### Component 5 — Power management interaction

Modern laptops expect WiFi to suspend/resume with the rest of the system. AltimitNeo doesn't yet have a power-management subsystem. WiFi PM is gated on PM landing.

PM lean: WiFi initial deliverable can ship without sleep/wake support — the chip stays powered while the laptop is on, idle when not actively transmitting. Suspend/resume is a follow-up after the broader PM subsystem lands (not in any current phase plan).

### Component 6 — Regulatory domain handling

Allowed frequencies differ by country (channels 12-14 in 2.4GHz, U-NII bands in 5GHz, channels in 6GHz, etc.). Linux uses CRDA + a database. Options:

- **Hardware-default regulatory.** Trust the chip's factory regulatory domain. Limits user control but is technically acceptable; many chips ship with reasonable defaults.
- **Static compiled-in regulatory database.** Ship a small database in the kernel; user selects country at install or first boot.
- **Userspace regulatory daemon.** Out of scope for initial deliverable.

PM lean: hardware-default for the initial deliverable, with a defined extension point for static database in a follow-up.

### Component 7 — Userspace connection management

A wpa_supplicant-equivalent and an iwctl-like CLI. The kernel exposes the lower-level mechanics; userspace handles "scan, select SSID, prompt password, connect."

PM lean: this is Phase 8+ userland work. The kernel's syscall interface to userspace WiFi management is a Phase 5+ concern (it lands when WiFi lands). Initial deliverable can ship with a minimal hardcoded-config option for development and a basic interactive CLI; full graphical connection management is post-Phase-8.

---

## Total honest cost

Single-chipset iwlwifi deliverable, client-mode only, no PM, hardware-default regulatory, basic userspace CLI:

- **2–3 focused engineer-years** of work.
- **40,000–80,000 LOC** of new project-authored code.
- **Bundled vendor firmware** with appropriate license disclosure.
- **A single Bible amendment** to Article 9.10 (phase-binding) and possibly Article 26 (firmware licensing).

Adding a second chipset family (Realtek, Qualcomm, or MediaTek) is roughly **+1–2 engineer-years** on top.

Reaching "any modern consumer laptop has working WiFi" — i.e., Intel + Qualcomm + Realtek + MediaTek coverage — is a **5–7 engineer-year program** in total.

This is a real cost. It competes directly with Phases 5-7 (the AI-native phases) for engineering attention. Whether to pursue it now, later, or in parallel is the load-bearing question this ADR proposes the project lead answer.

---

## Decision points

The project lead picks one option in each section. PM recommendations are noted as "PM lean" but are advisory.

### D1 — Initial chipset family

- **D1-A:** Intel iwlwifi only. Single family; ~60-70% of modern x86_64 laptop coverage. (PM lean.)
- **D1-B:** Intel iwlwifi + Realtek. Two families; ~80-85% coverage. Doubles cost.
- **D1-C:** Intel iwlwifi + Qualcomm + Realtek. Three families; ~90-95% coverage. Triples cost.

### D2 — Phase-binding

- **D2-A:** WiFi as Phase 4.5 (between network stack and AI subsystem). Pulls WiFi forward of the AI-native phases. *Cost: delays Phases 5-7 by 2-3 years; AI-native thesis ships later. (Not recommended unless WiFi accessibility is judged more critical than AI-native delivery timing.)*
- **D2-B:** WiFi as a parallel-track sub-phase, kicked off after Phase 4 lands but pursued in parallel with Phases 5-7 if engineering bandwidth allows. *Cost: requires additional engineering capacity beyond the core path; if no second engineer, this collapses to D2-C.*
- **D2-C:** WiFi as Phase 7.5 or 8. After AI-native phases land. *Cost: accessibility goal in Article 28 is partial for laptop users until ~3-5 years out; stepping stones (D3) bridge the gap. (PM lean if engineering capacity is one engineer.)*
- **D2-D:** WiFi remains "Phase 8+ at earliest" per current Article 9.10. *Cost: status quo; ADR records the discussion but no commitment changes. (Not recommended given user statement that WiFi is a must.)*

### D3 — Stepping stones (cumulative — pick all that apply)

- **D3-A:** USB Ethernet adapter support in Phase 4. Cheap dongle gives wired networking on any laptop. *Cost: ~1-2 engineer-months in Phase 4. (PM lean — strongly recommended regardless of D2 choice.)*
- **D3-B:** USB tethering from phone in Phase 4 (uses same USB Ethernet plumbing). *Cost: minimal additional work given D3-A. (PM lean.)*
- **D3-C:** A "WiFi-via-router-Ethernet" install posture documented in user docs — i.e., AltimitNeo expects a wired network for installation; you set up WiFi after install. *Cost: documentation only; no engineering. (PM lean — universal stepping stone.)*

### D4 — Firmware blob discipline

- **D4-A:** Bundled redistributable firmware in `boot/firmware/<vendor>/<chipset>/`, with vendor licenses preserved and disclosure in distribution README. (PM lean.)
- **D4-B:** Firmware loaded from external source (user's existing Linux install, manual download). *Cost: degraded user experience; not recommended.*
- **D4-C:** Reverse-engineered firmware-free drivers only. *Cost: limits chipset coverage drastically; iwlwifi requires firmware. (Not recommended.)*

### D5 — Bible amendment scope on ratification

- **D5-A:** Amend Article 9.10 only (phase-binding update). (PM lean if D4-A is chosen — Article 26 is permissive enough on third-party content.)
- **D5-B:** Amend Article 9.10 + Article 26 to add a third-party-firmware clause clarifying that vendor firmware retains its own license and is distributed alongside but not as part of AltimitNeo project-authored code. *Cost: one extra amendment; PM lean if firmware discipline becomes a recurring concern beyond WiFi (e.g., GPU firmware, NVMe firmware, etc. — most of which AltimitNeo won't ship).*

---

## PM-recommended package

If the project lead wants a single recommended path:

- **D1-A:** Intel iwlwifi only as initial chipset family.
- **D2-C:** WiFi as Phase 7.5 (after AI-native phases). Realistic if engineering capacity is one engineer; preserves the AI-native thesis as the project's spine.
- **D3-A + D3-B + D3-C:** All three stepping stones. USB Ethernet + tethering + documented install-via-wired-network posture. These bridge the gap between Phase 4 (network stack) and WiFi (Phase 7.5) so accessibility doesn't have to wait years.
- **D4-A:** Bundled firmware in `boot/firmware/`, vendor licenses preserved.
- **D5-A:** Article 9.10 amendment only.

This package is honest about cost (multi-year), preserves the AI-native phases as the spine of the project, gives real users a working laptop networking story via stepping stones in Phase 4 (much sooner), and delivers full WiFi as a planned post-AI-phases deliverable rather than a vaporware "someday."

If the project lead wants WiFi sooner — e.g., D2-A or D2-B — that's a legitimate choice but means either delaying the AI-native thesis or finding additional engineering capacity. Either is a real trade-off worth naming.

---

## Consequences

### If recommended package ratified:

- **Article 9.10** amended to reflect Phase 7.5 (or chosen phase) for WiFi, replacing the current "Phase 8+ at earliest, ADR if pursued" with a concrete commitment.
- **Phase 4 plan** adds USB Ethernet adapter support and USB tethering as deliverables.
- **Phase 7.5** added to Article 13's phase ledger as "WiFi support — Intel iwlwifi family, WPA2/WPA3 client mode."
- **`boot/firmware/` directory** structure added to Article 17's source layout.
- **Distribution README** template includes a third-party firmware enumeration section.
- **No change to AI-native Phase 5-7 timeline.**

### If WiFi pulled forward (D2-A or D2-B chosen):

- **Phase 4.5** added to Article 13's phase ledger.
- **Phases 5-7 timeline** revised to account for the WiFi work between them and Phase 4.
- **All other consequences from "recommended package" still apply.**

### If status quo (D2-D):

- **No changes.** ADR is filed for record. Future ADR may revisit.

---

## Implementation discipline

Whichever phase-binding is chosen, the implementation must respect:

- **Bible Article 1.3 (every commit ships).** WiFi work goes through the same per-phase + verification log discipline as every other phase. No "WiFi by Phase X" commitment is made unless the engineering plan supports it.
- **Bible Article 8.5 (critical-section discipline).** WiFi handlers run in interrupt context (frame RX); they obey 8.5.1-8.5.4 like every other ISR.
- **Bible Article 12.1 (capability-based security).** WiFi network access is a capability; processes don't get ambient network access via WiFi any more than via Ethernet.
- **Bible Article 18.1 (ADRs).** Major sub-decisions inside the WiFi work (specific chipset variant choice, supplicant API design, regulatory database approach) get their own ADRs as they come up.

---

## References

- Bible Article 9.10 (current "WiFi out of scope until Phase 8+, ADR if pursued" framing — the trigger for this ADR).
- Bible Article 28 (Product Accessibility — the principle this ADR helps deliver).
- Bible Article 26 (license — affected by D4-A's third-party firmware discipline).
- `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md` — the amendment proposal that landed Articles 9.5-9.10 + 28.
- Linux mac80211 / cfg80211 / nl80211 architecture (prior art for in-kernel 802.11 stack).
- Linux iwlwifi driver (prior art for Intel chipset support).
- FreeBSD wpa_supplicant (prior art for WPA crypto handling).
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` — Janus first-pass; mentions Asterinas's framekernel architecture and the discipline of confining unsafe Rust to a small audited core; relevant for WiFi driver code's unsafe-boundary handling.

---

## Project-lead decision required

For each of D1, D2, D3, D4, D5 above, project lead picks one option (or "PM-recommended package" as a shortcut). On ratification:

1. ADR gets a number, lands at `docs/ADR-NNN-wifi-support-strategy.md`.
2. Article 9.10 amendment gets drafted (small follow-up amendment) with the chosen phase-binding.
3. If D5-B chosen, Article 26 amendment also drafted.
4. Phase plans for affected phases (Phase 4 stepping stones; chosen WiFi phase) get updated.

---

**End of ADR.**
