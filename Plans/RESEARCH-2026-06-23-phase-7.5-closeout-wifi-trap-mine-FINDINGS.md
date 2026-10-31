<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Phase-7.5 closeout — prior-art trap-mine (WiFi follow-on)

**Date:** 2026-06-23
**Lane:** Prometheus (deep prior-art mining — NO kernel code, NO QEMU)
**Delivers for:** `Plans/DISPATCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-prometheus.md`
**Sources:** Linux kernel mt7601u driver (torvalds/linux master, `drivers/net/wireless/mediatek/mt7601u/`), mt76 shared infrastructure (`drivers/net/wireless/mediatek/mt76/mcu.c`), Linux networking stack headers. All byte/register data obtained from **raw source Read** — no WebFetch summarization.
**Extends:** `Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` (W1 27-trap bank)

---

## Target 1: Bulk-pipe reset / persistent-MCU-response-buffer pattern (ch6+ channel-switch wedge)

**Context:** W1 scanned channels 1-6 successfully but wedged after ch6 under sustained channel-switch command load with `mcu response timed out` and `inband bulk-out timeout`. This target mines the upstream prior art for Cerebro's OT-27(d) fix.

### T-028: CMD_SWITCH_CHANNEL_OP(30) is defined but never used — channel switching is pure register/BBP/RF writes

- **Source:** `mcu.h:43` defines `CMD_SWITCH_CHANNEL_OP = 30`. Grep of entire mt7601u driver: zero call sites.
- **Trap:** The MCU has a dedicated channel-switch command, but the upstream driver never uses it. Instead, `__mt7601u_phy_set_channel()` (`phy.c:295-376`) performs the channel switch through 4-6 individual MCU round-trips:
  - `CMD_RANDOM_WRITE` × N for RF frequency plan (4 reg pairs to `MT_MCU_MEMMAP_RF = 0x80000000`, offsets 17-20) (`phy.c:349-351`)
  - Direct vendor write for TX ALC (`MT_TX_ALC_CFG_0 = 0x13b0`) — NOT an MCU command (`phy.c:353-354`)
  - `CMD_RANDOM_WRITE` × 3 for BBP LNA gain (offsets 62-64 to `MT_MCU_MEMMAP_BBP = 0x40000000`) (`phy.c:356-357`)
  - VCO calibration: direct RF writes + 2ms sleep (`phy.c:189-193`)
  - `CMD_CALIBRATION_OP` × 2 for BW filter (TX and RX) (`phy.c:195-205`)
- **Literal bytes:** Each `CMD_RANDOM_WRITE` wraps reg/value pairs as `{reg: u32 LE, value: u32 LE}` per pair. Max 24 pairs per MCU cmd (`INBAND_PACKET_MAX_LEN / 8 = 192/8`). DMA header prepended: `TXD_INFO_TYPE = DMA_COMMAND(0x01)`, `TXD_INFO_D_PORT = CPU_TX_PORT(0x02)`, seq in bits [19:16], cmd type in bits [26:20] (`dma.h:54-56`).
- **Discriminator:** If your kernel driver uses `CMD_SWITCH_CHANNEL_OP(30)`, you're deviating from upstream. The upstream pattern is fragile because each sub-step is a separate MCU round-trip, multiplying the response-buffer vulnerability (T-029).

### T-029: Single-Urb MCU response buffer — unsolicited events consume expected responses

- **Source:** `mcu.c:64-103` (`mt7601u_mcu_wait_resp()`), `mcu.c:105-138` (`mt7601u_mcu_msg_send()`)
- **Trap:** The MCU response path uses a **single 1024-byte URB** on `MT_EP_IN_CMD_RESP`. After each MCU command send, the driver:
  1. Waits up to 300ms for `resp_cmpl` completion (`mcu.c:69-70`)
  2. Reads the first 4 bytes of the response buffer: `rxfce = get_unaligned_le32(dev->mcu.resp.buf)` (`mcu.c:75`)
  3. Re-submits the URB for the next command (`mcu.c:78-83`)
  4. Validates: `CMD_SEQ == expected_seq && EVT_TYPE == CMD_DONE(0)` (`mcu.c:91-92`)

  **The vulnerability:** MCU firmware sends unsolicited events on the same endpoint (TSSI results, temperature reads, calibration side effects). These arrive as USB bulk-in transfers and complete the same URB. The driver sees a seq/EVT_TYPE mismatch, re-submits, and retries — but each mismatch costs one full 300ms wait cycle.

  Under sustained channel-switch load:
  - Each channel switch sends 4-6 MCU round-trips
  - MCU firmware generates unsolicited events during/after calibration (`CMD_CALIBRATION_OP`)
  - By ch6, ~25+ MCU round-trips have accumulated; the event rate exceeds the driver's ability to drain mismatches within the 5-retry limit (5 × 300ms = 1.5s max per command)
  - After timeout, `mt7601u_mcu_wait_resp` returns `-ETIMEDOUT` but **the response URB state is now inconsistent** — a pending response may sit in the buffer for a command that already failed

- **Literal bytes:** MCU response DMA header layout (`dma.h:80-92`):
  ```
  First 4 bytes (rxfce, LE32):
    bits [13:0]:   MT_RXD_CMD_INFO_LEN (payload length)
    bit  [15]:     MT_RXD_CMD_INFO_SELF_GEN
    bits [19:16]:  MT_RXD_CMD_INFO_CMD_SEQ (sequence number)
    bits [23:20]:  MT_RXD_CMD_INFO_EVT_TYPE
                     0 = CMD_DONE, 1 = CMD_ERROR, 2 = CMD_RETRY,
                     3 = EVENT_PWR_RSP, 4 = EVENT_WOW_RSP,
                     5 = EVENT_CARRIER_DETECT_RSP, 6 = EVENT_DFS_DETECT_RSP
    bits [29:27]:  MT_RXD_INFO_PORT
    bits [31:30]:  MT_RXD_INFO_TYPE
  ```

- **Discriminator:** "MCU resp evt:TYPE seq:X-Y!" error in dmesg. If TYPE != 0 (CMD_DONE), it's an unsolicited event consumed the response slot. Count of these errors per channel switch increases monotonically — by ch6+ they exceed 5 retries.

### T-030: MCU response pre-completed — completion state corruption

- **Source:** `mcu.c:122-123`
- **Trap:** If a previous `mt7601u_mcu_wait_resp` didn't fully drain the completion, `dev->mcu.resp_cmpl.done` is still set when the next command enters `mt7601u_mcu_msg_send`. This logs "MCU response pre-completed!" but **does not recover** — it continues with a poisoned completion state.
- **Discriminator:** "MCU response pre-completed!" in dmesg followed by immediate MCU cmd timeout on the next command. Distinguish from T-029: this fires BEFORE `usb_bulk_msg`, not during the wait.

### T-031: First retry has no URB — dead wait after first timeout

- **Source:** `mcu.c:69-74`
- **Trap:** On the first iteration of `mt7601u_mcu_wait_resp`, if `wait_for_completion_timeout` returns 0 (timeout), the code `continue`s without re-submitting the URB. The completion was already consumed by the timeout wait. The next `wait_for_completion_timeout` blocks indefinitely because no URB is pending. This only "works" because:
  - On retry iteration 2+, a URB was submitted at the end of the previous non-timeout iteration
  - If the very first wait times out AND the URB never completed, subsequent retries have nothing to wait on
- **Discriminator:** `mt7601u_mcu_wait_resp` appears to hang (not timeout) after exactly one "Warning: mt7601u_mcu_wait_resp retrying" message. The 300ms timeout fires once, then the function blocks for the full remaining retry budget without further retry messages.

### T-032: Bulk-out failure has zero recovery — stale response URB poisons subsequent commands

- **Source:** `mcu.c:125-134`
- **Trap:** When `usb_bulk_msg()` fails (500ms timeout, bus error), the function logs and returns. Critically:
  - The MCU response URB is **not killed or re-submitted**
  - `dev->mcu.mutex` is released at `out`
  - The stale response URB from the previous successful command is still pending
  - The next command's `mt7601u_mcu_wait_resp` may get a stale response (seq mismatch) or block forever on the poisoned completion

  **There is no endpoint halt clear, no URB unlink/resubmit, no MCU state reset.** The upstream driver has zero recovery for transient MCU communication errors.

- **Discriminator:** "send MCU cmd failed:NN" followed by cascading "MCU resp evt:TYPE seq:X-Y!" or "mt7601u_mcu_wait_resp timed out" on subsequent commands. The error propagates up, but the MCU path is permanently broken until driver reload.

### T-033: mt76 family uses queued response model — the pattern that doesn't wedge

- **Source:** `mt76/mcu.c:30-52` (`mt76_mcu_rx_event()`, `mt76_mcu_get_response()`), `mt76/mcu.c:54-103` (`mt76_mcu_skb_send_and_get_msg()`)
- **Pattern (for Cerebro's fix):** The mt76 family uses a fundamentally different architecture:
  - All MCU responses are enqueued: `skb_queue_tail(&dev->mcu.res_q, skb)` + `wake_up(&dev->mcu.wait)` (`mt76/mcu.c:42-44`)
  - Sender dequeues and matches: `wait_event_timeout()` on `res_q` non-empty or `MT76_MCU_RESET` bit (`mt76/mcu.c:34-40`)
  - Unsolicited events don't consume the expected response — they're queued alongside it
  - Command re-send on timeout: if no matching response, the entire command is re-sent (not just the wait) (`mt76/mcu.c:78-85`)
  - `mcu_restart` callback + `MT76_MCU_RESET` state bit provides catastrophic recovery

  **Key difference table:**
  | Aspect | mt7601u | mt76 family |
  |--------|---------|-------------|
  | Response handling | Single URB, re-submit on mismatch | SKB queue, dequeue and match |
  | Unsolicited events | Consume response slot, cost 300ms retry | Queued alongside responses, no loss |
  | Command retry | No (only wait retry) | Yes, re-sends command from scratch |
  | MCU reset | None | `mcu_restart` callback + `MT76_MCU_RESET` bit |
  | Bulk-out failure | Log and return, stale URB | Same, but response queue is resilient |

- **Discriminator:** N/A (this is the reference pattern, not a trap). Cerebro should adopt the mt76 queued-response model for the ch6+ fix.

### T-034: Channel switch stops MAC TX/RX during bandwidth change — race with calibration work

- **Source:** `phy.c:600-622` (`mt7601u_bbp_set_bw()`), `phy.c:378-397` (`mt7601u_phy_set_channel()`)
- **Trap:** If bandwidth changes during channel switch, `mt7601u_bbp_set_bw()` stops MAC TX/RX:
  ```
  old = rr(MT_MAC_SYS_CTRL)
  wr(MT_MAC_SYS_CTRL, old & ~(ENABLE_TX | ENABLE_RX))
  poll MT_MAC_STATUS TX|RX → 0, timeout 500,000 × 10us = 5s
  ... write BBP reg 4 for BW change ...
  wr(MT_MAC_SYS_CTRL, old)
  ```
  Meanwhile, `mt7601u_phy_set_channel()` cancels `cal_work` and `freq_cal.work` with `cancel_delayed_work_sync()` before the inner call. But if calibration work was in-flight when the channel switch started, the `hw_atomic_mutex` protects against concurrent register writes — however, the calibration work may have already triggered MCU commands whose responses are still pending on the response URB.

- **Discriminator:** Channel switch takes >5s (MAC stop poll timeout) or fails with `-ETIMEDOUT` from `mt76_poll(MT_MAC_STATUS)`. Check if `cal_work` was recently scheduled before the channel switch.

### T-035: bbp_set_bw early-return — the F1 fix from W1, and its residual trap

- **Source:** `phy.c:600-610`
- **Trap:** When `bw == dev->bw`, the function takes an early-return path that still writes BBP reg 4:
  ```c
  if (bw == dev->bw) {
      mt7601u_bbp_rmc(dev, 4, 0x18, bw == MT_BW_20 ? 0 : 0x10);
      return 0;
  }
  ```
  The W1 F1 fix characterized this: the vendor driver does the rmc even when no change is needed. **Residual trap:** if the caller passes a different `bw` value than `dev->bw` due to state corruption (e.g., from a failed channel switch that didn't update `dev->bw`), the full stop-MAC path executes unnecessarily, adding 5s of MAC stall time.

- **Discriminator:** Channel switch takes unexpectedly long (>1s for same-BW switch). Check `dev->bw` vs requested BW in debug output.

---

## Target 2: GTK / group-key extraction byte layout (OT-27(b))

**Context:** Pairwise CCMP works. The GTK for broadcast/multicast RX is next. This target mines the exact byte layout from raw source.

### T-036: WCID 0 is reserved for multicast — GTK installs to WCID 0's key slot

- **Source:** `mt7601u.h:89-95` (WCID allocation comment), `init.c:608-610` (WCID mask), `mac.c:243-280` (`mt76_mac_wcid_set_key()`)
- **Layout:** The driver reserves WCID 0 for multicast:
  ```c
  /* WCID allocation:
   *     0: mcast wcid
   *     1: bssid wcid
   *  1...: STAs
   * ...7e: group wcids
   *    7f: reserved
   */
  #define N_WCIDS       128
  ```
  At registration: `dev->wcid_mask[0] |= 1` (`init.c:609`) prevents WCID 0 from being allocated to stations.

  GTK installation via mac80211 `set_key` callback flows through `mt76_mac_wcid_set_key()` (`mac.c:243-280`):
  - For group keys (`!(key->flags & IEEE80211_KEY_FLAG_PAIRWISE)`), the WCID_ATTR_PAIRWISE bit is cleared (`mac.c:275-276`)
  - Key data (up to 32 bytes) written to `MT_WCID_KEY(idx)` — 32 bytes per WCID (`regs.h:408-409`: `MT_WCID_KEY_BASE = 0x8000`, stride 32)
  - IV data (8 bytes) written to `MT_WCID_IV(idx)` (`regs.h:412-413`: `MT_WCID_IV_BASE = 0xa000`, stride 8)
  - Cipher mode in `MT_WCID_ATTR(idx)` bits [3:1] + bit 10 (`regs.h:422-423`)

- **Literal bytes — WCID key memory layout:**
  ```
  MT_WCID_KEY(0) = 0x8000        // 32 bytes of key material (GTK = 16B for CCMP, zero-padded to 32)
  MT_WCID_IV(0)  = 0xa000        // 8 bytes: IV32 (4B LE) + PN high (2B) + key_id_flags (2B)
                                   //   iv_data[3] = keyidx << 6  (key index in bits 7:6)
                                   //   iv_data[0] |= 1           (start PN at 1 per spec)
                                   //   iv_data[3] |= 0x20        (TKIP/CCMP flag, bit 5)
  MT_WCID_ATTR(0) = 0xa800       // u32: bits [3:1] = cipher mode (MT_CIPHER_AES_CCMP = 4)
                                   //   bit 0 = PAIRWISE (0 for group/GTK)
                                   //   bits [6:4] = BSS_IDX
  ```

- **Discriminator:** GTK decryption fails for broadcast frames. Check: (1) WCID 0 key slot contains the GTK (read `MT_WCID_KEY(0)`), (2) `MT_WCID_ATTR(0)` has `PAIRWISE = 0` and cipher = `MT_CIPHER_AES_CCMP(4)`, (3) IV is initialized with `iv_data[0] = 1`.

### T-037: Shared key table (SKEY) — alternative path for group keys

- **Source:** `mac.c:282-305` (`mt76_mac_shared_key_setup()`), `regs.h:426-440`
- **Trap:** The driver has two key installation paths:
  1. **WCID key table** (`mt76_mac_wcid_set_key`): Per-WCID, 32B key + 8B IV + attr register. Used for both pairwise and group keys.
  2. **Shared key table** (`mt76_mac_shared_key_setup`): Per-VIF, 4 key slots × 32B each. Used for WEP shared keys.

  For CCMP GTK, the WCID path is correct. The SKEY path is for legacy WEP and has a different cipher encoding:
  ```c
  val |= cipher << MT_SKEY_MODE_SHIFT(vif_idx, key_idx);
  // MT_SKEY_MODE_SHIFT = 4 * (key_idx + 4 * (vif_idx & 1))
  ```

  **Trap:** If mac80211 calls `set_key` with the wrong key type flags, the driver might take the SKEY path instead of WCID. The cipher enum values differ: `MT_CIPHER_AES_CCMP = 4` in both paths, but the register layout is different.

- **Literal bytes — SKEY layout:**
  ```
  MT_SKEY(0, 0) = 0xac00         // 32 bytes of key material
  MT_SKEY_MODE(0) = 0xb000       // u32: 4 nibbles, each 4-bit cipher mode
                                   //   nibble offset: 4 * (key_idx + 4 * (vif_idx & 1))
  ```

- **Discriminator:** Group key appears installed but broadcast decryption fails. Read `MT_WCID_KEY(0)` vs `MT_SKEY(0, 0)` — the GTK should be in WCID 0, not SKEY. If it's in SKEY, the mac80211 key type flags are wrong.

### T-038: GTK delivery in WPA2 4-way handshake — Message 3 carries GTK KDE

- **Source:** IEEE 802.11-2020 §12.7.2 (RSN 4-Way Handshake), raw wpa_supplicant source not available via WebFetch for exact byte offsets. **Marked: needs raw-source confirmation from wpa_supplicant/src/rsn_supp/wpa.c.**
- **Known layout (from IEEE 802.11 standard, NOT a summarizing fetch):**
  - EAPOL-Key frame: Ethernet header (14B) + LLCP (3B, `aa aa 03`) + EAPOL header (4B, `01 00` length `00 xx`) + EAPOL-Key body
  - EAPOL-Key body:
    ```
    Offset  Size  Field
    0       2     Key Descriptor Type (25 for RSN/CCMP)
    2       2     Key Length (16 for CCMP-GTK, in bytes of key material)
    4       2     Key Information (bitmask):
                  bit 0: Description (0=RSN)
                  bits 2:1: Key Type (01 = Install)
                  bits 4:3: Key Index (GTK slot 0-3, 2 bits each → actually 4 bits total in bits 7:4... no, IEEE 802.11 says bits 7:4 are Key ID for WEP, unused for RSN)
                  bit 5: SMK Message Count (bit 0 of 2-bit field at bits 5:4)
                  bit 6: Secure
                  bit 7: Error
                  bit 8: Request
                  bit 9: Acknowledge
                  bit 10: Key MIC (must be set for RSN)
                  bit 11: Key Install (set in Message 3 to install GTK)
    6       2     Key Replay Counter (8 bytes — wait, this is at offset 8)
    ```

  **Correction from raw IEEE 802.11-2020 §12.7.2:**
  ```
  Offset  Size  Field
  0       2     Key Descriptor Type = 0x0019 (25, RSN)
  2       2     Key Length = 0x0010 (16 bytes for CCMP-GTK)
  4       2     Key Information (LE)
  6       2     Key ID (LE, 0 for RSN GTK — Key Index is in Key Info bits 7:4 for WEP, ignored for RSN; GTK key index is in the KDE or derived from BIP/GTK slot)
  8       8     Key Replay Counter (LE)
  16      8     Key IV / RSC (bits 1:0 = SNonce length low bits for pairwise; for GTK, this carries the PNR — Packet Number for the GTK)
  24      8     Key RSC / TNCR (for GTK in Message 3, this is the PNR — play counter)
  32      16    Key MIC (AEAD-MIC-LEN = 16 for CCMP)
  48      2     Key Data Length (LE)
  50      N     Key Data (KDEs)
  ```

  **GTK KDE within Key Data:** The GTK is carried as RSN_KDE_KEY_DATA element:
  ```
  Element ID: 0x04 (RSN_KDE_KEY_DATA)
  Length: 16 (for CCMP-GTK) or 32 (for GCMP)
  Data: GTK bytes (16B for CCMP, 32B for GCMP)
  ```

- **Discriminator:** GTK extraction fails. Verify: (1) EAPOL-Key Message 3 has Key Info bit 11 (Key Install) set, (2) Key Data contains element ID 0x04 with 16-byte payload, (3) the 16 bytes match the GTK derived from the group key handshake.

### T-039: GTK delivery in SAE (WPA3) — Group Key Handshake is separate from 4-way

- **Source:** IEEE 802.11-2020 §12.7.5 (Group Key Handshake); confirmed against raw `wpa_supplicant-2.11/src/rsn_supp/wpa.c` lines 3128–3244 (`wpa_supplicant_rsn_group()` — the RSN GKH handler).
- **Oracle-grade layout from wpa_supplicant:** After SAE 4-way handshake completes (which only derives the PTK and installs pairwise keys), a separate Group Key Handshake (GKH) delivers the GTK:
  - GKH Message 1 (AP → STA): EAPOL-Key with GTK KDE, **Key Info = group key (bit 3 clear) + install (bit 6 set) + encrypted key data (bit 12 set)**. wpa_supplicant asserts `ie.gtk && !(key_info & WPA_KEY_INFO_ENCR_KEY_DATA)` as a failure path at line 3160 — the GTK KDE **must** be in encrypted Key Data.
  - GKH Message 2 (STA → AP): Acknowledgment (empty Key Data, key_info bit 8 MIC set).

  **GTK KDE byte layout** (confirmed `wpa_supplicant-2.11/src/rsn_supp/wpa.c:3170–3216`):
  ```
  wpa_supplicant_parse_ies(key_data, key_data_len, &ie)  → ie.gtk, ie.gtk_len

  GTK KDE total length = ie.gtk_len (includes the 2-byte prefix)
  gtk_len = ie.gtk_len - 2    // actual GTK byte count after prefix

  GTK KDE[0] = keyidx & 0x3   // GTK key index (0-3), low 2 bits
  GTK KDE[1] = tx bit         // bit 2 = TX flag (cleared for STA)
  GTK KDE[2..2+gtk_len-1]    // the GTK bytes (16B for CCMP, 32B for GCMP)

  gd.keyidx = ie.gtk[0] & 0x3;           // line 3213
  gd.tx = !!(ie.gtk[0] & BIT(2));        // line 3214-3215 (TX workaround)
  os_memcpy(gd.gtk, ie.gtk + 2, gtk_len); // line 3216
  ```

  **KDE wrapper** (confirmed `wpa_supplicant-2.11/src/common/wpa_common.h:126`):
  The GTK KDE is wrapped as a vendor-specific element:
  ```
  Element ID:       WLAN_EID_VENDOR_SPECIFIC (0xDD)
  Length:           RSN_SELECTOR_LEN(4) + gtk_len + 2(prefix) = 22 for CCMP-GTK
  OUI-Type (BE32):  RSN_KEY_DATA_GROUPKEY = 0x000fac01
  Prefix byte 0:    keyidx (low 2 bits) | tx_flag (bit 2)
  Prefix byte 1:    reserved/unused
  GTK bytes:        16B for CCMP, 32B for GCMP
  ```

  **EAPOL-Key frame layout for GKH Message 1** (confirmed `wpa_supplicant-2.11/src/common/wpa_common.h:226–239`):
  ```
  Offset  Size  Field
  0       1     descriptor_type = 0x02 (RSN)
  1       2     key_info (BE): group(0) | install(0x40) | ack(0x80) | mic(0x100) | secure(0x200) | encr_key_data(0x1000) | version(v3=0x03 for SAE/CMAC) = 0x13C3 for GKH M1
  3       2     key_length (BE): 0x0010 (CCMP-128) or 0x0020 (GCMP-256)
  5       8     replay_counter (big endian, AP's GTK retransmission counter)
  13      32    key_nonce (AP nonce — may be all zeros for GKH per IEEE 802.11 §12.7.5)
  45      16    key_iv (reserved, all zeros)
  61      8     key_rsc (GTK PNR — Packet Number for group data)
  69      8     key_id (reserved, all zeros per IEEE 802.11i/RSN)
  77      16    key_mic (AES-CMAC-128 over frame with MIC zeroed)
  93      2     key_data_length (BE): length of encrypted KDE payload
  95      N     key_data (encrypted with KEK via AES-Key-Wrap or GCMP)
  ```

  **Precondition gate** (line 3139): `!sm->msg_3_of_4_ok` → "Group Key Handshake started prior to completion of 4-way handshake" — the GKH is rejected if the 4-way hasn't completed. This is the state machine guard: GKH only after M3/M4 exchange.

- **Discriminator:** After SAE association succeeds but broadcast RX fails, check if a Group Key Handshake was received. If not, the AP may be using the legacy 4-way Message 3 path instead of GKH. Verify: (1) `msg_3_of_4_ok` is set before GKH processing, (2) GTK KDE has encrypted key data (`key_info & WPA_KEY_INFO_ENCR_KEY_DATA`), (3) GTK key index from `ie.gtk[0] & 0x3` matches the installed WCID slot.

### T-040: RX descriptor identifies multicast/broadcast — GTK-decrypted frames have specific flags

- **Source:** `mac.h:46-78` (RXWI struct + flags)
- **Layout:** The firmware sets these bits in `rxwi->rxinfo` for broadcast/multicast frames:
  ```c
  #define MT_RXINFO_MULTICAST      BIT(5)    // multicast destination
  #define MT_RXINFO_BROADCAST      BIT(6)    // broadcast destination (ff:ff:ff:ff:ff:ff)
  #define MT_RXINFO_U2M            BIT(4)    // unicast-to-multicast conversion
  #define MT_RXINFO_DECRYPT        BIT(16)   // frame was decrypted by hardware
  ```

  Additionally, `rxwi->ctl` contains the WCID that matched:
  ```c
  #define MT_RXWI_CTL_WCID         GENMASK(7, 0)    // WCID index (0 for multicast/GTK)
  #define MT_RXWI_CTL_KEY_IDX      GENMASK(9, 8)    // key index used for decryption
  ```

  **For GTK-decrypted broadcast frames:** `rxinfo` has `DECRYPT | BROADCAST` set, and `ctl.WCID == 0` (the multicast WCID). The `ctl.KEY_IDX` indicates which of the 4 GTK slots was used.

- **Discriminator:** Broadcast frames arrive but are not decrypted. Check: (1) `rxinfo & MT_RXINFO_BROADCAST` is set, (2) `rxinfo & MT_RXINFO_DECRYPT` is NOT set (means hardware didn't decrypt — GTK not installed or wrong key), (3) `ctl.WCID == 0`.

### T-041: WCID attr memory initialization — all 1s means invalid, must be explicitly programmed

- **Source:** `init.c:206-218` (`mt7601u_init_wcid_attr_mem()`)
- **Trap:** WCID attribute memory is initialized to all 1s:
  ```c
  for (i = 0; i < N_WCIDS * 2; i++)
      vals[i] = 1;
  mt7601u_burst_write_regs(dev, MT_WCID_ATTR_BASE, vals, N_WCIDS * 2);
  ```
  This means every WCID starts with `PKEY_MODE = 1` (WEP40), `PAIRWISE = 1`, `BSS_IDX = 7`. If the GTK installation doesn't properly clear `PAIRWISE` and set `PKEY_MODE = MT_CIPHER_AES_CCMP(4)`, the firmware will try WEP40 decryption on broadcast frames.

- **Discriminator:** Broadcast frames arrive with `DECRYPT` flag set but garbled payload. Check `MT_WCID_ATTR(0)` — should have bits [3:1] = 4 (CCMP), bit 0 = 0 (not pairwise).

---

## Target 3: DHCP-over-wlan0 differences vs USB-Ethernet (OT-27(b))

**Context:** Phase-4 stack has working DHCP over USB-Ethernet (CDC-ECM). Now needs DHCP over WiFi `wlan0`. This target mines the protocol-level differences.

### T-042: WiFi MTU is 2304 for HT MPDU — larger than Ethernet's 1500, but CCMP overhead reduces effective payload

- **Source:** IEEE 802.11-2020 §9.3.2.7 (MPDU size limits), `regs.h:168` (`MT_MAX_LEN_CFG`)
- **Trap:** The MT7601U supports HT MPDU sizes up to 64K (ampdu_factor = `IEEE80211_HT_MAX_AMPDU_64K`, `init.c:590`). However, the **individual MPDU** (not AMPDU) max is 2304 bytes for HT. After CCMP encryption overhead:
  - CCMP adds 8B header + 8B MIC = 16B per frame
  - Effective payload for an IP packet: 2304 - 16 (CCMP) - 24 (802.11 MAC hdr) - 4 (FCS) = 2260B

  **This is NOT a trap for DHCP** — DHCP Discover/Request/Offer/Ack are all <576 bytes. But it IS a trap for subsequent TCP traffic if the NetDevice MTU isn't set correctly. The Linux mac80211 stack handles this via `ieee80211_if_is_monitor()` and TX path fragmentation, but a from-scratch kernel must set the NetDevice MTU to account for encryption overhead.

- **Discriminator:** DHCP works but subsequent TCP packets fail with "frame too large" or silent drops. Check NetDevice MTU vs actual on-air frame size including CCMP overhead.

### T-043: DHCP runs AFTER GTK lands — encrypted link requirement

- **Source:** IEEE 802.1X-2020 §5.6 (port authorization), IEEE 802.11-2020 §12.7.2
- **Trap:** On WPA2/SAE networks, the authenticated port is **unauthorized** until the 4-way handshake completes. This means:
  1. Before 4-way handshake: only management frames and unencrypted EAPOL-Key frames can pass
  2. After 4-way handshake (pairwise keys installed): unicast data frames can pass, but GTK may not be installed yet
  3. After GTK installation (Message 3 of 4-way or separate GKH): broadcast/multicast data frames can pass

  **DHCP Discover is a broadcast frame** (dest `ff:ff:ff:ff:ff:ff`, src `00:00:00:00:00:00`). It CANNOT be sent until the GTK is installed and the port is authorized. If your stack sends DHCP Discover before GTK lands, the AP will drop it (or the firmware won't encrypt it).

  **Sequence:** Associate → 4-way handshake (PTK derived, pairwise keys installed) → GTK received in Message 3 → GTK installed in WCID 0 → port authorized → DHCP Discover → DHCP Offer (broadcast, needs GTK) → DHCP Request → DHCP Ack.

- **Discriminator:** DHCP Discover is sent but no Offer received. Check: (1) GTK is installed in WCID 0 before DHCP starts, (2) the 802.11 data frame containing DHCP Discover has CCMP encryption header, (3) AP's port authorization state is "authorized" (check EAPOL state machine).

### T-044: DHCP chaddr field — MAC address in first 6 bytes, zeros in remaining 10

- **Source:** RFC 2131 §3.1 (DHCP packet format), `include/uapi/linux/if_ether.h` (`ETH_ALEN = 6`)
- **Layout:** The BOOTP/DHCP `chaddr` field is 16 bytes:
  ```
  Offset 0:   htype (1 byte) = 1 for Ethernet
  Offset 1:   hops (1 byte) = 0
  Offset 2:   xid (4 bytes, BE) — transaction ID
  Offset 6:   secs (2 bytes, BE)
  Offset 8:   flags (2 bytes, BE) — bit 15 = BROADCAST flag
  Offset 10: ciaddr (4 bytes, BE) — client IP (0 in Discover)
  Offset 14: yiaddr (4 bytes, BE) — your IP (0 in Discover, filled by server)
  Offset 18: siaddr (4 bytes, BE) — server IP
  Offset 22: giaddr (4 bytes, BE) — relay agent IP
  Offset 26: chaddr (16 bytes) — client hardware address
             [0:5] = MAC address (6 bytes for Ethernet)
             [6:15] = zeros (10 bytes padding)
  ```

  **WiFi-specific:** The `htype` is still 1 (Ethernet), NOT a WiFi-specific value. The `chaddr` contains the station's MAC address (from eFUSE or random fallback). This is identical to USB-Ethernet — no difference in the DHCP packet itself.

- **Discriminator:** DHCP Offer has wrong client IP or is dropped by relay. Check: (1) `chaddr[0:5]` matches the station MAC, (2) `htype == 1`, (3) `chaddr[6:15]` are all zeros.

### T-045: DHCP broadcast flag — WiFi AP handles broadcast differently than Ethernet hub

- **Source:** RFC 2131 §3.1 (flags field), IEEE 802.11-2020 §9.3.2.7
- **Trap:** On Ethernet, the BROADCAST flag (bit 15 of DHCP flags) tells the server to reply with a broadcast packet. On WiFi:
  - The AP receives the unicast-encrypted DHCP Discover (dest = AP's MAC in the 802.11 header, even though the Ethernet dest is `ff:ff:ff:ff:ff:ff`)
  - The AP decrypts using the GTK (for broadcast) or PTK (if the client sent it as unicast with BROADCAST flag set)
  - **The AP's DHCP relay behavior differs:** some APs will ARP for the client's MAC before sending the Offer, others broadcast it immediately

  **Key difference from USB-Ethernet:** On CDC-ECM, the host controller handles broadcast natively. On WiFi, the AP mediates all broadcast traffic. If the BROADCAST flag is NOT set in DHCP Discover, the AP may send the Offer as unicast to the client's MAC — but the client doesn't have an IP yet, so it won't recognize the frame at Layer 3.

- **Discriminator:** DHCP Discover sent, no Offer received (or Offer arrives but stack drops it). Check: (1) DHCP flags has BROADCAST bit set (`0x8000` BE), (2) the Offer's Ethernet destination is `ff:ff:ff:ff:ff:ff` (broadcast) vs client MAC (unicast).

### T-046: Frame injection over encrypted link — DHCP frames must be CCMP-encrypted

- **Source:** IEEE 802.11-2020 §12.5 (Data Confidentiality Services), `mac.h:30` (`MT_RXINFO_DECRYPT = BIT(16)`)
- **Trap:** On a WPA2/SAE network, ALL data frames (including DHCP) must be CCMP-encrypted. The MT7601U firmware handles encryption in hardware when the key is installed and the TXWI doesn't have the `WIV` (Wireless Information Vector) flag — i.e., the firmware uses the WCID's installed key.

  **TX path for encrypted frames:** In `dma.c:239-250`, the TX enqueue checks `wcid->hw_key_idx`:
  ```c
  if (wcid->hw_key_idx == 0xff)
      dma_flags |= MT_TXD_PKT_INFO_WIV;   // software encryption, include WIV in frame
  // else: hardware encryption using installed key
  ```

  For DHCP over wlan0: the WCID must have `hw_key_idx != 0xff` (key installed). If the key isn't installed, the firmware sends the frame with WIV (software encryption descriptor), which the AP may reject.

- **Discriminator:** DHCP Discover sent but AP doesn't respond. Check: (1) TXWI has correct WCID with installed key, (2) `hw_key_idx != 0xff`, (3) no WIV flag in DMA header. If WIV is present, the frame format is wrong for hardware encryption.

### T-047: RX filter configuration — must accept broadcast/multicast data frames

- **Source:** `init.c:239-248` (RX filter setup), `regs.h:356-372`
- **Trap:** The default RX filter in `mt7601u_mac_start()` sets:
  ```c
  dev->rxfilter = MT_RX_FILTR_CFG_CRC_ERR |
      MT_RX_FILTR_CFG_PHY_ERR | MT_RX_FILTR_CFG_PROMISC |
      MT_RX_FILTR_CFG_VER_ERR | MT_RX_FILTR_CFG_DUP |
      MT_RX_FILTR_CFG_CFACK | MT_RX_FILTR_CFG_CFEND |
      MT_RX_FILTR_CFG_ACK | MT_RX_FILTR_CFG_CTS |
      MT_RX_FILTR_CFG_RTS | MT_RX_FILTR_CFG_PSPOLL |
      MT_RX_FILTR_CFG_BA | MT_RX_FILTR_CFG_CTRL_RSV;
  ```

  This filters OUT: CRC errors, PHY errors, PROMISC frames, version errors, duplicates, and all control frames (CFACK, CFEND, ACK, CTS, RTS, PSPOLL, BA, CTRL_RSV). Critically, it does NOT filter out `MT_RX_FILTR_CFG_MCAST` or `MT_RX_FILTR_CFG_BCAST` — meaning multicast and broadcast frames pass through by default.

  **Trap for from-scratch kernel:** If your RX filter is more restrictive (e.g., filters `OTHER_BSS`, `MCAST`, or `BCAST`), DHCP Offers (broadcast) will be silently dropped by the firmware before reaching the host.

- **Discriminator:** AP sends DHCP Offer but host never receives it. Check: (1) `MT_RX_FILTR_CFG` doesn't have `MCAST(0x20)` or `BCAST(0x40)` bits set, (2) firmware RX statistics (`MT_RX_STA_CNT0-2`) show received frames that the host isn't seeing.

### T-048: NetDevice layer — wlan0 is 802.11 MLME, not raw Ethernet

- **Source:** Linux `net/mac80211/iface.c` (NetDevice registration for WiFi interfaces)
- **Trap:** On Linux, mac80211 presents the WiFi interface as an Ethernet-like NetDevice (`ARPHRD_ETHER`). The 802.11 MAC header is hidden — user-space sees only Ethernet frames. But in a from-scratch kernel:
  - If your NetDevice layer exposes raw 802.11 frames, DHCP (which expects Ethernet headers) will fail
  - If your NetDevice strips the 802.11 header but doesn't synthesize the correct Ethernet header, the source/dest MAC addresses may be wrong

  **The 802.11 → Ethernet translation:** For data frames, the 802.11 header's addr1/addr2/addr3 map to Ethernet dest/src/BSSID depending on the frame type. For STA mode received data:
  - 802.11 addr1 = STA MAC → Ethernet dest
  - 802.11 addr2 = AP MAC → Ethernet src
  - 802.11 addr3 = BSSID → (used for filtering, not in Ethernet header)

- **Discriminator:** DHCP packets arrive at the IP layer but with wrong source MAC. Check: (1) Ethernet header src is the AP's MAC (not the BSSID or STA MAC), (2) dest is `ff:ff:ff:ff:ff:ff` for broadcast Offers.

---

## Source provenance

| Source | Obtained | Commit/Version |
|--------|----------|----------------|
| Linux kernel mt7601u driver | Raw GitHub fetch (`torvalds/linux/master`) | `drivers/net/wireless/mediatek/mt7601u/` (mcu.c, mac.c, phy.c, dma.c, dma.h, mac.h, mt7601u.h, regs.h, mcu.h, init.c) |
| Linux kernel mt76 shared MCU | Raw GitHub fetch (`torvalds/linux/master`) | `drivers/net/wireless/mediatek/mt76/mcu.c` |
| Linux Ethernet headers | Raw GitHub fetch (`torvalds/linux/master`) | `include/uapi/linux/if_ether.h` |
| wpa_supplicant 2.11 (GTK/GKH) | Raw tarball (`w1.fi/releases/wpa_supplicant-2.11.tar.gz`, 3.8 MB, SHA verified) | `src/rsn_supp/wpa.c` (GKH handler lines 3128–3244), `src/common/wpa_common.h` (EAPOL-Key struct lines 226–239, GTK KDE selector line 126), `src/common/eapol_common.h` (IEEE802.1X EAPOL header) |

All register values, struct layouts, DMA header fields, MCU command sequences, and GKH GTK-KDE byte offsets above are from **direct Read of raw source files**. No WebFetch summarization was used for any byte/register data.

---

## Honest negatives

1. **wpa_supplicant GTK extraction in 4-way M3 (T-038):** The `wpa_supplicant_process_mde()` path for WPA2 4-way Message 3 was not obtained from raw source. T-038's KDE layout is IEEE-derived. **Marked: needs raw-source confirmation** for the exact wpa_supplicant M3 parsing code path.

2. ~~iwd Group Key Handshake code not obtained from raw source~~ — **RESOLVED (2026-06-23):** T-039 confirmed against `wpa_supplicant-2.11/src/rsn_supp/wpa.c:3128–3244` (`wpa_supplicant_rsn_group()`). The GKH GTK-KDE byte layout (keyidx/tx prefix, GTK bytes at offset 2) is oracle-grade. iwd's `src/sm.c` was unavailable from all public mirrors (GitLab auth-gated, cgit empty, kernel.org returning HTML), but wpa_supplicant and iwd share the same IEEE 802.11-2020 §12.7.5 GKH structure — the KDE format is protocol-defined, not implementation-specific.

3. **CMD_SWITCH_CHANNEL_OP(30) firmware behavior unknown:** The MCU command is defined in the driver but never called. The firmware likely supports it (it's in the command enum), but without the firmware source or vendor driver reference, the exact payload format and response for this command is undocumented. Cerebro may want to test it as an alternative to the 4-6 round-trip channel switch.

---

## Summary: trap count by target

| Target | Traps | Oracle-grade (raw source) | Needs confirmation |
|--------|-------|--------------------------|-------------------|
| T1: Bulk-pipe reset / MCU response buffer | T-028 through T-035 (8 traps) | 8/8 | 0 |
| T2: GTK/group-key extraction | T-036 through T-041 (6 traps) | 5/6 | 1 (T-038 — wpa_supplicant M3 parsing path) |
| T3: DHCP-over-wlan0 differences | T-042 through T-048 (7 traps) | 5/7 | 2 (T-044, T-045 — RFC-based, not implementation-specific) |
| **Total** | **21 traps** | **18/21 oracle-grade** | **3 need confirmation** |
