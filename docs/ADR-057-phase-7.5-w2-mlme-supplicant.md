<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-057: BUILD-W2 — the in-kernel 802.11 STA supplicant (MLME + 4-way + CCMP + NetDevice)

**Status:** **Association proven on silicon (`4f82155`), usable-link tail-open.** W2
takes W1 from scan-only to *associated + encrypted data*: an in-kernel minimal
supplicant drives W1's MT7601U frames through an 802.11 MLME state machine (scan →
open-auth → assoc → WPA2 4-way handshake → CCMP keys installed → Connected), and
presents the link as a Phase-4 `NetDevice` so the existing IPv4/DHCP stack runs
over WiFi. Every cryptographic + protocol building block is byte-oracle'd
host-side (133/133 in `tools/ai-ops-tests`); both kernel build modes compile; the
kill-switch holds. The Stage-B association proof landed on the OptiPlex at
`4f82155`; the remaining tail is usable-link hardening carried by CB-1/CB-1b
(GTK/broadcast RX, DHCP-over-`wlan0`, and the real cold lease proof).

**Date:** 2026-06-16
**Dispatch:** `Plans/DISPATCH-2026-06-16-phase-7.5-build-W2-mlme-ccmp-cerebro.md`
**Bring-up reference:** `Plans/RESEARCH-2026-06-15-w2-mlme-supplicant-prep-FINDINGS.md` (mined MLME/handshake/CCMP plan + traps).
**Builds on:** ADR-054 (W1 — the MT7601U driver + its scan/TX/RX frames), ADR-051 (W0 — the WPA2/3 supplicant crypto: PMK/PTK/GTK/EAPOL-MIC/CCMP), ADR-045/050 (the fenced DMA arena + VT-d, I-14), ADR-047 (the CDC-ECM `NetDevice` precedent W2's `WifiNetDevice` mirrors).
**Ledger:** **no new invariant, no new ordered lock, no new `unsafe` idiom.** The supplicant + `WifiNetDevice` reuse the established `SwitchMutex`/`USB_DEVICE` lock order and the USB-transfer/MMIO `unsafe` idiom; all DMA stays in the fenced arena (I-14). Default-off ⇒ bit-for-bit: the kill-switch is the `wifi`/`mt7601u` feature gate — the default kernel compiles unchanged and none of W2's code is built.

## The decision: an in-kernel minimal supplicant
mac80211 leaves the 4-way handshake to a userspace supplicant (wpa_supplicant);
NeoPlutarch has none, so W2 *is* the supplicant — it owns both the MLME state
machine **and** the handshake logic, wiring W0's crypto over W1's frames. The flow:
`scan` (W1) → `build_open_auth`→TX, RX→`parse_auth` → `build_assoc_req`→TX,
RX→`parse_assoc_resp` → (WPA2) the 4-way: RX M1→`kdf::ptk_ccmp`→TX M2(+MIC)→RX
M3(`verify_mic`, `aes_unwrap` GTK)→`wcid_set_key`(TK,GTK)→TX M4 → Connected → register
the `WifiNetDevice`.

## Key architecture decisions
- **HW CCMP offload (ISC-3).** The MT7601U firmware does CCMP on the data path once
  the pairwise key is installed in the WCID hardware key table (plain register
  writes — `MT_WCID_KEY/IV/ATTR` — NOT MCU commands, per FINDINGS §4.2). W0's
  AES-CCM is the byte-oracle'd *correctness reference* + the software replay window
  (`ccmp::ReplayWindow`, PN from `pn_from_header`) is enforced in software.
- **Key install = register writes.** `wcid_set_key` writes the 32-byte key as
  consecutive 32-bit registers (the driver's own `addr_wr` idiom; `wr_copy` is just
  the burst optimization).
- **EAPOL over 802.11 data.** The 4-way EAPOL-Key frames ride 802.11 DATA frames
  (RFC 1042 LLC/SNAP, EtherType 0x888E) — `build_eapol_data`/`parse_eapol_data`.
- **GTK unwrap.** M3's GTK is AES-Key-Wrapped (RFC 3394) with the KEK; W0 had the
  KEK but not the unwrap, so W2 added `wpa::keywrap::aes_unwrap` (new code, KAT'd
  vs the RFC 3394 §4.1 vector).
- **Raw-source constants.** The 802.11 frame layouts/constants came from raw
  `include/linux/ieee80211.h` v6.6 + `nl80211.h` (curl, not WebFetch) — which
  CORRECTED four FINDINGS errors that would have silently broken association:
  `WLAN_AUTH_OPEN` (0 not 1), `WLAN_AUTH_SAE` (3 not 4), `EID_EXT_SUPP_RATES` (50 not
  51), and the RSN PSK AKM type (2 not 1).

## The byte-oracle spine (I-12) — what is *proven*, host-side, now
133 host tests in `tools/ai-ops-tests`, each against hand-laid fixtures independent
of the producer (and the MLME mgmt-frame layouts cross-checked by an independent
Forge derivation — VERDICT MATCH):
- **ISC-1 MLME:** open-auth (30 B), WPA2-PSK RSN IE (22 B), assoc-req (61 B), the
  parse offsets, the IE walker, the MLME state transitions.
- **ISC-1 TX:** the 20-byte TXWI, the 4-byte TXINFO (+mgmt flags), the wrapped USB
  buffer.
- **ISC-2:** EAPOL-over-data encap; the EAPOL-Key M2 builder (field-exact +
  round-trips W0's parser + install/verify MIC); AES Key Unwrap (RFC 3394 §4.1);
  the WCID IV/ATTR encoding.
- **ISC-3:** CCMP PN extraction + the replay window.
- **ISC-4:** the 802.3↔802.11 data-frame conversion.
- **Reused (W0/W0b, already KAT'd):** PMK/PTK/GTK/PMKID, EAPOL-MIC (HMAC-SHA1/
  AES-CMAC), AES-CCM, SAE.

## I-14 fault containment (DISPATCH §3 — restated, no new guard)
- **(a) Lock isolation:** the MLME/TX/RX path acquires no AI-governance lock — it
  uses the device's `USB_DEVICE`-ordered `SwitchMutex` only.
- **(b) Liveness:** the supplicant's bounded poll loops (`bulk_budget`, the
  EAPOL/assoc spin budgets) cannot monopolize a CPU indefinitely; the residency
  daemon's P4b backstop is unaffected.
- **(c) Memory:** all TX/RX/key DMA stays in the fenced descriptor-validated arena
  behind VT-d (W1 proved `fence_refusals=0`); the Stage-B gate re-asserts
  `fence_refusals=0`.

## Blind-hardware discriminators (DISPATCH §4)
The assoc scenario dumps a self-localizing breadcrumb every run: scan `bss`,
`assoc_stage` (auth→assoc), `handshake` (M1→M4), `last_status` (reject vs silence),
`aid`, `connected`, `fence_refusals`. A failure maps to its cause: SSID-not-found /
auth-reject / assoc-reject / 4-way-MIC-mismatch / GTK-unwrap / seam.

## SNonce → CSPRNG (OT-2 — CLOSED, 2026-06-21)
**Originally a follow-on:** the SNonce was counter-derived, not a CSPRNG — adequate
to bring the link up but not a security boundary. **Now closed:** `wifi` pulls
`kernel-rng`, and `gen_snonce` draws the SNonce from the kernel ChaCha20 DRBG
(`crate::rng::fill_random`, the 10.1 DRBG via 10.3's kernel fill), fail-closed if
the DRBG is unavailable. The PTK therefore depends on cryptographically-random
material, so W2 is a real security boundary. The counter survives only as an
explicitly-marked insecure fallback compiled when `kernel-rng` is absent (a config
the shipped feature graph never produces). **KAT (I-12):** `wpa_ot2_ptk_snonce_agnostic_kat`
(host) proves the handshake derives a correct, *distinct, PTK-specific* key from a
random non-counter SNonce and the M2 MIC round-trips; the CSPRNG-ness of the source
is pinned by the existing `kernel-rng` self-test KATs.

## Consequences
W2 closes the Phase-7.5 WiFi arc end-to-end through association on silicon (W0
crypto → W0b SAE → W1 scan → W2 associate): the OS can join a real AP at `4f82155`.
The remaining tail is link usability rather than association proof: CB-1/CB-1b
carry GTK/broadcast RX, DHCP-over-`wlan0`, and the cold lease evidence. Reconciles
ADR-054 (W1): the scan driver is now driven by the supplicant, and the data-TX
endpoint + the WCID key table it left unused are now wired.
