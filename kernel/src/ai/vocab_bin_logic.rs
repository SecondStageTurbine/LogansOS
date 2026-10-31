// SPDX-License-Identifier: MPL-2.0
//! `NEOVOCB1`: the tokenizer vocab prebuilt for mapping, not parsing
//! (`Plans/NOTE-2026-10-26-arthur-binary-vocab-design-s1.md`).
//!
//! `xtask` converts the pinned text sidecar (`NEOM_TOKENIZER 1`) once, at
//! image build, with [`write`]. Arthur reads the result into one buffer and
//! [`VocabBin::new`] validates it in one linear pass; the tokenizer then looks
//! everything up in place (binary search over sorted arrays). Encode and
//! decode are `tokenizer_logic`'s, shared with the text tables through
//! [`BpeTables`], so the two backings run one algorithm.
//!
//! Layout (all integers little-endian; sections 8-byte aligned with zero
//! padding, in this order, each offset from the start of the file):
//!
//! | Offset | Size | Field |
//! |---|---|---|
//! | 0x00 | 8 | magic `NEOVOCB1` |
//! | 0x08 | 4 | version = 1 |
//! | 0x0C | 4 | header_size = 64 |
//! | 0x10 | 4 | token_count N (ids dense, `0..N`) |
//! | 0x14 | 4 | merge_count M |
//! | 0x18 | 4 | bos id, or `0xFFFF_FFFF` for none |
//! | 0x1C | 4 | eos id, or none |
//! | 0x20 | 4 | pad id, or none |
//! | 0x24 | 4 | flags = 0 |
//! | 0x28 | 8 | total_size (== file length) |
//! | 0x30 | 4 | offsets_off: `N + 1` × u32, token `i` is `blob[off[i]..off[i+1]]` |
//! | 0x34 | 4 | blob_off: the token bytes, concatenated by id |
//! | 0x38 | 4 | sorted_off: `N` × u32, the ids ordered by their bytes |
//! | 0x3C | 4 | merges_off: `M` × `{left, right, result, rank}` u32, sorted by `(left, right)` |
//!
//! The section offsets are not free: each must be exactly where the previous
//! section ends, rounded up to 8. That is the writer's only layout, so the
//! reader accepts nothing else.
//!
//! Pure and `alloc`-only: host-tested by `tools/ai-ops-tests` through
//! `#[path]`, mounted beside `tokenizer_logic` (reached as
//! `super::tokenizer_logic`).
#![allow(dead_code)] // the writer is host/xtask-only; Arthur uses the reader.

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use super::tokenizer_logic::{BpeTables, TokenizerError, decode_with, encode_with};

pub const MAGIC: [u8; 8] = *b"NEOVOCB1";
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
/// The bos/eos/pad field value for "no such token".
pub const NO_ID: u32 = 0xFFFF_FFFF;
const MERGE_RECORD: usize = 16;

/// Why the converter refuses a source vocab. The file is never written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// The text sidecar did not parse.
    Unparseable,
    /// Ids are not exactly `0..N`, each once: this id is missing.
    MissingId(u32),
    /// This id appears twice, or is `>= N`.
    BadId(u32),
    /// Token `id` is empty.
    EmptyToken(u32),
    /// Token `id` has the same bytes as an earlier one.
    DuplicateToken(u32),
    /// Merge ranks are not exactly `0..M`, each once: this rank is wrong.
    BadRank(u32),
    /// The merge of this rank has a part that is not a token.
    MergePartNotToken(u32),
    /// The merge of this rank yields bytes that are not a token.
    MergeResultNotToken(u32),
    /// The merge of this rank repeats an earlier merge's pair.
    DuplicateMerge(u32),
    /// bos, eos or pad names no token.
    SpecialOutOfRange,
    /// The file would not fit the format's u32 offsets.
    TooLarge,
}

/// Why the reader refuses a file: one variant per validation rule, so the
/// rejection tests can show each rule is the one that fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VocabBinError {
    TooShort,
    Magic,
    Version,
    HeaderSize,
    Flags,
    TotalSize,
    /// A section is not where the previous one ends (rounded up to 8), or
    /// runs past the file.
    Layout,
    /// A padding byte is not zero.
    Padding,
    /// `offsets[0] != 0`, a token is empty, or an offset goes backwards.
    Offsets,
    /// `sorted` names an id twice or one `>= N`.
    SortedNotPermutation,
    /// `sorted` is not strictly increasing by token bytes.
    SortedOrder,
    /// Merges are not strictly increasing by `(left, right)`.
    MergeOrder,
    /// A merge names an id `>= N`.
    MergeId,
    /// A merge rank is `>= M` or repeats.
    MergeRank,
    /// `bytes(left) ++ bytes(right) != bytes(result)`.
    MergeBytes,
    /// bos, eos or pad is neither none nor `< N`.
    Special,
}

fn align8(x: u64) -> u64 {
    (x + 7) & !7
}

fn opt_id(id: Option<u32>) -> u32 {
    id.unwrap_or(NO_ID)
}

/// Build a `NEOVOCB1` file. `tokens` are `(id, bytes)` in any order, `merges`
/// are `(left, right, rank)` in any order. Refuses (and writes nothing)
/// unless the ids are dense, the tokens non-empty and unique, the ranks
/// exactly `0..M`, every merge's parts and result tokens, and no pair merged
/// twice. Deterministic: the same source gives the same bytes.
pub fn write(
    tokens: &[(u32, &[u8])],
    merges: &[(&[u8], &[u8], u32)],
    bos: Option<u32>,
    eos: Option<u32>,
    pad: Option<u32>,
) -> Result<Vec<u8>, ConvertError> {
    let n = tokens.len();
    if n >= NO_ID as usize || merges.len() >= NO_ID as usize {
        return Err(ConvertError::TooLarge);
    }
    // Dense ids: token `i` at index `i`.
    let mut by_id: Vec<Option<&[u8]>> = vec![None; n];
    for &(id, bytes) in tokens {
        let slot = by_id.get_mut(id as usize).ok_or(ConvertError::BadId(id))?;
        if slot.is_some() {
            return Err(ConvertError::BadId(id));
        }
        if bytes.is_empty() {
            return Err(ConvertError::EmptyToken(id));
        }
        *slot = Some(bytes);
    }
    let mut by_id_bytes: Vec<&[u8]> = Vec::with_capacity(n);
    for (id, slot) in by_id.iter().enumerate() {
        by_id_bytes.push(slot.ok_or(ConvertError::MissingId(id as u32))?);
    }
    let by_id = by_id_bytes;

    let mut sorted: Vec<u32> = (0..n as u32).collect();
    sorted.sort_by(|&a, &b| by_id[a as usize].cmp(by_id[b as usize]).then(a.cmp(&b)));
    for pair in sorted.windows(2) {
        if by_id[pair[0] as usize] == by_id[pair[1] as usize] {
            return Err(ConvertError::DuplicateToken(pair[0].max(pair[1])));
        }
    }
    let lookup = |bytes: &[u8]| -> Option<u32> {
        sorted
            .binary_search_by(|&id| by_id[id as usize].cmp(bytes))
            .ok()
            .map(|i| sorted[i])
    };

    let m = merges.len();
    let mut rank_seen = vec![false; m];
    let mut records: Vec<[u32; 4]> = Vec::with_capacity(m);
    for &(left, right, rank) in merges {
        let seen = rank_seen
            .get_mut(rank as usize)
            .ok_or(ConvertError::BadRank(rank))?;
        if *seen {
            return Err(ConvertError::BadRank(rank));
        }
        *seen = true;
        let l = lookup(left).ok_or(ConvertError::MergePartNotToken(rank))?;
        let r = lookup(right).ok_or(ConvertError::MergePartNotToken(rank))?;
        let mut joined = Vec::with_capacity(left.len() + right.len());
        joined.extend_from_slice(left);
        joined.extend_from_slice(right);
        let result = lookup(&joined).ok_or(ConvertError::MergeResultNotToken(rank))?;
        records.push([l, r, result, rank]);
    }
    records.sort_by(|a, b| (a[0], a[1]).cmp(&(b[0], b[1])).then(a[3].cmp(&b[3])));
    for pair in records.windows(2) {
        if (pair[0][0], pair[0][1]) == (pair[1][0], pair[1][1]) {
            return Err(ConvertError::DuplicateMerge(pair[0][3].max(pair[1][3])));
        }
    }
    for id in [bos, eos, pad].into_iter().flatten() {
        if id as usize >= n {
            return Err(ConvertError::SpecialOutOfRange);
        }
    }

    let blob_len: u64 = by_id.iter().map(|t| t.len() as u64).sum();
    let offsets_off = HEADER_SIZE as u64;
    let blob_off = align8(offsets_off + 4 * (n as u64 + 1));
    let sorted_off = align8(blob_off + blob_len);
    let merges_off = align8(sorted_off + 4 * n as u64);
    let total = merges_off + (MERGE_RECORD * m) as u64;
    if merges_off > u32::MAX as u64 || blob_len > u32::MAX as u64 {
        return Err(ConvertError::TooLarge);
    }

    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(&MAGIC);
    for v in [
        VERSION,
        HEADER_SIZE as u32,
        n as u32,
        m as u32,
        opt_id(bos),
        opt_id(eos),
        opt_id(pad),
        0,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&total.to_le_bytes());
    for v in [offsets_off, blob_off, sorted_off, merges_off] {
        out.extend_from_slice(&(v as u32).to_le_bytes());
    }
    let mut at = 0u32;
    out.extend_from_slice(&at.to_le_bytes());
    for token in &by_id {
        at += token.len() as u32;
        out.extend_from_slice(&at.to_le_bytes());
    }
    out.resize(blob_off as usize, 0);
    for token in &by_id {
        out.extend_from_slice(token);
    }
    out.resize(sorted_off as usize, 0);
    for id in &sorted {
        out.extend_from_slice(&id.to_le_bytes());
    }
    out.resize(merges_off as usize, 0);
    for record in &records {
        for v in record {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    debug_assert_eq!(out.len() as u64, total);
    Ok(out)
}

/// A validated `NEOVOCB1` file, used in place. `B` is the storage: Arthur
/// hands it the `Vec<u8>` it read; tests borrow a slice.
pub struct VocabBin<B> {
    bytes: B,
    n: u32,
    m: u32,
    bos: u32,
    eos: u32,
    pad: u32,
    offsets_off: usize,
    blob_off: usize,
    sorted_off: usize,
    merges_off: usize,
}

fn rd32(bytes: &[u8], at: usize) -> u32 {
    let b = &bytes[at..at + 4];
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn rd64(bytes: &[u8], at: usize) -> u64 {
    let b = &bytes[at..at + 8];
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

fn zeros(bytes: &[u8]) -> Result<(), VocabBinError> {
    if bytes.iter().all(|&b| b == 0) {
        Ok(())
    } else {
        Err(VocabBinError::Padding)
    }
}

impl<B: AsRef<[u8]>> VocabBin<B> {
    /// Validate `bytes` in one pass; nothing is trusted before it is
    /// bounded. Any failure refuses the whole file.
    pub fn new(bytes: B) -> Result<Self, VocabBinError> {
        let f = bytes.as_ref();
        let len = f.len() as u64;
        if f.len() < HEADER_SIZE {
            return Err(VocabBinError::TooShort);
        }
        if f[..8] != MAGIC {
            return Err(VocabBinError::Magic);
        }
        if rd32(f, 0x08) != VERSION {
            return Err(VocabBinError::Version);
        }
        if rd32(f, 0x0C) as usize != HEADER_SIZE {
            return Err(VocabBinError::HeaderSize);
        }
        let n = rd32(f, 0x10);
        let m = rd32(f, 0x14);
        let (bos, eos, pad) = (rd32(f, 0x18), rd32(f, 0x1C), rd32(f, 0x20));
        if rd32(f, 0x24) != 0 {
            return Err(VocabBinError::Flags);
        }
        if rd64(f, 0x28) != len {
            return Err(VocabBinError::TotalSize);
        }
        let offsets_off = rd32(f, 0x30) as u64;
        let blob_off = rd32(f, 0x34) as u64;
        let sorted_off = rd32(f, 0x38) as u64;
        let merges_off = rd32(f, 0x3C) as u64;
        let (n64, m64) = (n as u64, m as u64);

        // Offsets: exactly after the header, and inside the file.
        let offsets_end = offsets_off + 4 * (n64 + 1);
        if offsets_off != HEADER_SIZE as u64 || offsets_end > len {
            return Err(VocabBinError::Layout);
        }
        // The blob: exactly after the offsets, as long as `offsets[N]`.
        let blob_len = rd32(f, (offsets_off + 4 * n64) as usize) as u64;
        let blob_end = blob_off + blob_len;
        if blob_off != align8(offsets_end) || blob_end > len {
            return Err(VocabBinError::Layout);
        }
        let sorted_end = sorted_off + 4 * n64;
        if sorted_off != align8(blob_end) || sorted_end > len {
            return Err(VocabBinError::Layout);
        }
        if merges_off != align8(sorted_end) || merges_off + MERGE_RECORD as u64 * m64 != len {
            return Err(VocabBinError::Layout);
        }
        zeros(&f[offsets_end as usize..blob_off as usize])?;
        zeros(&f[blob_end as usize..sorted_off as usize])?;
        zeros(&f[sorted_end as usize..merges_off as usize])?;

        let v = Self {
            n,
            m,
            bos,
            eos,
            pad,
            offsets_off: offsets_off as usize,
            blob_off: blob_off as usize,
            sorted_off: sorted_off as usize,
            merges_off: merges_off as usize,
            bytes,
        };
        let f = v.bytes.as_ref();

        // Offsets start at 0 and strictly increase (no empty token).
        if rd32(f, v.offsets_off) != 0 {
            return Err(VocabBinError::Offsets);
        }
        for i in 0..v.n as usize {
            let a = rd32(f, v.offsets_off + 4 * i);
            let b = rd32(f, v.offsets_off + 4 * (i + 1));
            if b <= a {
                return Err(VocabBinError::Offsets);
            }
        }

        // `sorted` is a permutation of 0..N, strictly increasing by bytes.
        let mut seen = vec![0u64; (v.n as usize).div_ceil(64)];
        let mut prev: Option<u32> = None;
        for i in 0..v.n as usize {
            let id = rd32(f, v.sorted_off + 4 * i);
            if id >= v.n || seen[id as usize / 64] & (1 << (id % 64)) != 0 {
                return Err(VocabBinError::SortedNotPermutation);
            }
            seen[id as usize / 64] |= 1 << (id % 64);
            if let Some(p) = prev
                && v.bytes_of(p) >= v.bytes_of(id)
            {
                return Err(VocabBinError::SortedOrder);
            }
            prev = Some(id);
        }

        // Merges: strictly increasing pairs, ids < N, ranks a permutation of
        // 0..M, and left ++ right == result.
        let mut ranks = vec![0u64; (v.m as usize).div_ceil(64)];
        let mut prev: Option<(u32, u32)> = None;
        for i in 0..v.m as usize {
            let [l, r, res, rank] = v.merge_record(i);
            if l >= v.n || r >= v.n || res >= v.n {
                return Err(VocabBinError::MergeId);
            }
            if prev.is_some_and(|p| p >= (l, r)) {
                return Err(VocabBinError::MergeOrder);
            }
            prev = Some((l, r));
            if rank >= v.m || ranks[rank as usize / 64] & (1 << (rank % 64)) != 0 {
                return Err(VocabBinError::MergeRank);
            }
            ranks[rank as usize / 64] |= 1 << (rank % 64);
            let (lb, rb, xb) = (v.bytes_of(l), v.bytes_of(r), v.bytes_of(res));
            if xb.len() != lb.len() + rb.len() || &xb[..lb.len()] != lb || &xb[lb.len()..] != rb {
                return Err(VocabBinError::MergeBytes);
            }
        }

        for id in [v.bos, v.eos, v.pad] {
            if id != NO_ID && id >= v.n {
                return Err(VocabBinError::Special);
            }
        }
        Ok(v)
    }

    pub fn token_count(&self) -> u32 {
        self.n
    }

    pub fn merge_count(&self) -> u32 {
        self.m
    }

    pub fn bos(&self) -> Option<u32> {
        (self.bos != NO_ID).then_some(self.bos)
    }

    pub fn eos(&self) -> Option<u32> {
        (self.eos != NO_ID).then_some(self.eos)
    }

    pub fn pad(&self) -> Option<u32> {
        (self.pad != NO_ID).then_some(self.pad)
    }

    /// Token `id`'s bytes; `id` must be `< N`.
    fn bytes_of(&self, id: u32) -> &[u8] {
        let f = self.bytes.as_ref();
        let at = self.offsets_off + 4 * id as usize;
        let start = rd32(f, at) as usize;
        let end = rd32(f, at + 4) as usize;
        &f[self.blob_off + start..self.blob_off + end]
    }

    fn merge_record(&self, i: usize) -> [u32; 4] {
        let f = self.bytes.as_ref();
        let at = self.merges_off + MERGE_RECORD * i;
        [
            rd32(f, at),
            rd32(f, at + 4),
            rd32(f, at + 8),
            rd32(f, at + 12),
        ]
    }

    pub fn encode(&self, input: &str) -> Result<Vec<u32>, TokenizerError> {
        encode_with(self, input)
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String, TokenizerError> {
        decode_with(self, ids)
    }
}

impl<B: AsRef<[u8]>> BpeTables for VocabBin<B> {
    fn token_id(&self, token: &[u8]) -> Option<u32> {
        let f = self.bytes.as_ref();
        let (mut lo, mut hi) = (0usize, self.n as usize);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let id = rd32(f, self.sorted_off + 4 * mid);
            match self.bytes_of(id).cmp(token) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(id),
            }
        }
        None
    }

    fn token_bytes(&self, id: u32) -> Option<&[u8]> {
        (id < self.n).then(|| self.bytes_of(id))
    }

    fn merge_rank(&self, left: &[u8], right: &[u8]) -> Option<u32> {
        let key = (self.token_id(left)?, self.token_id(right)?);
        let (mut lo, mut hi) = (0usize, self.m as usize);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let [l, r, _, rank] = self.merge_record(mid);
            match (l, r).cmp(&key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(rank),
            }
        }
        None
    }
}
