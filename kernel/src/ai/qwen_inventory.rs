// SPDX-License-Identifier: MPL-2.0
//! Stage-B adapter: execute imported Qwen2.5 `.neom` tensor inventories.
//!
//! The L1 importer preserves the real GGUF weights as named `.neom` tensors but
//! does not emit a graph. This module is the thin runtime bridge from those
//! resident named tensors into the already-landed Qwen forward helpers.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::{vec, vec::Vec};

use super::decode_logic::{DecodeCursor, TokenForward};
use super::exec::InferenceError;
use super::forward_logic::{
    self, ForwardLayerError, Qwen2KvCacheSet, Qwen2LayerConfig, Qwen2LayerDtypes,
    Qwen2LayerWeights, Qwen2OutputWeights, Qwen2QuantizedLayerWeights, QwenAttentionNormWeights,
    TensorDtype,
};
use super::gguf_dequant_logic;
use super::gguf_logic::{GGML_MAX_DIMS, GgmlType, GgufTensorInfo};
use super::model::{Model, NeomDtype};
use super::model_logic::TensorMeta;
use super::ops::gdn::{self, GdnLayerConfig, GdnLayerWeights};
use super::tensor::{Dtype, MAX_RANK, Tensor, TensorCreateDesc};

const REAL_QWEN_VOCAB_SIZE: usize = 151_936;

pub fn run(model: &Model, inputs: &[Arc<Tensor>]) -> Result<Vec<Arc<Tensor>>, InferenceError> {
    if inputs.len() != 1 {
        return Err(InferenceError::InvalidInput);
    }
    // B1c (AAG F-A): the generate ABIs' prompt cap, before the token copy and
    // the KV cache. `SYS_INFER_RUN` already applied it at admission (J2); this
    // keeps every other caller bounded too.
    admit_input(&inputs[0])?;
    let prompt = token_input(&inputs[0])?;
    if prompt.is_empty() {
        return Err(InferenceError::InvalidInput);
    }

    let logits = next_logits(model, &prompt)?;
    let output = logits_tensor(&logits)?;
    Ok(vec![Arc::new(output)])
}

/// B1c (AAG J2): the inventory route's admission of its one input, by its
/// metadata alone (no copy, no allocation): an I32 vector of 1..=2,048 tokens,
/// the generate ABIs' prompt cap.
pub fn admit_input(input: &Tensor) -> Result<(), InferenceError> {
    if input.dtype() != Dtype::I32 || input.shape().rank() != 1 {
        return Err(InferenceError::InvalidInput);
    }
    crate::ai::generate_logic::check_prompt_len(input.bytes().len() / 4)
        .map_err(|_| InferenceError::InvalidInput)
}

fn token_input(input: &Tensor) -> Result<Vec<u32>, InferenceError> {
    if input.dtype() != Dtype::I32 || input.shape().rank() != 1 {
        return Err(InferenceError::InvalidInput);
    }
    let mut out = Vec::new();
    for chunk in input.bytes().chunks_exact(4) {
        let token = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if token < 0 {
            return Err(InferenceError::InvalidInput);
        }
        out.push(token as u32);
    }
    Ok(out)
}

pub fn next_logits(model: &Model, prompt: &[u32]) -> Result<Vec<f32>, InferenceError> {
    if prompt.is_empty() {
        return Err(InferenceError::InvalidInput);
    }
    let mut state = InventoryDecodeState::new(model, prompt.len())?;
    let mut logits = Vec::new();
    for &token in prompt {
        logits = state.advance(model, token)?;
    }
    Ok(logits)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct InventoryExecutionStats {
    pub attention_layers: usize,
    pub gdn_layers: usize,
    pub tokens_processed: usize,
    pub gdn_min_steps: usize,
    pub cross_token_state_carried: bool,
}

/// Persistent autoregressive state for an inventory-backed Qwen model.
///
/// `next_logits` remains the compatibility wrapper for callers that need one
/// complete prompt evaluation. Generation callers should retain this object so
/// attention KV and GDN recurrent state advance once per token instead of being
/// reconstructed from the growing context on every decode step.
pub struct InventoryDecodeState {
    config: Qwen2LayerConfig,
    layer_count: usize,
    vocab_size: usize,
    cursor: DecodeCursor,
    attention_layers: usize,
    gdn_layers: usize,
    caches: Qwen2KvCacheSet,
    #[cfg(feature = "ot16-graph-walker")]
    gdn_states: GdnStateSet,
}

impl InventoryDecodeState {
    pub fn new(model: &Model, max_context: usize) -> Result<Self, InferenceError> {
        if max_context == 0
            || model
                .model_config()
                .is_some_and(|model_config| max_context > model_config.context)
        {
            return Err(InferenceError::InvalidInput);
        }
        let config =
            Qwen2LayerConfig::for_model_config(model.model_config()).map_err(map_forward)?;
        let layer_count = model
            .model_config()
            .map(|model_config| model_config.layer_count)
            .unwrap_or(forward_logic::QWEN25_05B_LAYER_COUNT);
        let vocab_size = model
            .model_config()
            .map(|model_config| model_config.vocab)
            .unwrap_or(REAL_QWEN_VOCAB_SIZE);
        let caches = Qwen2KvCacheSet::new(layer_count, config, max_context).map_err(map_forward)?;
        #[cfg(feature = "ot16-graph-walker")]
        let (attention_layers, gdn_layers, gdn_states) = {
            let schedule = model.schedule().ok_or(InferenceError::InvalidGraph)?;
            if schedule.layers.len() != layer_count {
                return Err(InferenceError::InvalidGraph);
            }
            let attention_layers = schedule
                .layers
                .iter()
                .filter(|entry| entry.kind == super::model_logic::LayerKind::Qwen2AttentionFFN)
                .count();
            let gdn_layers = schedule
                .layers
                .iter()
                .filter(|entry| entry.kind == super::model_logic::LayerKind::GatedDeltaNetLinear)
                .count();
            (
                attention_layers,
                gdn_layers,
                GdnStateSet::new(model, schedule, config)?,
            )
        };
        #[cfg(not(feature = "ot16-graph-walker"))]
        let (attention_layers, gdn_layers) = (layer_count, 0);
        Ok(Self {
            config,
            layer_count,
            vocab_size,
            cursor: DecodeCursor::new(max_context),
            attention_layers,
            gdn_layers,
            caches,
            #[cfg(feature = "ot16-graph-walker")]
            gdn_states,
        })
    }

    pub fn advance(&mut self, model: &Model, token: u32) -> Result<Vec<f32>, InferenceError> {
        self.step(model, token, true)
            .map(|logits| logits.unwrap_or_default())
    }

    /// One token through [`DecodeCursor::step`]: the layer stack always, the
    /// output projection only when `want_logits` (Arthur latency NOTE, fix 3:
    /// prefill needs logits only for its last token). The skip lives in
    /// `decode_logic`, which the host prefill KAT mounts.
    pub fn step(
        &mut self,
        model: &Model,
        token: u32,
        want_logits: bool,
    ) -> Result<Option<Vec<f32>>, InferenceError> {
        // Arthur perf lane (OT-7): inventory-backed generation never enters
        // `exec::run`, so this call IS the real per-token forward behind
        // `sys_generate` — both persistent-KV and reprefill modes, and both
        // prefill and decode, funnel through here. Bracketing it is what makes
        // the `sys_generate` path measured at all. `read_tsc` is a pure register
        // read with no memory effect, so the logits are bit-identical with the
        // feature on or off (kill-switch clean).
        #[cfg(feature = "forward-perf")]
        let perf_t0 = crate::ai::forward_perf::read_tsc();
        let mut forward = InventoryForward {
            model,
            caches: &mut self.caches,
            #[cfg(feature = "ot16-graph-walker")]
            gdn_states: &mut self.gdn_states,
            config: self.config,
            layer_count: self.layer_count,
            vocab_size: self.vocab_size,
        };
        let logits = self.cursor.step(
            &mut forward,
            token,
            want_logits,
            InferenceError::InvalidInput,
        );
        // Record only a forward that succeeded. A failed forward returns early,
        // so timing it would both deflate the per-pass average and count a
        // "pass" that yielded no token. The error value and its propagation are
        // unchanged.
        #[cfg(feature = "forward-perf")]
        if logits.is_ok() {
            crate::ai::forward_perf::record_forward(
                crate::ai::forward_perf::read_tsc().wrapping_sub(perf_t0),
            );
        }
        logits
    }

    pub fn execution_stats(&self) -> InventoryExecutionStats {
        #[cfg(feature = "ot16-graph-walker")]
        let (gdn_min_steps, cross_token_state_carried) = (
            self.gdn_states.minimum_steps(),
            self.gdn_states.cross_token_state_carried(),
        );
        #[cfg(not(feature = "ot16-graph-walker"))]
        let (gdn_min_steps, cross_token_state_carried) = (0, false);
        InventoryExecutionStats {
            attention_layers: self.attention_layers,
            gdn_layers: self.gdn_layers,
            tokens_processed: self.cursor.position(),
            gdn_min_steps,
            cross_token_state_carried,
        }
    }

    /// Corrupt the first layer's retained K at prompt position 1 so the
    /// fast-generation equivalence KAT demonstrates a real red path.
    #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
    pub fn corrupt_retained_key_for_kat(&mut self) -> Result<bool, InferenceError> {
        self.caches
            .layer_mut(0)
            .map_err(map_forward)?
            .zero_key_position_for_kat(1)
            .map_err(map_forward)
    }
}

/// The kernel's [`TokenForward`]: the embedding + layer loop and the norm +
/// lm_head over the model's tensors. Neither half knows whether the caller
/// wants logits; that branch is [`DecodeCursor::step`]'s alone.
struct InventoryForward<'a> {
    model: &'a Model,
    caches: &'a mut Qwen2KvCacheSet,
    #[cfg(feature = "ot16-graph-walker")]
    gdn_states: &'a mut GdnStateSet,
    config: Qwen2LayerConfig,
    layer_count: usize,
    vocab_size: usize,
}

impl TokenForward for InventoryForward<'_> {
    type Error = InferenceError;

    fn layers(&mut self, token: u32, position: usize) -> Result<Vec<f32>, InferenceError> {
        #[cfg(feature = "ot16-graph-walker")]
        let gdn_states = Some(&mut *self.gdn_states);
        #[cfg(not(feature = "ot16-graph-walker"))]
        let gdn_states = None;
        token_layers(
            self.model,
            token,
            position,
            self.caches,
            gdn_states,
            self.config,
            self.layer_count,
            self.vocab_size,
        )
    }

    fn output(&mut self, hidden: &[f32]) -> Result<Vec<f32>, InferenceError> {
        token_output(self.model, hidden, self.config, self.vocab_size)
    }
}

// Eight parameters is one over clippy's default threshold. Grouping them into a
// struct would change the call ABI of a forward-path helper, which is a code
// change rather than a lint fix — out of scope for this hygiene lane.
#[allow(clippy::too_many_arguments)]
fn token_layers(
    model: &Model,
    token: u32,
    position: usize,
    caches: &mut Qwen2KvCacheSet,
    gdn_states: Option<&mut GdnStateSet>,
    config: Qwen2LayerConfig,
    layer_count: usize,
    vocab_size: usize,
) -> Result<Vec<f32>, InferenceError> {
    #[cfg(not(feature = "ot16-graph-walker"))]
    let _ = gdn_states;
    let mut hidden = token_embedding(model, token, config.hidden_size, vocab_size)?;
    #[cfg(feature = "prefill-perf")]
    let perf_t0 = crate::ai::prefill_perf::tsc();
    // Kill-switch 1: with the walker OFF this is the exact hardcoded per-layer
    // loop as before — bit-for-bit unchanged (it just calls the extracted body).
    #[cfg(not(feature = "ot16-graph-walker"))]
    {
        let mut layer_idx = 0usize;
        while layer_idx < layer_count {
            let cache = caches.layer_mut(layer_idx).map_err(map_forward)?;
            hidden =
                run_qwen2_attention_ffn_layer(model, &hidden, config, position, layer_idx, cache)?;
            layer_idx += 1;
        }
    }
    // Kill-switch 2: with the walker ON, drive the per-layer loop from the
    // data-driven schedule instead of `0..layer_count`. For a Qwen2 model the
    // schedule is n x Qwen2AttentionFFN (explicit v3 or synthesized v1/v2), so
    // the same layers run in the same order through the same op helper —
    // byte-identical to the hardcoded path.
    #[cfg(feature = "ot16-graph-walker")]
    {
        let schedule = model.schedule().ok_or(InferenceError::InvalidGraph)?;
        if schedule.layers.len() != layer_count {
            return Err(InferenceError::InvalidGraph);
        }
        let gdn_states = gdn_states.ok_or(InferenceError::InvalidGraph)?;
        // Drive the per-layer loop from the data-driven schedule via the shared
        // walk dispatch — the SAME function the host KS2 KAT executes (so the KAT
        // proves this exact index+kind-match+order logic, not a reimplementation).
        // For a Qwen2 model the schedule is n x Qwen2AttentionFFN (explicit v3 or
        // synthesized v1/v2), so the same layers run in the same order through the
        // same op helper — byte-identical to the hardcoded path. GatedDeltaNetLinear
        // is accepted at load in B2; walk_qwen2_schedule dispatches GDN layers to the
        // GDN forward. The GDN runner closure captures the model state for recurrent caching.
        hidden = super::model_logic::walk_qwen2_schedule(
            schedule,
            hidden,
            |layer_idx, h| {
                let cache = caches.layer_mut(layer_idx).map_err(map_forward)?;
                run_qwen2_attention_ffn_layer(model, h, config, position, layer_idx, cache)
            },
            |layer_idx, config_ref, h| {
                let cache = gdn_states.layer_mut(layer_idx, config_ref)?;
                run_gdn_linear_layer(model, h, config, layer_idx, cache)
            },
        )?;
    }

    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::layers(crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0));
    Ok(hidden)
}

/// The output projection of a final hidden state, with the finiteness check.
fn token_output(
    model: &Model,
    hidden: &[f32],
    config: Qwen2LayerConfig,
    vocab_size: usize,
) -> Result<Vec<f32>, InferenceError> {
    #[cfg(feature = "prefill-perf")]
    let perf_t1 = crate::ai::prefill_perf::tsc();
    let output_norm = tensor_to_f32(model, "output_norm.weight", 1, config.hidden_size)?;
    let output_dtype = tensor_dtype(model, "output.weight", config.hidden_size, vocab_size)?;
    let output = tensor_to_ops_rhs_placeholder_if_quantized(
        model,
        "output.weight",
        config.hidden_size,
        vocab_size,
    )?;
    let output_quantized =
        tensor_to_quantized_arc(model, "output.weight", config.hidden_size, vocab_size)?;
    let logits = forward_logic::qwen2_output_logits(
        hidden,
        Qwen2OutputWeights {
            output_norm: &output_norm,
            output: &output,
            output_dtype,
            output_quantized: output_quantized.as_ref().map(|tensor| tensor.bytes()),
            output_bias: None,
        },
        config,
        vocab_size,
    )
    .map_err(map_forward)?;
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::lm_head(crate::ai::prefill_perf::tsc().wrapping_sub(perf_t1));
    if logits.iter().any(|value| !value.is_finite()) {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(logits)
}

// B1c (F3): the GDN (Qwen3.5) recurrent state is constructed only by the OT-16
// graph walker; real-LM builds without the walker (the product among them)
// compile these items unused, as the real-brain test builds on `main` already did.
#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
struct GdnLayerCache {
    layer_idx: usize,
    config_ref: u32,
    config: GdnLayerConfig,
    recurrent_state: Vec<f32>,
    conv_state: Vec<f32>,
    steps: usize,
    cross_token_state_carried: bool,
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
impl GdnLayerCache {
    fn new(
        layer_idx: usize,
        config_ref: u32,
        config: GdnLayerConfig,
    ) -> Result<Self, gdn::GdnError> {
        Ok(Self {
            layer_idx,
            config_ref,
            config,
            recurrent_state: vec![0.0f32; config.state_size()],
            conv_state: vec![0.0f32; config.conv_state_size()?],
            steps: 0,
            cross_token_state_carried: false,
        })
    }
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
struct GdnStateSet {
    layers: Vec<GdnLayerCache>,
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
impl GdnStateSet {
    fn new(
        model: &Model,
        schedule: &super::model_logic::LayerSchedule,
        config: Qwen2LayerConfig,
    ) -> Result<Self, InferenceError> {
        let mut layers = Vec::new();
        for (layer_idx, entry) in schedule.layers.iter().enumerate() {
            if entry.kind == super::model_logic::LayerKind::GatedDeltaNetLinear {
                let gdn_config = gdn_config_for_entry(model, config, layer_idx, entry.config_ref)?;
                layers.push(
                    GdnLayerCache::new(layer_idx, entry.config_ref, gdn_config).map_err(map_gdn)?,
                );
            }
        }
        Ok(Self { layers })
    }

    fn layer_mut(
        &mut self,
        layer_idx: usize,
        config_ref: u32,
    ) -> Result<&mut GdnLayerCache, InferenceError> {
        self.layers
            .iter_mut()
            .find(|cache| cache.layer_idx == layer_idx && cache.config_ref == config_ref)
            .ok_or(InferenceError::InvalidGraph)
    }

    fn minimum_steps(&self) -> usize {
        self.layers
            .iter()
            .map(|cache| cache.steps)
            .min()
            .unwrap_or(0)
    }

    fn cross_token_state_carried(&self) -> bool {
        !self.layers.is_empty()
            && self
                .layers
                .iter()
                .all(|cache| cache.cross_token_state_carried)
    }
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn gdn_config_for_entry(
    model: &Model,
    config: Qwen2LayerConfig,
    layer: usize,
    config_ref: u32,
) -> Result<GdnLayerConfig, InferenceError> {
    let _ = config_ref;
    let conv_dim = rhs_cols_any(
        model,
        &gdn_layer_tensor_candidates(layer, "in_proj_qkv.weight"),
        config.hidden_size,
    )?;
    let value_dim = rhs_cols_any(
        model,
        &gdn_layer_tensor_candidates(layer, "in_proj_z.weight"),
        config.hidden_size,
    )?;
    let num_v_heads = rhs_cols_any(
        model,
        &gdn_layer_tensor_candidates(layer, "in_proj_b.weight"),
        config.hidden_size,
    )?;
    let alpha_heads = rhs_cols_any(
        model,
        &gdn_layer_tensor_candidates(layer, "in_proj_a.weight"),
        config.hidden_size,
    )?;
    let dt_heads = flat_tensor_len_any(model, &gdn_layer_tensor_candidates(layer, "dt_bias"))?;
    let a_heads = flat_tensor_len_any(model, &gdn_layer_tensor_candidates(layer, "A_log"))?;
    let head_v_dim =
        flat_tensor_len_any(model, &gdn_layer_tensor_candidates(layer, "norm.weight"))?;
    if num_v_heads == 0
        || alpha_heads != num_v_heads
        || dt_heads != num_v_heads
        || a_heads != num_v_heads
        || head_v_dim == 0
        || value_dim
            != num_v_heads
                .checked_mul(head_v_dim)
                .ok_or(InferenceError::InvalidGraph)?
        || conv_dim <= value_dim
    {
        return Err(InferenceError::InvalidGraph);
    }
    let key_twice = conv_dim - value_dim;
    if !key_twice.is_multiple_of(2) {
        return Err(InferenceError::InvalidGraph);
    }
    let key_dim = key_twice / 2;
    if !key_dim.is_multiple_of(head_v_dim) {
        return Err(InferenceError::InvalidGraph);
    }
    let num_k_heads = key_dim / head_v_dim;
    let conv_elems =
        flat_tensor_len_any(model, &gdn_layer_tensor_candidates(layer, "conv1d.weight"))?;
    if conv_elems == 0 || !conv_elems.is_multiple_of(conv_dim) {
        return Err(InferenceError::InvalidGraph);
    }
    let conv_kernel_size = conv_elems / conv_dim;
    let out_cols = rhs_cols_any(
        model,
        &gdn_layer_tensor_candidates(layer, "out_proj.weight"),
        value_dim,
    )?;
    if num_k_heads == 0 || out_cols != config.hidden_size {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(GdnLayerConfig {
        hidden_size: config.hidden_size,
        num_k_heads,
        num_v_heads,
        head_k_dim: head_v_dim,
        head_v_dim,
        conv_kernel_size,
        rms_norm_eps: f32::from_bits(config.rms_norm_eps_bits),
    })
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn run_gdn_linear_layer(
    model: &Model,
    hidden: &[f32],
    qwen_config: Qwen2LayerConfig,
    layer_idx: usize,
    cache: &mut GdnLayerCache,
) -> Result<Vec<f32>, InferenceError> {
    let carried_nonzero_state = cache.steps != 0
        && (cache.recurrent_state.iter().any(|&value| value != 0.0)
            || cache.conv_state.iter().any(|&value| value != 0.0));
    let attn_norm = tensor_to_f32(
        model,
        &qwen_layer_tensor_name(layer_idx, "attn_norm.weight"),
        1,
        qwen_config.hidden_size,
    )?;
    let attn_norm = forward_logic::rms_norm_vec(hidden, &attn_norm, qwen_config.rms_norm_eps_bits)
        .map_err(map_forward)?;
    let buffers = GdnLayerBuffers::new(model, layer_idx, cache.config)?;
    let qkv = buffers.in_proj_qkv.project(
        &attn_norm,
        qwen_config.hidden_size,
        cache.config.conv_dim(),
    )?;
    let z = buffers.in_proj_z.project(
        &attn_norm,
        qwen_config.hidden_size,
        cache.config.value_dim(),
    )?;
    let beta_raw = buffers.in_proj_b.project(
        &attn_norm,
        qwen_config.hidden_size,
        cache.config.num_v_heads,
    )?;
    let a_raw = buffers.in_proj_a.project(
        &attn_norm,
        qwen_config.hidden_size,
        cache.config.num_v_heads,
    )?;
    let normed = gdn::gdn_layer_core_projected(
        gdn::GdnProjectedInputs {
            qkv: &qkv,
            z: &z,
            beta_raw: &beta_raw,
            a_raw: &a_raw,
        },
        buffers.weights(),
        cache.config,
        &mut cache.recurrent_state,
        &mut cache.conv_state,
    )
    .map_err(map_gdn)?;
    cache.steps += 1;
    cache.cross_token_state_carried |= carried_nonzero_state;
    let attention_output =
        buffers
            .out_proj
            .project(&normed, cache.config.value_dim(), qwen_config.hidden_size)?;
    let ffn_input = forward_logic::add_vec(hidden, &attention_output).map_err(map_forward)?;
    let ffn = GdnFfnBuffers::new(model, layer_idx, qwen_config)?;
    Ok(
        forward_logic::swiglu_ffn(&ffn_input, ffn.weights(), qwen_config)
            .map_err(map_forward)?
            .residual,
    )
}

/// Run one `Qwen2AttentionFFN` layer — the exact per-layer body of the forward.
/// Shared by the hardcoded loop (kill-switch 1) and the OT-16 schedule walker
/// (kill-switch 2) so the two paths are byte-identical by construction: same op
/// helper, same arguments, same order. Returns the layer's residual output.
fn run_qwen2_attention_ffn_layer(
    model: &Model,
    hidden: &[f32],
    config: Qwen2LayerConfig,
    position: usize,
    layer_idx: usize,
    cache: &mut forward_logic::Qwen2KvCacheLayer,
) -> Result<Vec<f32>, InferenceError> {
    let layer = LayerBuffers::new(model, layer_idx, config)?;
    let weights = layer.weights();
    let norms = layer.attention_norms();
    let out = forward_logic::forward_layer_with_gqa_attention_with_norms(
        hidden,
        weights,
        config,
        position as u64,
        true,
        Some(cache),
        None,
        norms,
    )
    .map_err(map_forward)?;
    Ok(out.ffn.residual)
}

struct LayerBuffers {
    attn_norm: Vec<f32>,
    attn_q: Vec<f32>,
    attn_k: Vec<f32>,
    attn_v: Vec<f32>,
    attn_q_bias: Option<Vec<f32>>,
    attn_k_bias: Option<Vec<f32>>,
    attn_v_bias: Option<Vec<f32>>,
    attn_q_norm: Option<Vec<f32>>,
    attn_k_norm: Option<Vec<f32>>,
    attn_o: Vec<f32>,
    ffn_norm: Vec<f32>,
    ffn_gate: Vec<f32>,
    ffn_up: Vec<f32>,
    ffn_down: Vec<f32>,
    dtypes: Qwen2LayerDtypes,
    attn_q_quantized: Option<Arc<Tensor>>,
    attn_k_quantized: Option<Arc<Tensor>>,
    attn_v_quantized: Option<Arc<Tensor>>,
    attn_o_quantized: Option<Arc<Tensor>>,
    ffn_gate_quantized: Option<Arc<Tensor>>,
    ffn_up_quantized: Option<Arc<Tensor>>,
    ffn_down_quantized: Option<Arc<Tensor>>,
}

impl LayerBuffers {
    fn new(model: &Model, layer: usize, config: Qwen2LayerConfig) -> Result<Self, InferenceError> {
        let attn_q_name = qwen_layer_tensor_name(layer, "attn_q.weight");
        let attn_k_name = qwen_layer_tensor_name(layer, "attn_k.weight");
        let attn_v_name = qwen_layer_tensor_name(layer, "attn_v.weight");
        let attn_o_name = qwen_layer_tensor_name(layer, "attn_output.weight");
        let ffn_gate_name = qwen_layer_tensor_name(layer, "ffn_gate.weight");
        let ffn_up_name = qwen_layer_tensor_name(layer, "ffn_up.weight");
        let ffn_down_name = qwen_layer_tensor_name(layer, "ffn_down.weight");
        Ok(Self {
            attn_norm: tensor_to_f32(
                model,
                &qwen_layer_tensor_name(layer, "attn_norm.weight"),
                1,
                config.hidden_size,
            )?,
            attn_q: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_q_name,
                config.hidden_size,
                config.attn_q_projection_size,
            )?,
            attn_k: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_k_name,
                config.hidden_size,
                config.kv_len(),
            )?,
            attn_v: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_v_name,
                config.hidden_size,
                config.kv_len(),
            )?,
            attn_q_bias: tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_q.bias"),
                1,
                config.attn_q_projection_size,
            )?,
            attn_k_bias: tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_k.bias"),
                1,
                config.kv_len(),
            )?,
            attn_v_bias: tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_v.bias"),
                1,
                config.kv_len(),
            )?,
            attn_q_norm: tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_q_norm.weight"),
                1,
                config.head_dim,
            )?,
            attn_k_norm: tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_k_norm.weight"),
                1,
                config.head_dim,
            )?,
            attn_o: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_o_name,
                config.q_len(),
                config.hidden_size,
            )?,
            ffn_norm: tensor_to_flat_f32_any(
                model,
                &ffn_norm_tensor_candidates(layer),
                config.hidden_size,
            )?,
            ffn_gate: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_up: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_down: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            )?,
            dtypes: Qwen2LayerDtypes {
                attn_q: tensor_dtype(
                    model,
                    &attn_q_name,
                    config.hidden_size,
                    config.attn_q_projection_size,
                )?,
                attn_k: tensor_dtype(model, &attn_k_name, config.hidden_size, config.kv_len())?,
                attn_v: tensor_dtype(model, &attn_v_name, config.hidden_size, config.kv_len())?,
                attn_o: tensor_dtype(model, &attn_o_name, config.q_len(), config.hidden_size)?,
                ffn_gate: tensor_dtype(
                    model,
                    &ffn_gate_name,
                    config.hidden_size,
                    config.intermediate_size,
                )?,
                ffn_up: tensor_dtype(
                    model,
                    &ffn_up_name,
                    config.hidden_size,
                    config.intermediate_size,
                )?,
                ffn_down: tensor_dtype(
                    model,
                    &ffn_down_name,
                    config.intermediate_size,
                    config.hidden_size,
                )?,
            },
            attn_q_quantized: tensor_to_quantized_arc(
                model,
                &attn_q_name,
                config.hidden_size,
                config.attn_q_projection_size,
            )?,
            attn_k_quantized: tensor_to_quantized_arc(
                model,
                &attn_k_name,
                config.hidden_size,
                config.kv_len(),
            )?,
            attn_v_quantized: tensor_to_quantized_arc(
                model,
                &attn_v_name,
                config.hidden_size,
                config.kv_len(),
            )?,
            attn_o_quantized: tensor_to_quantized_arc(
                model,
                &attn_o_name,
                config.q_len(),
                config.hidden_size,
            )?,
            ffn_gate_quantized: tensor_to_quantized_arc(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_up_quantized: tensor_to_quantized_arc(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_down_quantized: tensor_to_quantized_arc(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            )?,
        })
    }

    fn weights(&self) -> Qwen2LayerWeights<'_> {
        Qwen2LayerWeights {
            attn_norm: &self.attn_norm,
            attn_q: &self.attn_q,
            attn_k: &self.attn_k,
            attn_v: &self.attn_v,
            attn_q_bias: self.attn_q_bias.as_deref(),
            attn_k_bias: self.attn_k_bias.as_deref(),
            attn_v_bias: self.attn_v_bias.as_deref(),
            attn_o: &self.attn_o,
            ffn_norm: &self.ffn_norm,
            ffn_gate: &self.ffn_gate,
            ffn_up: &self.ffn_up,
            ffn_down: &self.ffn_down,
            dtypes: self.dtypes,
            quantized: Qwen2QuantizedLayerWeights {
                attn_q: self.attn_q_quantized.as_ref().map(|tensor| tensor.bytes()),
                attn_k: self.attn_k_quantized.as_ref().map(|tensor| tensor.bytes()),
                attn_v: self.attn_v_quantized.as_ref().map(|tensor| tensor.bytes()),
                attn_o: self.attn_o_quantized.as_ref().map(|tensor| tensor.bytes()),
                ffn_gate: self
                    .ffn_gate_quantized
                    .as_ref()
                    .map(|tensor| tensor.bytes()),
                ffn_up: self.ffn_up_quantized.as_ref().map(|tensor| tensor.bytes()),
                ffn_down: self
                    .ffn_down_quantized
                    .as_ref()
                    .map(|tensor| tensor.bytes()),
            },
        }
    }

    fn attention_norms(&self) -> QwenAttentionNormWeights<'_> {
        QwenAttentionNormWeights {
            q_norm: self.attn_q_norm.as_deref(),
            k_norm: self.attn_k_norm.as_deref(),
        }
    }
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
struct GdnMatrixBuffer {
    f32: Vec<f32>,
    dtype: TensorDtype,
    quantized: Option<Arc<Tensor>>,
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
impl GdnMatrixBuffer {
    fn new_any(
        model: &Model,
        names: &[String],
        rows: usize,
        cols: usize,
    ) -> Result<Self, InferenceError> {
        let name = names
            .iter()
            .find(|name| model.mint_weight_named(name).is_some())
            .ok_or(InferenceError::InvalidGraph)?;
        Ok(Self {
            f32: tensor_to_ops_rhs_placeholder_if_quantized(model, name, rows, cols)?,
            dtype: tensor_dtype(model, name, rows, cols)?,
            quantized: tensor_to_quantized_arc(model, name, rows, cols)?,
        })
    }

    fn project(&self, input: &[f32], rows: usize, cols: usize) -> Result<Vec<f32>, InferenceError> {
        forward_logic::matmul_vec_mixed_weight(
            input,
            1,
            rows,
            &self.f32,
            self.dtype,
            self.quantized.as_ref().map(|tensor| tensor.bytes()),
            cols,
        )
        .map_err(map_forward)
    }
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
struct GdnLayerBuffers {
    in_proj_qkv: GdnMatrixBuffer,
    in_proj_z: GdnMatrixBuffer,
    in_proj_b: GdnMatrixBuffer,
    in_proj_a: GdnMatrixBuffer,
    conv1d_weight: Vec<f32>,
    a_log: Vec<f32>,
    dt_bias: Vec<f32>,
    norm_weight: Vec<f32>,
    out_proj: GdnMatrixBuffer,
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
impl GdnLayerBuffers {
    fn new(model: &Model, layer: usize, config: GdnLayerConfig) -> Result<Self, InferenceError> {
        let value_dim = config.value_dim();
        let conv_dim = config.conv_dim();
        Ok(Self {
            in_proj_qkv: GdnMatrixBuffer::new_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_qkv.weight"),
                config.hidden_size,
                conv_dim,
            )?,
            in_proj_z: GdnMatrixBuffer::new_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_z.weight"),
                config.hidden_size,
                value_dim,
            )?,
            in_proj_b: GdnMatrixBuffer::new_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_b.weight"),
                config.hidden_size,
                config.num_v_heads,
            )?,
            in_proj_a: GdnMatrixBuffer::new_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_a.weight"),
                config.hidden_size,
                config.num_v_heads,
            )?,
            conv1d_weight: tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "conv1d.weight"),
                conv_dim
                    .checked_mul(config.conv_kernel_size)
                    .ok_or(InferenceError::InvalidGraph)?,
            )?,
            a_log: tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "A_log"),
                config.num_v_heads,
            )?,
            dt_bias: tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "dt_bias"),
                config.num_v_heads,
            )?,
            norm_weight: tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "norm.weight"),
                config.head_v_dim,
            )?,
            out_proj: GdnMatrixBuffer::new_any(
                model,
                &gdn_layer_tensor_candidates(layer, "out_proj.weight"),
                value_dim,
                config.hidden_size,
            )?,
        })
    }

    fn weights(&self) -> GdnLayerWeights<'_> {
        GdnLayerWeights {
            in_proj_qkv: &self.in_proj_qkv.f32,
            in_proj_z: &self.in_proj_z.f32,
            in_proj_b: &self.in_proj_b.f32,
            in_proj_a: &self.in_proj_a.f32,
            conv1d_weight: &self.conv1d_weight,
            a_log: &self.a_log,
            dt_bias: &self.dt_bias,
            norm_weight: &self.norm_weight,
            out_proj: &self.out_proj.f32,
        }
    }
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
struct GdnFfnBuffers {
    ffn_norm: Vec<f32>,
    ffn_gate: Vec<f32>,
    ffn_up: Vec<f32>,
    ffn_down: Vec<f32>,
    ffn_gate_dtype: TensorDtype,
    ffn_up_dtype: TensorDtype,
    ffn_down_dtype: TensorDtype,
    ffn_gate_quantized: Option<Arc<Tensor>>,
    ffn_up_quantized: Option<Arc<Tensor>>,
    ffn_down_quantized: Option<Arc<Tensor>>,
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
impl GdnFfnBuffers {
    fn new(model: &Model, layer: usize, config: Qwen2LayerConfig) -> Result<Self, InferenceError> {
        let ffn_gate_name = qwen_layer_tensor_name(layer, "ffn_gate.weight");
        let ffn_up_name = qwen_layer_tensor_name(layer, "ffn_up.weight");
        let ffn_down_name = qwen_layer_tensor_name(layer, "ffn_down.weight");
        Ok(Self {
            ffn_norm: tensor_to_flat_f32_any(
                model,
                &ffn_norm_tensor_candidates(layer),
                config.hidden_size,
            )?,
            ffn_gate: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_up: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_down: tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            )?,
            ffn_gate_dtype: tensor_dtype(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_up_dtype: tensor_dtype(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_down_dtype: tensor_dtype(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            )?,
            ffn_gate_quantized: tensor_to_quantized_arc(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_up_quantized: tensor_to_quantized_arc(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            )?,
            ffn_down_quantized: tensor_to_quantized_arc(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            )?,
        })
    }

    fn weights(&self) -> Qwen2LayerWeights<'_> {
        Qwen2LayerWeights {
            attn_norm: &[],
            attn_q: &[],
            attn_k: &[],
            attn_v: &[],
            attn_q_bias: None,
            attn_k_bias: None,
            attn_v_bias: None,
            attn_o: &[],
            ffn_norm: &self.ffn_norm,
            ffn_gate: &self.ffn_gate,
            ffn_up: &self.ffn_up,
            ffn_down: &self.ffn_down,
            dtypes: Qwen2LayerDtypes {
                attn_q: TensorDtype::F32,
                attn_k: TensorDtype::F32,
                attn_v: TensorDtype::F32,
                attn_o: TensorDtype::F32,
                ffn_gate: self.ffn_gate_dtype,
                ffn_up: self.ffn_up_dtype,
                ffn_down: self.ffn_down_dtype,
            },
            quantized: Qwen2QuantizedLayerWeights {
                attn_q: None,
                attn_k: None,
                attn_v: None,
                attn_o: None,
                ffn_gate: self
                    .ffn_gate_quantized
                    .as_ref()
                    .map(|tensor| tensor.bytes()),
                ffn_up: self.ffn_up_quantized.as_ref().map(|tensor| tensor.bytes()),
                ffn_down: self
                    .ffn_down_quantized
                    .as_ref()
                    .map(|tensor| tensor.bytes()),
            },
        }
    }
}

fn tensor_to_ops_rhs_placeholder_if_quantized(
    model: &Model,
    name: &str,
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, InferenceError> {
    if tensor_dtype(model, name, rows, cols)?.is_quantized() {
        Ok(Vec::new())
    } else {
        tensor_to_ops_rhs(model, name, rows, cols)
    }
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn rhs_cols_any(model: &Model, names: &[String], rows: usize) -> Result<usize, InferenceError> {
    for name in names {
        if let Some((meta, tensor)) = model.mint_weight_named(name) {
            return rhs_cols_for_tensor(name, meta, tensor.bytes().len(), rows);
        }
    }
    Err(InferenceError::InvalidGraph)
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn rhs_cols_for_tensor(
    name: &str,
    meta: TensorMeta,
    byte_len: usize,
    rows: usize,
) -> Result<usize, InferenceError> {
    if rows == 0 {
        return Err(InferenceError::InvalidGraph);
    }
    if meta.dtype == NeomDtype::U8 {
        return quantized_rhs_cols(name, rows, byte_len);
    }
    if meta.dtype == NeomDtype::Q5K {
        return quantized_rhs_cols_for_type(rows, byte_len, GgmlType::Q5K);
    }
    let elements = scalar_tensor_elements(meta, byte_len)?;
    if meta.shape.rank == 2 {
        let row_dim =
            usize::try_from(meta.shape.dims[0]).map_err(|_| InferenceError::InvalidGraph)?;
        let col_dim =
            usize::try_from(meta.shape.dims[1]).map_err(|_| InferenceError::InvalidGraph)?;
        if row_dim != rows
            || elements
                != rows
                    .checked_mul(col_dim)
                    .ok_or(InferenceError::InvalidGraph)?
        {
            return Err(InferenceError::InvalidGraph);
        }
        return Ok(col_dim);
    }
    if !elements.is_multiple_of(rows) {
        return Err(InferenceError::InvalidGraph);
    }
    let cols = elements / rows;
    if cols == 0 {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(cols)
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn quantized_rhs_cols(name: &str, rows: usize, byte_len: usize) -> Result<usize, InferenceError> {
    let candidates = [
        (TensorDtype::Q4K, GgmlType::Q4K),
        (TensorDtype::Q5_0, GgmlType::Q5_0),
        (TensorDtype::Q6K, GgmlType::Q6K),
        (TensorDtype::Q8_0, GgmlType::Q8_0),
    ];
    let mut matched = None;
    for (_dtype, ty) in candidates {
        let block = usize::try_from(ty.block_size()).map_err(|_| InferenceError::InvalidGraph)?;
        let type_size =
            usize::try_from(ty.type_size()).map_err(|_| InferenceError::InvalidGraph)?;
        if !rows.is_multiple_of(block) {
            continue;
        }
        let per_col = (rows / block)
            .checked_mul(type_size)
            .ok_or(InferenceError::InvalidGraph)?;
        if per_col == 0 || !byte_len.is_multiple_of(per_col) {
            continue;
        }
        let cols = byte_len / per_col;
        if cols == 0 || infer_quant_dtype(name, rows, cols, byte_len).is_err() {
            continue;
        }
        if matched.is_some_and(|found| found != cols) {
            return Err(InferenceError::InvalidGraph);
        }
        matched = Some(cols);
    }
    matched.ok_or(InferenceError::InvalidGraph)
}

fn quantized_rhs_cols_for_type(
    rows: usize,
    byte_len: usize,
    ty: GgmlType,
) -> Result<usize, InferenceError> {
    let block = usize::try_from(ty.block_size()).map_err(|_| InferenceError::InvalidGraph)?;
    let type_size = usize::try_from(ty.type_size()).map_err(|_| InferenceError::InvalidGraph)?;
    if rows == 0 || !rows.is_multiple_of(block) {
        return Err(InferenceError::InvalidGraph);
    }
    let per_col = (rows / block)
        .checked_mul(type_size)
        .ok_or(InferenceError::InvalidGraph)?;
    if per_col == 0 || !byte_len.is_multiple_of(per_col) {
        return Err(InferenceError::InvalidGraph);
    }
    let cols = byte_len / per_col;
    if cols == 0 {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(cols)
}

fn tensor_to_flat_f32_any(
    model: &Model,
    names: &[String],
    expected_len: usize,
) -> Result<Vec<f32>, InferenceError> {
    for name in names {
        if model.mint_weight_named(name).is_some() {
            let values = tensor_to_f32(model, name, 1, expected_len)?;
            if values.len() == expected_len {
                return Ok(values);
            }
            return Err(InferenceError::InvalidGraph);
        }
    }
    Err(InferenceError::InvalidGraph)
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn flat_tensor_len_any(model: &Model, names: &[String]) -> Result<usize, InferenceError> {
    for name in names {
        if let Some((meta, tensor)) = model.mint_weight_named(name) {
            return scalar_tensor_elements(meta, tensor.bytes().len());
        }
    }
    Err(InferenceError::InvalidGraph)
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn scalar_tensor_elements(meta: TensorMeta, byte_len: usize) -> Result<usize, InferenceError> {
    match meta.dtype {
        NeomDtype::F32
        | NeomDtype::F16
        | NeomDtype::BF16
        | NeomDtype::I32
        | NeomDtype::I8
        | NeomDtype::Bool => {}
        NeomDtype::U8 | NeomDtype::Q5K => return Err(InferenceError::InvalidGraph),
    }
    let element_size =
        usize::try_from(meta.dtype.size_bytes()).map_err(|_| InferenceError::InvalidGraph)?;
    if element_size == 0 || !byte_len.is_multiple_of(element_size) {
        return Err(InferenceError::InvalidGraph);
    }
    let elements = byte_len / element_size;
    let mut shape_elements = 1usize;
    let rank = usize::try_from(meta.shape.rank).map_err(|_| InferenceError::InvalidGraph)?;
    if rank == 0 || rank > MAX_RANK {
        return Err(InferenceError::InvalidGraph);
    }
    let mut idx = 0usize;
    while idx < rank {
        let dim =
            usize::try_from(meta.shape.dims[idx]).map_err(|_| InferenceError::InvalidGraph)?;
        if dim == 0 {
            return Err(InferenceError::InvalidGraph);
        }
        shape_elements = shape_elements
            .checked_mul(dim)
            .ok_or(InferenceError::InvalidGraph)?;
        idx += 1;
    }
    if elements != shape_elements {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(elements)
}

fn tensor_to_ops_rhs(
    model: &Model,
    name: &str,
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, InferenceError> {
    let raw = tensor_to_f32(model, name, rows, cols)?;
    if raw.len() != rows.checked_mul(cols).ok_or(InferenceError::InvalidGraph)? {
        return Err(InferenceError::InvalidGraph);
    }
    let mut out = vec![0.0f32; raw.len()];
    let mut col = 0usize;
    while col < cols {
        let mut row = 0usize;
        while row < rows {
            out[row * cols + col] = raw[col * rows + row];
            row += 1;
        }
        col += 1;
    }
    Ok(out)
}

fn tensor_to_f32(
    model: &Model,
    name: &str,
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, InferenceError> {
    let (meta, tensor) = model
        .mint_weight_named(name)
        .ok_or(InferenceError::InvalidGraph)?;
    let data = tensor.bytes();
    match meta.dtype {
        NeomDtype::F32 => f32_from_bytes(data),
        NeomDtype::F16 => Ok(data
            .chunks_exact(2)
            .map(|chunk| f16_bits_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
            .collect()),
        NeomDtype::BF16 => Ok(data
            .chunks_exact(2)
            .map(|chunk| f32::from_bits((u16::from_le_bytes([chunk[0], chunk[1]]) as u32) << 16))
            .collect()),
        NeomDtype::U8 => {
            let dtype = infer_quant_dtype(name, rows, cols, data.len())?;
            let gguf = gguf_tensor(
                name,
                real_tensor_dtype_to_ggml(dtype),
                &[rows as u64, cols as u64],
            );
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&gguf, data)
                .map_err(|_| InferenceError::InvalidGraph)
        }
        _ => Err(InferenceError::InvalidGraph),
    }
}

fn tensor_to_f32_if_present(
    model: &Model,
    name: &str,
    rows: usize,
    cols: usize,
) -> Result<Option<Vec<f32>>, InferenceError> {
    if model.mint_weight_named(name).is_none() {
        return Ok(None);
    }
    let values = tensor_to_f32(model, name, rows, cols)?;
    let expected = rows.checked_mul(cols).ok_or(InferenceError::InvalidGraph)?;
    if values.len() != expected {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(Some(values))
}

fn token_embedding(
    model: &Model,
    token: u32,
    hidden_size: usize,
    vocab_size: usize,
) -> Result<Vec<f32>, InferenceError> {
    if token as usize >= vocab_size {
        return Err(InferenceError::InvalidInput);
    }
    let (meta, tensor) = model
        .mint_weight_named("token_embd.weight")
        .ok_or(InferenceError::InvalidGraph)?;
    let data = tensor.bytes();
    match meta.dtype {
        NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => {
            let element_size = usize::try_from(meta.dtype.size_bytes())
                .map_err(|_| InferenceError::InvalidGraph)?;
            let row_bytes = hidden_size
                .checked_mul(element_size)
                .ok_or(InferenceError::InvalidGraph)?;
            let start = (token as usize)
                .checked_mul(row_bytes)
                .ok_or(InferenceError::InvalidGraph)?;
            let row = data
                .get(start..start + row_bytes)
                .ok_or(InferenceError::InvalidGraph)?;
            scalar_data_to_f32(meta.dtype, row)
        }
        NeomDtype::U8 => {
            let dtype =
                infer_quant_dtype("token_embd.weight", hidden_size, vocab_size, data.len())?;
            let ty = real_tensor_dtype_to_ggml(dtype);
            let block =
                usize::try_from(ty.block_size()).map_err(|_| InferenceError::InvalidGraph)?;
            let type_size =
                usize::try_from(ty.type_size()).map_err(|_| InferenceError::InvalidGraph)?;
            if !hidden_size.is_multiple_of(block) {
                return Err(InferenceError::InvalidGraph);
            }
            let row_bytes = hidden_size
                .checked_div(block)
                .and_then(|blocks| blocks.checked_mul(type_size))
                .ok_or(InferenceError::InvalidGraph)?;
            let start = (token as usize)
                .checked_mul(row_bytes)
                .ok_or(InferenceError::InvalidGraph)?;
            let row = data
                .get(start..start + row_bytes)
                .ok_or(InferenceError::InvalidGraph)?;
            let row_tensor = gguf_tensor("token_embd.row", ty, &[hidden_size as u64]);
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&row_tensor, row)
                .map_err(|_| InferenceError::InvalidGraph)
        }
        _ => Err(InferenceError::InvalidGraph),
    }
}

fn scalar_data_to_f32(dtype: NeomDtype, data: &[u8]) -> Result<Vec<f32>, InferenceError> {
    match dtype {
        NeomDtype::F32 => f32_from_bytes(data),
        NeomDtype::F16 => Ok(data
            .chunks_exact(2)
            .map(|chunk| f16_bits_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
            .collect()),
        NeomDtype::BF16 => Ok(data
            .chunks_exact(2)
            .map(|chunk| f32::from_bits((u16::from_le_bytes([chunk[0], chunk[1]]) as u32) << 16))
            .collect()),
        _ => Err(InferenceError::InvalidGraph),
    }
}

fn f32_from_bytes(data: &[u8]) -> Result<Vec<f32>, InferenceError> {
    if !data.len().is_multiple_of(4) {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(data
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn tensor_to_quantized_arc(
    model: &Model,
    name: &str,
    rows: usize,
    cols: usize,
) -> Result<Option<Arc<Tensor>>, InferenceError> {
    let (meta, tensor) = model
        .mint_weight_named(name)
        .ok_or(InferenceError::InvalidGraph)?;
    if !matches!(meta.dtype, NeomDtype::U8 | NeomDtype::Q5K) {
        return Ok(None);
    }
    if meta.dtype == NeomDtype::Q5K {
        let inferred_cols = quantized_rhs_cols_for_type(rows, tensor.bytes().len(), GgmlType::Q5K)?;
        if inferred_cols != cols {
            return Err(InferenceError::InvalidGraph);
        }
    } else {
        infer_quant_dtype(name, rows, cols, tensor.bytes().len())?;
    }
    Ok(Some(tensor))
}

fn tensor_dtype(
    model: &Model,
    name: &str,
    rows: usize,
    cols: usize,
) -> Result<TensorDtype, InferenceError> {
    let (meta, tensor) = model
        .mint_weight_named(name)
        .ok_or(InferenceError::InvalidGraph)?;
    match meta.dtype {
        NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => Ok(TensorDtype::F32),
        NeomDtype::Q5K => {
            let inferred_cols =
                quantized_rhs_cols_for_type(rows, tensor.bytes().len(), GgmlType::Q5K)?;
            if inferred_cols != cols {
                return Err(InferenceError::InvalidGraph);
            }
            Ok(TensorDtype::Q5K)
        }
        NeomDtype::U8 => infer_quant_dtype(name, rows, cols, tensor.bytes().len()),
        _ => Err(InferenceError::InvalidGraph),
    }
}

fn infer_quant_dtype(
    _name: &str,
    rows: usize,
    cols: usize,
    byte_len: usize,
) -> Result<TensorDtype, InferenceError> {
    let candidates = [
        (TensorDtype::Q4K, GgmlType::Q4K),
        (TensorDtype::Q5_0, GgmlType::Q5_0),
        (TensorDtype::Q6K, GgmlType::Q6K),
        (TensorDtype::Q8_0, GgmlType::Q8_0),
    ];
    let mut matched = None;
    for (dtype, ty) in candidates {
        let block = usize::try_from(ty.block_size()).map_err(|_| InferenceError::InvalidGraph)?;
        let type_size =
            usize::try_from(ty.type_size()).map_err(|_| InferenceError::InvalidGraph)?;
        if !rows.is_multiple_of(block) {
            continue;
        }
        let expected = cols
            .checked_mul(rows / block)
            .and_then(|value| value.checked_mul(type_size))
            .ok_or(InferenceError::InvalidGraph)?;
        if expected == byte_len {
            if matched.is_some() {
                return Err(InferenceError::InvalidGraph);
            }
            matched = Some(dtype);
        }
    }
    matched.ok_or(InferenceError::InvalidGraph)
}

fn real_tensor_dtype_to_ggml(dtype: TensorDtype) -> GgmlType {
    match dtype {
        TensorDtype::F32 => GgmlType::F32,
        TensorDtype::Q4K => GgmlType::Q4K,
        TensorDtype::Q5_0 => GgmlType::Q5_0,
        TensorDtype::Q5K => GgmlType::Q5K,
        TensorDtype::Q6K => GgmlType::Q6K,
        TensorDtype::Q8_0 => GgmlType::Q8_0,
    }
}

fn gguf_tensor(name: &str, ty: GgmlType, dims_in: &[u64]) -> GgufTensorInfo {
    let mut dims = [1u64; GGML_MAX_DIMS];
    for (idx, dim) in dims_in.iter().copied().enumerate().take(GGML_MAX_DIMS) {
        dims[idx] = dim;
    }
    let byte_len = dims_in
        .iter()
        .copied()
        .try_fold(1u64, |acc, dim| acc.checked_mul(dim))
        .and_then(|elements| {
            elements
                .checked_div(ty.block_size())
                .and_then(|blocks| blocks.checked_mul(ty.type_size()))
        })
        .unwrap_or(0);
    GgufTensorInfo {
        name: String::from(name),
        n_dims: dims_in.len() as u32,
        dims,
        ty,
        offset: 0,
        byte_len,
        data_start: 0,
    }
}

fn logits_tensor(logits: &[f32]) -> Result<Tensor, InferenceError> {
    let mut bytes = Vec::with_capacity(
        logits
            .len()
            .checked_mul(core::mem::size_of::<f32>())
            .ok_or(InferenceError::InvalidGraph)?,
    );
    for value in logits {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut dims = [0u64; MAX_RANK];
    dims[0] = logits.len() as u64;
    let desc = TensorCreateDesc {
        rank: 1,
        dtype: Dtype::F32.as_abi(),
        mutable: 0,
        _pad: 0,
        dims,
        data_ptr: 0,
        data_len: bytes.len() as u64,
    };
    Tensor::from_desc_and_bytes(desc, &bytes).map_err(InferenceError::Tensor)
}

fn qwen_layer_tensor_name(layer: usize, suffix: &str) -> String {
    use alloc::format;

    format!("blk.{layer}.{suffix}")
}

fn ffn_norm_tensor_candidates(layer: usize) -> [String; 2] {
    use alloc::format;

    [
        format!("blk.{layer}.post_attention_norm.weight"),
        format!("blk.{layer}.ffn_norm.weight"),
    ]
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn gdn_layer_tensor_candidates(layer: usize, suffix: &str) -> [String; 5] {
    use alloc::format;

    let qwen35_name = match suffix {
        "in_proj_qkv.weight" => format!("blk.{layer}.attn_qkv.weight"),
        "in_proj_z.weight" => format!("blk.{layer}.attn_gate.weight"),
        "in_proj_b.weight" => format!("blk.{layer}.ssm_beta.weight"),
        "in_proj_a.weight" => format!("blk.{layer}.ssm_alpha.weight"),
        "conv1d.weight" => format!("blk.{layer}.ssm_conv1d.weight"),
        "A_log" => format!("blk.{layer}.ssm_a"),
        "dt_bias" => format!("blk.{layer}.ssm_dt.bias"),
        "norm.weight" => format!("blk.{layer}.ssm_norm.weight"),
        "out_proj.weight" => format!("blk.{layer}.ssm_out.weight"),
        _ => format!("blk.{layer}.{suffix}"),
    };

    [
        qwen35_name,
        format!("blk.{layer}.gdn.{suffix}"),
        format!("blk.{layer}.linear_attn.{suffix}"),
        format!("blk.{layer}.ssm.{suffix}"),
        format!("blk.{layer}.{suffix}"),
    ]
}

fn map_forward(_error: ForwardLayerError) -> InferenceError {
    InferenceError::InvalidGraph
}

#[cfg_attr(not(feature = "ot16-graph-walker"), allow(dead_code))]
fn map_gdn(_error: gdn::GdnError) -> InferenceError {
    InferenceError::InvalidGraph
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
