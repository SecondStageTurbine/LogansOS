// SPDX-License-Identifier: MPL-2.0
//! Arthur M2 — pure ChaCha20-Poly1305 crypto core (no I/O, no syscalls).
//!
//! This module is the **byte-layout authority** for M2's on-disk format:
//! `version(1B) || nonce(12B) || ciphertext(var) || tag(16B)`. It depends only
//! on `alloc` and `chacha20poly1305` — NOT on `libneo` (no syscalls) and NOT on
//! `MemoryType` — so it can be `#[path]`-mounted into a host test crate and
//! exercised by a *direct-function* byte-oracle (see `tools/arthur-logic-tests`).
//!
//! The stateful key/counter/I-O layer lives in `memory_encryption.rs`, which
//! re-exports this module's primitives via `use crate::memory_crypto::*`.
//!
//! Counter-nonce derivation (single-blob model): each `.mem` file is ONE AEAD
//! blob written under ONE nonce. The per-category counter is the *next nonce to
//! use*; `nonce_for(counter_val)` takes the low 12 bytes of `counter_val` as a
//! little-endian u128. One nonce is consumed per whole-file save (counter += 1),
//! persisted write-ahead, so a crash only ever *skips* nonce values — never
//! reuses one.

use alloc::vec::Vec;

use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce, Tag};

/// M2 encrypted file format version.
pub(crate) const ENCRYPTED_FORMAT_VERSION: u8 = 0x01;

/// AEAD key length in bytes.
pub(crate) const KEY_LEN: usize = 32;

/// Nonce length in bytes (96-bit for ChaCha20-Poly1305).
pub(crate) const NONCE_LEN: usize = 12;

/// Poly1305 tag length in bytes.
pub(crate) const TAG_LEN: usize = 16;

/// Counter length in bytes (128-bit monotonic counter).
pub(crate) const COUNTER_LEN: usize = 16;

/// Total `.key` file size: key || counter.
pub const KEY_FILE_LEN: usize = KEY_LEN + COUNTER_LEN;

/// On-disk header: version(1) + nonce(12).
pub(crate) const HEADER_LEN: usize = 1 + NONCE_LEN;

/// Minimum valid encrypted file: version + nonce + tag (zero-length ciphertext).
pub(crate) const MIN_ENCRYPTED_FILE_LEN: usize = HEADER_LEN + TAG_LEN;

/// Memory encryption errors (fail-closed).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MemoryEncryptError {
    /// `SYS_GETRANDOM` failed — cannot generate key material.
    GetRandom,
    /// `.key` file has wrong length or corrupt content.
    BadKeyFile,
    /// Encrypted file version byte doesn't match.
    BadVersion,
    /// Encrypted file is too short to contain a valid header + tag.
    TruncatedFile,
    /// AEAD encryption failed (should not happen with valid key).
    Encrypt,
    /// AEAD authentication failed — reject file, never surface plaintext.
    AuthFail,
    /// Key burn failed (overwrite or unlink error).
    Burn,
    /// Counter overflow (theoretical: 2^128 writes).
    CounterOverflow,
}

/// Per-category key material: 32-byte AEAD key + 128-bit monotonic counter.
///
/// The `counter` is the *next nonce value to use* (see module docs). After a
/// write that consumes nonce `n`, the persisted counter is `n + 1`.
#[derive(Copy, Clone)]
pub(crate) struct CategoryKey {
    key: [u8; KEY_LEN],
    counter: u128,
}

impl CategoryKey {
    /// Construct from raw key bytes + counter. The sole constructor used by both
    /// the stateful layer (`generate`) and the host byte-oracle (known-key KATs).
    pub(crate) fn from_raw(key: [u8; KEY_LEN], counter: u128) -> Self {
        Self { key, counter }
    }

    /// Read key + counter from a raw `.key` file.
    pub(crate) fn read(bytes: &[u8]) -> Result<Self, MemoryEncryptError> {
        if bytes.len() != KEY_FILE_LEN {
            return Err(MemoryEncryptError::BadKeyFile);
        }
        let mut key = [0u8; KEY_LEN];
        let mut counter_le = [0u8; COUNTER_LEN];
        key.copy_from_slice(&bytes[..KEY_LEN]);
        counter_le.copy_from_slice(&bytes[KEY_LEN..]);
        let counter = u128::from_le_bytes(counter_le);
        Ok(Self { key, counter })
    }

    /// Serialize to raw `.key` file bytes.
    pub(crate) fn as_bytes(&self) -> [u8; KEY_FILE_LEN] {
        let mut out = [0u8; KEY_FILE_LEN];
        out[..KEY_LEN].copy_from_slice(&self.key);
        out[KEY_LEN..].copy_from_slice(&self.counter.to_le_bytes());
        out
    }

    /// The current counter value (the next nonce to use).
    pub(crate) fn counter(&self) -> u128 {
        self.counter
    }

    /// Return a copy with the counter set to `counter`.
    pub(crate) fn with_counter(&self, counter: u128) -> Self {
        Self {
            key: self.key,
            counter,
        }
    }

    /// Derive the nonce for a given counter value (low 12 bytes, LE).
    pub(crate) fn nonce_for(&self, counter_val: u128) -> [u8; NONCE_LEN] {
        let le = counter_val.to_le_bytes();
        le[..NONCE_LEN].try_into().unwrap()
    }

    /// Create a new AEAD cipher instance from this key.
    fn cipher(&self) -> Result<ChaCha20Poly1305, MemoryEncryptError> {
        ChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| MemoryEncryptError::BadKeyFile)
    }
}

/// Encrypt plaintext into M2's on-disk format.
///
/// Output: `version(1B) || nonce(12B) || ciphertext(var) || tag(16B)`
pub(crate) fn encrypt_plaintext(
    ck: &CategoryKey,
    nonce_val: u128,
    plaintext: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), MemoryEncryptError> {
    out.clear();
    out.push(ENCRYPTED_FORMAT_VERSION);

    let nonce_bytes = ck.nonce_for(nonce_val);
    out.extend_from_slice(&nonce_bytes);

    // Ciphertext is same length as plaintext (stream cipher property).
    let ct_len = plaintext.len();
    out.resize(out.len() + ct_len + TAG_LEN, 0);

    // The ciphertext + tag region. encrypt_in_place_detached writes ciphertext
    // in-place and returns the tag separately.
    let nonce_region_end = 1 + NONCE_LEN;
    let ct_region = &mut out[nonce_region_end..nonce_region_end + ct_len];
    ct_region.copy_from_slice(plaintext);

    let cipher = ck.cipher()?;
    let tag = cipher
        .encrypt_in_place_detached(Nonce::from_slice(&nonce_bytes), b"", ct_region)
        .map_err(|_| MemoryEncryptError::Encrypt)?;

    out[nonce_region_end + ct_len..].copy_from_slice(tag.as_slice());
    Ok(())
}

/// Decrypt M2's on-disk format back to plaintext.
///
/// **Fail-closed:** auth-tag mismatch rejects the entire file. No unverified
/// bytes are ever surfaced. The `out` buffer is only written on success.
pub(crate) fn decrypt_file(
    bytes: &[u8],
    ck: &CategoryKey,
    out: &mut Vec<u8>,
) -> Result<(), MemoryEncryptError> {
    if bytes.len() < MIN_ENCRYPTED_FILE_LEN {
        return Err(MemoryEncryptError::TruncatedFile);
    }

    if bytes[0] != ENCRYPTED_FORMAT_VERSION {
        return Err(MemoryEncryptError::BadVersion);
    }

    let nonce_bytes: [u8; NONCE_LEN] = bytes[1..1 + NONCE_LEN]
        .try_into()
        .map_err(|_| MemoryEncryptError::TruncatedFile)?;

    let ct_len = bytes.len() - HEADER_LEN - TAG_LEN;
    let tag_bytes: [u8; TAG_LEN] = bytes[bytes.len() - TAG_LEN..]
        .try_into()
        .map_err(|_| MemoryEncryptError::TruncatedFile)?;

    // Allocate output for plaintext (same length as ciphertext).
    out.clear();
    out.resize(ct_len, 0);
    out.copy_from_slice(&bytes[HEADER_LEN..HEADER_LEN + ct_len]);

    let cipher = ck.cipher()?;
    let tag = Tag::from_slice(&tag_bytes);

    // decrypt_in_place_detached returns Err on auth failure. On error, `out`
    // contains garbage that must never be surfaced — the caller must not read it.
    // We clear it on failure so a misuse can't leak unverified bytes.
    if cipher
        .decrypt_in_place_detached(Nonce::from_slice(&nonce_bytes), b"", out, tag)
        .is_err()
    {
        out.clear();
        return Err(MemoryEncryptError::AuthFail);
    }

    Ok(())
}

/// Extract the nonce value from an encrypted file (stored in cleartext at bytes [1..13]).
pub(crate) fn extract_nonce_from_file(bytes: &[u8]) -> Option<u128> {
    if bytes.len() < 1 + NONCE_LEN {
        return None;
    }
    if bytes[0] != ENCRYPTED_FORMAT_VERSION {
        return None;
    }
    let mut le = [0u8; COUNTER_LEN];
    le[..NONCE_LEN].copy_from_slice(&bytes[1..1 + NONCE_LEN]);
    Some(u128::from_le_bytes(le))
}
