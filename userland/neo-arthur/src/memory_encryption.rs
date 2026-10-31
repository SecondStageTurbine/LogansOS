// SPDX-License-Identifier: MPL-2.0
//! Arthur M2 — stateful key/counter/crypto-erase layer over `memory_crypto`.
//!
//! The pure byte-layout + AEAD primitives live in [`crate::memory_crypto`]; this
//! module owns the per-category key material, the monotonic nonce counter, and
//! the crypto-erase path. It is the only M2 module that touches `libneo`
//! (`sys_getrandom`) and `MemoryType`.
//!
//! **Single-blob model.** Each `.mem` file is ONE AEAD blob written under ONE
//! nonce; the whole plaintext is re-encrypted and rewritten on every save (see
//! `append_record` in `main.rs`). The counter is the *next nonce to use*.
//!
//! **Write-ahead durability (the caller's contract).** Per save, `main.rs` must,
//! in order: (1) `reserve_nonce` — advance the counter in RAM and get this write's
//! nonce; (2) persist + `fsync` the `.key` file (the advanced counter) DURABLE
//! FIRST; (3) `encrypt_with_nonce` with that nonce; (4) write + `fsync` the `.mem`
//! blob. A crash between (2) and (4) leaves the counter advanced on disk, so the
//! next boot only ever *skips* a nonce value — it can never *reuse* one. Nonce
//! reuse under one key is catastrophic for ChaCha20-Poly1305; this ordering
//! forbids it.
//!
//! **Rollback (named per PM-review).** A full-consistent `.mem`+`.key` snapshot
//! rollback rewinds both in lockstep — out of the N=1 threat model, closed by
//! OT-17's TPM-monotonic-counter hardening. The asymmetric `.key`-only rollback
//! IS closed here by [`apply_rollback_safety`](MemoryEncryption::apply_rollback_safety).

use alloc::vec::Vec;

use libneo::sys_getrandom;

use crate::memory_crypto::{
    CategoryKey, KEY_FILE_LEN, KEY_LEN, MemoryEncryptError, decrypt_file, encrypt_plaintext,
    extract_nonce_from_file,
};
use crate::memory_logic::MemoryType;

impl CategoryKey {
    /// Generate a fresh key from `SYS_GETRANDOM`. Counter starts at 0 (the first
    /// nonce a fresh category will use).
    pub(crate) fn generate() -> Result<Self, MemoryEncryptError> {
        let mut key = [0u8; KEY_LEN];
        if sys_getrandom(&mut key) < 0 {
            return Err(MemoryEncryptError::GetRandom);
        }
        Ok(Self::from_raw(key, 0))
    }
}

/// M2 encryption runtime state. Holds per-category key material.
pub struct MemoryEncryption {
    /// Validated flag — set to false on any crypto failure (fail-closed degrade).
    valid: bool,
    /// Per-category key material. Index by `MemoryType` discriminant.
    keys: [Option<CategoryKey>; 4],
}

impl MemoryEncryption {
    pub fn new() -> Self {
        Self {
            valid: true,
            keys: Default::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Mark encryption as unavailable (degrade to ephemeral).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Serialized `.key` bytes for a category, for write-ahead persistence.
    /// Returns the raw `key || counter` image (never exposes the key type).
    pub fn key_bytes(&self, kind: MemoryType) -> Option<[u8; KEY_FILE_LEN]> {
        let idx = Self::type_index(kind);
        self.keys[idx].map(|ck| ck.as_bytes())
    }

    fn type_index(kind: MemoryType) -> usize {
        match kind {
            MemoryType::Fact => 0,
            MemoryType::Preference => 1,
            MemoryType::Event => 2,
            MemoryType::Relationship => 3,
        }
    }

    /// Ensure the key for a category is loaded. Generates a fresh one if the
    /// `.key` file is absent.
    pub fn ensure_key(
        &mut self,
        kind: MemoryType,
        key_file_bytes: Option<&[u8]>,
    ) -> Result<(), MemoryEncryptError> {
        if !self.valid {
            return Err(MemoryEncryptError::BadKeyFile);
        }
        let idx = Self::type_index(kind);

        if self.keys[idx].is_none() {
            let ck = match key_file_bytes {
                Some(bytes) => CategoryKey::read(bytes)?,
                None => CategoryKey::generate()?,
            };
            self.keys[idx] = Some(ck);
        }

        Ok(())
    }

    /// Advance the counter past any nonce observed in the existing `.mem` file.
    /// This closes the asymmetric `.key`-only rollback: if a restored old `.key`
    /// carries a counter at or below the nonce already committed to `.mem`, the
    /// next write would reuse a nonce — so we jump the counter past it.
    ///
    /// In the normal (non-attack) case the counter is already `max_nonce + 1`,
    /// so this is a no-op.
    pub fn apply_rollback_safety(&mut self, kind: MemoryType, existing_mem_bytes: &[u8]) {
        let idx = Self::type_index(kind);
        let Some(ck) = self.keys[idx] else {
            return;
        };

        let max_nonce = extract_nonce_from_file(existing_mem_bytes).unwrap_or(0);
        if ck.counter() <= max_nonce {
            self.keys[idx] = Some(ck.with_counter(max_nonce.saturating_add(1)));
        }
    }

    /// Reserve the nonce for the next whole-file write and advance the counter
    /// (in RAM). The caller MUST persist + `fsync` the `.key` file before writing
    /// the `.mem` blob (see the module-level write-ahead contract).
    pub fn reserve_nonce(&mut self, kind: MemoryType) -> Result<u128, MemoryEncryptError> {
        if !self.valid {
            return Err(MemoryEncryptError::BadKeyFile);
        }
        let idx = Self::type_index(kind);
        let ck = self.keys[idx].ok_or(MemoryEncryptError::BadKeyFile)?;

        let nonce_val = ck.counter();
        let next = nonce_val
            .checked_add(1)
            .ok_or(MemoryEncryptError::CounterOverflow)?;
        self.keys[idx] = Some(ck.with_counter(next));
        Ok(nonce_val)
    }

    /// Encrypt the whole-file plaintext under a nonce previously obtained from
    /// [`reserve_nonce`](Self::reserve_nonce). Does NOT advance the counter — the
    /// reservation already did, write-ahead.
    pub fn encrypt_with_nonce(
        &mut self,
        kind: MemoryType,
        nonce_val: u128,
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), MemoryEncryptError> {
        if !self.valid {
            return Err(MemoryEncryptError::BadKeyFile);
        }
        let idx = Self::type_index(kind);
        let ck = self.keys[idx].ok_or(MemoryEncryptError::BadKeyFile)?;
        encrypt_plaintext(&ck, nonce_val, plaintext, out)
    }

    /// Decrypt an encrypted `.mem` file back to M1 plaintext records.
    ///
    /// **Fail-closed:** on auth failure, returns `AuthFail` and `out` is cleared.
    pub fn decrypt_for_read(
        &mut self,
        kind: MemoryType,
        encrypted_bytes: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), MemoryEncryptError> {
        if !self.valid {
            return Err(MemoryEncryptError::BadKeyFile);
        }
        let idx = Self::type_index(kind);
        let ck = self.keys[idx].ok_or(MemoryEncryptError::BadKeyFile)?;
        decrypt_file(encrypted_bytes, &ck, out)
    }

    /// Crypto-erase a category by burning its key.
    ///
    /// Returns CSPRNG bytes for the caller to overwrite the `.key` file with
    /// (NIST 800-88), then unlink it. After burn, all ciphertext under this key
    /// is irrecoverable.
    ///
    /// ISC-3 (B3 fix): the in-RAM key is wiped by DROPPING it (`None`). The prior
    /// code copied the *real* key into a buffer misleadingly named `zeroed_key`
    /// (via an unflagged `unsafe` `ptr::copy`), preserving the key in RAM. `None`
    /// is the correct, safe wipe — no `unsafe`. The next `remember` re-keys the
    /// category cleanly (`ensure_key` → `generate`).
    pub fn burn_key(&mut self, kind: MemoryType) -> Result<[u8; KEY_FILE_LEN], MemoryEncryptError> {
        let idx = Self::type_index(kind);

        let mut burn_bytes = [0u8; KEY_FILE_LEN];
        if sys_getrandom(&mut burn_bytes) < 0 {
            return Err(MemoryEncryptError::Burn);
        }

        self.keys[idx] = None;

        Ok(burn_bytes)
    }
}
