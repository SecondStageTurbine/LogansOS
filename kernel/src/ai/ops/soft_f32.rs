// SPDX-License-Identifier: MPL-2.0
//! Software IEEE-754 f32 helpers for kernel inference.

const SIGN_MASK: u32 = 0x8000_0000;
const EXP_MASK: u32 = 0x7f80_0000;
const FRAC_MASK: u32 = 0x007f_ffff;
const QUIET_NAN: u32 = 0x7fc0_0000;

#[derive(Copy, Clone)]
struct Finite {
    sign: bool,
    mant: u64,
    exp2: i32,
}

impl Finite {
    fn from_bits(bits: u32) -> Self {
        let sign = bits & SIGN_MASK != 0;
        let exp = ((bits & EXP_MASK) >> 23) as i32;
        let frac = (bits & FRAC_MASK) as u64;
        if exp == 0 {
            Self {
                sign,
                mant: frac,
                exp2: -149,
            }
        } else {
            Self {
                sign,
                mant: (1u64 << 23) | frac,
                exp2: exp - 127 - 23,
            }
        }
    }

    const fn is_zero(self) -> bool {
        self.mant == 0
    }
}

pub const fn zero() -> u32 {
    0
}

pub const fn relu(bits: u32) -> u32 {
    if bits & SIGN_MASK != 0 { 0 } else { bits }
}

// Hardware add/sub/mul (lever 1, Plans/NOTE-2026-10-25-arthur-non-matvec-cost.md).
//
// SSE `addss`/`mulss` under MXCSR 0x1F80 (round-to-nearest-even, no FTZ/DAZ) return the same bits as
// the integer `add_int`/`mul_int` below, with two adjustments that keep the outputs byte-identical:
// every NaN becomes `QUIET_NAN`, and when both addends are zero the result is `b` (`add_int` returns
// `-0` for `+0 + -0`, where IEEE gives `+0`). The ai-ops-tests oracles hold the identity: the edge-value
// sweep, the uniform sample, and `exp` over every input that reaches its polynomial.
// `soft-f32-only` compiles the integer path, bit-for-bit the previous layer.

// Host tests compare both paths in one binary; the kernel never builds `cfg(test)`.
#[cfg(test)]
std::thread_local! {
    static FORCE_INTEGER: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Run `f` with the integer add/sub/mul on this thread (test only).
#[cfg(test)]
pub fn with_integer_ops<R>(f: impl FnOnce() -> R) -> R {
    let prev = FORCE_INTEGER.with(|c| c.replace(true));
    let out = f();
    FORCE_INTEGER.with(|c| c.set(prev));
    out
}

#[inline(always)]
fn use_hardware() -> bool {
    #[cfg(test)]
    if FORCE_INTEGER.with(|c| c.get()) {
        return false;
    }
    !cfg!(feature = "soft-f32-only")
}

/// The identity with `add_int`/`mul_int` assumes the kernel MXCSR control bits (CP-41). Bits 0-5
/// are sticky exception flags that arithmetic sets, so they are not compared.
///
/// Called once at the entry of each op that loops over soft-f32 arithmetic (the `ops` element-wise,
/// norm, softmax, RoPE and attention kernels, and each forward layer), not per add/mul: the product
/// image keeps debug assertions on, and STMXCSR per element cost more than the hardware op it
/// guarded (Chad's review of #128). MXCSR only changes at kernel entry and exit, never inside an op.
#[inline]
pub fn debug_assert_kernel_mxcsr() {
    #[cfg(all(debug_assertions, target_arch = "x86_64"))]
    if use_hardware() {
        let mut csr: u32 = 0;
        // SAFETY: STMXCSR stores the 32-bit MXCSR to the given address; `csr` is a live local.
        unsafe {
            core::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
        }
        debug_assert_eq!(
            csr & 0xffc0,
            0x1f80,
            "soft_f32 hardware ops need MXCSR control 0x1F80"
        );
    }
}

/// Host tests also check on every add/mul (`cfg(test)` only).
#[inline(always)]
fn debug_assert_mxcsr() {
    #[cfg(test)]
    if PER_OP_MXCSR_CHECK.with(|c| c.get()) {
        debug_assert_kernel_mxcsr();
    }
}

#[cfg(test)]
std::thread_local! {
    static PER_OP_MXCSR_CHECK: core::cell::Cell<bool> = const { core::cell::Cell::new(true) };
}

/// Run `f` with only the entry-level MXCSR checks, as the kernel runs (test only): the per-op
/// check would otherwise hide an op whose entry check is missing.
#[cfg(test)]
pub fn with_entry_mxcsr_checks_only<R>(f: impl FnOnce() -> R) -> R {
    let prev = PER_OP_MXCSR_CHECK.with(|c| c.replace(false));
    let out = f();
    PER_OP_MXCSR_CHECK.with(|c| c.set(prev));
    out
}

#[inline(always)]
fn canonical(r: f32) -> u32 {
    if r.is_nan() { QUIET_NAN } else { r.to_bits() }
}

pub fn add(a: u32, b: u32) -> u32 {
    if use_hardware() {
        debug_assert_mxcsr();
        if (a | b) << 1 == 0 {
            return b;
        }
        return canonical(f32::from_bits(a) + f32::from_bits(b));
    }
    add_int(a, b)
}

pub fn mul(a: u32, b: u32) -> u32 {
    if use_hardware() {
        debug_assert_mxcsr();
        return canonical(f32::from_bits(a) * f32::from_bits(b));
    }
    mul_int(a, b)
}

/// Integer IEEE-754 add: the reference the hardware path must match.
pub fn add_int(a: u32, b: u32) -> u32 {
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    if is_inf(a) || is_inf(b) {
        return add_inf(a, b);
    }

    let a = Finite::from_bits(a);
    let b = Finite::from_bits(b);
    if a.is_zero() {
        return pack(b.sign, b.mant, b.exp2);
    }
    if b.is_zero() {
        return pack(a.sign, a.mant, a.exp2);
    }

    let min_exp = if a.exp2 < b.exp2 { a.exp2 } else { b.exp2 };
    let a_shift = (a.exp2 - min_exp) as u32;
    let b_shift = (b.exp2 - min_exp) as u32;
    if a_shift > 31 {
        return pack(a.sign, a.mant, a.exp2);
    }
    if b_shift > 31 {
        return pack(b.sign, b.mant, b.exp2);
    }

    let a_mant = signed_shifted_mant(a, a_shift);
    let b_mant = signed_shifted_mant(b, b_shift);
    pack_signed(a_mant + b_mant, min_exp)
}

/// Integer IEEE-754 multiply: the reference the hardware path must match.
pub fn mul_int(a: u32, b: u32) -> u32 {
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    if is_inf(a) || is_inf(b) {
        if is_zero_bits(a) || is_zero_bits(b) {
            return QUIET_NAN;
        }
        return ((a ^ b) & SIGN_MASK) | EXP_MASK;
    }

    let a = Finite::from_bits(a);
    let b = Finite::from_bits(b);
    if a.is_zero() || b.is_zero() {
        return ((u32::from(a.sign ^ b.sign)) << 31) & SIGN_MASK;
    }
    pack(a.sign ^ b.sign, a.mant * b.mant, a.exp2 + b.exp2)
}

fn signed_shifted_mant(value: Finite, shift: u32) -> i64 {
    let shifted = (value.mant << shift) as i64;
    if value.sign { -shifted } else { shifted }
}

fn pack_signed(value: i64, exp2: i32) -> u32 {
    if value == 0 {
        return 0;
    }
    let sign = value < 0;
    let mant = if sign {
        value.wrapping_neg() as u64
    } else {
        value as u64
    };
    pack(sign, mant, exp2)
}

fn pack(sign: bool, mant: u64, exp2: i32) -> u32 {
    if mant == 0 {
        return (u32::from(sign)) << 31;
    }

    let bit_len = 64 - mant.leading_zeros() as i32;
    let mut unbiased = exp2 + bit_len - 1;
    let sign_bits = (u32::from(sign)) << 31;

    if unbiased > 127 {
        return sign_bits | EXP_MASK;
    }

    if unbiased >= -126 {
        let shift = bit_len - 24;
        let mut sig = if shift > 0 {
            round_shift_right(mant, shift as u32)
        } else {
            mant << (-shift as u32)
        };
        if sig >= (1u64 << 24) {
            sig >>= 1;
            unbiased += 1;
            if unbiased > 127 {
                return sign_bits | EXP_MASK;
            }
        }
        let exp_bits = ((unbiased + 127) as u32) << 23;
        let frac_bits = (sig as u32) & FRAC_MASK;
        return sign_bits | exp_bits | frac_bits;
    }

    let scaled_exp = exp2 + 149;
    let frac = if scaled_exp >= 0 {
        let shift = scaled_exp as u32;
        if shift >= 64 { u64::MAX } else { mant << shift }
    } else {
        round_shift_right(mant, (-scaled_exp) as u32)
    };
    if frac >= (1u64 << 23) {
        return sign_bits | (1 << 23);
    }
    if frac == 0 {
        return sign_bits;
    }
    sign_bits | frac as u32
}

fn round_shift_right(value: u64, shift: u32) -> u64 {
    if shift == 0 {
        return value;
    }
    if shift > 64 {
        return 0;
    }
    if shift == 64 {
        let half = 1u64 << 63;
        return u64::from(value >= half);
    }
    let truncated = value >> shift;
    let mask = (1u64 << shift) - 1;
    let remainder = value & mask;
    let half = 1u64 << (shift - 1);
    if remainder > half || (remainder == half && (truncated & 1) != 0) {
        truncated + 1
    } else {
        truncated
    }
}

const fn is_nan(bits: u32) -> bool {
    bits & EXP_MASK == EXP_MASK && bits & FRAC_MASK != 0
}

const fn is_inf(bits: u32) -> bool {
    bits & EXP_MASK == EXP_MASK && bits & FRAC_MASK == 0
}

const fn is_zero_bits(bits: u32) -> bool {
    bits & !SIGN_MASK == 0
}

fn add_inf(a: u32, b: u32) -> u32 {
    if is_inf(a) && is_inf(b) && ((a ^ b) & SIGN_MASK != 0) {
        QUIET_NAN
    } else if is_inf(a) {
        a
    } else {
        b
    }
}

/// IEEE-754 compatible floating-point max (signed comparison).
pub fn fmax(a: u32, b: u32) -> u32 {
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    let a_neg = a & SIGN_MASK != 0;
    let b_neg = b & SIGN_MASK != 0;
    if a_neg && !b_neg {
        return b;
    }
    if !a_neg && b_neg {
        return a;
    }
    if a_neg {
        return if a < b { a } else { b };
    }
    if a > b { a } else { b }
}

/// IEEE-754 compatible floating-point min (signed comparison).
pub fn fmin(a: u32, b: u32) -> u32 {
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    if fmax(a, b) == a { b } else { a }
}

/// Software f32 subtraction.
pub fn sub(a: u32, b: u32) -> u32 {
    add(a, b ^ SIGN_MASK)
}

/// Software f32 division.
pub fn div(a: u32, b: u32) -> u32 {
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    if is_zero_bits(a) && is_zero_bits(b) {
        return QUIET_NAN;
    }
    if is_inf(a) && is_inf(b) {
        return QUIET_NAN;
    }
    if is_zero_bits(b) {
        return if a & SIGN_MASK != 0 {
            0xcf00_0000
        } else {
            0x7f80_0000
        };
    }
    if is_inf(a) {
        return ((a ^ b) & SIGN_MASK) | EXP_MASK;
    }
    if is_zero_bits(a) {
        return (a ^ b) & SIGN_MASK;
    }

    let a = Finite::from_bits(a);
    let b = Finite::from_bits(b);
    let sign = a.sign ^ b.sign;

    let num = (a.mant as u128) << 32;
    let q = num / (b.mant as u128);
    let rem = num % (b.mant as u128);

    let unbiased = a.exp2 - b.exp2 - 32;
    let bit_len = if q == 0 {
        0
    } else {
        128 - q.leading_zeros() as i32
    };
    let norm_exp = unbiased + bit_len - 1;

    if norm_exp > 127 {
        return ((u32::from(sign)) << 31) | EXP_MASK;
    }

    if norm_exp >= -126 {
        let shift = bit_len - 24;
        let mut sig = if shift > 0 {
            q >> shift as u32
        } else {
            q << ((-shift) as u32)
        };

        let round_bit = if shift as u32 >= 1 {
            ((q >> (shift as u32 - 1)) & 1) != 0
        } else {
            false
        };
        let sticky_bits = if shift as u32 >= 2 {
            (q & ((1u128 << (shift as u32 - 1)) - 1)) != 0
        } else {
            false
        };
        let rem_sticky = rem * 2 >= b.mant as u128;

        if round_bit && (sticky_bits || rem_sticky || (sig & 1) != 0) {
            sig += 1;
        }

        if sig >= (1u128 << 24) {
            sig >>= 1;
            if norm_exp + 1 > 127 {
                return ((u32::from(sign)) << 31) | EXP_MASK;
            }
        }

        let final_exp = if sig >= (1u128 << 24) {
            norm_exp + 1
        } else {
            norm_exp
        };
        let exp_bits = ((final_exp + 127) as u32) << 23;
        let frac_bits = (sig as u32) & FRAC_MASK;
        (u32::from(sign)) << 31 | exp_bits | frac_bits
    } else {
        let sign_bits = (u32::from(sign)) << 31;
        let sub_exp = unbiased + 149 + bit_len - 1;
        if sub_exp <= 0 {
            return sign_bits;
        }
        let frac = q << (sub_exp as u32);
        if frac >= (1u128 << 23) {
            return sign_bits | (1u32 << 23);
        }
        sign_bits | (frac as u32)
    }
}

/// Software f32 sqrt via Newton's method in soft-float space.
pub fn sqrt(bits: u32) -> u32 {
    if is_nan(bits) {
        return QUIET_NAN;
    }
    if bits & SIGN_MASK != 0 {
        return QUIET_NAN;
    }
    if is_inf(bits) {
        return 0x7f80_0000;
    }
    if is_zero_bits(bits) {
        return 0;
    }

    let exp = ((bits & EXP_MASK) >> 23) as i32;
    let frac = bits & FRAC_MASK;
    let result_exp = (((exp - 127) / 2) + 127) as u32;
    let biased_exp: u32 = if exp & 1 == 1 {
        (result_exp << 23) | (0x400000u32 << 1)
    } else {
        result_exp << 23
    };

    let mut x = biased_exp | ((frac >> 1) & FRAC_MASK);
    let half: u32 = 0x3f00_0000;
    let n_float = bits;

    for _ in 0..8 {
        let nx = div(n_float, x);
        let next = mul(add(x, nx), half);
        x = next;
    }
    x
}

/// Software f32 rsqrt.
pub fn rsqrt(bits: u32) -> u32 {
    div(0x3f80_0000, sqrt(bits))
}

/// Convert a signed integer to f32 bit pattern.
pub fn int_to_f32(k: i32) -> u32 {
    if k == 0 {
        return 0;
    }
    let sign = if k < 0 { SIGN_MASK } else { 0 };
    let abs_k = if k < 0 {
        if k == i32::MIN {
            0x8000_0000u32
        } else {
            (-k) as u32
        }
    } else {
        k as u32
    };
    let lz = abs_k.leading_zeros();
    let bit_pos = 31 - lz;
    let biased_exp = (bit_pos as i32 + 127) as u32;
    let frac_shift = 23 - bit_pos;
    let frac = if frac_shift <= 23 {
        (abs_k << frac_shift) & FRAC_MASK
    } else {
        abs_k >> (frac_shift - 23)
    };
    sign | (biased_exp << 23) | frac
}

/// Round an f32 bit pattern to the nearest integer.
pub fn round_f32_to_int(bits: u32) -> i32 {
    if is_nan(bits) || is_inf(bits) {
        return if bits & SIGN_MASK != 0 { -127 } else { 127 };
    }
    if is_zero_bits(bits) {
        return 0;
    }
    let f = Finite::from_bits(bits);
    let sign = f.sign;
    let m = f.mant;
    let e = f.exp2;

    if e >= 31 {
        return if sign { -127 } else { 127 };
    }
    if e < -23 {
        return 0;
    }

    let shift = (-e) as u32;
    if shift >= 63 {
        return 0;
    }
    let rounded = (m + (1u64 << (shift.saturating_sub(1)))) >> shift;
    let result: i64 = if sign {
        -(rounded as i64)
    } else {
        rounded as i64
    };
    if result > 127 {
        127
    } else if result < -127 {
        -127
    } else {
        result as i32
    }
}

/// Naive range reduction: r = x - k*ln(2).
fn reduce_exp(x_bits: u32, k: i32) -> u32 {
    if k == 0 {
        return x_bits;
    }
    let ln2: u32 = 0x3f31_7218;
    let kf_ln2 = mul(int_to_f32(k), ln2);
    sub(x_bits, kf_ln2)
}

/// Make a power-of-two f32: 2^k.
pub fn make_power_of_two(k: i32) -> u32 {
    let exp = (k + 127) as i16;
    if exp > 254 {
        return 0x7f80_0000;
    }
    if exp < 1 {
        if exp < -126 {
            return 0;
        }
        let sub = 1 - exp;
        return 0x007f_ffff >> sub as u32;
    }
    (exp as u32) << 23
}

/// Software f32 exp(e^x) using argument reduction + Chebyshev-like polynomial.
/// Range reduction: x = k*ln(2) + r, where |r| <= ln(2)/2.
/// Then e^r is approximated by a degree-8 polynomial fitted on [-ln(2)/2, ln(2)/2].
pub fn exp(bits: u32) -> u32 {
    if is_nan(bits) {
        return QUIET_NAN;
    }
    if bits == 0 || bits == SIGN_MASK {
        return 0x3f80_0000;
    }
    if is_inf(bits) {
        return if bits & SIGN_MASK != 0 {
            0
        } else {
            0x7f80_0000
        };
    }

    let f = Finite::from_bits(bits);
    let x_sign = f.sign;

    let abs_bits = bits & !SIGN_MASK;
    let overflow_threshold: u32 = 0x42b1_7218;
    if abs_bits > overflow_threshold {
        return if x_sign { 0 } else { 0x7f80_0000 };
    }

    let ln2_inv: u32 = 0x3fb8_aa3b;
    let fx = mul(bits, ln2_inv);
    let k = round_f32_to_int(fx);

    if k >= 128 {
        return if x_sign { 0 } else { 0x7f80_0000 };
    }
    if k <= -128 {
        return if x_sign { 0x7f80_0000 } else { 0 };
    }

    let r = reduce_exp(bits, k);

    let p = exp_poly(r);
    let two_pow_k = make_power_of_two(k);
    let result = mul(p, two_pow_k);

    if is_nan(result) { QUIET_NAN } else { result }
}

/// Evaluate e^r for |r| <= ln(2)/2 via plain degree-6 Horner.
/// Coefficients: 1/720, 1/120, 1/24, 1/6, 1/2, 1, 1
/// acc = c6; for c in [c5..c0]: acc = acc*r + c
pub fn exp_poly(r: u32) -> u32 {
    let c6: u32 = 0x3ab6_0b61; // 1/720
    let c5: u32 = 0x3c08_8889; // 1/120
    let c4: u32 = 0x3d2a_aaab; // 1/24
    let c3: u32 = 0x3e2a_aaab; // 1/6
    let c2: u32 = 0x3f00_0000; // 1/2
    let c1: u32 = 0x3f80_0000; // 1
    let c0: u32 = 0x3f80_0000; // 1

    let mut acc = c6;
    acc = add(mul(acc, r), c5);
    acc = add(mul(acc, r), c4);
    acc = add(mul(acc, r), c3);
    acc = add(mul(acc, r), c2);
    acc = add(mul(acc, r), c1);
    add(mul(acc, r), c0)
}

/// Software f32 tanh. Uses (e^(2x)-1)/(e^(2x)+1) for |x| >= 0.1,
/// and a Taylor series x - x^3/3 + 2*x^5/15 - 17*x^7/315 for small |x|.
pub fn tanh(bits: u32) -> u32 {
    if is_nan(bits) {
        return QUIET_NAN;
    }
    if bits == 0 || bits == SIGN_MASK {
        return 0;
    }
    if is_inf(bits) {
        return if bits & SIGN_MASK != 0 {
            0xbf80_0000
        } else {
            0x3f80_0000
        };
    }

    let abs_bits = bits & !SIGN_MASK;
    let threshold: u32 = 0x3dcc_cccd; // 0.1

    if abs_bits >= threshold {
        let two = 0x4000_0000;
        let two_x = mul(two, bits);
        let e2x = exp(two_x);

        if is_inf(e2x) {
            return if bits & SIGN_MASK != 0 {
                0xbf80_0000
            } else {
                0x3f80_0000
            };
        }

        let one = 0x3f80_0000;
        let num = sub(e2x, one);
        let den = add(e2x, one);
        div(num, den)
    } else {
        tanh_small_x(bits)
    }
}

/// Taylor series for tanh(x) when |x| < 0.1:
/// tanh(x) ≈ x - x^3/3 + 2*x^5/15 - 17*x^7/315
fn tanh_small_x(bits: u32) -> u32 {
    let x = bits;
    let x2 = mul(x, x);
    let x3 = mul(x2, x);
    let x5 = mul(x3, x2);
    let x7 = mul(x5, x2);

    let third: u32 = 0x3eaa_aaab; // 1/3
    let fifteenths: u32 = 0x3e08_8889; // 2/15
    let coeff7: u32 = 0x3d5d_0dd1; // 17/315

    let t1 = mul(x3, third);
    let t2 = mul(x5, fifteenths);
    let t3 = mul(x7, coeff7);

    let result = sub(x, t1);
    let result = add(result, t2);
    sub(result, t3)
}

/// Natural logarithm for positive inputs near the RoPE scaling range.
pub fn ln(bits: u32) -> u32 {
    if is_nan(bits) || bits & SIGN_MASK != 0 || is_zero_bits(bits) {
        return QUIET_NAN;
    }
    if is_inf(bits) {
        return 0x7f80_0000;
    }
    if bits == 0x3f80_0000 {
        return 0;
    }

    // ln(x) = 2 * atanh((x - 1) / (x + 1)). YaRN scaling only needs this for
    // modest positive values (normally 1/freq_scale), where the series is stable.
    let one = 0x3f80_0000;
    let two = 0x4000_0000;
    let z = div(sub(bits, one), add(bits, one));
    let z2 = mul(z, z);
    let mut term = z;
    let mut acc = z;
    let mut denom = 3i32;
    while denom <= 25 {
        term = mul(term, z2);
        acc = add(acc, div(term, int_to_f32(denom)));
        denom += 2;
    }
    mul(two, acc)
}

/// Software f32 sine with deterministic range reduction and polynomial core.
pub fn sin(bits: u32) -> u32 {
    if is_nan(bits) || is_inf(bits) {
        return QUIET_NAN;
    }
    if is_zero_bits(bits) {
        return 0;
    }

    let x = reduce_angle(bits);
    let x2 = mul(x, x);
    let x3 = mul(x2, x);
    let x5 = mul(x3, x2);
    let x7 = mul(x5, x2);
    let x9 = mul(x7, x2);
    let x11 = mul(x9, x2);

    let inv_3_fact = 0x3e2a_aaab; // 1/6
    let inv_5_fact = 0x3c08_8889; // 1/120
    let inv_7_fact = 0x3950_0d01; // 1/5040
    let inv_9_fact = 0x3638_ef1d; // 1/362880
    let inv_11_fact = 0x32d7_322b; // 1/39916800

    let mut result = sub(x, mul(x3, inv_3_fact));
    result = add(result, mul(x5, inv_5_fact));
    result = sub(result, mul(x7, inv_7_fact));
    result = add(result, mul(x9, inv_9_fact));
    sub(result, mul(x11, inv_11_fact))
}

/// Software f32 cosine with deterministic range reduction and polynomial core.
pub fn cos(bits: u32) -> u32 {
    if is_nan(bits) || is_inf(bits) {
        return QUIET_NAN;
    }

    let x = reduce_angle(bits);
    let x2 = mul(x, x);
    let x4 = mul(x2, x2);
    let x6 = mul(x4, x2);
    let x8 = mul(x6, x2);
    let x10 = mul(x8, x2);

    let one = 0x3f80_0000;
    let inv_2_fact = 0x3f00_0000; // 1/2
    let inv_4_fact = 0x3d2a_aaab; // 1/24
    let inv_6_fact = 0x3ab6_0b61; // 1/720
    let inv_8_fact = 0x37d0_0d01; // 1/40320
    let inv_10_fact = 0x3493_f27e; // 1/3628800

    let mut result = sub(one, mul(x2, inv_2_fact));
    result = add(result, mul(x4, inv_4_fact));
    result = sub(result, mul(x6, inv_6_fact));
    result = add(result, mul(x8, inv_8_fact));
    sub(result, mul(x10, inv_10_fact))
}

fn reduce_angle(mut bits: u32) -> u32 {
    const PI: u32 = 0x4049_0fdb;
    const TWO_PI: u32 = 0x40c9_0fdb;

    let mut steps = 0usize;
    while (bits & !SIGN_MASK) > PI && steps < 8192 {
        bits = if bits & SIGN_MASK != 0 {
            add(bits, TWO_PI)
        } else {
            sub(bits, TWO_PI)
        };
        steps += 1;
    }
    bits
}
