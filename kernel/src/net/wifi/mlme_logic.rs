// SPDX-License-Identifier: MPL-2.0
//! ISC-1: 802.11 management-frame build/parse + the minimal STA MLME state
//! machine — the protocol layer of W2's in-kernel supplicant.
//!
//! mac80211 delegates the 4-way handshake to a userspace supplicant; NeoPlutarch
//! has none, so W2 *is* an in-kernel minimal supplicant. This module is its
//! frame + state layer: it builds the management frames that drive W1's MT7601U
//! TX (ISC-1 wiring) and parses the responses, then hands off to W0/W0b crypto
//! at the 4-way handshake (ISC-2) and CCMP (ISC-3).
//!
//! Pure `no_std` logic, byte-faithful to IEEE 802.11. Every constant and struct
//! offset below was verified against the in-kernel `include/linux/ieee80211.h`
//! (v6.6) and `include/uapi/linux/nl80211.h` — NOT the W2 FINDINGS, which had
//! four wrong values (AUTH_OPEN, AUTH_SAE, EXT_SUPP_RATES, and the RSN PSK AKM
//! type) that the raw source corrected before any code was written. Host-oracle'd
//! in `tools/ai-ops-tests` against hand-laid frame fixtures (I-12) — the builders
//! here are never the source of the oracle's bytes.

#![allow(dead_code)] // the protocol surface is assembled across ISC-1..4

// ── Frame Control (IEEE 802.11 §9.2.4.1; ieee80211.h:44-102) ────────────────
// `frame_control` is __le16. The STYPE_* values already fold in the FTYPE bits,
// so a management Authentication frame's FC == FTYPE_MGMT | STYPE_AUTH == 0x00B0.
pub const FCTL_FTYPE: u16 = 0x000c;
pub const FCTL_STYPE: u16 = 0x00f0;
pub const FCTL_TODS: u16 = 0x0100;
pub const FCTL_FROMDS: u16 = 0x0200;
pub const FCTL_PROTECTED: u16 = 0x4000;

pub const FTYPE_MGMT: u16 = 0x0000;
pub const FTYPE_DATA: u16 = 0x0008;

pub const STYPE_ASSOC_REQ: u16 = 0x0000;
pub const STYPE_ASSOC_RESP: u16 = 0x0010;
pub const STYPE_PROBE_REQ: u16 = 0x0040;
pub const STYPE_PROBE_RESP: u16 = 0x0050;
pub const STYPE_BEACON: u16 = 0x0080;
pub const STYPE_DISASSOC: u16 = 0x00a0;
pub const STYPE_AUTH: u16 = 0x00b0;
pub const STYPE_DEAUTH: u16 = 0x00c0;

// ── Authentication algorithm numbers (ieee80211.h:3142-3145) ────────────────
// IEEE 802.11 §9.4.1.1. (FINDINGS said OPEN=1/SAE=4 — both wrong.)
pub const AUTH_OPEN: u16 = 0;
pub const AUTH_SHARED_KEY: u16 = 1;
pub const AUTH_SAE: u16 = 3;

// ── Status / reason codes (ieee80211.h:3230, 3320s) ─────────────────────────
pub const STATUS_SUCCESS: u16 = 0;
pub const STATUS_UNSPECIFIED_FAILURE: u16 = 1;
pub const REASON_DEAUTH_LEAVING: u16 = 3;
pub const REASON_4WAY_HANDSHAKE_TIMEOUT: u16 = 15;

// ── Element IDs (ieee80211.h:3359-3394) ─────────────────────────────────────
pub const EID_SSID: u8 = 0;
pub const EID_SUPP_RATES: u8 = 1;
pub const EID_DS_PARAMS: u8 = 3;
pub const EID_HT_CAPABILITY: u8 = 45;
pub const EID_RSN: u8 = 48;
pub const EID_EXT_SUPP_RATES: u8 = 50;

// ── Capability Information (ieee80211.h:3153-3167) ──────────────────────────
pub const CAP_ESS: u16 = 1 << 0;
pub const CAP_PRIVACY: u16 = 1 << 4;
pub const CAP_SHORT_PREAMBLE: u16 = 1 << 5;

// ── RSN suite selectors (nl80211.h:4071-4092; SUITE(oui,id)=(oui<<8)|id, the
// 4 octets are the OUI in MAC order followed by the type, i.e. be32). ─────────
/// The IEEE OUI for registered cipher/AKM suites: 00:0F:AC.
pub const RSN_OUI: [u8; 3] = [0x00, 0x0f, 0xac];
pub const CIPHER_TYPE_CCMP: u8 = 4;
pub const CIPHER_TYPE_TKIP: u8 = 2;
pub const AKM_TYPE_8021X: u8 = 1;
pub const AKM_TYPE_PSK: u8 = 2; // FINDINGS said 1 — wrong; PSK is AKM type 2.
pub const AKM_TYPE_SAE: u8 = 8;

// ── RSN Capabilities (IEEE 802.11-2020 §9.4.2.24.4). SAE-only mandates BOTH —
// an SAE AP rejects an assoc whose RSNE lacks them (trap-mine T-059). ──────────
/// Management Frame Protection Required (RSN cap bit 6).
pub const RSN_CAP_MFPR: u16 = 1 << 6;
/// Management Frame Protection Capable (RSN cap bit 7).
pub const RSN_CAP_MFPC: u16 = 1 << 7;

// ── Layout (struct ieee80211_mgmt; ieee80211.h:1242) ────────────────────────
pub const ETH_ALEN: usize = 6;
pub type MacAddr = [u8; ETH_ALEN];
/// FC(2)+duration(2)+da(6)+sa(6)+bssid(6)+seq_ctrl(2).
pub const MGMT_HDR_LEN: usize = 24;
/// auth body: auth_alg(2)+auth_transaction(2)+status_code(2).
pub const AUTH_FIXED_LEN: usize = 6;
/// assoc_req body: capab_info(2)+listen_interval(2).
pub const ASSOC_REQ_FIXED_LEN: usize = 4;
/// assoc_resp body: capab_info(2)+status_code(2)+aid(2).
pub const ASSOC_RESP_FIXED_LEN: usize = 6;
/// Non-S1G AID mask (ieee80211.h `IEEE80211_MAX_AID`).
pub const MAX_AID: u16 = 0x07ff;

#[inline]
fn put_u16_le(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

#[inline]
fn get_u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

/// Write the 24-byte management MAC header. For a STA→AP frame: `da`=AP (Addr1),
/// `sa`=STA (Addr2), `bssid`=AP (Addr3). `duration` and `seq_ctrl` are left 0 —
/// the MT7601U TXWI path fills the sequence number (FINDINGS §5.1) and the
/// firmware sets duration. Returns the bytes written (`MGMT_HDR_LEN`).
pub fn build_mgmt_header(
    buf: &mut [u8],
    frame_control: u16,
    da: &MacAddr,
    sa: &MacAddr,
    bssid: &MacAddr,
) -> Option<usize> {
    if buf.len() < MGMT_HDR_LEN {
        return None;
    }
    put_u16_le(buf, 0, frame_control);
    put_u16_le(buf, 2, 0); // duration — firmware fills
    buf[4..10].copy_from_slice(da);
    buf[10..16].copy_from_slice(sa);
    buf[16..22].copy_from_slice(bssid);
    put_u16_le(buf, 22, 0); // seq_ctrl — TXWI assigns
    Some(MGMT_HDR_LEN)
}

/// Build an Open-System Authentication request (transaction 1), STA→AP.
/// Body: auth_alg=OPEN(0), auth_transaction=1, status_code=0.
pub fn build_open_auth(buf: &mut [u8], sta: &MacAddr, ap: &MacAddr) -> Option<usize> {
    let fc = FTYPE_MGMT | STYPE_AUTH;
    let mut n = build_mgmt_header(buf, fc, ap, sta, ap)?;
    if buf.len() < n + AUTH_FIXED_LEN {
        return None;
    }
    put_u16_le(buf, n, AUTH_OPEN);
    n += 2;
    put_u16_le(buf, n, 1); // transaction 1
    n += 2;
    put_u16_le(buf, n, STATUS_SUCCESS); // 0 in a request
    n += 2;
    Some(n)
}

/// Build an SAE Authentication frame, STA→AP (CB-5a ISC-1). Body: auth_alg=SAE(3),
/// auth_transaction (1=Commit, 2=Confirm), status_code, then the SAE body from
/// `sae_sm::SaeSession::commit_body`/`confirm_body`. The Commit status is
/// `SAE_HASH_TO_ELEMENT` (126, H2E) and the Confirm status is 0 — the STA/AP must
/// agree on the method or the frame is silently discarded (trap-mine T-052).
pub fn build_sae_auth(
    buf: &mut [u8],
    sta: &MacAddr,
    ap: &MacAddr,
    transaction: u16,
    status: u16,
    body: &[u8],
) -> Option<usize> {
    let fc = FTYPE_MGMT | STYPE_AUTH;
    let mut n = build_mgmt_header(buf, fc, ap, sta, ap)?;
    if buf.len() < n + AUTH_FIXED_LEN + body.len() {
        return None;
    }
    put_u16_le(buf, n, AUTH_SAE);
    n += 2;
    put_u16_le(buf, n, transaction);
    n += 2;
    put_u16_le(buf, n, status);
    n += 2;
    buf[n..n + body.len()].copy_from_slice(body);
    Some(n + body.len())
}

/// The SAE body of a received Authentication frame — the bytes after the fixed
/// `alg ‖ transaction ‖ status`. `None` if the frame is too short or not a
/// management Authentication frame. Pair with `parse_auth` for the fixed fields.
pub fn sae_auth_body(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < MGMT_HDR_LEN + AUTH_FIXED_LEN {
        return None;
    }
    let fc = get_u16_le(frame, 0);
    if (fc & (FCTL_FTYPE | FCTL_STYPE)) != (FTYPE_MGMT | STYPE_AUTH) {
        return None;
    }
    Some(&frame[MGMT_HDR_LEN + AUTH_FIXED_LEN..])
}

/// Append one Information Element `[id][len][data...]` at `at`. Returns the new
/// end offset.
pub fn put_ie(buf: &mut [u8], at: usize, eid: u8, data: &[u8]) -> Option<usize> {
    if data.len() > 255 {
        return None;
    }
    let end = at + 2 + data.len();
    if buf.len() < end {
        return None;
    }
    buf[at] = eid;
    buf[at + 1] = data.len() as u8;
    buf[at + 2..end].copy_from_slice(data);
    Some(end)
}

/// Append one RSN suite selector (`OUI[3] || type`) at `at`.
fn put_suite(buf: &mut [u8], at: usize, ty: u8) -> Option<usize> {
    let end = at + 4;
    if buf.len() < end {
        return None;
    }
    buf[at..at + 3].copy_from_slice(&RSN_OUI);
    buf[at + 3] = ty;
    Some(end)
}

/// Build the WPA2-PSK RSN Information Element (CCMP group + pairwise, PSK AKM,
/// RSN capabilities 0). The exact 22-byte IE (EID 48, len 20):
/// `30 14 | 01 00 | 00 0F AC 04 | 01 00 | 00 0F AC 04 | 01 00 | 00 0F AC 02 | 00 00`.
pub fn build_rsn_ie_wpa2_psk(buf: &mut [u8], at: usize) -> Option<usize> {
    // Body length: version(2)+group(4)+pwcount(2)+pw(4)+akmcount(2)+akm(4)+caps(2)=20.
    let body_len = 20usize;
    let end = at + 2 + body_len;
    if buf.len() < end {
        return None;
    }
    buf[at] = EID_RSN;
    buf[at + 1] = body_len as u8;
    let mut p = at + 2;
    put_u16_le(buf, p, 1); // RSN version 1
    p += 2;
    p = put_suite(buf, p, CIPHER_TYPE_CCMP)?; // group cipher
    put_u16_le(buf, p, 1); // pairwise count
    p += 2;
    p = put_suite(buf, p, CIPHER_TYPE_CCMP)?; // pairwise[0]
    put_u16_le(buf, p, 1); // AKM count
    p += 2;
    p = put_suite(buf, p, AKM_TYPE_PSK)?; // AKM[0] = PSK
    put_u16_le(buf, p, 0); // RSN capabilities
    p += 2;
    debug_assert_eq!(p, end);
    Some(end)
}

/// Build the WPA3-SAE RSN Information Element (CB-5a ISC-2): CCMP group +
/// pairwise, **AKM = SAE (8)**, RSN capabilities **MFPC + MFPR** (SAE-only
/// mandates MFPR — trap-mine T-059). The exact 22-byte IE (EID 48, len 20):
/// `30 14 | 01 00 | 00 0F AC 04 | 01 00 | 00 0F AC 04 | 01 00 | 00 0F AC 08 | C0 00`.
pub fn build_rsn_ie_wpa3_sae(buf: &mut [u8], at: usize) -> Option<usize> {
    let body_len = 20usize;
    let end = at + 2 + body_len;
    if buf.len() < end {
        return None;
    }
    buf[at] = EID_RSN;
    buf[at + 1] = body_len as u8;
    let mut p = at + 2;
    put_u16_le(buf, p, 1); // RSN version 1
    p += 2;
    p = put_suite(buf, p, CIPHER_TYPE_CCMP)?; // group cipher
    put_u16_le(buf, p, 1); // pairwise count
    p += 2;
    p = put_suite(buf, p, CIPHER_TYPE_CCMP)?; // pairwise[0]
    put_u16_le(buf, p, 1); // AKM count
    p += 2;
    p = put_suite(buf, p, AKM_TYPE_SAE)?; // AKM[0] = SAE
    put_u16_le(buf, p, RSN_CAP_MFPC | RSN_CAP_MFPR); // MFP capable + required
    p += 2;
    debug_assert_eq!(p, end);
    Some(end)
}

/// The variable content of an Association Request (everything past the fixed
/// capability + listen-interval fields). `rates` are the raw Supported-Rates IE
/// payload octets (500 kbps units; bit 7 = basic rate).
pub struct AssocReqParams<'a> {
    pub ssid: &'a [u8],
    pub rates: &'a [u8],
    pub capab: u16,
    pub listen_interval: u16,
    /// Append the WPA2-PSK RSN IE (an RSN/encrypted network).
    pub rsn: bool,
}

/// Build an Association Request, STA→AP. Body: capability + listen-interval,
/// then the SSID, Supported-Rates, and (for WPA2) RSN IEs from `p`.
pub fn build_assoc_req(
    buf: &mut [u8],
    sta: &MacAddr,
    ap: &MacAddr,
    p: &AssocReqParams,
) -> Option<usize> {
    let fc = FTYPE_MGMT | STYPE_ASSOC_REQ;
    let mut n = build_mgmt_header(buf, fc, ap, sta, ap)?;
    if buf.len() < n + ASSOC_REQ_FIXED_LEN {
        return None;
    }
    put_u16_le(buf, n, p.capab);
    n += 2;
    put_u16_le(buf, n, p.listen_interval);
    n += 2;
    n = put_ie(buf, n, EID_SSID, p.ssid)?;
    n = put_ie(buf, n, EID_SUPP_RATES, p.rates)?;
    if p.rsn {
        n = build_rsn_ie_wpa2_psk(buf, n)?;
    }
    Some(n)
}

// ── Parsing ─────────────────────────────────────────────────────────────────

/// Parsed Authentication frame (the three fixed fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthFrame {
    pub alg: u16,
    pub transaction: u16,
    pub status: u16,
}

/// Parse an Authentication frame body (validates length + frame type/subtype).
pub fn parse_auth(frame: &[u8]) -> Option<AuthFrame> {
    if frame.len() < MGMT_HDR_LEN + AUTH_FIXED_LEN {
        return None;
    }
    let fc = get_u16_le(frame, 0);
    if (fc & (FCTL_FTYPE | FCTL_STYPE)) != (FTYPE_MGMT | STYPE_AUTH) {
        return None;
    }
    Some(AuthFrame {
        alg: get_u16_le(frame, MGMT_HDR_LEN),
        transaction: get_u16_le(frame, MGMT_HDR_LEN + 2),
        status: get_u16_le(frame, MGMT_HDR_LEN + 4),
    })
}

/// Parsed Association Response (the three fixed fields; AID masked to non-S1G).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssocResp {
    pub capab: u16,
    pub status: u16,
    pub aid: u16,
}

/// Parse an Association/Reassociation Response body (validates length + type).
pub fn parse_assoc_resp(frame: &[u8]) -> Option<AssocResp> {
    if frame.len() < MGMT_HDR_LEN + ASSOC_RESP_FIXED_LEN {
        return None;
    }
    let fc = get_u16_le(frame, 0);
    let stype = fc & FCTL_STYPE;
    if (fc & FCTL_FTYPE) != FTYPE_MGMT
        || (stype != STYPE_ASSOC_RESP && stype != (STYPE_ASSOC_RESP | 0x0020))
    {
        // 0x0030 == reassoc_resp; assoc_resp is 0x0010. Accept both.
        return None;
    }
    Some(AssocResp {
        capab: get_u16_le(frame, MGMT_HDR_LEN),
        status: get_u16_le(frame, MGMT_HDR_LEN + 2),
        aid: get_u16_le(frame, MGMT_HDR_LEN + 4) & MAX_AID,
    })
}

/// Find the first Information Element with `eid` in a frame body (the bytes
/// AFTER the fixed portion). Returns its data slice (without id/len).
pub fn find_ie(body: &[u8], eid: u8) -> Option<&[u8]> {
    let mut i = 0;
    while i + 2 <= body.len() {
        let id = body[i];
        let len = body[i + 1] as usize;
        let data_start = i + 2;
        let data_end = data_start + len;
        if data_end > body.len() {
            return None; // truncated IE
        }
        if id == eid {
            return Some(&body[data_start..data_end]);
        }
        i = data_end;
    }
    None
}

// ── EAPOL over 802.11 data (ISC-2: the 4-way handshake transport) ───────────
// The 4-way EAPOL-Key frames ride 802.11 DATA frames (not mgmt), pre-key, with
// an RFC 1042 LLC/SNAP header carrying the EAPOL EtherType. The EAPOL-Key
// payload itself (parse/MIC/PTK) is W0's `net::wpa` — W2 only does the on-wire
// data-frame encapsulation here.

pub const STYPE_DATA: u16 = 0x0000;
/// QoS-data subtype bit — a QoS data frame has a 2-byte QoS-control after the
/// 24-byte header (ieee80211.h `IEEE80211_STYPE_QOS_DATA = 0x0080`).
pub const STYPE_QOS_DATA: u16 = 0x0080;
/// EAPOL / IEEE 802.1X EtherType (ETH_P_PAE), big-endian on the wire.
pub const ETHERTYPE_EAPOL: u16 = 0x888e;
/// RFC 1042 LLC/SNAP header for an EAPOL frame: `AA AA 03 | 00 00 00 | 88 8E`.
pub const LLC_SNAP_EAPOL: [u8; 8] = [0xaa, 0xaa, 0x03, 0x00, 0x00, 0x00, 0x88, 0x8e];

/// Build an 802.11 DATA frame carrying an EAPOL payload, STA→AP (To-DS): the
/// 24-byte data header (addr1=BSSID, addr2=STA, addr3=AP) + the LLC/SNAP + the
/// EAPOL bytes. Non-QoS (the supplicant's control-port frames).
pub fn build_eapol_data(
    buf: &mut [u8],
    sta: &MacAddr,
    ap: &MacAddr,
    eapol: &[u8],
) -> Option<usize> {
    let fc = FTYPE_DATA | STYPE_DATA | FCTL_TODS; // 0x0108
    let mut n = build_mgmt_header(buf, fc, ap, sta, ap)?;
    let end = n + LLC_SNAP_EAPOL.len() + eapol.len();
    if buf.len() < end {
        return None;
    }
    buf[n..n + LLC_SNAP_EAPOL.len()].copy_from_slice(&LLC_SNAP_EAPOL);
    n += LLC_SNAP_EAPOL.len();
    buf[n..n + eapol.len()].copy_from_slice(eapol);
    Some(end)
}

/// Extract the EAPOL payload from a received 802.11 DATA frame (From-DS, AP→STA):
/// skip the data header (26 bytes if QoS, else 24) + the LLC/SNAP, and confirm
/// the SNAP EtherType is EAPOL. `None` if not an EAPOL data frame.
pub fn parse_eapol_data(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < MGMT_HDR_LEN {
        return None;
    }
    let fc = get_u16_le(frame, 0);
    if (fc & FCTL_FTYPE) != FTYPE_DATA {
        return None;
    }
    // QoS data frames carry a 2-byte QoS-control field after the base header.
    let hdr = if (fc & STYPE_QOS_DATA) == STYPE_QOS_DATA {
        MGMT_HDR_LEN + 2
    } else {
        MGMT_HDR_LEN
    };
    let snap_end = hdr + LLC_SNAP_EAPOL.len();
    if frame.len() < snap_end || frame[hdr..snap_end] != LLC_SNAP_EAPOL {
        return None;
    }
    Some(&frame[snap_end..])
}

// ── 802.3 ↔ 802.11 data conversion (ISC-4: the NetDevice seam rides this) ───
// The NetDevice presents an ethernet link; the driver converts each ethernet
// frame to/from an 802.11 DATA frame (RFC 1042 LLC/SNAP carries the EtherType).
/// RFC 1042 LLC/SNAP prefix (before the 2-byte EtherType): `AA AA 03 00 00 00`.
pub const LLC_SNAP_PREFIX: [u8; 6] = [0xaa, 0xaa, 0x03, 0x00, 0x00, 0x00];

/// Build an 802.11 DATA frame (To-DS) from an outbound ethernet payload: data
/// hdr (addr1=BSSID, addr2=STA, addr3=DA) + LLC/SNAP + EtherType + payload.
/// The HW encrypts (CCMP) once the pairwise key is installed. `da` is the
/// ethernet destination (addr3); `ethertype` is big-endian on the wire.
pub fn build_data_frame(
    buf: &mut [u8],
    sta: &MacAddr,
    bssid: &MacAddr,
    da: &MacAddr,
    ethertype: u16,
    payload: &[u8],
) -> Option<usize> {
    let fc = FTYPE_DATA | STYPE_DATA | FCTL_TODS;
    let mut n = build_mgmt_header(buf, fc, bssid, sta, da)?; // addr1/2/3
    let end = n + LLC_SNAP_PREFIX.len() + 2 + payload.len();
    if buf.len() < end {
        return None;
    }
    buf[n..n + LLC_SNAP_PREFIX.len()].copy_from_slice(&LLC_SNAP_PREFIX);
    n += LLC_SNAP_PREFIX.len();
    buf[n..n + 2].copy_from_slice(&ethertype.to_be_bytes());
    n += 2;
    buf[n..n + payload.len()].copy_from_slice(payload);
    Some(end)
}

/// Parse a received 802.11 DATA frame (From-DS) → (EtherType, payload), skipping
/// the data header (26B if QoS, else 24B) + the LLC/SNAP. `None` if it is not a
/// SNAP-encapsulated data frame.
pub fn parse_data_frame(frame: &[u8]) -> Option<(u16, &[u8])> {
    if frame.len() < MGMT_HDR_LEN {
        return None;
    }
    let fc = get_u16_le(frame, 0);
    if (fc & FCTL_FTYPE) != FTYPE_DATA {
        return None;
    }
    let hdr = if (fc & STYPE_QOS_DATA) == STYPE_QOS_DATA {
        MGMT_HDR_LEN + 2
    } else {
        MGMT_HDR_LEN
    };
    let et_off = hdr + LLC_SNAP_PREFIX.len();
    if frame.len() < et_off + 2 || frame[hdr..et_off] != LLC_SNAP_PREFIX {
        return None;
    }
    let ethertype = u16::from_be_bytes([frame[et_off], frame[et_off + 1]]);
    Some((ethertype, &frame[et_off + 2..]))
}

// ── The minimal STA MLME state machine (FINDINGS §1.1-1.3) ──────────────────

/// STA association state. Forward-only; regression requires a full teardown
/// (mac80211 `ieee80211_sta_state`, mlme.c). `Connected` == 802.1X-authorized:
/// for WPA2-PSK it follows the 4-way handshake (ISC-2), for open networks it
/// follows association directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MlmeState {
    /// Scan complete; BSS chosen; nothing sent yet.
    Idle,
    /// Auth request sent; awaiting the auth response.
    Authenticating,
    /// Authenticated; assoc request sent; awaiting the assoc response.
    Associating,
    /// Associated; running the 4-way handshake (WPA2/3).
    Handshaking,
    /// Authorized — the data plane is live (CCMP up; DHCP may proceed).
    Connected,
    /// Terminal failure (timeout/retries exhausted or a deauth/disassoc).
    Failed,
}

/// MLME timing/retry budget (FINDINGS §1.3; mac80211 mlme.c:36-46). Times in ms.
pub const AUTH_TIMEOUT_MS: u32 = 200;
pub const ASSOC_TIMEOUT_MS: u32 = 200;
pub const AUTH_MAX_TRIES: u8 = 3;
pub const ASSOC_MAX_TRIES: u8 = 3;
pub const HANDSHAKE_TIMEOUT_MS: u32 = 1000;

/// The pure state-transition decision: given the current state and an event,
/// return the next state. The driver layer (ISC-1 wiring) owns the actual
/// TX/RX, timers, and retry counters; this keeps the *decision* logic pure and
/// host-testable. `open` selects the open-network short-circuit (assoc →
/// Connected) vs the WPA2/3 path (assoc → Handshaking).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MlmeEvent {
    /// Auth response received with this status code.
    AuthResp(u16),
    /// Assoc response received with this status code.
    AssocResp(u16),
    /// The 4-way handshake completed (keys installed).
    HandshakeDone,
    /// A deauth/disassoc arrived, or a stage's retries were exhausted.
    Reset,
}

/// Advance the MLME state for `event`. Pure; no I/O. Unknown/invalid
/// transitions stay put (the driver layer treats a stuck state as a timeout).
pub fn mlme_next(state: MlmeState, event: MlmeEvent, open: bool) -> MlmeState {
    use MlmeEvent::*;
    use MlmeState::*;
    match (state, event) {
        (_, Reset) => Failed,
        (Authenticating, AuthResp(STATUS_SUCCESS)) => Associating,
        (Authenticating, AuthResp(_)) => Failed,
        (Associating, AssocResp(STATUS_SUCCESS)) => {
            if open {
                Connected
            } else {
                Handshaking
            }
        }
        (Associating, AssocResp(_)) => Failed,
        (Handshaking, HandshakeDone) => Connected,
        (s, _) => s,
    }
}
