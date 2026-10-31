// SPDX-License-Identifier: MPL-2.0
//! WPA2/WPA3 supplicant crypto — Phase 7.5 BUILD-W0 (the WiFi arc's first,
//! dongle-independent slice; ADR-051, ANSPEC §10.9).
//!
//! A self-contained key-management + EAPOL + CCMP (+ SAE) library: the crypto
//! core the W2 on-air supplicant will drive. It computes keys and encrypted
//! frames; it touches no hardware, no MLME state machine, and no kernel glue
//! (those are W1/W2). Every primitive is a **vetted RustCrypto crate** (`aes`,
//! `ccm`, `cmac`, `sha1`, `sha2`, `hmac`, `pbkdf2`, `p256`) — AES/SHA/HMAC/ECC
//! are NEVER hand-rolled (Article 15.7; the kTLS-`chacha20poly1305` precedent).
//! This module writes only the WiFi-specific protocol layers on top.
//!
//! **Proof discipline (I-12, the byte-oracle applied to crypto):** every layer
//! is asserted against *published* IEEE 802.11 / RFC known-answer vectors in
//! the host suite (`tools/ai-ops-tests`), never round-trip — a self-consistent
//! encrypt/decrypt that is both wrong still passes, so only published answers
//! prove correctness.
//!
//! **I-14(c):** no new containment vector — the firmware adversary (W1's
//! MT7601U blob) is contained by USB itself (device-initiated DMA is impossible;
//! the xHCI controller mediates all memory movement and already sits in VT-d's
//! fence domain, 7.5-V). This library moves no memory and runs in the host tier.
//!
//! Compiled only under the `wifi` feature (the kill-switch: off ⇒ the crypto
//! deps are not pulled and the kernel is bit-for-bit today).

/// CB-5a ISC-3: BIP-CMAC-128 802.11w management-frame protection (software IGTK).
pub mod bip;
pub mod ccmp;
pub mod eapol;
pub mod kdf;
/// W2 ISC-2: AES Key Unwrap (RFC 3394) for the 4-way M3 GTK.
pub mod keywrap;
pub mod sae;
/// CB-5a ISC-1: the SAE protocol state machine driving `sae` over the air.
pub mod sae_sm;
