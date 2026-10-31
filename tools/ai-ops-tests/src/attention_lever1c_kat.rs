// SPDX-License-Identifier: MPL-2.0
//! Lever 1c (Plans/NOTE-2026-10-26-arthur-lever1c-kv-in-place.md): the cached attention step reads
//! the KV cache in place (`Qwen2KvCacheLayer::attention_last`). It must give the same bits as the
//! path it replaced: copy the active prefix out (`active_key`/`active_value`), build the
//! `[q_heads, seq, head_dim]` query with only the last row set, and call
//! `ops::gqa_causal_attention_last` on byte views.

use super::forward_logic::{Qwen2KvCacheLayer, Qwen2LayerConfig};
use super::ops::{self, GqaAttentionParams, TensorView};
use super::tensor_logic::{Dtype, MAX_RANK, Shape};

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    fn f32s(&mut self, n: usize, scale: f32) -> Vec<f32> {
        (0..n)
            .map(|_| ((self.next() % 20_001) as f32 - 10_000.0) * (scale / 10_000.0))
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

fn bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The pre-1c path, as `gqa_attention_cached_decode` ran it.
fn copy_path(cache: &Qwen2KvCacheLayer, q_rope: &[f32], c: Qwen2LayerConfig) -> Vec<u8> {
    let seq = cache.used_len();
    let mut query = vec![0.0f32; c.q_heads * seq * c.head_dim];
    for head in 0..c.q_heads {
        for dim in 0..c.head_dim {
            query[(head * seq + seq - 1) * c.head_dim + dim] = q_rope[head * c.head_dim + dim];
        }
    }
    let (qb, kb, vb) = (
        bytes(&query),
        bytes(&cache.active_key().unwrap()),
        bytes(&cache.active_value().unwrap()),
    );
    let (qs, ks) = (
        shape3(c.q_heads, seq, c.head_dim),
        shape3(c.kv_heads, seq, c.head_dim),
    );
    let view = |shape, b| TensorView {
        dtype: Dtype::F32,
        shape,
        bytes: b,
    };
    let params = GqaAttentionParams {
        seq_len: seq as u64,
        n_q_heads: c.q_heads as u64,
        n_kv_heads: c.kv_heads as u64,
        head_dim: c.head_dim as u64,
    };
    let mut out = vec![0u8; c.q_heads * c.head_dim * 4];
    ops::gqa_causal_attention_last(
        &view(&qs, &qb),
        &view(&ks, &kb),
        &view(&ks, &vb),
        params,
        &mut out,
    )
    .unwrap();
    out
}

fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    (0..a.len() / 4).find(|&i| a[i * 4..i * 4 + 4] != b[i * 4..i * 4 + 4])
}

/// 0.5B shapes, contexts from 1 to the 2,304-token cap, three value scales; the cache sized past
/// the context and every unused position filled with a large value, so a read past `used_len`
/// changes the result.
#[test]
fn attention_last_in_place_bytes_identical_to_copy_path() {
    let c = Qwen2LayerConfig::qwen25_05b();
    let mut r = Xorshift(0x1c1c_2d2d_3e3e_4f4f);
    for (seq, max_context) in [
        (1usize, 4usize),
        (2, 2),
        (17, 64),
        (255, 300),
        (1024, 1030),
        (2304, 2304),
    ] {
        for scale in [0.5f32, 3.0, 40.0] {
            let mut cache = Qwen2KvCacheLayer::new(c, max_context).unwrap();
            for pos in 0..seq {
                let k = r.f32s(c.kv_len(), scale);
                let v = r.f32s(c.kv_len(), scale);
                cache.append_rope_kv(pos as u64, &k, &v).unwrap();
            }
            cache.fill_unused_for_test(1.0e4);
            let q = r.f32s(c.q_len(), scale);
            let new = bytes(&cache.attention_last(&q).unwrap());
            let old = copy_path(&cache, &q, c);
            if let Some(i) = first_difference(&new, &old) {
                panic!("seq {seq} max_context {max_context} scale {scale}: element {i} differs");
            }
        }
    }
}

/// Falsification of the KAT itself: the same strided call with the head stride one position
/// short (as an off-by-one in `max_context * head_dim` would give) must not match.
#[test]
fn attention_last_wrong_head_stride_is_caught() {
    let c = Qwen2LayerConfig::qwen25_05b();
    let mut r = Xorshift(7);
    let (seq, max_context) = (17usize, 64usize);
    let mut cache = Qwen2KvCacheLayer::new(c, max_context).unwrap();
    let mut keys = Vec::new();
    let mut values = Vec::new();
    for pos in 0..seq {
        let k = r.f32s(c.kv_len(), 3.0);
        let v = r.f32s(c.kv_len(), 3.0);
        cache.append_rope_kv(pos as u64, &k, &v).unwrap();
        keys.push(k);
        values.push(v);
    }
    let q = r.f32s(c.q_len(), 3.0);
    let good = bytes(&cache.attention_last(&q).unwrap());
    assert_eq!(first_difference(&good, &copy_path(&cache, &q, c)), None);

    // Rebuild the cache's storage layout by hand and call the op with a stride one position short.
    let stride = max_context * c.head_dim;
    let mut key = vec![0.0f32; c.kv_heads * stride];
    let mut value = vec![0.0f32; c.kv_heads * stride];
    for (pos, (k, v)) in keys.iter().zip(&values).enumerate() {
        for head in 0..c.kv_heads {
            for dim in 0..c.head_dim {
                key[head * stride + pos * c.head_dim + dim] = k[head * c.head_dim + dim];
                value[head * stride + pos * c.head_dim + dim] = v[head * c.head_dim + dim];
            }
        }
    }
    let params = GqaAttentionParams {
        seq_len: seq as u64,
        n_q_heads: c.q_heads as u64,
        n_kv_heads: c.kv_heads as u64,
        head_dim: c.head_dim as u64,
    };
    let mut right = vec![0.0f32; c.q_len()];
    ops::gqa_causal_attention_last_strided(&q, &key, &value, stride, params, &mut right).unwrap();
    assert_eq!(
        bytes(&right),
        good,
        "hand-built layout must match the cache's"
    );
    let mut wrong = vec![0.0f32; c.q_len()];
    ops::gqa_causal_attention_last_strided(
        &q,
        &key,
        &value,
        stride - c.head_dim,
        params,
        &mut wrong,
    )
    .unwrap();
    assert_ne!(
        bytes(&wrong),
        good,
        "a stride off by one position went unnoticed"
    );
}

/// Bounds: a key or value slice too short for the last KV head fails with `Input`, a query of the
/// wrong length or a context longer than the stride with `Shape`.
#[test]
fn attention_last_strided_rejects_short_or_inconsistent_inputs() {
    let params = GqaAttentionParams {
        seq_len: 3,
        n_q_heads: 4,
        n_kv_heads: 2,
        head_dim: 2,
    };
    let q = vec![0.5f32; 8];
    let stride = 4 * 2;
    let full = vec![0.25f32; 2 * stride];
    let mut out = vec![0.0f32; 8];
    let call = |q: &[f32], k: &[f32], v: &[f32], stride, out: &mut [f32]| {
        ops::gqa_causal_attention_last_strided(q, k, v, stride, params, out)
    };
    assert!(call(&q, &full, &full, stride, &mut out).is_ok());
    let short = &full[..stride + 3 * 2 - 1];
    assert_eq!(
        call(&q, short, &full, stride, &mut out),
        Err(ops::OpError::Input)
    );
    assert_eq!(
        call(&q, &full, short, stride, &mut out),
        Err(ops::OpError::Input)
    );
    assert_eq!(
        call(&q[..7], &full, &full, stride, &mut out),
        Err(ops::OpError::Shape)
    );
    assert_eq!(
        call(&q, &full, &full, 5, &mut out),
        Err(ops::OpError::Shape)
    );
}
