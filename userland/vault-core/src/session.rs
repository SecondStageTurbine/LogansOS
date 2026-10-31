// SPDX-License-Identifier: MPL-2.0
//! Phase-B vault session — the stateful, I/O-free core the running vault process
//! drives (ADR-075 §5, §F4/§F5).
//!
//! This module composes the pure crypto primitives ([`crate::derive_master_key`],
//! [`crate::seal`], [`crate::open`]) and the record byte layout ([`crate::record`])
//! into the vault's operation state machine: `unlock` / `store` / `retrieve` /
//! `lock`. It owns **no** ambient authority and performs **no** syscalls — all
//! randomness is injected through the [`Rng`] trait, so the same logic is
//! host-testable with deterministic/failing sources and runs unchanged inside the
//! `#![no_std]` vault process (which injects `SYS_GETRANDOM`).
//!
//! ## The three Phase-A obligations, discharged on the live path
//!
//! 1. **Fresh nonce per seal, fail-closed (§F4).** [`Vault::store`] draws a fresh
//!    96-bit nonce from the injected [`Rng`] for *every* seal. A single failed
//!    draw aborts the store with [`VaultError::Rng`] and leaves the store
//!    unchanged — no seal ever runs with a stale/zero/reused nonce.
//! 2. **Header-as-AAD binding (§F4).** Both `store` and `retrieve` pass the
//!    record's 40-byte header as the AEAD associated data, so any mutation of the
//!    salt, KDF parameters, lengths, or version forces an authentication failure
//!    instead of silently decrypting under attacker-chosen metadata.
//! 3. **KDF parameter floor (§F5).** Before deriving from parameters carried in a
//!    record header, [`Vault::retrieve`] rejects any record whose Argon2 cost is
//!    below [`crate::policy::FLOOR`] — a weak-parameter record is refused, never
//!    honored.
//!
//! ## Key model
//!
//! One master key per unlock (the split-GPG / KeePass model), not per record:
//! `unlock` runs Argon2id **once** over the master password and the vault's
//! stable salt to derive the master key, which the session retains (zeroizing)
//! until `lock`. Records are sealed/opened directly with that key under a fresh
//! per-record nonce, so `retrieve` never re-runs Argon2. The per-record header is
//! self-describing (it carries the salt and parameters) so the store is portable.
//!
//! Wrong-password detection has **no separate verifier** (ADR-075 §8): on a vault
//! that already holds records, `unlock` proves the password by authenticating an
//! existing record with the freshly derived key. A wrong password derives a wrong
//! key, the authentication fails, and `unlock` returns the single coarse
//! [`VaultError::AuthFailed`] class — the same class a corrupt record yields, so
//! no "bad-password vs corrupt-record" oracle is leaked.

use alloc::vec::Vec;
use zeroize::Zeroize;

use crate::record::{self, HEADER_LEN};
use crate::{
    KdfParams, MasterKey, NONCE_LEN, SALT_LEN, VaultCryptoError, derive_master_key, open, policy,
    seal,
};

/// Length of a record selector. `secret_id` is random/non-secret and cannot by
/// itself authorize access (ADR-075 §5.2).
pub const SECRET_ID_LEN: usize = 16;

/// Upper bound on a single stored plaintext, enforced before allocation so a
/// hostile `store` cannot force unbounded memory.
pub const MAX_SECRET_LEN: usize = 4096;

/// Upper bound on the number of records a Phase-B (in-memory) vault holds.
pub const MAX_RECORDS: usize = 256;

/// A vault operation error. These map onto the coarse public statuses of
/// ADR-075 §5.2; `AuthFailed` deliberately collapses wrong-password,
/// wrong-key, corrupt-record, and wrong-AAD so no oracle is exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultError {
    /// No master key is loaded (locked, or unlock never succeeded).
    Locked,
    /// Authentication failed — wrong password, wrong key, tampered header, or
    /// corrupt record. One class, no oracle.
    AuthFailed,
    /// Malformed request: bad lengths, unknown record, oversized input, or a
    /// structurally invalid record header.
    BadRequest,
    /// No record with the requested `secret_id`.
    NotFound,
    /// The caller-provided output buffer/grant is smaller than the plaintext.
    OutputTooSmall,
    /// The store is full (`MAX_RECORDS` reached).
    Full,
    /// Randomness draw failed. The vault fails closed — no seal runs, and no
    /// vault is created — rather than falling back to a weaker source.
    Rng,
    /// A record's KDF parameters are below the [`policy::FLOOR`]; refused, not
    /// honored (ADR-075 §F5).
    WeakParams,
}

/// Injected cryptographic randomness. The running vault wraps `SYS_GETRANDOM`
/// with **no** RDRAND fallback (ADR-075 §3: `SYS_GETRANDOM` failure is fatal to
/// the vault); host tests inject deterministic or deliberately-failing sources.
pub trait Rng {
    /// Fill `buf` **completely** with cryptographically secure random bytes.
    /// Returns `true` on success and `false` on any partial or failed draw — a
    /// short read must be reported, not silently accepted, so the caller fails
    /// closed.
    fn fill(&mut self, buf: &mut [u8]) -> bool;
}

/// One encrypted record in the in-memory store: the full
/// `header || salt || nonce || ciphertext || tag` byte string.
struct StoredRecord {
    id: [u8; SECRET_ID_LEN],
    bytes: Vec<u8>,
}

/// The vault session state machine.
///
/// Holds the encrypted store (survives `lock`), the stable KDF salt, the working
/// KDF parameters, and — only while unlocked — the master key.
pub struct Vault {
    /// Vault KDF salt, stable across unlocks. `None` until the first unlock
    /// creates the vault and draws it.
    salt: Option<[u8; SALT_LEN]>,
    /// Working Argon2 parameters. Always at or above [`policy::FLOOR`].
    params: KdfParams,
    /// The master key — `Some` iff unlocked. Dropping it zeroizes the bytes.
    master: Option<MasterKey>,
    /// The ciphertext store. Not secret material, but never leaves the vault.
    store: Vec<StoredRecord>,
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

impl Vault {
    /// A fresh, locked, empty vault using the production [`policy::FLOOR`]
    /// parameters. The salt is drawn on first unlock.
    pub fn new() -> Self {
        Self {
            salt: None,
            params: policy::FLOOR,
            master: None,
            store: Vec::new(),
        }
    }

    /// `true` iff a master key is currently loaded.
    pub fn is_unlocked(&self) -> bool {
        self.master.is_some()
    }

    /// Number of records held.
    pub fn record_count(&self) -> usize {
        self.store.len()
    }

    /// Unlock the vault with `password`, deriving the master key.
    ///
    /// `password` is zeroized before return on **every** path (success, wrong
    /// password, or error) by [`derive_master_key`]. On a vault that already
    /// holds records, the password is proven by authenticating record 0 with the
    /// derived key; a failure returns [`VaultError::AuthFailed`] and leaves the
    /// vault locked.
    pub fn unlock(&mut self, password: &mut [u8], rng: &mut dyn Rng) -> Result<(), VaultError> {
        // The working parameters must themselves meet the floor (defense in
        // depth — they always do for a `new()` vault).
        if !policy::meets_floor(self.params) {
            password.zeroize();
            return Err(VaultError::WeakParams);
        }

        // First unlock creates the vault: draw the stable salt. Fail closed.
        if self.salt.is_none() {
            let mut salt = [0u8; SALT_LEN];
            if !rng.fill(&mut salt) {
                password.zeroize();
                return Err(VaultError::Rng);
            }
            self.salt = Some(salt);
        }
        let salt = self.salt.expect("salt set above");

        // Argon2id once. `derive_master_key` zeroizes `password` on all paths.
        let key =
            derive_master_key(password, &salt, self.params).map_err(|_| VaultError::AuthFailed)?;

        // No separate verifier: prove the password against an existing record.
        if let Some(first) = self.store.first() {
            match open_record(&key, &first.bytes) {
                Ok(_) => {}
                Err(_) => return Err(VaultError::AuthFailed), // key dropped → zeroized
            }
        }

        self.master = Some(key);
        Ok(())
    }

    /// Wipe the master key. The ciphertext store is retained.
    pub fn lock(&mut self) {
        // `MasterKey`'s Drop zeroizes the backing bytes.
        self.master = None;
    }

    /// Seal `plaintext` under `secret_id`, drawing a fresh nonce from `rng`.
    ///
    /// `plaintext` is zeroized before return on every path (by [`seal`] on the
    /// sealed path; explicitly on the early-error paths). If a record with the
    /// same `secret_id` exists it is replaced (upsert). Fails closed on a
    /// randomness error with the store unchanged.
    pub fn store(
        &mut self,
        secret_id: &[u8; SECRET_ID_LEN],
        plaintext: &mut [u8],
        rng: &mut dyn Rng,
    ) -> Result<(), VaultError> {
        let master = match self.master.as_ref() {
            Some(m) => m,
            None => {
                plaintext.zeroize();
                return Err(VaultError::Locked);
            }
        };
        if plaintext.len() > MAX_SECRET_LEN {
            plaintext.zeroize();
            return Err(VaultError::BadRequest);
        }
        let replacing = self.store.iter().any(|r| &r.id == secret_id);
        if !replacing && self.store.len() >= MAX_RECORDS {
            plaintext.zeroize();
            return Err(VaultError::Full);
        }
        let salt = match self.salt {
            Some(s) => s,
            None => {
                // Unreachable while unlocked (unlock sets the salt), but never
                // seal without a bound salt.
                plaintext.zeroize();
                return Err(VaultError::Locked);
            }
        };

        // Fresh per-record nonce. One draw per seal; fail closed.
        let mut nonce = [0u8; NONCE_LEN];
        if !rng.fill(&mut nonce) {
            plaintext.zeroize();
            return Err(VaultError::Rng);
        }

        let header = record::Header {
            memory_kib: self.params.memory_kib,
            iterations: self.params.iterations,
            parallelism: self.params.parallelism,
            ciphertext_len: plaintext.len() as u32,
        };
        // The 40-byte header IS the AEAD associated data (§F4).
        let aad = header.encode();
        let ciphertext_and_tag = match seal(master, &nonce, &aad, plaintext) {
            Ok(ct) => ct,
            Err(_) => return Err(VaultError::AuthFailed), // plaintext already zeroized by seal
        };
        let bytes = match record::assemble(header, &salt, &nonce, &ciphertext_and_tag) {
            Ok(b) => b,
            Err(_) => return Err(VaultError::BadRequest),
        };

        if let Some(existing) = self.store.iter_mut().find(|r| &r.id == secret_id) {
            existing.bytes = bytes;
        } else {
            self.store.push(StoredRecord {
                id: *secret_id,
                bytes,
            });
        }
        Ok(())
    }

    /// Open the record named by `secret_id` and copy its plaintext into `out`.
    ///
    /// Returns the number of bytes written. Enforces the KDF floor on the
    /// record's own parameters before trusting it (§F5), and authenticates the
    /// full header as AAD (§F4) — a tampered header yields [`VaultError::AuthFailed`].
    /// Never returns the master key, another record, or vault memory.
    pub fn retrieve(
        &self,
        secret_id: &[u8; SECRET_ID_LEN],
        out: &mut [u8],
    ) -> Result<usize, VaultError> {
        let master = self.master.as_ref().ok_or(VaultError::Locked)?;
        let record = self
            .store
            .iter()
            .find(|r| &r.id == secret_id)
            .ok_or(VaultError::NotFound)?;

        // §F5: reject weak parameters before deriving/trusting anything.
        let view = record::parse(&record.bytes).map_err(|_| VaultError::BadRequest)?;
        let record_params = KdfParams {
            memory_kib: view.header.memory_kib,
            iterations: view.header.iterations,
            parallelism: view.header.parallelism,
        };
        if !policy::meets_floor(record_params) {
            return Err(VaultError::WeakParams);
        }

        // §F4: authenticate under the exact stored header bytes as AAD, then
        // expose the plaintext only after the tag verifies.
        let secret = open_record(master, &record.bytes).map_err(|_| VaultError::AuthFailed)?;
        let plaintext = secret.expose();
        if plaintext.len() > out.len() {
            return Err(VaultError::OutputTooSmall);
        }
        out[..plaintext.len()].copy_from_slice(plaintext);
        Ok(plaintext.len())
        // `secret` (Zeroizing) drops here, wiping the vault's plaintext copy.
    }

    // ── Scenario/test hooks (not part of the operation protocol) ─────────────

    /// Flip one byte of a stored record's header, in place, to exercise the
    /// header-as-AAD binding on the **live** open path. Test/scenario use only;
    /// a real caller never reaches a record's stored bytes.
    #[doc(hidden)]
    pub fn corrupt_record_header_for_test(
        &mut self,
        secret_id: &[u8; SECRET_ID_LEN],
        header_offset: usize,
    ) -> Result<(), VaultError> {
        let record = self
            .store
            .iter_mut()
            .find(|r| &r.id == secret_id)
            .ok_or(VaultError::NotFound)?;
        if header_offset >= HEADER_LEN || header_offset >= record.bytes.len() {
            return Err(VaultError::BadRequest);
        }
        record.bytes[header_offset] ^= 0x01;
        Ok(())
    }

    /// Install a raw record byte string directly, bypassing `store` — used by
    /// the §F5 scenario to inject a record claiming sub-floor parameters.
    /// Test/scenario use only.
    #[doc(hidden)]
    pub fn inject_raw_record_for_test(
        &mut self,
        secret_id: &[u8; SECRET_ID_LEN],
        bytes: Vec<u8>,
    ) -> Result<(), VaultError> {
        if self.store.len() >= MAX_RECORDS && !self.store.iter().any(|r| &r.id == secret_id) {
            return Err(VaultError::Full);
        }
        if let Some(existing) = self.store.iter_mut().find(|r| &r.id == secret_id) {
            existing.bytes = bytes;
        } else {
            self.store.push(StoredRecord {
                id: *secret_id,
                bytes,
            });
        }
        Ok(())
    }
}

/// Open a full `header || salt || nonce || ciphertext || tag` record with `key`,
/// using the exact stored 40-byte header as the AEAD associated data.
///
/// Using the literal header slice (not a re-encoding) means **any** flipped
/// header byte changes the AAD — a mutation in a validated field is caught by
/// the parser, and a mutation in a parameter field is caught by the tag.
fn open_record(key: &MasterKey, bytes: &[u8]) -> Result<crate::SecretBytes, VaultCryptoError> {
    let view = record::parse(bytes).map_err(|_| VaultCryptoError::Authentication)?;
    let aad = &bytes[..HEADER_LEN];
    open(key, view.nonce, aad, view.ciphertext_and_tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Deterministic counting RNG: each fill writes a strictly increasing byte
    /// pattern so successive draws are distinct and inspectable.
    struct CountingRng {
        counter: u64,
        draws: usize,
    }
    impl CountingRng {
        fn new() -> Self {
            Self {
                counter: 1,
                draws: 0,
            }
        }
    }
    impl Rng for CountingRng {
        fn fill(&mut self, buf: &mut [u8]) -> bool {
            self.draws += 1;
            for b in buf.iter_mut() {
                *b = (self.counter & 0xff) as u8;
                self.counter += 1;
            }
            true
        }
    }

    /// RNG that fails on the Nth draw (1-indexed), succeeds otherwise.
    struct FailOnDraw {
        fail_at: usize,
        draws: usize,
    }
    impl Rng for FailOnDraw {
        fn fill(&mut self, buf: &mut [u8]) -> bool {
            self.draws += 1;
            if self.draws == self.fail_at {
                return false;
            }
            for (i, b) in buf.iter_mut().enumerate() {
                *b = (self.draws as u8).wrapping_add(i as u8).wrapping_add(1);
            }
            true
        }
    }

    fn id(n: u8) -> [u8; SECRET_ID_LEN] {
        [n; SECRET_ID_LEN]
    }

    #[test]
    fn store_then_retrieve_roundtrips() {
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"correct horse battery staple".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        assert!(pw.iter().all(|b| *b == 0), "password wiped after unlock");

        let mut secret = b"s3cr3t-value".to_vec();
        let expected = secret.clone();
        v.store(&id(1), &mut secret, &mut rng).unwrap();
        assert!(
            secret.iter().all(|b| *b == 0),
            "plaintext wiped after store"
        );

        let mut out = [0u8; 64];
        let n = v.retrieve(&id(1), &mut out).unwrap();
        assert_eq!(&out[..n], expected.as_slice());
    }

    #[test]
    fn obligation_1_fresh_nonce_per_seal_no_reuse() {
        // Each store draws exactly one fresh nonce; N stores yield N distinct
        // nonces (proving no reuse with the same derived key).
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"pw".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        let draws_after_unlock = rng.draws; // 1 salt draw

        let mut nonces = vec![];
        for n in 0..8u8 {
            let mut secret = vec![n; 8];
            v.store(&id(n), &mut secret, &mut rng).unwrap();
            let view = record::parse(&v.store[n as usize].bytes).unwrap();
            nonces.push(*view.nonce);
        }
        // Exactly one nonce draw per store.
        assert_eq!(rng.draws - draws_after_unlock, 8);
        // All distinct.
        for i in 0..nonces.len() {
            for j in (i + 1)..nonces.len() {
                assert_ne!(nonces[i], nonces[j], "nonce reuse between records {i},{j}");
            }
        }
    }

    #[test]
    fn obligation_1_rng_failure_fails_closed() {
        // A nonce-draw failure aborts the store and leaves the store unchanged;
        // no seal ran under a stale/zero nonce.
        let mut good = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"pw".to_vec();
        v.unlock(&mut pw, &mut good).unwrap();

        let mut failing = FailOnDraw {
            fail_at: 1,
            draws: 0,
        };
        let mut secret = b"never-sealed".to_vec();
        assert_eq!(
            v.store(&id(1), &mut secret, &mut failing),
            Err(VaultError::Rng)
        );
        assert_eq!(v.record_count(), 0, "no record added on RNG failure");
        assert!(
            secret.iter().all(|b| *b == 0),
            "plaintext wiped on fail-closed"
        );
    }

    #[test]
    fn obligation_1_unlock_salt_draw_fails_closed() {
        // First unlock draws the vault salt; a failure fails closed (no vault
        // created, still locked).
        let mut failing = FailOnDraw {
            fail_at: 1,
            draws: 0,
        };
        let mut v = Vault::new();
        let mut pw = b"pw".to_vec();
        assert_eq!(v.unlock(&mut pw, &mut failing), Err(VaultError::Rng));
        assert!(!v.is_unlocked());
        assert!(
            pw.iter().all(|b| *b == 0),
            "password wiped even on rng-fail"
        );
    }

    #[test]
    fn obligation_2_every_mutated_header_byte_is_rejected() {
        // Every header byte, when flipped, must be REJECTED — never a silent
        // decrypt. The rejection may come from the structural parser
        // (BadRequest, for magic/version/reserved/length fields), the KDF floor
        // guard (WeakParams, for a parameter field driven below the floor), or
        // the AEAD tag (AuthFailed, via the header-as-AAD binding). All three
        // are valid "mutation caught" outcomes.
        for off in 0..HEADER_LEN {
            let mut vv = fresh_with_one_record(&id(1), b"aad-bound");
            vv.corrupt_record_header_for_test(&id(1), off).unwrap();
            let mut out = [0u8; 64];
            let r = vv.retrieve(&id(1), &mut out);
            assert!(
                matches!(
                    r,
                    Err(VaultError::AuthFailed)
                        | Err(VaultError::BadRequest)
                        | Err(VaultError::WeakParams)
                ),
                "header byte {off} flip was not rejected: {r:?}"
            );
            assert!(out.iter().all(|b| *b == 0), "no plaintext leaked on tamper");
        }
    }

    #[test]
    fn obligation_2_aad_binding_specifically_catches_param_mutation() {
        // Prove the AEAD *header-as-AAD* binding specifically: mutate a
        // parameter byte that keeps the parameters ABOVE the floor (so the
        // floor guard passes and the parser accepts it) — the record must then
        // fail with AuthFailed, i.e. the tag caught the header mutation.
        let mut vv = fresh_with_one_record(&id(1), b"aad-bound");
        // memory_kib is at header offset 16..20, value 65536 = 0x00010000 (LE).
        // Flipping bit 0 of offset 16 -> 65537, still >= the 65536 floor.
        vv.corrupt_record_header_for_test(&id(1), 16).unwrap();
        let mut out = [0u8; 64];
        assert_eq!(vv.retrieve(&id(1), &mut out), Err(VaultError::AuthFailed));
        assert!(out.iter().all(|b| *b == 0));
    }

    #[test]
    fn obligation_3_sub_floor_record_is_refused() {
        // A record whose header claims weak (sub-floor) parameters is rejected
        // before it can be trusted, with WeakParams — not honored.
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"pw".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        v.store(&id(1), &mut b"strong".to_vec(), &mut rng).unwrap();

        // Hand-build a record claiming 1 KiB / 1 pass / 1 lane (far below floor).
        let weak_header = record::Header {
            memory_kib: 1,
            iterations: 1,
            parallelism: 1,
            ciphertext_len: 3,
        };
        let body = vec![0xAB, 0xCD, 0xEF]; // 3 ct + 16 tag
        let mut body_full = body.clone();
        body_full.extend_from_slice(&[0u8; crate::TAG_LEN]);
        let raw = record::assemble(
            weak_header,
            &[0x11; SALT_LEN],
            &[0x22; NONCE_LEN],
            &body_full,
        )
        .unwrap();
        v.inject_raw_record_for_test(&id(2), raw).unwrap();

        let mut out = [0u8; 64];
        assert_eq!(v.retrieve(&id(2), &mut out), Err(VaultError::WeakParams));
    }

    #[test]
    fn wrong_password_fails_with_one_class_after_relock() {
        // A vault with a record: right password unlocks and opens; wrong
        // password derives a wrong key, fails to authenticate the record, and
        // returns the single AuthFailed class (no oracle).
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"the-right-password".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        v.store(&id(1), &mut b"treasure".to_vec(), &mut rng)
            .unwrap();
        v.lock();
        assert!(!v.is_unlocked());

        let mut wrong = b"the-WRONG-password".to_vec();
        assert_eq!(v.unlock(&mut wrong, &mut rng), Err(VaultError::AuthFailed));
        assert!(!v.is_unlocked(), "still locked after wrong password");

        // And a retrieve while locked leaks nothing.
        let mut out = [0u8; 64];
        assert_eq!(v.retrieve(&id(1), &mut out), Err(VaultError::Locked));

        // The right password still works after the failed attempt.
        let mut right = b"the-right-password".to_vec();
        v.unlock(&mut right, &mut rng).unwrap();
        let n = v.retrieve(&id(1), &mut out).unwrap();
        assert_eq!(&out[..n], b"treasure");
    }

    #[test]
    fn retrieve_output_too_small_reports_and_leaks_nothing() {
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"pw".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        v.store(&id(1), &mut b"0123456789".to_vec(), &mut rng)
            .unwrap();
        let mut tiny = [0u8; 4];
        assert_eq!(
            v.retrieve(&id(1), &mut tiny),
            Err(VaultError::OutputTooSmall)
        );
        assert!(tiny.iter().all(|b| *b == 0), "no partial plaintext written");
    }

    #[test]
    fn locked_store_and_unknown_id_are_rejected() {
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        // store while locked
        let mut s = b"x".to_vec();
        assert_eq!(v.store(&id(1), &mut s, &mut rng), Err(VaultError::Locked));
        assert!(s.iter().all(|b| *b == 0));
        // unknown id after unlock
        let mut pw = b"pw".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        let mut out = [0u8; 8];
        assert_eq!(v.retrieve(&id(9), &mut out), Err(VaultError::NotFound));
    }

    // Build a fresh vault holding exactly one record with a deterministic RNG,
    // so per-test corruption starts from an identical, uncorrupted store.
    fn fresh_with_one_record(secret_id: &[u8; SECRET_ID_LEN], plaintext: &[u8]) -> Vault {
        let mut rng = CountingRng::new();
        let mut v = Vault::new();
        let mut pw = b"pw".to_vec();
        v.unlock(&mut pw, &mut rng).unwrap();
        let mut p = plaintext.to_vec();
        v.store(secret_id, &mut p, &mut rng).unwrap();
        v
    }
}
