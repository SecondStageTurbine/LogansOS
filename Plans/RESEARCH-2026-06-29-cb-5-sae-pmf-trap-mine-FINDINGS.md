<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: CB-5 pre-seed — WPA3-SAE on-air + full PMF/802.11w trap-mine

**Date:** 2026-06-29 (executed 2026-07-02)
**Lane:** Oracle (deep prior-art mining — NO kernel code, NO QEMU, host-only)
**Delivers for:** `Plans/DISPATCH-2026-06-29-cb-5-trap-mine-prometheus.md` (reassigned Prometheus → Oracle)
**Seeds:** `Plans/DISPATCH-2026-06-29-phase-7.5-cb-5-wpa3-sae-onair-cerebro.md` — CB-5's §5 blind-hardware discriminator bank (the last gate before the Phase-7.5 tag)
**Extends:** `Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md` (T-028…T-048). **This adds T-049…T-061.**
**Sources (raw, direct `Read` — no WebFetch summarization for any byte/register/status value):**
- **iwd** (git.kernel.org/pub/scm/network/wireless/iwd.git, `plain/`): `src/sae.c`, `unit/test-sae.c`, `src/eapol.c` — now reachable (the 2026-06-23 FINDINGS noted iwd was auth-gated/empty; the cgit `plain/` endpoint served the files clean this run, which is what unblocks the T-039 iwd cross-confirm below).
- **hostap/wpa_supplicant** (w1.fi/cgit/hostap, `plain/`): `src/common/sae.c` (cross-reference).
- **Linux mac80211** (torvalds/linux master, raw): `net/mac80211/wpa.c`, `aes_cmac.c`, `key.c`, `rx.c`.
- **Linux mt7601u** (torvalds/linux master, raw): `drivers/net/wireless/mediatek/mt7601u/main.c`, `mac.c`.
- **Linux uAPI** (torvalds/linux master, raw): `include/linux/ieee80211.h` (struct/constant confirmation).

All line anchors below are into these raw files as fetched. Values I could not confirm from a downloaded file (a few live in headers not in the fetch set — `mmpdu.h`, `ieee80211_i.h`, iwd `handshake.c`) are marked **[header, cross-referenced]** with the IEEE 802.11-2020 clause.

---

## Target 1 — SAE on-air (ISC-1): Auth Commit/Confirm exchange

The SAE crypto is shipped (`kernel/src/net/wpa/sae.rs`, W0b). What CB-5 builds is the on-air Auth exchange. These traps are the on-wire framing around that crypto.

### T-049: SAE Commit frame body — field order + lengths (the byte-oracle)

- **Source:** iwd `src/sae.c:546-654` (`sae_build_commit`); status via `sae_status_code` (`src/sae.c:102-109`); byte-oracle vector `unit/test-sae.c:192-201` (`aa_commit[]`).
- **Trap / literal bytes.** The SAE body carried in the 802.11 Authentication frame (after the 2-byte **Auth Algorithm Number = 3 (SAE)**), for the shipped group 19 / NIST P-256:
  ```
  Offset  Size  Field                                    sae.c anchor
  0       2     Transaction Sequence Number = 1 (LE16)   :610  l_put_le16(1, ptr)
  2       2     Status Code (LE16)                       :614  l_put_le16(sae_status_code(sm))
                  = 0   (SUCCESS)  for hunting-and-pecking (CRYPTO_SAE_LOOPING)
                  = 126 (SAE_HASH_TO_ELEMENT) for H2E
  4       2     Finite Cyclic Group (LE16) = 19          :618  l_put_le16(sm->group)
  6       V     [LOOPING only, if a token was requested] anti-clogging token INLINE  :621-623
  ...     32    commit-scalar (32 B for P-256)           :626  l_ecc_scalar_get_data
  ...     64    commit-element (uncompressed EC point x||y = 2×32 B)  :627  l_ecc_point_get_data
  ...     TLV   [H2E only] Rejected-Groups / Anti-Clogging-Token-Container / Password-Identifier IEs  :631-649
  ```
  Cross-confirmed by the parse side: the receiver skips `scalar_bytes * 3` after the group (`src/sae.c:1179` `skip = l_ecc_curve_get_scalar_bytes(sm->curve) * 3`) — i.e. scalar(32) + element(64) = 96 B for P-256.
  Byte-oracle vector `aa_commit[]` (`unit/test-sae.c:192`) begins `0x13, 0x00` (group = 0x0013 = **19**, little-endian) followed by 32 B scalar + 64 B element = 98 B (the test feeds the body from the Group field; txn/status are prepended by the MLME layer).
- **Discriminator:** `sae_commit_tx` then no `sae_commit_rx` → dump the sent Commit and check, in order: txn == 1; **status matches the method** (0 for hunting-and-pecking, 126 for H2E — see T-052); group == `0x13 0x00`; scalar length == 32; element length == 64 (2× scalar). A wrong element length (32 instead of 64 — using a compressed point) is the classic mistake.

### T-050: SAE Confirm frame body — send-confirm + confirm hash

- **Source:** iwd `src/sae.c:656-686` (`sae_send_confirm`); `sae_cn` (`src/sae.c:281-320`); byte-oracle vector `unit/test-sae.c:204-208` (`aa_confirm[]`).
- **Trap / literal bytes:**
  ```
  Offset  Size  Field                                    sae.c anchor
  0       2     Transaction Sequence Number = 2 (LE16)   :672  l_put_le16(2, ptr)
  2       2     Status Code = 0 (LE16)                   :674  l_put_le16(0, ptr)
  4       2     send-confirm counter (sc, LE16)          :676  l_put_le16(sm->sc, ptr)
  6       H     Confirm hash (hash_len B: 32 for SHA-256/P-256)  :678  memcpy(ptr, confirm, r)
  ```
  `confirm = CN(KCK, send-confirm, own-scalar, OWN-ELEMENT, peer-scalar, PEER-ELEMENT)`; the `send-confirm` fed into CN is itself LE16 (`src/sae.c:304` `send_confirm = L_CPU_TO_LE16(send_confirm)`).
  Byte-oracle vector `aa_confirm[]` (`unit/test-sae.c:204`) = `0x00, 0x00` (send-confirm = 0, LE16) + 32 B confirm hash = 34 B.
- **Discriminator:** `sae_confirm` sent then no `assoc_resp` **before** any RSNE/PMF check → the Confirm body is malformed. Verify txn == 2, status == 0, the send-confirm counter is present (LE16), and the confirm hash length equals the group hash (32 for P-256, not 48/64).

### T-051: Anti-clogging token placement differs by SAE method (the retry trap)

- **Source:** iwd `src/sae.c:621-624` (LOOPING inline), `:637-641` (H2E IE container), `:948-1009` (`sae_process_anti_clogging`), container format at `:975`. Status value `WLAN_STATUS_ANTI_CLOG_REQUIRED = 76` (`ieee80211.h`, confirmed).
- **Trap:** when the AP is under load it answers Commit with **status 76 (ANTI_CLOG_REQUIRED)** and a token; the STA must re-send its Commit carrying that token — **but the byte placement depends on the SAE method:**
  - **Hunting-and-pecking (LOOPING):** token is **inline**, immediately after the Group field, **before** the scalar (`src/sae.c:621-623`).
  - **H2E:** token is an **IE_TYPE_EXTENSION (255) element with Extended-ID = 93** (Anti-Clogging Token Container), placed **after** scalar+element (`src/sae.c:637-640`; parse asserts `ptr[0]==IE_TYPE_EXTENSION && ptr[2]==93` at `:975`).
  The token is 1–256 B (`src/sae.c:993`). The anti-clogging *request* body from the AP is `group (LE16)` + token (`src/sae.c:959-963`).
- **Discriminator:** `sae_commit_tx`, AP replies **status 76**, then the exchange stalls → the retry Commit put the token in the wrong place. Log status-76 receipt and whether the retry Commit used inline (H&P) vs the 255/ext-93 container (H2E). Mixing them = the AP silently drops the retry.

### T-052: Status-code + group-rejection dance — the method-mismatch stall

- **Source:** iwd `src/sae.c:1094-1224` (rx status handling); `sae_rejected_groups_append` (`:114-133`), `sae_choose_next_group` (`:151-196`), the status dispatch (`:1154-1166`), group-change guard (`:1171-1174`).
- **Trap / literal values:**
  - Commit status must be **consistent between STA and AP** — `src/sae.c:1157` `if (status != sae_status_code(sm)) return -EBADMSG`. If the STA offers H2E (126) and the AP answers hunting-and-pecking (0), or vice-versa, **the frame is silently discarded** and the exchange stalls with no error frame. This is the highest-probability "assoc never starts" cause on a mismatched-config AP.
  - Group rejection: AP answers **status 77 (UNSUPP_FINITE_CYCLIC_GROUP** — IEEE 802.11-2020 Table 9-50 **[mmpdu.h header, cross-referenced]**) → the STA appends the group to a **Rejected-Groups list** (`:1119`), picks the next group (`:1129`), and re-commits; if none remain it rejects with UNSUPP_FINITE_CYCLIC_GROUP (`:1221-1223`).
  - The peer may **not** change the group mid-exchange (`:1171` `if (l_get_le16(frame) != sm->group) … Reject`).
  - Any other nonzero status → silently discard, set the retransmit timer (`:1150-1166`).
- **Discriminator:** `sae_commit_tx` then no `sae_commit_rx` → log the received Auth **status code** and the **offered group**. Status 76 → T-051 (anti-clogging). Status 77 → group unsupported (T-053). Status 0-vs-126 disagreement → method mismatch (this row). A silent stall with *no* Auth reply at all → the AP dropped our Commit on a status/method mismatch.

### T-053: Group is 19 (P-256) only — group-negotiation dead-ends

- **Source:** iwd `src/sae.c:151-196` (`sae_choose_next_group`), `:164` (`force_default_group` → only group 19), `:177` (explicitly skips groups 21, 25, 26), `:168` (H2E requires the PT/PWE derived per group).
- **Trap:** the shipped SAE curve is **NIST P-256 = IKE group 19** (`sae.rs`). iwd's default-group mode restricts the offer to 19; groups 21/25/26 are avoided even when supported. If the target AP is configured to *require* a group the STA does not implement (e.g. group 20 / P-384, group 21 / P-521), SAE dead-ends at group selection — there is no fallback to a group the STA lacks.
- **Discriminator:** for the on-air run against the PL's AP, ensure the **AP offers/accepts group 19**. If `sae_commit_rx` carries status 77 with a group the STA can't do, log the offered-vs-supported group set; the fix is AP config (offer P-256), not code.

---

## Target 2 — Full 802.11w / PMF (ISC-2, ISC-3): BIP, IGTK, IPN, protected mgmt

This is the un-mined surface the CB-5 dispatch calls out (full PMF, not just negotiation).

### T-054: BIP-CMAC-128 MMIE — on-wire byte layout (the byte-oracle)

- **Source:** `ieee80211.h:1222-1228` (`struct ieee80211_mmie`), mac80211 `wpa.c:885-906` (MMIE construction), `wpa.c:933-936` (parse/validate). `WLAN_EID_MMIE = 76`, `WLAN_KEY_LEN_AES_CMAC = 16` (`ieee80211.h`, confirmed).
- **Trap / literal bytes** — appended to the *end* of the management frame body:
  ```
  struct ieee80211_mmie {              // ieee80211.h:1222 (18 bytes for CMAC-128)
      u8      element_id;   // = WLAN_EID_MMIE = 76
      u8      length;       // = 16   (everything after these 2 bytes: 2+6+8)
      __le16  key_id;       // IGTK key id (4 or 5), little-endian
      u8      sequence_number[6];   // IPN (6 bytes)
      u8      mic[8];       // BIP-CMAC-128 MIC (8 bytes)
  };
  ```
  Constructed at `wpa.c:889-906`: `element_id = WLAN_EID_MMIE`, `length = mmie_len - 2` (= 16), `key_id = cpu_to_le16(key->conf.keyidx)`, IPN via `bip_ipn_set64`, then `ieee80211_aes_cmac(...)`. For **CMAC-256 / GMAC** the MIC is 16 B (`struct ieee80211_mmie_16`, `ieee80211.h:1231`; `length = 24`, total 26 B) — CB-5 ships CMAC-128 so the MMIE is **18 bytes**.
- **Discriminator:** a protected broadcast mgmt frame (deauth) that should be dropped is accepted, or a good one is rejected → dump the trailing 18 B: `element_id == 76`, `length == 16`, `key_id ∈ {4,5}`. A wrong `element_id`/`length` is caught by `wpa.c:934-936` → **`RX_DROP_U_BAD_MMIE`**; put that breadcrumb on the receive path.

### T-055: BIP AAD construction — the CMAC input (silent MIC-mismatch trap)

- **Source:** mac80211 `wpa.c:823-838` (`bip_aad`), `aes_cmac.c:18` (`AAD_LEN 20`), `aes_cmac.c:22-43` (`ieee80211_aes_cmac`).
- **Trap / literal bytes:** the AES-CMAC is **not** computed over the raw frame. The AAD is exactly **20 bytes**:
  ```
  AAD[0..1]  = frame_control, with IEEE80211_FCTL_RETRY | FCTL_PM | FCTL_MOREDATA masked to 0   (wpa.c:831-836)
  AAD[2..7]  = addr1  (A1)
  AAD[8..13] = addr2  (A2)
  AAD[14..19]= addr3  (A3)
  ```
  The CMAC then runs `aes_cmac_update(AAD, 20)` then the frame body from `skb->data + 24` (after the 24-B MAC header) for `skb->len - 24` bytes, **with the trailing `mic_len` bytes treated as zero** and — for beacons only — the first 8 body bytes (Timestamp) masked to zero (`aes_cmac.c:28-43`). So the MIC covers the MMIE's own `element_id/length/key_id/IPN` but zeros its 8-B `mic` field.
- **Discriminator:** MMIE present, byte layout correct, but the MIC never matches → the AAD is wrong. Verify: FC has Retry/PwrMgt/MoreData zeroed *before* CMAC; AAD == FC‖A1‖A2‖A3 (20 B); the CMAC input includes the MMIE header but zeros the 8-B MIC field; beacons zero the Timestamp. Any of these missed = 100 % MIC-fail on otherwise-correct frames.

### T-056: IPN replay check — wire byte-order + advance-only-after-MIC

- **Source:** mac80211 `wpa.c:851-859` (`bip_ipn_swap`), `wpa.c:938-943` (replay), `wpa.c:945-956` (MIC then advance).
- **Trap:** the 6-byte IPN in the MMIE is on the wire **LSByte-first**; the receiver **byte-swaps** it (`bip_ipn_swap` reverses the 6 bytes) into a MSByte-first value before comparing:
  ```c
  bip_ipn_swap(ipn, mmie->sequence_number);          // wpa.c:938
  if (memcmp(ipn, key->u.aes_cmac.rx_pn, 6) <= 0) {   // wpa.c:940  — drop if received <= stored
      key->u.aes_cmac.replays++;
      return RX_DROP_U_REPLAY;
  }
  ...verify CMAC...  (RX_DROP_U_MIC_FAIL on mismatch, wpa.c:950-953)
  memcpy(key->u.aes_cmac.rx_pn, ipn, 6);              // wpa.c:956  — advance ONLY after MIC verifies
  ```
- **Discriminator:** valid protected mgmt dropped as replay, or a replay accepted → check (1) the IPN byte-swap (wire LSByte-first vs stored MSByte-first — a from-scratch kernel that compares without swapping will mis-order), (2) that `rx_pn` advances **only after** the MIC verifies (a bad-MIC forged frame must not poison the window). On install, mac80211 loads the seq MSB-first into `rx_pn` (`key.c:698-701`).

### T-057: IGTK KDE ≠ GTK KDE — 2-byte KeyID + 6-byte IPN (parse-misalignment trap)

- **Source:** iwd `src/eapol.c:1946-1958` (IGTK KDE parse, gated on `hs->mfp`) vs `src/eapol.c:1931-1943` (GTK KDE parse); mac80211 `key.c:1406` (`/* For IGTK we have 4 and 5 and for BIGTK - 6 and 7 */`). RSN KDE OUI-Type IGTK = `00-0F-AC-09` **[iwd handshake.c / IEEE 802.11-2020 Table 12-9, cross-referenced]** vs GTK = `00-0F-AC-01`.
- **Trap / literal bytes** — inside the encrypted Key Data of 4-way M3 (when MFP is negotiated), each wrapped as `DD | len | 00 0F AC | type`:
  ```
  IGTK KDE (type 9):   KeyID (2 B, LE, value 4 or 5)  ── eapol.c:1955  igtk_key_index = l_get_le16(igtk)
                       IPN   (6 B)                    ── eapol.c:1956-1957  igtk += 2 (skip KeyID)
                       IGTK  (16 B, BIP-CMAC-128 key)
  GTK KDE  (type 1):   keyidx/tx (1 B: idx bits 0-1, TX bit 2)  ── eapol.c:1941  bit_field(gtk[0],0,2)
                       reserved  (1 B)               ── eapol.c:1942  gtk += 2
                       GTK   (16 B)
  ```
  The IGTK KeyID is a **2-byte little-endian field** (`l_get_le16`), NOT the GTK's 1-byte keyidx, and the IGTK carries a **6-byte IPN** the GTK KDE has no equivalent of.
- **Discriminator:** PMF negotiated but a forged deauth still disconnects → the IGTK was never installed. Check M3's encrypted Key Data for the IGTK KDE (RSN type 9); parse KeyID as **2 B LE (4/5)**, then 6 B IPN, then 16 B IGTK. Parsing it with the GTK layout (1-B keyidx, no IPN) misaligns the key by 5 bytes and every subsequent BIP verify fails.

### T-058: Protected-mgmt enforcement — deauth/disassoc drop + the pre-key exception

- **Source:** mac80211 `rx.c:2556-2642` (`ieee80211_drop_unencrypted_mgmt`), called from `rx.c:3568`; robust-frame definition `ieee80211.h:2429-2459` (`_ieee80211_is_robust_mgmt_frame`).
- **Trap:** with MFP active (`test_sta_flag(rx->sta, WLAN_STA_MFP)`, `rx.c:2575`) **and a key installed**, an unprotected **unicast** robust mgmt frame (deauth/disassoc) is reported to userspace and then **`return RX_DROP_U_UNPROT_UCAST_MGMT`** — it never reaches the disconnect path (`rx.c:2576-2596`). For **multicast** robust mgmt, BIP does not use the Protected bit, so mac80211 checks the MMIE is present (`ieee80211_get_mmie_keyidx(skb) < 0` → drop, `rx.c:2600-2611`). **Robust subtypes** = **deauth + disassoc always**, plus action frames except category Public/HT/WNM-unprotected/Self-protected/UNPROT-DMG/VHT/S1G/Vendor (`ieee80211.h:2431-2455`).
  **The critical exception** (`rx.c:2585-2588`):
  ```c
  /* Permit unprotected deauth/disassoc during the 4-way HS (key installed after HS) */
  if (!rx->key)
      return RX_CONTINUE;
  ```
  An unprotected deauth is **permitted** before the PTK/IGTK is installed, so a STA can recover from a mid-handshake failure. A from-scratch kernel that drops *all* unprotected deauths (even pre-key) will hang forever on a failed handshake.
- **Discriminator (the R1 headline PASS):** post-key, a forged **unprotected** deauth must be **ignored** (STA stays associated). If it disconnects us → PMF/BIP not enforcing. Breadcrumb: `MMIE-present? + BIP-verify result + IPN + key-installed?`. If key-not-installed, the pre-key exception legitimately lets it through — that's not a failure, so the discriminator must log the key state to distinguish "PMF broken" from "still in handshake."

### T-059: MFPC/MFPR RSN capability bits — SAE mandates both (negotiation, not datapath)

- **Source:** IEEE 802.11-2020 §9.4.2.24.4 (RSN Capabilities) **[include/linux/ieee80211.h header + SME layer, cross-referenced — NOT in the mac80211 datapath]**. In the datapath these are already-resolved flags: `WLAN_STA_MFP` (`rx.c:2575`), `sdata->u.mgd.mfp != IEEE80211_MFP_DISABLED` (`key.c:1424`).
- **Trap / literal bits:** RSN Capabilities field (2 B, in the RSN IE): **bit 6 = MFPR** (Management Frame Protection *Required*), **bit 7 = MFPC** (Management Frame Protection *Capable*). **SAE (AKM-8) mandates MFPC=1 and MFPR=1** — a WPA3-SAE-only AP rejects an association whose RSNE does not set both. (Confirmed by construction: iwd only reaches BIP install when `hs->mfp` is set, `eapol.c:1946.`)
- **Discriminator:** `sae_confirm` ok then no `assoc_resp` → RSNE/PMF mismatch. Dump the sent WPA3 RSNE's RSN Capabilities: **bits 6 and 7 must both be set** for a SAE-only AP. AKM must be `00-0F-AC-08` (SAE); group+pairwise = CCMP (`00-0F-AC-04`). Missing MFPR is the single most likely "assoc rejected after SAE succeeds" cause.

---

## Target 3 — Group Key Handshake vs iwd (ISC-3): T-039 confirmed + corrected

The existing T-039 (RESEARCH-2026-06-23) was confirmed against wpa_supplicant only (iwd `src/sm.c` was unavailable then). iwd `src/eapol.c` is now fetched, so this cross-confirms it — and finds one byte-level correction.

### T-060: GKH Message 1/2 key_info — iwd raw confirm; **Install bit MUST be 0** (corrects T-039)

- **Source:** iwd `src/eapol.c:648-658` (`VERIFY_GTK_COMMON`), `:660-709` (`eapol_verify_gtk_1_of_2`), `:711-739` (`eapol_verify_gtk_2_of_2`), GTK KDE parse `:1941`.
- **Trap / literal bits** — the standalone AP-initiated 2-message Group Key Handshake (EAPOL-Key), iwd's verifier requires:
  ```
  GKH Message 1 (AP → STA), Key Information bits:            eapol.c anchor
     key_type            = 0 (Group, not Pairwise)          :649 VERIFY_GTK_COMMON
     smk_message         = 0                                :651
     request             = 0                                :653
     error               = 0                                :655
     install (bit 6)     = 0   ◀── MUST be clear            :657  if (ek->install) return false
     key_ack  (bit 7)    = 1                                :667
     key_mic  (bit 8)    = 1                                :670
     secure   (bit 9)    = 1                                :673
     encrypted_key_data (bit 12) = 1 (RSN; GTK KDE encrypted) :677
     key_length          ∈ {0, 16, 32}                      :687-689
   ⇒ key_info = 0x1383  (ack 0x80 | mic 0x100 | secure 0x200 | encr 0x1000 | version 0x03)

  GKH Message 2 (STA → AP, ACK):
     key_type=0, install=0, key_ack=0 (:718), key_mic=1 (:721), secure=1 (:724),
     encrypted_key_data=0 (:727), key_length=0 (:731),
     key_nonce / eapol_key_iv / key_rsc / reserved  all ZERO (:734-737)
  ```
  GTK KDE inside M1: `keyidx = bit_field(gtk[0], 0, 2)`, then skip 2 bytes, then GTK (`eapol.c:1941-1943`) — matches the existing T-039 GTK KDE. GKH is only accepted after the 4-way completes.
- **CORRECTION to T-039** (`RESEARCH-2026-06-23-…-FINDINGS.md:290`): that entry gave GKH M1 `key_info = 0x13C3` with **install (0x40) set**. iwd's `eapol_verify_gtk_1_of_2` **rejects** any GKH frame whose Install bit is set (`VERIFY_GTK_COMMON`, `eapol.c:657-658`). The correct value is **`0x1383` (Install = 0)**. The Install bit is set only in **4-way M3** (install the PTK); the standalone **GKH M1 has Install = 0**. A from-scratch kernel that copies M3's `Install=1` into the GKH parser/builder will reject (or emit) the wrong frame.
- **Discriminator:** broadcast RX fails after SAE association → check the GKH. M1 `key_info` must decode to **Install=0** (0x1383, not 0x13C3); the GTK KDE must be in **encrypted** Key Data (`encrypted_key_data=1`); M2 is a bare MIC'd ACK with all of nonce/iv/rsc zero. Dump `key_info` + the GTK KDE keyidx; an Install-bit set on M1 is the corrected trap.

---

## Target 4 — mt7601u IGTK/BIP path (ISC-4): software-verify, no hardware slot

### T-061: mt7601u has NO hardware BIP/IGTK — BIP MMIE + IPN run in software

- **Source:** mt7601u `main.c:303-312` (`set_key` cipher switch), `mac.c:504-528` (`mt76_mac_get_key_info`), `mac.c:480-484` (HW does not PN-validate either); mac80211 software-crypto list `key.c:207-228`.
- **Trap / literal:** the mt7601u hardware key engine supports **only** four ciphers:
  ```c
  switch (key->cipher) {                    // main.c:304-312
  case WLAN_CIPHER_SUITE_WEP40:
  case WLAN_CIPHER_SUITE_WEP104:
  case WLAN_CIPHER_SUITE_TKIP:
  case WLAN_CIPHER_SUITE_CCMP:
      break;
  default:
      return -EOPNOTSUPP;                    // ◀── BIP/AES_CMAC lands here
  }
  ```
  `mt76_mac_get_key_info` (`mac.c:513-527`) likewise maps only WEP40/WEP104/TKIP/CCMP to `MT_CIPHER_*`; everything else → `MT_CIPHER_NONE`. So **`WLAN_CIPHER_SUITE_AES_CMAC` (BIP-CMAC-128, `00-0F-AC-06`) returns `-EOPNOTSUPP`**, and mac80211 keeps the IGTK as a **software** key. Corroboration: the driver already offloads PN validation to software (`mac.c:480-484`: *"let mac80211 take care of PN validation since apparently the hardware does not support it"*).
- **This answers CB-5 ISC-3's open question directly:** on mt7601u the IGTK is **never a hardware key slot**. There is **no WCID entry for the IGTK** (contrast the GTK at WCID-0, T-036). BIP MMIE compute/verify **and** IPN replay must run on the host CPU (the kernel's own software path), over received protected management frames. CB-5's `set_key`-equivalent should return "unsupported" for the CMAC/GMAC suites and do BIP in software — exactly the mt7601u behavior. Do **not** design an IGTK-to-WCID install; it cannot work on this silicon.
- **Discriminator:** if CB-5's ISC-3 attempts an IGTK hardware key-slot install (like the GTK at WCID-0), it fails at `set_key` with `-EOPNOTSUPP`. The correct path: keep the IGTK in a software structure, verify BIP on the CPU in the RX mgmt path. Breadcrumb: on the on-air run, the IGTK-related register dump is *absent by design* — the discriminator for "BIP not enforcing" is the software BIP-verify result (T-054/T-055/T-056), not a WCID register.

---

## CB-5 line-items (code decisions implied by these traps — NOT implemented here)

Per the scope guard (host-only; note code decisions, don't implement):
1. **SAE method selection (T-052):** CB-5 must decide H2E (status 126) vs hunting-and-pecking (0) and keep the Commit status consistent with what the AP offers; a mismatch stalls silently. Recommend H2E (the modern default; the PT is derivable for group 19).
2. **Anti-clogging retry (T-051):** CB-5's Commit builder needs both token placements (inline for H&P, 255/ext-93 container for H2E) OR commit to H2E-only and implement only the container.
3. **IGTK is a software key (T-061):** CB-5 ISC-3 must NOT install the IGTK to a WCID; BIP verify is a host-CPU software path. This is a design constraint on ISC-3, not an option.
4. **GKH M1 Install=0 (T-060):** CB-5's group-key-handshake path must not require/emit the Install bit on M1 (it is a 4-way-M3-only bit).
5. **MFPR mandatory (T-059):** CB-5's WPA3 RSNE (ISC-2) must set RSN Capabilities bits 6+7; a SAE-only AP rejects assoc otherwise.

---

## Source provenance

| Source | Obtained | Path / anchors |
|--------|----------|----------------|
| iwd (SAE) | Raw `git.kernel.org/…/iwd.git/plain/` Read | `src/sae.c` (build/process commit+confirm, anti-clogging, groups), `unit/test-sae.c` (aa_commit/aa_confirm byte vectors) |
| iwd (GKH/IGTK) | Raw `git.kernel.org/…/iwd.git/plain/` Read | `src/eapol.c` (`eapol_verify_gtk_1_of_2`/`_2_of_2`, IGTK/GTK KDE parse) |
| hostap/wpa_supplicant | Raw `w1.fi/cgit/hostap/plain/` Read | `src/common/sae.c` (cross-reference) |
| Linux mac80211 | Raw `torvalds/linux/master` Read | `net/mac80211/wpa.c` (MMIE, bip_aad, IPN), `aes_cmac.c` (CMAC input), `key.c` (IGTK idx, cipher→mgmt), `rx.c` (protected-mgmt drop) |
| Linux mt7601u | Raw `torvalds/linux/master` Read | `drivers/net/wireless/mediatek/mt7601u/main.c` (set_key), `mac.c` (get_key_info, PN note) |
| Linux uAPI | Raw `torvalds/linux/master` Read | `include/linux/ieee80211.h` (`struct ieee80211_mmie{,_16}`, WLAN_EID_MMIE=76, WLAN_STATUS_* 76/126, WLAN_CIPHER_SUITE_AES_CMAC=0x000FAC06, robust-mgmt) |

All byte layouts, struct field orders, status codes, and register/cipher facts above are from **direct Read of raw source**. No WebFetch summarization was used for any byte/status/register value.

---

## Honest negatives

1. **Status code 77 (UNSUPP_FINITE_CYCLIC_GROUP)** and the SAE status enum names live in iwd's `mmpdu.h`, not in the fetch set; the numeric value is IEEE 802.11-2020 Table 9-50 and the enum is used at `sae.c:339/1036/1222`. Confirmed by *usage*, cross-referenced to the standard, not quoted from a downloaded header. (Status **76** and **126** ARE confirmed from `ieee80211.h`.)
2. **IGTK/GTK RSN KDE OUI-Types (9 / 1)** — the *inner* KeyID/IPN/IGTK layout is oracle-grade from iwd `eapol.c:1955`; the *wrapper* OUI-Type value (`00-0F-AC-09`) is from IEEE 802.11-2020 Table 12-9 / iwd `handshake.c` (`handshake_util_build_igtk_kde`, called at `eapol.c:1464` but not in the fetch set). Add iwd `src/handshake.c` to confirm the wrapper bytes verbatim if CB-5 wants the KDE builder byte-oracle'd end-to-end.
3. **RSN Capabilities MFPR/MFPC bit macros (T-059)** are in `include/linux/ieee80211.h` and the SME layer, not the mac80211 datapath; bit positions (6=MFPR, 7=MFPC) are IEEE 802.11-2020 §9.4.2.24.4, confirmed-by-construction (iwd gates BIP on `hs->mfp`) but not quoted from a downloaded macro.
4. **`NUM_DEFAULT_KEYS`/`NUM_DEFAULT_MGMT_KEYS` numeric values (4/2)** are in `net/mac80211/ieee80211_i.h` (not fetched); the **idx-4/5-is-IGTK** fact is confirmed verbatim from the mac80211 `key.c:1406` comment and the range checks at `key.c:395-398`.

---

## Summary: trap count

| Target | Traps | Oracle-grade (raw source) | Header cross-referenced |
|--------|-------|--------------------------|-------------------------|
| T1: SAE on-air (ISC-1) | T-049…T-053 (5) | 5/5 (status 77 cross-ref) | status-77 name |
| T2: Full PMF/802.11w (ISC-2) | T-054…T-059 (6) | 5/6 | T-059 RSN cap bits |
| T3: Group Key Handshake vs iwd (ISC-3) | T-060 (1) — **corrects T-039** | 1/1 | — |
| T4: mt7601u IGTK/BIP (ISC-4) | T-061 (1) | 1/1 | — |
| **Total** | **T-049…T-061 (13 traps)** | **12/13 oracle-grade** | **1 (RSN cap bits)** |

**Headline for CB-5:** the IGTK is a **software** key on mt7601u (T-061); GKH M1 has **Install=0** (T-060 corrects T-039); the on-air PASS is a **forged unprotected deauth ignored post-key** (T-058); SAE stalls silently on a **method/status mismatch** (T-052) or missing **MFPR** (T-059). All thirteen drop straight into CB-5's §5 discriminator bank.
