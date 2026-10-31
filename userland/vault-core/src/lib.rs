// SPDX-License-Identifier: MPL-2.0
//! `no_std` cryptographic and byte-layout core for the cap-bounded vault.
//!
//! This crate performs no I/O and owns no ambient authority. Phase B supplies
//! salts/nonces with `libneo::sys_getrandom`, persists records, and carries the
//! protocol over a dedicated capability channel.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use argon2::{Algorithm, Argon2, AssociatedData, ParamsBuilder, Version};
use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce, Tag};
use zeroize::{Zeroize, Zeroizing};

pub mod policy;
pub mod protocol;
pub mod record;
pub mod session;

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultCryptoError {
    BadParameters,
    Encrypt,
    Authentication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub const RFC_9106_KAT: Self = Self {
        memory_kib: 32,
        iterations: 3,
        parallelism: 4,
    };

    /// Interactive baseline; deployment tuning must be benchmarked in Phase B.
    pub const INTERACTIVE_V1: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };
}

/// Master key whose backing bytes are wiped on drop.
pub struct MasterKey(Zeroizing<[u8; KEY_LEN]>);

impl MasterKey {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    fn from_zeroizing(bytes: Zeroizing<[u8; KEY_LEN]>) -> Self {
        Self(bytes)
    }
}

/// Authenticated plaintext whose allocation is wiped on drop.
pub struct SecretBytes(Zeroizing<Vec<u8>>);

impl SecretBytes {
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

/// The tree's one Argon2id configuration (version 0x13, RFC 9106): the
/// vault's master key and OS sign-in's verifiers (OT-66 L1) both derive
/// through it. `secret` and `associated_data` are RFC 9106's optional K and
/// X inputs (empty: none); `out`'s length is the tag length.
pub fn argon2id(
    password: &[u8],
    salt: &[u8],
    secret: &[u8],
    associated_data: &[u8],
    params: KdfParams,
    out: &mut [u8],
) -> Result<(), VaultCryptoError> {
    let mut builder = ParamsBuilder::new();
    builder
        .m_cost(params.memory_kib)
        .t_cost(params.iterations)
        .p_cost(params.parallelism)
        .output_len(out.len());
    if !associated_data.is_empty() {
        builder.data(
            AssociatedData::new(associated_data).map_err(|_| VaultCryptoError::BadParameters)?,
        );
    }
    let params = builder
        .build()
        .map_err(|_| VaultCryptoError::BadParameters)?;
    let argon = if secret.is_empty() {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
    } else {
        Argon2::new_with_secret(secret, Algorithm::Argon2id, Version::V0x13, params)
            .map_err(|_| VaultCryptoError::BadParameters)?
    };
    argon
        .hash_password_into(password, salt, out)
        .map_err(|_| VaultCryptoError::BadParameters)
}

/// Derive a vault master key and wipe the caller-owned password on every exit.
pub fn derive_master_key(
    password: &mut [u8],
    salt: &[u8; SALT_LEN],
    params: KdfParams,
) -> Result<MasterKey, VaultCryptoError> {
    // Argon2 writes directly into the allocation guarded by MasterKey.
    // There is no intermediate `[u8; 32]` copied by value onto the stack.
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    let result = argon2id(password, salt, &[], &[], params, &mut *key)
        .map(|()| MasterKey::from_zeroizing(key));
    password.zeroize();
    result
}

/// Encrypt in place, return `ciphertext || tag`, and wipe plaintext on exit.
pub fn seal(
    key: &MasterKey,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &mut [u8],
) -> Result<Vec<u8>, VaultCryptoError> {
    let result = (|| {
        let cipher =
            ChaCha20Poly1305::new_from_slice(key.bytes()).map_err(|_| VaultCryptoError::Encrypt)?;
        // Keep the copied plaintext under a zeroizing guard until encryption
        // succeeds. On error, no plaintext copy survives this scope.
        let mut out = Zeroizing::new(plaintext.to_vec());
        let tag = cipher
            .encrypt_in_place_detached(Nonce::from_slice(nonce), aad, &mut out)
            .map_err(|_| VaultCryptoError::Encrypt)?;
        out.extend_from_slice(tag.as_slice());
        Ok(core::mem::take(&mut *out))
    })();
    plaintext.zeroize();
    result
}

/// Authenticate before exposing plaintext; failed authentication returns none.
pub fn open(
    key: &MasterKey,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext_and_tag: &[u8],
) -> Result<SecretBytes, VaultCryptoError> {
    if ciphertext_and_tag.len() < TAG_LEN {
        return Err(VaultCryptoError::Authentication);
    }
    let split = ciphertext_and_tag.len() - TAG_LEN;
    let mut plaintext = Zeroizing::new(ciphertext_and_tag[..split].to_vec());
    let tag = Tag::from_slice(&ciphertext_and_tag[split..]);
    let cipher = ChaCha20Poly1305::new_from_slice(key.bytes())
        .map_err(|_| VaultCryptoError::Authentication)?;
    cipher
        .decrypt_in_place_detached(Nonce::from_slice(nonce), aad, &mut plaintext, tag)
        .map_err(|_| VaultCryptoError::Authentication)?;
    Ok(SecretBytes(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Literal RFC 8439 section 2.8.2 bytes, acquired from the RFC Editor raw
    // text (SHA-256 25bef70fbf7a07ff45c2fe4cb7c6ce954eac687413d8610603268b4e4415324c).
    #[test]
    fn rfc_8439_section_2_8_2_aead_kat_and_zeroize() {
        let key: [u8; 32] = core::array::from_fn(|i| 0x80 + i as u8);
        let nonce = [
            0x07, 0, 0, 0, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
        ];
        let aad = [
            0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
        ];
        let mut plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.".to_vec();
        let expected = hex(
            "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6\
             3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36\
             92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc\
             3ff4def08e4b7a9de576d26586cec64b61161ae10b594f09e26a7e902ecbd0600691",
        );
        let sealed = seal(&MasterKey::from_bytes(key), &nonce, &aad, &mut plaintext).unwrap();
        assert_eq!(sealed, expected);
        assert!(plaintext.iter().all(|byte| *byte == 0));
    }

    // RFC 9106 section 5.3. The RFC vector includes secret and associated-data
    // inputs; this exercises the same Argon2id primitive through its complete API.
    #[test]
    fn rfc_9106_section_5_3_argon2id_kat() {
        // Through the shared entry point (OT-66 L1 derives through it too).
        let mut tag = [0u8; 32];
        argon2id(
            &[0x01; 32],
            &[0x02; 16],
            &[0x03; 8],
            &[0x04; 12],
            KdfParams::RFC_9106_KAT,
            &mut tag,
        )
        .unwrap();
        assert_eq!(
            tag,
            hex_array("0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659")
        );
    }

    // Fixed production-path vector computed independently with the official
    // P-H-C Argon2 reference CLI at commit f57e61e19229e23c4445b85494dbf7c07de721cb:
    // printf '%s' "$password" | ./argon2 "$salt" -id -t 3 -m 16 -p 1 -l 32 -r
    #[test]
    fn production_derive_master_key_fixed_to_fixed_kat() {
        let mut password = b"NeoPlutarch vault production KAT v1".to_vec();
        let salt = *b"NeoVaultKATsalt!";
        let key = derive_master_key(&mut password, &salt, KdfParams::INTERACTIVE_V1).unwrap();
        let expected =
            hex_array("572f73475f2deb7cf26e6c4c6fb1cfaf4f89314bcf731db40415ce7c24f7c4bb");
        assert_eq!(key.bytes(), &expected);
        assert!(password.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn password_is_zeroized_and_auth_failure_exposes_nothing() {
        let mut password = b"correct horse battery staple".to_vec();
        let key =
            derive_master_key(&mut password, &[7; SALT_LEN], KdfParams::RFC_9106_KAT).unwrap();
        assert!(password.iter().all(|byte| *byte == 0));
        let mut secret = b"vault secret".to_vec();
        let mut sealed = seal(&key, &[9; NONCE_LEN], b"record header", &mut secret).unwrap();
        sealed[0] ^= 1;
        assert!(matches!(
            open(&key, &[9; NONCE_LEN], b"record header", &sealed),
            Err(VaultCryptoError::Authentication)
        ));
        assert!(secret.iter().all(|byte| *byte == 0));
    }

    fn hex(input: &str) -> Vec<u8> {
        let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        bytes
            .chunks_exact(2)
            .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
            .collect()
    }

    fn hex_array<const N: usize>(input: &str) -> [u8; N] {
        hex(input).try_into().ok().unwrap()
    }

    fn nibble(value: u8) -> u8 {
        match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            _ => panic!("bad hex"),
        }
    }
}
