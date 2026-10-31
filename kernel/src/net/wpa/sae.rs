// SPDX-License-Identifier: MPL-2.0
//! WPA3 SAE (Simultaneous Authentication of Equals — "dragonfly") over NIST
//! P-256 (group 19), **Hash-to-Element (H2E) only** — Phase 7.5 BUILD-W0b
//! (ISC-1..6; ADR-051, ANSPEC §10.9). Kernel-free, RustCrypto-backed, host-
//! tested against published cross-source vectors (I-12, never round-trip).
//!
//! H2E is the modern, constant-time-by-construction PWE derivation that WPA3
//! negotiates; the legacy hunting-and-pecking loop (with its documented timing
//! side-channel) is **deliberately not implemented**.
//!
//! **The one hard rule (Article 15.7 / the W0 + kTLS precedent): never hand-roll
//! the field math, the SSWU map, HKDF, or HMAC.** Every primitive is a vetted
//! RustCrypto crate — `p256` (field/scalar/point ops and its RFC 9380 SSWU
//! `MapToCurve`/`FromOkm` surface), `hkdf`, `hmac`, `sha2`. This module writes
//! only the SAE-specific protocol layers.
//!
//! Pipeline (IEEE 802.11-2020 §12.4):
//!  - **PT (password element, MAC-independent):** `pwd-seed = HKDF-Extract(SSID,
//!    password‖identifier)`; for i∈{1,2} `u_i = HKDF-Expand(pwd-seed, "SAE Hash
//!    to Element u{i} P{i}", 48) mod p`, `P_i = SSWU(u_i)`; `PT = P1 + P2`. The
//!    48→field reduction is exactly `p256`'s `FromOkm` (`d0·2¹⁹²+d1` ≡ a 48-byte
//!    big-endian integer mod p); the SSWU is `p256`'s `MapToCurve` (P-256 has
//!    cofactor 1, so no cofactor clearing).
//!  - **PWE (MAC-bound):** `val = (HKDF-Extract(0, MAX(A,B)‖MIN(A,B)) mod (q−1))
//!    + 1`; `PWE = val · PT`. The MAC **sort order** feeding `val` is the trap.
//!  - **Commit:** `scalar = (rand + mask) mod q`; `element = −(mask · PWE)` (the
//!    inverse — the classic SAE bug); wire = `group(2 LE)‖scalar(32)‖x(32)‖y(32)`.
//!  - **Keys:** `k = (peer-elem + peer-scalar·PWE)·rand` → `k = K.x`; `keyseed =
//!    HMAC-SHA256(0³², k)`; `context = (scalar + peer-scalar) mod q`; `KCK‖PMK =
//!    KDF-SHA256(keyseed, "SAE KCK and PMK", context, 64)`; `PMKID = context[..16]`.
//!  - **Confirm:** `HMAC-SHA256(KCK, send-confirm‖scalar1‖element1‖scalar2‖element2)`.
//!
//! Compiled only under the `wifi` feature (the kill-switch).

extern crate alloc;

use alloc::vec::Vec;

use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use p256::elliptic_curve::Curve;
use p256::elliptic_curve::bigint::{ArrayEncoding, NonZero, U256};
use p256::elliptic_curve::ff::PrimeField;
use p256::elliptic_curve::generic_array::GenericArray;
use p256::elliptic_curve::hash2curve::{FromOkm, MapToCurve};
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::{AffinePoint, EncodedPoint, FieldElement, NistP256, ProjectivePoint, Scalar};

type HmacSha256 = Hmac<Sha256>;

/// Group 19 (NIST P-256) identifier, as it appears on the SAE wire (LE u16).
pub const GROUP_P256: u16 = 19;
/// SAE commit message length: group(2) ‖ scalar(32) ‖ element-x(32) ‖ element-y(32).
pub const COMMIT_LEN: usize = 98;
/// H2E per-element HKDF-Expand length: olen(p) + floor(olen(p)/2) = 32 + 16.
const PWD_VALUE_LEN: usize = 48;

// ── ISC-2: PT derivation (password-only, MAC-independent) ────────────────────

/// pwd-seed = HKDF-Extract(salt=SSID, ikm=password ‖ identifier).
pub fn pwd_seed(ssid: &[u8], password: &[u8], identifier: &[u8]) -> [u8; 32] {
    let mut ikm = Vec::with_capacity(password.len() + identifier.len());
    ikm.extend_from_slice(password);
    ikm.extend_from_slice(identifier);
    let (prk, _) = Hkdf::<Sha256>::extract(Some(ssid), &ikm);
    let mut out = [0u8; 32];
    out.copy_from_slice(&prk);
    out
}

/// pwd-value = HKDF-Expand(pwd-seed, label, 48). `label` is the ASCII string with
/// no trailing NUL (e.g. `b"SAE Hash to Element u1 P1"`).
pub fn pwd_value(pwd_seed: &[u8; 32], label: &[u8]) -> [u8; PWD_VALUE_LEN] {
    let hk = Hkdf::<Sha256>::from_prk(pwd_seed).expect("32-byte PRK is valid");
    let mut okm = [0u8; PWD_VALUE_LEN];
    hk.expand(label, &mut okm).expect("48 octets < 255*hashlen");
    okm
}

/// The 48-octet pwd-value reduced to a field element mod p, as 32 big-endian
/// octets (`p256`'s `FromOkm`: `d0·2¹⁹² + d1` ≡ the 48-byte BE integer mod p).
pub fn field_from_okm(pwd_value: &[u8; PWD_VALUE_LEN]) -> [u8; 32] {
    let u = FieldElement::from_okm(GenericArray::from_slice(pwd_value));
    let mut out = [0u8; 32];
    out.copy_from_slice(&u.to_bytes());
    out
}

/// SSWU-map a pwd-value to a curve point (RFC 9380 simplified-SWU, `p256`'s
/// `MapToCurve`).
pub fn sswu_point(pwd_value: &[u8; PWD_VALUE_LEN]) -> ProjectivePoint {
    FieldElement::from_okm(GenericArray::from_slice(pwd_value)).map_to_curve()
}

/// PT = SSWU(u1) + SSWU(u2) — the MAC-independent password element.
pub fn derive_pt(ssid: &[u8], password: &[u8], identifier: &[u8]) -> ProjectivePoint {
    let seed = pwd_seed(ssid, password, identifier);
    let p1 = sswu_point(&pwd_value(&seed, b"SAE Hash to Element u1 P1"));
    let p2 = sswu_point(&pwd_value(&seed, b"SAE Hash to Element u2 P2"));
    p1 + p2
}

// ── ISC-3: PWE = val · PT (MAC-bound) ────────────────────────────────────────

/// val = (HKDF-Extract(0, MAX(mac1,mac2) ‖ MIN(mac1,mac2)) mod (q−1)) + 1.
/// The MAC sort order (MAX first) is the trap.
pub fn pwe_val(mac1: &[u8; 6], mac2: &[u8; 6]) -> Scalar {
    let mut sorted = [0u8; 12];
    if mac1 > mac2 {
        sorted[..6].copy_from_slice(mac1);
        sorted[6..].copy_from_slice(mac2);
    } else {
        sorted[..6].copy_from_slice(mac2);
        sorted[6..].copy_from_slice(mac1);
    }
    // salt = None ≡ HashLen zeros (hostap uses an explicit zero salt; identical).
    let (val_buf, _) = Hkdf::<Sha256>::extract(None, &sorted);

    let order_minus_1 = NistP256::ORDER.wrapping_sub(&U256::ONE);
    let reduced = U256::from_be_slice(&val_buf) % NonZero::new(order_minus_1).expect("q-1 != 0");
    let val_u256 = reduced.wrapping_add(&U256::ONE);
    Scalar::from_repr(*GenericArray::from_slice(&val_u256.to_be_byte_array()))
        .expect("val in [1, q-1] is a valid scalar")
}

/// PWE = val · PT.
pub fn derive_pwe(pt: &ProjectivePoint, mac1: &[u8; 6], mac2: &[u8; 6]) -> ProjectivePoint {
    *pt * pwe_val(mac1, mac2)
}

// ── point / scalar (de)serialization helpers ─────────────────────────────────

/// A scalar from 32 big-endian octets that must already be in [0, q−1].
fn scalar_from_bytes(b: &[u8; 32]) -> Option<Scalar> {
    Scalar::from_repr(*GenericArray::from_slice(b)).into()
}

/// The affine (x, y) of a point, each 32 big-endian octets; `None` at infinity.
pub fn point_xy(p: &ProjectivePoint) -> Option<([u8; 32], [u8; 32])> {
    let ep = p.to_affine().to_encoded_point(false);
    let (x, y) = (ep.x()?, ep.y()?);
    let mut xb = [0u8; 32];
    let mut yb = [0u8; 32];
    xb.copy_from_slice(x);
    yb.copy_from_slice(y);
    Some((xb, yb))
}

/// A point from affine (x, y) big-endian coordinates; `None` if not on-curve.
fn point_from_xy(x: &[u8; 32], y: &[u8; 32]) -> Option<ProjectivePoint> {
    let ep = EncodedPoint::from_affine_coordinates(
        GenericArray::from_slice(x),
        GenericArray::from_slice(y),
        false,
    );
    let ap = AffinePoint::from_encoded_point(&ep);
    Option::<AffinePoint>::from(ap).map(ProjectivePoint::from)
}

// ── ISC-4: commit construct / parse / serialize ──────────────────────────────

/// A parsed SAE commit: the group id, the commit-scalar, and the commit-element.
pub struct Commit {
    pub group: u16,
    pub scalar: Scalar,
    pub element: ProjectivePoint,
}

/// commit-scalar = (rand + mask) mod q. `None` if either input is ≥ q.
pub fn commit_scalar(rand: &[u8; 32], mask: &[u8; 32]) -> Option<Scalar> {
    Some(scalar_from_bytes(rand)? + scalar_from_bytes(mask)?)
}

/// commit-element = inverse(mask · PWE) = −(mask · PWE). `None` if mask ≥ q.
pub fn commit_element(pwe: &ProjectivePoint, mask: &[u8; 32]) -> Option<ProjectivePoint> {
    Some(-(*pwe * scalar_from_bytes(mask)?))
}

/// Serialize a 98-octet commit: group(2 LE) ‖ scalar(32 BE) ‖ x(32) ‖ y(32).
pub fn serialize_commit(
    group: u16,
    scalar: &Scalar,
    element: &ProjectivePoint,
) -> Option<[u8; COMMIT_LEN]> {
    let (x, y) = point_xy(element)?;
    let mut out = [0u8; COMMIT_LEN];
    out[0..2].copy_from_slice(&group.to_le_bytes());
    out[2..34].copy_from_slice(&scalar.to_bytes());
    out[34..66].copy_from_slice(&x);
    out[66..98].copy_from_slice(&y);
    Some(out)
}

/// Parse + validate a 98-octet peer commit: the group must be P-256, the scalar
/// in [1, q−1], and the element a point on the curve.
pub fn parse_commit(buf: &[u8]) -> Option<Commit> {
    if buf.len() != COMMIT_LEN {
        return None;
    }
    let group = u16::from_le_bytes([buf[0], buf[1]]);
    if group != GROUP_P256 {
        return None;
    }
    let mut s = [0u8; 32];
    s.copy_from_slice(&buf[2..34]);
    if s == [0u8; 32] {
        return None; // scalar must be ≥ 1
    }
    let scalar = scalar_from_bytes(&s)?;
    let mut x = [0u8; 32];
    let mut y = [0u8; 32];
    x.copy_from_slice(&buf[34..66]);
    y.copy_from_slice(&buf[66..98]);
    let element = point_from_xy(&x, &y)?;
    Some(Commit {
        group,
        scalar,
        element,
    })
}

// ── ISC-5: shared secret → KCK / PMK / PMKID ─────────────────────────────────

/// k = F((peer-element + peer-scalar · PWE) · local-rand) = the x-coordinate of
/// the shared point. `None` if local-rand ≥ q or the point is at infinity.
pub fn derive_k(
    pwe: &ProjectivePoint,
    peer_scalar: &Scalar,
    peer_element: &ProjectivePoint,
    local_rand: &[u8; 32],
) -> Option<[u8; 32]> {
    let rand = scalar_from_bytes(local_rand)?;
    let k_point = (*pwe * *peer_scalar + *peer_element) * rand;
    Some(point_xy(&k_point)?.0)
}

/// The IEEE 802.11 KDF over HMAC-SHA256 (§12.7.1.6.2): for each block,
/// `HMAC-SHA256(key, counter(2 LE) ‖ label ‖ context ‖ length-in-bits(2 LE))`,
/// concatenated and truncated to `out_len`.
fn kdf_sha256(key: &[u8], label: &[u8], context: &[u8], out_len: usize) -> Vec<u8> {
    let bits = (out_len * 8) as u16;
    let mut result = Vec::with_capacity(out_len + 32);
    let mut counter: u16 = 1;
    while result.len() < out_len {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(&counter.to_le_bytes());
        mac.update(label);
        mac.update(context);
        mac.update(&bits.to_le_bytes());
        result.extend_from_slice(&mac.finalize().into_bytes());
        counter += 1;
    }
    result.truncate(out_len);
    result
}

/// The SAE-derived keys.
pub struct SaeKeys {
    pub kck: [u8; 32],
    pub pmk: [u8; 32],
    pub pmkid: [u8; 16],
}

/// keyseed = HMAC-SHA256(0³², k); context = (own-scalar + peer-scalar) mod q;
/// KCK‖PMK = KDF(keyseed, "SAE KCK and PMK", context, 64); PMKID = context[..16].
pub fn derive_keys(k: &[u8; 32], own_scalar: &Scalar, peer_scalar: &Scalar) -> SaeKeys {
    let mut ks = HmacSha256::new_from_slice(&[0u8; 32]).expect("HMAC accepts any key length");
    ks.update(k);
    let keyseed = ks.finalize().into_bytes();

    let context = (*own_scalar + *peer_scalar).to_bytes(); // (mod q), 32 BE
    let keys = kdf_sha256(&keyseed, b"SAE KCK and PMK", &context, 64);

    let mut out = SaeKeys {
        kck: [0u8; 32],
        pmk: [0u8; 32],
        pmkid: [0u8; 16],
    };
    out.kck.copy_from_slice(&keys[0..32]);
    out.pmk.copy_from_slice(&keys[32..64]);
    out.pmkid.copy_from_slice(&context[0..16]);
    out
}

// ── ISC-6: confirm ───────────────────────────────────────────────────────────

/// confirm = `CN(KCK, send-confirm, scalar1, element1, scalar2, element2)` =
/// `HMAC-SHA256(KCK, send-confirm(2 LE) ‖ scalar1(32) ‖ x1‖y1 ‖ scalar2(32) ‖
/// x2‖y2)`. The element encoding is the uncompressed affine x‖y (no SEC1 tag).
pub fn confirm(
    kck: &[u8],
    send_confirm: u16,
    scalar1: &Scalar,
    element1: &ProjectivePoint,
    scalar2: &Scalar,
    element2: &ProjectivePoint,
) -> Option<[u8; 32]> {
    let (x1, y1) = point_xy(element1)?;
    let (x2, y2) = point_xy(element2)?;
    let mut mac = HmacSha256::new_from_slice(kck).expect("HMAC accepts any key length");
    mac.update(&send_confirm.to_le_bytes());
    mac.update(&scalar1.to_bytes());
    mac.update(&x1);
    mac.update(&y1);
    mac.update(&scalar2.to_bytes());
    mac.update(&x2);
    mac.update(&y2);
    let tag = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&tag);
    Some(out)
}
