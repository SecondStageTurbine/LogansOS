// SPDX-License-Identifier: MPL-2.0
//! Pure CPU op kernels for Phase 5 BUILD-3 inference.

extern crate alloc;

use alloc::vec;

pub mod gdn;
pub mod q4_k;
#[cfg(any(
    test,
    feature = "phase9-quantized-matmul",
    feature = "phase9-real-quant"
))]
pub mod qk_matmul;
pub mod soft_f32;

use super::tensor_logic::{Dtype, Shape, TensorError};

#[derive(Copy, Clone)]
pub struct TensorView<'a> {
    pub dtype: Dtype,
    pub shape: &'a Shape,
    pub bytes: &'a [u8],
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum OpError {
    Dtype,
    Shape,
    Index,
    Input,
    Output,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum OpKind {
    Identity = 0,
    Add = 1,
    Mul = 2,
    Matmul = 3,
    Relu = 4,
    Gelu = 5,
    Softmax = 6,
    LayerNorm = 7,
    RmsNorm = 8,
    Embedding = 9,
    Reshape = 10,
    Transpose = 11,
    Silu = 12,
    Swiglu = 13,
    Rope = 14,
    GqaAttention = 15,
}

impl OpKind {
    pub const fn from_abi(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Identity),
            1 => Some(Self::Add),
            2 => Some(Self::Mul),
            3 => Some(Self::Matmul),
            4 => Some(Self::Relu),
            5 => Some(Self::Gelu),
            6 => Some(Self::Softmax),
            7 => Some(Self::LayerNorm),
            8 => Some(Self::RmsNorm),
            9 => Some(Self::Embedding),
            10 => Some(Self::Reshape),
            11 => Some(Self::Transpose),
            12 => Some(Self::Silu),
            13 => Some(Self::Swiglu),
            14 => Some(Self::Rope),
            15 => Some(Self::GqaAttention),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RopeYarnParams {
    pub position: u64,
    pub n_dims: u64,
    pub theta_scale: u32,
    pub freq_scale: u32,
    pub ext_factor: u32,
    pub attn_factor: u32,
    pub corr_dim_low: u32,
    pub corr_dim_high: u32,
}

pub const ROPE_YARN_PARAM_COUNT: usize = 8;

#[derive(Copy, Clone, Debug)]
pub struct GqaAttentionParams {
    pub seq_len: u64,
    pub n_q_heads: u64,
    pub n_kv_heads: u64,
    pub head_dim: u64,
}

pub const GQA_ATTENTION_PARAM_COUNT: usize = 4;

pub fn dispatch(
    op: OpKind,
    inputs: &[TensorView<'_>],
    output_shape: &Shape,
    out: &mut [u8],
) -> Result<(), OpError> {
    match op {
        OpKind::Identity => {
            require_input_count(inputs, 1)?;
            identity(&inputs[0], out)
        }
        OpKind::Reshape => {
            require_input_count(inputs, 1)?;
            reshape(&inputs[0], output_shape, out)
        }
        OpKind::Transpose => {
            require_input_count(inputs, 1)?;
            transpose(&inputs[0], out)
        }
        OpKind::Add => {
            require_input_count(inputs, 2)?;
            add(&inputs[0], &inputs[1], out)
        }
        OpKind::Mul => {
            require_input_count(inputs, 2)?;
            mul(&inputs[0], &inputs[1], out)
        }
        OpKind::Matmul => {
            require_input_count(inputs, 2)?;
            matmul(&inputs[0], &inputs[1], out)
        }
        OpKind::Relu => {
            require_input_count(inputs, 1)?;
            relu(&inputs[0], out)
        }
        OpKind::Softmax => {
            require_input_count(inputs, 1)?;
            softmax(&inputs[0], out)
        }
        OpKind::Gelu => {
            require_input_count(inputs, 1)?;
            gelu(&inputs[0], out)
        }
        OpKind::Silu => {
            require_input_count(inputs, 1)?;
            silu(&inputs[0], out)
        }
        OpKind::Swiglu => {
            require_input_count(inputs, 2)?;
            swiglu(&inputs[0], &inputs[1], out)
        }
        OpKind::Rope => {
            require_input_count(inputs, 2)?;
            let params = rope_yarn_params(&inputs[1])?;
            rope_neox_yarn(&inputs[0], params, out)
        }
        OpKind::GqaAttention => {
            require_input_count(inputs, 4)?;
            let params = gqa_attention_params(&inputs[3])?;
            gqa_causal_attention(&inputs[0], &inputs[1], &inputs[2], params, out)
        }
        OpKind::LayerNorm => match inputs.len() {
            1 => layer_norm(&inputs[0], out),
            3 => layer_norm_affine(&inputs[0], &inputs[1], &inputs[2], out),
            _ => Err(OpError::Input),
        },
        OpKind::RmsNorm => match inputs.len() {
            1 => rms_norm(&inputs[0], out),
            2 => rms_norm_affine(&inputs[0], &inputs[1], out),
            _ => Err(OpError::Input),
        },
        OpKind::Embedding => {
            require_input_count(inputs, 2)?;
            embedding(&inputs[0], &inputs[1], out)
        }
    }
}

fn require_input_count(inputs: &[TensorView<'_>], expected: usize) -> Result<(), OpError> {
    if inputs.len() == expected {
        Ok(())
    } else {
        Err(OpError::Input)
    }
}

pub fn identity(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    require_f32(input)?;
    require_output_len(out, input.shape, input.dtype)?;
    copy_bytes(input.bytes, out)?;
    Ok(())
}

pub fn reshape(
    input: &TensorView<'_>,
    output_shape: &Shape,
    out: &mut [u8],
) -> Result<(), OpError> {
    require_f32(input)?;
    if input.shape.num_elements().map_err(tensor_err)?
        != output_shape.num_elements().map_err(tensor_err)?
    {
        return Err(OpError::Shape);
    }
    require_output_len(out, output_shape, input.dtype)?;
    copy_bytes(input.bytes, out)?;
    Ok(())
}

pub fn transpose(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    require_f32(input)?;
    if input.shape.rank() != 2 {
        return Err(OpError::Shape);
    }
    let rows64 = input.shape.dim(0);
    let cols64 = input.shape.dim(1);
    let rows = checked_usize(rows64)?;
    let cols = checked_usize(cols64)?;
    require_output_len_2d(out, cols64, rows64, input.dtype)?;
    let mut r = 0usize;
    while r < rows {
        let mut c = 0usize;
        while c < cols {
            let src = r * cols + c;
            let dst = c * rows + r;
            write_bits(out, dst, read_bits(input.bytes, src)?);
            c += 1;
        }
        r += 1;
    }
    Ok(())
}

pub fn add(lhs: &TensorView<'_>, rhs: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    elementwise_binary(lhs, rhs, out, soft_f32::add)
}

pub fn mul(lhs: &TensorView<'_>, rhs: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    elementwise_binary(lhs, rhs, out, soft_f32::mul)
}

pub fn silu(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(input)?;
    require_output_len(out, input.shape, Dtype::F32)?;
    let count = element_count(input.shape)?;
    let mut idx = 0usize;
    while idx < count {
        write_bits(out, idx, silu_bits(read_bits(input.bytes, idx)?));
        idx += 1;
    }
    Ok(())
}

pub fn swiglu(gate: &TensorView<'_>, up: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_same_f32_shape(gate, up)?;
    require_output_len(out, gate.shape, Dtype::F32)?;
    let count = element_count(gate.shape)?;
    let mut idx = 0usize;
    while idx < count {
        let activated = silu_bits(read_bits(gate.bytes, idx)?);
        write_bits(
            out,
            idx,
            soft_f32::mul(activated, read_bits(up.bytes, idx)?),
        );
        idx += 1;
    }
    Ok(())
}

/// `swiglu` on `f32` slices, for `forward_logic`'s `par-elementwise` path (lever 2,
/// Plans/NOTE-2026-10-25-arthur-lever2-design.md): the same `silu_bits` and
/// `soft_f32::mul` per element, so the output bits are `swiglu`'s. Each element
/// depends only on its own `gate[i]` and `up[i]`, so any split of the index
/// range into disjoint sub-slices gives the same bytes.
#[allow(dead_code)] // called only under `par-elementwise`
pub fn swiglu_f32(gate: &[f32], up: &[f32], out: &mut [f32]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    if gate.len() != up.len() || out.len() != gate.len() {
        return Err(OpError::Shape);
    }
    for ((o, g), u) in out.iter_mut().zip(gate).zip(up) {
        *o = f32::from_bits(soft_f32::mul(silu_bits(g.to_bits()), u.to_bits()));
    }
    Ok(())
}

pub fn rope_neox_yarn(
    input: &TensorView<'_>,
    params: RopeYarnParams,
    out: &mut [u8],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(input)?;
    if input.shape.rank() < 1 || params.n_dims == 0 || !params.n_dims.is_multiple_of(2) {
        return Err(OpError::Shape);
    }
    if is_zero_f32(params.freq_scale) {
        return Err(OpError::Input);
    }

    let rank = input.shape.rank();
    let last_dim64 = input.shape.dim((rank - 1) as usize);
    if params.n_dims > last_dim64 {
        return Err(OpError::Shape);
    }
    require_output_len(out, input.shape, Dtype::F32)?;
    copy_bytes(input.bytes, out)?;

    let last_dim = checked_usize(last_dim64)?;
    let n_dims = checked_usize(params.n_dims)?;
    let pair_offset = n_dims / 2;
    let total = element_count(input.shape)?;
    let rows = total / last_dim;
    let position_i32 = i32::try_from(params.position).map_err(|_| OpError::Input)?;

    let mut row = 0usize;
    while row < rows {
        let base = row * last_dim;
        let mut theta = soft_f32::int_to_f32(position_i32);
        let mut pair = 0usize;
        while pair < pair_offset {
            let (cos_theta, sin_theta) = rope_yarn_theta(theta, pair, params);
            let x0 = read_bits(input.bytes, base + pair)?;
            let x1 = read_bits(input.bytes, base + pair_offset + pair)?;
            let rotated0 =
                soft_f32::sub(soft_f32::mul(x0, cos_theta), soft_f32::mul(x1, sin_theta));
            let rotated1 =
                soft_f32::add(soft_f32::mul(x0, sin_theta), soft_f32::mul(x1, cos_theta));
            write_bits(out, base + pair, rotated0);
            write_bits(out, base + pair_offset + pair, rotated1);
            theta = soft_f32::mul(theta, params.theta_scale);
            pair += 1;
        }
        row += 1;
    }

    Ok(())
}

pub fn gqa_causal_attention(
    query: &TensorView<'_>,
    key: &TensorView<'_>,
    value: &TensorView<'_>,
    params: GqaAttentionParams,
    out: &mut [u8],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_gqa_shapes(query, key, value, params)?;
    require_output_len(out, query.shape, Dtype::F32)?;

    let seq_len = checked_usize(params.seq_len)?;
    let n_q_heads = checked_usize(params.n_q_heads)?;
    let head_dim = checked_usize(params.head_dim)?;
    let group = checked_usize(params.n_q_heads / params.n_kv_heads)?;
    let head_dim_i32 = i32::try_from(params.head_dim).map_err(|_| OpError::Input)?;
    let scale = soft_f32::rsqrt(soft_f32::int_to_f32(head_dim_i32));
    let score_ctx = GqaScoreContext {
        query,
        key,
        seq_len,
        head_dim,
        scale,
    };

    let mut q_head = 0usize;
    while q_head < n_q_heads {
        let kv_head = q_head / group;
        let mut q_pos = 0usize;
        while q_pos < seq_len {
            let max_score = gqa_score_max(&score_ctx, q_head, kv_head, q_pos)?;
            let denom = gqa_score_denom(&score_ctx, q_head, kv_head, q_pos, max_score)?;

            let mut out_dim = 0usize;
            while out_dim < head_dim {
                let mut acc = soft_f32::zero();
                let mut k_pos = 0usize;
                while k_pos <= q_pos {
                    let score = gqa_attention_score(&score_ctx, q_head, kv_head, q_pos, k_pos)?;
                    let weight = soft_f32::exp(soft_f32::sub(score, max_score));
                    let v_idx = gqa_index(kv_head, k_pos, out_dim, seq_len, head_dim)?;
                    acc = soft_f32::add(acc, soft_f32::mul(weight, read_bits(value.bytes, v_idx)?));
                    k_pos += 1;
                }
                let out_idx = gqa_index(q_head, q_pos, out_dim, seq_len, head_dim)?;
                write_bits(out, out_idx, soft_f32::div(acc, denom));
                out_dim += 1;
            }
            q_pos += 1;
        }
        q_head += 1;
    }

    Ok(())
}

/// Computes only the newest causal-attention row used by incremental decode.
/// The generic operator above remains the full-sequence reference path.
#[allow(dead_code)]
/// Causal attention for the last position only.
///
/// Lever 1b (Plans/NOTE-2026-10-25-arthur-lever2-design.md): each score is
/// computed once and kept in `weights` until it becomes its softmax weight,
/// and rows are sliced once instead of indexed per element. Every sum keeps
/// its order (the score's over `dim`, the denominator's and each output's
/// over `k_pos`), so the bytes are those of the reference body below.
pub fn gqa_causal_attention_last(
    query: &TensorView<'_>,
    key: &TensorView<'_>,
    value: &TensorView<'_>,
    params: GqaAttentionParams,
    out: &mut [u8],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_gqa_shapes(query, key, value, params)?;
    require_output_len_2d(out, params.n_q_heads, params.head_dim, Dtype::F32)?;

    let seq_len = checked_usize(params.seq_len)?;
    let n_q_heads = checked_usize(params.n_q_heads)?;
    let n_kv_heads = checked_usize(params.n_kv_heads)?;
    let head_dim = checked_usize(params.head_dim)?;
    // Each tensor's last element is read (the last query row of the last
    // head, every key and value row), so a short tensor fails here, as the
    // per-element reads did: index overflow is `Shape`, too few bytes `Input`.
    let head_len = seq_len.checked_mul(head_dim).ok_or(OpError::Shape)?;
    let query = gqa_f32_bytes(query.bytes, n_q_heads, head_len)?;
    let key = gqa_f32_bytes(key.bytes, n_kv_heads, head_len)?;
    let value = gqa_f32_bytes(value.bytes, n_kv_heads, head_len)?;
    let q_pos = seq_len - 1;
    let mut weights = vec![soft_f32::zero(); seq_len];
    gqa_last_core(
        params,
        0..n_q_heads,
        &mut weights,
        |q_head| query.lanes(q_head * head_len + q_pos * head_dim, head_dim),
        |kv_head| key.lanes(kv_head * head_len, head_len),
        |kv_head| value.lanes(kv_head * head_len, head_len),
        |idx, bits| write_bits(out, idx, bits),
    )
}

/// Lever 1c (Plans/NOTE-2026-10-26-arthur-lever1c-kv-in-place.md): the same
/// attention, reading the KV cache where it is. `q_last` is the last
/// position's `[q_heads * head_dim]` query; `key` and `value` are the cache's
/// `[kv_heads][max_context][head_dim]` storage, `head_stride` apart
/// (`max_context * head_dim`), of which the first `seq_len` positions of each
/// head are read. The arithmetic and every summation order are
/// `gqa_causal_attention_last`'s (`gqa_last_core`), so the output bits are
/// the same as copying the active prefix out and calling it.
#[allow(dead_code)] // called only from `forward_logic`'s default-off cached decode
pub fn gqa_causal_attention_last_strided(
    q_last: &[f32],
    key: &[f32],
    value: &[f32],
    head_stride: usize,
    params: GqaAttentionParams,
    out: &mut [f32],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let (n_q_heads, seq_len) = require_strided_gqa(q_last, key, value, head_stride, params)?;
    if out.len() != q_last.len() {
        return Err(OpError::Shape);
    }
    let mut weights = vec![soft_f32::zero(); seq_len];
    strided_core(
        q_last,
        key,
        value,
        head_stride,
        params,
        0..n_q_heads,
        &mut weights,
        out,
    )
}

/// Lever 2 (Plans/NOTE-2026-10-25-arthur-lever2-design.md): `gqa_causal_attention_last_strided`
/// for the q heads `heads` only, into `out` (`heads.len() * head_dim` values, those heads'
/// rows), with the caller's `weights` scratch (at least `seq_len` values) instead of an
/// allocation. A q head's output depends only on its own query row, its KV head and the
/// cache, and each head's summation orders are unchanged, so every split of the heads
/// into disjoint ranges gives the bytes of the whole call. The executor runs one range
/// per CPU; allocating nothing keeps a partition pure compute.
#[allow(dead_code)] // called only under `par-elementwise`
#[allow(clippy::too_many_arguments)]
pub fn gqa_causal_attention_last_strided_heads(
    q_last: &[f32],
    key: &[f32],
    value: &[f32],
    head_stride: usize,
    params: GqaAttentionParams,
    heads: core::ops::Range<usize>,
    weights: &mut [u32],
    out: &mut [f32],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let (n_q_heads, seq_len) = require_strided_gqa(q_last, key, value, head_stride, params)?;
    let head_dim = checked_usize(params.head_dim)?;
    if heads.start > heads.end
        || heads.end > n_q_heads
        || out.len() != (heads.end - heads.start) * head_dim
        || weights.len() < seq_len
    {
        return Err(OpError::Shape);
    }
    strided_core(
        q_last,
        key,
        value,
        head_stride,
        params,
        heads,
        &mut weights[..seq_len],
        out,
    )
}

/// The strided entries' checks: `n_q % n_kv == 0`, `q_last` is `n_q * head_dim` long,
/// `seq_len * head_dim <= head_stride` (`Shape`), and `key`/`value` reach the last KV
/// head's `seq_len` rows (`Input`). Returns `(n_q_heads, seq_len)`.
fn require_strided_gqa(
    q_last: &[f32],
    key: &[f32],
    value: &[f32],
    head_stride: usize,
    params: GqaAttentionParams,
) -> Result<(usize, usize), OpError> {
    require_nonzero_gqa_params(params)?;
    if !params.n_q_heads.is_multiple_of(params.n_kv_heads) {
        return Err(OpError::Shape);
    }
    let seq_len = checked_usize(params.seq_len)?;
    let n_q_heads = checked_usize(params.n_q_heads)?;
    let n_kv_heads = checked_usize(params.n_kv_heads)?;
    let head_dim = checked_usize(params.head_dim)?;
    let q_len = n_q_heads.checked_mul(head_dim).ok_or(OpError::Shape)?;
    let head_len = seq_len.checked_mul(head_dim).ok_or(OpError::Shape)?;
    if q_last.len() != q_len || head_len > head_stride {
        return Err(OpError::Shape);
    }
    // The last KV head ends `head_len` past its start.
    let needed = (n_kv_heads - 1)
        .checked_mul(head_stride)
        .and_then(|n| n.checked_add(head_len))
        .ok_or(OpError::Shape)?;
    if key.len() < needed || value.len() < needed {
        return Err(OpError::Input);
    }
    Ok((n_q_heads, seq_len))
}

/// `gqa_last_core` over validated strided inputs; `out` holds the rows of `heads`.
#[allow(clippy::too_many_arguments)]
fn strided_core(
    q_last: &[f32],
    key: &[f32],
    value: &[f32],
    head_stride: usize,
    params: GqaAttentionParams,
    heads: core::ops::Range<usize>,
    weights: &mut [u32],
    out: &mut [f32],
) -> Result<(), OpError> {
    let head_dim = checked_usize(params.head_dim)?;
    let head_len = checked_usize(params.seq_len)? * head_dim;
    let first = heads.start * head_dim;
    gqa_last_core(
        params,
        heads,
        weights,
        |q_head| q_last.lanes(q_head * head_dim, head_dim),
        |kv_head| key.lanes(kv_head * head_stride, head_len),
        |kv_head| value.lanes(kv_head * head_stride, head_len),
        |idx, bits| out[idx - first] = f32::from_bits(bits),
    )
}

/// f32 values read as IEEE bits: little-endian bytes or `f32`s. On x86 the
/// two give the same bits for the same stored value.
trait Lanes: Copy {
    /// `len` values starting at value `start`.
    fn lanes(self, start: usize, len: usize) -> Self;
    fn at(self, idx: usize) -> u32;
}

impl Lanes for &[u8] {
    #[inline(always)]
    fn lanes(self, start: usize, len: usize) -> Self {
        &self[start * 4..][..len * 4]
    }
    #[inline(always)]
    fn at(self, idx: usize) -> u32 {
        bits_at(self, idx)
    }
}

impl Lanes for &[f32] {
    #[inline(always)]
    fn lanes(self, start: usize, len: usize) -> Self {
        &self[start..][..len]
    }
    #[inline(always)]
    fn at(self, idx: usize) -> u32 {
        self[idx].to_bits()
    }
}

/// The last-position attention every entry shares, over already validated
/// rows, for the q heads in `heads`: `q_row(q_head)` is that head's `head_dim`
/// query values, and `k_head(kv_head)`/`v_head(kv_head)` are that KV head's
/// `seq_len * head_dim` values, position-major. `weights` (`seq_len` values) is
/// scratch, overwritten for each head before it is read. `write` gets each
/// output's index in the full `[n_q_heads * head_dim]` row.
#[inline(always)]
fn gqa_last_core<L: Lanes>(
    params: GqaAttentionParams,
    heads: core::ops::Range<usize>,
    weights: &mut [u32],
    q_row: impl Fn(usize) -> L,
    k_head: impl Fn(usize) -> L,
    v_head: impl Fn(usize) -> L,
    mut write: impl FnMut(usize, u32),
) -> Result<(), OpError> {
    let seq_len = checked_usize(params.seq_len)?;
    let head_dim = checked_usize(params.head_dim)?;
    let group = checked_usize(params.n_q_heads / params.n_kv_heads)?;
    let head_dim_i32 = i32::try_from(params.head_dim).map_err(|_| OpError::Input)?;
    let scale = soft_f32::rsqrt(soft_f32::int_to_f32(head_dim_i32));
    let q_pos = seq_len - 1;
    let weights = &mut weights[..seq_len];

    let mut q_head = heads.start;
    while q_head < heads.end {
        let kv_head = q_head / group;
        let q_row = q_row(q_head);
        let k_head = k_head(kv_head);
        let v_head = v_head(kv_head);

        let mut max_score = 0xff80_0000;
        let mut k_pos = 0usize;
        while k_pos <= q_pos {
            let k_row = k_head.lanes(k_pos * head_dim, head_dim);
            let mut acc = soft_f32::zero();
            let mut dim = 0usize;
            while dim < head_dim {
                acc = soft_f32::add(acc, soft_f32::mul(q_row.at(dim), k_row.at(dim)));
                dim += 1;
            }
            let score = soft_f32::mul(acc, scale);
            weights[k_pos] = score;
            max_score = soft_f32::fmax(max_score, score);
            k_pos += 1;
        }

        let mut denom = soft_f32::zero();
        let mut k_pos = 0usize;
        while k_pos <= q_pos {
            let weight = soft_f32::exp(soft_f32::sub(weights[k_pos], max_score));
            weights[k_pos] = weight;
            denom = soft_f32::add(denom, weight);
            k_pos += 1;
        }

        let mut out_dim = 0usize;
        while out_dim < head_dim {
            let mut acc = soft_f32::zero();
            let mut k_pos = 0usize;
            while k_pos <= q_pos {
                acc = soft_f32::add(
                    acc,
                    soft_f32::mul(weights[k_pos], v_head.at(k_pos * head_dim + out_dim)),
                );
                k_pos += 1;
            }
            write(q_head * head_dim + out_dim, soft_f32::div(acc, denom));
            out_dim += 1;
        }
        q_head += 1;
    }

    Ok(())
}

/// The first `heads * head_len` f32 values of `bytes`.
fn gqa_f32_bytes(bytes: &[u8], heads: usize, head_len: usize) -> Result<&[u8], OpError> {
    let len = heads
        .checked_mul(head_len)
        .and_then(|n| n.checked_mul(4))
        .ok_or(OpError::Shape)?;
    bytes.get(..len).ok_or(OpError::Input)
}

/// The `idx`-th little-endian f32 of a slice already checked to hold it.
#[inline(always)]
fn bits_at(bytes: &[u8], idx: usize) -> u32 {
    let b = &bytes[idx * 4..idx * 4 + 4];
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// The body before lever 1b (each score computed twice, indexed per
/// element): the host KAT's reference for byte identity
/// (`tools/ai-ops-tests` `attention_lever1b_kat`).
#[cfg(test)]
pub fn gqa_causal_attention_last_reference(
    query: &TensorView<'_>,
    key: &TensorView<'_>,
    value: &TensorView<'_>,
    params: GqaAttentionParams,
    out: &mut [u8],
) -> Result<(), OpError> {
    require_gqa_shapes(query, key, value, params)?;
    require_output_len_2d(out, params.n_q_heads, params.head_dim, Dtype::F32)?;

    let seq_len = checked_usize(params.seq_len)?;
    let n_q_heads = checked_usize(params.n_q_heads)?;
    let head_dim = checked_usize(params.head_dim)?;
    let group = checked_usize(params.n_q_heads / params.n_kv_heads)?;
    let head_dim_i32 = i32::try_from(params.head_dim).map_err(|_| OpError::Input)?;
    let scale = soft_f32::rsqrt(soft_f32::int_to_f32(head_dim_i32));
    let score_ctx = GqaScoreContext {
        query,
        key,
        seq_len,
        head_dim,
        scale,
    };
    let q_pos = seq_len - 1;
    let mut weights = vec![soft_f32::zero(); seq_len];

    let mut q_head = 0usize;
    while q_head < n_q_heads {
        let kv_head = q_head / group;
        let max_score = gqa_score_max(&score_ctx, q_head, kv_head, q_pos)?;
        let mut denom = soft_f32::zero();
        let mut k_pos = 0usize;
        while k_pos <= q_pos {
            let score = gqa_attention_score(&score_ctx, q_head, kv_head, q_pos, k_pos)?;
            let weight = soft_f32::exp(soft_f32::sub(score, max_score));
            weights[k_pos] = weight;
            denom = soft_f32::add(denom, weight);
            k_pos += 1;
        }

        let mut out_dim = 0usize;
        while out_dim < head_dim {
            let mut acc = soft_f32::zero();
            let mut k_pos = 0usize;
            while k_pos <= q_pos {
                let v_idx = gqa_index(kv_head, k_pos, out_dim, seq_len, head_dim)?;
                acc = soft_f32::add(
                    acc,
                    soft_f32::mul(weights[k_pos], read_bits(value.bytes, v_idx)?),
                );
                k_pos += 1;
            }
            write_bits(out, q_head * head_dim + out_dim, soft_f32::div(acc, denom));
            out_dim += 1;
        }
        q_head += 1;
    }

    Ok(())
}

struct GqaScoreContext<'a, 'b> {
    query: &'a TensorView<'b>,
    key: &'a TensorView<'b>,
    seq_len: usize,
    head_dim: usize,
    scale: u32,
}

pub fn matmul(lhs: &TensorView<'_>, rhs: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(lhs)?;
    require_f32(rhs)?;
    if lhs.shape.rank() != 2 || rhs.shape.rank() != 2 {
        return Err(OpError::Shape);
    }
    if lhs.shape.dim(1) != rhs.shape.dim(0) {
        return Err(OpError::Shape);
    }
    let m = checked_usize(lhs.shape.dim(0))?;
    let k = checked_usize(lhs.shape.dim(1))?;
    let n = checked_usize(rhs.shape.dim(1))?;
    require_output_len_2d(out, m as u64, n as u64, Dtype::F32)?;

    let mut row = 0usize;
    while row < m {
        let mut col = 0usize;
        while col < n {
            let mut acc = soft_f32::zero();
            let mut inner = 0usize;
            while inner < k {
                let a = read_bits(lhs.bytes, row * k + inner)?;
                let b = read_bits(rhs.bytes, inner * n + col)?;
                acc = soft_f32::add(acc, soft_f32::mul(a, b));
                inner += 1;
            }
            write_bits(out, row * n + col, acc);
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

pub fn embedding(
    indices: &TensorView<'_>,
    table: &TensorView<'_>,
    out: &mut [u8],
) -> Result<(), OpError> {
    require_i32(indices)?;
    require_f32(table)?;
    if indices.shape.rank() != 1 || table.shape.rank() != 2 {
        return Err(OpError::Shape);
    }

    let seq64 = indices.shape.dim(0);
    let vocab64 = table.shape.dim(0);
    let dim64 = table.shape.dim(1);
    require_output_len_2d(out, seq64, dim64, Dtype::F32)?;

    let seq = checked_usize(seq64)?;
    let vocab = checked_usize(vocab64)?;
    let dim = checked_usize(dim64)?;

    let mut pos = 0usize;
    while pos < seq {
        let idx = read_i32(indices.bytes, pos)?;
        if idx < 0 {
            return Err(OpError::Index);
        }
        let row = usize::try_from(idx).map_err(|_| OpError::Index)?;
        if row >= vocab {
            return Err(OpError::Index);
        }
        let row_base = row.checked_mul(dim).ok_or(OpError::Shape)?;
        let out_base = pos.checked_mul(dim).ok_or(OpError::Shape)?;

        let mut col = 0usize;
        while col < dim {
            write_bits(out, out_base + col, read_bits(table.bytes, row_base + col)?);
            col += 1;
        }
        pos += 1;
    }
    Ok(())
}

pub fn relu(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    require_f32(input)?;
    require_output_len(out, input.shape, input.dtype)?;
    let count = element_count(input.shape)?;
    let mut idx = 0usize;
    while idx < count {
        write_bits(out, idx, soft_f32::relu(read_bits(input.bytes, idx)?));
        idx += 1;
    }
    Ok(())
}

fn elementwise_binary(
    lhs: &TensorView<'_>,
    rhs: &TensorView<'_>,
    out: &mut [u8],
    f: fn(u32, u32) -> u32,
) -> Result<(), OpError> {
    require_f32(lhs)?;
    require_f32(rhs)?;
    if lhs.shape.rank() == 1 && rhs.shape.rank() == 1 {
        if lhs.shape.dim(0) != rhs.shape.dim(0) {
            return Err(OpError::Shape);
        }
        require_output_len(out, lhs.shape, Dtype::F32)?;
        let count = checked_usize(lhs.shape.dim(0))?;
        let mut idx = 0usize;
        while idx < count {
            write_bits(
                out,
                idx,
                f(read_bits(lhs.bytes, idx)?, read_bits(rhs.bytes, idx)?),
            );
            idx += 1;
        }
        return Ok(());
    }

    let (rows64, cols64) = broadcast_output(lhs, rhs)?;
    require_output_len_2d(out, rows64, cols64, Dtype::F32)?;
    let rows = checked_usize(rows64)?;
    let cols = checked_usize(cols64)?;
    let lhs_rows = checked_usize(lhs.shape.dim(0))?;
    let lhs_cols = checked_usize(lhs.shape.dim(1))?;
    let rhs_rows = checked_usize(rhs.shape.dim(0))?;
    let rhs_cols = checked_usize(rhs.shape.dim(1))?;

    let mut row = 0usize;
    while row < rows {
        let mut col = 0usize;
        while col < cols {
            let lhs_row = if lhs_rows == 1 { 0 } else { row };
            let lhs_col = if lhs_cols == 1 { 0 } else { col };
            let rhs_row = if rhs_rows == 1 { 0 } else { row };
            let rhs_col = if rhs_cols == 1 { 0 } else { col };
            let lhs_bits = read_bits(lhs.bytes, lhs_row * lhs_cols + lhs_col)?;
            let rhs_bits = read_bits(rhs.bytes, rhs_row * rhs_cols + rhs_col)?;
            write_bits(out, row * cols + col, f(lhs_bits, rhs_bits));
            col += 1;
        }
        row += 1;
    }
    Ok(())
}

fn broadcast_output(lhs: &TensorView<'_>, rhs: &TensorView<'_>) -> Result<(u64, u64), OpError> {
    require_f32(lhs)?;
    require_f32(rhs)?;
    if lhs.shape.rank() != 2 || rhs.shape.rank() != 2 {
        return Err(OpError::Shape);
    }
    let rows = broadcast_dim(lhs.shape.dim(0), rhs.shape.dim(0))?;
    let cols = broadcast_dim(lhs.shape.dim(1), rhs.shape.dim(1))?;
    Ok((rows, cols))
}

fn broadcast_dim(lhs: u64, rhs: u64) -> Result<u64, OpError> {
    if lhs == rhs {
        Ok(lhs)
    } else if lhs == 1 {
        Ok(rhs)
    } else if rhs == 1 {
        Ok(lhs)
    } else {
        Err(OpError::Shape)
    }
}

fn require_f32(input: &TensorView<'_>) -> Result<(), OpError> {
    if input.dtype != Dtype::F32 {
        return Err(OpError::Dtype);
    }
    require_output_len(input.bytes, input.shape, input.dtype)
}

fn require_i32(input: &TensorView<'_>) -> Result<(), OpError> {
    if input.dtype != Dtype::I32 {
        return Err(OpError::Dtype);
    }
    require_output_len(input.bytes, input.shape, input.dtype)
}

fn require_same_f32_shape(lhs: &TensorView<'_>, rhs: &TensorView<'_>) -> Result<(), OpError> {
    require_f32(lhs)?;
    require_f32(rhs)?;
    if lhs.shape.rank() != rhs.shape.rank() {
        return Err(OpError::Shape);
    }
    let rank = lhs.shape.rank();
    let mut idx = 0usize;
    while idx < rank as usize {
        if lhs.shape.dim(idx) != rhs.shape.dim(idx) {
            return Err(OpError::Shape);
        }
        idx += 1;
    }
    Ok(())
}

fn require_output_len(bytes: &[u8], shape: &Shape, dtype: Dtype) -> Result<(), OpError> {
    if bytes.len() != shape.byte_len(dtype).map_err(tensor_err)? {
        return Err(OpError::Output);
    }
    Ok(())
}

fn require_output_len_2d(bytes: &[u8], rows: u64, cols: u64, dtype: Dtype) -> Result<(), OpError> {
    let Some(elements) = rows.checked_mul(cols) else {
        return Err(OpError::Shape);
    };
    let Some(byte_len) = elements.checked_mul(dtype.size_bytes() as u64) else {
        return Err(OpError::Shape);
    };
    if byte_len == 0 || byte_len > usize::MAX as u64 || bytes.len() != byte_len as usize {
        return Err(OpError::Output);
    }
    Ok(())
}

fn element_count(shape: &Shape) -> Result<usize, OpError> {
    let count = shape.num_elements().map_err(tensor_err)?;
    checked_usize(count)
}

fn read_bits(bytes: &[u8], idx: usize) -> Result<u32, OpError> {
    let off = idx.checked_mul(4).ok_or(OpError::Input)?;
    let slice = bytes.get(off..off + 4).ok_or(OpError::Input)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_i32(bytes: &[u8], idx: usize) -> Result<i32, OpError> {
    let off = idx.checked_mul(4).ok_or(OpError::Input)?;
    let slice = bytes.get(off..off + 4).ok_or(OpError::Input)?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn write_bits(bytes: &mut [u8], idx: usize, value: u32) {
    let off = idx * 4;
    bytes[off] = value as u8;
    bytes[off + 1] = (value >> 8) as u8;
    bytes[off + 2] = (value >> 16) as u8;
    bytes[off + 3] = (value >> 24) as u8;
}

fn checked_usize(value: u64) -> Result<usize, OpError> {
    usize::try_from(value).map_err(|_| OpError::Shape)
}

fn copy_bytes(src: &[u8], dst: &mut [u8]) -> Result<(), OpError> {
    if src.len() != dst.len() {
        return Err(OpError::Output);
    }
    let mut idx = 0usize;
    while idx < src.len() {
        dst[idx] = src[idx];
        idx += 1;
    }
    Ok(())
}

fn tensor_err(err: TensorError) -> OpError {
    match err {
        TensorError::InvalidDtype => OpError::Dtype,
        TensorError::InvalidShape | TensorError::InvalidLength => OpError::Shape,
        TensorError::PermissionDenied | TensorError::OutOfMemory | TensorError::InvalidDevice => {
            OpError::Output
        }
    }
}

/// Softmax over last axis.
pub fn softmax(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(input)?;
    let rank = input.shape.rank();
    if rank < 1 {
        return Err(OpError::Shape);
    }
    let last_dim = checked_usize(input.shape.dim((rank - 1) as usize))?;
    let total_elems = input.shape.num_elements().map_err(tensor_err)?;
    let prefix_elems = checked_usize(total_elems / (last_dim as u64).max(1))?;
    require_output_len(out, input.shape, input.dtype)?;

    let mut p = 0usize;
    while p < prefix_elems {
        let base = p * last_dim;

        let mut max_val: u32 = 0xff80_0000;
        let mut i = 0usize;
        while i < last_dim {
            let v = read_bits(input.bytes, base + i)?;
            max_val = soft_f32::fmax(max_val, v);
            i += 1;
        }

        let mut sum = soft_f32::zero();
        i = 0usize;
        while i < last_dim {
            let v = read_bits(input.bytes, base + i)?;
            let shifted = soft_f32::exp(soft_f32::sub(v, max_val));
            write_bits(out, base + i, shifted);
            sum = soft_f32::add(sum, shifted);
            i += 1;
        }

        i = 0usize;
        while i < last_dim {
            let v = read_bits(out, base + i)?;
            write_bits(out, base + i, soft_f32::div(v, sum));
            i += 1;
        }

        p += 1;
    }
    Ok(())
}

/// GELU approximation: x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3))).
pub fn gelu(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(input)?;
    require_output_len(out, input.shape, input.dtype)?;
    let count = element_count(input.shape)?;

    let sqrt_2_over_pi: u32 = 0x3f4c_4229;
    let coeff: u32 = 0x3d37_2713;
    let half: u32 = 0x3f00_0000;
    let one: u32 = 0x3f80_0000;

    let mut idx = 0usize;
    while idx < count {
        let x = read_bits(input.bytes, idx)?;
        let x2 = soft_f32::mul(x, x);
        let x3 = soft_f32::mul(x2, x);
        let cx3 = soft_f32::mul(coeff, x3);
        let sum = soft_f32::add(x, cx3);
        let scaled = soft_f32::mul(sqrt_2_over_pi, sum);
        let tanh_val = soft_f32::tanh(scaled);
        let inner = soft_f32::add(one, tanh_val);
        let result = soft_f32::mul(x, soft_f32::mul(half, inner));
        write_bits(out, idx, result);
        idx += 1;
    }
    Ok(())
}

/// LayerNorm over last axis with fixed epsilon = 1e-5.
pub fn layer_norm(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    layer_norm_impl(input, None, None, out)
}

pub fn layer_norm_affine(
    input: &TensorView<'_>,
    gamma: &TensorView<'_>,
    beta: &TensorView<'_>,
    out: &mut [u8],
) -> Result<(), OpError> {
    layer_norm_impl(input, Some(gamma), Some(beta), out)
}

fn layer_norm_impl(
    input: &TensorView<'_>,
    gamma: Option<&TensorView<'_>>,
    beta: Option<&TensorView<'_>>,
    out: &mut [u8],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(input)?;
    let rank = input.shape.rank();
    if rank < 1 {
        return Err(OpError::Shape);
    }
    let last_dim = checked_usize(input.shape.dim((rank - 1) as usize))?;
    if let Some(gamma) = gamma {
        require_last_axis_vector(gamma, last_dim)?;
    }
    if let Some(beta) = beta {
        require_last_axis_vector(beta, last_dim)?;
    }
    let total_elems = input.shape.num_elements().map_err(tensor_err)?;
    let prefix_elems = checked_usize(total_elems / (last_dim as u64).max(1))?;
    require_output_len(out, input.shape, input.dtype)?;

    let eps: u32 = 0x3727_c5ac;
    let zero = soft_f32::zero();

    let mut p = 0usize;
    while p < prefix_elems {
        let base = p * last_dim;

        let mut mean = zero;
        let mut i = 0usize;
        while i < last_dim {
            mean = soft_f32::add(mean, read_bits(input.bytes, base + i)?);
            i += 1;
        }
        mean = soft_f32::div(mean, int_to_f32(last_dim as i32));

        let mut var_sum = zero;
        i = 0usize;
        while i < last_dim {
            let diff = soft_f32::sub(read_bits(input.bytes, base + i)?, mean);
            var_sum = soft_f32::add(var_sum, soft_f32::mul(diff, diff));
            i += 1;
        }
        var_sum = soft_f32::div(var_sum, int_to_f32(last_dim as i32));
        let std = soft_f32::sqrt(soft_f32::add(var_sum, eps));

        i = 0usize;
        while i < last_dim {
            let v = read_bits(input.bytes, base + i)?;
            let mut normalized = soft_f32::div(soft_f32::sub(v, mean), std);
            if let Some(gamma) = gamma {
                normalized = soft_f32::mul(normalized, read_bits(gamma.bytes, i)?);
            }
            if let Some(beta) = beta {
                normalized = soft_f32::add(normalized, read_bits(beta.bytes, i)?);
            }
            write_bits(out, base + i, normalized);
            i += 1;
        }

        p += 1;
    }
    Ok(())
}

/// RMSNorm over last axis with fixed epsilon = 1e-5.
pub fn rms_norm(input: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError> {
    rms_norm_impl(input, None, 1.0e-5f32.to_bits(), out)
}

pub fn rms_norm_affine(
    input: &TensorView<'_>,
    gamma: &TensorView<'_>,
    out: &mut [u8],
) -> Result<(), OpError> {
    rms_norm_impl(input, Some(gamma), 1.0e-5f32.to_bits(), out)
}

#[allow(dead_code)] // Qwen-family RMSNorm epsilon variant staged for importer/forward wiring.
pub fn rms_norm_affine_eps(
    input: &TensorView<'_>,
    gamma: &TensorView<'_>,
    eps_bits: u32,
    out: &mut [u8],
) -> Result<(), OpError> {
    rms_norm_impl(input, Some(gamma), eps_bits, out)
}

fn rms_norm_impl(
    input: &TensorView<'_>,
    gamma: Option<&TensorView<'_>>,
    eps: u32,
    out: &mut [u8],
) -> Result<(), OpError> {
    soft_f32::debug_assert_kernel_mxcsr();
    require_f32(input)?;
    let rank = input.shape.rank();
    if rank < 1 {
        return Err(OpError::Shape);
    }
    let last_dim = checked_usize(input.shape.dim((rank - 1) as usize))?;
    if let Some(gamma) = gamma {
        require_last_axis_vector(gamma, last_dim)?;
    }
    let total_elems = input.shape.num_elements().map_err(tensor_err)?;
    let prefix_elems = checked_usize(total_elems / (last_dim as u64).max(1))?;
    require_output_len(out, input.shape, input.dtype)?;

    let zero = soft_f32::zero();

    let mut p = 0usize;
    while p < prefix_elems {
        let base = p * last_dim;

        let mut rms_sq = zero;
        let mut i = 0usize;
        while i < last_dim {
            let v = read_bits(input.bytes, base + i)?;
            rms_sq = soft_f32::add(rms_sq, soft_f32::mul(v, v));
            i += 1;
        }
        rms_sq = soft_f32::div(rms_sq, int_to_f32(last_dim as i32));
        let rms = soft_f32::sqrt(soft_f32::add(rms_sq, eps));

        i = 0usize;
        while i < last_dim {
            let v = read_bits(input.bytes, base + i)?;
            let mut normalized = soft_f32::div(v, rms);
            if let Some(gamma) = gamma {
                normalized = soft_f32::mul(normalized, read_bits(gamma.bytes, i)?);
            }
            write_bits(out, base + i, normalized);
            i += 1;
        }

        p += 1;
    }
    Ok(())
}

fn require_last_axis_vector(input: &TensorView<'_>, last_dim: usize) -> Result<(), OpError> {
    require_f32(input)?;
    if input.shape.rank() != 1 || checked_usize(input.shape.dim(0))? != last_dim {
        return Err(OpError::Shape);
    }
    Ok(())
}

fn int_to_f32(k: i32) -> u32 {
    soft_f32::int_to_f32(k)
}

fn silu_bits(x: u32) -> u32 {
    let one = 0x3f80_0000;
    let sigmoid_den = soft_f32::add(one, soft_f32::exp(x ^ 0x8000_0000));
    soft_f32::div(x, sigmoid_den)
}

fn rope_yarn_theta(theta_extrap: u32, pair: usize, params: RopeYarnParams) -> (u32, u32) {
    let one = 0x3f80_0000;
    let tenth = 0x3dcc_cccd;
    let theta_interp = soft_f32::mul(params.freq_scale, theta_extrap);
    let mut theta = theta_interp;
    let mut mscale = params.attn_factor;

    if !is_zero_f32(params.ext_factor) {
        let ramp = rope_yarn_ramp(params.corr_dim_low, params.corr_dim_high, pair);
        let ramp_mix = soft_f32::mul(ramp, params.ext_factor);
        theta = soft_f32::add(
            soft_f32::mul(theta_interp, soft_f32::sub(one, ramp_mix)),
            soft_f32::mul(theta_extrap, ramp_mix),
        );
        let inv_freq = soft_f32::div(one, params.freq_scale);
        let yarn_mscale = soft_f32::add(one, soft_f32::mul(tenth, soft_f32::ln(inv_freq)));
        mscale = soft_f32::mul(mscale, yarn_mscale);
    }

    (
        soft_f32::mul(soft_f32::cos(theta), mscale),
        soft_f32::mul(soft_f32::sin(theta), mscale),
    )
}

fn rope_yarn_ramp(low: u32, high: u32, pair: usize) -> u32 {
    let one = 0x3f80_0000;
    let zero = soft_f32::zero();
    let min_width = 0x3a83_126f; // 0.001
    let pair_bits = soft_f32::int_to_f32(pair as i32);
    let width = soft_f32::fmax(min_width, soft_f32::sub(high, low));
    let y = soft_f32::div(soft_f32::sub(pair_bits, low), width);
    let clamped = soft_f32::fmin(one, soft_f32::fmax(zero, y));
    soft_f32::sub(one, clamped)
}

fn rope_yarn_params(input: &TensorView<'_>) -> Result<RopeYarnParams, OpError> {
    require_f32(input)?;
    if input.shape.rank() != 1 || checked_usize(input.shape.dim(0))? != ROPE_YARN_PARAM_COUNT {
        return Err(OpError::Shape);
    }
    Ok(RopeYarnParams {
        position: f32_bits_to_u64(read_bits(input.bytes, 0)?)?,
        n_dims: f32_bits_to_u64(read_bits(input.bytes, 1)?)?,
        theta_scale: read_bits(input.bytes, 2)?,
        freq_scale: read_bits(input.bytes, 3)?,
        ext_factor: read_bits(input.bytes, 4)?,
        attn_factor: read_bits(input.bytes, 5)?,
        corr_dim_low: read_bits(input.bytes, 6)?,
        corr_dim_high: read_bits(input.bytes, 7)?,
    })
}

fn gqa_attention_params(input: &TensorView<'_>) -> Result<GqaAttentionParams, OpError> {
    require_f32(input)?;
    if input.shape.rank() != 1 || checked_usize(input.shape.dim(0))? != GQA_ATTENTION_PARAM_COUNT {
        return Err(OpError::Shape);
    }
    let params = GqaAttentionParams {
        seq_len: f32_bits_to_u64(read_bits(input.bytes, 0)?)?,
        n_q_heads: f32_bits_to_u64(read_bits(input.bytes, 1)?)?,
        n_kv_heads: f32_bits_to_u64(read_bits(input.bytes, 2)?)?,
        head_dim: f32_bits_to_u64(read_bits(input.bytes, 3)?)?,
    };
    require_nonzero_gqa_params(params)?;
    Ok(params)
}

fn require_gqa_shapes(
    query: &TensorView<'_>,
    key: &TensorView<'_>,
    value: &TensorView<'_>,
    params: GqaAttentionParams,
) -> Result<(), OpError> {
    require_nonzero_gqa_params(params)?;
    require_f32(query)?;
    require_f32(key)?;
    require_f32(value)?;
    if query.shape.rank() != 3 || key.shape.rank() != 3 || value.shape.rank() != 3 {
        return Err(OpError::Shape);
    }
    if !params.n_q_heads.is_multiple_of(params.n_kv_heads) {
        return Err(OpError::Shape);
    }
    if query.shape.dim(0) != params.n_q_heads
        || query.shape.dim(1) != params.seq_len
        || query.shape.dim(2) != params.head_dim
        || key.shape.dim(0) != params.n_kv_heads
        || key.shape.dim(1) != params.seq_len
        || key.shape.dim(2) != params.head_dim
        || value.shape.dim(0) != params.n_kv_heads
        || value.shape.dim(1) != params.seq_len
        || value.shape.dim(2) != params.head_dim
    {
        return Err(OpError::Shape);
    }
    Ok(())
}

fn require_nonzero_gqa_params(params: GqaAttentionParams) -> Result<(), OpError> {
    if params.seq_len == 0
        || params.n_q_heads == 0
        || params.n_kv_heads == 0
        || params.head_dim == 0
    {
        return Err(OpError::Input);
    }
    Ok(())
}

fn gqa_score_max(
    ctx: &GqaScoreContext<'_, '_>,
    q_head: usize,
    kv_head: usize,
    q_pos: usize,
) -> Result<u32, OpError> {
    let mut max_score = 0xff80_0000;
    let mut k_pos = 0usize;
    while k_pos <= q_pos {
        let score = gqa_attention_score(ctx, q_head, kv_head, q_pos, k_pos)?;
        max_score = soft_f32::fmax(max_score, score);
        k_pos += 1;
    }
    Ok(max_score)
}

fn gqa_score_denom(
    ctx: &GqaScoreContext<'_, '_>,
    q_head: usize,
    kv_head: usize,
    q_pos: usize,
    max_score: u32,
) -> Result<u32, OpError> {
    let mut denom = soft_f32::zero();
    let mut k_pos = 0usize;
    while k_pos <= q_pos {
        let score = gqa_attention_score(ctx, q_head, kv_head, q_pos, k_pos)?;
        denom = soft_f32::add(denom, soft_f32::exp(soft_f32::sub(score, max_score)));
        k_pos += 1;
    }
    Ok(denom)
}

fn gqa_attention_score(
    ctx: &GqaScoreContext<'_, '_>,
    q_head: usize,
    kv_head: usize,
    q_pos: usize,
    k_pos: usize,
) -> Result<u32, OpError> {
    let mut acc = soft_f32::zero();
    let mut dim = 0usize;
    while dim < ctx.head_dim {
        let q_idx = gqa_index(q_head, q_pos, dim, ctx.seq_len, ctx.head_dim)?;
        let k_idx = gqa_index(kv_head, k_pos, dim, ctx.seq_len, ctx.head_dim)?;
        acc = soft_f32::add(
            acc,
            soft_f32::mul(
                read_bits(ctx.query.bytes, q_idx)?,
                read_bits(ctx.key.bytes, k_idx)?,
            ),
        );
        dim += 1;
    }
    Ok(soft_f32::mul(acc, ctx.scale))
}

fn gqa_index(
    head: usize,
    pos: usize,
    dim: usize,
    seq_len: usize,
    head_dim: usize,
) -> Result<usize, OpError> {
    head.checked_mul(seq_len)
        .and_then(|idx| idx.checked_add(pos))
        .and_then(|idx| idx.checked_mul(head_dim))
        .and_then(|idx| idx.checked_add(dim))
        .ok_or(OpError::Shape)
}

fn f32_bits_to_u64(bits: u32) -> Result<u64, OpError> {
    const SIGN_MASK: u32 = 0x8000_0000;
    const EXP_MASK: u32 = 0x7f80_0000;
    const FRAC_MASK: u32 = 0x007f_ffff;

    if bits & SIGN_MASK != 0 {
        return Err(OpError::Input);
    }
    if bits & !SIGN_MASK == 0 {
        return Ok(0);
    }

    let exp = ((bits & EXP_MASK) >> 23) as i32;
    if exp == 0xff {
        return Err(OpError::Input);
    }
    let mant = if exp == 0 {
        (bits & FRAC_MASK) as u64
    } else {
        (1u64 << 23) | ((bits & FRAC_MASK) as u64)
    };
    let shift = exp - 127 - 23;
    if shift >= 0 {
        mant.checked_shl(shift as u32).ok_or(OpError::Input)
    } else {
        let rshift = (-shift) as u32;
        if rshift < 64 && (mant & ((1u64 << rshift) - 1)) != 0 {
            return Err(OpError::Input);
        }
        Ok(if rshift >= 64 { 0 } else { mant >> rshift })
    }
}

fn is_zero_f32(bits: u32) -> bool {
    bits & !0x8000_0000 == 0
}
