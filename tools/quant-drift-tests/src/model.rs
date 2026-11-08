// SPDX-License-Identifier: MPL-2.0
//! `.neom` loader + the kernel forward pass over it.
//!
//! The loader and tensor helpers mirror `tools/ai-ops-tests`' real-model harness
//! (`src/lib.rs`, `read_real_neom_or_skip` / `real_neom_*`), trimmed to the
//! Qwen2 attention stack this 0.5B model uses. The maths is not re-implemented:
//! every layer runs `forward_logic::forward_layer_with_gqa_attention_with_norms`
//! and the output projection runs `forward_logic::qwen2_output_logits`.

use std::path::Path;

use crate::forward_logic::{
    self, Qwen2KvCacheSet, Qwen2LayerConfig, Qwen2LayerDtypes, Qwen2LayerWeights,
    Qwen2OutputWeights, Qwen2QuantizedLayerWeights, QwenAttentionNormWeights, TensorDtype,
};
use crate::gguf_dequant_logic;
use crate::gguf_logic::{GGML_MAX_DIMS, GgmlType, GgufTensorInfo};
use crate::model_logic::{
    LayerKind, NeomDtype, derive_model_config, resolve_schedule, validate_neom,
};
use crate::vocab_sidecar::{QwenTemplate, RealVocab};

pub const QWEN25_05B_LAYERS: usize = forward_logic::QWEN25_05B_LAYER_COUNT;
pub const QWEN25_05B_VOCAB: usize = 151_936;
pub const QWEN_IM_END_ID: u32 = crate::vocab_sidecar::QWEN_IM_END_ID;

#[derive(Clone, Debug)]
pub struct NeomTensor {
    pub name: String,
    pub dtype: NeomDtype,
    pub rank: u32,
    pub dims: [u64; 8],
    pub weight_off: usize,
    pub weight_len: usize,
}

pub struct Model {
    bytes: Vec<u8>,
    tensors: Vec<NeomTensor>,
    pub config: Qwen2LayerConfig,
    pub layer_count: usize,
    pub vocab_size: usize,
    /// `true` iff every scheduled layer is the Qwen2 attention+FFN stack. The
    /// harness refuses a hybrid/GDN model rather than silently skipping layers.
    pub all_attention: bool,
}

#[derive(Clone, Debug)]
pub struct DecodeStep {
    pub token: u32,
    pub logit: f32,
    pub runner_up_token: u32,
    pub runner_up_logit: f32,
}

struct RuntimeState {
    kv: Qwen2KvCacheSet,
}

impl Model {
    pub fn load(neom_path: &Path) -> Result<(Self, RealVocab), String> {
        let bytes = std::fs::read(neom_path)
            .map_err(|err| format!("read {}: {err}", neom_path.display()))?;
        let validated = validate_neom(&bytes).map_err(|err| format!("{err:?}"))?;
        let model_config =
            derive_model_config(&bytes, &validated).map_err(|err| format!("{err:?}"))?;
        let config =
            Qwen2LayerConfig::for_model_config(model_config).map_err(|err| format!("{err:?}"))?;
        let schedule = resolve_schedule(&bytes, &validated.header, QWEN25_05B_LAYERS)
            .map_err(|err| format!("{err:?}"))?;
        let all_attention = schedule
            .layers
            .iter()
            .all(|layer| layer.kind == LayerKind::Qwen2AttentionFFN);
        if !all_attention {
            return Err("quant-drift supports only the Qwen2 attention stack".to_string());
        }
        let tensors = parse_neom(&bytes)?;

        let vocab_path = neom_path.with_extension("vocab");
        let sidecar = std::fs::read(&vocab_path)
            .map_err(|err| format!("read {}: {err}", vocab_path.display()))?;
        let vocab = crate::vocab_sidecar::try_build_real_vocab(&sidecar)
            .ok_or_else(|| format!("{} is not a valid .vocab sidecar", vocab_path.display()))?;

        Ok((
            Self {
                bytes,
                tensors,
                config,
                layer_count: schedule.layers.len(),
                vocab_size: QWEN25_05B_VOCAB,
                all_attention,
            },
            vocab,
        ))
    }

    fn tensor_named_opt(&self, name: &str) -> Option<&NeomTensor> {
        self.tensors.iter().find(|tensor| tensor.name == name)
    }

    fn tensor_named(&self, name: &str) -> &NeomTensor {
        self.tensor_named_opt(name)
            .unwrap_or_else(|| panic!("real .neom tensor '{name}' exists"))
    }

    fn tensor_data(&self, tensor: &NeomTensor) -> &[u8] {
        &self.bytes[tensor.weight_off..tensor.weight_off + tensor.weight_len]
    }

    /// Frame a user turn exactly as Arthur's boot path does (`QwenTemplate`,
    /// atomic `<|im_start|>`/`<|im_end|>` IDs, default system turn).
    pub fn frame_prompt(&self, vocab: &RealVocab, user: &str) -> Result<Vec<u32>, String> {
        let mut out = Vec::new();
        QwenTemplate::new()
            .frame_turn_ids(
                user,
                |segment| vocab.tokenizer.encode(segment).ok(),
                &mut out,
            )
            .ok_or_else(|| format!("tokenize prompt {user:?}"))?;
        Ok(out)
    }

    pub fn encode(&self, vocab: &RealVocab, text: &str) -> Result<Vec<u32>, String> {
        vocab
            .tokenizer
            .encode(text)
            .map_err(|err| format!("encode corpus: {err:?}"))
    }

    fn runtime(&self, max_context: usize) -> RuntimeState {
        RuntimeState {
            kv: Qwen2KvCacheSet::new(self.layer_count, self.config, max_context)
                .expect("allocate KV cache"),
        }
    }

    /// Greedy-decode `max_new` tokens after `prompt` (32 in the KAT).
    pub fn greedy(&self, prompt: &[u32], max_new: usize) -> Vec<u32> {
        self.greedy_with_steps(prompt, max_new)
            .into_iter()
            .map(|step| step.token)
            .collect()
    }

    /// Greedy-decode `max_new` tokens, keeping each step's top-1/top-2 logits
    /// so a cross-runtime divergence can be classified as a near-tie or a
    /// clear gap.
    pub fn greedy_with_steps(&self, prompt: &[u32], max_new: usize) -> Vec<DecodeStep> {
        let mut state = self.runtime(prompt.len() + max_new);
        let mut logits = Vec::new();
        for (position, &token) in prompt.iter().enumerate() {
            logits = self.next_logits(token, position, &mut state);
        }
        let mut out = Vec::with_capacity(max_new);
        for step in 0..max_new {
            let choice = top_two(&logits);
            let token = choice.token;
            out.push(choice);
            if step + 1 < max_new {
                logits = self.next_logits(token, prompt.len() + step, &mut state);
            }
        }
        out
    }

    /// Top-1 next-token at every position of `tokens`, teacher-forced on the
    /// same IDs all three models receive. Index `i` is the prediction after
    /// consuming `tokens[i]` (i.e. for `tokens[i + 1]`).
    pub fn top1_per_position(&self, tokens: &[u32]) -> Vec<u32> {
        let mut state = self.runtime(tokens.len());
        let mut out = Vec::with_capacity(tokens.len());
        for (position, &token) in tokens.iter().enumerate() {
            let logits = self.next_logits(token, position, &mut state);
            out.push(argmax(&logits).expect("non-empty logits") as u32);
        }
        out
    }

    /// Top-1 and top-2 next-token at every position of `tokens`, teacher-forced
    /// on the same IDs all three models receive. Used for the bar-A near-tie
    /// evidence: a divergence at a wide gap is a forward-pass difference, a
    /// divergence at a narrow gap is numerics.
    pub fn top_two_per_position(&self, tokens: &[u32]) -> Vec<DecodeStep> {
        let mut state = self.runtime(tokens.len());
        let mut out = Vec::with_capacity(tokens.len());
        for (position, &token) in tokens.iter().enumerate() {
            let logits = self.next_logits(token, position, &mut state);
            out.push(top_two(&logits));
        }
        out
    }

    /// Sum of negative log-likelihoods for targets at indices `eval_start + 1 ..
    /// tokens.len()`, plus the count. Position `i` predicts `tokens[i + 1]`.
    pub fn nll_sum(&self, tokens: &[u32], eval_start: usize) -> (f64, usize) {
        assert!(tokens.len() >= 2 && eval_start < tokens.len() - 1);
        let mut state = self.runtime(tokens.len());
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for (position, &token) in tokens.iter().enumerate() {
            let logits = self.next_logits(token, position, &mut state);
            if position >= eval_start && position + 1 < tokens.len() {
                let target = tokens[position + 1] as usize;
                let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let sum_exp: f64 = logits
                    .iter()
                    .map(|&logit| ((logit - max) as f64).exp())
                    .sum();
                let log_sum_exp = max as f64 + sum_exp.ln();
                sum += log_sum_exp - logits[target] as f64;
                count += 1;
            }
        }
        (sum, count)
    }

    fn next_logits(&self, token: u32, position: usize, state: &mut RuntimeState) -> Vec<f32> {
        let config = self.config;
        let mut hidden = self.token_embedding(token);
        for layer_idx in 0..self.layer_count {
            let layer = self.attention_layer_buffers(layer_idx);
            let out = forward_logic::forward_layer_with_gqa_attention_with_norms(
                &hidden,
                layer.weights(),
                config,
                position as u64,
                true,
                Some(state.kv.layer_mut(layer_idx).expect("kv layer")),
                None,
                layer.attention_norms(),
            )
            .expect("real .neom attention layer");
            hidden = out.ffn.residual;
        }

        let output_norm = self.tensor_to_f32("output_norm.weight", 1, config.hidden_size);
        let output_dtype = self.tensor_dtype("output.weight", config.hidden_size, self.vocab_size);
        let output = self.tensor_to_ops_rhs_placeholder(
            "output.weight",
            config.hidden_size,
            self.vocab_size,
        );
        let output_quantized =
            self.tensor_to_quantized_rhs("output.weight", config.hidden_size, self.vocab_size);
        forward_logic::qwen2_output_logits(
            &hidden,
            Qwen2OutputWeights {
                output_norm: &output_norm,
                output: &output,
                output_dtype,
                output_quantized,
                output_bias: None,
            },
            config,
            self.vocab_size,
        )
        .expect("real .neom output logits")
    }

    fn token_embedding(&self, token: u32) -> Vec<f32> {
        assert!((token as usize) < self.vocab_size);
        let tensor = self.tensor_named("token_embd.weight");
        let data = self.tensor_data(tensor);
        let hidden = self.config.hidden_size;
        match tensor.dtype {
            NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => {
                let element_size = tensor.dtype.size_bytes() as usize;
                let row_bytes = hidden * element_size;
                let start = token as usize * row_bytes;
                scalar_to_f32(tensor.dtype, &data[start..start + row_bytes])
            }
            NeomDtype::U8 => {
                let dtype =
                    infer_quant_dtype("token_embd.weight", hidden, self.vocab_size, data.len())
                        .expect("token_embd quant dtype");
                let ty = dtype_to_ggml(dtype);
                let row_bytes = hidden / ty.block_size() as usize * ty.type_size() as usize;
                let start = token as usize * row_bytes;
                let row = &data[start..start + row_bytes];
                let row_tensor = gguf_tensor("token_embd.row", ty, &[hidden as u64]);
                tensor_data_to_f32(&row_tensor, row)
            }
            other => panic!("token_embd.weight has unsupported dtype {other:?}"),
        }
    }

    fn attention_layer_buffers(&self, layer: usize) -> AttentionLayerBuffers<'_> {
        let config = self.config;
        let q = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "attn_q.weight"),
            config.hidden_size,
            config.attn_q_projection_size,
        );
        let k = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "attn_k.weight"),
            config.hidden_size,
            config.kv_len(),
        );
        let v = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "attn_v.weight"),
            config.hidden_size,
            config.kv_len(),
        );
        let o = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "attn_output.weight"),
            config.q_len(),
            config.hidden_size,
        );
        let gate = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "ffn_gate.weight"),
            config.hidden_size,
            config.intermediate_size,
        );
        let up = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "ffn_up.weight"),
            config.hidden_size,
            config.intermediate_size,
        );
        let down = self.tensor_to_ops_rhs_placeholder(
            &qwen_name(layer, "ffn_down.weight"),
            config.intermediate_size,
            config.hidden_size,
        );
        AttentionLayerBuffers {
            attn_norm: self.tensor_to_f32(
                &qwen_name(layer, "attn_norm.weight"),
                1,
                config.hidden_size,
            ),
            attn_q: q,
            attn_k: k,
            attn_v: v,
            attn_q_bias: self.tensor_if_present(
                &qwen_name(layer, "attn_q.bias"),
                1,
                config.attn_q_projection_size,
            ),
            attn_k_bias: self.tensor_if_present(
                &qwen_name(layer, "attn_k.bias"),
                1,
                config.kv_len(),
            ),
            attn_v_bias: self.tensor_if_present(
                &qwen_name(layer, "attn_v.bias"),
                1,
                config.kv_len(),
            ),
            attn_q_norm: self.tensor_if_present(
                &qwen_name(layer, "attn_q_norm.weight"),
                1,
                config.head_dim,
            ),
            attn_k_norm: self.tensor_if_present(
                &qwen_name(layer, "attn_k_norm.weight"),
                1,
                config.head_dim,
            ),
            attn_o: o,
            ffn_norm: self.tensor_flat_any(
                &[
                    qwen_name(layer, "post_attention_norm.weight"),
                    qwen_name(layer, "ffn_norm.weight"),
                ],
                config.hidden_size,
            ),
            ffn_gate: gate,
            ffn_up: up,
            ffn_down: down,
            dtypes: Qwen2LayerDtypes {
                attn_q: self.tensor_dtype(
                    &qwen_name(layer, "attn_q.weight"),
                    config.hidden_size,
                    config.attn_q_projection_size,
                ),
                attn_k: self.tensor_dtype(
                    &qwen_name(layer, "attn_k.weight"),
                    config.hidden_size,
                    config.kv_len(),
                ),
                attn_v: self.tensor_dtype(
                    &qwen_name(layer, "attn_v.weight"),
                    config.hidden_size,
                    config.kv_len(),
                ),
                attn_o: self.tensor_dtype(
                    &qwen_name(layer, "attn_output.weight"),
                    config.q_len(),
                    config.hidden_size,
                ),
                ffn_gate: self.tensor_dtype(
                    &qwen_name(layer, "ffn_gate.weight"),
                    config.hidden_size,
                    config.intermediate_size,
                ),
                ffn_up: self.tensor_dtype(
                    &qwen_name(layer, "ffn_up.weight"),
                    config.hidden_size,
                    config.intermediate_size,
                ),
                ffn_down: self.tensor_dtype(
                    &qwen_name(layer, "ffn_down.weight"),
                    config.intermediate_size,
                    config.hidden_size,
                ),
            },
            attn_q_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "attn_q.weight"),
                config.hidden_size,
                config.attn_q_projection_size,
            ),
            attn_k_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "attn_k.weight"),
                config.hidden_size,
                config.kv_len(),
            ),
            attn_v_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "attn_v.weight"),
                config.hidden_size,
                config.kv_len(),
            ),
            attn_o_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "attn_output.weight"),
                config.q_len(),
                config.hidden_size,
            ),
            ffn_gate_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "ffn_gate.weight"),
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "ffn_up.weight"),
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down_quantized: self.tensor_to_quantized_rhs(
                &qwen_name(layer, "ffn_down.weight"),
                config.intermediate_size,
                config.hidden_size,
            ),
        }
    }

    fn tensor_if_present(&self, name: &str, rows: usize, cols: usize) -> Option<Vec<f32>> {
        self.tensor_named_opt(name)
            .map(|_| self.tensor_to_f32(name, rows, cols))
    }

    fn tensor_flat_any(&self, names: &[String], len: usize) -> Vec<f32> {
        for name in names {
            if self.tensor_named_opt(name).is_some() {
                return self.tensor_to_f32(name, 1, len);
            }
        }
        panic!("real .neom tensor exists for one of {names:?}");
    }

    fn tensor_to_f32(&self, name: &str, rows: usize, cols: usize) -> Vec<f32> {
        let tensor = self.tensor_named(name);
        let data = self.tensor_data(tensor);
        match tensor.dtype {
            NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => {
                let values = scalar_to_f32(tensor.dtype, data);
                assert_eq!(values.len(), rows * cols, "{name} element count");
                values
            }
            NeomDtype::U8 => {
                let dtype = infer_quant_dtype(name, rows, cols, data.len())
                    .unwrap_or_else(|err| panic!("{name}: {err}"));
                let gguf = gguf_tensor(name, dtype_to_ggml(dtype), &[rows as u64, cols as u64]);
                tensor_data_to_f32(&gguf, data)
            }
            other => panic!("{name} has unsupported real .neom dtype {other:?}"),
        }
    }

    /// Ops-layout (transposed) f32 weights for a non-quantized tensor; an empty
    /// vector for a quantized one, which travels through `*_quantized`.
    fn tensor_to_ops_rhs_placeholder(&self, name: &str, rows: usize, cols: usize) -> Vec<f32> {
        if self.tensor_dtype(name, rows, cols).is_quantized() {
            Vec::new()
        } else {
            let raw = self.tensor_to_f32(name, rows, cols);
            let mut out = vec![0.0f32; raw.len()];
            for col in 0..cols {
                for row in 0..rows {
                    out[row * cols + col] = raw[col * rows + row];
                }
            }
            out
        }
    }

    fn tensor_to_quantized_rhs<'a>(
        &'a self,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Option<&'a [u8]> {
        let tensor = self.tensor_named(name);
        if !matches!(tensor.dtype, NeomDtype::U8 | NeomDtype::Q5K) {
            return None;
        }
        assert!(self.tensor_dtype(name, rows, cols).is_quantized());
        Some(self.tensor_data(tensor))
    }

    fn tensor_dtype(&self, name: &str, rows: usize, cols: usize) -> TensorDtype {
        let tensor = self.tensor_named(name);
        match tensor.dtype {
            NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => TensorDtype::F32,
            NeomDtype::Q5K => {
                assert_eq!(quantized_cols(rows, tensor.weight_len, GgmlType::Q5K), cols);
                TensorDtype::Q5K
            }
            NeomDtype::U8 => infer_quant_dtype(name, rows, cols, tensor.weight_len).unwrap(),
            other => panic!("{name} has unsupported real .neom dtype {other:?}"),
        }
    }
}

struct AttentionLayerBuffers<'a> {
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
    attn_q_quantized: Option<&'a [u8]>,
    attn_k_quantized: Option<&'a [u8]>,
    attn_v_quantized: Option<&'a [u8]>,
    attn_o_quantized: Option<&'a [u8]>,
    ffn_gate_quantized: Option<&'a [u8]>,
    ffn_up_quantized: Option<&'a [u8]>,
    ffn_down_quantized: Option<&'a [u8]>,
}

impl AttentionLayerBuffers<'_> {
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
                attn_q: self.attn_q_quantized,
                attn_k: self.attn_k_quantized,
                attn_v: self.attn_v_quantized,
                attn_o: self.attn_o_quantized,
                ffn_gate: self.ffn_gate_quantized,
                ffn_up: self.ffn_up_quantized,
                ffn_down: self.ffn_down_quantized,
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

pub fn argmax(values: &[f32]) -> Option<usize> {
    let mut best = None;
    let mut best_value = f32::NEG_INFINITY;
    for (index, &value) in values.iter().enumerate() {
        if value > best_value {
            best_value = value;
            best = Some(index);
        }
    }
    best
}

pub fn top_two(logits: &[f32]) -> DecodeStep {
    assert!(logits.len() >= 2);
    let mut best = 0usize;
    let mut best_value = logits[0];
    let mut runner = 1usize;
    let mut runner_value = logits[1];
    if runner_value > best_value {
        std::mem::swap(&mut best, &mut runner);
        std::mem::swap(&mut best_value, &mut runner_value);
    }
    for (token, &logit) in logits.iter().enumerate().skip(2) {
        if logit > best_value {
            runner = best;
            runner_value = best_value;
            best = token;
            best_value = logit;
        } else if logit > runner_value {
            runner = token;
            runner_value = logit;
        }
    }
    DecodeStep {
        token: best as u32,
        logit: best_value,
        runner_up_token: runner as u32,
        runner_up_logit: runner_value,
    }
}

fn parse_neom(bytes: &[u8]) -> Result<Vec<NeomTensor>, String> {
    if bytes.len() < 96 {
        return Err("real .neom is shorter than the fixed header".to_string());
    }
    if bytes.get(0..4) != Some(b"NEOM") {
        return Err("real .neom has invalid magic".to_string());
    }
    let version = read_u32(bytes, 4)?;
    let header_size = read_u32(bytes, 8)? as usize;
    if !matches!(version, 1..=4) {
        return Err(format!("real .neom version {version} is unsupported"));
    }
    let expected_header = match version {
        1 => 96,
        2 => 108,
        3 => 124,
        _ => 128,
    };
    if header_size != expected_header {
        return Err(format!("real .neom version {version} has bad header size"));
    }
    if read_u32(bytes, 12)? != 0 {
        return Err("real .neom flags must be zero".to_string());
    }
    let tensor_count = read_u32(bytes, 16)? as usize;
    let tensor_table_off = read_u64(bytes, 32)? as usize;
    let string_off = read_u64(bytes, 56)? as usize;
    let string_len = read_u64(bytes, 64)? as usize;
    if read_u64(bytes, 88)? as usize != bytes.len() {
        return Err("real .neom total_size != file size".to_string());
    }
    checked_slice(bytes, tensor_table_off, tensor_count * 96)?;
    checked_slice(bytes, string_off, string_len)?;

    let mut tensors = Vec::with_capacity(tensor_count);
    for index in 0..tensor_count {
        let base = tensor_table_off + index * 96;
        let dtype = NeomDtype::from_abi(read_u32(bytes, base)?)
            .map_err(|_| format!("real .neom tensor[{index}] has invalid dtype"))?;
        let rank = read_u32(bytes, base + 4)?;
        let mut dims = [0u64; 8];
        for (dim_index, dim) in dims.iter_mut().enumerate() {
            *dim = read_u64(bytes, base + 8 + dim_index * 8)?;
        }
        let weight_off = read_u64(bytes, base + 72)? as usize;
        let weight_len = read_u64(bytes, base + 80)? as usize;
        let name_off = read_u32(bytes, base + 88)? as usize;
        let name_len = read_u32(bytes, base + 92)? as usize;
        checked_slice(bytes, weight_off, weight_len)?;
        let name = std::str::from_utf8(checked_slice(bytes, string_off + name_off, name_len)?)
            .map_err(|err| format!("real .neom tensor[{index}] name is not UTF-8: {err}"))?
            .to_string();
        tensors.push(NeomTensor {
            name,
            dtype,
            rank,
            dims,
            weight_off,
            weight_len,
        });
    }
    Ok(tensors)
}

fn scalar_to_f32(dtype: NeomDtype, data: &[u8]) -> Vec<f32> {
    match dtype {
        NeomDtype::F32 => data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
        NeomDtype::F16 => data
            .chunks_exact(2)
            .map(|chunk| f16_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
            .collect(),
        NeomDtype::BF16 => data
            .chunks_exact(2)
            .map(|chunk| f32::from_bits((u16::from_le_bytes([chunk[0], chunk[1]]) as u32) << 16))
            .collect(),
        other => panic!("{other:?} is not a scalar dtype"),
    }
}

fn infer_quant_dtype(
    name: &str,
    rows: usize,
    cols: usize,
    byte_len: usize,
) -> Result<TensorDtype, String> {
    let mut matched = None;
    for (dtype, ty) in [
        (TensorDtype::Q4K, GgmlType::Q4K),
        (TensorDtype::Q5_0, GgmlType::Q5_0),
        (TensorDtype::Q6K, GgmlType::Q6K),
        (TensorDtype::Q8_0, GgmlType::Q8_0),
    ] {
        let block = ty.block_size() as usize;
        if !rows.is_multiple_of(block) {
            continue;
        }
        let expected = cols * (rows / block) * ty.type_size() as usize;
        if expected == byte_len {
            if matched.is_some() {
                return Err(format!(
                    "{name} quant dtype is ambiguous for {byte_len} bytes"
                ));
            }
            matched = Some(dtype);
        }
    }
    matched.ok_or_else(|| {
        format!("{name} byte_len={byte_len} does not match q4_K/q5_0/q6_K/q8_0 for {rows}x{cols}")
    })
}

fn quantized_cols(rows: usize, byte_len: usize, ty: GgmlType) -> usize {
    let per_col = rows / ty.block_size() as usize * ty.type_size() as usize;
    assert_ne!(per_col, 0);
    assert_eq!(byte_len % per_col, 0);
    byte_len / per_col
}

fn dtype_to_ggml(dtype: TensorDtype) -> GgmlType {
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
    for (index, dim) in dims_in.iter().enumerate() {
        dims[index] = *dim;
    }
    let elements = dims_in.iter().product::<u64>();
    GgufTensorInfo {
        name: name.to_string(),
        n_dims: dims_in.len() as u32,
        dims,
        ty,
        offset: 0,
        byte_len: elements / ty.block_size() * ty.type_size(),
        data_start: 0,
    }
}

fn tensor_data_to_f32(tensor: &GgufTensorInfo, data: &[u8]) -> Vec<f32> {
    match tensor.ty {
        GgmlType::Q4K | GgmlType::Q5_0 | GgmlType::Q5K | GgmlType::Q6K | GgmlType::Q8_0 => {
            gguf_dequant_logic::dequantize_tensor_data_to_f32(tensor, data).unwrap()
        }
        GgmlType::F32 => data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
        GgmlType::F16 => data
            .chunks_exact(2)
            .map(|chunk| f16_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
            .collect(),
        other => panic!("unsupported tensor type {}", other.name()),
    }
}

fn f16_to_f32(bits: u16) -> f32 {
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

fn qwen_name(layer: usize, suffix: &str) -> String {
    format!("blk.{layer}.{suffix}")
}

fn checked_slice(bytes: &[u8], start: usize, len: usize) -> Result<&[u8], String> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| "real .neom range overflow".to_string())?;
    bytes
        .get(start..end)
        .ok_or_else(|| format!("real .neom range {start}..{end} is out of bounds"))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_slice(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        checked_slice(bytes, offset, 8)?.try_into().unwrap(),
    ))
}
