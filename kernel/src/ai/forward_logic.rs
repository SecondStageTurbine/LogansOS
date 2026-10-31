// SPDX-License-Identifier: MPL-2.0
//! Qwen2 layer forward scaffold for the default-off GGUF/AI-generate path.
//!
//! BUILD-8.4 introduced the projection/RoPE and SwiGLU pieces. BUILD-8.5 adds
//! the cache-aware GQA attention seam on top of those proven components.
#![allow(dead_code)] // default-off Phase 8 scaffold; host KATs consume it first.

extern crate alloc;

#[cfg(feature = "phase9-real-quant")]
use alloc::string::String;
use alloc::{vec, vec::Vec};

#[cfg(feature = "phase9-real-quant")]
use super::gguf_logic::{GGML_MAX_DIMS, GgmlType, GgufTensorInfo};
use super::kv_layout;
use super::model_logic::ModelConfig;
#[cfg(all(
    feature = "phase9-quantized-matmul",
    not(feature = "phase9-real-quant")
))]
use super::ops::qk_matmul::{self, BlockQ8K};
#[cfg(feature = "phase9-real-quant")]
use super::ops::qk_matmul::{self, BlockQ8K, BlockQ8Zero, QKMatmulKernel};
use super::ops::{self, OpError, RopeYarnParams, TensorView};
use super::tensor_logic::{Dtype, MAX_RANK, Shape, TensorError};

pub const QWEN25_05B_HIDDEN_SIZE: usize = 896;
pub const QWEN25_05B_INTERMEDIATE_SIZE: usize = 4864;
pub const QWEN25_05B_LAYER_COUNT: usize = 24;
pub const QWEN25_05B_Q_HEADS: usize = 14;
pub const QWEN25_05B_KV_HEADS: usize = 2;
pub const QWEN25_05B_HEAD_DIM: usize = 64;
pub const QWEN25_05B_CONTEXT: usize = 4096;
pub const QWEN25_ROPE_THETA: f32 = 1_000_000.0;
// QWEN25_ROPE_THETA.powf(-2.0 / 64.0), pinned here to keep kernel code no_std.
pub const QWEN25_05B_ROPE_THETA_SCALE: f32 = 0.64938164;
// QWEN25_ROPE_THETA.powf(-2.0 / 128.0), used by the 1.5B F2 config KAT.
pub const QWEN25_15B_ROPE_THETA_SCALE: f32 = 0.80584216;
pub const QWEN25_05B_RMS_NORM_EPS: f32 = 1.0e-6;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ForwardLayerError {
    Shape,
    Tensor(TensorError),
    Op(OpError),
    AttentionUnsupported,
    CacheFull,
    CachePosition,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TensorDtype {
    F32,
    Q4K,
    Q5_0,
    Q5K,
    Q6K,
    Q8_0,
}

impl TensorDtype {
    pub const fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::Q4K => "q4_K",
            Self::Q5_0 => "q5_0",
            Self::Q5K => "q5_K",
            Self::Q6K => "q6_K",
            Self::Q8_0 => "q8_0",
        }
    }

    pub const fn is_quantized(self) -> bool {
        !matches!(self, Self::F32)
    }
}

impl From<TensorError> for ForwardLayerError {
    fn from(err: TensorError) -> Self {
        Self::Tensor(err)
    }
}

impl From<OpError> for ForwardLayerError {
    fn from(err: OpError) -> Self {
        Self::Op(err)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Qwen2LayerConfig {
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub q_heads: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
    pub rope_dimension_count: usize,
    pub attn_q_projection_size: usize,
    pub rope_theta_scale_bits: u32,
    pub rms_norm_eps_bits: u32,
}

impl Qwen2LayerConfig {
    pub const fn qwen25_05b() -> Self {
        Self {
            hidden_size: QWEN25_05B_HIDDEN_SIZE,
            intermediate_size: QWEN25_05B_INTERMEDIATE_SIZE,
            q_heads: QWEN25_05B_Q_HEADS,
            kv_heads: QWEN25_05B_KV_HEADS,
            head_dim: QWEN25_05B_HEAD_DIM,
            rope_dimension_count: QWEN25_05B_HEAD_DIM,
            attn_q_projection_size: QWEN25_05B_Q_HEADS * QWEN25_05B_HEAD_DIM,
            rope_theta_scale_bits: QWEN25_05B_ROPE_THETA_SCALE.to_bits(),
            rms_norm_eps_bits: QWEN25_05B_RMS_NORM_EPS.to_bits(),
        }
    }

    pub fn from_model_config(config: &ModelConfig) -> Result<Self, ForwardLayerError> {
        let q_len = checked_mul(config.q_heads, config.head_dim)?;
        let gated_q_len = checked_mul(q_len, 2)?;
        if config.hidden_size == 0
            || config.intermediate_size == 0
            || config.q_heads == 0
            || config.kv_heads == 0
            || config.head_dim == 0
            || config.rope_dimension_count == 0
            || config.rope_dimension_count > config.head_dim
            || !config.rope_dimension_count.is_multiple_of(2)
            || config.q_projection_size == 0
            || config.context == 0
            || config.vocab == 0
            || (config.q_projection_size != q_len && config.q_projection_size != gated_q_len)
            || checked_mul(config.kv_heads, config.head_dim)? > config.hidden_size
            || config.kv_heads > config.q_heads
        {
            return Err(ForwardLayerError::Shape);
        }
        Ok(Self {
            hidden_size: config.hidden_size,
            intermediate_size: config.intermediate_size,
            q_heads: config.q_heads,
            kv_heads: config.kv_heads,
            head_dim: config.head_dim,
            rope_dimension_count: config.rope_dimension_count,
            attn_q_projection_size: config.q_projection_size,
            rope_theta_scale_bits: rope_theta_scale_bits(
                config.rope_theta,
                config.rope_dimension_count,
            )?,
            rms_norm_eps_bits: config.rms_norm_eps.to_bits(),
        })
    }

    pub fn for_model_config(config: Option<ModelConfig>) -> Result<Self, ForwardLayerError> {
        #[cfg(feature = "phase9-data-driven-config")]
        {
            let config = config.ok_or(ForwardLayerError::Shape)?;
            Self::from_model_config(&config)
        }
        #[cfg(not(feature = "phase9-data-driven-config"))]
        {
            let _ = config;
            Ok(Self::qwen25_05b())
        }
    }

    pub const fn q_len(self) -> usize {
        self.q_heads * self.head_dim
    }

    pub const fn kv_len(self) -> usize {
        self.kv_heads * self.head_dim
    }

    pub const fn has_attention_gate(self) -> bool {
        self.attn_q_projection_size == self.q_len() * 2
    }
}

fn rope_theta_scale_bits(
    rope_theta: f32,
    rope_dimensions: usize,
) -> Result<u32, ForwardLayerError> {
    if rope_theta.to_bits() == QWEN25_ROPE_THETA.to_bits() {
        match rope_dimensions {
            64 => return Ok(QWEN25_05B_ROPE_THETA_SCALE.to_bits()),
            128 => return Ok(QWEN25_15B_ROPE_THETA_SCALE.to_bits()),
            _ => {}
        }
    }
    if rope_dimensions == 0 || !rope_theta.is_finite() || rope_theta <= 0.0 {
        return Err(ForwardLayerError::Shape);
    }
    let exponent = -2.0f32 / rope_dimensions as f32;
    let scale = ops::soft_f32::exp(ops::soft_f32::mul(
        exponent.to_bits(),
        ops::soft_f32::ln(rope_theta.to_bits()),
    ));
    let scale_f32 = f32::from_bits(scale);
    if !scale_f32.is_finite() || scale_f32 <= 0.0 {
        return Err(ForwardLayerError::Shape);
    }
    Ok(scale)
}

#[derive(Copy, Clone)]
pub struct Qwen2LayerDtypes {
    pub attn_q: TensorDtype,
    pub attn_k: TensorDtype,
    pub attn_v: TensorDtype,
    pub attn_o: TensorDtype,
    pub ffn_gate: TensorDtype,
    pub ffn_up: TensorDtype,
    pub ffn_down: TensorDtype,
}

impl Qwen2LayerDtypes {
    pub const F32: Self = Self {
        attn_q: TensorDtype::F32,
        attn_k: TensorDtype::F32,
        attn_v: TensorDtype::F32,
        attn_o: TensorDtype::F32,
        ffn_gate: TensorDtype::F32,
        ffn_up: TensorDtype::F32,
        ffn_down: TensorDtype::F32,
    };
}

#[derive(Copy, Clone)]
pub struct Qwen2LayerWeights<'a> {
    pub attn_norm: &'a [f32],
    pub attn_q: &'a [f32],
    pub attn_k: &'a [f32],
    pub attn_v: &'a [f32],
    pub attn_q_bias: Option<&'a [f32]>,
    pub attn_k_bias: Option<&'a [f32]>,
    pub attn_v_bias: Option<&'a [f32]>,
    pub attn_o: &'a [f32],
    pub ffn_norm: &'a [f32],
    pub ffn_gate: &'a [f32],
    pub ffn_up: &'a [f32],
    pub ffn_down: &'a [f32],
    pub dtypes: Qwen2LayerDtypes,
    pub quantized: Qwen2QuantizedLayerWeights<'a>,
}

#[derive(Copy, Clone)]
pub struct Qwen2QuantizedLayerWeights<'a> {
    pub attn_q: Option<&'a [u8]>,
    pub attn_k: Option<&'a [u8]>,
    pub attn_v: Option<&'a [u8]>,
    pub attn_o: Option<&'a [u8]>,
    pub ffn_gate: Option<&'a [u8]>,
    pub ffn_up: Option<&'a [u8]>,
    pub ffn_down: Option<&'a [u8]>,
}

impl<'a> Qwen2QuantizedLayerWeights<'a> {
    pub const NONE: Self = Self {
        attn_q: None,
        attn_k: None,
        attn_v: None,
        attn_o: None,
        ffn_gate: None,
        ffn_up: None,
        ffn_down: None,
    };
}

#[derive(Copy, Clone)]
pub struct QwenAttentionNormWeights<'a> {
    pub q_norm: Option<&'a [f32]>,
    pub k_norm: Option<&'a [f32]>,
}

impl<'a> QwenAttentionNormWeights<'a> {
    pub const NONE: Self = Self {
        q_norm: None,
        k_norm: None,
    };
}

#[derive(Copy, Clone)]
pub struct Qwen2OutputWeights<'a> {
    pub output_norm: &'a [f32],
    pub output: &'a [f32],
    pub output_dtype: TensorDtype,
    pub output_quantized: Option<&'a [u8]>,
    pub output_bias: Option<&'a [f32]>,
}

#[derive(Clone, Debug)]
pub struct Qwen2AttentionSeam {
    pub attn_norm: Vec<f32>,
    pub q: Vec<f32>,
    pub k: Vec<f32>,
    pub v: Vec<f32>,
    pub gate: Option<Vec<f32>>,
    pub q_rope: Vec<f32>,
    pub k_rope: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Qwen2FfnOutput {
    pub ffn_norm: Vec<f32>,
    pub gate: Vec<f32>,
    pub up: Vec<f32>,
    pub swiglu: Vec<f32>,
    pub ffn_out: Vec<f32>,
    pub residual: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Qwen2LayerOutput {
    pub attention: Qwen2AttentionSeam,
    pub ffn_input: Vec<f32>,
    pub ffn: Qwen2FfnOutput,
}

#[derive(Copy, Clone)]
pub struct Qwen2UncachedAttention<'a> {
    pub q_rope_tokens: &'a [f32],
    pub k_rope_tokens: &'a [f32],
    pub value_tokens: &'a [f32],
    pub seq_len: usize,
}

#[derive(Clone, Debug)]
pub struct Qwen2KvCacheLayer {
    config: Qwen2LayerConfig,
    max_context: usize,
    used_len: usize,
    key: Vec<f32>,
    value: Vec<f32>,
}

impl Qwen2KvCacheLayer {
    pub fn new(config: Qwen2LayerConfig, max_context: usize) -> Result<Self, ForwardLayerError> {
        if max_context == 0 || config.kv_len() == 0 {
            return Err(ForwardLayerError::Shape);
        }
        let elements = checked_mul(config.kv_len(), max_context)?;
        Ok(Self {
            config,
            max_context,
            used_len: 0,
            key: vec![0.0; elements],
            value: vec![0.0; elements],
        })
    }

    pub const fn used_len(&self) -> usize {
        self.used_len
    }

    pub const fn max_context(&self) -> usize {
        self.max_context
    }

    /// Store RoPE-rotated K and unrotated V at exactly `position`.
    ///
    /// llama.cpp writes K after RoPE and reads the cached K directly for
    /// attention. Matching that convention here means RoPE is never applied
    /// while reading the cache.
    pub fn append_rope_kv(
        &mut self,
        position: u64,
        k_rope: &[f32],
        v: &[f32],
    ) -> Result<usize, ForwardLayerError> {
        if self.config.kv_len() != k_rope.len() || self.config.kv_len() != v.len() {
            return Err(ForwardLayerError::Shape);
        }
        let position = usize::try_from(position).map_err(|_| ForwardLayerError::CachePosition)?;
        if position != self.used_len {
            return Err(ForwardLayerError::CachePosition);
        }
        if self.used_len >= self.max_context {
            return Err(ForwardLayerError::CacheFull);
        }
        let pos = self.used_len;
        kv_layout::visit_token(
            self.config.kv_heads,
            self.config.head_dim,
            |head, dim, src| {
                let dst = kv_layout::storage_index(
                    head,
                    pos,
                    dim,
                    self.max_context,
                    self.config.head_dim,
                )?;
                self.key[dst] = k_rope[src];
                self.value[dst] = v[src];
                Ok::<(), ForwardLayerError>(())
            },
        )?;
        self.used_len += 1;
        Ok(pos)
    }

    /// Last-position attention over this cache, read in place (lever 1c,
    /// Plans/NOTE-2026-10-26-arthur-lever1c-kv-in-place.md). Same bits as
    /// copying the active prefix out (`active_key`/`active_value`), building
    /// the `[q_heads, used_len, head_dim]` query and calling
    /// `ops::gqa_causal_attention_last`, without the copies.
    pub fn attention_last(&self, q_rope: &[f32]) -> Result<Vec<f32>, ForwardLayerError> {
        if self.used_len == 0 || q_rope.len() != self.config.q_len() {
            return Err(ForwardLayerError::Shape);
        }
        let params = ops::GqaAttentionParams {
            seq_len: self.used_len as u64,
            n_q_heads: self.config.q_heads as u64,
            n_kv_heads: self.config.kv_heads as u64,
            head_dim: self.config.head_dim as u64,
        };
        let head_stride = checked_mul(self.max_context, self.config.head_dim)?;
        let mut out = vec![0.0; self.config.q_len()];
        // Lever 2: past the threshold, the q heads fork across cores; the same
        // per-head arithmetic, so the same bytes.
        #[cfg(feature = "par-elementwise")]
        if crate::ai::par_matmul::should_parallelize_attention(params) {
            crate::ai::par_matmul::run_attention(
                q_rope,
                &self.key,
                &self.value,
                head_stride,
                params,
                &mut out,
            )?;
            return Ok(out);
        }
        ops::gqa_causal_attention_last_strided(
            q_rope,
            &self.key,
            &self.value,
            head_stride,
            params,
            &mut out,
        )?;
        Ok(out)
    }

    /// Fills every position past `used_len` with `value` (test only), so a
    /// read past the active prefix shows in the result.
    #[cfg(test)]
    pub fn fill_unused_for_test(&mut self, value: f32) {
        for head in 0..self.config.kv_heads {
            for pos in self.used_len..self.max_context {
                for dim in 0..self.config.head_dim {
                    let idx = (head * self.max_context + pos) * self.config.head_dim + dim;
                    self.key[idx] = value;
                    self.value[idx] = value;
                }
            }
        }
    }

    pub fn active_key(&self) -> Result<Vec<f32>, ForwardLayerError> {
        self.active_cache(&self.key)
    }

    pub fn active_value(&self) -> Result<Vec<f32>, ForwardLayerError> {
        self.active_cache(&self.value)
    }

    /// Deliberately corrupt one retained K position for the KV-persist KAT's
    /// negative control. Test-feature-only: production cannot call this hook.
    #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
    pub fn zero_key_position_for_kat(
        &mut self,
        position: usize,
    ) -> Result<bool, ForwardLayerError> {
        if position >= self.used_len {
            return Err(ForwardLayerError::CachePosition);
        }
        let mut changed = false;
        kv_layout::visit_token(
            self.config.kv_heads,
            self.config.head_dim,
            |head, dim, _| {
                let dst = kv_layout::storage_index(
                    head,
                    position,
                    dim,
                    self.max_context,
                    self.config.head_dim,
                )?;
                changed |= self.key[dst] != 0.0;
                self.key[dst] = 0.0;
                Ok::<(), ForwardLayerError>(())
            },
        )?;
        Ok(changed)
    }

    fn active_cache(&self, storage: &[f32]) -> Result<Vec<f32>, ForwardLayerError> {
        if self.used_len == 0 {
            return Err(ForwardLayerError::Shape);
        }
        let mut out = Vec::with_capacity(checked_mul(self.used_len, self.config.kv_len())?);
        kv_layout::visit_active_prefix(
            self.config.kv_heads,
            self.used_len,
            self.max_context,
            self.config.head_dim,
            |_, _, _, src| {
                out.push(storage[src]);
                Ok::<(), ForwardLayerError>(())
            },
        )?;
        Ok(out)
    }
}

#[derive(Clone, Debug)]
pub struct Qwen2KvCacheSet {
    layers: Vec<Qwen2KvCacheLayer>,
}

impl Qwen2KvCacheSet {
    pub fn new(
        layer_count: usize,
        config: Qwen2LayerConfig,
        max_context: usize,
    ) -> Result<Self, ForwardLayerError> {
        if layer_count == 0 {
            return Err(ForwardLayerError::Shape);
        }
        let mut layers = Vec::with_capacity(layer_count);
        let mut idx = 0usize;
        while idx < layer_count {
            layers.push(Qwen2KvCacheLayer::new(config, max_context)?);
            idx += 1;
        }
        Ok(Self { layers })
    }

    pub fn layer_mut(&mut self, idx: usize) -> Result<&mut Qwen2KvCacheLayer, ForwardLayerError> {
        self.layers.get_mut(idx).ok_or(ForwardLayerError::Shape)
    }

    pub fn layers(&self) -> &[Qwen2KvCacheLayer] {
        &self.layers
    }
}

/// RMSNorm -> Q/K/V projections -> RoPE(Q/K).
///
/// This is the explicit BUILD-8.4 attention seam. BUILD-8.5 consumes the
/// returned Q/K/V values to add GQA attention and KV cache behavior.
pub fn attention_seam(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
    position: u64,
) -> Result<Qwen2AttentionSeam, ForwardLayerError> {
    attention_seam_with_norms(
        input,
        weights,
        config,
        position,
        QwenAttentionNormWeights::NONE,
    )
}

pub fn attention_seam_with_norms(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
    position: u64,
    norms: QwenAttentionNormWeights<'_>,
) -> Result<Qwen2AttentionSeam, ForwardLayerError> {
    validate_attention_shapes(input, weights, config)?;
    validate_attention_norm_shapes(norms, config)?;

    let attn_norm = rms_norm_vec(input, weights.attn_norm, config.rms_norm_eps_bits)?;
    let mut q_full = matmul_vec_weight(
        &attn_norm,
        1,
        config.hidden_size,
        weights.attn_q,
        weights.dtypes.attn_q,
        weights.quantized.attn_q,
        config.attn_q_projection_size,
    )?;
    add_optional_bias(
        &mut q_full,
        weights.attn_q_bias,
        config.attn_q_projection_size,
    )?;
    let (mut q, gate) = split_attention_q_gate(&q_full, config)?;
    if let Some(q_norm) = norms.q_norm {
        q = rms_norm_heads(
            &q,
            q_norm,
            config.q_heads,
            config.head_dim,
            config.rms_norm_eps_bits,
        )?;
    }
    let mut k = matmul_vec_weight(
        &attn_norm,
        1,
        config.hidden_size,
        weights.attn_k,
        weights.dtypes.attn_k,
        weights.quantized.attn_k,
        config.kv_len(),
    )?;
    add_optional_bias(&mut k, weights.attn_k_bias, config.kv_len())?;
    if let Some(k_norm) = norms.k_norm {
        k = rms_norm_heads(
            &k,
            k_norm,
            config.kv_heads,
            config.head_dim,
            config.rms_norm_eps_bits,
        )?;
    }
    let mut v = matmul_vec_weight(
        &attn_norm,
        1,
        config.hidden_size,
        weights.attn_v,
        weights.dtypes.attn_v,
        weights.quantized.attn_v,
        config.kv_len(),
    )?;
    add_optional_bias(&mut v, weights.attn_v_bias, config.kv_len())?;
    let q_rope = rope_vec(&q, config.q_heads, config.head_dim, config, position)?;
    let k_rope = rope_vec(&k, config.kv_heads, config.head_dim, config, position)?;

    Ok(Qwen2AttentionSeam {
        attn_norm,
        q,
        k,
        v,
        gate,
        q_rope,
        k_rope,
    })
}

/// FFN half of a Qwen2 decoder layer:
/// `x + down_proj(swiglu(gate_proj(rms_norm(x)), up_proj(rms_norm(x))))`.
pub fn swiglu_ffn(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
) -> Result<Qwen2FfnOutput, ForwardLayerError> {
    validate_ffn_shapes(input, weights, config)?;

    let ffn_norm = rms_norm_vec(input, weights.ffn_norm, config.rms_norm_eps_bits)?;
    let gate = matmul_vec_weight(
        &ffn_norm,
        1,
        config.hidden_size,
        weights.ffn_gate,
        weights.dtypes.ffn_gate,
        weights.quantized.ffn_gate,
        config.intermediate_size,
    )?;
    let up = matmul_vec_weight(
        &ffn_norm,
        1,
        config.hidden_size,
        weights.ffn_up,
        weights.dtypes.ffn_up,
        weights.quantized.ffn_up,
        config.intermediate_size,
    )?;
    #[cfg(feature = "prefill-perf")]
    let perf_t0 = crate::ai::prefill_perf::tsc();
    let swiglu = swiglu_vec(&gate, &up)?;
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::swiglu(crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0));
    let ffn_out = matmul_vec_weight(
        &swiglu,
        1,
        config.intermediate_size,
        weights.ffn_down,
        weights.dtypes.ffn_down,
        weights.quantized.ffn_down,
        config.hidden_size,
    )?;
    let residual = add_vec(input, &ffn_out)?;

    Ok(Qwen2FfnOutput {
        ffn_norm,
        gate,
        up,
        swiglu,
        ffn_out,
        residual,
    })
}

/// Full layer scaffold with the BUILD-8.5 attention result injected.
///
/// Passing `None` makes the unsupported attention seam explicit, rather than
/// silently treating attention as zero or bypassing it.
pub fn forward_layer_with_attention_output(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
    position: u64,
    attention_output: Option<&[f32]>,
) -> Result<Qwen2LayerOutput, ForwardLayerError> {
    validate_layer_shapes(input, weights, config)?;
    let attention = attention_seam(input, weights, config, position)?;
    let attention_output = attention_output.ok_or(ForwardLayerError::AttentionUnsupported)?;
    if attention_output.len() != config.hidden_size {
        return Err(ForwardLayerError::Shape);
    }
    let ffn_input = add_vec(input, attention_output)?;
    let ffn = swiglu_ffn(&ffn_input, weights, config)?;
    Ok(Qwen2LayerOutput {
        attention,
        ffn_input,
        ffn,
    })
}

/// Full single-position layer step with BUILD-8.5 GQA attention.
///
/// When `kv_cache_enabled` is true, the current RoPE'd K/V are appended to the
/// fixed-capacity cache and the attention read uses the active cache prefix.
/// When false, the caller supplies a recompute-all prefix and the cache is not
/// read or written; this is the kill-switch path.
pub fn forward_layer_with_gqa_attention(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
    position: u64,
    kv_cache_enabled: bool,
    cache: Option<&mut Qwen2KvCacheLayer>,
    uncached: Option<Qwen2UncachedAttention<'_>>,
) -> Result<Qwen2LayerOutput, ForwardLayerError> {
    forward_layer_with_gqa_attention_with_norms(
        input,
        weights,
        config,
        position,
        kv_cache_enabled,
        cache,
        uncached,
        QwenAttentionNormWeights::NONE,
    )
}

#[allow(clippy::too_many_arguments)] // same layer step as the kill-switch path, plus optional Q/K norms
pub fn forward_layer_with_gqa_attention_with_norms(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
    position: u64,
    kv_cache_enabled: bool,
    cache: Option<&mut Qwen2KvCacheLayer>,
    uncached: Option<Qwen2UncachedAttention<'_>>,
    norms: QwenAttentionNormWeights<'_>,
) -> Result<Qwen2LayerOutput, ForwardLayerError> {
    ops::soft_f32::debug_assert_kernel_mxcsr();
    validate_layer_shapes(input, weights, config)?;
    let q_len = config.q_len();
    let attention = attention_seam_with_norms(input, weights, config, position, norms)?;
    let attention_output = if kv_cache_enabled {
        let cache = cache.ok_or(ForwardLayerError::AttentionUnsupported)?;
        gqa_attention_cached_decode(&attention, cache, config, position)?
    } else {
        let uncached = uncached.ok_or(ForwardLayerError::AttentionUnsupported)?;
        gqa_attention_uncached_decode(
            uncached.q_rope_tokens,
            uncached.k_rope_tokens,
            uncached.value_tokens,
            config,
            uncached.seq_len,
        )?
    };
    if !rhs_storage_valid(
        weights.attn_o,
        weights.dtypes.attn_o,
        weights.quantized.attn_o,
        checked_mul(q_len, config.hidden_size)?,
    ) {
        return Err(ForwardLayerError::Shape);
    }
    let attention_output = apply_attention_gate(&attention_output, attention.gate.as_deref())?;
    let attention_output = matmul_vec_weight(
        &attention_output,
        1,
        q_len,
        weights.attn_o,
        weights.dtypes.attn_o,
        weights.quantized.attn_o,
        config.hidden_size,
    )?;
    let ffn_input = add_vec(input, &attention_output)?;
    let ffn = swiglu_ffn(&ffn_input, weights, config)?;
    Ok(Qwen2LayerOutput {
        attention,
        ffn_input,
        ffn,
    })
}

pub fn gqa_attention_cached_decode(
    attention: &Qwen2AttentionSeam,
    cache: &mut Qwen2KvCacheLayer,
    config: Qwen2LayerConfig,
    position: u64,
) -> Result<Vec<f32>, ForwardLayerError> {
    if cache.config != config {
        return Err(ForwardLayerError::Shape);
    }
    #[cfg(feature = "prefill-perf")]
    let perf_t0 = crate::ai::prefill_perf::tsc();
    cache.append_rope_kv(position, &attention.k_rope, &attention.v)?;
    let out = cache.attention_last(&attention.q_rope);
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::attn(crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0));
    out
}

pub fn gqa_attention_uncached_decode(
    q_rope_tokens: &[f32],
    k_rope_tokens: &[f32],
    value_tokens: &[f32],
    config: Qwen2LayerConfig,
    seq_len: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if seq_len == 0
        || q_rope_tokens.len() != checked_mul(seq_len, config.q_len())?
        || k_rope_tokens.len() != checked_mul(seq_len, config.kv_len())?
        || value_tokens.len() != checked_mul(seq_len, config.kv_len())?
    {
        return Err(ForwardLayerError::Shape);
    }
    let query = token_major_to_gqa(q_rope_tokens, seq_len, config.q_heads, config.head_dim)?;
    let key = token_major_to_gqa(k_rope_tokens, seq_len, config.kv_heads, config.head_dim)?;
    let value = token_major_to_gqa(value_tokens, seq_len, config.kv_heads, config.head_dim)?;
    gqa_attention_last(&query, &key, &value, config, seq_len)
}

/// Final Qwen2 decoder head: `RMSNorm(hidden) * lm_head + optional bias`.
pub fn qwen2_output_logits(
    input: &[f32],
    weights: Qwen2OutputWeights<'_>,
    config: Qwen2LayerConfig,
    vocab_size: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    ops::soft_f32::debug_assert_kernel_mxcsr();
    if input.is_empty()
        || vocab_size == 0
        || weights.output_norm.len() != input.len()
        || !rhs_storage_valid(
            weights.output,
            weights.output_dtype,
            weights.output_quantized,
            input.len() * vocab_size,
        )
        || weights
            .output_bias
            .is_some_and(|bias| bias.len() != vocab_size)
    {
        return Err(ForwardLayerError::Shape);
    }
    let norm = rms_norm_vec(input, weights.output_norm, config.rms_norm_eps_bits)?;
    let mut logits = matmul_vec_weight(
        &norm,
        1,
        input.len(),
        weights.output,
        weights.output_dtype,
        weights.output_quantized,
        vocab_size,
    )?;
    if let Some(bias) = weights.output_bias {
        let mut idx = 0usize;
        while idx < vocab_size {
            logits[idx] += bias[idx];
            idx += 1;
        }
    }
    Ok(logits)
}

fn validate_layer_shapes(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
) -> Result<(), ForwardLayerError> {
    validate_attention_shapes(input, weights, config)?;
    validate_ffn_shapes(input, weights, config)?;
    Ok(())
}

fn validate_attention_shapes(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
) -> Result<(), ForwardLayerError> {
    let q_len = checked_mul(config.q_heads, config.head_dim)?;
    let kv_len = checked_mul(config.kv_heads, config.head_dim)?;
    let gated_q_len = checked_mul(q_len, 2)?;
    let attn_q_storage = checked_mul(config.hidden_size, config.attn_q_projection_size)?;
    let attn_kv_storage = checked_mul(config.hidden_size, kv_len)?;
    if config.hidden_size == 0
        || config.q_heads == 0
        || config.kv_heads == 0
        || config.head_dim == 0
        || q_len == 0
        || kv_len == 0
        || !(config.attn_q_projection_size == q_len || config.attn_q_projection_size == gated_q_len)
        || input.len() != config.hidden_size
        || weights.attn_norm.len() != config.hidden_size
        || !rhs_storage_valid(
            weights.attn_q,
            weights.dtypes.attn_q,
            weights.quantized.attn_q,
            attn_q_storage,
        )
        || !rhs_storage_valid(
            weights.attn_k,
            weights.dtypes.attn_k,
            weights.quantized.attn_k,
            attn_kv_storage,
        )
        || !rhs_storage_valid(
            weights.attn_v,
            weights.dtypes.attn_v,
            weights.quantized.attn_v,
            attn_kv_storage,
        )
        || weights
            .attn_q_bias
            .is_some_and(|bias| bias.len() != config.attn_q_projection_size)
        || weights.attn_k_bias.is_some_and(|bias| bias.len() != kv_len)
        || weights.attn_v_bias.is_some_and(|bias| bias.len() != kv_len)
    {
        return Err(ForwardLayerError::Shape);
    }
    Ok(())
}

fn validate_attention_norm_shapes(
    norms: QwenAttentionNormWeights<'_>,
    config: Qwen2LayerConfig,
) -> Result<(), ForwardLayerError> {
    if norms
        .q_norm
        .is_some_and(|norm| norm.len() != config.head_dim)
        || norms
            .k_norm
            .is_some_and(|norm| norm.len() != config.head_dim)
    {
        return Err(ForwardLayerError::Shape);
    }
    Ok(())
}

fn split_attention_q_gate(
    q_full: &[f32],
    config: Qwen2LayerConfig,
) -> Result<(Vec<f32>, Option<Vec<f32>>), ForwardLayerError> {
    if q_full.len() != config.attn_q_projection_size {
        return Err(ForwardLayerError::Shape);
    }
    if config.attn_q_projection_size == config.q_len() {
        return Ok((q_full.to_vec(), None));
    }
    if config.attn_q_projection_size == checked_mul(config.q_len(), 2)? {
        let mut q = vec![0.0f32; config.q_len()];
        let mut gate = vec![0.0f32; config.q_len()];
        let mut head = 0usize;
        while head < config.q_heads {
            let src = checked_mul(checked_mul(head, config.head_dim)?, 2)?;
            let dst = checked_mul(head, config.head_dim)?;
            let mut dim = 0usize;
            while dim < config.head_dim {
                q[dst + dim] = q_full[src + dim];
                gate[dst + dim] = q_full[src + config.head_dim + dim];
                dim += 1;
            }
            head += 1;
        }
        return Ok((q, Some(gate)));
    }
    Err(ForwardLayerError::Shape)
}

fn rms_norm_heads(
    input: &[f32],
    weights: &[f32],
    heads: usize,
    head_dim: usize,
    eps_bits: u32,
) -> Result<Vec<f32>, ForwardLayerError> {
    if heads == 0 || head_dim == 0 || input.len() != checked_mul(heads, head_dim)? {
        return Err(ForwardLayerError::Shape);
    }
    if weights.len() != head_dim {
        return Err(ForwardLayerError::Shape);
    }
    let mut out = Vec::with_capacity(input.len());
    let mut head = 0usize;
    while head < heads {
        let start = checked_mul(head, head_dim)?;
        let end = checked_add(start, head_dim)?;
        out.extend_from_slice(&rms_norm_vec(&input[start..end], weights, eps_bits)?);
        head += 1;
    }
    Ok(out)
}

fn apply_attention_gate(
    attention: &[f32],
    gate: Option<&[f32]>,
) -> Result<Vec<f32>, ForwardLayerError> {
    let Some(gate) = gate else {
        return Ok(attention.to_vec());
    };
    if gate.len() != attention.len() {
        return Err(ForwardLayerError::Shape);
    }
    let mut out = Vec::with_capacity(attention.len());
    let mut idx = 0usize;
    while idx < attention.len() {
        let sigmoid = sigmoid_bits(gate[idx].to_bits());
        out.push(f32::from_bits(ops::soft_f32::mul(
            attention[idx].to_bits(),
            sigmoid,
        )));
        idx += 1;
    }
    Ok(out)
}

fn sigmoid_bits(x: u32) -> u32 {
    let one = 0x3f80_0000;
    let sigmoid_den = ops::soft_f32::add(one, ops::soft_f32::exp(x ^ 0x8000_0000));
    ops::soft_f32::div(one, sigmoid_den)
}

fn validate_ffn_shapes(
    input: &[f32],
    weights: Qwen2LayerWeights<'_>,
    config: Qwen2LayerConfig,
) -> Result<(), ForwardLayerError> {
    if config.hidden_size == 0
        || config.intermediate_size == 0
        || input.len() != config.hidden_size
        || weights.ffn_norm.len() != config.hidden_size
        || !rhs_storage_valid(
            weights.ffn_gate,
            weights.dtypes.ffn_gate,
            weights.quantized.ffn_gate,
            config.hidden_size * config.intermediate_size,
        )
        || !rhs_storage_valid(
            weights.ffn_up,
            weights.dtypes.ffn_up,
            weights.quantized.ffn_up,
            config.hidden_size * config.intermediate_size,
        )
        || !rhs_storage_valid(
            weights.ffn_down,
            weights.dtypes.ffn_down,
            weights.quantized.ffn_down,
            config.intermediate_size * config.hidden_size,
        )
    {
        return Err(ForwardLayerError::Shape);
    }
    Ok(())
}

fn rhs_storage_valid(
    rhs_f32: &[f32],
    rhs_dtype: TensorDtype,
    rhs_quantized: Option<&[u8]>,
    f32_len: usize,
) -> bool {
    if rhs_dtype.is_quantized() {
        rhs_quantized.is_some()
    } else {
        rhs_quantized.is_none() && rhs_f32.len() == f32_len
    }
}

fn matmul_vec(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs: &[f32],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if lhs.len() != lhs_rows * lhs_cols || rhs.len() != lhs_cols * rhs_cols {
        return Err(ForwardLayerError::Shape);
    }
    let lhs_shape = shape2(lhs_rows as u64, lhs_cols as u64)?;
    let rhs_shape = shape2(lhs_cols as u64, rhs_cols as u64)?;
    let lhs_bytes = bytes_from_f32(lhs);
    let rhs_bytes = bytes_from_f32(rhs);
    let mut out = vec![0u8; lhs_rows * rhs_cols * 4];
    ops::matmul(
        &view(&lhs_shape, &lhs_bytes),
        &view(&rhs_shape, &rhs_bytes),
        &mut out,
    )?;
    Ok(f32_from_bytes(&out))
}

fn matmul_vec_weight(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_f32: &[f32],
    rhs_dtype: TensorDtype,
    rhs_quantized: Option<&[u8]>,
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    #[cfg(feature = "phase9-real-quant")]
    {
        if rhs_dtype.is_quantized() {
            let rhs_quantized = rhs_quantized.ok_or(ForwardLayerError::Shape)?;
            #[cfg(feature = "prefill-perf")]
            let perf_t0 = crate::ai::prefill_perf::tsc();
            let out =
                matmul_vec_dequantized(lhs, lhs_rows, lhs_cols, rhs_dtype, rhs_quantized, rhs_cols);
            #[cfg(feature = "prefill-perf")]
            crate::ai::prefill_perf::matvec(crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0));
            return out;
        }
    }

    #[cfg(all(
        feature = "phase9-quantized-matmul",
        not(feature = "phase9-real-quant")
    ))]
    {
        if rhs_dtype == TensorDtype::Q4K
            && let Some(rhs_q4_k) = rhs_quantized
        {
            return matmul_vec_quantized(lhs, lhs_rows, lhs_cols, rhs_q4_k, rhs_cols);
        }
    }
    #[cfg(not(any(feature = "phase9-quantized-matmul", feature = "phase9-real-quant")))]
    let _ = (rhs_dtype, rhs_quantized);

    matmul_vec(lhs, lhs_rows, lhs_cols, rhs_f32, rhs_cols)
}

/// Shared storage-aware matrix projection used by attention, FFN, and the
/// Qwen3.5 GDN inventory runner. Quantized weights stay in their GGUF block
/// representation and use the same native mixed-quant path as the rest of the
/// decoder; f32 weights retain the deterministic soft-float matmul.
pub fn matmul_vec_mixed_weight(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_f32: &[f32],
    rhs_dtype: TensorDtype,
    rhs_quantized: Option<&[u8]>,
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    matmul_vec_weight(
        lhs,
        lhs_rows,
        lhs_cols,
        rhs_f32,
        rhs_dtype,
        rhs_quantized,
        rhs_cols,
    )
}

#[cfg(feature = "phase9-real-quant")]
fn matmul_vec_dequantized(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if lhs_rows == 0
        || lhs_cols == 0
        || rhs_cols == 0
        || lhs.len() != checked_mul(lhs_rows, lhs_cols)?
    {
        return Err(ForwardLayerError::Shape);
    }
    matmul_vec_quantized_native(lhs, lhs_rows, lhs_cols, rhs_dtype, rhs_quantized, rhs_cols)
}

#[cfg(feature = "phase9-real-quant")]
fn matmul_vec_quantized_native(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    match rhs_dtype {
        TensorDtype::F32 => Err(ForwardLayerError::Shape),
        TensorDtype::Q4K | TensorDtype::Q5K | TensorDtype::Q6K => matmul_vec_quantized_native_q8_k(
            lhs,
            lhs_rows,
            lhs_cols,
            rhs_dtype,
            rhs_quantized,
            rhs_cols,
        ),
        TensorDtype::Q5_0 | TensorDtype::Q8_0 => matmul_vec_quantized_native_q8_0(
            lhs,
            lhs_rows,
            lhs_cols,
            rhs_dtype,
            rhs_quantized,
            rhs_cols,
        ),
    }
}

#[cfg(feature = "phase9-real-quant")]
fn matmul_vec_quantized_native_q8_k(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if !lhs_cols.is_multiple_of(qk_matmul::Q8_K_VALUES) {
        return Err(ForwardLayerError::Shape);
    }
    let block_values = qk_matmul::Q8_K_VALUES;
    let block_bytes = match rhs_dtype {
        TensorDtype::Q4K => ops::q4_k::Q4_K_BLOCK_BYTES,
        TensorDtype::Q5K => qk_matmul::Q5_K_BLOCK_BYTES,
        TensorDtype::Q6K => qk_matmul::Q6_K_BLOCK_BYTES,
        _ => return Err(ForwardLayerError::Shape),
    };
    let blocks_per_col = lhs_cols / block_values;
    let expected = checked_mul(checked_mul(rhs_cols, blocks_per_col)?, block_bytes)?;
    if rhs_quantized.len() != expected {
        return Err(ForwardLayerError::Shape);
    }

    let q8_blocks = checked_mul(lhs_rows, blocks_per_col)?;
    let mut lhs_q8 = vec![BlockQ8K::ZERO; q8_blocks];
    qk_matmul::quantize_row_q8_k_with_kernel(lhs, &mut lhs_q8, QKMatmulKernel::AutoAvx2)
        .map_err(|_| ForwardLayerError::Shape)?;

    let kind = match rhs_dtype {
        TensorDtype::Q4K => MatmulPartitionKind::Q4K,
        TensorDtype::Q5K => MatmulPartitionKind::Q5K,
        TensorDtype::Q6K => MatmulPartitionKind::Q6K,
        _ => return Err(ForwardLayerError::Shape),
    };
    let mut out = vec![0.0f32; checked_mul(lhs_rows, rhs_cols)?];
    // Single-position decode (`lhs_rows == 1`) — every real forward matmul — takes
    // the OT-34 column-partition path (single-threaded whole-column range unless
    // `threaded-matmul` forks it across cores). Multi-row calls keep the exact
    // pre-OT-34 whole-matrix kernel call (column slicing is only contiguous for a
    // single output row).
    if lhs_rows == 1 {
        run_matmul_q8k_row(kind, &lhs_q8, rhs_quantized, rhs_cols, lhs_cols, &mut out)?;
    } else {
        dispatch_q8k_kernel(
            kind,
            &lhs_q8,
            rhs_quantized,
            lhs_rows,
            rhs_cols,
            lhs_cols,
            &mut out,
        )?;
    }
    Ok(out)
}

#[cfg(feature = "phase9-real-quant")]
fn matmul_vec_quantized_native_q8_0(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if !lhs_cols.is_multiple_of(qk_matmul::Q8_0_VALUES) {
        return Err(ForwardLayerError::Shape);
    }
    let block_values = qk_matmul::Q8_0_VALUES;
    let block_bytes = match rhs_dtype {
        TensorDtype::Q5_0 => qk_matmul::Q5_0_BLOCK_BYTES,
        TensorDtype::Q8_0 => qk_matmul::Q8_0_BLOCK_BYTES,
        _ => return Err(ForwardLayerError::Shape),
    };
    let blocks_per_col = lhs_cols / block_values;
    let expected = checked_mul(checked_mul(rhs_cols, blocks_per_col)?, block_bytes)?;
    if rhs_quantized.len() != expected {
        return Err(ForwardLayerError::Shape);
    }

    let q8_blocks = checked_mul(lhs_rows, blocks_per_col)?;
    let mut lhs_q8 = vec![BlockQ8Zero::ZERO; q8_blocks];
    qk_matmul::quantize_row_q8_0_with_kernel(lhs, &mut lhs_q8, QKMatmulKernel::AutoAvx2)
        .map_err(|_| ForwardLayerError::Shape)?;

    let kind = match rhs_dtype {
        TensorDtype::Q5_0 => MatmulPartitionKind::Q5_0,
        TensorDtype::Q8_0 => MatmulPartitionKind::Q8_0,
        _ => return Err(ForwardLayerError::Shape),
    };
    let mut out = vec![0.0f32; checked_mul(lhs_rows, rhs_cols)?];
    if lhs_rows == 1 {
        run_matmul_q8_0_row(kind, &lhs_q8, rhs_quantized, rhs_cols, lhs_cols, &mut out)?;
    } else {
        dispatch_q8_0_kernel(
            kind,
            &lhs_q8,
            rhs_quantized,
            lhs_rows,
            rhs_cols,
            lhs_cols,
            &mut out,
        )?;
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// OT-34: deterministic column-partitioned matmul (the `threaded-matmul` substrate)
//
// The forward matmul is `out[col] = vec_dot(rhs_col, lhs_row)` with a SINGLE LHS
// row (single-position decode). Each output column is an INDEPENDENT dot product
// — there is no cross-column reduction — so splitting the OUTPUT COLUMNS across
// cores keeps every dot product whole inside one core. The assembled output is
// therefore byte-identical regardless of how many partitions ran it: this is
// I-1 (result invariance / the byte oracle) applied to parallelism. Splitting the
// CONTRACTION (`shared`) axis would instead force a cross-core partial-sum
// combine, whose float non-associativity changes bytes and would poison the
// OT-30 parity oracle — so that axis is never split.
//
// The kernel math is UNCHANGED: a partition slices rhs/out to its column range
// (contiguous for a single LHS row) and calls the existing F4 kernels verbatim.
// The single-threaded / feature-off path is the whole-column range `[0, cols)`,
// which is bit-for-bit the pre-OT-34 kernel call.
// ─────────────────────────────────────────────────────────────────────────────

/// Which quantized F4 kernel a matmul partition runs. Selects the LHS Q8 block
/// type (Q8_K vs Q8_0) and the RHS dtype.
#[cfg(any(test, feature = "phase9-real-quant"))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MatmulPartitionKind {
    Q4K,
    Q5K,
    Q6K,
    Q5_0,
    Q8_0,
}

/// Fixed, thread-count-independent contiguous column split. Partition `t` of
/// `parts` owns output columns `[start, end)`; the low `cols % parts` partitions
/// take one extra column. The column set owned by any partition is a pure
/// function of `(cols, parts, t)`, and — because columns are independent dot
/// products — the assembled output is identical for every `parts`. Returns an
/// empty range for out-of-range `t` or `parts == 0`.
pub const fn column_partition(cols: usize, parts: usize, t: usize) -> (usize, usize) {
    if parts == 0 || t >= parts {
        return (0, 0);
    }
    let base = cols / parts;
    let rem = cols % parts;
    let start = t * base + if t < rem { t } else { rem };
    let len = base + if t < rem { 1 } else { 0 };
    (start, start + len)
}

/// Dispatch a whole Q8_K-family matmul (`Q4K`/`Q6K` RHS) to the existing F4
/// kernel. The `AutoAvx2` selector matches the pre-OT-34 native path exactly;
/// the chosen variant (AVX2 vs scalar) is uniform across cores on a homogeneous
/// machine, so every partition produces byte-identical results.
#[cfg(any(test, feature = "phase9-real-quant"))]
fn dispatch_q8k_kernel(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8K],
    rhs: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    let kernel = ops::qk_matmul::QKMatmulKernel::AutoAvx2;
    match kind {
        MatmulPartitionKind::Q4K => ops::qk_matmul::matmul_q4_k_q8_k_with_kernel(
            lhs_q8, rhs, rows, cols, shared, out, kernel,
        ),
        MatmulPartitionKind::Q5K => ops::qk_matmul::matmul_q5_k_q8_k_with_kernel(
            lhs_q8, rhs, rows, cols, shared, out, kernel,
        ),
        MatmulPartitionKind::Q6K => ops::qk_matmul::matmul_q6_k_q8_k_with_kernel(
            lhs_q8, rhs, rows, cols, shared, out, kernel,
        ),
        _ => return Err(ForwardLayerError::Shape),
    }
    .map_err(|_| ForwardLayerError::Shape)
}

/// Dispatch a whole Q8_0-family matmul (`Q5_0`/`Q8_0` RHS) to the existing F4
/// kernel.
#[cfg(any(test, feature = "phase9-real-quant"))]
fn dispatch_q8_0_kernel(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8Zero],
    rhs: &[u8],
    rows: usize,
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    let kernel = ops::qk_matmul::QKMatmulKernel::AutoAvx2;
    match kind {
        MatmulPartitionKind::Q5_0 => ops::qk_matmul::matmul_q5_0_q8_0_with_kernel(
            lhs_q8, rhs, rows, cols, shared, out, kernel,
        ),
        MatmulPartitionKind::Q8_0 => ops::qk_matmul::matmul_q8_0_q8_0_with_kernel(
            lhs_q8, rhs, rows, cols, shared, out, kernel,
        ),
        _ => return Err(ForwardLayerError::Shape),
    }
    .map_err(|_| ForwardLayerError::Shape)
}

/// Compute output columns `[col_start, col_end)` of a single-row Q8_K matmul into
/// `out_part` (whose length MUST equal `col_end - col_start`). `rhs_full` is the
/// FULL weight buffer; this slices out the partition's columns. Byte-for-byte
/// identical to computing those columns inside the whole-matrix call — the kernel
/// is unchanged, only the column range differs. This is the single source of
/// truth for the per-partition math, called by both the sequential driver and
/// the cross-core fork-join executor.
#[cfg(any(test, feature = "phase9-real-quant"))]
#[allow(clippy::too_many_arguments)] // (kind, lhs, rhs, cols, shared, start, end, out) — the matmul's irreducible shape
pub(crate) fn column_matmul_q8k_part(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8K],
    rhs_full: &[u8],
    cols: usize,
    shared: usize,
    col_start: usize,
    col_end: usize,
    out_part: &mut [f32],
) -> Result<(), ForwardLayerError> {
    if col_start > col_end || col_end > cols || out_part.len() != col_end - col_start {
        return Err(ForwardLayerError::Shape);
    }
    if out_part.is_empty() {
        return Ok(());
    }
    if shared == 0 || !shared.is_multiple_of(ops::qk_matmul::Q8_K_VALUES) {
        return Err(ForwardLayerError::Shape);
    }
    let bpd = shared / ops::qk_matmul::Q8_K_VALUES;
    let block_bytes = match kind {
        MatmulPartitionKind::Q4K => ops::q4_k::Q4_K_BLOCK_BYTES,
        MatmulPartitionKind::Q5K => ops::qk_matmul::Q5_K_BLOCK_BYTES,
        MatmulPartitionKind::Q6K => ops::qk_matmul::Q6_K_BLOCK_BYTES,
        _ => return Err(ForwardLayerError::Shape),
    };
    let start_byte = checked_mul(checked_mul(col_start, bpd)?, block_bytes)?;
    let end_byte = checked_mul(checked_mul(col_end, bpd)?, block_bytes)?;
    if end_byte > rhs_full.len() {
        return Err(ForwardLayerError::Shape);
    }
    dispatch_q8k_kernel(
        kind,
        lhs_q8,
        &rhs_full[start_byte..end_byte],
        1,
        col_end - col_start,
        shared,
        out_part,
    )
}

/// Q8_0-family analogue of [`column_matmul_q8k_part`].
#[cfg(any(test, feature = "phase9-real-quant"))]
#[allow(clippy::too_many_arguments)] // (kind, lhs, rhs, cols, shared, start, end, out) — the matmul's irreducible shape
pub(crate) fn column_matmul_q8_0_part(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8Zero],
    rhs_full: &[u8],
    cols: usize,
    shared: usize,
    col_start: usize,
    col_end: usize,
    out_part: &mut [f32],
) -> Result<(), ForwardLayerError> {
    if col_start > col_end || col_end > cols || out_part.len() != col_end - col_start {
        return Err(ForwardLayerError::Shape);
    }
    if out_part.is_empty() {
        return Ok(());
    }
    if shared == 0 || !shared.is_multiple_of(ops::qk_matmul::Q8_0_VALUES) {
        return Err(ForwardLayerError::Shape);
    }
    let bpd = shared / ops::qk_matmul::Q8_0_VALUES;
    let block_bytes = match kind {
        MatmulPartitionKind::Q5_0 => ops::qk_matmul::Q5_0_BLOCK_BYTES,
        MatmulPartitionKind::Q8_0 => ops::qk_matmul::Q8_0_BLOCK_BYTES,
        _ => return Err(ForwardLayerError::Shape),
    };
    let start_byte = checked_mul(checked_mul(col_start, bpd)?, block_bytes)?;
    let end_byte = checked_mul(checked_mul(col_end, bpd)?, block_bytes)?;
    if end_byte > rhs_full.len() {
        return Err(ForwardLayerError::Shape);
    }
    dispatch_q8_0_kernel(
        kind,
        lhs_q8,
        &rhs_full[start_byte..end_byte],
        1,
        col_end - col_start,
        shared,
        out_part,
    )
}

/// Sequential column-partitioned Q8_K matmul: run all `parts` partitions
/// in-thread. This is the host-testable determinism substrate — it produces
/// byte-identical output for every `parts` (including `parts == 1`, the
/// single-threaded reference). The kernel fork-join executor computes the SAME
/// partitions across cores.
#[cfg(any(test, feature = "phase9-real-quant"))]
pub(crate) fn matmul_q8k_columns_seq(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8K],
    rhs_full: &[u8],
    cols: usize,
    shared: usize,
    parts: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    if parts == 0 || out.len() != cols {
        return Err(ForwardLayerError::Shape);
    }
    let mut t = 0usize;
    while t < parts {
        let (start, end) = column_partition(cols, parts, t);
        if start != end {
            column_matmul_q8k_part(
                kind,
                lhs_q8,
                rhs_full,
                cols,
                shared,
                start,
                end,
                &mut out[start..end],
            )?;
        }
        t += 1;
    }
    Ok(())
}

/// Sequential column-partitioned Q8_0 matmul — the Q8_0-family analogue of
/// [`matmul_q8k_columns_seq`].
#[cfg(any(test, feature = "phase9-real-quant"))]
pub(crate) fn matmul_q8_0_columns_seq(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8Zero],
    rhs_full: &[u8],
    cols: usize,
    shared: usize,
    parts: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    if parts == 0 || out.len() != cols {
        return Err(ForwardLayerError::Shape);
    }
    let mut t = 0usize;
    while t < parts {
        let (start, end) = column_partition(cols, parts, t);
        if start != end {
            column_matmul_q8_0_part(
                kind,
                lhs_q8,
                rhs_full,
                cols,
                shared,
                start,
                end,
                &mut out[start..end],
            )?;
        }
        t += 1;
    }
    Ok(())
}

/// Single-row Q8_K matmul with the OT-34 threaded-matmul kill-switch. With the
/// `threaded-matmul` feature on AND the width past the parallel threshold, the
/// columns fork-join across cores; otherwise (the default, and always when the
/// feature is off) it is one whole-matrix call — bit-for-bit the pre-OT-34 path.
#[cfg(any(test, feature = "phase9-real-quant"))]
fn run_matmul_q8k_row(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8K],
    rhs: &[u8],
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    #[cfg(all(feature = "threaded-matmul", feature = "phase9-real-quant"))]
    {
        if crate::ai::par_matmul::should_parallelize(cols) {
            return crate::ai::par_matmul::run_matmul_q8k(kind, lhs_q8, rhs, cols, shared, out);
        }
    }
    column_matmul_q8k_part(kind, lhs_q8, rhs, cols, shared, 0, cols, out)
}

/// Single-row Q8_0 matmul with the OT-34 threaded-matmul kill-switch.
#[cfg(any(test, feature = "phase9-real-quant"))]
fn run_matmul_q8_0_row(
    kind: MatmulPartitionKind,
    lhs_q8: &[ops::qk_matmul::BlockQ8Zero],
    rhs: &[u8],
    cols: usize,
    shared: usize,
    out: &mut [f32],
) -> Result<(), ForwardLayerError> {
    #[cfg(all(feature = "threaded-matmul", feature = "phase9-real-quant"))]
    {
        if crate::ai::par_matmul::should_parallelize(cols) {
            return crate::ai::par_matmul::run_matmul_q8_0(kind, lhs_q8, rhs, cols, shared, out);
        }
    }
    column_matmul_q8_0_part(kind, lhs_q8, rhs, cols, shared, 0, cols, out)
}

#[cfg(feature = "phase9-real-quant")]
fn matmul_vec_quantized_dequant_reference(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    let block_values = quant_block_values(rhs_dtype)?;
    if !lhs_cols.is_multiple_of(block_values) {
        return Err(ForwardLayerError::Shape);
    }
    let block_bytes = quant_block_bytes(rhs_dtype);
    let blocks_per_col = lhs_cols / block_values;
    let expected = checked_mul(checked_mul(rhs_cols, blocks_per_col)?, block_bytes)?;
    if rhs_quantized.len() != expected {
        return Err(ForwardLayerError::Shape);
    }

    let mut out = vec![0.0f32; checked_mul(lhs_rows, rhs_cols)?];
    let mut lhs_row = 0usize;
    while lhs_row < lhs_rows {
        let lhs_base = lhs_row * lhs_cols;
        let mut col = 0usize;
        while col < rhs_cols {
            let mut acc = 0.0f32;
            let mut block_idx = 0usize;
            while block_idx < blocks_per_col {
                let weight_off = (col * blocks_per_col + block_idx) * block_bytes;
                let lhs_off = lhs_base + block_idx * block_values;
                acc += dot_quant_block(
                    &lhs[lhs_off..lhs_off + block_values],
                    rhs_dtype,
                    &rhs_quantized[weight_off..weight_off + block_bytes],
                )?;
                block_idx += 1;
            }
            out[lhs_row * rhs_cols + col] = acc;
            col += 1;
        }
        lhs_row += 1;
    }
    Ok(out)
}

#[cfg(feature = "phase9-real-quant")]
fn quant_block_values(dtype: TensorDtype) -> Result<usize, ForwardLayerError> {
    match dtype {
        TensorDtype::F32 => Err(ForwardLayerError::Shape),
        TensorDtype::Q4K => Ok(super::ops::q4_k::Q4_K_VALUES),
        TensorDtype::Q5_0 => Ok(32),
        TensorDtype::Q5K => Ok(256),
        TensorDtype::Q6K => Ok(256),
        TensorDtype::Q8_0 => Ok(32),
    }
}

#[cfg(feature = "phase9-real-quant")]
fn quant_block_bytes(dtype: TensorDtype) -> usize {
    match dtype {
        TensorDtype::F32 => 0,
        TensorDtype::Q4K => super::ops::q4_k::Q4_K_BLOCK_BYTES,
        TensorDtype::Q5_0 => 22,
        TensorDtype::Q5K => super::ops::qk_matmul::Q5_K_BLOCK_BYTES,
        TensorDtype::Q6K => 210,
        TensorDtype::Q8_0 => 34,
    }
}

#[cfg(feature = "phase9-real-quant")]
fn dot_quant_block(
    lhs: &[f32],
    dtype: TensorDtype,
    block: &[u8],
) -> Result<f32, ForwardLayerError> {
    match dtype {
        TensorDtype::F32 => Err(ForwardLayerError::Shape),
        TensorDtype::Q4K => dot_q4_k_block(lhs, block),
        TensorDtype::Q5_0 => dot_q5_0_block(lhs, block),
        TensorDtype::Q5K => dot_q5_k_block(lhs, block),
        TensorDtype::Q6K => dot_q6_k_block(lhs, block),
        TensorDtype::Q8_0 => dot_q8_0_block(lhs, block),
    }
}

#[cfg(feature = "phase9-real-quant")]
fn dot_q4_k_block(lhs: &[f32], block: &[u8]) -> Result<f32, ForwardLayerError> {
    if lhs.len() != super::ops::q4_k::Q4_K_VALUES
        || block.len() != super::ops::q4_k::Q4_K_BLOCK_BYTES
    {
        return Err(ForwardLayerError::Shape);
    }
    let values = super::ops::q4_k::dequantize_q4_k(block).map_err(|_| ForwardLayerError::Shape)?;
    let mut acc = 0.0f32;
    let mut idx = 0usize;
    while idx < values.len() {
        acc += lhs[idx] * values[idx];
        idx += 1;
    }
    Ok(acc)
}

#[cfg(feature = "phase9-real-quant")]
fn dot_q5_0_block(lhs: &[f32], block: &[u8]) -> Result<f32, ForwardLayerError> {
    if lhs.len() != 32 || block.len() != 22 {
        return Err(ForwardLayerError::Shape);
    }
    let d = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
    let qh = u32::from_le_bytes([block[2], block[3], block[4], block[5]]);
    let qs = &block[6..22];
    let mut acc = 0.0f32;
    let mut idx = 0usize;
    while idx < 16 {
        let high = ((qh >> idx) & 1) << 4;
        let quant = ((qs[idx] & 0x0f) as u32 | high) as i32 - 16;
        acc += lhs[idx] * (quant as f32 * d);
        idx += 1;
    }
    idx = 0;
    while idx < 16 {
        let high = ((qh >> (idx + 16)) & 1) << 4;
        let quant = ((qs[idx] >> 4) as u32 | high) as i32 - 16;
        acc += lhs[idx + 16] * (quant as f32 * d);
        idx += 1;
    }
    Ok(acc)
}

#[cfg(feature = "phase9-real-quant")]
fn dot_q5_k_block(lhs: &[f32], block: &[u8]) -> Result<f32, ForwardLayerError> {
    if lhs.len() != 256 || block.len() != super::ops::qk_matmul::Q5_K_BLOCK_BYTES {
        return Err(ForwardLayerError::Shape);
    }
    let d = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
    let dmin = f16_bits_to_f32(u16::from_le_bytes([block[2], block[3]]));
    let scales = &block[4..16];
    let qh = &block[16..48];
    let qs = &block[48..176];
    let mut acc = 0.0f32;
    let mut sub = 0usize;
    while sub < 8 {
        let (scale, min) = super::ops::q4_k::get_scale_min_k4(sub, scales)
            .map_err(|_| ForwardLayerError::Shape)?;
        let quant_base = (sub / 2) * 32;
        let value_base = sub * 32;
        let high_mask = 1u8 << sub;
        let mut lane = 0usize;
        while lane < 32 {
            let packed = qs[quant_base + lane];
            let low = if sub.is_multiple_of(2) {
                packed & 0x0f
            } else {
                packed >> 4
            };
            let quant = f32::from(low) + if qh[lane] & high_mask != 0 { 16.0 } else { 0.0 };
            let value = d * f32::from(scale) * quant - dmin * f32::from(min);
            acc += lhs[value_base + lane] * value;
            lane += 1;
        }
        sub += 1;
    }
    Ok(acc)
}

#[cfg(feature = "phase9-real-quant")]
fn dot_q6_k_block(lhs: &[f32], block: &[u8]) -> Result<f32, ForwardLayerError> {
    if lhs.len() != 256 || block.len() != 210 {
        return Err(ForwardLayerError::Shape);
    }
    let ql = &block[0..128];
    let qh = &block[128..192];
    let scales = &block[192..208];
    let d = f16_bits_to_f32(u16::from_le_bytes([block[208], block[209]]));

    let mut acc = 0.0f32;
    let mut ql_base = 0usize;
    let mut qh_base = 0usize;
    let mut scale_base = 0usize;
    let mut out_base = 0usize;
    while ql_base < 128 {
        let mut lane = 0usize;
        while lane < 32 {
            let scale_pair = lane / 16;
            let high = qh[qh_base + lane];
            let low_a = ql[ql_base + lane];
            let low_b = ql[ql_base + 32 + lane];
            let q1 = (((low_a & 0x0f) | ((high & 0x03) << 4)) as i8) - 32;
            let q2 = (((low_b & 0x0f) | (((high >> 2) & 0x03) << 4)) as i8) - 32;
            let q3 = (((low_a >> 4) | (((high >> 4) & 0x03) << 4)) as i8) - 32;
            let q4 = (((low_b >> 4) | (((high >> 6) & 0x03) << 4)) as i8) - 32;
            acc += lhs[out_base + lane]
                * (d * scales[scale_base + scale_pair] as i8 as f32 * q1 as f32);
            acc += lhs[out_base + lane + 32]
                * (d * scales[scale_base + scale_pair + 2] as i8 as f32 * q2 as f32);
            acc += lhs[out_base + lane + 64]
                * (d * scales[scale_base + scale_pair + 4] as i8 as f32 * q3 as f32);
            acc += lhs[out_base + lane + 96]
                * (d * scales[scale_base + scale_pair + 6] as i8 as f32 * q4 as f32);
            lane += 1;
        }
        ql_base += 64;
        qh_base += 32;
        scale_base += 8;
        out_base += 128;
    }
    Ok(acc)
}

#[cfg(feature = "phase9-real-quant")]
fn dot_q8_0_block(lhs: &[f32], block: &[u8]) -> Result<f32, ForwardLayerError> {
    if lhs.len() != 32 || block.len() != 34 {
        return Err(ForwardLayerError::Shape);
    }
    let d = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
    let mut acc = 0.0f32;
    let mut idx = 0usize;
    while idx < 32 {
        acc += lhs[idx] * (block[2 + idx] as i8 as f32 * d);
        idx += 1;
    }
    Ok(acc)
}

#[cfg(feature = "phase9-real-quant")]
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

#[cfg(feature = "phase9-real-quant")]
fn gguf_tensor_for_weight(
    rhs_dtype: TensorDtype,
    rows: usize,
    cols: usize,
    byte_len: usize,
) -> Result<GgufTensorInfo, ForwardLayerError> {
    let mut dims = [0u64; GGML_MAX_DIMS];
    dims[0] = rows as u64;
    dims[1] = cols as u64;
    let ty = match rhs_dtype {
        TensorDtype::F32 => return Err(ForwardLayerError::Shape),
        TensorDtype::Q4K => GgmlType::Q4K,
        TensorDtype::Q5_0 => GgmlType::Q5_0,
        TensorDtype::Q5K => GgmlType::Q5K,
        TensorDtype::Q6K => GgmlType::Q6K,
        TensorDtype::Q8_0 => GgmlType::Q8_0,
    };
    Ok(GgufTensorInfo {
        name: String::new(),
        n_dims: 2,
        dims,
        ty,
        offset: 0,
        byte_len: byte_len as u64,
        data_start: 0,
    })
}

#[cfg(feature = "phase9-real-quant")]
fn gguf_col_major_to_ops_rhs(
    values: &[f32],
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if values.len() != checked_mul(rows, cols)? {
        return Err(ForwardLayerError::Shape);
    }
    let mut out = vec![0.0f32; values.len()];
    let mut col = 0usize;
    while col < cols {
        let mut row = 0usize;
        while row < rows {
            out[row * cols + col] = values[col * rows + row];
            row += 1;
        }
        col += 1;
    }
    Ok(out)
}

#[cfg(any(test, feature = "phase9-real-quant"))]
pub fn matmul_vec_mixed_quant_for_test(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    matmul_vec_weight(
        lhs,
        lhs_rows,
        lhs_cols,
        &[],
        rhs_dtype,
        Some(rhs_quantized),
        rhs_cols,
    )
}

#[cfg(feature = "phase9-real-quant")]
pub fn matmul_vec_mixed_quant_dequant_reference_for_test(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_dtype: TensorDtype,
    rhs_quantized: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if lhs_rows == 0
        || lhs_cols == 0
        || rhs_cols == 0
        || lhs.len() != checked_mul(lhs_rows, lhs_cols)?
    {
        return Err(ForwardLayerError::Shape);
    }
    matmul_vec_quantized_dequant_reference(
        lhs,
        lhs_rows,
        lhs_cols,
        rhs_dtype,
        rhs_quantized,
        rhs_cols,
    )
}

#[cfg(any(test, feature = "phase9-real-quant"))]
pub fn matmul_vec_f32_for_test(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs: &[f32],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    matmul_vec(lhs, lhs_rows, lhs_cols, rhs, rhs_cols)
}

#[cfg(all(
    feature = "phase9-quantized-matmul",
    not(feature = "phase9-real-quant")
))]
fn matmul_vec_quantized(
    lhs: &[f32],
    lhs_rows: usize,
    lhs_cols: usize,
    rhs_q4_k: &[u8],
    rhs_cols: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if lhs_rows == 0
        || lhs_cols == 0
        || rhs_cols == 0
        || lhs.len() != checked_mul(lhs_rows, lhs_cols)?
        || !lhs_cols.is_multiple_of(qk_matmul::Q8_K_VALUES)
    {
        return Err(ForwardLayerError::Shape);
    }

    let blocks_per_dot = lhs_cols / qk_matmul::Q8_K_VALUES;
    let rhs_rows = checked_mul(rhs_cols, blocks_per_dot)?;
    let expected_rhs_bytes = checked_mul(rhs_rows, ops::q4_k::Q4_K_BLOCK_BYTES)?;
    if rhs_q4_k.len() != expected_rhs_bytes {
        return Err(ForwardLayerError::Shape);
    }

    let q8_blocks = checked_mul(lhs_rows, blocks_per_dot)?;
    let mut lhs_q8 = vec![BlockQ8K::ZERO; q8_blocks];
    qk_matmul::quantize_row_q8_k(lhs, &mut lhs_q8).map_err(|_| ForwardLayerError::Shape)?;

    let mut out = vec![0.0; checked_mul(lhs_rows, rhs_cols)?];
    qk_matmul::matmul_q4_k_q8_k(&lhs_q8, rhs_q4_k, lhs_rows, rhs_cols, lhs_cols, &mut out)
        .map_err(|_| ForwardLayerError::Shape)?;
    Ok(out)
}

pub fn rms_norm_vec(
    input: &[f32],
    gamma: &[f32],
    eps_bits: u32,
) -> Result<Vec<f32>, ForwardLayerError> {
    if input.len() != gamma.len() {
        return Err(ForwardLayerError::Shape);
    }
    let shape = shape1(input.len() as u64)?;
    let input_bytes = bytes_from_f32(input);
    let gamma_bytes = bytes_from_f32(gamma);
    let mut out = vec![0u8; input.len() * 4];
    ops::rms_norm_affine_eps(
        &view(&shape, &input_bytes),
        &view(&shape, &gamma_bytes),
        eps_bits,
        &mut out,
    )?;
    Ok(f32_from_bytes(&out))
}

/// Lever 2 (`par-elementwise`): `ops::swiglu_f32` on the `f32` rows, forked
/// across cores by element range past the parallel threshold. The same per-element
/// arithmetic as the byte path below, so the bytes are the same at any core count.
#[cfg(feature = "par-elementwise")]
fn swiglu_vec(gate: &[f32], up: &[f32]) -> Result<Vec<f32>, ForwardLayerError> {
    if gate.len() != up.len() {
        return Err(ForwardLayerError::Shape);
    }
    let mut out = vec![0.0f32; gate.len()];
    if crate::ai::par_matmul::should_parallelize(gate.len()) {
        crate::ai::par_matmul::run_swiglu(gate, up, &mut out)?;
    } else {
        ops::swiglu_f32(gate, up, &mut out)?;
    }
    Ok(out)
}

#[cfg(not(feature = "par-elementwise"))]
fn swiglu_vec(gate: &[f32], up: &[f32]) -> Result<Vec<f32>, ForwardLayerError> {
    if gate.len() != up.len() {
        return Err(ForwardLayerError::Shape);
    }
    let shape = shape1(gate.len() as u64)?;
    let gate_bytes = bytes_from_f32(gate);
    let up_bytes = bytes_from_f32(up);
    let mut out = vec![0u8; gate.len() * 4];
    ops::swiglu(
        &view(&shape, &gate_bytes),
        &view(&shape, &up_bytes),
        &mut out,
    )?;
    Ok(f32_from_bytes(&out))
}

fn rope_vec(
    input: &[f32],
    heads: usize,
    head_dim: usize,
    config: Qwen2LayerConfig,
    position: u64,
) -> Result<Vec<f32>, ForwardLayerError> {
    if heads == 0 || head_dim == 0 || input.len() != heads * head_dim {
        return Err(ForwardLayerError::Shape);
    }
    let shape = shape2(heads as u64, head_dim as u64)?;
    let input_bytes = bytes_from_f32(input);
    let params = RopeYarnParams {
        position,
        n_dims: config.rope_dimension_count as u64,
        theta_scale: config.rope_theta_scale_bits,
        freq_scale: 1.0f32.to_bits(),
        ext_factor: 0.0f32.to_bits(),
        attn_factor: 1.0f32.to_bits(),
        corr_dim_low: 0.0f32.to_bits(),
        corr_dim_high: (config.rope_dimension_count.saturating_sub(1) as f32).to_bits(),
    };
    let mut out = vec![0u8; input.len() * 4];
    ops::rope_neox_yarn(&view(&shape, &input_bytes), params, &mut out)?;
    Ok(f32_from_bytes(&out))
}

pub fn add_vec(lhs: &[f32], rhs: &[f32]) -> Result<Vec<f32>, ForwardLayerError> {
    if lhs.len() != rhs.len() {
        return Err(ForwardLayerError::Shape);
    }
    let shape = shape1(lhs.len() as u64)?;
    let lhs_bytes = bytes_from_f32(lhs);
    let rhs_bytes = bytes_from_f32(rhs);
    let mut out = vec![0u8; lhs.len() * 4];
    ops::add(
        &view(&shape, &lhs_bytes),
        &view(&shape, &rhs_bytes),
        &mut out,
    )?;
    Ok(f32_from_bytes(&out))
}

fn add_optional_bias(
    values: &mut [f32],
    bias: Option<&[f32]>,
    expected_len: usize,
) -> Result<(), ForwardLayerError> {
    let Some(bias) = bias else {
        return Ok(());
    };
    if values.len() != expected_len || bias.len() != expected_len {
        return Err(ForwardLayerError::Shape);
    }
    let mut idx = 0usize;
    while idx < expected_len {
        values[idx] += bias[idx];
        idx += 1;
    }
    Ok(())
}

fn gqa_attention_last(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    config: Qwen2LayerConfig,
    seq_len: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if seq_len == 0
        || query.len() != checked_mul(config.q_len(), seq_len)?
        || key.len() != checked_mul(config.kv_len(), seq_len)?
        || value.len() != checked_mul(config.kv_len(), seq_len)?
    {
        return Err(ForwardLayerError::Shape);
    }
    let query_shape = shape3(
        config.q_heads as u64,
        seq_len as u64,
        config.head_dim as u64,
    )?;
    let kv_shape = shape3(
        config.kv_heads as u64,
        seq_len as u64,
        config.head_dim as u64,
    )?;
    let query_bytes = bytes_from_f32(query);
    let key_bytes = bytes_from_f32(key);
    let value_bytes = bytes_from_f32(value);
    let mut out = vec![0u8; config.q_len() * 4];
    let params = ops::GqaAttentionParams {
        seq_len: seq_len as u64,
        n_q_heads: config.q_heads as u64,
        n_kv_heads: config.kv_heads as u64,
        head_dim: config.head_dim as u64,
    };
    ops::gqa_causal_attention_last(
        &view(&query_shape, &query_bytes),
        &view(&kv_shape, &key_bytes),
        &view(&kv_shape, &value_bytes),
        params,
        &mut out,
    )?;
    Ok(f32_from_bytes(&out))
}

fn token_major_to_gqa(
    tokens: &[f32],
    seq_len: usize,
    heads: usize,
    head_dim: usize,
) -> Result<Vec<f32>, ForwardLayerError> {
    if seq_len == 0 || heads == 0 || head_dim == 0 {
        return Err(ForwardLayerError::Shape);
    }
    let per_token = checked_mul(heads, head_dim)?;
    if tokens.len() != checked_mul(seq_len, per_token)? {
        return Err(ForwardLayerError::Shape);
    }
    let mut out = vec![0.0; tokens.len()];
    let mut pos = 0usize;
    while pos < seq_len {
        let mut head = 0usize;
        while head < heads {
            let mut dim = 0usize;
            while dim < head_dim {
                let src = checked_add(checked_mul(pos, per_token)?, checked_mul(head, head_dim)?)?;
                let src = checked_add(src, dim)?;
                let dst = gqa_index(head, pos, dim, seq_len, head_dim)?;
                out[dst] = tokens[src];
                dim += 1;
            }
            head += 1;
        }
        pos += 1;
    }
    Ok(out)
}

fn gqa_index(
    head: usize,
    pos: usize,
    dim: usize,
    seq_len: usize,
    head_dim: usize,
) -> Result<usize, ForwardLayerError> {
    checked_add(
        checked_mul(checked_add(checked_mul(head, seq_len)?, pos)?, head_dim)?,
        dim,
    )
}

fn view<'a>(shape: &'a Shape, bytes: &'a [u8]) -> TensorView<'a> {
    TensorView {
        dtype: Dtype::F32,
        shape,
        bytes,
    }
}

fn shape1(cols: u64) -> Result<Shape, ForwardLayerError> {
    let mut dims = [0u64; MAX_RANK];
    dims[0] = cols;
    Ok(Shape::new(1, dims)?)
}

fn shape2(rows: u64, cols: u64) -> Result<Shape, ForwardLayerError> {
    let mut dims = [0u64; MAX_RANK];
    dims[0] = rows;
    dims[1] = cols;
    Ok(Shape::new(2, dims)?)
}

fn shape3(a: u64, b: u64, c: u64) -> Result<Shape, ForwardLayerError> {
    let mut dims = [0u64; MAX_RANK];
    dims[0] = a;
    dims[1] = b;
    dims[2] = c;
    Ok(Shape::new(3, dims)?)
}

fn checked_add(lhs: usize, rhs: usize) -> Result<usize, ForwardLayerError> {
    lhs.checked_add(rhs).ok_or(ForwardLayerError::Shape)
}

fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, ForwardLayerError> {
    lhs.checked_mul(rhs).ok_or(ForwardLayerError::Shape)
}

fn bytes_from_f32(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn f32_from_bytes(bytes: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    out
}
