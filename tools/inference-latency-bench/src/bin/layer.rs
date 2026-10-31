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
    let positions: usize = std::env::var("POS").ok().and_then(|v| v.parse().ok()).unwrap_or(32);
    let mut cache = Qwen2KvCacheLayer::new(c, 64).unwrap();
    let x: Vec<f32> = (0..h).map(|i| ((i % 13) as f32 - 6.0) * 0.05).collect();
    let mut layer_times = Vec::new();
    for p in 0..positions {
        let t = Instant::now();
        let out = forward_layer_with_gqa_attention_with_norms(&x, weights, c, p as u64, true, Some(&mut cache), None, QwenAttentionNormWeights::NONE).unwrap();
        layer_times.push(t.elapsed().as_secs_f64());
        std::hint::black_box(&out);
    }
    let avg = layer_times.iter().sum::<f64>() / positions as f64;
    let t = Instant::now();
    let logits = qwen2_output_logits(&x, Qwen2OutputWeights { output_norm: &norm, output: &[], output_dtype: TensorDtype::Q8_0,
        output_quantized: Some(&wlm), output_bias: None }, c, vocab).unwrap();
    let lm = t.elapsed().as_secs_f64();
    std::hint::black_box(&logits);
    println!("ffn_down={:?}  layer avg over positions 0..{}: {:.2} ms (first {:.2}, last {:.2})", down_dt, positions, avg*1e3, layer_times[0]*1e3, layer_times[positions-1]*1e3);
    println!("lm_head+norm: {:.1} ms", lm*1e3);
    let per_token = avg * 24.0 + lm;
    // A 28-token prompt (Arthur's chat framing + a short question): shipped
    // prefill computes the lm_head for every token; fixed (NOTE fix 3) for
    // the last only.
    let prompt = 28.0;
    let prefill = if fixed { avg * 24.0 * prompt + lm } else { per_token * prompt };
    println!("=> {}: per token (24 layers + lm_head, 1 thread) {:.0} ms; 28-token prefill {:.1} s",
        if fixed { "fixed" } else { "shipped" }, per_token * 1e3, prefill);
}
