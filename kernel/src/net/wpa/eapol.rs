// SPDX-License-Identifier: MPL-2.0
//! EAPOL-Key frame parse + Key MIC — Phase 7.5 BUILD-W0 (ISC-3). Kernel-free,
//! RustCrypto-backed, host-tested against published vectors (IWD
//! `unit/test-eapol.c`; I-12).
//!
//! The EAPOL-Key descriptor (IEEE 802.11 §12.7.2) carries the 4-way handshake.
//! Two traps, both pinned by KATs:
//!  - **Zero-the-MIC-before-compute:** the Key MIC is computed over the WHOLE
//!    EAPOL-Key frame with the 16-byte MIC field set to zero. Forgetting to
//!    zero it (computing over the already-filled field) is the classic bug.
//!  - **Version → algorithm:** the Key-Descriptor-Version field (low 3 bits of
//!    Key Information) selects the MIC algorithm: v1 = HMAC-MD5 (WPA1/TKIP, out
//!    of scope), **v2 = HMAC-SHA1-128**, **v3 = AES-CMAC-128**.

extern crate alloc;

use aes::Aes128;
use cmac::Cmac;
use hmac::{Hmac, Mac};
use sha1::Sha1;

// ── EAPOL-Key frame layout (IEEE 802.11 §12.7.2; offsets from the EAPOL
// header start: version(1) type(1) length(2), then the Key descriptor). ─────────
/// Offset of the descriptor Type octet (0x02 = RSN, 0xfe = WPA1).
pub const DESCRIPTOR_TYPE_OFFSET: usize = 4;
/// Offset of the 2-octet Key Information field (big-endian).
pub const KEY_INFO_OFFSET: usize = 5;
/// Offset of the 32-octet Key Nonce.
pub const KEY_NONCE_OFFSET: usize = 17;
/// Offset of the 16-octet Key MIC field (computed over the frame with this
/// field zeroed).
pub const KEY_MIC_OFFSET: usize = 81;
/// Key MIC length (16 octets for the WPA2/WPA3 algorithms in scope).
pub const KEY_MIC_LEN: usize = 16;
/// Offset of the 2-octet Key Data Length (big-endian), just past the MIC.
pub const KEY_DATA_LEN_OFFSET: usize = KEY_MIC_OFFSET + KEY_MIC_LEN;
/// Offset of the Key Data field.
pub const KEY_DATA_OFFSET: usize = KEY_DATA_LEN_OFFSET + 2;

// ── Key Information bits (IEEE 802.11 §12.7.2; for building M2/M4) ───────────
pub const KEY_INFO_KEY_TYPE: u16 = 1 << 3; // 1 = pairwise
pub const KEY_INFO_INSTALL: u16 = 1 << 6;
pub const KEY_INFO_ACK: u16 = 1 << 7;
pub const KEY_INFO_MIC: u16 = 1 << 8;
pub const KEY_INFO_SECURE: u16 = 1 << 9;
pub const KEY_INFO_ENCRYPTED: u16 = 1 << 12; // encrypted Key Data
/// EAPOL packet type for an EAPOL-Key frame, and the RSN descriptor type.
pub const EAPOL_TYPE_KEY: u8 = 3;
pub const DESCRIPTOR_TYPE_RSN: u8 = 2;

/// The Key-Descriptor-Version (low 3 bits of Key Information) → MIC algorithm.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum KeyDescriptorVersion {
    /// v1 — HMAC-MD5 + ARC4 (WPA1/TKIP). Out of scope; no MIC computed.
    HmacMd5Rc4,
    /// v2 — HMAC-SHA1-128 + AES key wrap (WPA2-CCMP).
    HmacSha1Aes,
    /// v3 — AES-128-CMAC (WPA2/WPA3 with the AES-CMAC AKM).
    AesCmac,
    /// Any other (reserved) value — unsupported.
    Unknown(u8),
}

/// Decode the Key-Descriptor-Version from a 16-bit Key Information field.
pub const fn descriptor_version(key_info: u16) -> KeyDescriptorVersion {
    match (key_info & 0x0007) as u8 {
        1 => KeyDescriptorVersion::HmacMd5Rc4,
        2 => KeyDescriptorVersion::HmacSha1Aes,
        3 => KeyDescriptorVersion::AesCmac,
        other => KeyDescriptorVersion::Unknown(other),
    }
}

/// A parsed, borrowed view over an EAPOL-Key frame (no copy). `None` if the
/// buffer is too short to hold the fixed descriptor up to the MIC.
#[derive(Copy, Clone, Debug)]
pub struct KeyFrame<'a> {
    bytes: &'a [u8],
}

impl<'a> KeyFrame<'a> {
    /// Parse `bytes` as an EAPOL-Key frame; `None` if shorter than the fixed
    /// header through the Key MIC field.
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < KEY_DATA_LEN_OFFSET + 2 {
            return None;
        }
        Some(KeyFrame { bytes })
    }

    pub fn key_info(&self) -> u16 {
        u16::from_be_bytes([self.bytes[KEY_INFO_OFFSET], self.bytes[KEY_INFO_OFFSET + 1]])
    }
    pub fn version(&self) -> KeyDescriptorVersion {
        descriptor_version(self.key_info())
    }
    pub fn nonce(&self) -> &'a [u8] {
        &self.bytes[KEY_NONCE_OFFSET..KEY_NONCE_OFFSET + 32]
    }
    pub fn mic(&self) -> &'a [u8] {
        &self.bytes[KEY_MIC_OFFSET..KEY_MIC_OFFSET + KEY_MIC_LEN]
    }
    pub fn key_data_len(&self) -> u16 {
        u16::from_be_bytes([
            self.bytes[KEY_DATA_LEN_OFFSET],
            self.bytes[KEY_DATA_LEN_OFFSET + 1],
        ])
    }
}

/// Compute the Key MIC over `frame` (whose MIC field is ALREADY zeroed), using
/// the algorithm selected by `version`. v2 = first 16 octets of HMAC-SHA1;
/// v3 = the 16-octet AES-128-CMAC. Returns `None` for the unsupported v1/other.
/// The KCK is 16 octets.
pub fn compute_mic(version: KeyDescriptorVersion, kck: &[u8], frame: &[u8]) -> Option<[u8; 16]> {
    let mut mic = [0u8; 16];
    match version {
        KeyDescriptorVersion::HmacSha1Aes => {
            let mut mac = Hmac::<Sha1>::new_from_slice(kck).ok()?;
            mac.update(frame);
            let tag = mac.finalize().into_bytes();
            mic.copy_from_slice(&tag[..16]);
            Some(mic)
        }
        KeyDescriptorVersion::AesCmac => {
            let mut mac = Cmac::<Aes128>::new_from_slice(kck).ok()?;
            mac.update(frame);
            let tag = mac.finalize().into_bytes();
            mic.copy_from_slice(&tag[..16]);
            Some(mic)
        }
        _ => None,
    }
}

/// Zero the MIC field, compute the MIC, and write it back into `frame` — the
/// "zero-before-compute" sequence made unbypassable. Returns false on a short
/// frame or unsupported version.
pub fn install_mic(version: KeyDescriptorVersion, kck: &[u8], frame: &mut [u8]) -> bool {
    if frame.len() < KEY_MIC_OFFSET + KEY_MIC_LEN {
        return false;
    }
    frame[KEY_MIC_OFFSET..KEY_MIC_OFFSET + KEY_MIC_LEN].fill(0);
    match compute_mic(version, kck, frame) {
        Some(mic) => {
            frame[KEY_MIC_OFFSET..KEY_MIC_OFFSET + KEY_MIC_LEN].copy_from_slice(&mic);
            true
        }
        None => false,
    }
}

/// Verify a received frame's Key MIC: extract it, zero the field on a working
/// copy, recompute, and compare in constant time. Returns false on a short
/// frame or unsupported version.
pub fn verify_mic(version: KeyDescriptorVersion, kck: &[u8], frame: &[u8]) -> bool {
    if frame.len() < KEY_MIC_OFFSET + KEY_MIC_LEN {
        return false;
    }
    // The MIC is computed over EXACTLY the EAPOL-Key PDU: the 4-byte EAPOL header
    // plus its Packet Body Length field (bytes 2..4, big-endian). A received M3
    // arrives inside an 802.11 DATA frame that can carry trailing bytes after the
    // PDU (the FCS the chip delivers, and/or 802.11 padding), and the extraction
    // hands those back too. The AP MIC'd only the PDU, so we must bound to the
    // declared length or the recomputed MIC covers slop the AP never saw and the
    // compare always fails. (M2/M4 we build to an exact length, so this only bit
    // the received frames — the silicon "M3 MIC mismatch" with a correct PTK.)
    let body_len = u16::from_be_bytes([frame[2], frame[3]]) as usize;
    let pdu_len = (4 + body_len).min(frame.len());
    if pdu_len < KEY_MIC_OFFSET + KEY_MIC_LEN {
        return false;
    }
    let frame = &frame[..pdu_len];
    let mut received = [0u8; 16];
    received.copy_from_slice(&frame[KEY_MIC_OFFSET..KEY_MIC_OFFSET + KEY_MIC_LEN]);

    let mut zeroed = alloc::vec::Vec::from(frame);
    zeroed[KEY_MIC_OFFSET..KEY_MIC_OFFSET + KEY_MIC_LEN].fill(0);
    let Some(computed) = compute_mic(version, kck, &zeroed) else {
        return false;
    };
    // Constant-time compare (the MIC is a 16-byte authenticator).
    let mut diff = 0u8;
    for i in 0..16 {
        diff |= received[i] ^ computed[i];
    }
    diff == 0
}

/// Build an EAPOL-Key frame (the supplicant's M2/M4) into `buf`, leaving the MIC
/// field zeroed for a subsequent `install_mic`. The fixed descriptor is 99 bytes
/// (`KEY_DATA_OFFSET`); `key_data` (empty for WPA2-PSK M2/M4) is appended and its
/// length recorded. `replay_counter` echoes the AP's (M1 for M2, M3 for M4),
/// `nonce` is the SNonce in M2 (zeroed in M4), `key_len` is the pairwise cipher
/// key length (16 for CCMP). Returns the total frame length.
pub fn build_key_frame(
    buf: &mut [u8],
    eapol_version: u8,
    key_info: u16,
    key_len: u16,
    replay_counter: &[u8; 8],
    nonce: &[u8; 32],
    key_data: &[u8],
) -> Option<usize> {
    let total = KEY_DATA_OFFSET + key_data.len();
    if buf.len() < total {
        return None;
    }
    for b in buf[..total].iter_mut() {
        *b = 0;
    }
    // EAPOL header: version, type=Key, body length (big-endian) = total - 4.
    buf[0] = eapol_version;
    buf[1] = EAPOL_TYPE_KEY;
    buf[2..4].copy_from_slice(&((total - 4) as u16).to_be_bytes());
    // Key descriptor.
    buf[DESCRIPTOR_TYPE_OFFSET] = DESCRIPTOR_TYPE_RSN;
    buf[KEY_INFO_OFFSET..KEY_INFO_OFFSET + 2].copy_from_slice(&key_info.to_be_bytes());
    buf[7..9].copy_from_slice(&key_len.to_be_bytes()); // Key Length
    buf[9..17].copy_from_slice(replay_counter); // Key Replay Counter
    buf[KEY_NONCE_OFFSET..KEY_NONCE_OFFSET + 32].copy_from_slice(nonce);
    // EAPOL IV (49..65), Key RSC (65..73), Reserved (73..81), MIC (81..97) = 0.
    buf[KEY_DATA_LEN_OFFSET..KEY_DATA_LEN_OFFSET + 2]
        .copy_from_slice(&(key_data.len() as u16).to_be_bytes());
    buf[KEY_DATA_OFFSET..total].copy_from_slice(key_data);
    Some(total)
}

/// The RSN GTK KDE selector (`00-0F-AC`, data type 1 = GROUPKEY).
pub const RSN_OUI: [u8; 3] = [0x00, 0x0f, 0xac];
/// RSN KDE data type for the group key (`RSN_KEY_DATA_GROUPKEY`).
pub const GTK_KDE_TYPE: u8 = 1;

/// CB-1 (T-038): pull the GTK out of the (already AES-key-unwrapped) M3 Key Data.
/// Walk the KDE list for the GTK-KDE and return `(gtk_bytes, key_index)`, or
/// `None` if absent/malformed.
///
/// Byte layout — confirmed against RAW hostap source (per the CLAUDE.md byte-oracle
/// rule; `w1.fi/cgit/hostap/plain/src/common/wpa_common.h`):
/// `struct wpa_gtk_kde { u8 keyid; u8 reserved; u8 gtk[]; }`, `WPA_GTK_KDE_PREFIX_LEN = 2`,
/// `RSN_KEY_DATA_GROUPKEY = RSN_SELECTOR(0x00,0x0f,0xac,1)`. So one KDE is:
/// `DD | len | [00 0F AC 01] | keyid(b0-1=KeyID, b2=Tx) | reserved | GTK[16 for CCMP]`,
/// with `len = 4(selector) + 2(prefix) + gtk_len` (0x16 ⇒ gtk_len = 16 for CCMP).
pub fn gtk_from_kde(kd: &[u8]) -> Option<(&[u8], u8)> {
    let mut i = 0;
    while i + 2 <= kd.len() {
        if kd[i] != 0xdd {
            break; // padding / end of the KDE list
        }
        let len = kd[i + 1] as usize;
        let body = i + 2;
        if len < 6 || body + len > kd.len() {
            break;
        }
        if kd[body..body + 3] == RSN_OUI && kd[body + 3] == GTK_KDE_TYPE {
            // data (after the 4-byte selector) = keyid(1) | reserved(1) | GTK
            let data = &kd[body + 4..body + len];
            if data.len() >= 2 + 16 {
                return Some((&data[2..], data[0] & 0x03));
            }
        }
        i = body + len;
    }
    None
}

/// CB-1 ISC-6 kill-switch gate (host-testable): the group-key install decision.
/// Returns `Some((gtk, key_index))` to install, or `None` to leave the link
/// pairwise-only. When `group_rx_enabled` is `false` this ALWAYS returns `None`
/// — even with a valid GTK in the M3 key-data — so the link is association +
/// pairwise-CCMP only, bit-for-bit the W2 baseline (no WCID-0 write, no broadcast
/// RX). On, it defers to `gtk_from_kde`.
pub fn group_key_to_install(
    group_rx_enabled: bool,
    unwrapped_key_data: &[u8],
) -> Option<(&[u8], u8)> {
    if !group_rx_enabled {
        return None;
    }
    gtk_from_kde(unwrapped_key_data)
}

// ── CB-5a ISC-4: the standalone Group-Key-Handshake (GKH) ────────────────────

/// GKH Message 1 (AP→STA) Key Information (trap-mine T-060, corrected): version 3
/// (AES-CMAC) | ACK | MIC | SECURE | ENCRYPTED, with KEY_TYPE **and INSTALL both
/// clear** = `0x1383`. Install=1 is a 4-way-M3-only bit; iwd's
/// `eapol_verify_gtk_1_of_2` rejects it on a GKH frame (the earlier `0x13C3`
/// value was wrong).
pub const GKH_M1_KEY_INFO: u16 =
    3 | KEY_INFO_ACK | KEY_INFO_MIC | KEY_INFO_SECURE | KEY_INFO_ENCRYPTED;
/// GKH Message 2 (STA→AP ACK) Key Information: version 3 | MIC | SECURE, group,
/// no ACK/INSTALL/ENCRYPTED = `0x0303`.
pub const GKH_M2_KEY_INFO: u16 = 3 | KEY_INFO_MIC | KEY_INFO_SECURE;

/// Verify a received Group-Key-Handshake Message 1 (AP→STA). Per T-060 the Key
/// Information must be group (KEY_TYPE clear), **INSTALL clear**, ACK+MIC+SECURE+
/// ENCRYPTED set, descriptor version 3 (AES-CMAC). Returns false for a
/// 4-way-M3-style `Install=1` frame — the corrected trap.
pub fn verify_gkh_m1(kf: &KeyFrame) -> bool {
    let ki = kf.key_info();
    ki & KEY_INFO_KEY_TYPE == 0
        && ki & KEY_INFO_INSTALL == 0
        && ki & KEY_INFO_ACK != 0
        && ki & KEY_INFO_MIC != 0
        && ki & KEY_INFO_SECURE != 0
        && ki & KEY_INFO_ENCRYPTED != 0
        && matches!(descriptor_version(ki), KeyDescriptorVersion::AesCmac)
}

/// Build the Group-Key-Handshake Message 2 ACK (STA→AP): `key_info = 0x0303`,
/// empty Key Data, zeroed nonce/IV/RSC, MIC left for `install_mic`,
/// `replay_counter` echoing M1. Returns the frame length.
pub fn build_gkh_m2(buf: &mut [u8], eapol_version: u8, replay_counter: &[u8; 8]) -> Option<usize> {
    build_key_frame(
        buf,
        eapol_version,
        GKH_M2_KEY_INFO,
        0,
        replay_counter,
        &[0u8; 32],
        &[],
    )
}
