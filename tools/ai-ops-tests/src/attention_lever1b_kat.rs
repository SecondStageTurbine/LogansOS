// SPDX-License-Identifier: MPL-2.0
//! Lever 1b (Plans/NOTE-2026-10-25-arthur-lever2-design.md): the rewritten
//! `gqa_causal_attention_last` (each score computed once, rows sliced once)
//! must give the same bytes as the body it replaced, kept as
//! `gqa_causal_attention_last_reference` (test only), on the same inputs.

use super::ops::{self, GqaAttentionParams, OpError, TensorView};
use super::tensor_logic::{Dtype, MAX_RANK, Shape};

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    fn bytes(&mut self, n: usize, scale: f32) -> Vec<u8> {
        (0..n)
            .flat_map(|_| {
                let v = ((self.next() % 20_001) as f32 - 10_000.0) * (scale / 10_000.0);
                v.to_le_bytes()
            })
            .collect()
    }
}

fn shape3(a: usize, b: usize, c: usize) -> Shape {
    let mut d = [0u64; MAX_RANK];
    d[0] = a as u64;
    d[1] = b as u64;
    d[2] = c as u64;
    Shape::new(3, d).unwrap()
}

fn view<'a>(shape: &'a Shape, bytes: &'a [u8]) -> TensorView<'a> {
    TensorView {
        dtype: Dtype::F32,
        shape,
        bytes,
    }
}

type Attn = fn(
    &TensorView<'_>,
    &TensorView<'_>,
    &TensorView<'_>,
    GqaAttentionParams,
    &mut [u8],
) -> Result<(), OpError>;

fn run(
    f: Attn,
    q: &[u8],
    k: &[u8],
    v: &[u8],
    (q_heads, kv_heads, seq, head_dim): (usize, usize, usize, usize),
) -> Result<Vec<u8>, OpError> {
    let qs = shape3(q_heads, seq, head_dim);
    let ks = shape3(kv_heads, seq, head_dim);
    let params = GqaAttentionParams {
        seq_len: seq as u64,
        n_q_heads: q_heads as u64,
        n_kv_heads: kv_heads as u64,
        head_dim: head_dim as u64,
    };
    let mut out = vec![0u8; q_heads * head_dim * 4];
    f(
        &view(&qs, q),
        &view(&ks, k),
        &view(&ks, v),
        params,
        &mut out,
    )?;
    Ok(out)
}

/// Qwen2.5-0.5B: 14 query heads over 2 KV heads, head_dim 64. Positions from
/// the first token to the 2,304-token context cap; scales from activation
/// size to large enough that most softmax weights underflow to zero.
#[test]
fn attention_last_bytes_identical_to_reference_at_qwen_shapes() {
    let dims = |seq| (14usize, 2usize, seq, 64usize);
    let mut r = Xorshift(0x1b1b_2c2c_3d3d_4e4e);
    for seq in [1usize, 2, 3, 17, 64, 255, 1024, 2304] {
        for scale in [0.5f32, 3.0, 40.0] {
            let (qh, kvh, _, hd) = dims(seq);
            let q = r.bytes(qh * seq * hd, scale);
            let k = r.bytes(kvh * seq * hd, scale);
            let v = r.bytes(kvh * seq * hd, scale);
            let new = run(ops::gqa_causal_attention_last, &q, &k, &v, dims(seq)).unwrap();
            let old = run(
                ops::gqa_causal_attention_last_reference,
                &q,
                &k,
                &v,
                dims(seq),
            )
            .unwrap();
            if let Some(i) =
                (0..new.len() / 4).find(|&i| new[i * 4..i * 4 + 4] != old[i * 4..i * 4 + 4])
            {
                panic!(
                    "seq {seq} scale {scale}: element {i} differs: new {:02x?} reference {:02x?}",
                    &new[i * 4..i * 4 + 4],
                    &old[i * 4..i * 4 + 4]
                );
            }
        }
    }
}

/// Other GQA groupings (MHA, and 4 query heads per KV head), odd head_dim.
#[test]
fn attention_last_bytes_identical_to_reference_at_other_groupings() {
    let mut r = Xorshift(0x77);
    for dims in [
        (4usize, 4usize, 9usize, 8usize),
        (8, 2, 33, 5),
        (1, 1, 1, 1),
        (6, 3, 128, 16),
    ] {
        let (qh, kvh, seq, hd) = dims;
        let q = r.bytes(qh * seq * hd, 2.0);
        let k = r.bytes(kvh * seq * hd, 2.0);
        let v = r.bytes(kvh * seq * hd, 2.0);
        assert_eq!(
            run(ops::gqa_causal_attention_last, &q, &k, &v, dims),
            run(ops::gqa_causal_attention_last_reference, &q, &k, &v, dims),
            "{dims:?}"
        );
    }
}

/// A tensor one value short fails with the reference's error, for each input.
#[test]
fn attention_last_short_inputs_fail_like_the_reference() {
    let dims = (4usize, 2usize, 5usize, 8usize);
    let (qh, kvh, seq, hd) = dims;
    let mut r = Xorshift(5);
    let q = r.bytes(qh * seq * hd, 1.0);
    let k = r.bytes(kvh * seq * hd, 1.0);
    let v = r.bytes(kvh * seq * hd, 1.0);
    let short = |b: &[u8]| b[..b.len() - 4].to_vec();
    for (q, k, v) in [
        (short(&q), k.clone(), v.clone()),
        (q.clone(), short(&k), v.clone()),
        (q.clone(), k.clone(), short(&v)),
    ] {
        let new = run(ops::gqa_causal_attention_last, &q, &k, &v, dims);
        let old = run(ops::gqa_causal_attention_last_reference, &q, &k, &v, dims);
        assert!(new.is_err());
        assert_eq!(new.err(), old.err());
    }
}
