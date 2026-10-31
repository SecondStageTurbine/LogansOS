<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CB-5 trap-mine pre-seed — SAE-on-air + full PMF/802.11w prior art (runner-free) → Prometheus

**Date:** 2026-06-29
**From:** PM (Claude)
**To:** ~~Prometheus~~ **→ REASSIGNED to Oracle 2026-07-02** (Prometheus out of commission; Oracle just landed the 1.5B build and is free). Main-tier on a host-only research task — no runner cost. New branch off `origin/main` HEAD (`1fec219`): `codex/cb-5-trap-mine-oracle`.
**Status:** DISPATCHED — **runner-free groundwork that makes CB-5 turnkey** (same pattern as the CB-6 prep audit). CB-5 (`DISPATCH-2026-06-29-phase-7.5-cb-5-wpa3-sae-onair-cerebro.md`) is the last engineering before the Phase-7.5 tag; it carries a mandatory prior-art trap-mining pass (driver/protocol standing rule). Doing that mine **now**, while Cerebro is on the CB-1b bare-metal run, means Cerebro starts CB-5 with the failure bank already seeded. **Scope: research/docs/host-only — NO kernel code, no QEMU, no behavior change. Output is a FINDINGS doc.**

## 0. The decision this prep makes
Pre-seed CB-5's §5 blind-hardware discriminator bank from **documented field gotchas for the exact protocols CB-5 builds** — SAE (dragonfly) on-air and full 802.11w/PMF — so the on-air run is conclusive on the first boot instead of burning blind iterations. The cheapest iteration is the one prior art already paid for.

## 1. References
```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-06-29-phase-7.5-cb-5-wpa3-sae-onair-cerebro.md   # the build this seeds — its ISC-1…5 + §5 truth-table
git show origin/main:kernel/src/net/wpa/sae.rs                       # the SAE crypto already shipped (what the exchange drives)
git show origin/main:Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md   # the existing T-028…T-048 bank to extend (don't duplicate)
git show origin/main:Plans/DISPATCH-2026-06-13-sae-vector-sourcing-prometheus.md             # how W0b's SAE vectors were sourced (raw IWD, the pattern to repeat)
```

## 2. ISCs (all produce ANALYSIS, not code)
- **ISC-1 — SAE on-air trap mine.** From **raw** IWD (`src/sae.c`, `unit/test-sae.c`) and wpa_supplicant (`src/common/sae.c`) source: the Commit/Confirm frame field order + lengths, the **anti-clogging-token** retry dance, the **status-code** paths (`SAE_HASH_TO_ELEMENT=126`, the rejected-groups handling), which groups to offer (19 = P-256, the shipped curve), and the H2E-vs-hunting-and-pecking selection. Each trap → a discriminator (what register/breadcrumb localizes it). **Confirm the on-wire Commit/Confirm bytes against IWD's unit vectors — raw source, NEVER WebFetch** (the byte-oracle acquisition rule; a summarizing fetch poisons the oracle).
- **ISC-2 — Full 802.11w / PMF trap mine.** From IEEE 802.11-2020 §12.5–12.6 + mac80211/`mt76` source: the **MFPC/MFPR** RSN-cap bits (and what a SAE-only AP rejects without them), the **IGTK KDE** layout (key id 4/5) in 4-way M3, **BIP-CMAC-128** MMIE byte layout + the **IPN** replay window, and exactly which mgmt subtypes are protected (deauth/disassoc/robust-action). Flag the headline property to prove on-air: **a forged unprotected deauth must be ignored.** Each → a discriminator.
- **ISC-3 — Group-Key-Handshake (T-039) confirm.** The standalone AP-initiated 2-message group rekey (EAPOL-Key group M1/M2) byte layout from raw source — T-039 was flagged "needs-raw-source-confirm vs iwd" in the existing FINDINGS; resolve it here so CB-5's ISC-4 byte-oracle is anchored.
- **ISC-4 — mt7601u-specific PMF/IGTK quirks.** Does the mt7601u WCID/key-table path have a slot for the IGTK (vs the WCID-0 GTK at T-036), or does BIP verify in software? Mine the driver for the IGTK install path (or its absence) so CB-5's ISC-3 knows whether IGTK is a hardware-key-slot or a software-verify path.

## 3. Scope guard
**Host-only + a FINDINGS doc.** Do NOT modify any code, add a QEMU scenario, or touch a Cargo feature. If a trap implies a code decision, note it as a CB-5 line-item. Output: `Plans/RESEARCH-2026-06-29-cb-5-sae-pmf-trap-mine-FINDINGS.md`, numbered continuing the existing T-series (T-049…), each with a one-line discriminator, so CB-5 drops them straight into its §5 bank.

## 4. Gate + close
- **Host only:** no code ships. `cargo fmt`/SPDX clean on the added doc. **No QEMU gate** (research/docs; paths-ignored CI).
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). RETURN the FINDINGS path + the new T-numbers. PM folds the reference into CB-5's dispatch.
- **Disk reclaim** after merge per CLAUDE.md.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-29-cb-5-trap-mine-prometheus.md` (+ the CB-5 dispatch it seeds, the existing T-028…T-048 FINDINGS, `net/wpa/sae.rs`). **Branch:** `prometheus/cb-5-trap-mine` off `origin/main` HEAD.
- **One-line:** **Runner-free CB-5 groundwork — mine the SAE-on-air + full-PMF/802.11w field traps so CB-5 is turnkey when Cerebro starts it.** Produce `Plans/RESEARCH-2026-06-29-cb-5-sae-pmf-trap-mine-FINDINGS.md` (T-049…) from **raw** IWD/hostapd/mac80211/mt76 source: SAE Commit/Confirm fields + anti-clogging + status codes (ISC-1), MFPC/MFPR + IGTK KDE + BIP-CMAC-128 MMIE + IPN + protected-deauth (ISC-2), the T-039 group-key-handshake bytes (ISC-3), and the mt7601u IGTK install path / software-BIP question (ISC-4) — each with a discriminator. **Byte references from raw source, NEVER WebFetch.** **Host-only, no code.** Close-gate; RETURN the FINDINGS path + T-numbers.
