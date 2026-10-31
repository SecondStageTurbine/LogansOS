// SPDX-License-Identifier: MPL-2.0
//! AES-CCMP (IEEE 802.11 §12.5.3) — Phase 7.5 BUILD-W0 (ISC-4). The 802.11
//! data-confidentiality protocol: AES in CCM mode (CTR + CBC-MAC), M=8 (an
//! 8-octet MIC), L=2 (a 13-octet nonce). Kernel-free, RustCrypto-backed
//! (`aes` + `ccm` — never hand-rolled, Article 15.7), host-tested against
//! published vectors (I-12).
//!
//! The crypto core (AES-CCM with associated data) is the audited `ccm` crate,
//! driven exactly as the kTLS record layer drives `chacha20poly1305`
//! (`AeadInPlace::encrypt_in_place_detached`). What this file *adds* — and what
//! the KATs must pin — is the **802.11-specific construction** of the CCM nonce
//! and AAD, which carries two classic traps:
//!
//!  - **Nonce = priority(1) ‖ A2(6) ‖ PN(6, big-endian).** The 48-bit Packet
//!    Number goes in MSB-first (PN5 … PN0), the *reverse* of its little-endian
//!    on-wire CCMP-header order. The priority octet is the QoS TID (data) or
//!    0x10 (management), 0 for non-QoS data.
//!  - **AAD = masked-FC ‖ A1‖A2‖A3 ‖ masked-SeqCtrl ‖ [A4] ‖ [masked-QoS].**
//!    The Frame Control's mutable bits (Retry/PwrMgmt/MoreData, the subtype low
//!    bits for data, HTC/Order for QoS) are zeroed and Protected is forced on;
//!    the Sequence-Number subfield is masked out (the Fragment number is kept).
//!    Authenticating the mutable bits would make every retransmit fail its MIC.
//!
//! **Proof discipline (I-12 + the repo's "format/ABI specs require an
//! independent byte oracle" rule):** the CCM core is pinned by the RFC 3610
//! packet vectors (AES-128, M=8, L=2 — *exactly* CCMP's configuration), and the
//! 802.11 nonce/AAD byte layout is pinned by a hand-laid byte oracle computed
//! from the spec, independent of this code. Never round-trip.
//!
//! Compiled only under the `wifi` feature.

extern crate alloc;

use alloc::vec::Vec;

use aes::Aes128;
use ccm::Ccm;
use ccm::aead::{AeadInPlace, KeyInit, Nonce, Tag};
use ccm::consts::{U8, U13};

/// CCMP uses AES-128 in CCM with an 8-octet MIC (`U8`) and a 13-octet nonce
/// (`U13`, ⇒ L = 15 − 13 = 2).
type Aes128Ccm = Ccm<Aes128, U8, U13>;

/// CCMP MIC length (octets).
pub const CCMP_MIC_LEN: usize = 8;
/// CCMP header length (octets): PN0 PN1 Rsvd KeyID PN2 PN3 PN4 PN5.
pub const CCMP_HDR_LEN: usize = 8;

// ── Frame Control bits (IEEE 802.11 §9.2.4.1; little-endian u16). ────────────
const FC_TODS: u16 = 0x0100;
const FC_FROMDS: u16 = 0x0200;
const FC_RETRY: u16 = 0x0800;
const FC_PWRMGT: u16 = 0x1000;
const FC_MOREDATA: u16 = 0x2000;
const FC_PROTECTED: u16 = 0x4000;
const FC_ORDER_HTC: u16 = 0x8000;
const FC_TYPE_MGMT: u16 = 0;
const FC_TYPE_DATA: u16 = 2;

#[inline]
fn fc_type(fc: u16) -> u16 {
    (fc >> 2) & 0x3
}
#[inline]
fn fc_subtype(fc: u16) -> u16 {
    (fc >> 4) & 0xF
}
#[inline]
fn is_qos_data(fc: u16) -> bool {
    fc_type(fc) == FC_TYPE_DATA && (fc_subtype(fc) & 0x08) != 0
}
#[inline]
fn has_addr4(fc: u16) -> bool {
    (fc & (FC_TODS | FC_FROMDS)) == (FC_TODS | FC_FROMDS)
}

/// Offset of the QoS Control field within the MAC header (after A1/A2/A3 and the
/// Sequence Control field, plus A4 for a 4-address WDS frame).
#[inline]
fn qos_offset(fc: u16) -> usize {
    if has_addr4(fc) { 30 } else { 24 }
}

/// The nonce priority octet (IEEE 802.11 §12.5.3.3.4): QoS TID for QoS data,
/// 0x10 for management, 0 otherwise.
pub fn nonce_priority(header: &[u8]) -> u8 {
    if header.len() < 2 {
        return 0;
    }
    let fc = u16::from_le_bytes([header[0], header[1]]);
    if fc_type(fc) == FC_TYPE_MGMT {
        return 0x10;
    }
    if is_qos_data(fc) {
        let off = qos_offset(fc);
        if header.len() > off {
            return header[off] & 0x0f;
        }
    }
    0
}

/// Build the 13-octet CCM nonce: priority ‖ A2 ‖ PN(big-endian, PN5…PN0).
/// `pn` is the 48-bit Packet Number as `[PN0, PN1, PN2, PN3, PN4, PN5]`
/// (PN0 = least-significant octet, the first transmitted in the CCMP header).
pub fn build_nonce(priority: u8, a2: &[u8; 6], pn: &[u8; 6]) -> [u8; 13] {
    let mut nonce = [0u8; 13];
    nonce[0] = priority;
    nonce[1..7].copy_from_slice(a2);
    nonce[7] = pn[5];
    nonce[8] = pn[4];
    nonce[9] = pn[3];
    nonce[10] = pn[2];
    nonce[11] = pn[1];
    nonce[12] = pn[0];
    nonce
}

/// Build the CCMP Additional Authentication Data from a plaintext 802.11 MAC
/// header (IEEE 802.11 §12.5.3.3.3). The Frame Control mutable bits are masked
/// and Protected forced on; the Sequence Number is masked out (Fragment kept);
/// A4 is included for WDS frames and the masked QoS Control for QoS frames.
/// `None` if the header is too short for the address fields it claims.
pub fn build_aad(header: &[u8]) -> Option<Vec<u8>> {
    if header.len() < 24 {
        return None;
    }
    let mut fc = u16::from_le_bytes([header[0], header[1]]);
    let qos = is_qos_data(fc);
    let addr4 = has_addr4(fc);

    if fc_type(fc) == FC_TYPE_DATA {
        fc &= !0x0070; // mask the data subtype's low bits
        if qos {
            fc &= !FC_ORDER_HTC;
        }
    }
    fc &= !(FC_RETRY | FC_PWRMGT | FC_MOREDATA);
    fc |= FC_PROTECTED;

    let mut aad = Vec::with_capacity(30);
    aad.extend_from_slice(&fc.to_le_bytes());
    // A1 ‖ A2 ‖ A3 (contiguous at offset 4).
    aad.extend_from_slice(&header[4..22]);
    // Sequence Control with the Sequence Number subfield masked to zero.
    let seq = u16::from_le_bytes([header[22], header[23]]) & 0x000f;
    aad.extend_from_slice(&seq.to_le_bytes());

    let mut off = 24;
    if addr4 {
        if header.len() < 30 {
            return None;
        }
        aad.extend_from_slice(&header[24..30]);
        off = 30;
    }
    if qos {
        if header.len() < off + 2 {
            return None;
        }
        // QoS Control: TID kept; TXOP/EOSP/ack-policy/A-MSDU bits masked.
        let qc0 = header[off] & !0x70 & !0x80;
        aad.push(qc0);
        aad.push(0x00);
    }
    Some(aad)
}

/// Build the 8-octet CCMP header for a Packet Number and Key ID: PN0 PN1 0x00
/// (0x20|KeyID<<6) PN2 PN3 PN4 PN5 (the 0x20 ExtIV bit is always set).
pub fn ccmp_header(pn: &[u8; 6], keyid: u8) -> [u8; 8] {
    [
        pn[0],
        pn[1],
        0x00,
        0x20 | (keyid << 6),
        pn[2],
        pn[3],
        pn[4],
        pn[5],
    ]
}

/// Extract the 48-bit Packet Number from a received CCMP header (the inverse of
/// `ccmp_header`): the layout is PN0 PN1 _ _ PN2 PN3 PN4 PN5, so the PN as a
/// little-endian-valued integer is PN0 | PN1<<8 | PN2<<16 | PN3<<24 | PN4<<32 |
/// PN5<<40. `None` if the header is short. (W2 ISC-3 RX/replay.)
pub fn pn_from_header(hdr: &[u8]) -> Option<u64> {
    if hdr.len() < CCMP_HDR_LEN {
        return None;
    }
    Some(
        (hdr[0] as u64)
            | (hdr[1] as u64) << 8
            | (hdr[4] as u64) << 16
            | (hdr[5] as u64) << 24
            | (hdr[6] as u64) << 32
            | (hdr[7] as u64) << 40,
    )
}

/// CCMP replay protection (IEEE 802.11 §12.5.3.4.4): the receiver keeps the
/// highest accepted PN per key; a frame whose PN does not strictly advance it is
/// a replay (or out-of-order) and is dropped. A single counter suffices for the
/// non-QoS / single-TID path W2 brings up; per-TID windows are a later refinement.
#[derive(Default)]
pub struct ReplayWindow {
    last: u64,
}

impl ReplayWindow {
    pub const fn new() -> Self {
        Self { last: 0 }
    }
    /// Accept `pn` if it strictly advances the window (recording it); reject
    /// (return false) a replayed or out-of-order PN.
    pub fn accept(&mut self, pn: u64) -> bool {
        if pn > self.last {
            self.last = pn;
            true
        } else {
            false
        }
    }
    pub fn last(&self) -> u64 {
        self.last
    }
}

/// AES-CCM encrypt (M=8, L=2): returns `ciphertext ‖ 8-octet MIC`. `None` on a
/// bad key length or a message too long for L=2 (≥ 2¹⁶ octets).
pub fn aes_ccm_ae(tk: &[u8; 16], nonce: &[u8; 13], aad: &[u8], msg: &[u8]) -> Option<Vec<u8>> {
    let cipher = Aes128Ccm::new_from_slice(tk).ok()?;
    let mut buf = Vec::from(msg);
    let tag = cipher
        .encrypt_in_place_detached(Nonce::<Aes128Ccm>::from_slice(nonce), aad, &mut buf)
        .ok()?;
    buf.extend_from_slice(tag.as_slice());
    Some(buf)
}

/// AES-CCM decrypt + authenticate. `data` is `ciphertext ‖ 8-octet MIC`.
/// Returns the plaintext, or `None` if the MIC fails (constant-time in `ccm`).
pub fn aes_ccm_ad(tk: &[u8; 16], nonce: &[u8; 13], aad: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < CCMP_MIC_LEN {
        return None;
    }
    let (ct, tag) = data.split_at(data.len() - CCMP_MIC_LEN);
    let cipher = Aes128Ccm::new_from_slice(tk).ok()?;
    let mut buf = Vec::from(ct);
    cipher
        .decrypt_in_place_detached(
            Nonce::<Aes128Ccm>::from_slice(nonce),
            aad,
            &mut buf,
            Tag::<Aes128Ccm>::from_slice(tag),
        )
        .ok()?;
    Some(buf)
}

/// CCMP encapsulation: encrypt `plaintext` under the temporal key `tk` for the
/// given plaintext MAC `header`, Packet Number `pn`, and `keyid`. Returns the
/// protected MPDU: `header(Protected set) ‖ CCMP-header(8) ‖ ciphertext ‖ MIC(8)`.
/// `None` if the header is malformed.
pub fn ccmp_encrypt(
    tk: &[u8; 16],
    header: &[u8],
    pn: &[u8; 6],
    keyid: u8,
    plaintext: &[u8],
) -> Option<Vec<u8>> {
    let a2: [u8; 6] = header.get(10..16)?.try_into().ok()?;
    let aad = build_aad(header)?;
    let nonce = build_nonce(nonce_priority(header), &a2, pn);
    let core = aes_ccm_ae(tk, &nonce, &aad, plaintext)?;

    let mut out = Vec::with_capacity(header.len() + CCMP_HDR_LEN + core.len());
    out.extend_from_slice(header);
    let fc = u16::from_le_bytes([out[0], out[1]]) | FC_PROTECTED;
    out[0..2].copy_from_slice(&fc.to_le_bytes());
    out.extend_from_slice(&ccmp_header(pn, keyid));
    out.extend_from_slice(&core);
    Some(out)
}

/// CCMP decapsulation: the inverse of [`ccmp_encrypt`]. `mpdu` is
/// `header(hdrlen) ‖ CCMP-header(8) ‖ ciphertext ‖ MIC(8)`; the PN is read back
/// from the CCMP header. Returns the plaintext, or `None` if too short or the
/// MIC fails.
pub fn ccmp_decrypt(tk: &[u8; 16], mpdu: &[u8], hdrlen: usize) -> Option<Vec<u8>> {
    if hdrlen < 24 || mpdu.len() < hdrlen + CCMP_HDR_LEN + CCMP_MIC_LEN {
        return None;
    }
    let header = &mpdu[..hdrlen];
    let a2: [u8; 6] = header.get(10..16)?.try_into().ok()?;
    let ch = &mpdu[hdrlen..hdrlen + CCMP_HDR_LEN];
    // CCMP header → PN: PN0 PN1 (rsvd) (keyid) PN2 PN3 PN4 PN5.
    let pn = [ch[0], ch[1], ch[4], ch[5], ch[6], ch[7]];
    let aad = build_aad(header)?;
    let nonce = build_nonce(nonce_priority(header), &a2, &pn);
    aes_ccm_ad(tk, &nonce, &aad, &mpdu[hdrlen + CCMP_HDR_LEN..])
}
