<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: W2 on-air MLME supplicant — deep prep research (raw-source) → Prometheus

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** Prometheus (research / raw-source mining lane; NO kernel code)
**Status:** DISPATCHED. **Goal: scope BUILD-7.5-W2 (the on-air MLME + supplicant) *before* it's built — turn it into a port, not a from-scratch.** W2 is the WiFi arc's last build: the minimal STA state machine that wires **W0/W0b's proven crypto** (PMK/PTK/GTK/EAPOL-MIC/CCMP + SAE, already host-KAT'd on `main`) to **W1's frames** (the MT7601U driver, in flight) so the Phase-4 network stack lights up over WiFi.
**Prep, not build:** W2 is gated behind W1 (needs the driver + scan working). This research runs ahead, exactly like the trap-mine did for W1 and the gap analysis did for Phase 8.
**Related (read for the baseline):** `Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` **§3** (the starter mac80211 STA crib — you wrote it; this deepens it), `Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` §1–§2 (the minimal-STA scope), the W0/W0b crypto on `main` (`kernel/src/net/wpa/`).

## 0. What W2 is (and is NOT) — the boundary to scope against
**In scope (minimal STA):** scan (have it from W1) → **authenticate** (open for WPA2, SAE-commit/confirm for WPA3) → **associate** → the **WPA2 4-way handshake** (or WPA3 SAE-derived keys) → **key install** → **data** (RUN) → the `NetDevice` glue so the Phase-4 stack runs over it (DHCP/DNS/TCP, same as USB-Ethernet did). **NOT in scope:** AP/mesh/P2P, roaming, 802.11n aggregation tuning, WPA-Enterprise/802.1X, band-steering. Minimal-sufficient: the smallest MLME that *associates and passes IP*.

## 1. The mission — five deliverable sections (raw-source; the raw-source rule binds on exact frame/IE bytes)
- **§1 — The minimal STA MLME state machine.** From raw Linux `mac80211`/`cfg80211` (`net/mac80211/mlme.c`, the SME/auth/assoc path) — the exact states + transitions for a STA: `SCAN → AUTH → ASSOC → 4WAY/SAE → RUN`, the timeouts/retries at each, and which events drive each transition. Cite file:line. This is the spine W2 implements.
- **§2 — The management frame + IE byte layouts (raw — the raw-source-KAT rule binds).** The exact on-air frames W2 builds/parses: **Authentication** (open; and the SAE auth frames carrying commit scalar/element + confirm), **Association Request/Response**, **Deauth/Disassoc**. And the **Information Elements** verbatim: SSID, supported/extended rates, **the RSN IE** (the WPA2/WPA3 cipher+AKM suites — this is what advertises CCMP + PSK/SAE), the SAE elements. Pull the literal IE byte structure from the IEEE 802.11 frame format (raw source — a reference impl's frame builder, e.g. `net/mac80211` or `wpa_supplicant`'s `src/common/ieee802_11_defs.h`), **not a summarizing fetch.** These become W2's frame byte-oracles (I-12).
- **§3 — The WPA2 4-way handshake on-air sequencing.** How the EAPOL-Key M1→M2→M3→M4 exchange flows over the air (which side sends what, the ANonce/SNonce exchange, the MIC/replay-counter handling, the GTK delivery in M3, retries/timeouts) — i.e. **how W0's already-proven EAPOL/PTK/GTK crypto is driven on the wire.** W0 computes the keys + MIC; this is the *sequencing* that wires it. Plus the WPA3 path: the SAE commit/confirm auth frames precede assoc, then a 4-way handshake derives the session keys from the SAE PMK (W0b computes the SAE side).
- **§4 — Key install via the MT7601U MCU (the driver seam).** From the raw `mt7601u` driver: the **WCID (WLAN Client ID)** model, and the **MCU command that installs the PTK/GTK into the hardware** so **firmware does the data-plane CCMP** (the trap-mine noted firmware-offload CCMP — confirm the exact MCU key-install command + WCID table layout, file:line). This is the W1↔W2 boundary: W2 derives keys (via W0), then pushes them to the chip through this seam.
- **§5 — Mgmt/data frame TX/RX on the MT7601U + the `NetDevice` glue.** How mgmt frames (auth/assoc) and data frames map onto the MT7601U TXWI/RXWI path (vs the data path W1 already does); and how the associated STA presents as a `NetDevice` so the Phase-4 stack runs over it (the CDC-ECM precedent — ADR-047). Honest scoping note: WPA2 first, WPA3 after; what the minimal RUN state needs.

## 2. Deliverable
`Plans/RESEARCH-2026-06-15-w2-mlme-supplicant-prep-FINDINGS.md` — the five sections, with: the STA state diagram (states/transitions/timeouts, file:line), the **verbatim mgmt-frame + RSN-IE byte layouts** (raw-source, for W2's frame oracles), the 4-way/SAE on-air sequencing, the MCU key-install command + WCID layout (file:line), and the NetDevice mapping. Plus a provenance table (repo/commit/file:line/how-obtained) and honest negatives. A recommended **W2 sub-slice** (e.g. WPA2-associate-only first → WPA3 → full RUN) is welcome.

## 3. Scope guard
Research/acquisition only. **Do NOT** write Rust, implement the MLME, or design the W2 architecture beyond a suggested slice (you supply the mined facts). **Do NOT** WebFetch exact frame/IE/MCU byte layouts — `git clone`/`curl` raw and read the literal bytes (the raw-source rule — it caught the SAE `mac2` slip, the GGUF q4_K size, the AX88179 day). **Do NOT** re-scope into AP/roaming/Enterprise. If a layout can't be found in raw source, flag it for a discriminator rather than guessing.

## 4. ⚠ CLOSE-GATE — PUSH WHEN DONE (standing reminder)
**Before you report "done," push the branch:** `git push -u origin prometheus/w2-mlme-supplicant-prep`. A local-only branch is invisible to the PM seat — the last four lanes (SAE, trap-mine, gap-analysis, INDEX) each needed a follow-up "please push." End the lane with the push, then report. The tree is the arbiter; unpushed work isn't real.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-w2-mlme-supplicant-prep-prometheus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-w2-mlme-supplicant-prep-prometheus.md
  git show origin/main:Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md   # §3 = your starter STA crib to deepen
  ```
- **Branch:** `prometheus/w2-mlme-supplicant-prep` off `origin/main` HEAD.
- **One-line:** Scope **W2** (the on-air MLME supplicant — the WiFi arc's last build) ahead of time. Raw-source mine: (1) the **minimal STA state machine** (SCAN→AUTH→ASSOC→4WAY/SAE→RUN) from Linux `mac80211` (`net/mac80211/mlme.c`), file:line; (2) the **mgmt-frame + RSN-IE byte layouts** verbatim (auth/assoc/deauth + the RSN IE advertising CCMP+PSK/SAE — `git clone`, never WebFetch the bytes); (3) the **WPA2 4-way (M1–M4) + WPA3 SAE on-air sequencing** that drives W0/W0b's already-proven crypto onto the wire; (4) the **MT7601U MCU key-install command + WCID** so firmware does data-plane CCMP (raw `mt7601u`, file:line); (5) the mgmt/data TXWI-RXWI path + the **`NetDevice` glue** (CDC-ECM precedent). Deliver `Plans/RESEARCH-2026-06-15-w2-mlme-supplicant-prep-FINDINGS.md` + a suggested W2 slice. No code. **PUSH the branch (`git push -u origin prometheus/w2-mlme-supplicant-prep`) before reporting done** — don't make me chase it. Turns W2 into a port, not a from-scratch build.
