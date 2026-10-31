// SPDX-License-Identifier: MPL-2.0
//! Lever 1 (Plans/NOTE-2026-10-25-arthur-non-matvec-cost.md): `soft_f32` add/sub/mul run on SSE,
//! and must return the same bits as the integer path they replace. Every test here compares the
//! default (hardware) path against `with_integer_ops`, the integer path, in the same binary.
//! The integer path is the bit-exact CPU reference, so any mismatch fails the lever.

use super::forward_logic::{
    Qwen2AttentionSeam, Qwen2KvCacheLayer, Qwen2LayerConfig, gqa_attention_cached_decode,
};
use super::ops::{self, TensorView, soft_f32};
use super::tensor_logic::{Dtype, MAX_RANK, Shape};

fn integer<R>(f: impl FnOnce() -> R) -> R {
    soft_f32::with_integer_ops(f)
}

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    /// A normal value with |x| in [2^-7, 2^9): activation range.
    fn activation(&mut self) -> u32 {
        (self.next() & 0x807f_ffff) | ((120 + (self.next() % 16)) << 23)
    }

    fn f32s(&mut self, n: usize, scale: f32) -> Vec<f32> {
        (0..n)
            .map(|_| ((self.next() % 20_001) as f32 - 10_000.0) * (scale / 10_000.0))
            .collect()
    }
}

/// Zeros, subnormals, normal-range boundaries, max, inf and NaNs of both signs, plus five
/// mantissas at every exponent.
fn edge_values() -> Vec<u32> {
    let mut v: Vec<u32> = vec![
        0,
        1,
        2,
        3,
        0x007f_fffe,
        0x007f_ffff,
        0x0080_0000,
        0x0080_0001,
        0x00ff_ffff,
        0x0100_0000,
        0x0d00_0000,
        0x2000_0000,
        0x3300_0000,
        0x3400_0000,
        0x3f7f_ffff,
        0x3f80_0000,
        0x3f80_0001,
        0x4000_0000,
        0x4b00_0000,
        0x4b80_0000,
        0x6000_0000,
        0x7e80_0000,
        0x7f00_0000,
        0x7f7f_fffe,
        0x7f7f_ffff,
        0x7f80_0000,
        0x7f80_0001,
        0x7fc0_0000,
        0x7fff_ffff,
    ];
    let n = v.len();
    for i in 0..n {
        v.push(v[i] | 0x8000_0000);
    }
    for exp in 0..=255u32 {
        for mant in [0u32, 1, 0x40_0000, 0x7f_ffff, 0x55_5555] {
            v.push((exp << 23) | mant);
            v.push((exp << 23) | mant | 0x8000_0000);
        }
    }
    v
}

type BinOp = fn(u32, u32) -> u32;
const OPS: [(&str, BinOp); 3] = [
    ("add", soft_f32::add),
    ("sub", soft_f32::sub),
    ("mul", soft_f32::mul),
];

#[test]
fn hw_add_sub_mul_match_integer_on_edge_pairs() {
    let v = edge_values();
    for (name, op) in OPS {
        for &a in &v {
            for &b in &v {
                let hw = op(a, b);
                let int = integer(|| op(a, b));
                assert_eq!(
                    hw, int,
                    "{name}({a:#010x}, {b:#010x}): hw {hw:#010x} int {int:#010x}"
                );
            }
        }
    }
}

#[test]
fn hw_add_sub_mul_match_integer_on_uniform_and_activation_pairs() {
    let mut r = Xorshift(0x9e37_79b9_7f4a_7c15);
    for (name, op) in OPS {
        for i in 0..2_000_000u32 {
            let (a, b) = if i % 2 == 0 {
                (r.next(), r.next())
            } else {
                (r.activation(), r.activation())
            };
            let hw = op(a, b);
            let int = integer(|| op(a, b));
            assert_eq!(
                hw, int,
                "{name}({a:#010x}, {b:#010x}): hw {hw:#010x} int {int:#010x}"
            );
        }
    }
}

/// The largest |x| that reaches `exp`'s polynomial; every other input returns before any
/// arithmetic, through code both paths share.
const EXP_POLY_LIMIT: u32 = 0x42b1_7218;

fn exp_mismatches(start: u32, step: u32) -> (u64, Option<(u32, u32, u32)>) {
    let (mut checked, mut first) = (0u64, None);
    let mut a = start;
    while a <= EXP_POLY_LIMIT + 1 {
        for x in [a, a | 0x8000_0000] {
            checked += 1;
            let hw = soft_f32::exp(x);
            let int = integer(|| soft_f32::exp(x));
            if hw != int && first.is_none() {
                first = Some((x, hw, int));
            }
        }
        match a.checked_add(step) {
            Some(next) => a = next,
            None => break,
        }
    }
    (checked, first)
}

#[test]
fn hw_exp_matches_integer_sampled() {
    let (checked, first) = exp_mismatches(0, 1021);
    assert!(checked > 2_000_000);
    assert_eq!(first, None, "exp(x) differs: (x, hw, int)");
    for &x in &edge_values() {
        assert_eq!(
            soft_f32::exp(x),
            integer(|| soft_f32::exp(x)),
            "exp({x:#010x})"
        );
    }
}

/// Every input that reaches the polynomial (2,237,850,676 of them). Too slow for the CI debug
/// build; run it with
/// `cargo test --release -p ai-ops-tests hw_exp_matches_integer_exhaustive -- --ignored`.
#[test]
#[ignore]
fn hw_exp_matches_integer_exhaustive() {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as u32;
    let workers: Vec<_> = (0..threads)
        .map(|t| std::thread::spawn(move || exp_mismatches(t, threads)))
        .collect();
    let mut total = 0;
    for w in workers {
        let (checked, first) = w.join().unwrap();
        assert_eq!(first, None, "exp(x) differs: (x, hw, int)");
        total += checked;
    }
    assert_eq!(total, 2 * (u64::from(EXP_POLY_LIMIT) + 2));
}

fn shape(dims: &[u64]) -> Shape {
    let mut d = [0u64; MAX_RANK];
    d[..dims.len()].copy_from_slice(dims);
    Shape::new(dims.len() as u32, d).unwrap()
}

fn bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn view<'a>(shape: &'a Shape, bytes: &'a [u8]) -> TensorView<'a> {
    TensorView {
        dtype: Dtype::F32,
        shape,
        bytes,
    }
}

/// Runs `f` on both paths and requires identical output bytes.
fn same_bytes(name: &str, f: impl Fn() -> Vec<u8>) {
    let hw = f();
    let int = integer(&f);
    assert_eq!(hw.len(), int.len(), "{name}: length");
    if let Some(i) = (0..hw.len() / 4).find(|&i| hw[i * 4..i * 4 + 4] != int[i * 4..i * 4 + 4]) {
        panic!(
            "{name}: element {i} differs: hw {:02x?} int {:02x?}",
            &hw[i * 4..i * 4 + 4],
            &int[i * 4..i * 4 + 4]
        );
    }
}

#[test]
fn elementwise_and_norm_ops_bytes_identical_on_both_paths() {
    let mut r = Xorshift(0x2545_f491_4f6c_dd1d);
    // The Qwen2.5-0.5B FFN width, at activation scale and at a wide scale that reaches exp's
    // under- and overflow and subnormal SiLU outputs.
    for scale in [4.0f32, 120.0] {
        let gate = bytes(&r.f32s(4864, scale));
        let up = bytes(&r.f32s(4864, scale));
        let s = shape(&[4864]);
        let run = |op: fn(&TensorView<'_>, &mut [u8]) -> Result<(), ops::OpError>| {
            let mut out = vec![0u8; gate.len()];
            op(&view(&s, &gate), &mut out).unwrap();
            out
        };
        same_bytes("swiglu", || {
            let mut out = vec![0u8; gate.len()];
            ops::swiglu(&view(&s, &gate), &view(&s, &up), &mut out).unwrap();
            out
        });
        same_bytes("silu", || run(ops::silu));
        same_bytes("gelu", || run(ops::gelu));
        same_bytes("rms_norm", || run(ops::rms_norm));
        same_bytes("layer_norm", || run(ops::layer_norm));
        let rows = shape(&[19, 256]);
        same_bytes("softmax", || {
            let mut out = vec![0u8; 19 * 256 * 4];
            ops::softmax(&view(&rows, &gate[..19 * 256 * 4]), &mut out).unwrap();
            out
        });
    }
}

#[test]
fn attention_core_bytes_identical_on_both_paths() {
    let config = Qwen2LayerConfig::qwen25_05b();
    let q_len = config.q_heads * config.head_dim;
    let kv_len = config.kv_heads * config.head_dim;
    let mut r = Xorshift(0x1234_5678_9abc_def0);
    let seams: Vec<Qwen2AttentionSeam> = (0..12)
        .map(|_| Qwen2AttentionSeam {
            attn_norm: Vec::new(),
            q: Vec::new(),
            k: Vec::new(),
            v: r.f32s(kv_len, 2.0),
            gate: None,
            q_rope: r.f32s(q_len, 3.0),
            k_rope: r.f32s(kv_len, 3.0),
        })
        .collect();
    same_bytes("gqa_attention_cached_decode", || {
        let mut cache = Qwen2KvCacheLayer::new(config, 16).unwrap();
        let mut out = Vec::new();
        for (pos, seam) in seams.iter().enumerate() {
            out.extend(bytes(
                &gqa_attention_cached_decode(seam, &mut cache, config, pos as u64).unwrap(),
            ));
        }
        out
    });
}

/// A full production layer (Qwen2.5-0.5B shapes, the A6 quant mix, synthetic weights) over six
/// positions with a KV cache: every intermediate the forward returns is byte-identical on both paths.
#[cfg(feature = "phase9-real-quant")]
#[test]
fn qwen2_layer_forward_bytes_identical_on_both_paths() {
    use super::forward_logic::{
        Qwen2LayerDtypes, Qwen2LayerWeights, Qwen2QuantizedLayerWeights, QwenAttentionNormWeights,
        TensorDtype, forward_layer_with_gqa_attention_with_norms,
    };
    ops::qk_matmul::init_avx2_decision();
    let (h, kv, ff) = (896usize, 128usize, 4864usize);
    let config = Qwen2LayerConfig::qwen25_05b();
    let mut r = Xorshift(0x5eed_0f1e_7e11_0001);
    // Block bytes with every f16 scale pinned to 2^-7, so the weights stay finite.
    let mut quant = |dtype: TensorDtype, shared: usize, cols: usize| -> Vec<u8> {
        let (block, values, scales): (usize, usize, &[usize]) = match dtype {
            TensorDtype::Q4K => (144, 256, &[0, 2]),
            TensorDtype::Q5_0 => (22, 32, &[0]),
            TensorDtype::Q8_0 => (34, 32, &[0]),
            _ => unreachable!(),
        };
        let mut w: Vec<u8> = (0..cols * shared / values * block)
            .map(|_| r.next() as u8)
            .collect();
        for b in w.chunks_mut(block) {
            for &o in scales {
                b[o] = 0x00;
                b[o + 1] = 0x20;
            }
        }
        w
    };
    let dtypes = Qwen2LayerDtypes {
        attn_q: TensorDtype::Q5_0,
        attn_k: TensorDtype::Q5_0,
        attn_v: TensorDtype::Q8_0,
        attn_o: TensorDtype::Q5_0,
        ffn_gate: TensorDtype::Q5_0,
        ffn_up: TensorDtype::Q5_0,
        ffn_down: TensorDtype::Q4K,
    };
    let w = [
        quant(dtypes.attn_q, h, h),
        quant(dtypes.attn_k, h, kv),
        quant(dtypes.attn_v, h, kv),
        quant(dtypes.attn_o, h, h),
        quant(dtypes.ffn_gate, h, ff),
        quant(dtypes.ffn_up, h, ff),
        quant(dtypes.ffn_down, ff, h),
    ];
    let norm: Vec<f32> = (0..h).map(|i| 0.75 + (i % 7) as f32 * 0.1).collect();
    let q_bias = vec![0.01f32; h];
    let kv_bias = vec![0.02f32; kv];
    let weights = Qwen2LayerWeights {
        attn_norm: &norm,
        attn_q: &[],
        attn_k: &[],
        attn_v: &[],
        attn_q_bias: Some(&q_bias),
        attn_k_bias: Some(&kv_bias),
        attn_v_bias: Some(&kv_bias),
        attn_o: &[],
        ffn_norm: &norm,
        ffn_gate: &[],
        ffn_up: &[],
        ffn_down: &[],
        dtypes,
        quantized: Qwen2QuantizedLayerWeights {
            attn_q: Some(&w[0]),
            attn_k: Some(&w[1]),
            attn_v: Some(&w[2]),
            attn_o: Some(&w[3]),
            ffn_gate: Some(&w[4]),
            ffn_up: Some(&w[5]),
            ffn_down: Some(&w[6]),
        },
    };
    let inputs: Vec<Vec<f32>> = (0..6).map(|_| r.f32s(h, 1.5)).collect();
    same_bytes("forward_layer_with_gqa_attention_with_norms", || {
        let mut cache = Qwen2KvCacheLayer::new(config, 8).unwrap();
        let mut out = Vec::new();
        for (pos, x) in inputs.iter().enumerate() {
            let o = forward_layer_with_gqa_attention_with_norms(
                x,
                weights,
                config,
                pos as u64,
                true,
                Some(&mut cache),
                None,
                QwenAttentionNormWeights::NONE,
            )
            .unwrap();
            for part in [
                &o.attention.attn_norm,
                &o.attention.q_rope,
                &o.attention.k_rope,
                &o.attention.v,
                &o.ffn_input,
                &o.ffn.ffn_norm,
                &o.ffn.swiglu,
                &o.ffn.residual,
            ] {
                out.extend(bytes(part));
            }
        }
        out
    });
}
