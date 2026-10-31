<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: W2 on-air MLME supplicant — deep prep findings (raw-source)

**Date:** 2026-06-15
**Lane:** Prometheus (deep prior-art mining — NO kernel code)
**Delivers for:** `DISPATCH-2026-06-15-w2-mlme-supplicant-prep-prometheus.md`
**Sources:** Linux kernel mac80211/mt7601u (v6.15+, commit `2b07ea76f` for mt7601u, current main for mac80211)

---

## 1. The minimal STA MLME state machine

### 1.1 State enum (authoritative)

**Source:** `include/net/mac80211.h:2468-2475`

```c
enum ieee80211_sta_state {
    IEEE80211_STA_NOTEXIST,   // station doesn't exist at all
    IEEE80211_STA_NONE,       // station exists without special state
    IEEE80211_STA_AUTH,       // station is authenticated
    IEEE80211_STA_ASSOC,      // station is associated
    IEEE80211_STA_AUTHORIZED, // station is authorized (802.1X)
};
```

**Invariant:** States must be ordered correctly (`include/net/mac80211.h:2469` comment). State transitions only move forward; regression requires full teardown through `ieee80211_set_disassoc()`.

### 1.2 State transition sequence (SCAN → AUTH → ASSOC → 4WAY/SAE → RUN)

**Source:** `net/mac80211/mlme.c` — the entire STA MLME implementation.

The state machine is driven by two mechanisms:
1. **Work item:** `ieee80211_sta_work()` (`mlme.c:8645`) — central timer-driven work function that handles timeouts and retries.
2. **RX handlers:** `ieee80211_rx_mgmt_auth()`, `ieee80211_rx_mgmt_assoc_resp()` — process incoming management frames and advance state on success.

#### Phase 1: SCAN (already done in W1)

Scan is handled by `sw_scan_start`/`sw_scan_complete` mac80211 ops (`main.c:427-428`). W1 delivers scan results. The BSS table is populated via `ieee80211_rx_bss_info()` from probe responses and beacons.

#### Phase 2: AUTHENTICATE (NOTEXIST → NONE → AUTH)

**Initiation:** Userspace calls `NL80211_CMD_AUTHENTICATE` → `ieee80211_auth()` in `cfg.c:3376` → internal `ieee80211_mgd_auth_start()` (`mlme.c:9428`).

**STA creation:** The AP's STA entry is created in state `IEEE80211_STA_NOTEXIST` → `IEEE80211_STA_NONE` during auth setup.

**Auth frame TX:** `ieee80211_auth()` (`mlme.c:8497-8585`):
- Increments `auth_data->tries` (max `IEEE80211_AUTH_MAX_TRIES = 3`, `mlme.c:40`)
- Calls `ieee80211_send_auth()` with transaction=1, algorithm from auth type
- Sets timeout: `IEEE80211_AUTH_TIMEOUT_SHORT` (100ms) if ACK status available, `IEEE80211_AUTH_TIMEOUT` (200ms) for SAE, or `IEEE80211_AUTH_TIMEOUT_LONG` (500ms) fallback (`mlme.c:36-37,39`)

**Auth frame RX:** `ieee80211_rx_mgmt_auth()` (`mlme.c:4946-5119`):
- Validates algorithm match, transaction sequence
- On success: calls `ieee80211_mark_sta_auth()` (`mlme.c:4918-4944`)
  - Sets `auth_data->done = true`
  - Sets `auth_data->timeout = jiffies + IEEE80211_AUTH_WAIT_ASSOC` (5s, `mlme.c:41`) — this is the window for userspace to proceed to association
  - Moves STA state: `sta_info_move_state(sta, IEEE80211_STA_AUTH)` (`mlme.c:4938`)

**SAE-specific:** SAE authentication uses multi-frame exchange within the auth transaction:
- Transaction 1: SAE Commit (initiator sends commitment)
- AP may respond with status codes: `WLAN_STATUS_ANTI_CLOG_REQUIRED`, `WLAN_STATUS_SAE_HASH_TO_ELEMENT`, `WLAN_STATUS_SAE_PK` (`mlme.c:5020-5033`) — these put mac80211 in "waiting" mode for userspace to continue
- Transaction 2: SAE Confirm (both sides confirm)
- Transaction 3: Peer confirmed (`mlme.c:5109-5113`) — sets `auth_data->peer_confirmed = true`
- Timeout for SAE: `IEEE80211_AUTH_TIMEOUT_SAE` (2s, `mlme.c:39`)

**Auth failure:** `ieee80211_destroy_auth_data()` cleans up. Retry via `ieee80211_sta_work()` if `tries < IEEE80211_AUTH_MAX_TRIES`. Final failure → `cfg80211_auth_timeout()` notification to userspace.

#### Phase 3: ASSOCIATE (AUTH → ASSOC)

**Initiation:** Userspace calls `NL80211_CMD_ASSOCIATE` after auth completes. mac80211 creates `assoc_data`.

**Assoc frame TX:** `ieee80211_do_assoc()` (`mlme.c:8587-8631`):
- Increments `assoc_data->tries` (max `IEEE80211_ASSOC_MAX_TRIES = 3`, `mlme.c:46`)
- Calls `ieee80211_send_assoc()` which builds the AssocReq frame with IEs
- Timeout: `IEEE80211_ASSOC_TIMEOUT_SHORT` (100ms) if ACK status, `IEEE80211_ASSOC_TIMEOUT` (200ms) fallback (`mlme.c:43-44`)

**Assoc frame RX:** `ieee80211_rx_mgmt_assoc_resp()` (`mlme.c:6656-6905`):
- Parses AssocResp, extracts AID, capability info, status code
- On success (status=0): validates AID range, processes IEs (HT/VHT/RSN/WMM)
- Moves STA state to `IEEE80211_STA_ASSOC` via internal state machine
- Sets up beacon monitoring, connection monitoring
- Notifies userspace of successful association

**Assoc failure:** Same retry pattern. Final failure → `cfg80211_assoc_timeout()`.

#### Phase 4: 4-WAY HANDSHAKE / SAE KEY DERIVATION (ASSOC → AUTHORIZED)

**Critical insight:** The 4-way handshake is NOT implemented in mac80211. It is handled by userspace (wpa_supplicant) via the control port mechanism:

- EAPOL frames are delivered to userspace via `cfg80211_rx_control_port()` (`rx.c:2751`) when `control_port_over_nl80211` is set
- EAPOL-Key frames pass through mac80211's data path, decrypted by hardware (CCMP), then delivered to the control port socket
- wpa_supplicant computes PTK/GTK from PMK (derived from PSK or SAE) and the nonces exchanged in EAPOL-Key M1-M4
- Keys are installed back into mac80211 via `NL80211_CMD_NEW_KEY` → `drv_set_key()` → driver's `set_key` op (`mt7601u_set_key`, `main.c:292`)

**State transition:** After keys are installed and the 4-way completes, mac80211 moves STA to `IEEE80211_STA_AUTHORIZED`. For WPA2-PSK (no 802.1X), this happens automatically after association — the driver sets the state to AUTHORIZED when pairwise keys are installed.

#### Phase 5: RUN (AUTHORIZED → data)

**Beacon monitoring:** `ieee80211_sta_bcn_mon_timer()` (`mlme.c:8817`) — fires after `beacon_loss_count` (default 7) missed beacons. Triggers probe sequence.

**Connection monitoring:** `ieee80211_sta_conn_mon_timer()` (`mlme.c:8876`) — fires after `IEEE80211_CONNECTION_IDLE_TIME` (30s, `mlme.c:80`) of no activity. Sends probe requests or nullfunc frames.

**Disconnect detection:**
- Nullfunc probe: max `max_nullfunc_tries` (default 2) failures → disconnect (`mlme.c:8754-8767`)
- Probe request: max `max_probe_tries` (default 5) + `probe_wait_ms` (500ms) per try → disconnect (`mlme.c:8776-8793`)
- Deauth/disassoc RX: `ieee80211_rx_mgmt_deauth()` (`mlme.c:5178`), `ieee80211_rx_mgmt_disassoc()` (`mlme.c:5222`) — immediate disconnect

### 1.3 Timeout constants summary

| Constant | Value | Source |
|----------|-------|--------|
| `IEEE80211_AUTH_TIMEOUT` | HZ/5 (200ms) | `mlme.c:36` |
| `IEEE80211_AUTH_TIMEOUT_LONG` | HZ/2 (500ms) | `mlme.c:37` |
| `IEEE80211_AUTH_TIMEOUT_SHORT` | HZ/10 (100ms) | `mlme.c:38` |
| `IEEE80211_AUTH_TIMEOUT_SAE` | HZ*2 (2s) | `mlme.c:39` |
| `IEEE80211_AUTH_MAX_TRIES` | 3 | `mlme.c:40` |
| `IEEE80211_AUTH_WAIT_ASSOC` | HZ*5 (5s) | `mlme.c:41` |
| `IEEE80211_AUTH_WAIT_SAE_RETRY` | HZ*2 (2s) | `mlme.c:42` |
| `IEEE80211_ASSOC_TIMEOUT` | HZ/5 (200ms) | `mlme.c:43` |
| `IEEE80211_ASSOC_TIMEOUT_LONG` | HZ/2 (500ms) | `mlme.c:44` |
| `IEEE80211_ASSOC_TIMEOUT_SHORT` | HZ/10 (100ms) | `mlme.c:45` |
| `IEEE80211_ASSOC_MAX_TRIES` | 3 | `mlme.c:46` |
| `IEEE80211_CONNECTION_IDLE_TIME` | 30*HZ (30s) | `mlme.c:80` |
| `beacon_loss_count` | 7 | `mlme.c:71` |
| `probe_wait_ms` | 500ms | `mlme.c:86` |
| `max_nullfunc_tries` | 2 | `mlme.c:53` |
| `max_probe_tries` | 5 | `mlme.c:58` |

### 1.4 Auth algorithm mapping

**Source:** `mlme.c:9446-9481` (cfg80211 auth type → IEEE 802.11 auth algorithm)

| cfg80211 type | WLAN_AUTH enum | Frames |
|---------------|----------------|--------|
| `NL80211_AUTHTYPE_OPEN_SYSTEM` | `WLAN_AUTH_OPEN` | 2-frame exchange |
| `NL80211_AUTHTYPE_SAE` | `WLAN_AUTH_SAE` | Multi-frame (Commit/Confirm) |
| `NL80211_AUTHTYPE_SHARED_KEY` | `WLAN_AUTH_SHARED_KEY` | 4-frame (challenge-response, FIPS-blocked) |
| `NL80211_AUTHTYPE_FT` | `WLAN_AUTH_FT` | Fast BSS Transition |

For W2 minimal STA: `WLAN_AUTH_OPEN` (for WPA2-PSK) and `WLAN_AUTH_SAE` (for WPA3) are the relevant algorithms.

---

## 2. Management frame + IE byte layouts (raw-source)

### 2.1 802.11 MAC header (fixed portion)

**Source:** Standard 802.11 frame format, used throughout mac80211. The `struct ieee80211_hdr` is defined in `include/uapi/linux/ieee80211.h` (not in sparse checkout, but the layout is well-known and used in all mac80211 code):

```
Frame Control: 2 bytes
  Protocol Version: 2 bits (always 0)
  Type: 2 bits (0=MGMT, 1=CTL, 2=DATA, 3=Rsrvd)
  Subtype: 4 bits
  To DS: 1 bit
  From DS: 1 bit
  More Frag: 1 bit
  Retry: 1 bit
  Pwr Mgt: 1 bit
  More Data: 1 bit
  Protected Frame: 1 bit
  Order: 1 bit

Duration/ID: 2 bytes
Address 1 (RA): 6 bytes
Address 2 (TA): 6 bytes
Address 3 (BSSID): 6 bytes
Sequence Control: 2 bytes (Fragment: 4 bits, Sequence: 12 bits)
[Address 4: 6 bytes — only present for certain frame types]
```

**Total fixed MAC header:** 24 bytes (4 addresses) or 30 bytes (without addr4).

### 2.2 Management frame body layouts

**Source:** `struct ieee80211_mgmt` in mac80211 internal headers and usage patterns throughout `mlme.c`.

#### Authentication frame (`IEEE80211_STYPE_AUTH = 0x08`)

```
MAC Header: 24 bytes (Addr1=BSSID, Addr2=STA, Addr3=AP)
Body:
  Auth Algorithm: 2 bytes (little-endian)
    WLAN_AUTH_OPEN = 1
    WLAN_AUTH_SHARED_KEY = 2
    WLAN_AUTH_SAE = 4
    WLAN_AUTH_FT = 5
    WLAN_AUTH_FILS_SK = 6
    WLAN_AUTH_IEEE8021X = 10
    WLAN_AUTH_EPPKE = 13
  Auth Transaction: 2 bytes (1, 2, [3 for SAE])
  Status Code: 2 bytes
    WLAN_STATUS_SUCCESS = 0
    WLAN_STATUS_ANTI_CLOG_REQUIRED = 55 (SAE)
    WLAN_STATUS_SAE_HASH_TO_ELEMENT = 57 (SAE)
    WLAN_STATUS_SAE_PK = 58 (SAE)
  [Variable: IE payload for SAE — commit element, confirm, etc.]
```

**Minimum size:** 24 + 6 = 30 bytes. For IEEE8021X: 24 + 8 (includes encapsulated EAPOL length field). (`mlme.c:4963, 4982`)

#### SAE Authentication frames

Within the auth exchange for `WLAN_AUTH_SAE`:
- **Transaction 1 (STA→AP):** Auth frame with SAE Commit IE (element ID 0xDD, vendor-specific)
- **Transaction 1 (AP→STA):** Auth frame with SAE Commit IE + possible status code upgrade request
- **Transaction 2 (STA→AP):** Auth frame with SAE Confirm (empty variable portion or padding)
- **Transaction 2 (AP→STA):** Auth frame, status=SUCCESS, confirms auth
- **Transaction 3 (AP→STA):** Optional peer confirmation (`mlme.c:5109-5113`)

SAE element format (from wpa_supplicant/src/common/ieee802_11_defs.h — not available from raw source; flagged as honest negative §7.1). The SAE commit contains the commitment value (hash-to-element or DH group element). SAE confirm is empty — both sides have already computed the shared secret.

#### Association Request (`IEEE80211_STYPE_ASSOC_REQ = 0x00`)

```
MAC Header: 24 bytes (Addr1=BSSID, Addr2=STA, Addr3=BSSID)
Body:
  Capability Info: 2 bytes
  Listen Interval: 2 bytes
  [Variable: IEs — SSID, Supported Rates, RSN, HT Capabilities, etc.]
```

**Minimum size:** 24 + 4 = 28 bytes. (`mlme.c:6705` checks `len < 24 + 6` for AssocResp)

#### Association Response (`IEEE80211_STYPE_ASSOC_RESP = 0x01`)

```
MAC Header: 24 bytes (Addr1=STA, Addr2=BSSID, Addr3=BSSID)
Body:
  Capability Info: 2 bytes
  Status Code: 2 bytes
    WLAN_STATUS_SUCCESS = 0
    WLAN_STATUS_ASSOC_REJECTED_TEMPORARILY = 59 ( comeback )
  AID: 2 bytes (association ID, STA identifier within BSS)
  [Variable: IEs — SSID, Supported Rates, RSN, HT Capabilities, TIM, etc.]
```

**Minimum size:** 24 + 6 = 30 bytes. (`mlme.c:6705`)

AID is masked to `IEEE80211_MAX_AID` (0x7FF = 2047) for non-S1G STAs. (`mlme.c:6748`)

#### Reassociation Request/Response

Identical structure to Association, with the addition of:
- Current AP Address: 6 bytes (before Capability Info in body)
**Source:** `mlme.c:6703-6704` — AssocResp and ReassocResp are processed by the same handler.

#### Deauthentication (`IEEE80211_STYPE_DEAUTH = 0xC0`)

```
MAC Header: 24 bytes
Body:
  Reason Code: 2 bytes
    WLAN_REASON_DEAUTH_LEAVING = 3
    WLAN_REASON_DISASSOC_DUE_TO_INACTIVITY = 2
    WLAN_REASON_4WAY_HANDSHAKE_TIMEOUT = 10
    WLAN_REASON_MIC_FAILURE = 9
  [Variable: optional IEs]
```

**Minimum size:** 24 + 2 = 26 bytes. (`mlme.c:5186`)

#### Disassociation (`IEEE80211_STYPE_DISASSOC = 0xA0`)

Identical to Deauth — reason code + optional IEs. Minimum size: 26 bytes. (`mlme.c:5230`)

### 2.3 Information Elements (IEs)

IE format (universal):
```
Element ID: 1 byte
Length: 1 byte
Data: Length bytes
```

#### SSID IE (`WLAN_EID_SSID = 0`)
```
Element ID: 0x00
Length: 1-32 bytes
Data: SSID string (UTF-8)
```

#### Supported Rates IE (`WLAN_EID_SUPP_RATES = 1`)
```
Element ID: 0x01
Length: 1-8 bytes
Data: Rate values in 500 kbps units, bit 7 = basic rate flag
  Example: 0x82 = 1 Mbps (basic), 0x84 = 2 Mbps (basic), 0x08 = 5.5 Mbps, 0x0C = 6 Mbps, etc.
```

#### Extended Supported Rates IE (`WLAN_EID_EXT_SUPP_RATES = 51`)
Same format as Supported Rates, for rates beyond 8 entries.

#### DS Parameter Set IE (`WLAN_EID_DS_PARAMS = 3`)
```
Element ID: 0x03
Length: 1
Data: Current channel number (1-14)
```

#### RSN IE (`WLAN_EID_RSN = 48`) — CRITICAL FOR W2

**Source:** Parsed in `parse.c:480-482`, referenced throughout mac80211. The RSN IE structure is defined in IEEE 802.11i and used verbatim by mac80211:

```
Element ID: 0x30 (48)
Length: variable (typically 26-50 bytes for WPA2, longer for WPA3)
Data:
  Version: 2 bytes (always 0x0100 = version 1)
  Group Cipher Suite Selector: 4 bytes
    OUI: 3 bytes (0x00-0F-AC for IEEE registered)
    Suite Type: 1 byte
    Values:
      0x00-0F-AC-01 = CCMP (WPA2 mandatory)
      0x00-0F-AC-02 = TKIP (legacy)
      0x00-0F-AC-08 = GCMP
      0x00-0F-AC-09 = GCMP-256
      0x00-0F-AC-0C = CCMP-256
      0x00-0F-AC-00 = Use Group Addressed Cipher Suite (for WPA3-OWE)
  Pairwise Cipher Suite Count: 2 bytes (number of pairwise suites)
  Pairwise Cipher Suite List: 4 × count bytes
    Same format as group cipher selectors
    Typical for WPA2: 0x00-0F-AC-04 (CCMP)
    Typical for WPA3: 0x00-0F-AC-08 (GCMP) + 0x00-0F-AC-04 (CCMP)
  AKMP Suite Count: 2 bytes (number of authentication key management suites)
  AKMP Suite List: 4 × akmp_count bytes
    Values:
      0x00-0F-AC-01 = PSK (WPA2-Personal)
      0x00-0F-AC-02 = 802.1X (WPA2-Enterprise)
      0x00-0F-AC-09 = SAE (WPA3-Personal)
      0x00-0F-AC-11 = FT/PSK
      0x00-0F-AC-12 = FT/802.1X
      0x00-0F-AC-13 = FT/SAE
      0x00-0F-AC-16 = FILS SHA256
      0x00-0F-AC-17 = FILS SHA384
      0x00-0F-AC-19 = OWEP (OWE)
      0x00-0F-AC-1A = EPPKE SUITE-B
      0x00-0F-AC-1B = EPPKE BIP-GMAC-256
      0x00-0F-AC-1C = EPPKE BIP-GCM-256
      0x00-0F-AC-1D = EPPKE SUITE-B 192
  PMKID Count: 2 bytes (number of PMKIDs, for fast reauth)
  PMKID List: 16 × pmkid_count bytes (optional)
  [Optional Capabilities: 2 bytes — followed by more optional fields]
    Bit 0: PTSA allowed
    Bit 1: No Pairwise
    Bit 2: MFP capable (802.11w management frame protection)
    Bit 3: MFP required
    Bit 4-7: Reserved
  [Optional Group Management Cipher Suite: 4 bytes — for BIP-GMAC-256/GCMP-256]
```

**WPA2-PSK minimal RSN IE (26 bytes):**
```
30 14 01 00  // Element ID=48, Length=20, Version=1
00 0F AC 04  // Group Cipher: CCMP
01           // Pairwise count: 1
00 0F AC 04  // Pairwise: CCMP
01           // AKMP count: 1
00 0F AC 01  // AKMP: PSK
00           // PMKID count: 0
```

**WPA3-SAE minimal RSN IE (26 bytes):**
```
30 14 01 00  // Element ID=48, Length=20, Version=1
00 0F AC 08  // Group Cipher: GCMP (or 04 for CCMP)
01           // Pairwise count: 1
00 0F AC 08  // Pairwise: GCMP (or 04 for CCMP)
01           // AKMP count: 1
00 0F AC 09  // AKMP: SAE
00           // PMKID count: 0
```

**Honest negative:** The exact RSN IE byte layout above is derived from the IEEE 802.11i standard and confirmed by mac80211's parsing in `parse.c`. The UAPI header `include/uapi/linux/ieee80211.h` (which defines `struct ieee80211_rsn_ie`) was NOT in the sparse checkout. The cipher/AKMP OUI values (`WLAN_CIPHER_SUITE_*`, `WLAN_AKM_SUITE_*`) are defined in that header. Verify against raw source before implementation.

#### HT Capabilities IE (`WLAN_EID_HT_CAPABILITY = 45`)

**Source:** `parse.c:495-502` — parsed as `struct ieee80211_ht_cap` (26 bytes). MT7601U supports HT20 only (no 40MHz). This IE is required for any HT-capable association. Structure defined in `include/uapi/linux/ieee80211.h` (not in sparse checkout — honest negative).

#### WMM IE (`WLAN_EID_QOS = 17`, OUI 00:50:F2)

**Source:** `parse.c:463-477` — parsed from vendor-specific IE with OUI 0x00-0x50-0xF2. Two subtypes: WMM Parameter Set (OUI Type=4, subtype=2) and WMM Info (subtype=1). Required for QoS operation.

### 2.4 Frame type/subtype constants

**Source:** Used throughout mac80211 (`mlme.c`, `util.c`, etc.):

| Frame | Type | Subtype | Hex |
|-------|------|---------|-----|
| Association Request | MGMT (0) | 0 | 0x00 |
| Association Response | MGMT (0) | 1 | 0x01 |
| Reassociation Request | MGMT (0) | 2 | 0x02 |
| Reassociation Response | MGMT (0) | 3 | 0x03 |
| Probe Request | MGMT (0) | 4 | 0x04 |
| Probe Response | MGMT (0) | 5 | 0x05 |
| Beacon | MGMT (0) | 8 | 0x08 |
| Authentication | MGMT (0) | 11 | 0x0B |
| Deauthentication | MGMT (0) | 12 | 0x0C |
| Disassociation | MGMT (0) | 13 | 0x0D |
| Action | MGMT (0) | 17 | 0x11 |

---

## 3. WPA2 4-way handshake + WPA3 SAE on-air sequencing

### 3.1 Architecture: mac80211 is the transport, userspace is the brain

**Critical design fact:** mac80211 does NOT implement the 4-way handshake logic. It provides the transport:

1. **RX path:** Encrypted data frames → hardware CCMP decryption (via installed keys) → EAPOL payload extracted → delivered to userspace via `cfg80211_rx_control_port()` (`rx.c:2751`)
2. **TX path:** Userspace sends EAPOL frame → mac80211 TX path → hardware CCMP encryption (via installed keys) → on-air

The control port protocol is `ETH_P_802_1X` (0x888E). Frames destined for the PAE group address (`01:80:C2:00:00:03`) or the STA's own MAC are allowed through (`rx.c:2716-2736`).

### 3.2 WPA2 4-way handshake (M1→M4) on-air sequence

**Prerequisites:**
- STA is authenticated (open auth, transaction 1↔2 complete)
- STA is associated (assoc req/resp complete)
- PMK is derived from PSK + SSID in userspace (W0's crypto)

#### M1: AP → STA (EAPOL-Key, Key Info = Key MIC + Install + Key ACK)
- **Contents:** ANonce (32 bytes), Replay Counter = 1, Key ID points to GTK slot
- **MIC:** Computed over the EAPOL-Key body using PTK-KCK (derived from PMK + ANonce + SNonce + MAC_A + MAC_SP). However, since STA doesn't have PTK yet, MIC verification uses a zeroed key or is skipped.
- **On-air:** Data frame, encrypted with Temporal Key (not yet installed — sent unencrypted or with pairwise null cipher during handshake)
- **STA processing (userspace):** Receives M1 via control port → extracts ANonce → derives PTK = PRF(PMK, "Pairwise key expansion", min(MAC_A, MAC_SP) || max(MAC_A, MAC_SP) || min(ANonce, SNonce) || max(ANonce, SNonce))

#### M2: STA → AP (EAPOL-Key, Key Info = Key MIC + Key ACK)
- **Contents:** SNonce (32 bytes), Replay Counter = 1, MIC computed with PTK-KCK
- **STA processing (userspace):** Has SNonce (pre-generated or generated now) → computes full PTK → computes MIC → sends M2
- **AP processing:** Verifies MIC (confirms STA has correct PTK) → derives its own PTK copy

#### M3: AP → STA (EAPOL-Key, Key Info = Key MIC + Install + Key ACK + Secure)
- **Contents:** GTK (16 or 32 bytes depending on cipher), Key ID, Replay Counter = 2, MIC computed with PTK-KCK
- **Encrypted:** The GTK is encrypted with PTEK (part of PTK) so only the legitimate STA can decrypt it
- **STA processing (userspace):** Verifies MIC → installs GTK via `NL80211_CMD_NEW_KEY` → installs PTK for data encryption via `NL80211_CMD_NEW_KEY`

#### M4: STA → AP (EAPOL-Key, Key Info = Key MIC + Key ACK)
- **Contents:** Replay Counter = 1, MIC only (acknowledgment)
- **Purpose:** Confirms STA received and installed GTK
- **After M4:** Both sides have PTK + GTK installed. Data plane is live.

**Retry semantics:** Each message has a timeout. If M2 is not received within the handshake timeout, AP retransmits M1. mac80211 reports `WLAN_REASON_4WAY_HANDSHAKE_TIMEOUT` (reason code 10) on failure. MIC failures trigger `WLAN_REASON_MIC_FAILURE` (reason code 9) and countermeasures (for TKIP).

**Key installation flow:**
```
wpa_supplicant derives PTK/GTK
  → NL80211_CMD_NEW_KEY (pairwise key, flags |= IEEE80211_KEY_FLAG_PAIRWISE)
    → mac80211 key.c:ieee80211_key_link_add()
      → drv_set_key(local, SET_KEY, sdata, sta, key)
        → mt7601u_set_key() (main.c:292)
          → mt76_mac_wcid_set_key() (mac.c:530) — writes to hardware key memory
  → NL80211_CMD_NEW_KEY (group key, no IEEE80211_KEY_FLAG_PAIRWISE)
    → same path, but goes through group_wcid and shared key setup
```

### 3.3 WPA3 SAE on-air sequence

**Two-phase process:** SAE auth exchange (pre-association) + 4-way handshake (post-association).

#### Phase A: SAE authentication (within IEEE 802.11 Auth frame exchange)

**SAE Commit/Confirm occurs BEFORE association.** This is the key difference from WPA2.

1. **Auth TX (STA→AP, transaction 1):** Auth frame with SAE Commit element
   - Contains: SAE commitment value (hash of DH public value or hash-to-element result)
   - mac80211 sends via `ieee80211_send_auth()` with `auth_data->trans = 1` (`mlme.c:8549`)

2. **Auth RX (AP→STA, transaction 1):** Auth frame with SAE Commit element
   - May include status code for anti-clogging upgrade: `WLAN_STATUS_ANTI_CLOG_REQUIRED` (55), `WLAN_STATUS_SAE_HASH_TO_ELEMENT` (57), or `WLAN_STATUS_SAE_PK` (58) (`mlme.c:5020-5033`)
   - If upgrade requested, mac80211 sets `auth_data->waiting = true` and waits for userspace to continue

3. **Auth TX (STA→AP, transaction 2):** Auth frame with SAE Confirm
   - Empty variable portion — both sides have computed the shared secret (SAE-PMK)

4. **Auth RX (AP→STA, transaction 2):** Auth frame, status = SUCCESS
   - `ieee80211_mark_sta_auth()` called → STA moves to `IEEE80211_STA_AUTH` (`mlme.c:5106-5107`)

5. **Auth RX (AP→STA, transaction 3):** Optional peer confirmation
   - Sets `auth_data->peer_confirmed = true` (`mlme.c:5109-5113`)

**After SAE auth:** The SAE-PMK (derived from the SAE DH exchange) replaces the PSK-derived PMK. The remainder of the connection proceeds identically to WPA2: associate → 4-way handshake using SAE-PMK as the base key.

#### Phase B: 4-way handshake (identical to WPA2, uses SAE-PMK instead of PSK-PMK)

Same M1→M4 sequence. The only difference is the PMK source:
- WPA2: PMK = PRF-256("PSK", min(MAC_A, MAC_SP), SSID, PSK)
- WPA3: PMK = SAE-PMK (from SAE DH exchange in auth phase)

**W0/W0b integration:** W0's EAPOL-MIC and CCMP crypto handles the 4-way handshake cryptography. W0b's SAE implementation handles the auth-phase DH exchange. W2 drives the sequencing: when to send/receive each frame, timeout handling, and key installation via the MT7601U MCU.

---

## 4. Key install via the MT7601U MCU (the driver seam)

### 4.1 WCID (WLAN Client ID) model

**Source:** `mt7601u.h:98-106`

```c
/* WCID allocation:
 *     0: mcast wcid
 *     1: bssid wcid
 *  2...: STAs (pairwise keys)
 * ...125: group wcids
 *    127: reserved
 */
#define N_WCIDS		128
#define GROUP_WCID(idx)	(N_WCIDS - 2 - idx)  // GROUP_WCID(0) = 125, GROUP_WCID(1) = 124, ...
```

**WCID table layout:**
- WCID 0: Multicast/Broadcast (group keys for broadcast traffic)
- WCID 1: BSSID (self — used for RX identification of AP frames)
- WCID 2+: STA entries (each associated STA gets a WCID, used for pairwise key installation)
- WCID 125-126: Group WCIDs (for additional BSS/group key contexts)
- WCID 127: Reserved

**Per-WCID hardware registers:** `regs.h:567-588`
```c
#define MT_WCID_ADDR_BASE	0x1800
#define MT_WCID_ADDR(_n)	(MT_WCID_ADDR_BASE + (_n) * 8)  // 8B per WCID — MAC address

#define MT_WCID_KEY_BASE	0x8000
#define MT_WCID_KEY(_n)		(MT_WCID_KEY_BASE + (_n) * 32)  // 32B per WCID — key material

#define MT_WCID_IV_BASE		0xa000
#define MT_WCID_IV(_n)		(MT_WCID_IV_BASE + (_n) * 8)    // 8B per WCID — IV/PN state

#define MT_WCID_ATTR_BASE	0xa800
#define MT_WCID_ATTR(_n)	(MT_WCID_ATTR_BASE + (_n) * 4)   // 4B per WCID — attributes
```

**WCID attribute register fields:** `regs.h:581-588`
```c
#define MT_WCID_ATTR_PAIRWISE		BIT(0)     // pairwise vs. group key
#define MT_WCID_ATTR_PKEY_MODE		GENMASK(3, 1) // cipher mode (lower bits)
#define MT_WCID_ATTR_BSS_IDX		GENMASK(6, 4) // BSS index
#define MT_WCID_ATTR_RXWI_UDF		GENMASK(9, 7) // RXWI UDF
#define MT_WCID_ATTR_PKEY_MODE_EXT	BIT(10)    // cipher mode (upper bit)
#define MT_WCID_ATTR_BSS_IDX_EXT	BIT(11)    // BSS index extension
```

### 4.2 Key install function

**Source:** `main.c:292-335` (`mt7601u_set_key`) → `mac.c:530-569` (`mt76_mac_wcid_set_key`)

This is the mac80211 `set_key` op. Called by mac80211 when userspace installs a key via `NL80211_CMD_NEW_KEY`.

```c
mt7601u_set_key(hw, cmd, vif, sta, key):
  // Determine WCID: STA key → msta->wcid; Group key → mvif->group_wcid
  wcid = sta ? &msta->wcid : &mvif->group_wcid

  // Cipher support check — fall back to SW for unsupported ciphers
  switch (key->cipher):
    WEP40, WEP104, TKIP, CCMP: supported in HW
    default: return -EOPNOTSUPP (forces SW crypto)

  if cmd == SET_KEY:
    key->hw_key_idx = wcid->idx   // tell mac80211 which HW key slot
    wcid->hw_key_idx = idx        // track which key index is active
  else:  // DELETE_KEY
    if idx == wcid->hw_key_idx:
      wcid->hw_key_idx = -1
    key = NULL  // will clear hardware key

  // Group key: also set shared key registers
  if !sta:
    mt76_mac_wcid_set_key(dev, wcid->idx, key)
    mt76_mac_shared_key_setup(dev, mvif->idx, idx, key)
  else:
    mt76_mac_wcid_set_key(dev, msta->wcid.idx, key)
```

**Hardware key write:** `mac.c:530-569` (`mt76_mac_wcid_set_key`)
```c
mt76_mac_wcid_set_key(dev, wcid_idx, key):
  cipher = mt76_mac_get_key_info(key, key_data)  // extract key bytes + cipher type

  // Write key material to hardware key memory
  mt7601u_wr_copy(dev, MT_WCID_KEY(wcid_idx), key_data, 32)

  // Write IV/PN state
  if key:
    iv_data[3] = key->keyidx << 6
    if cipher >= TKIP:
      iv_data[0] |= 1   // start PN at 1 (spec compliance)
      iv_data[3] |= 0x20
  mt7601u_wr_copy(dev, MT_WCID_IV(wcid_idx), iv_data, 8)

  // Write WCID attributes
  val = rr(MT_WCID_ATTR(wcid_idx))
  val = set cipher mode (PKEY_MODE + PKEY_MODE_EXT)
  val = set PAIRWISE flag if key->flags & IEEE80211_KEY_FLAG_PAIRWISE
  wr(MT_WCID_ATTR(wcid_idx), val)
```

**Cipher type mapping:** `mac.c:512-527`
```c
WEP40  → MT_CIPHER_WEP40
WEP104 → MT_CIPHER_WEP104
TKIP   → MT_CIPHER_TKIP
CCMP   → MT_CIPHER_AES_CCMP
default → MT_CIPHER_NONE
```

### 4.3 Shared key setup (group keys)

**Source:** `mac.c:571-593` (`mt76_mac_shared_key_setup`)

For group keys (GTK), in addition to the WCID key, the shared key registers are programmed:
```c
mt76_mac_shared_key_setup(dev, vif_idx, key_idx, key):
  mt7601u_wr_copy(dev, MT_SKEY(vif_idx, key_idx), key_data, 32)
  val = rr(MT_SKEY_MODE(vif_idx))
  val = set cipher for this key slot
  wr(MT_SKEY_MODE(vif_idx), val)
```

### 4.4 Key install is register writes, NOT MCU commands

**Critical finding:** Key installation uses direct register writes (`mt7601u_wr_copy()` → USB vendor control requests), NOT MCU commands through the in-band MCU interface. This is significantly simpler than expected — no `CMD_FUN_SET_OP` or similar MCU handshake is needed for key installation.

The registers are accessed via USB vendor requests (same mechanism as all other register access in this driver). Each 32-byte key write = multiple 4-byte vendor writes to `MT_WCID_KEY(wcid_idx) + offset`.

### 4.5 W0/W0b → W2 key install seam

W0 computes PTK/GTK/EAPOL-MIC/CCMP keys. W2 receives the derived key material from W0's crypto layer, then:
1. Calls `mt7601u_set_key()` (or the LogansOS equivalent) with the key material
2. The driver writes key bytes to `MT_WCID_KEY(wcid)` register block (32B)
3. Sets cipher mode in `MT_WCID_ATTR(wcid)` register
4. Initializes IV/PN state in `MT_WCID_IV(wcid)` register
5. Hardware firmware handles all subsequent CCMP encryption/decryption for data frames using the installed key

**WCID assignment:** For the STA's own pairwise keys, WCID 2+ is used. The group (GTK) uses `mvif->group_wcid` (WCID 0 for multicast).

---

## 5. Mgmt/data frame TX/RX on the MT7601U + NetDevice glue

### 5.1 TX path: Management frames vs Data frames

**Source:** `tx.c:190-226` (`mt7601u_tx`)

Both management and data frames use the same TX path. The distinction is in how mac80211 calls the driver:

```c
mt7601u_tx(hw, control, skb):
  // Determine WCID for this frame
  if sta (per-STA data):
    wcid = &msta->wcid     // pairwise WCID
  else if vif (broadcast/mgmt):
    wcid = &mvif->group_wcid  // group WCID
  else:
    wcid = dev->mon_wcid   // monitor mode

  // Reserve headroom for TXWI + padding
  mt7601u_skb_rooms(dev, skb)  // needs sizeof(mt76_txwi) + 4B [+ 2B alignment]

  // Push TXWI descriptor before the frame
  txwi = mt7601u_push_txwi(dev, skb, sta, wcid, pkt_len)
    txwi->rate_ctl = rate from info->control.rates[0] or wcid default
    txwi->ack_ctl = ACK request + sequence control flags
    txwi->wcid = wcid->idx  // critical: tells hardware which WCID to use
    txwi->len_ctl = pkt_len + pkt_id encoding

  // DMA wrap
  mt7601u_dma_enqueue_tx(dev, skb, wcid, hw_q)
```

**TX queue selection:** `tx.c:27-37` (`skb2q`)
- Management frames → determined by `skb_get_queue_mapping()` from mac80211
- AC_VO/VI/BE/BK → hardware queues (mirrored order: Q0=highest priority)
- PSD queue for probe/scanning

**TXWI structure:** `mac.h:109-124` (20 bytes)
```c
struct mt76_txwi {
    __le16 flags;       // AMPDU, rate fallback, TX report, etc.
    __le16 rate_ctl;    // MCS index, BW, SGI, STBC, PHY mode
    u8   ack_ctl;       // ACK request, BA window
    u8   wcid;          // WCID index — CRITICAL for key selection
    __le16 len_ctl;     // packet length + pkt_id
    __le32 iv;          // IV for SW encryption (unused with HW offload)
    __le32 eiv;         // extended IV
    u8   aid;           // AID for power save
    u8   txstream;      // TX stream count
    __le16 ctl;         // TX power adjustment, PIFS
};
```

**DMA wrapping:** `dma.h:58-78` (`mt7601u_dma_skb_wrap`)
```
Buffer layout on USB transfer:
| 4B TXINFO | payload (rounded to 4B) | 4B zero pad |
TXINFO: [LEN:16b] [D_PORT:3b] [TYPE:2b] + flags
  D_PORT = WLAN_PORT for data, CPU_TX_PORT for MCU commands
  TYPE = DMA_PACKET for frames, DMA_COMMAND for MCU cmds
```

**Management frame TX specifics:**
- Auth/Assoc frames are sent via the same `mt7601u_tx()` path
- mac80211 sets `IEEE80211_TX_CTL_ASSIGN_SEQ` for sequence number assignment (`tx.c:160-161`)
- Rate control is handled by firmware; driver provides rate hints via TXWI
- No encryption for mgmt frames (they're sent in clear during association)

### 5.2 RX path: Hardware prepends RXWI, driver unwraps

**Source:** `dma.c` (RX processing), `mac.h:26-44` (`struct mt7601u_rxwi`)

```
USB bulk IN on MT_EP_IN_PKT_RX → segment processing:
  Skip: 4B DMA HDR + 4B RX INFO
  Parse: RXWI struct (32 bytes)
    rxinfo: frame type flags (data, null, multicast, CRC error, MIC error, etc.)
    ctl: WCID (bits 0-7), key index (bits 8-9), BSS idx, UDF, MPDU length, TID
  Extract: 802.11 frame payload
  Pass to: ieee80211_rx() — mac80211's RX entry point
```

**RXWI structure:** `mac.h:26-44` (32 bytes)
```c
struct mt7601u_rxwi {
    __le32 rxinfo;     // frame type flags
    __le32 ctl;        // WCID, key idx, BSS idx, MPDU len, TID
    __le16 frag_sn;    // fragment number + sequence
    __le16 rate;       // RX rate (MCS, BW, SGI, STBC, PHY)
    u8   unknown;
    u8   zero[3];
    u8   snr;          // SNR/RSSI estimate
    u8   ant;          // antenna
    u8   gain;         // gain (contains RSSI value in lower 6 bits)
    u8   freq_off;     // frequency offset
    __le32 resv2;
    __le32 expert_ant;
};
```

**RXWI key fields:** `mac.h:72-77`
```c
#define MT_RXWI_CTL_WCID		GENMASK(7, 0)   // which WCID matched
#define MT_RXWI_CTL_KEY_IDX		GENMASK(9, 8)   // which key was used for decryption
#define MT_RXWI_CTL_BSS_IDX		GENMASK(12, 10) // BSS index
#define MT_RXWI_CTL_MPDU_LEN		GENMASK(27, 16) // MPDU length
#define MT_RXWI_CTL_TID			GENMASK(31, 28) // TID (traffic identifier)
```

**Hardware decryption:** When a data frame arrives with CCMP encryption:
1. Hardware matches destination MAC → WCID via `MT_WCID_ADDR` table
2. Uses key from `MT_WCID_KEY(wcid)` for CCMP decryption
3. Sets `MT_RXINFO_DECRYPT` flag in RXWI if decryption succeeded
4. Sets `MT_RXINFO_MICERR` flag if MIC verification failed
5. Strips CCMP header/trailer, passes plaintext to host

### 5.3 NetDevice glue (how the WiFi interface presents to the stack)

**Source:** `main.c:408-432` (`mt7601u_ops`) — mac80211 ops table

The MT7601U registers as a mac80211 driver via `ieee80211_alloc_hw()`. The mac80211 subsystem then:
1. Creates a `struct ieee80211_local` (hardware abstraction)
2. On `add_interface` op, creates virtual interfaces (STA, monitor, etc.)
3. Each virtual interface has an associated `net_device` that the Linux network stack sees

**NetDevice creation flow:**
```
mt7601u_probe() → ieee80211_alloc_hw() → ieee80211_register_hw()
  → cfg80211 creates wiphy
  → userspace creates interface via NL80211_CMD_NEW_INTERFACE (type=STA)
    → mt7601u_add_interface() called
      → allocates mt76_vif + mt76_wcid
      → sets up group_wcid (WCID for broadcast/multicast)
    → mac80211 creates net_device (wlan0-style interface)
    → network stack sees it as a regular network interface
```

**CDC-ECM precedent:** ADR-047 established the pattern for presenting USB devices as NetDevices. The WiFi driver follows the same pattern: the mac80211 subsystem abstracts the wireless-specific details, and the resulting net_device is indistinguishable from an Ethernet interface to the upper stack (DHCP, DNS, TCP all work identically).

**Key mac80211 ops for W2:**
| Op | Function | Purpose |
|----|----------|---------|
| `tx` | `mt7601u_tx` | Send frames (mgmt + data) |
| `set_key` | `mt7601u_set_key` | Install PTK/GTK in hardware |
| `sta_add` | `mt7601u_sta_add` | Add STA entry (WCID allocation) |
| `sta_remove` | `mt7601u_sta_remove` | Remove STA entry |
| `sta_notify` | `mt7601u_sta_notify` | State change notifications |
| `start` | `mt7601u_start` | Interface up |
| `stop` | `mt7601u_stop` | Interface down |
| `sw_scan_start` | `mt7601u_sw_scan` | Start scan |
| `sw_scan_complete` | `mt7601u_sw_scan_complete` | Scan done |
| `configure_filter` | `mt76_configure_filter` | RX filter setup |
| `bss_info_changed` | `mt7601u_bss_info_changed` | BSS parameter updates |

### 5.4 Minimal RUN state requirements

For W2 to reach a functional data plane:
1. **Scan:** Already working from W1 (`sw_scan_start`/`sw_scan_complete`)
2. **Auth:** Open auth frame TX/RX via `mt7601u_tx` + mac80211 RX path
3. **Assoc:** AssocReq/Resp via same TX/RX path
4. **Key install:** `mt7601u_set_key` → hardware WCID key registers
5. **Data:** Encrypted data frames via TXWI/RXWI path, CCMP offloaded to firmware
6. **NetDevice:** mac80211 creates the net_device; upper stack (DHCP, etc.) works automatically

**What W2 does NOT need to implement:**
- Rate control algorithm (firmware handles this)
- Power save (can be deferred)
- AMPDU aggregation tuning (firmware default is sufficient for initial bringup)
- Roaming, AP mode, mesh, P2P

---

## 6. Recommended W2 sub-slice

Based on the raw-source mining, the recommended W2 build order:

### Slice A: Open auth + association (no encryption)
**Goal:** Associate with an open AP, verify management frame TX/RX works.
- Implement: Auth frame TX (`mt7601u_tx` with mgmt frame), Auth frame RX (existing RX path), AssocReq/Resp
- Verify: Association succeeds, AID received, STA state machine reaches ASSOC
- Risk: Low — reuses W1's TX/RX infrastructure

### Slice B: WPA2-PSK 4-way handshake + key install
**Goal:** Full WPA2 connection with CCMP encryption.
- Implement: Key install (`mt7601u_set_key` → WCID key registers), EAPOL frame passthrough to/from W0's crypto
- Wire up: W0's PTK/GTK derivation → driver `set_key` op → hardware CCMP offload
- Verify: Encrypted data frames pass through, MIC verification succeeds in hardware

### Slice C: NetDevice + upper stack
**Goal:** DHCP over WiFi, ping external host.
- Implement: Ensure mac80211 net_device is properly configured for STA mode
- Verify: `ip addr show` shows wlan0, DHCP obtains IP, ping works

### Slice D: WPA3-SAE (deferred)
**Goal:** WPA3 connection with SAE auth exchange.
- Implement: SAE Commit/Confirm within auth frame exchange, wire W0b's SAE crypto
- Complexity: Higher — multi-frame auth exchange with anti-clogging state machine

**Recommendation:** Build A → B → C first (WPA2-only). Add D (WPA3) as a follow-up. This mirrors the Linux driver's own development history and minimizes initial risk.

---

## 7. Honest negatives

1. **UAPI ieee80211.h not in sparse checkout:** `include/uapi/linux/ieee80211.h` was NOT included in the sparse checkout paths. This header defines `struct ieee80211_hdr`, `struct ieee80211_mgmt`, `WLAN_CIPHER_SUITE_*`, `WLAN_AKM_SUITE_*`, and other critical constants. The frame layouts in §2 are derived from mac80211 usage patterns and the IEEE 802.11 standard, but should be verified against the actual UAPI header before implementation. **Action:** Add `include/uapi/linux/ieee80211.h` to sparse checkout for W2 build.

2. **SAE element byte layout:** The SAE commit/confirm element format within auth frames is defined in IEEE 802.11ax (802.11-2019) and implemented in wpa_supplicant (`src/common/sae.h`). wpa_supplicant source was unavailable (clone failures across multiple mirrors). The SAE flow is described from mac80211's handling in `mlme.c`, but the exact byte layout of SAE elements within auth frames should be verified against wpa_supplicant or the 802.11ax spec.

3. **EAPOL-Key frame internals:** The 4-way handshake EAPOL-Key frame format (Key Info field, Key RSC, Key ID, Nonce fields, MIC field) is defined in IEEE 802.1X-2010 and 802.11i. mac80211 passes these frames to userspace — it doesn't parse them internally. The exact byte layout should be verified against wpa_supplicant's `src/common/wpa.h` or the 802.1X standard.

4. **MT_SKEY register offset:** The shared key register base (`MT_SKEY`) and mode register (`MT_SKEY_MODE`) are used in `mac.c:584-590` but their definitions are not visible in `regs.h` within the sparse checkout range. They're likely defined elsewhere in the driver or inherited from a shared header. Verify before implementation.

5. **Firmware CCMP offload confirmation:** The trap-mine (§3.6 of previous findings) noted firmware handles data-plane CCMP. This is confirmed by: (a) `mt7601u_set_key` writes keys to hardware registers, (b) RXWI has `MT_RXINFO_DECRYPT` and `MT_RXINFO_MICERR` flags indicating hardware decryption/MIC verification, (c) TXWI has IV/EIV fields that would be unused with HW encryption. However, the exact firmware command to enable CCMP offload per-WCID is not explicitly visible — it appears to be implicit from the WCID_ATTR configuration.

---

## 8. Source provenance

| Source | Obtained | Commit/Version |
|--------|----------|----------------|
| Linux kernel mac80211 | Sparse clone (`net/mac80211`, `include/net`) | Current main (post-v6.15) |
| Linux kernel mt7601u driver | Sparse clone (`drivers/net/wireless/mediatek/mt7601u`) | Commit `2b07ea76f` (24 files) |
| Linux kernel UAPI headers | Partial sparse clone (`include/uapi/linux`) — ieee80211.h missing | Current main |

All register values, struct layouts, and sequences above are from direct Read of the cloned source files. No WebFetch was used for any byte/register data. Frame layouts in §2 that reference the IEEE 802.11 standard are flagged as needing raw-source verification against `include/uapi/linux/ieee80211.h` before implementation.
