// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 2: deterministic, corpus-seeded mutation fuzzing of the AML
//! interpreter (`acpi+fix1`, or the stock crate for comparison).
//!
//! Every case is a pure function of `(seed, index)`: a source chosen from the
//! pool, then one to four mutations, all driven by a xorshift generator seeded
//! from the pair. A failing case is reproduced with
//! `acpi-aml-fuzz case --seed S --index I` and needs nothing but the corpus.
//!
//! **The pool** is real AML at object size:
//! - every corpus DSDT/SSDT of at most `WHOLE_TABLE_MAX` bytes, whole;
//! - every `Method`, `Device`, `ThermalZone`, `Processor` or `PowerResource`
//!   of at most `FRAGMENT_MAX` bytes from every corpus table, cut out at its
//!   PkgLength; `Scope`s and larger objects are split into their contents;
//! - the hand-laid Fix 1 KAT programs (`tests/fix1_kats.rs`).
//!
//! Loading a 400 KB DSDT per case would cost far more than the parser paths a
//! mutation can reach in it.

use crate::corpus::{CorpusSet, Table};
use std::path::Path;

/// Whole tables up to this size go into the pool as they are.
pub const WHOLE_TABLE_MAX: usize = 16 * 1024;
/// Objects cut out of tables are kept up to this size.
pub const FRAGMENT_MAX: usize = 4 * 1024;
/// Methods evaluated per case.
pub const CALLS_PER_CASE: usize = 16;

/// xorshift64*, seeded through splitmix64 so nearby seeds diverge at once.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64, index: u64) -> Rng {
        let mut z = seed ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// Uniform in `0..n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// FNV-1a, to name an input in a report.
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// One pool source: where it came from, its definition block revision, and
/// its bytes (an AML TermList, no table header).
#[derive(Clone)]
pub struct Source {
    pub origin: String,
    pub revision: u8,
    pub aml: Vec<u8>,
}

/// Decode a PkgLength at `at`: (encoded byte count, value).
pub fn pkg_length(aml: &[u8], at: usize) -> Option<(usize, usize)> {
    let lead = *aml.get(at)?;
    let extra = (lead >> 6) as usize;
    if extra == 0 {
        return Some((1, (lead & 0x3f) as usize));
    }
    let mut v = (lead & 0x0f) as usize;
    for i in 0..extra {
        v |= (*aml.get(at + 1 + i)? as usize) << (4 + 8 * i);
    }
    Some((1 + extra, v))
}

/// Length of a NameString at `at`, if one starts there.
fn name_len(aml: &[u8], at: usize) -> Option<usize> {
    let mut i = at;
    while matches!(aml.get(i), Some(b'\\') | Some(b'^')) {
        i += 1;
    }
    let segs = match *aml.get(i)? {
        0x00 => return Some(i + 1 - at),
        0x2e => {
            i += 1;
            2
        }
        0x2f => {
            i += 2;
            *aml.get(i - 1)? as usize
        }
        _ => 1,
    };
    let end = i + 4 * segs;
    (end <= aml.len()).then_some(end - at)
}

/// Cut objects out of a TermList: one with a PkgLength is kept whole if small
/// (Methods and named scopes), and objects holding a TermList are split into
/// it. Bytes the scan does not recognise are stepped over one at a time.
pub fn split(aml: &[u8], revision: u8, origin: &str, out: &mut Vec<Source>) {
    let mut i = 0;
    while i < aml.len() {
        let (op_len, keep, body_has_name, extra_fixed) = match aml[i] {
            0x10 => (1, false, true, 0), // Scope: split its contents
            0x14 => (1, true, false, 0), // Method
            0x5b => match aml.get(i + 1) {
                Some(0x82) | Some(0x85) => (2, true, true, 0), // Device, ThermalZone
                Some(0x83) => (2, true, true, 6),              // Processor: id, PBLK addr, len
                Some(0x84) => (2, true, true, 3),              // PowerResource: level, order
                _ => {
                    i += 1;
                    continue;
                }
            },
            _ => {
                i += 1;
                continue;
            }
        };
        let Some((pl_bytes, pl)) = pkg_length(aml, i + op_len) else {
            break;
        };
        let end = i + op_len + pl;
        if pl < pl_bytes || end > aml.len() {
            i += 1;
            continue;
        }
        let whole = &aml[i..end];
        if keep && whole.len() <= FRAGMENT_MAX {
            out.push(Source {
                origin: format!("{origin}@{i:#x}"),
                revision,
                aml: whole.to_vec(),
            });
        } else if body_has_name {
            let body_start = i + op_len + pl_bytes;
            if let Some(n) = name_len(aml, body_start) {
                let inner = body_start + n + extra_fixed;
                if inner <= end {
                    split(
                        &aml[inner..end],
                        revision,
                        &format!("{origin}@{inner:#x}"),
                        out,
                    );
                }
            }
        }
        i = end;
    }
}

/// The hand-laid KAT programs, read from `tests/fix1_kats.rs` (each
/// `let aml = [ ... ];` array of `0x..` and `b'.'` tokens).
pub fn kat_programs(path: &Path) -> Vec<Source> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (n, block) in text.split("let aml = [").skip(1).enumerate() {
        let Some(body) = block.split("];").next() else {
            continue;
        };
        let mut bytes = Vec::new();
        for line in body.lines() {
            let code = line.split("//").next().unwrap_or("");
            let mut rest = code;
            while let Some(p) = rest.find(['0', 'b']) {
                let tail = &rest[p..];
                if let Some(h) = tail.strip_prefix("0x") {
                    let digits: String = h.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                    if let Ok(v) = u8::from_str_radix(&digits, 16) {
                        bytes.push(v);
                    }
                    rest = &h[digits.len()..];
                } else if let Some(c) = tail.strip_prefix("b'") {
                    let (ch, skip) = if c.starts_with("\\\\") {
                        (b'\\', 3)
                    } else {
                        (c.as_bytes()[0], 2)
                    };
                    bytes.push(ch);
                    rest = &c[skip..];
                } else {
                    rest = &tail[1..];
                }
            }
        }
        out.push(Source {
            origin: format!("fix1_kats.rs program {n}"),
            revision: 2,
            aml: bytes,
        });
    }
    out
}

/// The whole pool, in a fixed order.
pub fn pool(sets: &[CorpusSet], kats: &Path) -> Vec<Source> {
    let mut out = Vec::new();
    for set in sets {
        for t in set.dsdt().into_iter().chain(set.ssdts()) {
            let body = t.aml_body();
            let origin = format!("{}/{}", set.name, t.file);
            if body.len() <= WHOLE_TABLE_MAX {
                out.push(Source {
                    origin: origin.clone(),
                    revision: t.revision(),
                    aml: body.to_vec(),
                });
            }
            split(body, t.revision(), &origin, &mut out);
        }
    }
    out.extend(kat_programs(kats));
    out
}

/// Bytes a mutation likes to write: opcode prefixes with a PkgLength or a
/// NameString after them, Local/Arg ops, and the boundary values.
const INTERESTING: &[u8] = &[
    0x00, 0x01, 0xff, 0x0a, 0x0b, 0x0c, 0x0e, 0x10, 0x11, 0x12, 0x13, 0x14, 0x2e, 0x2f, 0x5b, 0x5c,
    0x5e, 0x60, 0x68, 0x70, 0x71, 0x72, 0x83, 0x86, 0x88, 0x8c, 0x93, 0xa0, 0xa1, 0xa2, 0xa4, 0x7f,
    0x80, 0x40, 0xc0,
];

/// Mutate `src` with 1 to 4 random edits; `pool` supplies splice material.
pub fn mutate(rng: &mut Rng, src: &[u8], pool: &[Source]) -> Vec<u8> {
    let mut v = src.to_vec();
    for _ in 0..1 + rng.below(4) {
        if v.is_empty() {
            v.push(INTERESTING[rng.below(INTERESTING.len())]);
            continue;
        }
        let at = rng.below(v.len());
        match rng.below(8) {
            0 => v[at] ^= 1 << rng.below(8),
            1 => v[at] = INTERESTING[rng.below(INTERESTING.len())],
            2 => v.truncate(at.max(1)),
            3 => {
                // A PkgLength after a PkgLength-carrying opcode.
                let ops = [0x10u8, 0x11, 0x12, 0x13, 0x14, 0xa0, 0xa1, 0xa2];
                if let Some(p) = (0..v.len())
                    .map(|k| (at + k) % v.len())
                    .find(|&p| ops.contains(&v[p]))
                    && p + 1 < v.len()
                {
                    v[p + 1] = match rng.below(3) {
                        0 => v[p + 1].wrapping_add(1 + rng.below(8) as u8),
                        1 => v[p + 1].wrapping_sub(1 + rng.below(8) as u8),
                        _ => rng.next_u64() as u8,
                    };
                }
            }
            4 => v.insert(at, INTERESTING[rng.below(INTERESTING.len())]),
            5 => {
                let n = (1 + rng.below(8)).min(v.len() - at);
                v.drain(at..at + n);
            }
            6 => {
                let other = &pool[rng.below(pool.len())].aml;
                if !other.is_empty() {
                    let from = rng.below(other.len());
                    let n = (1 + rng.below(64)).min(other.len() - from);
                    let cut = (1 + rng.below(64)).min(v.len() - at);
                    v.splice(at..at + cut, other[from..from + n].iter().copied());
                }
            }
            _ => {
                let n = (1 + rng.below(32)).min(v.len() - at);
                let chunk: Vec<u8> = v[at..at + n].to_vec();
                v.splice(at..at, chunk);
            }
        }
    }
    v
}

/// Case `index` of run `seed`: (source index, mutated bytes, revision).
pub fn case(seed: u64, index: u64, pool: &[Source]) -> (usize, Vec<u8>, u8) {
    let mut rng = Rng::new(seed, index);
    let s = rng.below(pool.len());
    let bytes = mutate(&mut rng, &pool[s].aml, pool);
    (s, bytes, pool[s].revision)
}

/// The FADT every case runs on (QEMU q35 SeaBIOS: small and complete).
pub fn facp(corpus: &Path) -> std::io::Result<Table> {
    let set = crate::corpus::load_set(&corpus.join("qemu-q35-seabios-smp1"))?;
    let t = set
        .find("FACP")
        .ok_or_else(|| std::io::Error::other("qemu-q35-seabios-smp1 has no FACP"))?;
    Ok(Table {
        file: t.file.clone(),
        signature: t.signature.clone(),
        bytes: t.bytes.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cases_are_a_pure_function_of_seed_and_index() {
        let pool = vec![Source {
            origin: "t".into(),
            revision: 2,
            aml: (0u8..200).collect(),
        }];
        assert_eq!(case(7, 3, &pool).1, case(7, 3, &pool).1);
        assert_ne!(case(7, 3, &pool).1, case(7, 4, &pool).1);
    }

    #[test]
    fn pkg_length_decodes_one_and_two_byte_forms() {
        assert_eq!(pkg_length(&[0x12], 0), Some((1, 0x12)));
        // 0x40 | 0x0: one extra byte, value = 0x0 | 0x2b << 4 = 0x2b0.
        assert_eq!(pkg_length(&[0x40, 0x2b], 0), Some((2, 0x2b0)));
        assert_eq!(pkg_length(&[0x40], 0), None);
    }

    #[test]
    fn split_keeps_a_small_method_and_enters_a_scope() {
        // Scope (\_SB) { Method (MAIN, 0) { Return (One) } }
        let method = [0x14, 0x08, b'M', b'A', b'I', b'N', 0x00, 0xa4, 0x01];
        let mut scope = vec![0x10, 0x00, b'\\', b'_', b'S', b'B', b'_'];
        scope.extend_from_slice(&method);
        scope[1] = (scope.len() - 1) as u8;
        let mut out = Vec::new();
        split(&scope, 2, "t", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].aml, method);
    }
}
