<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (RESEARCH / TRAP-MINE): Phase-7.5 closeout — prior-art trap-mine the remaining WiFi pieces → Prometheus

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Prometheus (the strongest research seat; this is squarely its scoped lane — research / prior-art / trap-mining, host-only, NO QEMU/kernel build). New branch off `origin/main` HEAD: `prometheus/phase-7.5-closeout-wifi-trap-mine`.
**Status:** DISPATCHED. One of **four parallel closeout-research lanes**; the PM synthesizes all four THINKs into the Phase-7.5-closeout PLAN. **No code, no QEMU, no gate — a prior-art findings doc.**

## Why this lane
Closing 7.5 needs the *documented field gotchas* for the three remaining MT7601U/WiFi pieces mined from prior art **before** any closeout build boots — the cheapest iteration is the one prior art already paid for (the standing trap-mining rule; it is exactly what saved a day on BUILD-7.5-B and seeded the W1 27-trap bank). Mine the **exact** chip/protocol, from the authoritative sources, and **pre-seed each trap with its discriminator**. The targets:

### Target 1 — the ch6+ bulk-pipe / channel-switch reset (feeds Cerebro's OT-27(d))
From the upstream Linux `mt7601u` driver (and `mt76` where it shares code) + the kernel mailing lists / OpenWrt forums, mine the **exact** sequence for: `set_channel` under sustained command load, the **MCU-response-buffer persistence pattern** (the "persistent MCU response buffer" the W1 ADR already flagged), and **bulk-endpoint reset / recovery** after an `inband bulk-out timeout`. Cerebro owns the *fix design*; you supply the **literal documented byte/register sequences + the known traps** so his discriminator bank is grounded in source, not first principles.

### Target 2 — GTK / group-key extraction (broadcast RX) (OT-27(b))
Pairwise CCMP works; the **group key (GTK)** for broadcast/multicast RX is next. Mine: the GTK delivery in the 4-way handshake message 3 / group-key handshake, the WCID/key-table slot the MT7601U uses for the group key vs pairwise, and the RX path that must accept broadcast frames decrypted under the GTK. Cross-check the byte layout against an **independent authoritative source** (the byte-oracle acquisition rule — pull the raw IWD/`wpa_supplicant`/Linux source, never a summarizing fetch for exact bytes).

### Target 3 — real DHCP-over-`wlan0` (OT-27(b))
The Phase-4 stack already did DHCP over USB-Ethernet (BUILD-7.5-B); mine what's *different* over the WiFi `NetDevice`: the `chaddr`/broadcast-flag traps we already hit on ECM (do they recur?), frame-injection over the encrypted link (DHCP runs *after* the GTK lands), and any MTU/encryption-overhead gotcha for the WiFi data path.

## References
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md   # the existing 27-trap bank + format to extend (Channel switching section)
git show origin/main:docs/ADR-054-phase-7.5-mt7601u-driver.md                          # the ch6 wedge characterization + the "persistent-MCU-response-buffer" flag
git show origin/main:docs/ADR-051-w0-supplicant-crypto.md                              # W0 SAE/crypto + the byte-oracle acquisition discipline (raw source, never WebFetch for exact bytes)
git show origin/main:docs/ADR-057-kernel-csprng.md                                     # W2 4-way/CCMP context (where the GTK arrives)
git show origin/main:docs/OPEN-THREADS.md                                              # OT-27(b)(c)(d)
# Upstream prior art (pull RAW source — do NOT WebFetch exact bytes): Linux drivers/net/wireless/mediatek/mt7601u/ and mt76/; wpa_supplicant / IWD group-key handshake; OpenWrt mt7601u threads.
```

## Deliverable
A findings doc on your branch: `Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md` (SPDX `CC-BY-4.0`), structured like the existing W1 trap-mine: a numbered **trap bank** for Targets 1–3, each trap with (a) the documented source (file:line / RFC §), (b) the literal sequence/bytes where relevant, (c) the **discriminator** that would catch it on a bare-metal run. Flag exact-byte vectors you obtained from raw source vs anything you could only summarize (the latter is NOT oracle-grade — say so). RETURN the path.

## Scope guard
**Research only — a findings doc.** No code, no QEMU, no kernel touch, no gate. **Honor the byte-oracle acquisition rule:** pull raw authoritative source for any exact byte/register sequence; never launder exact bytes through a summarizing fetch (it poisons the oracle). If a trap's bytes can't be obtained from raw source, mark it "needs raw-source confirmation," don't guess.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-prometheus.md` (+ references). **Branch:** `prometheus/phase-7.5-closeout-wifi-trap-mine` off `origin/main`.
- **One-line:** **Prior-art trap-mine (research only, no code/QEMU)** — one of four Phase-7.5-closeout brains, your scoped lane. Mine the **exact** documented gotchas + literal byte/register sequences from the upstream Linux `mt7601u`/`mt76` driver + `wpa_supplicant`/IWD for three targets: **(1)** the ch6+ **bulk-pipe / channel-switch reset + persistent-MCU-response-buffer** pattern (feeds Cerebro's OT-27(d) fix), **(2)** **GTK/group-key extraction** for broadcast RX (OT-27(b) — verify byte layout against raw authoritative source), **(3)** **DHCP-over-`wlan0`** differences vs the USB-Ethernet path (OT-27(b)). Each trap gets source + literal sequence + a discriminator. **Honor the byte-oracle rule: raw source for exact bytes, NEVER a summarizing fetch** — mark anything unconfirmable. **Deliver `Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md`; RETURN the path.**
