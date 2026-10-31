// SPDX-License-Identifier: MPL-2.0
//! Byte-level BPE tokenizer logic for GGUF import tests.
//!
//! The implementation accepts raw-source vocab and merge slices. It does not
//! parse JSON; importer/tooling can feed the tables it extracted from GGUF or
//! tokenizer assets, while the kernel-side logic stays small and deterministic.
#![allow(dead_code)] // default-off importer surface; host KATs consume it first.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::str;

#[derive(Copy, Clone)]
pub struct BpeVocabEntry<'a> {
    pub token: &'a [u8],
    pub id: u32,
}

#[derive(Copy, Clone)]
pub struct BpeMergeEntry<'a> {
    pub left: &'a [u8],
    pub right: &'a [u8],
    pub rank: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TokenizerError {
    InvalidToken,
    UnknownToken,
    UnknownId,
    InvalidUtf8,
}

pub struct BpeTokenizer {
    vocab: BTreeMap<Vec<u8>, u32>,
    ids: BTreeMap<u32, Vec<u8>>,
    merges: BTreeMap<(Vec<u8>, Vec<u8>), u32>,
}

impl BpeTokenizer {
    pub fn from_slices(
        vocab: &[BpeVocabEntry<'_>],
        merges: &[BpeMergeEntry<'_>],
    ) -> Result<Self, TokenizerError> {
        let mut vocab_map = BTreeMap::new();
        let mut ids = BTreeMap::new();
        for entry in vocab {
            if entry.token.is_empty()
                || vocab_map.insert(entry.token.to_vec(), entry.id).is_some()
                || ids.insert(entry.id, entry.token.to_vec()).is_some()
            {
                return Err(TokenizerError::InvalidToken);
            }
        }

        let mut merge_map = BTreeMap::new();
        for entry in merges {
            if entry.left.is_empty() || entry.right.is_empty() {
                return Err(TokenizerError::InvalidToken);
            }
            let key = (entry.left.to_vec(), entry.right.to_vec());
            if merge_map.insert(key, entry.rank).is_some() {
                return Err(TokenizerError::InvalidToken);
            }
        }

        Ok(Self {
            vocab: vocab_map,
            ids,
            merges: merge_map,
        })
    }

    pub fn encode(&self, input: &str) -> Result<Vec<u32>, TokenizerError> {
        encode_with(self, input)
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String, TokenizerError> {
        decode_with(self, ids)
    }
}

impl BpeTables for BpeTokenizer {
    fn token_id(&self, token: &[u8]) -> Option<u32> {
        self.vocab.get(token).copied()
    }

    fn token_bytes(&self, id: u32) -> Option<&[u8]> {
        self.ids.get(&id).map(Vec::as_slice)
    }

    fn merge_rank(&self, left: &[u8], right: &[u8]) -> Option<u32> {
        self.merges.get(&(left.to_vec(), right.to_vec())).copied()
    }
}

/// The three lookups byte-level BPE needs. The text tables above and the
/// prebuilt binary vocab (`vocab_bin_logic`) both answer them, and share the
/// one encode/decode below, so the two backings cannot drift apart.
pub trait BpeTables {
    /// The id of the token spelled `token` (byte-level-mapped bytes).
    fn token_id(&self, token: &[u8]) -> Option<u32>;
    /// The bytes of token `id`.
    fn token_bytes(&self, id: u32) -> Option<&[u8]>;
    /// The rank of the merge `left` + `right` (lower merges first).
    fn merge_rank(&self, left: &[u8], right: &[u8]) -> Option<u32>;
}

pub fn encode_with<T: BpeTables + ?Sized>(
    tables: &T,
    input: &str,
) -> Result<Vec<u32>, TokenizerError> {
    let pieces = split_qwen_bytelevel_pieces(input.as_bytes());
    let mut out = Vec::new();
    for piece in pieces {
        let bytelevel = bytelevel_encode(piece)?;
        let mut symbols = split_utf8_symbols(&bytelevel)?;
        apply_merges(tables, &mut symbols);
        for symbol in symbols {
            let id = tables
                .token_id(&symbol)
                .ok_or(TokenizerError::UnknownToken)?;
            out.push(id);
        }
    }
    Ok(out)
}

pub fn decode_with<T: BpeTables + ?Sized>(
    tables: &T,
    ids: &[u32],
) -> Result<String, TokenizerError> {
    let mut bytelevel = Vec::new();
    for id in ids {
        let token = tables.token_bytes(*id).ok_or(TokenizerError::UnknownId)?;
        bytelevel.extend_from_slice(token);
    }
    bytelevel_decode(&bytelevel)
}

fn apply_merges<T: BpeTables + ?Sized>(tables: &T, symbols: &mut Vec<Vec<u8>>) {
    while let Some((_best_rank, best_left, best_right)) = best_pair(tables, symbols) {
        let mut merged = Vec::with_capacity(symbols.len());
        let mut idx = 0usize;
        while idx < symbols.len() {
            if idx + 1 < symbols.len()
                && symbols[idx] == best_left
                && symbols[idx + 1] == best_right
            {
                let mut token = symbols[idx].clone();
                token.extend_from_slice(&symbols[idx + 1]);
                merged.push(token);
                idx += 2;
            } else {
                merged.push(symbols[idx].clone());
                idx += 1;
            }
        }
        *symbols = merged;
    }
}

fn best_pair<T: BpeTables + ?Sized>(
    tables: &T,
    symbols: &[Vec<u8>],
) -> Option<(u32, Vec<u8>, Vec<u8>)> {
    if symbols.len() < 2 {
        return None;
    }
    let mut best: Option<(u32, Vec<u8>, Vec<u8>)> = None;
    let mut idx = 0usize;
    while idx + 1 < symbols.len() {
        if let Some(rank) = tables.merge_rank(&symbols[idx], &symbols[idx + 1]) {
            match &best {
                Some((best_rank, _, _)) if *best_rank <= rank => {}
                _ => best = Some((rank, symbols[idx].clone(), symbols[idx + 1].clone())),
            }
        }
        idx += 1;
    }
    best
}

fn split_qwen_bytelevel_pieces(input: &[u8]) -> Vec<&[u8]> {
    let mut pieces = Vec::new();
    let mut idx = 0usize;
    while idx < input.len() {
        let start = idx;
        let b = input[idx];
        if is_ascii_letter(b)
            || (b == b' ' && idx + 1 < input.len() && is_ascii_letter(input[idx + 1]))
        {
            idx += 1;
            while idx < input.len() && is_ascii_letter(input[idx]) {
                idx += 1;
            }
        } else if is_ascii_digit(b) {
            idx += 1;
        } else if is_ascii_space(b) {
            idx += 1;
            while idx < input.len() && is_ascii_space(input[idx]) {
                idx += 1;
            }
        } else {
            idx += 1;
            while idx < input.len()
                && !is_ascii_space(input[idx])
                && !is_ascii_letter(input[idx])
                && !is_ascii_digit(input[idx])
            {
                idx += 1;
            }
            while idx < input.len() && (input[idx] == b'\r' || input[idx] == b'\n') {
                idx += 1;
            }
        }
        pieces.push(&input[start..idx]);
    }
    pieces
}

fn split_utf8_symbols(bytes: &[u8]) -> Result<Vec<Vec<u8>>, TokenizerError> {
    let text = str::from_utf8(bytes).map_err(|_| TokenizerError::InvalidUtf8)?;
    let mut out = Vec::new();
    for ch in text.chars() {
        let mut buf = [0u8; 4];
        out.push(ch.encode_utf8(&mut buf).as_bytes().to_vec());
    }
    Ok(out)
}

fn bytelevel_encode(bytes: &[u8]) -> Result<Vec<u8>, TokenizerError> {
    let mut out = Vec::new();
    for byte in bytes {
        let scalar = byte_to_unicode_scalar(*byte);
        let ch = char::from_u32(scalar).ok_or(TokenizerError::InvalidToken)?;
        let mut buf = [0u8; 4];
        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
    }
    Ok(out)
}

fn bytelevel_decode(bytes: &[u8]) -> Result<String, TokenizerError> {
    let text = str::from_utf8(bytes).map_err(|_| TokenizerError::InvalidUtf8)?;
    let mut out = Vec::new();
    for ch in text.chars() {
        out.push(unicode_scalar_to_byte(ch as u32)?);
    }
    String::from_utf8(out).map_err(|_| TokenizerError::InvalidUtf8)
}

fn byte_to_unicode_scalar(byte: u8) -> u32 {
    if is_direct_byte(byte) {
        return byte as u32;
    }
    let mut skipped = 0u32;
    let mut current = 0u16;
    while current < byte as u16 {
        if !is_direct_byte(current as u8) {
            skipped += 1;
        }
        current += 1;
    }
    256 + skipped
}

fn unicode_scalar_to_byte(scalar: u32) -> Result<u8, TokenizerError> {
    let mut byte = 0u16;
    while byte <= u8::MAX as u16 {
        if byte_to_unicode_scalar(byte as u8) == scalar {
            return Ok(byte as u8);
        }
        byte += 1;
    }
    Err(TokenizerError::InvalidToken)
}

const fn is_direct_byte(byte: u8) -> bool {
    (byte >= 33 && byte <= 126) || (byte >= 161 && byte <= 172) || (byte >= 174)
}

const fn is_ascii_letter(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_uppercase()
}

const fn is_ascii_digit(byte: u8) -> bool {
    byte.is_ascii_digit()
}

const fn is_ascii_space(byte: u8) -> bool {
    byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r'
}
