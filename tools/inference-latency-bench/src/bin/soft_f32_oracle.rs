// SPDX-License-Identifier: MPL-2.0
// Byte-oracle: kernel soft_f32 vs hardware SSE f32 (IEEE-754, round-to-nearest-even),
// plus per-op cost. Answers whether hardware ops can replace soft ops bit-for-bit
// (Plans/NOTE-2026-10-25-arthur-non-matvec-cost.md).
// This is the pre-lever measurement: build it with `--features soft-f32-only` so `soft::` is the
// integer path. Since lever 1, the authoritative oracle is tools/ai-ops-tests `soft_f32_hw_kat`,
// which compares both paths in one binary.
#![allow(dead_code)]
#[path = "../../../../kernel/src/ai/ops/soft_f32.rs"]
mod soft;
use std::hint::black_box as bb;
use std::time::Instant;

struct R(u64);
impl R {
    fn n(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    // Normal-range value, |x| in [2^-7, 2^9): typical activations.
    fn act(&mut self) -> u32 {
        (self.n() & 0x807f_ffff) | ((120 + (self.n() % 16)) << 23)
    }
}
fn canon(x: u32) -> u32 { if f32::from_bits(x).is_nan() { 0x7fc0_0000 } else { x } }
fn is_sub_or_zero(x: u32) -> bool { x & 0x7f80_0000 == 0 }

// soft::exp's algorithm with hardware add/sub/mul; integer steps reused. Prototype for the
// byte-identity question only, not kernel code.
fn exp_hw(bits: u32) -> u32 {
    if f32::from_bits(bits).is_nan() { return 0x7fc0_0000; }
    if bits == 0 || bits == 0x8000_0000 { return 0x3f80_0000; }
    let x = f32::from_bits(bits);
    if x.is_infinite() { return if x < 0.0 { 0 } else { 0x7f80_0000 }; }
    let neg = bits & 0x8000_0000 != 0;
    if bits & 0x7fff_ffff > 0x42b1_7218 { return if neg { 0 } else { 0x7f80_0000 }; }
    let k = soft::round_f32_to_int((x * f32::from_bits(0x3fb8_aa3b)).to_bits());
    if k >= 128 { return if neg { 0 } else { 0x7f80_0000 }; }
    if k <= -128 { return if neg { 0x7f80_0000 } else { 0 }; }
    let r = if k == 0 { x } else { x - (k as f32) * f32::from_bits(0x3f31_7218) };
    let c = [0x3ab6_0b61u32, 0x3c08_8889, 0x3d2a_aaab, 0x3e2a_aaab, 0x3f00_0000, 0x3f80_0000, 0x3f80_0000];
    let mut acc = f32::from_bits(c[0]);
    for &ci in &c[1..] { acc = acc * r + f32::from_bits(ci); }
    let res = acc * f32::from_bits(soft::make_power_of_two(k));
    if res.is_nan() { 0x7fc0_0000 } else { res.to_bits() }
}

fn exhaustive_exp() {
    // Every input the polynomial path can see: |x| <= 0x42b1_7218, both signs, plus edges.
    let lim = 0x42b1_7218u32 + 2;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
    let hs: Vec<_> = (0..threads).map(|t| std::thread::spawn(move || {
        let (mut n, mut bad, mut first) = (0u64, 0u64, None);
        let mut a = t;
        while a < lim {
            for b in [a, a | 0x8000_0000] {
                n += 1;
                let (s, h) = (soft::exp(b), exp_hw(b));
                if s != h { bad += 1; if first.is_none() { first = Some((b, s, h)) } }
            }
            a += threads;
        }
        (n, bad, first)
    })).collect();
    let (mut n, mut bad) = (0u64, 0u64);
    for h in hs { let (a, b, f) = h.join().unwrap(); n += a; bad += b; if let Some(f) = f { println!("first mismatch {f:x?}") } }
    println!("exp exhaustive (|x| <= 88.72, both signs): n={n} mismatch={bad}");
}

fn main() {
    if std::env::args().any(|a| a == "exhaustive-exp") { exhaustive_exp(); return; }
    let mut r = R(0x9e37_79b9_7f4a_7c15);
    let n = 20_000_000u64;
    let ops: [(&str, fn(u32, u32) -> u32, fn(f32, f32) -> f32); 4] = [
        ("add", soft::add, |a, b| a + b),
        ("sub", soft::sub, |a, b| a - b),
        ("mul", soft::mul, |a, b| a * b),
        ("div", soft::div, |a, b| a / b),
    ];
    for (name, s, h) in ops {
        let (mut sub_res, mut other, mut max_ulp) = (0u64, 0u64, 0i64);
        for i in 0..n {
            let (a, b) = if i % 2 == 0 { (r.n(), r.n()) } else { (r.act(), r.act()) };
            let hw = canon(h(f32::from_bits(a), f32::from_bits(b)).to_bits());
            let sw = s(a, b);
            if hw != sw {
                if is_sub_or_zero(hw) || is_sub_or_zero(sw) { sub_res += 1 } else {
                    other += 1;
                    max_ulp = max_ulp.max((hw as i64 - sw as i64).abs());
                }
            }
        }
        println!("{name}: n={n} mismatch(subnormal/zero result)={sub_res} mismatch(other)={other} max_ulp(other)={max_ulp}");
    }
    let (mut bad, mut max_ulp) = (0u64, 0i64);
    for a in (0x0080_0000u32..0x7f80_0000).step_by(97) {
        let hw = f32::from_bits(a).sqrt().to_bits();
        let sw = soft::sqrt(a);
        if hw != sw { bad += 1; max_ulp = max_ulp.max((hw as i64 - sw as i64).abs()) }
    }
    println!("sqrt (normal inputs, stride 97): mismatch={bad} max_ulp={max_ulp}");

    let m = 4_000_000usize;
    let xs: Vec<u32> = (0..m).map(|i| (((i as f32) * 0.000_37).sin() * 6.0).to_bits()).collect();
    let time = |f: &dyn Fn(u32) -> u32| {
        let t = Instant::now();
        let mut s = 0u32;
        for &x in &xs { s ^= f(bb(x)); }
        bb(s);
        t.elapsed().as_nanos() as f64 / m as f64
    };
    let e = time(&|x| soft::exp(x ^ 0x8000_0000));
    let d = time(&|x| soft::div(x, 0x3fc0_0000));
    let a = time(&|x| soft::add(x, 0x3fc0_0000));
    let mu = time(&|x| soft::mul(x, 0x3fc0_0000));
    let silu = time(&|x| soft::mul(x, soft::div(x, soft::add(0x3f80_0000, soft::exp(x ^ 0x8000_0000)))));
    let eh = time(&|x| exp_hw(x ^ 0x8000_0000));
    println!("ns/op hw-op exp prototype: {eh:.1}");
    println!("ns/op soft: exp={e:.1} div={d:.1} add={a:.1} mul={mu:.1} swiglu_elem={silu:.1}");
}
