// SPDX-License-Identifier: MPL-2.0
//! WPA2 key derivation — Phase 7.5 BUILD-W0 (ISC-1, ISC-2). Kernel-free, built
//! on vetted RustCrypto primitives (never hand-rolled — Article 15.7 / the kTLS
//! precedent). Host-tested against published IEEE 802.11 / RFC known-answer
//! vectors (I-12), never round-trip.
//!
//! - **ISC-1 PMK:** `PBKDF2-HMAC-SHA1(passphrase, SSID, 4096, 256 bits)`.
//! - **ISC-2 PRF + PTK/GTK:** the IEEE 802.11 PRF over HMAC-SHA1, and the
//!   pairwise/group key expansions. The classic PTK bug is the **Min/Max
//!   ordering** of the two MAC addresses and the two nonces in the PRF input —
//!   made explicit here and pinned by a KAT.
//!
//! Spec: IEEE 802.11-2020 §12.7.1.2 (PRF), §12.7.1.3 (PTK), §12.7.1.4 (GTK),
//! §J.4 (PMK/PTK test vectors); RFC 6070 (PBKDF2-HMAC-SHA1 vectors).

extern crate alloc;

use alloc::vec::Vec;

use hmac::{Hmac, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// ISC-1: derive the 256-bit PMK from a passphrase + SSID.
/// `PBKDF2-HMAC-SHA1(passphrase, ssid, 4096, 32 bytes)`.
pub fn pmk_from_passphrase(passphrase: &[u8], ssid: &[u8]) -> [u8; 32] {
    let mut pmk = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha1>(passphrase, ssid, 4096, &mut pmk);
    pmk
}

/// The IEEE 802.11 PRF over HMAC-SHA1 (§12.7.1.2): for each 160-bit block,
/// `HMAC-SHA1(key, label ‖ 0x00 ‖ data ‖ counter_octet)`, concatenated and
/// truncated to `out_bits`. The counter starts at 0 and is a single octet.
pub fn prf_sha1(key: &[u8], label: &[u8], data: &[u8], out_bits: usize) -> Vec<u8> {
    let out_bytes = out_bits / 8;
    let mut result = Vec::with_capacity(out_bytes.next_multiple_of(20));
    let mut counter: u8 = 0;
    while result.len() < out_bytes {
        let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(label);
        mac.update(&[0u8]);
        mac.update(data);
        mac.update(&[counter]);
        result.extend_from_slice(&mac.finalize().into_bytes());
        counter = counter.wrapping_add(1);
    }
    result.truncate(out_bytes);
    result
}

/// Order two equal-length byte strings as `(min, max)` by lexicographic
/// (byte-wise) comparison — the IEEE 802.11 ordering for the PTK PRF input.
fn min_max<'a>(a: &'a [u8], b: &'a [u8]) -> (&'a [u8], &'a [u8]) {
    if a <= b { (a, b) } else { (b, a) }
}

/// ISC-2: the raw pairwise key (PTK), `out_bits` long, via the 802.11 PRF.
/// **The trap:** the PRF data is `Min(AA,SPA) ‖ Max(AA,SPA) ‖ Min(ANonce,SNonce)
/// ‖ Max(ANonce,SNonce)` — the addresses and nonces are each ordered, NOT taken
/// in (AA,SPA)/(ANonce,SNonce) order. Getting either ordering wrong yields a
/// wrong-but-plausible PTK that only the published KAT catches.
pub fn ptk_raw(
    pmk: &[u8],
    aa: &[u8; 6],
    spa: &[u8; 6],
    anonce: &[u8; 32],
    snonce: &[u8; 32],
    out_bits: usize,
) -> Vec<u8> {
    let (min_a, max_a) = min_max(aa, spa);
    let (min_n, max_n) = min_max(anonce, snonce);
    let mut data = Vec::with_capacity(76);
    data.extend_from_slice(min_a);
    data.extend_from_slice(max_a);
    data.extend_from_slice(min_n);
    data.extend_from_slice(max_n);
    prf_sha1(pmk, b"Pairwise key expansion", &data, out_bits)
}

/// The CCMP pairwise transient key split (§12.7.1.3): a 384-bit PTK divides into
/// the 128-bit KCK (EAPOL-Key MIC), 128-bit KEK (key-data encryption), and
/// 128-bit TK (CCMP data encryption).
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct CcmpPtk {
    pub kck: [u8; 16],
    pub kek: [u8; 16],
    pub tk: [u8; 16],
}

/// ISC-2: derive and split the CCMP PTK (384 bits).
pub fn ptk_ccmp(
    pmk: &[u8],
    aa: &[u8; 6],
    spa: &[u8; 6],
    anonce: &[u8; 32],
    snonce: &[u8; 32],
) -> CcmpPtk {
    let raw = ptk_raw(pmk, aa, spa, anonce, snonce, 384);
    let mut kck = [0u8; 16];
    let mut kek = [0u8; 16];
    let mut tk = [0u8; 16];
    kck.copy_from_slice(&raw[0..16]);
    kek.copy_from_slice(&raw[16..32]);
    tk.copy_from_slice(&raw[32..48]);
    CcmpPtk { kck, kek, tk }
}

/// ISC-2: the group temporal key (GTK), `out_bits` long (§12.7.1.4):
/// `PRF(GMK, "Group key expansion", AA ‖ GNonce, out_bits)`.
pub fn gtk(gmk: &[u8], aa: &[u8; 6], gnonce: &[u8; 32], out_bits: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(38);
    data.extend_from_slice(aa);
    data.extend_from_slice(gnonce);
    prf_sha1(gmk, b"Group key expansion", &data, out_bits)
}

/// ISC-5: the RSN PMKID (IEEE 802.11 §12.7.1.3), which names a cached PMK in the
/// RSNE of EAPOL-Key message 1 / (re)association:
/// `Truncate-128(HMAC-SHA1(PMK, "PMK Name" ‖ AA ‖ SPA))`. The label is the
/// 8-octet ASCII string `PMK Name` (no NUL); AA (Authenticator) precedes SPA
/// (Supplicant). This is the WPA2-PSK / SHA1 AKM derivation; the SHA256/SHA384
/// AKMs (FT, Suite-B) truncate an HMAC-SHA256/384 instead and are out of scope.
pub fn pmkid(pmk: &[u8], aa: &[u8; 6], spa: &[u8; 6]) -> [u8; 16] {
    let mut mac = HmacSha1::new_from_slice(pmk).expect("HMAC accepts any key length");
    mac.update(b"PMK Name");
    mac.update(aa);
    mac.update(spa);
    let tag = mac.finalize().into_bytes();
    let mut id = [0u8; 16];
    id.copy_from_slice(&tag[..16]);
    id
}
