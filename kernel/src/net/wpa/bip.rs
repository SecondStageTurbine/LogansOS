// SPDX-License-Identifier: MPL-2.0
//! BIP-CMAC-128 — 802.11w broadcast/multicast **management-frame protection**,
//! done entirely in **software** (Phase 7.5 CB-5a ISC-3). The mt7601u hardware
//! key engine has no BIP/IGTK slot — its `set_key` returns `-EOPNOTSUPP` for
//! `AES_CMAC` (trap-mine T-061) — so the IGTK is a host-side software key and the
//! MMIE compute/verify + IPN replay run on the CPU here, not in a WCID.
//!
//! Byte-oracle facts (raw source, `RESEARCH-2026-06-29-cb-5-sae-pmf-trap-mine`):
//!  - **MMIE** (`struct ieee80211_mmie`, T-054): `eid=76 ‖ len=16 ‖ key_id(2 LE) ‖
//!    IPN(6) ‖ MIC(8)` = 18 bytes appended to the frame body.
//!  - **AAD** (`bip_aad`, T-055): `masked-FC(2) ‖ A1 ‖ A2 ‖ A3` = 20 bytes, with
//!    the FC Retry/PwrMgt/MoreData bits zeroed; the CMAC runs over
//!    `AAD ‖ body+MMIE-with-MIC-zeroed`.
//!  - **IPN replay** (T-056): the wire IPN is least-significant-octet-first; a
//!    received IPN `<=` the stored one is a replay; the stored IPN advances only
//!    **after** the MIC verifies.
//!  - **IGTK KDE** (T-057): `KeyID(2 LE) ‖ IPN(6) ‖ IGTK(16)`, wrapped `DD ‖ len ‖
//!    00 0F AC 09` — structurally unlike the 1-byte-keyidx GTK KDE.
//!
//! Compiled only under the `wifi` feature. `aes`/`cmac` reused from the same
//! RustCrypto set the EAPOL v3 MIC uses (`eapol.rs`); AES-CMAC is never
//! hand-rolled (Article 15.7).

extern crate alloc;

use aes::Aes128;
use cmac::{Cmac, Mac};

// IEEE 802.11 Frame Control type/subtype bits — mirrored from `mlme_logic`
// (defined locally so this module resolves identically in the kernel tree and
// in the flattened `ai-ops-tests` mount, where `wifi::mlme_logic` is not a
// sibling of `wpa`).
const FCTL_FTYPE: u16 = 0x000c;
const FCTL_STYPE: u16 = 0x00f0;
const FTYPE_MGMT: u16 = 0x0000;
const STYPE_DISASSOC: u16 = 0x00a0;
const STYPE_DEAUTH: u16 = 0x00c0;

/// Management MIC IE element id (`WLAN_EID_MMIE`, T-054).
pub const MMIE_EID: u8 = 76;
/// The MMIE `length` field for CMAC-128 = key_id(2)+IPN(6)+MIC(8) = 16.
pub const MMIE_LEN_FIELD: u8 = 16;
/// Full MMIE octet count for CMAC-128 (eid+len+key_id+IPN+MIC).
pub const MMIE_LEN: usize = 18;
/// BIP-CMAC-128 MIC length (the CMAC-128 tag is truncated to 8).
pub const BIP_MIC_LEN: usize = 8;
/// The BIP AAD length: masked-FC(2) + A1(6) + A2(6) + A3(6).
pub const AAD_LEN: usize = 20;
/// IGTK packet-number length.
pub const IPN_LEN: usize = 6;
/// The 802.11 MAC header length (matches `mlme_logic::MGMT_HDR_LEN`).
const MGMT_HDR_LEN: usize = 24;

/// FC bits masked out of the AAD (Retry | PwrMgt | MoreData), T-055.
const FCTL_RETRY: u16 = 0x0800;
const FCTL_PM: u16 = 0x1000;
const FCTL_MOREDATA: u16 = 0x2000;

/// The IEEE OUI 00:0F:AC and the IGTK RSN KDE data type (9), T-057.
const RSN_OUI: [u8; 3] = [0x00, 0x0f, 0xac];
const IGTK_KDE_TYPE: u8 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BipError {
    /// The frame is shorter than header + MMIE.
    TooShort,
    /// The trailing IE is not a well-formed MMIE (wrong eid/len).
    BadMmie,
    /// The received IPN is `<=` the stored one (replay).
    Replay,
    /// The BIP-CMAC-128 MIC did not verify.
    MicFail,
}

/// The software IGTK: the 16-byte BIP key, its key id (4 or 5), and the receive
/// replay counter. Lives outside any hardware WCID (T-061).
#[derive(Debug, Clone)]
pub struct Igtk {
    pub key_id: u16,
    pub key: [u8; 16],
    /// Last accepted IPN (wire order, LSB-first). Starts all-zero.
    pub rx_pn: [u8; IPN_LEN],
}

impl Igtk {
    pub fn new(key_id: u16, key: [u8; 16], initial_ipn: [u8; IPN_LEN]) -> Self {
        Igtk {
            key_id,
            key,
            rx_pn: initial_ipn,
        }
    }
}

/// Interpret a 6-octet wire IPN (LSB-first) as a 48-bit counter.
fn ipn_value(ipn: &[u8]) -> u64 {
    let mut v = 0u64;
    for (i, &b) in ipn.iter().take(IPN_LEN).enumerate() {
        v |= (b as u64) << (8 * i);
    }
    v
}

/// The BIP AAD (T-055): `masked-FC ‖ A1 ‖ A2 ‖ A3` (20 octets). `frame` is the
/// full management frame (FC at [0..2]; A1/A2/A3 at [4..10]/[10..16]/[16..22]).
pub fn bip_aad(frame: &[u8]) -> Option<[u8; AAD_LEN]> {
    if frame.len() < MGMT_HDR_LEN {
        return None;
    }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    let masked = fc & !(FCTL_RETRY | FCTL_PM | FCTL_MOREDATA);
    let mut aad = [0u8; AAD_LEN];
    aad[0..2].copy_from_slice(&masked.to_le_bytes());
    aad[2..20].copy_from_slice(&frame[4..22]); // A1 ‖ A2 ‖ A3
    Some(aad)
}

/// Compute the BIP-CMAC-128 MIC over `AAD ‖ (body+MMIE with the 8-byte MIC field
/// zeroed)` and return the 8-octet tag (T-055). `frame` is the full management
/// frame **including** the trailing 18-byte MMIE; the MMIE's own MIC bytes are
/// treated as zero regardless of their current contents.
pub fn bip_cmac_mic(key: &[u8; 16], frame: &[u8]) -> Option<[u8; BIP_MIC_LEN]> {
    if frame.len() < MGMT_HDR_LEN + MMIE_LEN {
        return None;
    }
    let aad = bip_aad(frame)?;
    let mut mac = Cmac::<Aes128>::new_from_slice(key).ok()?;
    mac.update(&aad);
    // Body + MMIE header (eid‖len‖key_id‖IPN), i.e. everything after the 24-byte
    // MAC header up to the start of the 8-byte MIC field, then 8 zero octets.
    let mic_start = frame.len() - BIP_MIC_LEN;
    mac.update(&frame[MGMT_HDR_LEN..mic_start]);
    mac.update(&[0u8; BIP_MIC_LEN]);
    let tag = mac.finalize().into_bytes();
    let mut out = [0u8; BIP_MIC_LEN];
    out.copy_from_slice(&tag[..BIP_MIC_LEN]);
    Some(out)
}

/// Append a BIP-protected MMIE to a management frame (TX). `buf[..body_end]` is
/// the frame (header+body); writes the 18-byte MMIE with `key_id`/`ipn`, computes
/// the MIC over the whole thing, and fills it. BIP does **not** set the FC
/// Protected bit (T-058). Returns the new frame length. (TX is for symmetry /
/// KATs; the on-air deliverable is RX verify.)
pub fn protect_mgmt(
    buf: &mut [u8],
    body_end: usize,
    key: &[u8; 16],
    key_id: u16,
    ipn: &[u8; IPN_LEN],
) -> Option<usize> {
    let end = body_end + MMIE_LEN;
    if buf.len() < end || body_end < MGMT_HDR_LEN {
        return None;
    }
    buf[body_end] = MMIE_EID;
    buf[body_end + 1] = MMIE_LEN_FIELD;
    buf[body_end + 2..body_end + 4].copy_from_slice(&key_id.to_le_bytes());
    buf[body_end + 4..body_end + 10].copy_from_slice(ipn);
    buf[body_end + 10..end].fill(0); // MIC = 0 for the compute
    let mic = bip_cmac_mic(key, &buf[..end])?;
    buf[body_end + 10..end].copy_from_slice(&mic);
    Some(end)
}

/// Verify a received protected management frame's MMIE (RX): locate the trailing
/// MMIE, check eid/len, run the IPN replay check (T-056), then verify the
/// BIP-CMAC-128 MIC. `igtk.rx_pn` advances **only after** the MIC verifies — a
/// forged high-IPN frame with a bad MIC cannot poison the replay window.
pub fn verify_mmie(igtk: &mut Igtk, frame: &[u8]) -> Result<(), BipError> {
    if frame.len() < MGMT_HDR_LEN + MMIE_LEN {
        return Err(BipError::TooShort);
    }
    let mmie = &frame[frame.len() - MMIE_LEN..];
    if mmie[0] != MMIE_EID || mmie[1] != MMIE_LEN_FIELD {
        return Err(BipError::BadMmie);
    }
    let recv_ipn = &mmie[4..10];
    // Replay: received must be strictly greater than the stored counter.
    if ipn_value(recv_ipn) <= ipn_value(&igtk.rx_pn) {
        return Err(BipError::Replay);
    }
    let computed = bip_cmac_mic(&igtk.key, frame).ok_or(BipError::TooShort)?;
    // Constant-time compare (eapol.rs house style).
    let mut diff = 0u8;
    for i in 0..BIP_MIC_LEN {
        diff |= computed[i] ^ mmie[10 + i];
    }
    if diff != 0 {
        return Err(BipError::MicFail);
    }
    igtk.rx_pn.copy_from_slice(recv_ipn);
    Ok(())
}

/// Find and parse the IGTK KDE within a 4-way M3 Key-Data buffer (T-057). Scans
/// for a `DD ‖ len ‖ 00 0F AC ‖ 09` vendor KDE and returns the software IGTK
/// (`KeyID(2 LE) ‖ IPN(6) ‖ IGTK(16)`). The KeyID is a **2-byte LE** field — not
/// the GTK KDE's 1-byte keyidx.
pub fn find_igtk_kde(key_data: &[u8]) -> Option<Igtk> {
    let mut i = 0usize;
    while i + 2 <= key_data.len() {
        let eid = key_data[i];
        let len = key_data[i + 1] as usize;
        let val_start = i + 2;
        if val_start + len > key_data.len() {
            return None;
        }
        // Vendor-specific (0xDD), OUI 00:0F:AC, type IGTK(9).
        if eid == 0xDD
            && len >= 4
            && key_data[val_start..val_start + 3] == RSN_OUI
            && key_data[val_start + 3] == IGTK_KDE_TYPE
        {
            // KeyID(2 LE) + IPN(6) + IGTK(16) = 24 bytes of KDE data after the type.
            let data = &key_data[val_start + 4..val_start + len];
            if data.len() != 2 + IPN_LEN + 16 {
                return None;
            }
            let key_id = u16::from_le_bytes([data[0], data[1]]);
            let mut ipn = [0u8; IPN_LEN];
            ipn.copy_from_slice(&data[2..2 + IPN_LEN]);
            let mut key = [0u8; 16];
            key.copy_from_slice(&data[2 + IPN_LEN..2 + IPN_LEN + 16]);
            return Some(Igtk::new(key_id, key, ipn));
        }
        i = val_start + len;
    }
    None
}

/// True for a robust (PMF-protected) management frame: deauth or disassoc (T-058;
/// robust action frames are also robust but this stack drives only deauth/
/// disassoc on the connect path).
pub fn is_robust_mgmt(fc: u16) -> bool {
    if (fc & FCTL_FTYPE) != FTYPE_MGMT {
        return false;
    }
    let stype = fc & FCTL_STYPE;
    stype == STYPE_DEAUTH || stype == STYPE_DISASSOC
}

/// The decision for a received robust management frame under PMF (T-058 —
/// including the crucial pre-key exception).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MgmtAction {
    /// Not robust / PMF inactive — hand to normal processing.
    Process,
    /// Robust, protected + MMIE-verified — process it.
    ProcessProtected,
    /// Robust + unprotected while a key is installed — drop and ignore (the
    /// forged-deauth-DoS defence; the headline PMF property).
    DropUnprotected,
    /// Robust + unprotected but no key yet — permit (mid-handshake recovery,
    /// mac80211's `if (!rx->key) return RX_CONTINUE`).
    ProcessPreKey,
}

/// Classify a received robust management frame. `mfp_active` = MFP negotiated for
/// this peer; `key_installed` = the IGTK is installed; `has_mmie` = the frame
/// carries a (verified) MMIE. Non-robust frames / MFP-off always `Process`.
pub fn mgmt_action(fc: u16, mfp_active: bool, key_installed: bool, has_mmie: bool) -> MgmtAction {
    if !mfp_active || !is_robust_mgmt(fc) {
        return MgmtAction::Process;
    }
    if has_mmie {
        return MgmtAction::ProcessProtected;
    }
    // Unprotected robust mgmt under active MFP:
    if key_installed {
        MgmtAction::DropUnprotected // forged deauth is ignored (the PASS property)
    } else {
        MgmtAction::ProcessPreKey // pre-key exception: permit during the handshake
    }
}
