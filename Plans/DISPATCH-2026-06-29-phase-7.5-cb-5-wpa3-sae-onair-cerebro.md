<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 CB-5 — WPA3-SAE on-air + full PMF/802.11w (the R1 close gate) → Cerebro

**Date:** 2026-06-29
**From:** PM (Claude)
**To:** ~~Cerebro~~ **SPLIT (PL decision 2026-07-02, Cerebro + bare-metal out for a while):**
> - **CB-5a — the CODE + hermetic QEMU gate → Oracle, NOW.** ISC-1…ISC-5 (SAE on-air exchange, WPA3 RSNE/PMF, full 802.11w software-verify, SAE-PMK→4-way + GKH, kill-switch) **plus the device-less 4-shard QEMU gate (ISC-5's frame-injection pump) + AAG.** This is ~all the engineering and is fully gateable with no hardware. Merges to `main` on the hermetic gate. Branch off `origin/main` HEAD (`7f360fe`): `codex/cb-5a-wpa3-sae-code-oracle`. Author `SecondStageTurbine`. **Trap-mine is folded in (T-049…T-061) → turnkey.**
> - **CB-5b — the ON-AIR silicon proof (§5) → Cerebro, later.** The real-AP WPA3-SAE association + group-key-rekey + forged-unprotected-deauth-ignored run, when the bare-metal window opens. This is the R1 close gate that (with CB-1b) unblocks the tag. Oracle's CB-5a code is what Cerebro proves on-air; Cerebro reviews CB-5a on return.
>
> Rationale: with hardware parked, building the code now converts the tag's last engineering gate from "big kernel build + on-air run" down to **just the on-air run**. Reverses the earlier "Cerebro serial" ruling because Cerebro is out.

**Original To (for the record):** Cerebro (WiFi-lane owner — CB-1/CB-2 author). New branch off `origin/main` HEAD (`d958f8f`): `codex/cb-5-wpa3-sae-onair-cerebro`. Author `SecondStageTurbine`.
**Status:** DISPATCHED — CB-5a (code) is **active on Oracle now**; CB-5b (on-air) is Cerebro's bare-metal follow-on. This is the **last engineering before the Phase-7.5 tag.** CB-5 closes ratification **R1** (WPA3-SAE on-air is a REQUIRED close gate — crypto KATs alone are not sufficient; the bible's "WPA3 client mode" promise is honored on-air). **PMF scope: FULL 802.11w** (PL decision 2026-06-29) — MFP negotiation **and** IGTK/BIP protected-management-frame verification, not just negotiation.
>
> **Note for Oracle (CB-5a):** build all five ISCs + the **hermetic** QEMU gate (ISC-5, device-less frame-injection pump — that IS your gate). §5's on-air truth-table is NOT your deliverable (it's CB-5b/Cerebro) — but wire every §5 breadcrumb/register capture so Cerebro's on-air run is self-diagnosing (capture-before-you-need-it). Since you're on Cerebro's WiFi surface: touch only the SAE/PMF/connect path; do not disturb CB-1b/CB-2 mt7601u recovery/RX code beyond what SAE needs.
>
> **Driver-integration scoping — RULED 2026-07-02 (PL fork → Option 1):** the mt7601u DMA drive is blind (its runtime proof is CB-5b's silicon run), so CB-5a builds a **host-testable `sae_connect` orchestrator over an abstract TX/RX seam** that drives the full Commit→Confirm→assoc→SAE-PMK-4-way→GKH→BIP sequence — host-tested AND exercised by the hermetic gate's frame-injection pump (real coverage of the state machine, not just leaf KATs). It wires into `associate()` **minimally**, behind `wpa3-sae` (off ⇒ **CB-2 bit-for-bit**, kill-switch-equality KAT required). The only blind residue is a **thin abstract-TX/RX → real-mt7601u-mgmt adapter** — that adapter + the on-air run are **CB-5b's explicit deliverables** (Cerebro debugs the adapter + RF on silicon, not the already-gate-proven state machine). *Rejected: full DMA integration now (maximizes blind, gate-uncovered changes to Cerebro's driver → CB-2-regression risk only silicon catches); defer-all-DMA (under-uses the gate — leaves the sequencing unproven).* **CB-5a AAG will center on: kill-switch off ≡ CB-2, the pump drives the full sequence (no stub), no CB-2 recovery/RX regression, and I-14 fenced-arena DMA.**

## 0. What this build is

The SAE **crypto** shipped and is KAT'd against IWD vectors (W0b: `kernel/src/net/wpa/sae.rs` — Commit/Confirm scalars, H2E PWE over NIST P-256). What does **not** exist: on-air SAE **association**. `mt7601u::associate()` (`mod.rs:2098`) only drives `auth_alg=OPEN` → WPA2 4-way; there is no SAE Auth Commit/Confirm exchange, no WPA3 RSNE (AKM-8), **no PMF/802.11w at all**, and no standalone Group-Key-Handshake. CB-5 wires the on-air WPA3 path end-to-end and proves it on silicon against a real WPA3-SAE AP.

**Reuses (do not rebuild):** the SAE crypto (`sae.rs`), the mgmt TX/RX + MLME state machine (`mlme_logic.rs`, `associate()`), the 4-way handshake + the M3→WCID-0 GTK install (CB-1: `GTK_INSTALLED`/`GROUP_WCID`, `mod.rs:237-280`), the pairwise-CCMP path. **New:** the SAE auth exchange, the WPA3 RSNE/PMF negotiation, full 802.11w (IGTK/BIP), the SAE-PMK→4-way keying, and the T-039 Group-Key-Handshake rekey.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/net/wpa/sae.rs                    # SAE crypto (Commit/Confirm/H2E) — reuse, do not rebuild
git show origin/main:kernel/src/net/wifi/mlme_logic.rs           # AUTH_SAE=3, AKM_TYPE_SAE=8; build_auth/build_rsn_ie (WPA2 today)
git show origin/main:kernel/src/drivers/usb/mt7601u/mod.rs       # associate() (2098); 4-way (2304); M3 GTK install + GROUP_WCID (237-280)
git show origin/main:kernel/src/net/wpa/eapol.rs                 # EAPOL-Key; v3 AES-128-CMAC (the PMF/AKM-SAE MIC algo)
git show origin/main:Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md   # T-036/037 GTK→WCID-0, T-039 group-key-handshake (the trap bank)
git show origin/main:Plans/RESEARCH-2026-06-29-cb-5-sae-pmf-trap-mine-FINDINGS.md              # T-049…T-061 — THE CB-5 §5 discriminator bank (SAE/PMF/IGTK), pre-seeded. READ FIRST.
git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-cb-1-wifi-link-cerebro.md             # the CB-1 GTK/broadcast-RX pattern this extends
git show origin/main:Plans/DISPATCH-2026-06-24-baremetal-gold-standard-cerebro.md              # the bare-metal session pattern CB-5's §5 on-air proof rides
```

## 2. Prior-art trap-mining pass (driver/protocol standing rule — REQUIRED before first run)

> **DONE — pre-seeded (Oracle, 2026-07-02): `Plans/RESEARCH-2026-06-29-cb-5-sae-pmf-trap-mine-FINDINGS.md`, T-049…T-061 (13 traps, 12/13 oracle-grade from raw source).** The trap-mine below is already executed; the bank drops straight into §5. Two findings change the build itself — apply them before starting:
> - **ISC-4 design branch DELETED (T-061):** the mt7601u `set_key` handles only WEP/TKIP/CCMP → BIP/AES-CMAC returns `-EOPNOTSUPP`. **The IGTK has no hardware WCID slot — it is a *software* key.** So ISC-3's BIP MMIE + IPN run on the host CPU (software-verify), NOT a WCID program. **Do NOT build IGTK-to-WCID; it cannot work on this silicon.**
> - **T-039 byte correction (T-060):** the Group-Key-Handshake M1 `key_info` is **`0x1383` (Install bit = 0)**, NOT the previously-recorded `0x13C3` — Install=1 is a 4-way-M3-only bit; iwd's `eapol_verify_gtk_1_of_2` rejects Install=1 on GKH. Use `0x1383` in ISC-4's group-rekey byte-oracle.
> - Also folded: SAE Commit/Confirm byte-oracle + anti-clogging placement differs by PWE method (inline hunting-and-pecking vs ext-IE 255/ext-id 93 for H2E) + silent-stall on status method-mismatch (STA 126 vs AP 0), group-19-only (T-049–T-053); MMIE = eid 76·len 16·keyid·IPN 6B·MIC 8B, BIP AAD = masked-FC‖A1‖A2‖A3, IPN replay = byte-swap+memcmp+advance-after-MIC, SAE mandates MFPC+MFPR (T-054–T-059). See the doc's §"CB-5 line-items".

The mt7601u/GTK traps are banked (FINDINGS T-028…T-048, now T-049…T-061). CB-5 adds **two un-mined surfaces** — *these are now mined (above); the original mandate is kept for the record*:
- **SAE on-air** (IWD `src/sae.c` + `unit/test-sae.c`; wpa_supplicant `src/common/sae.c`): the Commit/Confirm frame field order, anti-clogging-token retry, the status-code path (e.g. `SAE_HASH_TO_ELEMENT`=126), groups offered (19 = P-256), and the **rejected-groups** dance. Confirm the on-wire Commit/Confirm bytes against IWD's unit vectors — **raw source, never WebFetch** (the byte-oracle acquisition rule; W0b's SAE vectors were sourced this way).
- **802.11w / PMF** (IEEE 802.11-2020 §12.5–12.6; mac80211/`mt76`): MFPC/MFPR RSN-cap bits, the IGTK KDE (key id 4/5) in 4-way M3, **BIP-CMAC-128** MMIE layout + the IPN replay window, which mgmt subtypes are protected (deauth/disassoc/robust-action). Seed each as a §5 discriminator row.

## 3. ISCs (ordered; each its own sub-commit; each byte-oracle'd where it defines bytes)

- **ISC-1 — SAE Auth exchange on-air.** In the WPA3 connect path, replace the OPEN auth with the SAE exchange: build/parse `Auth(alg=SAE=3)` **Commit** (txn 1: group + scalar + element) and **Confirm** (txn 2: send-confirm + confirm hash), driving `sae.rs` over the mgmt TX/RX; the SAE state machine (Nothing→Committed→Confirmed→**PMK derived**). **Byte-oracle:** the Commit + Confirm frame bodies against IWD `unit/test-sae.c`. Anti-clogging-token + one Commit retry handled (FINDINGS trap).
- **ISC-2 — WPA3 RSNE + AKM-8 + PMF negotiation.** Build the RSN IE for WPA3: group CCMP, pairwise CCMP, **AKM_TYPE_SAE=8**, RSN caps with **MFPC + MFPR** set (SAE-only mandates MFPR). Carried in the assoc request. **Byte-oracle:** the WPA3 RSNE against a hand-laid example + IWD/hostapd.
- **ISC-3 — Full 802.11w (IGTK/BIP), SOFTWARE-verify (T-061).** Install the **IGTK** as a **software key** (mt7601u `set_key` returns `-EOPNOTSUPP` for BIP — no hardware WCID slot), and run **BIP-CMAC-128** MMIE compute/verify **on the host CPU** over protected broadcast/multicast management frames + the **IPN** replay check (wire byte-swap + `memcmp<=0` + advance-only-after-MIC-passes, T-057); honor protected deauth/disassoc (a forged unprotected deauth must be ignored — the headline PMF property; mind the pre-key `RX_CONTINUE` exception, T-059). **Byte-oracle:** MMIE = `eid 76 · len 16 · keyid LE16 · IPN 6B · MIC 8B` (18B), BIP AAD = masked-FC (Retry/PwrMgt/MoreData zeroed) ‖ A1‖A2‖A3 (20B), IGTK KDE = `2B-LE KeyID + 6B IPN + 16B IGTK` (T-054–T-058, raw source). This is the FULL-PMF deliverable (PL R1 scope). **Do NOT attempt an IGTK-to-WCID hardware install — it can't work on this silicon.**
- **ISC-4 — SAE-PMK → 4-way + Group-Key-Handshake.** Feed the SAE-derived PMK into the **existing** 4-way (reuse CB-1's path; the only change is PMK provenance) → pairwise CCMP + the M3→WCID-0 GTK install. Add the standalone **Group-Key-Handshake** (T-039, corrected by T-060): the AP-initiated 2-message group rekey (EAPOL-Key group M1/M2) with **`key_info = 0x1383` (Install bit = 0)** — Install=1 is rejected on GKH — installing the refreshed GTK to `GROUP_WCID`; the R1 bar names this explicitly. **Byte-oracle:** the group EAPOL-Key M1/M2 layout with the corrected `0x1383` key_info (raw source).
- **ISC-5 — Kill-switch + I-14 + the hermetic QEMU gate.** Kill-switch `wpa3-sae` **default-off ⇒ the connect path is WPA2-only, bit-for-bit CB-2** (an I-2-form collapse; prove disabled≡previous in a host KAT). The **device-less QEMU scenario(s)** (frame-injection pump, the CB-1 pattern) exercising: SAE Commit/Confirm → PMF-negotiated assoc → SAE-PMK 4-way → IGTK/BIP (forged-unprotected-deauth-ignored) → group-key rekey — hermetically, no real device. **The on-air silicon run (§5) is the device gate** for the parts QEMU can't drive (real AP, real RF).

## 4. Standing-rule sections (REQUIRED for a connectivity/driver dispatch)
- **I-14 fault-containment (connectivity standing rule).** (a) **Lock isolation** — the SAE/PMF path acquires no AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) on any ISR path (grep-checkable + assertion). (b) **Liveness degradation** — inherited; SAE compute is bounded, on the connect path, not an ISR. (c) **Memory isolation** — SAE/PMF buffers ride the already-fenced mt7601u DMA arena (I-14c1/c2, W1/V-proven); no new DMA surface. State which vectors are touched in the RETURN.
- **Kill-switch ISC** — see ISC-5 (`wpa3-sae` default-off ≡ CB-2 bit-for-bit; the equality regression is the required artifact).
- **Invariant budget** — preserve the ledger; **no new invariant** (SAE/PMF is driver/crypto, not a cross-cutting governance property). The page-fit alarm is resolved (CB-6) but I-16 still must re-check it — CB-5 adds none.
- **Byte-oracle** — every ISC that defines on-wire bytes (Commit/Confirm, WPA3 RSNE, IGTK KDE, BIP MIC, group EAPOL-Key) ships a hand-laid/raw-source-confirmed fixture independent of writer+reader. **Exact bytes from raw IWD/hostapd source, never WebFetch.**

## 5. Blind-hardware failure-mode + discriminator truth-table (the on-air silicon proof)
The on-air run rides the **gold-standard bare-metal pattern** (Cerebro builds + SSH-deploys the ISO + log sinks; the PL configures a **WPA3-SAE-only AP** — SSID/PSK out-of-band — and the only manual step is the reboot). Pre-seed from §2's trap mine. Each row: breadcrumb stall → most likely cause → discriminator.
- no `sae_commit_tx` ⇒ WPA3 path not selected / config ⇒ log the chosen AKM + auth_alg.
- `sae_commit_tx` then no `sae_commit_rx` ⇒ AP rejected the group / anti-clogging ⇒ log the Auth status-code + offered group.
- `sae_confirm` ok then no `assoc_resp` ⇒ RSNE/PMF mismatch (MFPC/MFPR) ⇒ dump the sent WPA3 RSNE vs the AP's RSN caps.
- `assoc` ok then 4-way stalls ⇒ SAE-PMK wrong (provenance bug) ⇒ log PMK source + M1 MIC validation.
- broadcast mgmt accepted that should be dropped (forged unprotected deauth disconnects us) ⇒ **PMF/BIP not enforcing** ⇒ log MMIE-present + BIP-verify result + IPN.
- pairwise ok but no broadcast RX / no group rekey ⇒ GTK / Group-Key-Handshake (T-039) ⇒ dump `MT_WCID_KEY(0)` + `MT_WCID_ATTR(0)` (PAIRWISE=0, cipher=CCMP) per T-036.
**Capture-before-you-need-it:** every register/value above must already print before the PL boots.

## 6. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch + AAG** on the hermetic ISC-5 scenarios **before the PL flashes**. Push-CI green (host KATs incl. the byte-oracles + the kill-switch equality). **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main`, re-run the gate, push ff-clean; host set; SPDX; self-stat).
- **PASS = the silicon close-bar:** the on-air sink shows SAE Commit/Confirm → PMF-negotiated assoc → SAE-PMK 4-way → pairwise+GTK → a successful **group-key rekey**, AND a forged unprotected deauth is ignored (PMF enforced). PARTIAL/FAIL self-localizes to a single §5 row → scoped follow-on iteration.
- **RETURN** `Plans/RETURN-2026-06-29-phase-7.5-cb-5-…-cerebro.md` with the QEMU shard URLs + SHA, the byte-oracle fixtures, the kill-switch equality proof, the I-14 vector statement, and the on-air sink (or the localized failure row).
- **Disk reclaim** after merge per CLAUDE.md.
- **On merge + on-air PASS, CB-5 is the last close gate** → with CB-1b's silicon lease, the PM runs the integrated close gate + squash + **Phase-7.5 tag**.

---
## Relay — what to tell Cerebro (when the gold-standard session lands)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-29-phase-7.5-cb-5-wpa3-sae-onair-cerebro.md` (+ the SAE crypto, `mlme_logic.rs`, `associate()`, the FINDINGS trap bank). **Branch:** `codex/cb-5-wpa3-sae-onair-cerebro` off `origin/main` HEAD (`d958f8f`).
- **One-line:** **Wire WPA3-SAE on-air + full 802.11w, then prove it on silicon — the last gate before the tag.** The SAE crypto is shipped/KAT'd; build the on-air path: SAE Auth Commit/Confirm exchange (ISC-1), WPA3 RSNE/AKM-8 + MFPC/MFPR (ISC-2), **full PMF: IGTK + BIP-CMAC-128 + IPN, protected deauth/disassoc** (ISC-3), SAE-PMK→4-way + the T-039 Group-Key-Handshake (ISC-4), kill-switch `wpa3-sae` off ≡ WPA2/CB-2 bit-for-bit + the hermetic QEMU gate (ISC-5). Reuse CB-1's 4-way/GTK and `sae.rs`; don't rebuild them.
- **Mine first:** SAE-on-air + PMF traps from raw IWD/hostapd source (byte-oracle the Commit/Confirm, RSNE, IGTK KDE, BIP MIC — **never WebFetch**). **Carry:** the I-14 containment section + the kill-switch equality + the §5 blind-hardware discriminator bank.
- **Gate:** **4-shard QEMU + AAG on the hermetic scenarios before the PL flashes**; then the on-air run against a **WPA3-SAE-only AP** (PL configures it; PL's only manual step is the reboot, gold-standard pattern). Close-gate ritual; RETURN with shard URLs + SHA + the on-air sink. **PASS here + CB-1b's silicon lease = the PM tags Phase 7.5.**
