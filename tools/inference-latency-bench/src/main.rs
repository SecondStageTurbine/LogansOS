// SPDX-License-Identifier: MPL-2.0
// Host timing of the production quantized matvec kernels at Qwen2.5-0.5B shapes.
// Synthetic weight bytes (timing is value-independent); scale fields pinned finite.
#![allow(dead_code, unused_imports)]
extern crate alloc;
#[path = "../../../kernel/src/ai/tensor_logic.rs"]
mod tensor_logic;
#[path = "../../../kernel/src/ai/ops/mod.rs"]
mod ops;
use ops::qk_matmul::{self as qk, BlockQ8K, BlockQ8Zero, QKMatmulKernel};
use std::time::Instant;

fn rnd(n: usize, seed: &mut u64) -> Vec<u8> {
    (0..n).map(|_| { *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17; *seed as u8 }).collect()
}
fn pin_f16(buf: &mut [u8], block: usize, offs: &[usize]) {
    for b in buf.chunks_mut(block) { for &o in offs { b[o] = 0x00; b[o + 1] = 0x30; } } // 0x3000 = 0.125
}
#[derive(Clone, Copy, Debug)]
enum K { Q4K, Q6K, Q5_0, Q8_0 }
fn weights(k: K, shared: usize, cols: usize, s: &mut u64) -> Vec<u8> {
    match k {
        K::Q4K => { let mut w = rnd(cols * shared / 256 * 144, s); pin_f16(&mut w, 144, &[0, 2]); w }
        K::Q6K => { let mut w = rnd(cols * shared / 256 * 210, s); pin_f16(&mut w, 210, &[208]); w }
        K::Q5_0 => { let mut w = rnd(cols * shared / 32 * 22, s); pin_f16(&mut w, 22, &[0]); w }
        K::Q8_0 => { let mut w = rnd(cols * shared / 32 * 34, s); pin_f16(&mut w, 34, &[0]); w }
    }
}
fn time_matvec(k: K, shared: usize, cols: usize, kern: QKMatmulKernel, reps: usize) -> f64 {
    let mut s = 0x9e3779b97f4a7c15u64;
    let w = weights(k, shared, cols, &mut s);
    let x: Vec<f32> = (0..shared).map(|i| ((i % 17) as f32 - 8.0) * 0.01).collect();
    let mut out = vec![0f32; cols];
    let t = Instant::now();
    for _ in 0..reps {
        match k {
            K::Q4K | K::Q6K => {
                let mut q = vec![BlockQ8K::ZERO; shared / 256];
                qk::quantize_row_q8_k_with_kernel(&x, &mut q, kern).unwrap();
                if let K::Q4K = k { qk::matmul_q4_k_q8_k_with_kernel(&q, &w, 1, cols, shared, &mut out, kern).unwrap(); }
                else { qk::matmul_q6_k_q8_k_with_kernel(&q, &w, 1, cols, shared, &mut out, kern).unwrap(); }
            }
            K::Q5_0 | K::Q8_0 => {
                let mut q = vec![BlockQ8Zero::ZERO; shared / 32];
                qk::quantize_row_q8_0_with_kernel(&x, &mut q, kern).unwrap();
                if let K::Q5_0 = k { qk::matmul_q5_0_q8_0_with_kernel(&q, &w, 1, cols, shared, &mut out, kern).unwrap(); }
                else { qk::matmul_q8_0_q8_0_with_kernel(&q, &w, 1, cols, shared, &mut out, kern).unwrap(); }
            }
        }
    }
    std::hint::black_box(&out);
    t.elapsed().as_secs_f64() / reps as f64
}
fn main() {
    let t = Instant::now(); let mut acc = 0u32; for i in 0..20000u32 { acc ^= unsafe { core::arch::x86_64::__cpuid(1 + (i & 0)) }.eax; } std::hint::black_box(acc);
    println!("raw CPUID cost here: {:.2} us/call", t.elapsed().as_secs_f64() / 20000.0 * 1e6);
    let kern = if std::env::args().any(|a| a == "scalar") { QKMatmulKernel::Scalar } else { QKMatmulKernel::AutoAvx2 };
    // `fixed`: the boot-time AVX2 decision (NOTE fix 2), as the kernel now does.
    if std::env::args().any(|a| a == "fixed") { qk::init_avx2_decision(); }
    println!("avx2_available={} kernel={:?} decision_cached={}", qk::q4_k_q8_k_avx2_available(), kern, qk::avx2_decision_cached());
    // (name, kind, shared(in), cols(out), per-token count)
    // Assumed Q4_K_M dtype mix for Qwen2.5-0.5B (in-dim 896 cannot use K-quants):
    let h = 896; let kv = 128; let ff = 4864; let vocab = 151936;
    let shapes: &[(&str, K, usize, usize, usize)] = &[
        ("attn_q  Q5_0 896x896", K::Q5_0, h, h, 24),
        ("attn_k  Q5_0 896x128", K::Q5_0, h, kv, 24),
        ("attn_v  Q8_0 896x128", K::Q8_0, h, kv, 24),
        ("attn_o  Q5_0 896x896", K::Q5_0, h, h, 24),
        ("ffn_gate Q5_0 896x4864", K::Q5_0, h, ff, 24),
        ("ffn_up  Q5_0 896x4864", K::Q5_0, h, ff, 24),
        ("ffn_down Q4K 4864x896 (12 layers)", K::Q4K, ff, h, 12),
        ("ffn_down Q6K 4864x896 (12 layers)", K::Q6K, ff, h, 12),
        ("lm_head Q8_0 896x151936", K::Q8_0, h, vocab, 1),
    ];
    let reps_scale = std::env::var("REPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1usize);
    let mut total = 0.0; let mut lm = 0.0;
    for &(name, k, sh, cols, n) in shapes {
        let reps = if cols > 100_000 { reps_scale } else { 3 * reps_scale };
        let t = time_matvec(k, sh, cols, kern, reps);
        let contrib = t * n as f64;
        if cols == vocab { lm = contrib; }
        total += contrib;
        println!("{name:38} {:10.3} ms/call  x{n:2} = {:10.1} ms/token", t * 1e3, contrib * 1e3);
    }
    println!("TOTAL matvec per token (1 thread): {:.1} ms  (lm_head share {:.0}%)", total * 1e3, lm / total * 100.0);
}
