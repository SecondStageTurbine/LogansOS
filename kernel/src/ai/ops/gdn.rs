// SPDX-License-Identifier: MPL-2.0
//! Gated DeltaNet (GDN) ops — OT-16 B2.
//!
//! Pure f32 kernels ported from the HF `transformers` reference
//! (`torch_recurrent_gated_delta_rule`, `torch_causal_conv1d_update`,
//! `l2norm`, `Qwen3_5RMSNormGated`). Each op has an independent byte-oracle
//! KAT in `model-logic-tests` anchored to the HF source's literal values.
//!
//! The recurrence is inherently SEQUENTIAL in the state dimension (§4.3 of
//! the THINK) — sequential IS deterministic; prefill chunking is a FUTURE
//! optimization, NOT B2.
#![allow(dead_code)] // default-off Phase-9 scaffold; host KATs consume first

extern crate alloc;

use super::soft_f32;
use alloc::{vec, vec::Vec};

/// Error type for GDN ops.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GdnError {
    Shape,
}

#[derive(Copy, Clone)]
pub struct GdnProjectedInputs<'a> {
    pub qkv: &'a [f32],
    pub z: &'a [f32],
    pub beta_raw: &'a [f32],
    pub a_raw: &'a [f32],
}

// ─────────────────────────────────────────────────────────────────────────────
// ISC-1: GDN recurrent step (the load-bearing op)
//
// HF reference: `torch_recurrent_gated_delta_rule` (modeling_qwen3_5.py:329).
// Single-token decode path (batch=1, seq_len=1). The full function operates on
// 4D tensors [batch, heads, seq, dim]; we flatten to per-head per-token since
// the kernel forward is always batch-1, seq-1 for autoregressive decode.
//
// Per token t, per head h:
//   g_t = exp(g[h, t])                          // gate (scalar per head)
//   S = S * g_t                                  // state decay
//   kv_mem = sum_k(S[h, k, :] * k_t[h, k])      // kv-memory (v_dim scalar)
//   delta = (v_t[h, :] - kv_mem) * beta[h, t]   // delta (v_dim vector)
//   S = S + k_t[h, :, None] * delta[None, :]    // state update (outer product)
//   out[h, :] = sum_k(S[h, k, :] * q_t[h, k])  // output projection
//
// State S shape: [heads, k_dim, v_dim].
// ─────────────────────────────────────────────────────────────────────────────

/// Single-token GDN recurrent step. `state` is the recurrent state matrix
/// `[heads * k_dim * v_dim]`, mutated in-place. Returns `[heads * v_dim]`.
///
/// All inputs are f32, row-major. `g` and `beta` are `[heads]` (the single
/// token's values). The query/key/value/gate/beta are already per-token.
pub fn gdn_recurrent_step(
    q: &[f32],         // [heads * k_dim]
    k: &[f32],         // [heads * k_dim]
    v: &[f32],         // [heads * v_dim]
    g: &[f32], // [heads] — exp already applied by caller (exp(softplus(alpha + dt) * ssm_a))
    beta: &[f32], // [heads] — sigmoid already applied by caller
    state: &mut [f32], // [heads * k_dim * v_dim], mutated in-place
    out: &mut [f32], // [heads * v_dim], output
) -> Result<(), GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let heads = g.len();
    if beta.len() != heads {
        return Err(GdnError::Shape);
    }

    let k_dim = q.len().checked_div(heads).ok_or(GdnError::Shape)?;
    let v_dim = v.len().checked_div(heads).ok_or(GdnError::Shape)?;
    if heads == 0 {
        return Err(GdnError::Shape);
    }
    if k.len() != heads * k_dim || out.len() != heads * v_dim {
        return Err(GdnError::Shape);
    }
    if state.len() != heads * k_dim * v_dim {
        return Err(GdnError::Shape);
    }

    let mut h = 0usize;
    while h < heads {
        let g_t = g[h];
        let beta_t = beta[h];
        let q_base = h * k_dim;
        let k_base = h * k_dim;
        let v_base = h * v_dim;
        let out_base = h * v_dim;
        let state_base = h * k_dim * v_dim;

        // S = S * g_t (state decay — scalar multiply the whole head slice)
        let mut s_idx = state_base;
        while s_idx < state_base + k_dim * v_dim {
            state[s_idx] *= g_t;
            s_idx += 1;
        }

        // kv_mem = sum_k(S[h, k, :] * k_t[h, k])  → [v_dim]
        let mut kv_mem = vec![0.0f32; v_dim];
        let mut vd = 0usize;
        while vd < v_dim {
            kv_mem[vd] =
                recurrent_state_dot(state, state_base, v_dim, vd, &k[k_base..k_base + k_dim]);
            vd += 1;
        }

        // delta = (v_t - kv_mem) * beta_t  → [v_dim]
        vd = 0;
        while vd < v_dim {
            let delta_val = (v[v_base + vd] - kv_mem[vd]) * beta_t;
            // S = S + k_t[:, None] * delta[None, :]  (outer product)
            // For each k_dim row: S[k][v] += k[k] * delta[v]
            let mut kd2 = 0usize;
            while kd2 < k_dim {
                let state_idx = state_base + kd2 * v_dim + vd;
                state[state_idx] = fused_mul_add_f32(k[k_base + kd2], delta_val, state[state_idx]);
                kd2 += 1;
            }
            vd += 1;
        }

        // out[h, :] = sum_k(S[h, k, :] * q_t[h, k])
        vd = 0;
        while vd < v_dim {
            out[out_base + vd] =
                recurrent_state_dot(state, state_base, v_dim, vd, &q[q_base..q_base + k_dim]);
            vd += 1;
        }

        h += 1;
    }

    Ok(())
}

/// Match llama.cpp's AVX/FMA `ggml_vec_dot_f32` reduction tree for the
/// 128-lane Qwen3.5 recurrent state. Other widths retain scalar order for the
/// small hermetic KATs and any future non-Qwen3.5 geometry.
fn recurrent_state_dot(
    state: &[f32],
    state_base: usize,
    v_dim: usize,
    v_index: usize,
    rhs: &[f32],
) -> f32 {
    if rhs.len() != 128 {
        let mut sum = 0.0f32;
        let mut idx = 0usize;
        while idx < rhs.len() {
            sum += state[state_base + idx * v_dim + v_index] * rhs[idx];
            idx += 1;
        }
        return sum;
    }

    let mut accum = [[0.0f32; 8]; 4];
    let mut chunk = 0usize;
    while chunk < 4 {
        let chunk_base = chunk * 32;
        let mut vector = 0usize;
        while vector < 4 {
            let vector_base = chunk_base + vector * 8;
            let mut lane = 0usize;
            while lane < 8 {
                let idx = vector_base + lane;
                accum[vector][lane] = fused_mul_add_f32(
                    state[state_base + idx * v_dim + v_index],
                    rhs[idx],
                    accum[vector][lane],
                );
                lane += 1;
            }
            vector += 1;
        }
        chunk += 1;
    }

    let mut lane = 0usize;
    while lane < 8 {
        accum[0][lane] += accum[2][lane];
        accum[1][lane] += accum[3][lane];
        accum[0][lane] += accum[1][lane];
        lane += 1;
    }
    let t0 = [
        accum[0][0] + accum[0][4],
        accum[0][1] + accum[0][5],
        accum[0][2] + accum[0][6],
        accum[0][3] + accum[0][7],
    ];
    (t0[0] + t0[1]) + (t0[2] + t0[3])
}

fn fused_mul_add_f32(a: f32, b: f32, c: f32) -> f32 {
    (f64::from(a) * f64::from(b) + f64::from(c)) as f32
}

// ─────────────────────────────────────────────────────────────────────────────
// ISC-2: Supporting ops
// ─────────────────────────────────────────────────────────────────────────────

/// GDN causal conv1d update — depthwise conv1d + SiLU activation.
///
/// HF reference: `torch_causal_conv1d_update` (modeling_qwen3_5.py:224).
/// Single-token decode path: concatenate `[conv_state, hidden]`, update state,
/// depthwise conv1d, SiLU.
///
/// `conv_state` is `[channels * state_len]`, mutated in-place.
/// `hidden` is `[channels]` (single token).
/// `weight` is `[channels * kernel_size]` (depthwise, one filter per channel).
/// Returns `[channels]`.
pub fn gdn_causal_conv1d_update(
    hidden: &[f32],         // [channels]
    conv_state: &mut [f32], // [channels * state_len], mutated in-place
    weight: &[f32], // [channels * kernel_size] (depthwise: weight[channel * kernel_size + k])
    out: &mut [f32], // [channels]
) -> Result<(), GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let channels = hidden.len();
    if out.len() != channels || channels == 0 {
        return Err(GdnError::Shape);
    }
    let state_len = conv_state.len() / channels;
    let kernel_size = weight.len() / channels;
    if conv_state.len() != channels * state_len || weight.len() != channels * kernel_size {
        return Err(GdnError::Shape);
    }

    let concat_len = state_len + 1;

    // Save old state — the HF reference cats [conv_state, hidden] BEFORE updating conv_state.
    let mut old_state = Vec::with_capacity(conv_state.len());
    old_state.extend_from_slice(conv_state);

    // Reset conv_state for the proper update
    let mut c = 0usize;
    while c < channels {
        let cs_base = c * state_len;

        // Build concat: [old_state[c], hidden[c]]
        let mut concat = vec![0.0f32; concat_len];
        let mut i = 0usize;
        while i < state_len {
            concat[i] = old_state[cs_base + i];
            i += 1;
        }
        concat[state_len] = hidden[c];

        // Depthwise conv1d: output[c] = sum over k of concat[k] * weight[c * kernel_size + k]
        let w_base = c * kernel_size;
        let mut acc = 0.0f32;
        let mut k = 0usize;
        while k < kernel_size {
            if k < concat_len {
                acc += concat[k] * weight[w_base + k];
            }
            k += 1;
        }

        // Update conv_state: shift left, append hidden[c]
        let mut i2 = 0usize;
        while i2 < state_len - 1 {
            conv_state[cs_base + i2] = old_state[cs_base + i2 + 1];
            i2 += 1;
        }
        conv_state[cs_base + state_len - 1] = hidden[c];

        // SiLU activation: x * sigmoid(x)
        out[c] = silu_f32(acc);

        c += 1;
    }

    Ok(())
}

/// L2 normalization of a vector.
///
/// HF reference: `l2norm` (modeling_qwen3_5.py:242).
/// llama.cpp/ggml semantics: `x / max(sqrt(sum(x^2)), eps)`.
pub fn gdn_l2norm(x: &[f32], out: &mut [f32], eps: f32) -> Result<(), GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    if x.is_empty() || x.len() != out.len() {
        return Err(GdnError::Shape);
    }
    let mut sum_sq = 0.0f32;
    let mut i = 0usize;
    while i < x.len() {
        sum_sq += x[i] * x[i];
        i += 1;
    }
    let norm = soft_f32::sqrt(sum_sq.to_bits());
    let denom = soft_f32::fmax(norm, eps.to_bits());
    let inv_norm_bits = soft_f32::div(soft_f32::int_to_f32(1), denom);
    let inv_norm = f32::from_bits(inv_norm_bits);
    i = 0;
    while i < x.len() {
        out[i] = x[i] * inv_norm;
        i += 1;
    }
    Ok(())
}

/// RMSNormGated — RMSNorm + SiLU gate.
///
/// HF reference: `Qwen3_5RMSNormGated` (modeling_qwen3_5.py:189).
/// `variance = mean(x^2); x = x * rsqrt(variance + eps) * weight; x = x * silu(gate)`
pub fn gdn_rmsnorm_gated(
    x: &[f32],      // [dim]
    weight: &[f32], // [dim]
    gate: &[f32],   // [dim]
    eps: f32,
    out: &mut [f32], // [dim]
) -> Result<(), GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let dim = x.len();
    if dim == 0 || weight.len() != dim || gate.len() != dim || out.len() != dim {
        return Err(GdnError::Shape);
    }

    // variance = mean(x^2)
    let mut variance = 0.0f32;
    let mut i = 0usize;
    while i < dim {
        variance += x[i] * x[i];
        i += 1;
    }
    variance /= dim as f32;

    // Norm before gate: x * rsqrt(variance + eps) * weight * silu(gate)
    let norm_factor_bits = soft_f32::div(
        soft_f32::int_to_f32(1),
        soft_f32::sqrt(soft_f32::add(variance.to_bits(), eps.to_bits())),
    );
    let norm_factor = f32::from_bits(norm_factor_bits);
    i = 0;
    while i < dim {
        let normed = x[i] * norm_factor * weight[i];
        out[i] = normed * silu_f32(gate[i]);
        i += 1;
    }

    Ok(())
}

/// SiLU (Swish) activation: x * sigmoid(x).
fn silu_f32(x: f32) -> f32 {
    let one = 1.0f32.to_bits();
    let neg_x = (-x).to_bits();
    let exp_neg_x = soft_f32::exp(neg_x);
    let sigmoid_den = soft_f32::add(one, exp_neg_x);
    let sigmoid = soft_f32::div(one, sigmoid_den);
    x * f32::from_bits(sigmoid)
}

/// Sigmoid activation.
fn sigmoid_f32(x: f32) -> f32 {
    let one = 1.0f32.to_bits();
    let neg_x = (-x).to_bits();
    let exp_neg_x = soft_f32::exp(neg_x);
    let sigmoid_den = soft_f32::add(one, exp_neg_x);
    let sigmoid = soft_f32::div(one, sigmoid_den);
    f32::from_bits(sigmoid)
}

/// Softplus activation: ln(1 + exp(x)).
fn softplus_f32(x: f32) -> f32 {
    let one = 1.0f32.to_bits();
    if x > 0.0 {
        let sum = soft_f32::add(one, soft_f32::exp((-x).to_bits()));
        let correction = soft_f32::ln(sum);
        f32::from_bits(soft_f32::add(x.to_bits(), correction))
    } else {
        let sum = soft_f32::add(one, soft_f32::exp(x.to_bits()));
        f32::from_bits(soft_f32::ln(sum))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GDN layer forward — the complete per-token decode step
//
/// Per-token GDN layer forward for a single head. This is called by the walker
/// for each token in the recurrence. The full layer processes all heads.
///
/// Weights layout (matching HF `Qwen3_5GatedDeltaNet`):
/// - `in_proj_qkv`: [hidden_size, key_dim*2 + value_dim]
/// - `in_proj_z`: [hidden_size, value_dim]
/// - `in_proj_b`: [hidden_size, num_v_heads]
/// - `in_proj_a`: [hidden_size, num_v_heads]
/// - `conv1d.weight`: [conv_dim, conv_dim, kernel_size] (depthwise)
/// - `A_log`: [num_v_heads]
/// - `dt_bias`: [num_v_heads]
/// - `norm.weight`: [head_v_dim]
/// - `out_proj`: [value_dim, hidden_size]
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct GdnLayerConfig {
    pub hidden_size: usize,
    pub num_k_heads: usize,
    pub num_v_heads: usize,
    pub head_k_dim: usize,
    pub head_v_dim: usize,
    pub conv_kernel_size: usize,
    pub rms_norm_eps: f32,
}

impl GdnLayerConfig {
    pub fn key_dim(&self) -> usize {
        self.num_k_heads * self.head_k_dim
    }
    pub fn value_dim(&self) -> usize {
        self.num_v_heads * self.head_v_dim
    }
    pub fn conv_dim(&self) -> usize {
        self.key_dim() * 2 + self.value_dim()
    }
    /// Recurrent state size: [heads * k_dim * v_dim] — but heads here is num_v_heads
    /// (after the repeat_interleave in the HF forward).
    pub fn state_size(&self) -> usize {
        self.num_v_heads * self.head_k_dim * self.head_v_dim
    }
    /// Causal-conv cache size: [conv_dim * (kernel_size - 1)].
    pub fn conv_state_size(&self) -> Result<usize, GdnError> {
        if self.conv_kernel_size <= 1 {
            return Err(GdnError::Shape);
        }
        checked_mul(self.conv_dim(), self.conv_kernel_size - 1)
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct GdnLayerWeights<'a> {
    pub in_proj_qkv: &'a [f32],
    pub in_proj_z: &'a [f32],
    pub in_proj_b: &'a [f32],
    pub in_proj_a: &'a [f32],
    pub conv1d_weight: &'a [f32],
    pub a_log: &'a [f32],
    pub dt_bias: &'a [f32],
    pub norm_weight: &'a [f32],
    pub out_proj: &'a [f32],
}

/// Complete single-token GDN layer execution. The caller owns `recurrent_state`
/// and `conv_state` across token decode, exactly like the Qwen2 KV cache.
#[allow(dead_code)]
pub fn gdn_layer_forward(
    hidden: &[f32],
    weights: GdnLayerWeights<'_>,
    config: GdnLayerConfig,
    recurrent_state: &mut [f32],
    conv_state: &mut [f32],
) -> Result<Vec<f32>, GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let projected = gdn_layer_projected(hidden, weights, config, recurrent_state, conv_state)?;
    let mut out = vec![0.0f32; config.hidden_size];
    let mut idx = 0usize;
    while idx < config.hidden_size {
        out[idx] = f32::from_bits(soft_f32::add(
            hidden[idx].to_bits(),
            projected[idx].to_bits(),
        ));
        idx += 1;
    }
    Ok(out)
}

/// GDN attention projection without the decoder-block residual add. Qwen3.5
/// applies this after `attn_norm`, then adds the pre-norm block residual before
/// post-attention norm + FFN.
#[allow(dead_code)]
pub fn gdn_layer_projected(
    hidden: &[f32],
    weights: GdnLayerWeights<'_>,
    config: GdnLayerConfig,
    recurrent_state: &mut [f32],
    conv_state: &mut [f32],
) -> Result<Vec<f32>, GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    validate_layer_shapes(hidden, weights, config, recurrent_state, conv_state)?;

    let conv_dim = config.conv_dim();
    let value_dim = config.value_dim();
    let heads = config.num_v_heads;

    let qkv = matvec(hidden, config.hidden_size, weights.in_proj_qkv, conv_dim)?;
    let z = matvec(hidden, config.hidden_size, weights.in_proj_z, value_dim)?;
    let beta_raw = matvec(hidden, config.hidden_size, weights.in_proj_b, heads)?;
    let a_raw = matvec(hidden, config.hidden_size, weights.in_proj_a, heads)?;

    let normed = gdn_layer_core_projected(
        GdnProjectedInputs {
            qkv: &qkv,
            z: &z,
            beta_raw: &beta_raw,
            a_raw: &a_raw,
        },
        weights,
        config,
        recurrent_state,
        conv_state,
    )?;
    matvec(&normed, value_dim, weights.out_proj, config.hidden_size)
}

/// Run the storage-independent GDN core after the caller has projected the
/// current hidden state. This is the common recurrence used by f32 KATs and by
/// the real inventory path, where projections stay in their original GGUF
/// quantization and are evaluated by the shared mixed-quant matmul.
#[allow(dead_code)]
pub fn gdn_layer_core_projected(
    inputs: GdnProjectedInputs<'_>,
    weights: GdnLayerWeights<'_>,
    config: GdnLayerConfig,
    recurrent_state: &mut [f32],
    conv_state: &mut [f32],
) -> Result<Vec<f32>, GdnError> {
    soft_f32::debug_assert_kernel_mxcsr();
    validate_core_shapes(inputs, weights, config, recurrent_state, conv_state)?;

    let conv_dim = config.conv_dim();
    let key_dim = config.key_dim();
    let value_dim = config.value_dim();
    let k_heads = config.num_k_heads;
    let heads = config.num_v_heads;

    let mut conv_out = vec![0.0f32; conv_dim];
    gdn_causal_conv1d_update(inputs.qkv, conv_state, weights.conv1d_weight, &mut conv_out)?;

    let q_raw = &conv_out[0..key_dim];
    let k_raw = &conv_out[key_dim..key_dim * 2];
    let v = &conv_out[key_dim * 2..key_dim * 2 + value_dim];

    let mut q_k_heads = vec![0.0f32; key_dim];
    let mut k_k_heads = vec![0.0f32; key_dim];
    l2norm_heads(
        q_raw,
        &mut q_k_heads,
        k_heads,
        config.head_k_dim,
        config.rms_norm_eps,
    )?;
    l2norm_heads(
        k_raw,
        &mut k_k_heads,
        k_heads,
        config.head_k_dim,
        config.rms_norm_eps,
    )?;
    let q = repeat_heads(&q_k_heads, k_heads, heads, config.head_k_dim)?;
    let k = repeat_heads(&k_k_heads, k_heads, heads, config.head_k_dim)?;
    let q_scale = f32::from_bits(soft_f32::div(
        soft_f32::int_to_f32(1),
        soft_f32::sqrt(soft_f32::int_to_f32(config.head_k_dim as i32)),
    ));

    let mut beta = vec![0.0f32; heads];
    let mut gate = vec![0.0f32; heads];
    let mut h = 0usize;
    while h < heads {
        beta[h] = sigmoid_f32(inputs.beta_raw[h]);
        let a_plus_dt = f32::from_bits(soft_f32::add(
            inputs.a_raw[h].to_bits(),
            weights.dt_bias[h].to_bits(),
        ));
        let a_softplus = softplus_f32(a_plus_dt);
        // Qwen3.5 GGUF stores ssm_a as the negative decay multiplier used by
        // llama.cpp directly: gate = exp(softplus(alpha + dt_bias) * ssm_a).
        gate[h] = f32::from_bits(soft_f32::exp(soft_f32::mul(
            a_softplus.to_bits(),
            weights.a_log[h].to_bits(),
        )));
        h += 1;
    }

    let mut recurrent_out = vec![0.0f32; value_dim];
    gdn_recurrent_step(&q, &k, v, &gate, &beta, recurrent_state, &mut recurrent_out)?;
    let mut recurrent_idx = 0usize;
    while recurrent_idx < recurrent_out.len() {
        // llama.cpp's fused GGML_OP_GATED_DELTA_NET scales the completed
        // recurrent dot, not each Q lane before accumulation.
        recurrent_out[recurrent_idx] *= q_scale;
        recurrent_idx += 1;
    }

    let mut normed = vec![0.0f32; value_dim];
    let mut norm_head = 0usize;
    while norm_head < heads {
        let start = norm_head * config.head_v_dim;
        let end = start + config.head_v_dim;
        gdn_rmsnorm_gated(
            &recurrent_out[start..end],
            weights.norm_weight,
            &inputs.z[start..end],
            config.rms_norm_eps,
            &mut normed[start..end],
        )?;
        norm_head += 1;
    }
    Ok(normed)
}

fn validate_core_shapes(
    inputs: GdnProjectedInputs<'_>,
    weights: GdnLayerWeights<'_>,
    config: GdnLayerConfig,
    recurrent_state: &[f32],
    conv_state: &[f32],
) -> Result<(), GdnError> {
    let conv_dim = config.conv_dim();
    let value_dim = config.value_dim();
    if config.hidden_size == 0
        || config.num_k_heads == 0
        || config.num_v_heads == 0
        || !config.num_v_heads.is_multiple_of(config.num_k_heads)
        || config.head_k_dim == 0
        || config.head_v_dim == 0
        || inputs.qkv.len() != conv_dim
        || inputs.z.len() != value_dim
        || inputs.beta_raw.len() != config.num_v_heads
        || inputs.a_raw.len() != config.num_v_heads
        || recurrent_state.len() != config.state_size()
        || conv_state.len() != config.conv_state_size()?
        || weights.conv1d_weight.len() != checked_mul(conv_dim, config.conv_kernel_size)?
        || weights.a_log.len() != config.num_v_heads
        || weights.dt_bias.len() != config.num_v_heads
        || weights.norm_weight.len() != config.head_v_dim
    {
        return Err(GdnError::Shape);
    }
    Ok(())
}

fn validate_layer_shapes(
    hidden: &[f32],
    weights: GdnLayerWeights<'_>,
    config: GdnLayerConfig,
    recurrent_state: &[f32],
    conv_state: &[f32],
) -> Result<(), GdnError> {
    if config.hidden_size == 0
        || config.num_k_heads == 0
        || config.num_v_heads == 0
        || !config.num_v_heads.is_multiple_of(config.num_k_heads)
        || config.head_k_dim == 0
        || config.head_v_dim == 0
        || hidden.len() != config.hidden_size
        || recurrent_state.len() != config.state_size()
        || conv_state.len() != config.conv_state_size()?
    {
        return Err(GdnError::Shape);
    }
    let conv_dim = config.conv_dim();
    let value_dim = config.value_dim();
    if weights.in_proj_qkv.len() != checked_mul(config.hidden_size, conv_dim)?
        || weights.in_proj_z.len() != checked_mul(config.hidden_size, value_dim)?
        || weights.in_proj_b.len() != checked_mul(config.hidden_size, config.num_v_heads)?
        || weights.in_proj_a.len() != checked_mul(config.hidden_size, config.num_v_heads)?
        || weights.conv1d_weight.len() != checked_mul(conv_dim, config.conv_kernel_size)?
        || weights.a_log.len() != config.num_v_heads
        || weights.dt_bias.len() != config.num_v_heads
        || weights.norm_weight.len() != config.head_v_dim
        || weights.out_proj.len() != checked_mul(value_dim, config.hidden_size)?
    {
        return Err(GdnError::Shape);
    }
    Ok(())
}

fn repeat_heads(
    input: &[f32],
    src_heads: usize,
    dst_heads: usize,
    head_dim: usize,
) -> Result<Vec<f32>, GdnError> {
    if src_heads == 0 || dst_heads == 0 || head_dim == 0 {
        return Err(GdnError::Shape);
    }
    if input.len() != checked_mul(src_heads, head_dim)? || !dst_heads.is_multiple_of(src_heads) {
        return Err(GdnError::Shape);
    }
    if src_heads == dst_heads {
        return Ok(input.to_vec());
    }
    let repeat = dst_heads / src_heads;
    let mut out = vec![0.0f32; checked_mul(dst_heads, head_dim)?];
    let mut src = 0usize;
    while src < src_heads {
        let mut rep = 0usize;
        while rep < repeat {
            let dst = src * repeat + rep;
            let src_start = src * head_dim;
            let dst_start = dst * head_dim;
            out[dst_start..dst_start + head_dim]
                .copy_from_slice(&input[src_start..src_start + head_dim]);
            rep += 1;
        }
        src += 1;
    }
    Ok(out)
}

fn matvec(input: &[f32], rows: usize, rhs: &[f32], cols: usize) -> Result<Vec<f32>, GdnError> {
    if rows == 0 || cols == 0 || input.len() != rows || rhs.len() != checked_mul(rows, cols)? {
        return Err(GdnError::Shape);
    }
    let mut out = vec![0.0f32; cols];
    let mut col = 0usize;
    while col < cols {
        let mut acc = soft_f32::zero();
        let mut row = 0usize;
        while row < rows {
            acc = soft_f32::add(
                acc,
                soft_f32::mul(input[row].to_bits(), rhs[row * cols + col].to_bits()),
            );
            row += 1;
        }
        out[col] = f32::from_bits(acc);
        col += 1;
    }
    Ok(out)
}

fn l2norm_heads(
    input: &[f32],
    out: &mut [f32],
    heads: usize,
    head_dim: usize,
    eps: f32,
) -> Result<(), GdnError> {
    if heads == 0 || head_dim == 0 || input.len() != checked_mul(heads, head_dim)? {
        return Err(GdnError::Shape);
    }
    let mut head = 0usize;
    while head < heads {
        let start = head * head_dim;
        let end = start + head_dim;
        gdn_l2norm(&input[start..end], &mut out[start..end], eps)?;
        head += 1;
    }
    Ok(())
}

fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, GdnError> {
    lhs.checked_mul(rhs).ok_or(GdnError::Shape)
}

// ─────────────────────────────────────────────────────────────────────────────
// Host KATs — byte-oracle fixtures sourced from the HF reference.
//
/// Run the GDN recurrent step KAT against a hand-laid 2-token fixture.
/// Returns the output bytes and final state for comparison.
#[cfg(test)]
pub fn kat_gdn_recurrent_step() -> (Vec<f32>, Vec<f32>) {
    // Minimal fixture: 1 head, k_dim=2, v_dim=2, 2 tokens
    let heads = 1;
    let k_dim = 2;
    let v_dim = 2;

    let mut state = vec![0.0f32; heads * k_dim * v_dim]; // zero-initialized
    let mut out = vec![0.0f32; heads * v_dim];

    // Token 0
    let q0 = [1.0, 0.5];
    let k0 = [0.8, 0.3];
    let v0 = [2.0, 1.0];
    let g0 = [0.9]; // exp already applied
    let beta0 = [0.5]; // sigmoid already applied

    let mut out_buf = vec![0.0f32; heads * v_dim];
    gdn_recurrent_step(&q0, &k0, &v0, &g0, &beta0, &mut state, &mut out_buf).unwrap();
    out.copy_from_slice(&out_buf);

    // Token 1 (state carries over)
    let q1 = [0.5, 1.0];
    let k1 = [0.3, 0.8];
    let v1 = [1.0, 2.0];
    let g1 = [0.8];
    let beta1 = [0.6];

    gdn_recurrent_step(&q1, &k1, &v1, &g1, &beta1, &mut state, &mut out).unwrap();

    (out, state)
}

#[cfg(test)]
pub fn kat_gdn_l2norm() -> Vec<f32> {
    let x = [3.0f32, 4.0f32];
    let mut out = vec![0.0f32; x.len()];
    gdn_l2norm(&x, &mut out, 1e-6).unwrap();
    out
}

#[cfg(test)]
pub fn kat_gdn_rmsnorm_gated() -> Vec<f32> {
    let x = [1.0f32, 2.0f32, 3.0f32];
    let weight = [1.0f32, 1.0f32, 1.0f32];
    let gate = [1.0f32, 1.0f32, 1.0f32];
    let mut out = vec![0.0f32; x.len()];
    gdn_rmsnorm_gated(&x, &weight, &gate, 1e-6, &mut out).unwrap();
    out
}
