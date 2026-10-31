// SPDX-License-Identifier: MPL-2.0
//! Scalar/native quant dot products and q8 activation quantization helpers.
//!
//! Raw-source basis: ggml-org/llama.cpp
//! `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3`, `ggml/src/ggml-common.h`
//! (`block_q4_K`/`block_q5_0`/`block_q5_K`/`block_q6_K`/`block_q8_0`/`block_q8_K`),
//! `ggml/src/ggml-quants.c` (`quantize_row_q8_0_ref`,
//! `quantize_row_q8_K_ref`), and `ggml/src/ggml-cpu/quants.c`
//! (`ggml_vec_dot_q*_q8_*_generic`).
use super::q4_k::{self, Q4_K_BLOCK_BYTES, Q4_K_SCALE_BYTES, Q4_K_VALUES};

pub const Q5_0_VALUES: usize = 32;
pub const Q5_0_BLOCK_BYTES: usize = 22;
pub const Q5_K_VALUES: usize = Q4_K_VALUES;
pub const Q5_K_BLOCK_BYTES: usize = 176;
pub const Q6_K_VALUES: usize = Q4_K_VALUES;
pub const Q6_K_BLOCK_BYTES: usize = 210;
pub const Q8_0_VALUES: usize = 32;
pub const Q8_0_BLOCK_BYTES: usize = 34;
pub const Q8_K_VALUES: usize = Q4_K_VALUES;
pub const Q8_K_BSUMS: usize = Q8_K_VALUES / 16;
pub const Q8_K_BLOCK_BYTES: usize = 4 + Q8_K_VALUES + Q8_K_BSUMS * 2;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum QKMatmulError {
    Length,
    Shape,
    Q4KScale,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BlockQ8K {
    pub d: f32,
    pub qs: [i8; Q8_K_VALUES],
    pub bsums: [i16; Q8_K_BSUMS],
}

impl BlockQ8K {
    pub const ZERO: Self = Self {
        d: 0.0,
        qs: [0; Q8_K_VALUES],
        bsums: [0; Q8_K_BSUMS],
    };

    pub fn write_bytes(&self, out: &mut [u8]) -> Result<(), QKMatmulError> {
        if out.len() != Q8_K_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }
        out[0..4].copy_from_slice(&self.d.to_le_bytes());
        let mut idx = 0usize;
        while idx < Q8_K_VALUES {
            out[4 + idx] = self.qs[idx] as u8;
            idx += 1;
        }
        let mut bsum_idx = 0usize;
        while bsum_idx < Q8_K_BSUMS {
            let off = 4 + Q8_K_VALUES + bsum_idx * 2;
            out[off..off + 2].copy_from_slice(&self.bsums[bsum_idx].to_le_bytes());
            bsum_idx += 1;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BlockQ8Zero {
    pub d: f32,
    pub qs: [i8; Q8_0_VALUES],
}

impl BlockQ8Zero {
    pub const ZERO: Self = Self {
        d: 0.0,
        qs: [0; Q8_0_VALUES],
    };

    pub fn write_bytes(&self, out: &mut [u8]) -> Result<(), QKMatmulError> {
        if out.len() != Q8_0_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }
        out[0..2].copy_from_slice(&f32_to_f16_bits(self.d).to_le_bytes());
        let mut idx = 0usize;
        while idx < Q8_0_VALUES {
            out[2 + idx] = self.qs[idx] as u8;
            idx += 1;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum QKMatmulKernel {
    Scalar,
    /// Select the AVX2 dot when the default-off `phase9-avx2-matmul` code is
    /// compiled and runtime CPUID/XCR0 proves it is safe; otherwise use scalar.
    AutoAvx2,
}

pub fn quantize_row_q8_k(input: &[f32], out: &mut [BlockQ8K]) -> Result<(), QKMatmulError> {
    if !input.len().is_multiple_of(Q8_K_VALUES) || out.len() != input.len() / Q8_K_VALUES {
        return Err(QKMatmulError::Length);
    }

    let mut block_idx = 0usize;
    while block_idx < out.len() {
        let base = block_idx * Q8_K_VALUES;
        quantize_block_q8_k(&input[base..base + Q8_K_VALUES], &mut out[block_idx])?;
        block_idx += 1;
    }
    Ok(())
}

/// Same shape contract as `quantize_row_q8_k`, with an explicit kernel
/// selector. The default entry point above deliberately remains scalar so
/// Phase 9.2's in-kernel forward path cannot start using YMM state by
/// accident — mirrors the 9.3 matmul dispatch.
pub fn quantize_row_q8_k_with_kernel(
    input: &[f32],
    out: &mut [BlockQ8K],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    if !input.len().is_multiple_of(Q8_K_VALUES) || out.len() != input.len() / Q8_K_VALUES {
        return Err(QKMatmulError::Length);
    }

    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            let mut block_idx = 0usize;
            while block_idx < out.len() {
                let base = block_idx * Q8_K_VALUES;
                // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
                // and XCR0 XMM/YMM state enablement immediately before this
                // target-feature call. Block length is validated above.
                unsafe {
                    avx2::quantize_block_q8_k(
                        &input[base..base + Q8_K_VALUES],
                        &mut out[block_idx],
                    )?
                };
                block_idx += 1;
            }
            return Ok(());
        }
    }

    let _ = kernel;
    quantize_row_q8_k(input, out)
}

pub fn quantize_row_q8_0(input: &[f32], out: &mut [BlockQ8Zero]) -> Result<(), QKMatmulError> {
    if !input.len().is_multiple_of(Q8_0_VALUES) || out.len() != input.len() / Q8_0_VALUES {
        return Err(QKMatmulError::Length);
    }

    let mut block_idx = 0usize;
    while block_idx < out.len() {
        let base = block_idx * Q8_0_VALUES;
        quantize_block_q8_0(&input[base..base + Q8_0_VALUES], &mut out[block_idx])?;
        block_idx += 1;
    }
    Ok(())
}

pub fn quantize_row_q8_0_with_kernel(
    input: &[f32],
    out: &mut [BlockQ8Zero],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    let _ = kernel;
    quantize_row_q8_0(input, out)
}

pub fn write_q8_0_blocks(blocks: &[BlockQ8Zero], out: &mut [u8]) -> Result<(), QKMatmulError> {
    if out.len() != blocks.len() * Q8_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }
    let mut idx = 0usize;
    while idx < blocks.len() {
        let off = idx * Q8_0_BLOCK_BYTES;
        blocks[idx].write_bytes(&mut out[off..off + Q8_0_BLOCK_BYTES])?;
        idx += 1;
    }
    Ok(())
}

pub fn write_q8_k_blocks(blocks: &[BlockQ8K], out: &mut [u8]) -> Result<(), QKMatmulError> {
    if out.len() != blocks.len() * Q8_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }
    let mut idx = 0usize;
    while idx < blocks.len() {
        let off = idx * Q8_K_BLOCK_BYTES;
        blocks[idx].write_bytes(&mut out[off..off + Q8_K_BLOCK_BYTES])?;
        idx += 1;
    }
    Ok(())
}

/// The AVX2 decision, decided once instead of per dot product (Arthur
/// latency NOTE, fix 2). Every dot product used to run CPUID + XGETBV to
/// decide, about 450k serializing instructions per token (each a VM exit under
/// a hypervisor).
///
/// **The decision must hold on the CPU that executes**, and any CPU may run a
/// matmul (a `par_matmul` partition, or a generation on a migrated thread).
/// XCR0 is per-CPU and CPUs need not agree (an AP without XSAVE never writes
/// XCR0; an FXSAVE build leaves firmware's XCR0 per CPU). So it is a census:
///
/// 1. every CPU reports its own local check ([`report_cpu_avx2`]) in its own
///    `enable_sse`, after its XCR0 setup and before it can poll for or run a
///    matmul;
/// 2. any "no" vetoes at once: the decision becomes "no", which is safe on
///    every CPU (scalar runs everywhere);
/// 3. "yes" is published ([`publish_avx2_decision`]) only when every online CPU
///    has reported and none vetoed; a CPU that reports later still vetoes,
///    on itself, before it runs any matmul;
/// 4. until a decision exists, the hot path runs the old local check.
///
/// The state is one atomic word changed only by read-modify-write, so a veto
/// and a publish cannot interleave into a stale "yes".
pub static AVX2_CENSUS: Avx2Census = Avx2Census::new();

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Avx2Decision {
    /// Not decided yet: the caller runs its own local check.
    Undecided,
    No,
    Yes,
}

pub struct Avx2Census {
    /// bits 1..0: decision (0 undecided, 1 no, 2 yes); bit 2: veto;
    /// bits 63..32: CPUs that have reported.
    word: core::sync::atomic::AtomicU64,
}

const CENSUS_DECISION_MASK: u64 = 0b11;
const CENSUS_NO: u64 = 1;
const CENSUS_YES: u64 = 2;
const CENSUS_VETO: u64 = 1 << 2;
const CENSUS_REPORTED_SHIFT: u32 = 32;

impl Avx2Census {
    pub const fn new() -> Self {
        Self {
            word: core::sync::atomic::AtomicU64::new(0),
        }
    }

    fn update(&self, f: impl Fn(u64) -> u64) -> u64 {
        use core::sync::atomic::Ordering;
        match self
            .word
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |w| Some(f(w)))
        {
            Ok(prev) | Err(prev) => f(prev),
        }
    }

    /// One CPU's own check. A "no" decides "no" for everyone, at once.
    pub fn report(&self, local_yes: bool) {
        self.update(|w| {
            let reported = (w >> CENSUS_REPORTED_SHIFT).saturating_add(1);
            let mut low = w & ((1 << CENSUS_REPORTED_SHIFT) - 1);
            if !local_yes {
                low = (low & !CENSUS_DECISION_MASK) | CENSUS_VETO | CENSUS_NO;
            }
            (reported << CENSUS_REPORTED_SHIFT) | low
        });
    }

    /// Decide "yes" only if `online` CPUs have reported and none vetoed;
    /// otherwise leave the decision as it is (undecided, or "no" after a veto).
    pub fn publish(&self, online: u32) -> Avx2Decision {
        let w = self.update(|w| {
            let reported = w >> CENSUS_REPORTED_SHIFT;
            let undecided = w & CENSUS_DECISION_MASK == 0;
            if undecided && w & CENSUS_VETO == 0 && reported == u64::from(online) && online > 0 {
                w | CENSUS_YES
            } else {
                w
            }
        });
        Self::decode(w)
    }

    pub fn decision(&self) -> Avx2Decision {
        Self::decode(self.word.load(core::sync::atomic::Ordering::Acquire))
    }

    pub fn reported(&self) -> u32 {
        (self.word.load(core::sync::atomic::Ordering::Acquire) >> CENSUS_REPORTED_SHIFT) as u32
    }

    fn decode(w: u64) -> Avx2Decision {
        match w & CENSUS_DECISION_MASK {
            CENSUS_YES => Avx2Decision::Yes,
            CENSUS_NO => Avx2Decision::No,
            _ => Avx2Decision::Undecided,
        }
    }
}

impl Default for Avx2Census {
    fn default() -> Self {
        Self::new()
    }
}

/// This CPU's own AVX2 check: CPUID AVX/AVX2 + OSXSAVE and this CPU's XCR0.
/// "No" when the AVX2 kernels are not compiled (`phase9-avx2-matmul` off).
pub fn local_avx2_check() -> bool {
    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        avx2::runtime_available_uncached()
    }
    #[cfg(not(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul"))))]
    {
        false
    }
}

/// Report the calling CPU to the census (its `enable_sse`, after XCR0).
pub fn report_cpu_avx2() -> bool {
    let yes = local_avx2_check();
    AVX2_CENSUS.report(yes);
    yes
}

/// Publish the census once every online CPU has had the chance to report.
pub fn publish_avx2_decision(online_cpus: u32) -> Avx2Decision {
    AVX2_CENSUS.publish(online_cpus)
}

/// Host tools' single-CPU shortcut (the bench, tests): report this CPU and
/// publish for exactly the CPUs reported so far. The kernel never calls it.
#[allow(dead_code)]
pub fn init_avx2_decision() -> bool {
    report_cpu_avx2();
    AVX2_CENSUS.publish(AVX2_CENSUS.reported()) == Avx2Decision::Yes
}

/// Whether a decision exists (the KAT and the boot breadcrumb).
#[allow(dead_code)]
pub fn avx2_decision_cached() -> bool {
    AVX2_CENSUS.decision() != Avx2Decision::Undecided
}

pub fn q4_k_q8_k_avx2_available() -> bool {
    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        avx2::runtime_available()
    }

    #[cfg(not(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul"))))]
    {
        false
    }
}

/// Same contract as `q4_k::dequantize_q4_k`, with an explicit kernel selector.
/// The default scalar `dequantize_q4_k` stays the frozen oracle so the
/// off-build is byte-identical to today. Mirrors
/// `quantize_row_q8_k_with_kernel` / `vec_dot_q4_k_q8_k_with_kernel`.
pub fn dequantize_q4_k_with_kernel(
    block: &[u8],
    kernel: QKMatmulKernel,
) -> Result<[f32; Q4_K_VALUES], QKMatmulError> {
    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
            // and XCR0 XMM/YMM state enablement immediately before this
            // target-feature call. Block length is validated inside.
            return unsafe { avx2::dequantize_q4_k(block) };
        }
    }

    let _ = kernel;
    q4_k::dequantize_q4_k(block).map_err(|_| QKMatmulError::Q4KScale)
}

pub fn vec_dot_q4_k_q8_k(q4: &[u8], q8: &[BlockQ8K]) -> Result<f32, QKMatmulError> {
    if q4.len() != q8.len() * Q4_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let mut sum = 0.0f32;
    let mut block_idx = 0usize;
    while block_idx < q8.len() {
        let off = block_idx * Q4_K_BLOCK_BYTES;
        sum += vec_dot_block_q4_k_q8_k(&q4[off..off + Q4_K_BLOCK_BYTES], &q8[block_idx])?;
        block_idx += 1;
    }
    Ok(sum)
}

pub fn vec_dot_q4_k_q8_k_with_kernel(
    q4: &[u8],
    q8: &[BlockQ8K],
    kernel: QKMatmulKernel,
) -> Result<f32, QKMatmulError> {
    if q4.len() != q8.len() * Q4_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
            // and XCR0 XMM/YMM state enablement immediately before this
            // target-feature call. Length was validated above.
            return unsafe { avx2::vec_dot_q4_k_q8_k(q4, q8) };
        }
    }

    let _ = kernel;
    vec_dot_q4_k_q8_k(q4, q8)
}

pub fn vec_dot_q5_0_q8_0(q5: &[u8], q8: &[BlockQ8Zero]) -> Result<f32, QKMatmulError> {
    if q5.len() != q8.len() * Q5_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let mut sum = 0.0f32;
    let mut block_idx = 0usize;
    while block_idx < q8.len() {
        let off = block_idx * Q5_0_BLOCK_BYTES;
        sum += vec_dot_block_q5_0_q8_0(&q5[off..off + Q5_0_BLOCK_BYTES], &q8[block_idx])?;
        block_idx += 1;
    }
    Ok(sum)
}

pub fn vec_dot_q5_k_q8_k(q5: &[u8], q8: &[BlockQ8K]) -> Result<f32, QKMatmulError> {
    if q5.len() != q8.len() * Q5_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let mut sum = 0.0f32;
    let mut block_idx = 0usize;
    while block_idx < q8.len() {
        let off = block_idx * Q5_K_BLOCK_BYTES;
        sum += vec_dot_block_q5_k_q8_k(&q5[off..off + Q5_K_BLOCK_BYTES], &q8[block_idx])?;
        block_idx += 1;
    }
    Ok(sum)
}

pub fn vec_dot_q5_k_q8_k_with_kernel(
    q5: &[u8],
    q8: &[BlockQ8K],
    kernel: QKMatmulKernel,
) -> Result<f32, QKMatmulError> {
    if q5.len() != q8.len() * Q5_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }
    let _ = kernel;
    vec_dot_q5_k_q8_k(q5, q8)
}

pub fn vec_dot_q5_0_q8_0_with_kernel(
    q5: &[u8],
    q8: &[BlockQ8Zero],
    kernel: QKMatmulKernel,
) -> Result<f32, QKMatmulError> {
    if q5.len() != q8.len() * Q5_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
            // and XCR0 XMM/YMM state enablement immediately before this
            // target-feature call. Length was validated above.
            return unsafe { avx2::vec_dot_q5_0_q8_0(q5, q8) };
        }
    }

    let _ = kernel;
    vec_dot_q5_0_q8_0(q5, q8)
}

pub fn vec_dot_q6_k_q8_k(q6: &[u8], q8: &[BlockQ8K]) -> Result<f32, QKMatmulError> {
    if q6.len() != q8.len() * Q6_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let mut sum = 0.0f32;
    let mut block_idx = 0usize;
    while block_idx < q8.len() {
        let off = block_idx * Q6_K_BLOCK_BYTES;
        sum += vec_dot_block_q6_k_q8_k(&q6[off..off + Q6_K_BLOCK_BYTES], &q8[block_idx])?;
        block_idx += 1;
    }
    Ok(sum)
}

pub fn vec_dot_q6_k_q8_k_with_kernel(
    q6: &[u8],
    q8: &[BlockQ8K],
    kernel: QKMatmulKernel,
) -> Result<f32, QKMatmulError> {
    if q6.len() != q8.len() * Q6_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
            // and XCR0 XMM/YMM state enablement immediately before this
            // target-feature call. Length was validated above.
            return unsafe { avx2::vec_dot_q6_k_q8_k(q6, q8) };
        }
    }

    let _ = kernel;
    vec_dot_q6_k_q8_k(q6, q8)
}

pub fn vec_dot_q8_0_q8_0(q8_weight: &[u8], q8: &[BlockQ8Zero]) -> Result<f32, QKMatmulError> {
    if q8_weight.len() != q8.len() * Q8_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let mut sum = 0.0f32;
    let mut block_idx = 0usize;
    while block_idx < q8.len() {
        let off = block_idx * Q8_0_BLOCK_BYTES;
        sum += vec_dot_block_q8_0_q8_0(&q8_weight[off..off + Q8_0_BLOCK_BYTES], &q8[block_idx])?;
        block_idx += 1;
    }
    Ok(sum)
}

pub fn vec_dot_q8_0_q8_0_with_kernel(
    q8_weight: &[u8],
    q8: &[BlockQ8Zero],
    kernel: QKMatmulKernel,
) -> Result<f32, QKMatmulError> {
    if q8_weight.len() != q8.len() * Q8_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
            // and XCR0 XMM/YMM state enablement immediately before this
            // target-feature call. Length was validated above.
            return unsafe { avx2::vec_dot_q8_0_q8_0(q8_weight, q8) };
        }
    }

    let _ = kernel;
    vec_dot_q8_0_q8_0(q8_weight, q8)
}

/// Computes `(rows x shared) Q8_K` by `(cols x shared) Q4_K`, with each Q4_K
/// row stored contiguously as raw 144-byte `block_q4_K` blocks.
pub fn matmul_q4_k_q8_k(
    lhs_q8: &[BlockQ8K],
    rhs_q4: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), QKMatmulError> {
    matmul_q4_k_q8_k_with_kernel(
        lhs_q8,
        rhs_q4,
        rows,
        cols,
        shared,
        out,
        QKMatmulKernel::Scalar,
    )
}

/// Same shape contract as `matmul_q4_k_q8_k`, with an explicit kernel
/// selector. The default scalar entry point above deliberately remains scalar
/// so Phase 9.2's in-kernel forward path cannot start using YMM state by
/// accident.
pub fn matmul_q4_k_q8_k_with_kernel(
    lhs_q8: &[BlockQ8K],
    rhs_q4: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    if rows == 0 || cols == 0 || shared == 0 || !shared.is_multiple_of(Q4_K_VALUES) {
        return Err(QKMatmulError::Shape);
    }
    let blocks_per_dot = shared / Q4_K_VALUES;
    if lhs_q8.len() != rows * blocks_per_dot
        || rhs_q4.len() != cols * blocks_per_dot * Q4_K_BLOCK_BYTES
        || out.len() != rows * cols
    {
        return Err(QKMatmulError::Length);
    }

    let mut row = 0usize;
    while row < rows {
        let lhs_base = row * blocks_per_dot;
        let lhs = &lhs_q8[lhs_base..lhs_base + blocks_per_dot];
        let mut col = 0usize;
        while col < cols {
            let rhs_base = col * blocks_per_dot * Q4_K_BLOCK_BYTES;
            let rhs = &rhs_q4[rhs_base..rhs_base + blocks_per_dot * Q4_K_BLOCK_BYTES];
            out[row * cols + col] = vec_dot_q4_k_q8_k_with_kernel(rhs, lhs, kernel)?;
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

/// Computes `(rows x shared) Q8_0` by `(cols x shared) Q5_0`.
pub fn matmul_q5_0_q8_0(
    lhs_q8: &[BlockQ8Zero],
    rhs_q5: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), QKMatmulError> {
    matmul_q5_0_q8_0_with_kernel(
        lhs_q8,
        rhs_q5,
        rows,
        cols,
        shared,
        out,
        QKMatmulKernel::Scalar,
    )
}

/// Computes `(rows x shared) Q8_K` by `(cols x shared) Q5_K`.
pub fn matmul_q5_k_q8_k(
    lhs_q8: &[BlockQ8K],
    rhs_q5: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), QKMatmulError> {
    matmul_q5_k_q8_k_with_kernel(
        lhs_q8,
        rhs_q5,
        rows,
        cols,
        shared,
        out,
        QKMatmulKernel::Scalar,
    )
}

pub fn matmul_q5_k_q8_k_with_kernel(
    lhs_q8: &[BlockQ8K],
    rhs_q5: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    if rows == 0 || cols == 0 || shared == 0 || !shared.is_multiple_of(Q5_K_VALUES) {
        return Err(QKMatmulError::Shape);
    }
    let blocks_per_dot = shared / Q5_K_VALUES;
    if lhs_q8.len() != rows * blocks_per_dot
        || rhs_q5.len() != cols * blocks_per_dot * Q5_K_BLOCK_BYTES
        || out.len() != rows * cols
    {
        return Err(QKMatmulError::Length);
    }

    let mut row = 0usize;
    while row < rows {
        let lhs_base = row * blocks_per_dot;
        let lhs = &lhs_q8[lhs_base..lhs_base + blocks_per_dot];
        let mut col = 0usize;
        while col < cols {
            let rhs_base = col * blocks_per_dot * Q5_K_BLOCK_BYTES;
            let rhs = &rhs_q5[rhs_base..rhs_base + blocks_per_dot * Q5_K_BLOCK_BYTES];
            out[row * cols + col] = vec_dot_q5_k_q8_k_with_kernel(rhs, lhs, kernel)?;
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

pub fn matmul_q5_0_q8_0_with_kernel(
    lhs_q8: &[BlockQ8Zero],
    rhs_q5: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    if rows == 0 || cols == 0 || shared == 0 || !shared.is_multiple_of(Q5_0_VALUES) {
        return Err(QKMatmulError::Shape);
    }
    let blocks_per_dot = shared / Q5_0_VALUES;
    if lhs_q8.len() != rows * blocks_per_dot
        || rhs_q5.len() != cols * blocks_per_dot * Q5_0_BLOCK_BYTES
        || out.len() != rows * cols
    {
        return Err(QKMatmulError::Length);
    }

    let mut row = 0usize;
    #[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
    {
        if kernel == QKMatmulKernel::AutoAvx2 && avx2::runtime_available() {
            // SAFETY: `runtime_available` checked CPUID AVX/AVX2, OSXSAVE,
            // and XCR0 XMM/YMM state enablement once for this whole matmul.
            // Shapes and output lengths were validated above.
            return unsafe {
                avx2::matmul_q5_0_q8_0(lhs_q8, rhs_q5, rows, cols, blocks_per_dot, out)
            };
        }
    }
    let _ = kernel;

    while row < rows {
        let lhs_base = row * blocks_per_dot;
        let lhs = &lhs_q8[lhs_base..lhs_base + blocks_per_dot];
        let mut col = 0usize;
        while col < cols {
            let rhs_base = col * blocks_per_dot * Q5_0_BLOCK_BYTES;
            let rhs = &rhs_q5[rhs_base..rhs_base + blocks_per_dot * Q5_0_BLOCK_BYTES];
            out[row * cols + col] = vec_dot_q5_0_q8_0(rhs, lhs)?;
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

/// Computes `(rows x shared) Q8_K` by `(cols x shared) Q6_K`.
pub fn matmul_q6_k_q8_k(
    lhs_q8: &[BlockQ8K],
    rhs_q6: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), QKMatmulError> {
    matmul_q6_k_q8_k_with_kernel(
        lhs_q8,
        rhs_q6,
        rows,
        cols,
        shared,
        out,
        QKMatmulKernel::Scalar,
    )
}

pub fn matmul_q6_k_q8_k_with_kernel(
    lhs_q8: &[BlockQ8K],
    rhs_q6: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    if rows == 0 || cols == 0 || shared == 0 || !shared.is_multiple_of(Q6_K_VALUES) {
        return Err(QKMatmulError::Shape);
    }
    let blocks_per_dot = shared / Q6_K_VALUES;
    if lhs_q8.len() != rows * blocks_per_dot
        || rhs_q6.len() != cols * blocks_per_dot * Q6_K_BLOCK_BYTES
        || out.len() != rows * cols
    {
        return Err(QKMatmulError::Length);
    }

    let mut row = 0usize;
    while row < rows {
        let lhs_base = row * blocks_per_dot;
        let lhs = &lhs_q8[lhs_base..lhs_base + blocks_per_dot];
        let mut col = 0usize;
        while col < cols {
            let rhs_base = col * blocks_per_dot * Q6_K_BLOCK_BYTES;
            let rhs = &rhs_q6[rhs_base..rhs_base + blocks_per_dot * Q6_K_BLOCK_BYTES];
            out[row * cols + col] = vec_dot_q6_k_q8_k_with_kernel(rhs, lhs, kernel)?;
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

/// Computes `(rows x shared) Q8_0` by `(cols x shared) Q8_0`.
pub fn matmul_q8_0_q8_0(
    lhs_q8: &[BlockQ8Zero],
    rhs_q8: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), QKMatmulError> {
    matmul_q8_0_q8_0_with_kernel(
        lhs_q8,
        rhs_q8,
        rows,
        cols,
        shared,
        out,
        QKMatmulKernel::Scalar,
    )
}

pub fn matmul_q8_0_q8_0_with_kernel(
    lhs_q8: &[BlockQ8Zero],
    rhs_q8: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
    kernel: QKMatmulKernel,
) -> Result<(), QKMatmulError> {
    if rows == 0 || cols == 0 || shared == 0 || !shared.is_multiple_of(Q8_0_VALUES) {
        return Err(QKMatmulError::Shape);
    }
    let blocks_per_dot = shared / Q8_0_VALUES;
    if lhs_q8.len() != rows * blocks_per_dot
        || rhs_q8.len() != cols * blocks_per_dot * Q8_0_BLOCK_BYTES
        || out.len() != rows * cols
    {
        return Err(QKMatmulError::Length);
    }

    let mut row = 0usize;
    while row < rows {
        let lhs_base = row * blocks_per_dot;
        let lhs = &lhs_q8[lhs_base..lhs_base + blocks_per_dot];
        let mut col = 0usize;
        while col < cols {
            let rhs_base = col * blocks_per_dot * Q8_0_BLOCK_BYTES;
            let rhs = &rhs_q8[rhs_base..rhs_base + blocks_per_dot * Q8_0_BLOCK_BYTES];
            out[row * cols + col] = vec_dot_q8_0_q8_0_with_kernel(rhs, lhs, kernel)?;
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

fn quantize_block_q8_0(input: &[f32], out: &mut BlockQ8Zero) -> Result<(), QKMatmulError> {
    if input.len() != Q8_0_VALUES {
        return Err(QKMatmulError::Length);
    }

    let mut amax = 0.0f32;
    let mut idx = 0usize;
    while idx < Q8_0_VALUES {
        let abs = input[idx].abs();
        if abs > amax {
            amax = abs;
        }
        idx += 1;
    }

    if amax == 0.0 {
        *out = BlockQ8Zero::ZERO;
        return Ok(());
    }

    let d = amax / 127.0f32;
    let id = 1.0f32 / d;
    out.d = f16_bits_to_f32(f32_to_f16_bits(d));
    idx = 0;
    while idx < Q8_0_VALUES {
        let quant = round_away_from_zero(input[idx] * id).clamp(-127, 127);
        out.qs[idx] = quant as i8;
        idx += 1;
    }
    Ok(())
}

fn quantize_block_q8_k(input: &[f32], out: &mut BlockQ8K) -> Result<(), QKMatmulError> {
    if input.len() != Q8_K_VALUES {
        return Err(QKMatmulError::Length);
    }

    let mut max = 0.0f32;
    let mut amax = 0.0f32;
    let mut idx = 0usize;
    while idx < Q8_K_VALUES {
        let value = input[idx];
        let abs = value.abs();
        if abs > amax {
            amax = abs;
            max = value;
        }
        idx += 1;
    }

    if amax == 0.0 {
        *out = BlockQ8K::ZERO;
        return Ok(());
    }

    let iscale = -127.0f32 / max;
    idx = 0;
    while idx < Q8_K_VALUES {
        let quant = nearest_int(iscale * input[idx]).clamp(-127, 127);
        out.qs[idx] = quant as i8;
        idx += 1;
    }

    let mut group = 0usize;
    while group < Q8_K_BSUMS {
        let mut sum = 0i16;
        let mut lane = 0usize;
        while lane < 16 {
            sum += out.qs[group * 16 + lane] as i16;
            lane += 1;
        }
        out.bsums[group] = sum;
        group += 1;
    }
    out.d = 1.0f32 / iscale;
    Ok(())
}

fn vec_dot_block_q4_k_q8_k(q4: &[u8], q8: &BlockQ8K) -> Result<f32, QKMatmulError> {
    if q4.len() != Q4_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let d = f16_bits_to_f32(read_u16_le(q4, 0)) * q8.d;
    let dmin = f16_bits_to_f32(read_u16_le(q4, 2)) * q8.d;
    let scales = &q4[4..4 + Q4_K_SCALE_BYTES];
    let qs = &q4[4 + Q4_K_SCALE_BYTES..Q4_K_BLOCK_BYTES];

    let mut scaled_dot = 0i32;
    let mut min_correction = 0i32;
    let mut sub = 0usize;
    while sub < 8 {
        let (scale, min) =
            q4_k::get_scale_min_k4(sub, scales).map_err(|_| QKMatmulError::Q4KScale)?;
        let mut dot = 0i32;
        let quant_base = (sub / 2) * 32;
        let q8_base = sub * 32;
        let mut lane = 0usize;
        while lane < 32 {
            let packed = qs[quant_base + lane];
            let q4_value = if sub.is_multiple_of(2) {
                packed & 0x0f
            } else {
                packed >> 4
            } as i32;
            dot += q4_value * q8.qs[q8_base + lane] as i32;
            lane += 1;
        }
        scaled_dot += scale as i32 * dot;
        min_correction += min as i32 * q8_bsums_32(q8, sub);
        sub += 1;
    }

    Ok(d * scaled_dot as f32 - dmin * min_correction as f32)
}

fn vec_dot_block_q5_0_q8_0(q5: &[u8], q8: &BlockQ8Zero) -> Result<f32, QKMatmulError> {
    if q5.len() != Q5_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let d = f16_bits_to_f32(read_u16_le(q5, 0)) * q8.d;
    let qh = read_u32_le(q5, 2);
    let qs = &q5[6..Q5_0_BLOCK_BYTES];
    let mut dot = 0i32;
    let mut lane = 0usize;
    while lane < 16 {
        let high = ((qh >> lane) & 1) << 4;
        let quant = ((qs[lane] & 0x0f) as u32 | high) as i32 - 16;
        dot += quant * q8.qs[lane] as i32;
        lane += 1;
    }
    lane = 0;
    while lane < 16 {
        let high = ((qh >> (lane + 16)) & 1) << 4;
        let quant = ((qs[lane] >> 4) as u32 | high) as i32 - 16;
        dot += quant * q8.qs[lane + 16] as i32;
        lane += 1;
    }
    Ok(d * dot as f32)
}

fn vec_dot_block_q5_k_q8_k(q5: &[u8], q8: &BlockQ8K) -> Result<f32, QKMatmulError> {
    if q5.len() != Q5_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let d = f16_bits_to_f32(read_u16_le(q5, 0)) * q8.d;
    let dmin = f16_bits_to_f32(read_u16_le(q5, 2)) * q8.d;
    let scales = &q5[4..4 + Q4_K_SCALE_BYTES];
    let qh = &q5[4 + Q4_K_SCALE_BYTES..4 + Q4_K_SCALE_BYTES + 32];
    let qs = &q5[4 + Q4_K_SCALE_BYTES + 32..Q5_K_BLOCK_BYTES];

    let mut scaled_dot = 0i32;
    let mut min_correction = 0i32;
    let mut sub = 0usize;
    while sub < 8 {
        let (scale, min) =
            q4_k::get_scale_min_k4(sub, scales).map_err(|_| QKMatmulError::Q4KScale)?;
        let quant_base = (sub / 2) * 32;
        let q8_base = sub * 32;
        let high_mask = 1u8 << sub;
        let mut dot = 0i32;
        let mut lane = 0usize;
        while lane < 32 {
            let packed = qs[quant_base + lane];
            let low = if sub.is_multiple_of(2) {
                packed & 0x0f
            } else {
                packed >> 4
            };
            let quant = i32::from(low) + if qh[lane] & high_mask != 0 { 16 } else { 0 };
            dot += quant * q8.qs[q8_base + lane] as i32;
            lane += 1;
        }
        scaled_dot += scale as i32 * dot;
        min_correction += min as i32 * q8_bsums_32(q8, sub);
        sub += 1;
    }

    Ok(d * scaled_dot as f32 - dmin * min_correction as f32)
}

fn vec_dot_block_q6_k_q8_k(q6: &[u8], q8: &BlockQ8K) -> Result<f32, QKMatmulError> {
    if q6.len() != Q6_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let mut unpacked = [0i8; Q6_K_VALUES];
    unpack_q6_k(q6, &mut unpacked)?;
    let scales = &q6[192..208];
    let d = f16_bits_to_f32(read_u16_le(q6, 208)) * q8.d;
    let mut aux32 = [0i32; 8];
    let mut group = 0usize;
    while group < 16 {
        let scale = scales[group] as i8 as i32;
        let base = group * 16;
        let mut lane = 0usize;
        while lane < 16 {
            aux32[lane % 8] += scale * unpacked[base + lane] as i32 * q8.qs[base + lane] as i32;
            lane += 1;
        }
        group += 1;
    }

    let mut sum = 0.0f32;
    let mut lane = 0usize;
    while lane < aux32.len() {
        sum += d * aux32[lane] as f32;
        lane += 1;
    }
    Ok(sum)
}

fn vec_dot_block_q8_0_q8_0(q8_weight: &[u8], q8: &BlockQ8Zero) -> Result<f32, QKMatmulError> {
    if q8_weight.len() != Q8_0_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let d = f16_bits_to_f32(read_u16_le(q8_weight, 0)) * q8.d;
    let qs = &q8_weight[2..Q8_0_BLOCK_BYTES];
    let mut dot = 0i32;
    let mut lane = 0usize;
    while lane < Q8_0_VALUES {
        dot += qs[lane] as i8 as i32 * q8.qs[lane] as i32;
        lane += 1;
    }
    Ok(d * dot as f32)
}

fn unpack_q6_k(q6: &[u8], out: &mut [i8; Q6_K_VALUES]) -> Result<(), QKMatmulError> {
    if q6.len() != Q6_K_BLOCK_BYTES {
        return Err(QKMatmulError::Length);
    }

    let ql = &q6[0..128];
    let qh = &q6[128..192];
    let mut ql_base = 0usize;
    let mut qh_base = 0usize;
    let mut out_base = 0usize;
    while ql_base < 128 {
        let mut lane = 0usize;
        while lane < 32 {
            let high = qh[qh_base + lane];
            let low_a = ql[ql_base + lane];
            let low_b = ql[ql_base + 32 + lane];
            out[out_base + lane] = (((low_a & 0x0f) | ((high & 0x03) << 4)) as i8) - 32;
            out[out_base + lane + 32] = (((low_b & 0x0f) | (((high >> 2) & 0x03) << 4)) as i8) - 32;
            out[out_base + lane + 64] = (((low_a >> 4) | (((high >> 4) & 0x03) << 4)) as i8) - 32;
            out[out_base + lane + 96] = (((low_b >> 4) | (((high >> 6) & 0x03) << 4)) as i8) - 32;
            lane += 1;
        }
        ql_base += 64;
        qh_base += 32;
        out_base += 128;
    }
    Ok(())
}

fn q8_bsums_32(q8: &BlockQ8K, sub: usize) -> i32 {
    q8.bsums[sub * 2] as i32 + q8.bsums[sub * 2 + 1] as i32
}

#[cfg(all(target_arch = "x86_64", any(test, feature = "phase9-avx2-matmul")))]
mod avx2 {
    use core::arch::x86_64::{
        __cpuid, __cpuid_count, __m128i, __m256i, _mm_loadu_si128, _mm256_add_epi32, _mm256_add_ps,
        _mm256_and_si256, _mm256_castps_si256, _mm256_cvtepi8_epi16, _mm256_cvtepi16_epi32,
        _mm256_cvtepi32_ps, _mm256_loadu_ps, _mm256_loadu_si256, _mm256_madd_epi16,
        _mm256_maddubs_epi16, _mm256_max_epi32, _mm256_min_epi32, _mm256_mul_ps, _mm256_set1_epi8,
        _mm256_set1_epi16, _mm256_set1_epi32, _mm256_set1_ps, _mm256_setzero_si256,
        _mm256_srli_epi16, _mm256_srli_epi32, _mm256_storeu_ps, _mm256_storeu_si256,
        _mm256_sub_epi32, _mm256_sub_ps, _xgetbv,
    };

    use super::{
        BlockQ8K, BlockQ8Zero, Q4_K_BLOCK_BYTES, Q4_K_SCALE_BYTES, Q4_K_VALUES, Q5_0_BLOCK_BYTES,
        Q6_K_BLOCK_BYTES, Q8_0_BLOCK_BYTES, Q8_0_VALUES, QKMatmulError, f16_bits_to_f32,
    };
    use super::{q4_k, q8_bsums_32, read_u16_le, read_u32_le};

    const CPUID_ECX_OSXSAVE: u32 = 1 << 27;
    const CPUID_ECX_AVX: u32 = 1 << 28;
    const CPUID_EBX_AVX2: u32 = 1 << 5;
    const XCR0_XMM_YMM: u64 = 0b110;

    /// The census decision once one exists, else this CPU's own check.
    pub(super) fn runtime_available() -> bool {
        match super::AVX2_CENSUS.decision() {
            super::Avx2Decision::No => false,
            super::Avx2Decision::Yes => true,
            super::Avx2Decision::Undecided => runtime_available_uncached(),
        }
    }

    /// CPUID AVX/AVX2 + OSXSAVE, and XCR0 XMM|YMM enabled on this CPU.
    pub(super) fn runtime_available_uncached() -> bool {
        let leaf1 = __cpuid(1);
        if leaf1.ecx & (CPUID_ECX_OSXSAVE | CPUID_ECX_AVX) != CPUID_ECX_OSXSAVE | CPUID_ECX_AVX {
            return false;
        }

        // SAFETY: CPUID reported OSXSAVE, so XGETBV is valid. Requiring both
        // XMM and YMM bits prevents executing AVX2 when the OS/kernel has not
        // enabled extended vector-state management.
        let xcr0 = unsafe { _xgetbv(0) };
        if xcr0 & XCR0_XMM_YMM != XCR0_XMM_YMM {
            return false;
        }

        let leaf7 = __cpuid_count(7, 0);
        leaf7.ebx & CPUID_EBX_AVX2 != 0
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn vec_dot_q5_0_q8_0(
        q5: &[u8],
        q8: &[BlockQ8Zero],
    ) -> Result<f32, QKMatmulError> {
        if q5.len() != q8.len() * Q5_0_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        let mut sum = 0.0f32;
        let mut block_idx = 0usize;
        while block_idx < q8.len() {
            let off = block_idx * Q5_0_BLOCK_BYTES;
            // SAFETY: Same AVX2/XCR0 precondition was required to enter this
            // function and therefore also holds for the per-block helper.
            sum += unsafe {
                vec_dot_block_q5_0_q8_0(&q5[off..off + Q5_0_BLOCK_BYTES], &q8[block_idx])
            }?;
            block_idx += 1;
        }
        Ok(sum)
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q5_0_q8_0`.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn matmul_q5_0_q8_0(
        lhs_q8: &[BlockQ8Zero],
        rhs_q5: &[u8],
        rows: usize,
        cols: usize,
        blocks_per_dot: usize,
        out: &mut [f32],
    ) -> Result<(), QKMatmulError> {
        let mut row = 0usize;
        while row < rows {
            let lhs_base = row * blocks_per_dot;
            let lhs = &lhs_q8[lhs_base..lhs_base + blocks_per_dot];
            let mut col = 0usize;
            while col < cols {
                let rhs_base = col * blocks_per_dot * Q5_0_BLOCK_BYTES;
                let rhs = &rhs_q5[rhs_base..rhs_base + blocks_per_dot * Q5_0_BLOCK_BYTES];
                // SAFETY: The caller proved AVX2/XCR0 once for the whole
                // matmul. Slice bounds follow from the public wrapper's shape
                // validation.
                out[row * cols + col] = unsafe { vec_dot_q5_0_q8_0(rhs, lhs) }?;
                col += 1;
            }
            row += 1;
        }
        Ok(())
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn vec_dot_q6_k_q8_k(
        q6: &[u8],
        q8: &[BlockQ8K],
    ) -> Result<f32, QKMatmulError> {
        if q6.len() != q8.len() * Q6_K_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }
        super::vec_dot_q6_k_q8_k(q6, q8)
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn vec_dot_q8_0_q8_0(
        q8_weight: &[u8],
        q8: &[BlockQ8Zero],
    ) -> Result<f32, QKMatmulError> {
        if q8_weight.len() != q8.len() * Q8_0_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        let mut sum = 0.0f32;
        let mut block_idx = 0usize;
        while block_idx < q8.len() {
            let off = block_idx * Q8_0_BLOCK_BYTES;
            // SAFETY: Same AVX2/XCR0 precondition was required to enter this
            // function and therefore also holds for the per-block helper.
            sum += unsafe {
                vec_dot_block_q8_0_q8_0(&q8_weight[off..off + Q8_0_BLOCK_BYTES], &q8[block_idx])
            }?;
            block_idx += 1;
        }
        Ok(sum)
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    unsafe fn vec_dot_block_q5_0_q8_0(q5: &[u8], q8: &BlockQ8Zero) -> Result<f32, QKMatmulError> {
        if q5.len() != Q5_0_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        let d = f16_bits_to_f32(read_u16_le(q5, 0)) * q8.d;
        let qh = read_u32_le(q5, 2);
        let qs = &q5[6..Q5_0_BLOCK_BYTES];
        let mut unpacked = [0i8; Q8_0_VALUES];
        let mut lane = 0usize;
        while lane < 16 {
            let high = ((qh >> lane) & 1) << 4;
            unpacked[lane] = (((qs[lane] & 0x0f) as u32 | high) as i32 - 16) as i8;
            lane += 1;
        }
        lane = 0;
        while lane < 16 {
            let high = ((qh >> (lane + 16)) & 1) << 4;
            unpacked[lane + 16] = (((qs[lane] >> 4) as u32 | high) as i32 - 16) as i8;
            lane += 1;
        }
        // SAFETY: Both arrays contain exactly 32 i8 lanes. AVX2/XCR0 was
        // proven by the caller.
        let dot = unsafe { dot_i8x32(unpacked.as_ptr(), q8.qs.as_ptr()) };
        Ok(d * dot as f32)
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    unsafe fn vec_dot_block_q8_0_q8_0(
        q8_weight: &[u8],
        q8: &BlockQ8Zero,
    ) -> Result<f32, QKMatmulError> {
        if q8_weight.len() != Q8_0_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        let d = f16_bits_to_f32(read_u16_le(q8_weight, 0)) * q8.d;
        // SAFETY: The validated block has 32 signed lanes after its 2-byte
        // f16 scale. `q8.qs` has the same length. AVX2/XCR0 was proven.
        let dot = unsafe { dot_i8x32(q8_weight.as_ptr().add(2).cast(), q8.qs.as_ptr()) };
        Ok(d * dot as f32)
    }

    /// SAFETY: The caller must prove AVX2 is available and both pointers name
    /// at least 32 readable i8 lanes.
    #[target_feature(enable = "avx2")]
    unsafe fn dot_i8x32(lhs: *const i8, rhs: *const i8) -> i32 {
        // SAFETY: The caller provides 32 readable lanes for both inputs.
        let lhs_lo = unsafe { _mm_loadu_si128(lhs.cast::<__m128i>()) };
        let rhs_lo = unsafe { _mm_loadu_si128(rhs.cast::<__m128i>()) };
        let lhs_hi = unsafe { _mm_loadu_si128(lhs.add(16).cast::<__m128i>()) };
        let rhs_hi = unsafe { _mm_loadu_si128(rhs.add(16).cast::<__m128i>()) };
        let lhs_lo_wide = _mm256_cvtepi8_epi16(lhs_lo);
        let rhs_lo_wide = _mm256_cvtepi8_epi16(rhs_lo);
        let lhs_hi_wide = _mm256_cvtepi8_epi16(lhs_hi);
        let rhs_hi_wide = _mm256_cvtepi8_epi16(rhs_hi);
        let prod_lo = _mm256_madd_epi16(lhs_lo_wide, rhs_lo_wide);
        let prod_hi = _mm256_madd_epi16(lhs_hi_wide, rhs_hi_wide);
        let accum = _mm256_add_epi32(prod_lo, prod_hi);
        // SAFETY: Same AVX2/XCR0 precondition as this function.
        unsafe { horizontal_sum_i32x8(accum) }
    }

    /// SAFETY: The caller must prove AVX2 is available and that XCR0 enables
    /// XMM/YMM state before entering this target-feature body. The public
    /// selector does that with CPUID + XGETBV and falls back to scalar.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn vec_dot_q4_k_q8_k(
        q4: &[u8],
        q8: &[BlockQ8K],
    ) -> Result<f32, QKMatmulError> {
        if q4.len() != q8.len() * Q4_K_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        let mut sum = 0.0f32;
        let mut block_idx = 0usize;
        while block_idx < q8.len() {
            let off = block_idx * Q4_K_BLOCK_BYTES;
            // SAFETY: Same AVX2/XCR0 precondition was required to enter this
            // function and therefore also holds for the per-block helper.
            sum += unsafe {
                vec_dot_block_q4_k_q8_k(&q4[off..off + Q4_K_BLOCK_BYTES], &q8[block_idx])
            }?;
            block_idx += 1;
        }
        Ok(sum)
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    unsafe fn vec_dot_block_q4_k_q8_k(q4: &[u8], q8: &BlockQ8K) -> Result<f32, QKMatmulError> {
        if q4.len() != Q4_K_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        let d = f16_bits_to_f32(read_u16_le(q4, 0)) * q8.d;
        let dmin = f16_bits_to_f32(read_u16_le(q4, 2)) * q8.d;
        let scales = &q4[4..4 + Q4_K_SCALE_BYTES];
        let qs = &q4[4 + Q4_K_SCALE_BYTES..Q4_K_BLOCK_BYTES];

        let low_mask = _mm256_set1_epi8(0x0f);
        let mut accum = _mm256_setzero_si256();
        let mut min_correction = 0i32;
        let mut sub = 0usize;
        while sub < 8 {
            let (scale, min) =
                q4_k::get_scale_min_k4(sub, scales).map_err(|_| QKMatmulError::Q4KScale)?;
            let quant_base = (sub / 2) * 32;
            let q8_base = sub * 32;

            // SAFETY: `quant_base` and `q8_base` select exact 32-byte chunks
            // inside the validated 128-byte Q4_K quant array and 256-byte Q8_K
            // quant array. `loadu` permits unaligned addresses.
            let packed = unsafe { _mm256_loadu_si256(qs.as_ptr().add(quant_base).cast()) };
            let q8_vec = unsafe { _mm256_loadu_si256(q8.qs.as_ptr().add(q8_base).cast()) };
            let q4_vec = if sub.is_multiple_of(2) {
                _mm256_and_si256(packed, low_mask)
            } else {
                let shifted = _mm256_srli_epi16(packed, 4);
                _mm256_and_si256(shifted, low_mask)
            };
            // The q4 lane is unsigned 0..15 and q8 is signed -127..127, so
            // each adjacent-pair maddubs result is within +/-3810 and cannot
            // saturate i16. Scale is 6-bit, so widening to i32 remains bounded.
            let pair_sums = _mm256_maddubs_epi16(q4_vec, q8_vec);
            let scale_vec = _mm256_set1_epi16(scale as i16);
            let scaled = _mm256_madd_epi16(pair_sums, scale_vec);
            accum = _mm256_add_epi32(accum, scaled);

            min_correction += min as i32 * q8_bsums_32(q8, sub);
            sub += 1;
        }

        // SAFETY: Same AVX2/XCR0 precondition as this function.
        let scaled_dot = unsafe { horizontal_sum_i32x8(accum) };
        Ok(d * scaled_dot as f32 - dmin * min_correction as f32)
    }

    /// SAFETY: Same AVX2/XCR0 precondition as `vec_dot_q4_k_q8_k`.
    #[target_feature(enable = "avx2")]
    unsafe fn horizontal_sum_i32x8(value: __m256i) -> i32 {
        let mut lanes = [0i32; 8];
        // SAFETY: `lanes` is a 32-byte local buffer and `storeu` permits
        // unaligned destinations.
        unsafe {
            _mm256_storeu_si256(lanes.as_mut_ptr().cast::<__m256i>(), value);
        }
        let mut sum = 0i32;
        let mut idx = 0usize;
        while idx < lanes.len() {
            sum += lanes[idx];
            idx += 1;
        }
        sum
    }

    /// AVX2-optimized quantization of a single Q8_K block, bit-identical to the
    /// scalar `super::quantize_block_q8_k` oracle.
    ///
    /// Mirrors the scalar exactly: (1) find the signed element with the
    /// largest absolute value — done in scalar to preserve the oracle's
    /// first-wins tie-break (`if abs > amax`, strict `>`, so the lowest index
    /// wins on equal magnitudes; SIMD reductions do not give first-wins); (2)
    /// `iscale = -127.0 / max`; (3) quantize each element with the scalar's
    /// exact magic-number `nearest_int` (`x + 12582912.0`, mask the low 23
    /// bits, re-center by `0x0040_0000`) so IEEE-754 round-to-nearest-even tie
    /// handling is reproduced lane-for-lane — `_mm256_cvtps_epi32` is NOT used
    /// because its rounding can diverge from the magic-number trick at
    /// half-integer ties; (4) clamp to [-127, 127]; (5) compute the 16 group
    /// `bsums`. The degenerate `amax == 0.0` block writes `BlockQ8K::ZERO`
    /// with no division.
    ///
    /// SAFETY: The caller must prove AVX2 is available and XCR0 enables
    /// XMM/YMM state before entering this target-feature body, exactly as the
    /// public selector does via `runtime_available()`.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn quantize_block_q8_k(
        input: &[f32],
        out: &mut BlockQ8K,
    ) -> Result<(), QKMatmulError> {
        if input.len() != super::Q8_K_VALUES {
            return Err(QKMatmulError::Length);
        }

        // Phase 1 — signed max-abs selection. Scalar, to preserve the oracle's
        // first-wins tie-break exactly. 256 elements is negligible next to the
        // vectorized quantize + bsums phases below.
        let mut max_val = 0.0f32;
        let mut amax = 0.0f32;
        let mut idx = 0usize;
        while idx < super::Q8_K_VALUES {
            let value = input[idx];
            let abs = value.abs();
            if abs > amax {
                amax = abs;
                max_val = value;
            }
            idx += 1;
        }

        if amax == 0.0 {
            *out = BlockQ8K::ZERO;
            return Ok(());
        }

        let iscale = -127.0f32 / max_val;

        // Phase 2 — vectorized quantize with the scalar's exact magic-number
        // `nearest_int`. `magic = 12582912.0` (= 1.5 * 2^23) biases the float
        // so the rounded integer lands in the low 23 mantissa bits; masking
        // and re-centering extracts it. Per-lane f32 mul/add are IEEE-754
        // round-to-nearest-even, identical to the scalar product + bias, so
        // half-integer ties round the same direction. No FMA: the scalar is a
        // separate mul then add, and `_mm256_fmadd_ps` would fuse and diverge.
        let iscale_vec = _mm256_set1_ps(iscale);
        let magic_vec = _mm256_set1_ps(12_582_912.0f32);
        let mant_mask = _mm256_set1_epi32(0x007f_ffffi32);
        let recenter = _mm256_set1_epi32(0x0040_0000i32);
        let lo_clamp = _mm256_set1_epi32(-127);
        let hi_clamp = _mm256_set1_epi32(127);

        let mut base = 0usize;
        while base < super::Q8_K_VALUES {
            // SAFETY: `base` advances by 8 and stops at Q8_K_VALUES (256), so
            // the 8-element load stays in bounds. `loadu` permits unaligned
            // addresses. AVX2/XCR0 was proven by the caller.
            let in_vec = unsafe { _mm256_loadu_ps(input.as_ptr().add(base)) };
            let prod = _mm256_mul_ps(in_vec, iscale_vec);
            let biased = _mm256_add_ps(prod, magic_vec);
            let bits = _mm256_castps_si256(biased);
            let mant = _mm256_and_si256(bits, mant_mask);
            let quant = _mm256_sub_epi32(mant, recenter);
            let clamped = _mm256_min_epi32(_mm256_max_epi32(quant, lo_clamp), hi_clamp);

            let mut lanes = [0i32; 8];
            // SAFETY: `lanes` is a 32-byte local buffer; `storeu` permits
            // unaligned destinations. AVX2/XCR0 was proven by the caller.
            unsafe { _mm256_storeu_si256(lanes.as_mut_ptr().cast::<__m256i>(), clamped) };
            let mut lane = 0usize;
            while lane < 8 {
                // Values are clamped to [-127, 127], so the truncating `as i8`
                // cast matches the scalar `quant as i8` exactly.
                out.qs[base + lane] = lanes[lane] as i8;
                lane += 1;
            }
            base += 8;
        }

        // Phase 3 — vectorized bsums: 16 groups of 16 signed i8, each summed
        // into an i16. 16 * 127 = 2032, which fits in i16 (max 32767), so no
        // overflow. Reads back the `qs` written in phase 2, matching scalar.
        let mut group = 0usize;
        while group < super::Q8_K_BSUMS {
            let gbase = group * 16;
            // SAFETY: `gbase` advances by 16 and stops at Q8_K_VALUES, so the
            // 16-byte load stays in bounds. AVX2/XCR0 was proven by the caller.
            let narrow = unsafe { _mm_loadu_si128(out.qs.as_ptr().add(gbase).cast::<__m128i>()) };
            let wide = _mm256_cvtepi8_epi16(narrow);
            let mut lanes = [0i16; 16];
            // SAFETY: `lanes` is a 32-byte local buffer; `storeu` permits
            // unaligned destinations. AVX2/XCR0 was proven by the caller.
            unsafe { _mm256_storeu_si256(lanes.as_mut_ptr().cast::<__m256i>(), wide) };
            let mut sum = 0i16;
            let mut lane = 0usize;
            while lane < 16 {
                sum += lanes[lane];
                lane += 1;
            }
            out.bsums[group] = sum;
            group += 1;
        }

        out.d = 1.0f32 / iscale;
        Ok(())
    }

    /// AVX2-optimized dequantization of a single Q4_K block, bit-identical to
    /// the scalar `super::q4_k::dequantize_q4_k` oracle.
    ///
    /// Mirrors the scalar exactly: (1) `d`/`dmin` via `f16_bits_to_f32` (scalar
    /// — once per block, reusing the same function the scalar uses); (2)
    /// `get_scale_min_k4` for the 6-bit `(sc, m)` pairs (**scalar** — 8
    /// indices/block, tiny next to the vectorized dequant); (3) per 64-value
    /// sub-block, `d1 = d * sc1`, `min1 = dmin * m1` (native f32 broadcast),
    /// then vectorize the 32 low-nibble lanes `out = d1 * (qs & 0x0f) - min1`
    /// and the 32 high-nibble lanes `out = d2 * (qs >> 4) - min2`.
    ///
    /// **NO FMA:** the scalar is `d1 * q - min1` = a separate multiply *then*
    /// subtract; `_mm256_fmsub_ps` fuses and diverges — uses `_mm256_mul_ps`
    /// then `_mm256_sub_ps`. Nibble→f32 via `_mm256_cvtepi32_ps` (exact for
    /// 0..15). Lane order preserved (32 low lanes then 32 high lanes per
    /// sub-block, matching the scalar's two inner loops).
    ///
    /// SAFETY: The caller must prove AVX2 is available and XCR0 enables
    /// XMM/YMM state before entering this target-feature body, exactly as the
    /// public selector does via `runtime_available()`.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn dequantize_q4_k(
        block: &[u8],
    ) -> Result<[f32; Q4_K_VALUES], QKMatmulError> {
        if block.len() != Q4_K_BLOCK_BYTES {
            return Err(QKMatmulError::Length);
        }

        // Scalar d/dmin — reuse the exact same f16 conversion the oracle uses.
        let d = f16_bits_to_f32(read_u16_le(block, 0));
        let dmin = f16_bits_to_f32(read_u16_le(block, 2));
        let scales = &block[4..4 + Q4_K_SCALE_BYTES];
        let qs = &block[4 + Q4_K_SCALE_BYTES..Q4_K_BLOCK_BYTES];

        let mut out = [0.0f32; Q4_K_VALUES];

        let nibble_mask = _mm256_set1_epi32(0x0f);
        let mut scale_idx = 0usize;
        let mut quant_idx = 0usize;
        let mut out_idx = 0usize;
        while out_idx < Q4_K_VALUES {
            let (sc1, m1) =
                q4_k::get_scale_min_k4(scale_idx, scales).map_err(|_| QKMatmulError::Q4KScale)?;
            let (sc2, m2) = q4_k::get_scale_min_k4(scale_idx + 1, scales)
                .map_err(|_| QKMatmulError::Q4KScale)?;
            let d1 = d * sc1 as f32;
            let min1 = dmin * m1 as f32;
            let d2 = d * sc2 as f32;
            let min2 = dmin * m2 as f32;
            let d1_vec = _mm256_set1_ps(d1);
            let min1_vec = _mm256_set1_ps(min1);
            let d2_vec = _mm256_set1_ps(d2);
            let min2_vec = _mm256_set1_ps(min2);

            // The scalar processes 32 quant bytes per sub-block: 32 low-nibble
            // lanes then 32 high-nibble lanes. We load 32 bytes as two 16-byte
            // halves, widen each to i16x16 → two i32x8, extract nibbles, and
            // write 4×8 low lanes then 4×8 high lanes.
            let mut out_buf = [0f32; 8];
            let mut low_results: [[f32; 8]; 4] = [[0.0; 8]; 4];
            let mut high_results: [[f32; 8]; 4] = [[0.0; 8]; 4];

            let mut half = 0usize;
            while half < 2 {
                // SAFETY: `quant_idx + half*16` stays in bounds (quant_idx
                // advances by 32, qs has 128 bytes). AVX2/XCR0 proven by caller.
                let packed128 = unsafe {
                    _mm_loadu_si128(qs.as_ptr().add(quant_idx + half * 16).cast::<__m128i>())
                };
                let wide16 = _mm256_cvtepi8_epi16(packed128);
                let mut lanes16 = [0i16; 16];
                // SAFETY: 32-byte local buffer; storeu permits unaligned.
                unsafe { _mm256_storeu_si256(lanes16.as_mut_ptr().cast::<__m256i>(), wide16) };
                let lo128 = unsafe { _mm_loadu_si128(lanes16.as_ptr().cast::<__m128i>()) };
                let hi128 = unsafe { _mm_loadu_si128(lanes16.as_ptr().add(8).cast::<__m128i>()) };
                let lo32 = _mm256_cvtepi16_epi32(lo128);
                let hi32 = _mm256_cvtepi16_epi32(hi128);

                let lo_low = _mm256_and_si256(lo32, nibble_mask);
                let lo_high = _mm256_and_si256(_mm256_srli_epi32(lo32, 4), nibble_mask);
                let hi_low = _mm256_and_si256(hi32, nibble_mask);
                let hi_high = _mm256_and_si256(_mm256_srli_epi32(hi32, 4), nibble_mask);

                let group = half * 2;
                let ll = _mm256_cvtepi32_ps(lo_low);
                let lr = _mm256_sub_ps(_mm256_mul_ps(d1_vec, ll), min1_vec);
                // SAFETY: 32-byte local buffer; storeu permits unaligned.
                unsafe { _mm256_storeu_ps(out_buf.as_mut_ptr(), lr) };
                low_results[group] = out_buf;
                let hl = _mm256_cvtepi32_ps(hi_low);
                let hr = _mm256_sub_ps(_mm256_mul_ps(d1_vec, hl), min1_vec);
                // SAFETY: same as above.
                unsafe { _mm256_storeu_ps(out_buf.as_mut_ptr(), hr) };
                low_results[group + 1] = out_buf;
                let lh = _mm256_cvtepi32_ps(lo_high);
                let lhr = _mm256_sub_ps(_mm256_mul_ps(d2_vec, lh), min2_vec);
                // SAFETY: same as above.
                unsafe { _mm256_storeu_ps(out_buf.as_mut_ptr(), lhr) };
                high_results[group] = out_buf;
                let hh = _mm256_cvtepi32_ps(hi_high);
                let hhr = _mm256_sub_ps(_mm256_mul_ps(d2_vec, hh), min2_vec);
                // SAFETY: same as above.
                unsafe { _mm256_storeu_ps(out_buf.as_mut_ptr(), hhr) };
                high_results[group + 1] = out_buf;

                half += 1;
            }

            // Write low-nibble lanes (0..31) then high-nibble lanes (32..63).
            let mut g = 0;
            while g < 4 {
                let mut lane = 0;
                while lane < 8 {
                    out[out_idx + g * 8 + lane] = low_results[g][lane];
                    lane += 1;
                }
                g += 1;
            }
            g = 0;
            while g < 4 {
                let mut lane = 0;
                while lane < 8 {
                    out[out_idx + 32 + g * 8 + lane] = high_results[g][lane];
                    lane += 1;
                }
                g += 1;
            }

            quant_idx += 32;
            scale_idx += 2;
            out_idx += 64;
        }

        Ok(out)
    }
}

fn nearest_int(fval: f32) -> i32 {
    let val = fval + 12_582_912.0f32;
    ((val.to_bits() & 0x007f_ffff) as i32) - 0x0040_0000
}

fn round_away_from_zero(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let frac = bits & 0x007f_ffff;

    if exp == 0xff {
        if frac == 0 {
            return sign | 0x7c00;
        }
        return sign | 0x7e00;
    }

    let half_exp = exp - 127 + 15;
    if half_exp >= 0x1f {
        return sign | 0x7c00;
    }

    if half_exp <= 0 {
        if half_exp < -10 {
            return sign;
        }
        let mant = frac | 0x0080_0000;
        let shift = (14 - half_exp) as u32;
        let mut half = (mant >> shift) as u16;
        let round_bit = 1u32 << (shift - 1);
        let remainder = mant & (round_bit - 1);
        if mant & round_bit != 0 && (remainder != 0 || half & 1 != 0) {
            half += 1;
        }
        return sign | half;
    }

    let mut half = sign | ((half_exp as u16) << 10) | ((frac >> 13) as u16);
    let round = frac & 0x1fff;
    if round > 0x1000 || (round == 0x1000 && half & 1 != 0) {
        half += 1;
    }
    half
}

fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exp = ((bits & 0x7c00) >> 10) as u32;
    let frac = (bits & 0x03ff) as u32;
    let out = if exp == 0 {
        if frac == 0 {
            sign
        } else {
            let mut mant = frac;
            let mut exp2 = -14i32;
            while mant & 0x0400 == 0 {
                mant <<= 1;
                exp2 -= 1;
            }
            mant &= 0x03ff;
            sign | (((exp2 + 127) as u32) << 23) | (mant << 13)
        }
    } else if exp == 0x1f {
        sign | 0x7f80_0000 | (frac << 13)
    } else {
        sign | ((exp + 112) << 23) | (frac << 13)
    };
    f32::from_bits(out)
}
