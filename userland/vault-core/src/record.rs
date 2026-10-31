// SPDX-License-Identifier: MPL-2.0
//! Stable encrypted-secret record header and parser.

use alloc::vec::Vec;

pub const MAGIC: [u8; 8] = *b"NEOVLT01";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 40;
pub const KDF_ARGON2ID: u8 = 1;
pub const AEAD_CHACHA20_POLY1305: u8 = 1;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordError {
    Truncated,
    BadMagic,
    Unsupported,
    BadLength,
    Reserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub ciphertext_len: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RecordView<'a> {
    pub header: Header,
    pub salt: &'a [u8; SALT_LEN],
    pub nonce: &'a [u8; NONCE_LEN],
    pub ciphertext_and_tag: &'a [u8],
}

impl Header {
    pub fn encode(self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[..8].copy_from_slice(&MAGIC);
        out[8..10].copy_from_slice(&VERSION.to_le_bytes());
        out[10..12].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
        out[14] = KDF_ARGON2ID;
        out[15] = AEAD_CHACHA20_POLY1305;
        out[16..20].copy_from_slice(&self.memory_kib.to_le_bytes());
        out[20..24].copy_from_slice(&self.iterations.to_le_bytes());
        out[24..28].copy_from_slice(&self.parallelism.to_le_bytes());
        out[28..30].copy_from_slice(&(SALT_LEN as u16).to_le_bytes());
        out[30..32].copy_from_slice(&(NONCE_LEN as u16).to_le_bytes());
        out[32..36].copy_from_slice(&self.ciphertext_len.to_le_bytes());
        out[36..38].copy_from_slice(&(TAG_LEN as u16).to_le_bytes());
        out
    }
}

pub fn assemble(
    header: Header,
    salt: &[u8; SALT_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext_and_tag: &[u8],
) -> Result<Vec<u8>, RecordError> {
    if ciphertext_and_tag.len() != header.ciphertext_len as usize + TAG_LEN {
        return Err(RecordError::BadLength);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + SALT_LEN + NONCE_LEN + ciphertext_and_tag.len());
    out.extend_from_slice(&header.encode());
    out.extend_from_slice(salt);
    out.extend_from_slice(nonce);
    out.extend_from_slice(ciphertext_and_tag);
    Ok(out)
}

pub fn decode_header(input: &[u8]) -> Result<Header, RecordError> {
    if input.len() < HEADER_LEN {
        return Err(RecordError::Truncated);
    }
    if input[..8] != MAGIC {
        return Err(RecordError::BadMagic);
    }
    if u16::from_le_bytes([input[8], input[9]]) != VERSION
        || u16::from_le_bytes([input[10], input[11]]) as usize != HEADER_LEN
        || input[14] != KDF_ARGON2ID
        || input[15] != AEAD_CHACHA20_POLY1305
    {
        return Err(RecordError::Unsupported);
    }
    if input[12..14] != [0, 0] || input[38..40] != [0, 0] {
        return Err(RecordError::Reserved);
    }
    if u16::from_le_bytes([input[28], input[29]]) as usize != SALT_LEN
        || u16::from_le_bytes([input[30], input[31]]) as usize != NONCE_LEN
        || u16::from_le_bytes([input[36], input[37]]) as usize != TAG_LEN
    {
        return Err(RecordError::BadLength);
    }
    Ok(Header {
        memory_kib: u32::from_le_bytes(input[16..20].try_into().unwrap()),
        iterations: u32::from_le_bytes(input[20..24].try_into().unwrap()),
        parallelism: u32::from_le_bytes(input[24..28].try_into().unwrap()),
        ciphertext_len: u32::from_le_bytes(input[32..36].try_into().unwrap()),
    })
}

pub fn parse(input: &[u8]) -> Result<RecordView<'_>, RecordError> {
    let header = decode_header(input)?;
    let body_len = (header.ciphertext_len as usize)
        .checked_add(TAG_LEN)
        .and_then(|len| len.checked_add(SALT_LEN + NONCE_LEN))
        .and_then(|len| len.checked_add(HEADER_LEN))
        .ok_or(RecordError::BadLength)?;
    if input.len() != body_len {
        return Err(RecordError::BadLength);
    }
    let salt_start = HEADER_LEN;
    let nonce_start = salt_start + SALT_LEN;
    let body_start = nonce_start + NONCE_LEN;
    Ok(RecordView {
        header,
        salt: input[salt_start..nonce_start]
            .try_into()
            .map_err(|_| RecordError::BadLength)?,
        nonce: input[nonce_start..body_start]
            .try_into()
            .map_err(|_| RecordError::BadLength)?,
        ciphertext_and_tag: &input[body_start..],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn hand_laid_record_byte_oracle() {
        let header = Header {
            memory_kib: 0x0001_0000,
            iterations: 3,
            parallelism: 1,
            ciphertext_len: 3,
        };
        let mut expected = vec![
            0x4e, 0x45, 0x4f, 0x56, 0x4c, 0x54, 0x30, 0x31, 0x01, 0x00, 0x28, 0x00, 0x00, 0x00,
            0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x10, 0x00, 0x0c, 0x00, 0x03, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
        ];
        expected.extend(0xa0..=0xaf);
        expected.extend(0xb0..=0xbb);
        expected.extend_from_slice(&[0xc0, 0xc1, 0xc2]);
        expected.extend(0xd0..=0xdf);
        let mut body = vec![0xc0, 0xc1, 0xc2];
        body.extend(0xd0..=0xdf);
        let actual = assemble(
            header,
            &core::array::from_fn(|i| 0xa0 + i as u8),
            &core::array::from_fn(|i| 0xb0 + i as u8),
            &body,
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(decode_header(&expected).unwrap(), header);
        let parsed = parse(&expected).unwrap();
        assert_eq!(parsed.header, header);
        assert_eq!(parsed.salt, &core::array::from_fn(|i| 0xa0 + i as u8));
        assert_eq!(parsed.nonce, &core::array::from_fn(|i| 0xb0 + i as u8));
        assert_eq!(parsed.ciphertext_and_tag, &body);
        let mut trailing = expected;
        trailing.push(0);
        assert_eq!(parse(&trailing), Err(RecordError::BadLength));
    }
}
