// SPDX-License-Identifier: MPL-2.0
// Host timing of one full production Qwen2 layer + lm_head (forward_logic.rs),
// Qwen2.5-0.5B shapes, synthetic quantized weights (timing is value-independent).
#![allow(dead_code, unused_imports, unexpected_cfgs)]
extern crate alloc;
#[path = "../../../../kernel/src/ai/tensor_logic.rs"] mod tensor_logic;
#[path = "../../../../kernel/src/ai/ops/mod.rs"]
mod ops;
#[path = "../../../../kernel/src/ai/gguf_logic.rs"] mod gguf_logic;
#[path = "../../../../kernel/src/ai/kv_layout.rs"] mod kv_layout;
#[path = "../../../../kernel/src/ai/model_logic.rs"] mod model_logic;
#[path = "../../../../kernel/src/ai/forward_logic.rs"] mod forward_logic;
use forward_logic::*;
use std::time::Instant;

fn rnd(n: usize, s: &mut u64) -> Vec<u8> { (0..n).map(|_| { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s as u8 }).collect() }
fn q(dt: TensorDtype, shared: usize, cols: usize, s: &mut u64) -> Vec<u8> {
    let (blk, vals, f16s): (usize, usize, &[usize]) = match dt {
        TensorDtype::Q4K => (144, 256, &[0, 2]), TensorDtype::Q6K => (210, 256, &[208]),
        TensorDtype::Q5_0 => (22, 32, &[0]), TensorDtype::Q8_0 => (34, 32, &[0]), _ => unreachable!() };
    let mut w = rnd(cols * shared / vals * blk, s);
    for b in w.chunks_mut(blk) { for &o in f16s { b[o] = 0; b[o + 1] = 0x20; } }
    // keep Q4K/Q6K sub-scales small too
    w
}
fn main() {
    // `fixed`: the boot-time AVX2 decision (NOTE fix 2), as the kernel now does.
    let fixed = std::env::args().any(|a| a == "fixed");
    if fixed { ops::qk_matmul::init_avx2_decision(); }
    let c = Qwen2LayerConfig::qwen25_05b();
    let (h, kv, ff, vocab) = (896usize, 128usize, 4864usize, 151936usize);
    let mut s = 0x1234_5678_9abc_def0u64;
    let down_dt = if std::env::args().any(|a| a == "q6k") { TensorDtype::Q6K } else { TensorDtype::Q4K };
    let dts = Qwen2LayerDtypes { attn_q: TensorDtype::Q5_0, attn_k: TensorDtype::Q5_0, attn_v: TensorDtype::Q8_0,
        attn_o: TensorDtype::Q5_0, ffn_gate: TensorDtype::Q5_0, ffn_up: TensorDtype::Q5_0, ffn_down: down_dt };
    let wq = q(dts.attn_q, h, h, &mut s); let wk = q(dts.attn_k, h, kv, &mut s); let wv = q(dts.attn_v, h, kv, &mut s);
    let wo = q(dts.attn_o, h, h, &mut s); let wg = q(dts.ffn_gate, h, ff, &mut s); let wu = q(dts.ffn_up, h, ff, &mut s);
    let wd = q(dts.ffn_down, ff, h, &mut s); let wlm = q(TensorDtype::Q8_0, h, vocab, &mut s);
    let norm = vec![1.0f32; h]; let bq = vec![0.01f32; h]; let bkv = vec![0.01f32; kv];
    let weights = Qwen2LayerWeights { attn_norm: &norm, attn_q: &[], attn_k: &[], attn_v: &[],
        attn_q_bias: Some(&bq), attn_k_bias: Some(&bkv), attn_v_bias: Some(&bkv), attn_o: &[], ffn_norm: &norm,
        ffn_gate: &[], ffn_up: &[], ffn_down: &[], dtypes: dts,
        quantized: Qwen2QuantizedLayerWeights { attn_q: Some(&wq), attn_k: Some(&wk), attn_v: Some(&wv), attn_o: Some(&wo),
            ffn_gate: Some(&wg), ffn_up: Some(&wu), ffn_down: Some(&wd) } };
    let positions: usize = std::env::var("POS").ok().and_then(|v| v.parse().ok()).unwrap_or(25);
    let x: Vec<f32> = (0..h).map(|i| ((i % 13) as f32 - 6.0) * 0.05).collect();
    let n = QwenAttentionNormWeights::NONE;
    let reps = 20;
    let time = |f: &mut dyn FnMut()| { let t = Instant::now(); for _ in 0..reps { f(); } t.elapsed().as_secs_f64() / reps as f64 * 1e3 };
    // Whole layer over positions 0..POS, with a KV cache.
    let mut cache = Qwen2KvCacheLayer::new(c, positions.max(64)).unwrap();
    let t = Instant::now();
    for p in 0..positions {
        std::hint::black_box(forward_layer_with_gqa_attention_with_norms(&x, weights, c, p as u64, true, Some(&mut cache), None, n).unwrap());
    }
    let layer = t.elapsed().as_secs_f64() / positions as f64 * 1e3;
    // Pieces, at the last position.
    let pos = (positions - 1) as u64;
    let seam = time(&mut || { std::hint::black_box(attention_seam_with_norms(&x, weights, c, pos, n).unwrap()); });
    let ffn = time(&mut || { std::hint::black_box(swiglu_ffn(&x, weights, c).unwrap()); });
    let fo = swiglu_ffn(&x, weights, c).unwrap();
    let sw = time(&mut || { 
        let g: Vec<u8> = fo.gate.iter().flat_map(|v| v.to_le_bytes()).collect();
        let u: Vec<u8> = fo.up.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut dims = [0u64; tensor_logic::MAX_RANK];
        dims[0] = fo.gate.len() as u64;
        let sh = tensor_logic::Shape::new(1, dims).unwrap();
        fn v<'a>(sh: &'a tensor_logic::Shape, b: &'a [u8]) -> ops::TensorView<'a> { ops::TensorView { dtype: tensor_logic::Dtype::F32, shape: sh, bytes: b } }
        let mut o = vec![0u8; g.len()];
        ops::swiglu(&v(&sh, &g), &v(&sh, &u), &mut o).unwrap();
        std::hint::black_box(o); });
    let norm = time(&mut || { std::hint::black_box(rms_norm_vec(&x, &norm, c.rms_norm_eps_bits).unwrap()); });
    let add = time(&mut || { std::hint::black_box(add_vec(&x, &x).unwrap()); });
    let att = attention_seam_with_norms(&x, weights, c, pos, n).unwrap();
    let mut cache = Qwen2KvCacheLayer::new(c, positions.max(64)).unwrap();
    for p in 0..pos {
        forward_layer_with_gqa_attention_with_norms(&x, weights, c, p, true, Some(&mut cache), None, n).unwrap();
    }
    let core = time(&mut || {
        let mut cc = cache.clone();
        std::hint::black_box(gqa_attention_cached_decode(&att, &mut cc, c, pos).unwrap());
    });
    let clone = time(&mut || { std::hint::black_box(cache.clone()); });
    println!("POS={} swiglu_vec={:.3} ms (soft-f32 elementwise inside swiglu_ffn)", positions, sw);
    // The ops-level kernel alone, on prebuilt [heads, seq, dim] bytes at seq = POS.
    {
        let seq = positions;
        let mut s3 = |a: usize, b: usize| { let mut d = [0u64; tensor_logic::MAX_RANK]; d[0] = a as u64; d[1] = seq as u64; d[2] = b as u64; tensor_logic::Shape::new(3, d).unwrap() };
        let (qs, ks) = (s3(c.q_heads, c.head_dim), s3(c.kv_heads, c.head_dim));
        let mut seed = 0x51u64;
        let q = rnd(c.q_heads * seq * c.head_dim * 4, &mut seed).chunks(4).flat_map(|_| 0.01f32.to_le_bytes()).collect::<Vec<u8>>();
        let k: Vec<u8> = (0..c.kv_heads * seq * c.head_dim).flat_map(|i| (((i % 17) as f32 - 8.0) * 0.03).to_le_bytes()).collect();
        let v = k.clone();
        let params = ops::GqaAttentionParams { seq_len: seq as u64, n_q_heads: c.q_heads as u64, n_kv_heads: c.kv_heads as u64, head_dim: c.head_dim as u64 };
        let mut o = vec![0u8; c.q_heads * c.head_dim * 4];
        let kernel = time(&mut || {
            ops::gqa_causal_attention_last(&ops::TensorView { dtype: tensor_logic::Dtype::F32, shape: &qs, bytes: &q }, &ops::TensorView { dtype: tensor_logic::Dtype::F32, shape: &ks, bytes: &k }, &ops::TensorView { dtype: tensor_logic::Dtype::F32, shape: &ks, bytes: &v }, params, &mut o).unwrap();
            std::hint::black_box(&o);
        });
        println!("POS={} gqa_causal_attention_last alone={:.3} ms", positions, kernel);
    }
    println!("POS={} layer={:.3} ms | attention_seam={:.3} swiglu_ffn={:.3} attn_core(cached, last pos)={:.3} (cache clone {:.3}) | rms_norm={:.4} add={:.4}",
        positions, layer, seam, ffn, core - clone, clone, norm, add);
}
