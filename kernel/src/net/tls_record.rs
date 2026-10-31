// SPDX-License-Identifier: MPL-2.0
//! TLS 1.3 application-data record protection for the Phase 4 kTLS shim.
//!
//! This module owns record framing and sequence/nonce bookkeeping only. The
//! AEAD primitive is provided by the `chacha20poly1305` crate.

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{Ordering, compiler_fence};

use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce, Tag};

pub const KEY_LEN: usize = 32;
pub const IV_LEN: usize = 12;
pub const KEY_MATERIAL_LEN: usize = (KEY_LEN + IV_LEN) * 2;
pub const TLS_HEADER_LEN: usize = 5;
pub const TLS_TAG_LEN: usize = 16;
pub const TLS_INNER_CONTENT_TYPE_LEN: usize = 1;
pub const TLS_MAX_INNER_PLAINTEXT: usize = 16 * 1024;
pub const TLS_MAX_RECORD_PAYLOAD: usize =
    TLS_MAX_INNER_PLAINTEXT + TLS_INNER_CONTENT_TYPE_LEN + TLS_TAG_LEN;
pub const TLS_CONTENT_TYPE_APPLICATION_DATA: u8 = 23;
pub const TLS_LEGACY_VERSION: [u8; 2] = [0x03, 0x03];

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TlsRecordError {
    InvalidKeyMaterial,
    InvalidRecord,
    RecordTooLarge,
    Auth,
    SequenceOverflow,
}

pub struct TlsSocketState {
    tx: DirectionState,
    rx: DirectionState,
}

struct DirectionState {
    key: [u8; KEY_LEN],
    iv: [u8; IV_LEN],
    sequence: u64,
}

impl TlsSocketState {
    pub fn from_key_material(key_material: &mut [u8]) -> Result<Self, TlsRecordError> {
        if key_material.len() != KEY_MATERIAL_LEN {
            wipe_secret(key_material);
            return Err(TlsRecordError::InvalidKeyMaterial);
        }
        let mut tx_key = [0u8; KEY_LEN];
        let mut tx_iv = [0u8; IV_LEN];
        let mut rx_key = [0u8; KEY_LEN];
        let mut rx_iv = [0u8; IV_LEN];
        let mut cursor = 0;
        tx_key.copy_from_slice(&key_material[cursor..cursor + KEY_LEN]);
        cursor += KEY_LEN;
        tx_iv.copy_from_slice(&key_material[cursor..cursor + IV_LEN]);
        cursor += IV_LEN;
        rx_key.copy_from_slice(&key_material[cursor..cursor + KEY_LEN]);
        cursor += KEY_LEN;
        rx_iv.copy_from_slice(&key_material[cursor..cursor + IV_LEN]);
        wipe_secret(key_material);
        Ok(Self {
            tx: DirectionState::new(tx_key, tx_iv),
            rx: DirectionState::new(rx_key, rx_iv),
        })
    }

    pub fn seal_application_data(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, TlsRecordError> {
        self.tx.seal_application_data(plaintext)
    }

    pub fn open_application_records(
        &mut self,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, TlsRecordError> {
        self.rx.open_application_records(ciphertext)
    }
}

impl DirectionState {
    fn new(key: [u8; KEY_LEN], iv: [u8; IV_LEN]) -> Self {
        Self {
            key,
            iv,
            sequence: 0,
        }
    }

    fn seal_application_data(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, TlsRecordError> {
        let record_len = sealed_len(plaintext.len())?;
        let encrypted_len = record_len - TLS_HEADER_LEN;
        let mut header = [0u8; TLS_HEADER_LEN];
        header[0] = TLS_CONTENT_TYPE_APPLICATION_DATA;
        header[1..3].copy_from_slice(&TLS_LEGACY_VERSION);
        header[3..5].copy_from_slice(&(encrypted_len as u16).to_be_bytes());

        let mut body = Vec::new();
        body.extend_from_slice(plaintext);
        body.push(TLS_CONTENT_TYPE_APPLICATION_DATA);

        let nonce = self.next_nonce()?;
        let cipher =
            ChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| TlsRecordError::Auth)?;
        let tag = cipher
            .encrypt_in_place_detached(Nonce::from_slice(&nonce), &header, &mut body)
            .map_err(|_| TlsRecordError::Auth)?;
        self.advance_sequence()?;

        let mut record = Vec::new();
        record.extend_from_slice(&header);
        record.extend_from_slice(&body);
        record.extend_from_slice(tag.as_slice());
        Ok(record)
    }

    fn open_application_records(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, TlsRecordError> {
        let mut cursor = 0;
        let mut plaintext = Vec::new();
        while cursor < ciphertext.len() {
            if ciphertext.len() - cursor < TLS_HEADER_LEN {
                return Err(TlsRecordError::InvalidRecord);
            }
            let header = &ciphertext[cursor..cursor + TLS_HEADER_LEN];
            if header[0] != TLS_CONTENT_TYPE_APPLICATION_DATA || header[1..3] != TLS_LEGACY_VERSION
            {
                return Err(TlsRecordError::InvalidRecord);
            }
            let record_len = u16::from_be_bytes([header[3], header[4]]) as usize;
            if !(TLS_TAG_LEN..=TLS_MAX_RECORD_PAYLOAD).contains(&record_len) {
                return Err(TlsRecordError::InvalidRecord);
            }
            let record_end = cursor
                .checked_add(TLS_HEADER_LEN)
                .and_then(|v| v.checked_add(record_len))
                .ok_or(TlsRecordError::InvalidRecord)?;
            if record_end > ciphertext.len() {
                return Err(TlsRecordError::InvalidRecord);
            }

            let encrypted = &ciphertext[cursor + TLS_HEADER_LEN..record_end];
            let split = encrypted.len() - TLS_TAG_LEN;
            let (body_bytes, tag_bytes) = encrypted.split_at(split);
            let mut body = Vec::new();
            body.extend_from_slice(body_bytes);
            let nonce = self.next_nonce()?;
            let cipher =
                ChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| TlsRecordError::Auth)?;
            cipher
                .decrypt_in_place_detached(
                    Nonce::from_slice(&nonce),
                    header,
                    &mut body,
                    Tag::from_slice(tag_bytes),
                )
                .map_err(|_| TlsRecordError::Auth)?;
            self.advance_sequence()?;

            let content_type = trim_padding_and_type(&mut body)?;
            if content_type != TLS_CONTENT_TYPE_APPLICATION_DATA {
                return Err(TlsRecordError::InvalidRecord);
            }
            plaintext.extend_from_slice(&body);
            cursor = record_end;
        }
        Ok(plaintext)
    }

    fn next_nonce(&self) -> Result<[u8; IV_LEN], TlsRecordError> {
        if self.sequence == u64::MAX {
            return Err(TlsRecordError::SequenceOverflow);
        }
        let mut nonce = self.iv;
        let seq = self.sequence.to_be_bytes();
        for idx in 0..seq.len() {
            nonce[IV_LEN - seq.len() + idx] ^= seq[idx];
        }
        Ok(nonce)
    }

    fn advance_sequence(&mut self) -> Result<(), TlsRecordError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(TlsRecordError::SequenceOverflow)?;
        Ok(())
    }
}

impl Drop for DirectionState {
    fn drop(&mut self) {
        wipe_secret(&mut self.key);
        wipe_secret(&mut self.iv);
    }
}

pub fn sealed_len(plaintext_len: usize) -> Result<usize, TlsRecordError> {
    if plaintext_len > TLS_MAX_INNER_PLAINTEXT {
        return Err(TlsRecordError::RecordTooLarge);
    }
    TLS_HEADER_LEN
        .checked_add(plaintext_len)
        .and_then(|v| v.checked_add(TLS_INNER_CONTENT_TYPE_LEN))
        .and_then(|v| v.checked_add(TLS_TAG_LEN))
        .ok_or(TlsRecordError::RecordTooLarge)
}

pub fn wipe_secret(buf: &mut [u8]) {
    for byte in buf {
        unsafe {
            core::ptr::write_volatile(byte, 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
}

fn trim_padding_and_type(body: &mut Vec<u8>) -> Result<u8, TlsRecordError> {
    loop {
        let Some(last) = body.pop() else {
            return Err(TlsRecordError::InvalidRecord);
        };
        if last != 0 {
            return Ok(last);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_material() -> [u8; KEY_MATERIAL_LEN] {
        let mut material = [0u8; KEY_MATERIAL_LEN];
        for (idx, byte) in material.iter_mut().enumerate() {
            *byte = idx as u8;
        }
        material
    }

    #[test]
    fn chacha20_poly1305_rfc8439_aead_vector() {
        let key = [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d,
            0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b,
            0x9c, 0x9d, 0x9e, 0x9f,
        ];
        let nonce = [
            0x07, 0x00, 0x00, 0x00, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
        ];
        let aad = [
            0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
        ];
        let mut body = Vec::new();
        body.extend_from_slice(b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.");
        let expected = [
            0xd3, 0x1a, 0x8d, 0x34, 0x64, 0x8e, 0x60, 0xdb, 0x7b, 0x86, 0xaf, 0xbc, 0x53, 0xef,
            0x7e, 0xc2, 0xa4, 0xad, 0xed, 0x51, 0x29, 0x6e, 0x08, 0xfe, 0xa9, 0xe2, 0xb5, 0xa7,
            0x36, 0xee, 0x62, 0xd6, 0x3d, 0xbe, 0xa4, 0x5e, 0x8c, 0xa9, 0x67, 0x12, 0x82, 0xfa,
            0xfb, 0x69, 0xda, 0x92, 0x72, 0x8b, 0x1a, 0x71, 0xde, 0x0a, 0x9e, 0x06, 0x0b, 0x29,
            0x05, 0xd6, 0xa5, 0xb6, 0x7e, 0xcd, 0x3b, 0x36, 0x92, 0xdd, 0xbd, 0x7f, 0x2d, 0x77,
            0x8b, 0x8c, 0x98, 0x03, 0xae, 0xe3, 0x28, 0x09, 0x1b, 0x58, 0xfa, 0xb3, 0x24, 0xe4,
            0xfa, 0xd6, 0x75, 0x94, 0x55, 0x85, 0x80, 0x8b, 0x48, 0x31, 0xd7, 0xbc, 0x3f, 0xf4,
            0xde, 0xf0, 0x8e, 0x4b, 0x7a, 0x9d, 0xe5, 0x76, 0xd2, 0x65, 0x86, 0xce, 0xc6, 0x4b,
            0x61, 0x16,
        ];
        let expected_tag = [
            0x1a, 0xe1, 0x0b, 0x59, 0x4f, 0x09, 0xe2, 0x6a, 0x7e, 0x90, 0x2e, 0xcb, 0xd0, 0x60,
            0x06, 0x91,
        ];
        let cipher = match ChaCha20Poly1305::new_from_slice(&key) {
            Ok(cipher) => cipher,
            Err(_) => {
                assert!(false, "test key length is fixed");
                return;
            }
        };
        let tag = cipher
            .encrypt_in_place_detached(Nonce::from_slice(&nonce), &aad, &mut body)
            .map_or([0u8; TLS_TAG_LEN], |tag| {
                let mut out = [0u8; TLS_TAG_LEN];
                out.copy_from_slice(tag.as_slice());
                out
            });
        assert_eq!(body.as_slice(), expected.as_slice());
        assert_eq!(tag, expected_tag);
    }

    #[test]
    fn key_material_is_zeroized_on_install() {
        let mut material = key_material();
        let state = TlsSocketState::from_key_material(&mut material);
        assert!(matches!(state, Ok(_)));
        assert!(material.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn tls_record_round_trips_application_data() {
        let mut tx_material = key_material();
        let mut rx_material = key_material();
        let mut tx = match TlsSocketState::from_key_material(&mut tx_material) {
            Ok(state) => state,
            Err(_) => {
                assert!(false, "fixed key material should install");
                return;
            }
        };
        let mut rx = match TlsSocketState::from_key_material(&mut rx_material) {
            Ok(state) => state,
            Err(_) => {
                assert!(false, "fixed key material should install");
                return;
            }
        };
        let sealed = match tx.seal_application_data(b"GET / HTTP/1.1\r\n\r\n") {
            Ok(record) => record,
            Err(_) => {
                assert!(false, "record seal should succeed");
                return;
            }
        };
        assert_eq!(sealed[0], TLS_CONTENT_TYPE_APPLICATION_DATA);
        let opened = rx.open_application_records(&sealed);
        assert!(matches!(opened, Ok(ref data) if data == b"GET / HTTP/1.1\r\n\r\n"));
    }

    #[test]
    fn tls_record_rejects_forged_tag() {
        let mut tx_material = key_material();
        let mut rx_material = key_material();
        let mut tx = match TlsSocketState::from_key_material(&mut tx_material) {
            Ok(state) => state,
            Err(_) => {
                assert!(false, "fixed key material should install");
                return;
            }
        };
        let mut rx = match TlsSocketState::from_key_material(&mut rx_material) {
            Ok(state) => state,
            Err(_) => {
                assert!(false, "fixed key material should install");
                return;
            }
        };
        let mut sealed = match tx.seal_application_data(b"neo") {
            Ok(record) => record,
            Err(_) => {
                assert!(false, "record seal should succeed");
                return;
            }
        };
        if let Some(last) = sealed.last_mut() {
            *last ^= 0x80;
        }
        assert!(matches!(
            rx.open_application_records(&sealed),
            Err(TlsRecordError::Auth)
        ));
    }
}
