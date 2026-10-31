// SPDX-License-Identifier: MPL-2.0
//! Lever 2 (Plans/NOTE-2026-10-25-arthur-lever2-design.md): the cached decode step's
//! attention split by q-head range. `ops::gqa_causal_attention_last_strided_heads` over
//! every `column_partition` split of the heads, each range with its own reused scratch,
//! gives the whole `gqa_causal_attention_last_strided` call's bits. The cross-core run is
//! checked in QEMU (`threaded_matmul_parity`).

use super::forward_logic::column_partition;
use super::ops::{self, GqaAttentionParams, OpError};

const NQ: usize = 14;
const NKV: usize = 2;
const HD: usize = 64;

struct Case {
    q: Vec<f32>,
    key: Vec<f32>,
    value: Vec<f32>,
    stride: usize,
    params: GqaAttentionParams,
}

fn case(seq: usize, max_context: usize, scale: f32, seed: u64) -> Case {
    let mut x = seed;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        ((x >> 20) % 20_001) as f32 / 10_000.0 - 1.0
    };
    let stride = max_context * HD;
    // Positions past `seq` hold a large value, so a read past the active prefix shows.
    let mut key = vec![1.0e4; NKV * stride];
    let mut value = vec![1.0e4; NKV * stride];
    for h in 0..NKV {
        for i in 0..seq * HD {
            key[h * stride + i] = next() * scale;
            value[h * stride + i] = next() * scale;
        }
    }
    let q = (0..NQ * HD).map(|_| next() * scale).collect();
    let params = GqaAttentionParams {
        seq_len: seq as u64,
        n_q_heads: NQ as u64,
        n_kv_heads: NKV as u64,
        head_dim: HD as u64,
    };
    Case {
        q,
        key,
        value,
        stride,
        params,
    }
}

fn whole(c: &Case) -> Vec<u32> {
    let mut out = vec![0.0f32; NQ * HD];
    ops::gqa_causal_attention_last_strided(&c.q, &c.key, &c.value, c.stride, c.params, &mut out)
        .unwrap();
    out.iter().map(|v| v.to_bits()).collect()
}

/// `parts` ranges, each into its own output rows; `out_shift` moves every range's output
/// window by that many heads (wrapping), the misplacement a wrong stride would make.
fn split(c: &Case, parts: usize, out_shift: usize) -> Vec<u32> {
    let seq = c.params.seq_len as usize;
    let mut out = vec![0.0f32; NQ * HD];
    // One scratch reused across ranges and pre-filled with garbage: a range must not
    // depend on what an earlier one left there.
    let mut weights = vec![0x7fc0_0001u32; seq + 3];
    for t in 0..parts {
        let (start, end) = column_partition(NQ, parts, t);
        let mut rows = vec![0.0f32; (end - start) * HD];
        ops::gqa_causal_attention_last_strided_heads(
            &c.q,
            &c.key,
            &c.value,
            c.stride,
            c.params,
            start..end,
            &mut weights,
            &mut rows,
        )
        .unwrap();
        for (i, v) in rows.into_iter().enumerate() {
            out[(start * HD + i + out_shift * HD) % (NQ * HD)] = v;
        }
    }
    out.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn every_head_split_matches_the_whole_call() {
    for (seq, max_context) in [(1usize, 4usize), (17, 64), (255, 300), (2304, 2304)] {
        for (scale, seed) in [(0.5f32, 3u64), (6.0, 11)] {
            let c = case(seq, max_context, scale, seed);
            let truth = whole(&c);
            for parts in 1..=NQ {
                assert_eq!(
                    split(&c, parts, 0),
                    truth,
                    "seq {seq} scale {scale}: {parts}-way split differs"
                );
            }
        }
    }
}

/// Falsification: ranges written one head off their place must not match.
#[test]
fn a_misplaced_head_range_is_caught() {
    let c = case(17, 64, 3.0, 5);
    assert_ne!(split(&c, 4, 1), whole(&c));
}

#[test]
fn heads_entry_rejects_bad_ranges_and_short_scratch() {
    let c = case(5, 8, 1.0, 9);
    let mut w = vec![0u32; 5];
    let call = |heads: core::ops::Range<usize>, w: &mut [u32], out: &mut [f32]| {
        ops::gqa_causal_attention_last_strided_heads(
            &c.q, &c.key, &c.value, c.stride, c.params, heads, w, out,
        )
    };
    assert!(call(2..5, &mut w, &mut vec![0.0; 3 * HD]).is_ok());
    assert_eq!(
        call(2..5, &mut w, &mut vec![0.0; 2 * HD]),
        Err(OpError::Shape)
    );
    assert_eq!(
        call(12..15, &mut w, &mut vec![0.0; 3 * HD]),
        Err(OpError::Shape)
    );
    #[allow(clippy::reversed_empty_ranges)]
    let reversed = 5..2;
    assert_eq!(
        call(reversed, &mut w, &mut vec![0.0; 0]),
        Err(OpError::Shape)
    );
    assert_eq!(
        call(2..5, &mut w[..4], &mut vec![0.0; 3 * HD]),
        Err(OpError::Shape)
    );
}
