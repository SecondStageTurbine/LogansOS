// SPDX-License-Identifier: MPL-2.0
//! GGUF-to-`.neom` importer for Qwen2.5 host provisioning.
//!
//! `neomc` reaches the already-landed pure GGUF/tokenizer logic by path-including
//! the `kernel/src/ai` modules. This keeps L1 as an integration slice without
//! adopting GGUF in the kernel runtime: ring 0 still loads only validated `.neom`
//! files.

use std::fs;
use std::path::{Path, PathBuf};

use crate::format::{Dtype, LayerKind};
use crate::validator;
use crate::writer::{NeomBuilder, ScheduleEntrySpec, TensorSpec};

#[path = "../../../kernel/src/ai/gguf_logic.rs"]
mod gguf_logic;
#[path = "../../../kernel/src/ai/tokenizer_logic.rs"]
mod tokenizer_logic;

use gguf_logic::{GgmlType, GgufArray, GgufFile, GgufTensorInfo, GgufType, GgufValue, parse_gguf};
use tokenizer_logic::{BpeMergeEntry, BpeTokenizer, BpeVocabEntry};

/// Minimum declared context length we accept for a Qwen2-family GGUF. A floor,
/// not a pin: every Qwen2.5 model (0.5B/1.5B/3B) declares 32768, so this only
/// rejects a truncated/malformed header. The kernel runs a smaller live window
/// (`QWEN2_DEFAULT_CONTEXT`); this is purely an import sanity bound.
const QWEN2_MIN_CONTEXT: u64 = 4096;

const GGML_QK_K: u64 = 256;
const GGML_BLOCK_Q4_K_SIZE: u64 = 144;
const GGML_BLOCK_Q8_0_SIZE: u64 = 34;
const GGML_BLOCK_Q8_K_SIZE: u64 = 292;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportArch {
    Qwen2,
    Qwen35,
}

impl ImportArch {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "qwen2" => Ok(Self::Qwen2),
            "qwen35" | "qwen3_5" | "qwen3.5" => Ok(Self::Qwen35),
            other => Err(format!("unsupported import arch '{other}'")),
        }
    }

    fn metadata_value(self) -> &'static str {
        match self {
            Self::Qwen2 => "qwen2",
            Self::Qwen35 => "qwen35",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Qwen2 => "Qwen2",
            Self::Qwen35 => "Qwen3.5",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub arch: ImportArch,
    pub vocab_output: Option<PathBuf>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TensorImportCounts {
    pub f32: usize,
    pub f16: usize,
    pub bf16: usize,
    pub i32: usize,
    pub i8: usize,
    pub u8: usize,
    pub q4_k: usize,
    pub q5_0: usize,
    pub q5_k: usize,
    pub q6_k: usize,
    pub q8_0: usize,
    pub q8_k: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportInventory {
    pub tensor_count: usize,
    pub total_size: usize,
    pub weight_bytes: usize,
    pub output_path: PathBuf,
    pub vocab_path: PathBuf,
    pub vocab_tokens: usize,
    pub vocab_merges: usize,
    pub counts: TensorImportCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizerExport {
    pub tokens: Vec<String>,
    pub merges: Vec<String>,
    pub bos_token_id: Option<u64>,
    pub eos_token_id: Option<u64>,
    pub padding_token_id: Option<u64>,
}

pub fn import_gguf_file(
    input: &Path,
    output: &Path,
    options: &ImportOptions,
) -> Result<ImportInventory, String> {
    let bytes = fs::read(input)
        .map_err(|err| format!("failed to read GGUF '{}': {err}", input.display()))?;
    let imported = import_gguf_bytes(&bytes, options.arch, true)?;
    validator::validate(&imported.neom)
        .map_err(|err| format!("imported .neom failed validation: {err:?}"))?;

    ensure_parent_dir(output)?;
    fs::write(output, &imported.neom)
        .map_err(|err| format!("failed to write .neom '{}': {err}", output.display()))?;

    let vocab_path = options
        .vocab_output
        .clone()
        .unwrap_or_else(|| output.with_extension("vocab"));
    write_tokenizer_export(&vocab_path, &imported.tokenizer)?;

    Ok(ImportInventory {
        tensor_count: imported.tensor_count,
        total_size: imported.neom.len(),
        weight_bytes: imported.weight_bytes,
        output_path: output.to_path_buf(),
        vocab_path,
        vocab_tokens: imported.tokenizer.tokens.len(),
        vocab_merges: imported.tokenizer.merges.len(),
        counts: imported.counts,
    })
}

struct ImportedBytes {
    neom: Vec<u8>,
    tensor_count: usize,
    weight_bytes: usize,
    tokenizer: TokenizerExport,
    counts: TensorImportCounts,
}

fn import_gguf_bytes(
    bytes: &[u8],
    arch: ImportArch,
    validate_graph: bool,
) -> Result<ImportedBytes, String> {
    let gguf = parse_gguf(bytes).map_err(|err| format!("failed to parse GGUF: {err:?}"))?;
    let tokenizer = extract_tokenizer(&gguf)?;

    if validate_graph {
        match arch {
            ImportArch::Qwen2 => validate_qwen2_graph(&gguf, arch, tokenizer.tokens.len() as u64)?,
            ImportArch::Qwen35 => {
                validate_qwen35_graph(&gguf, arch, tokenizer.tokens.len() as u64)?
            }
        }
    } else {
        validate_arch_only(&gguf, arch)?;
    }

    let mut builder = NeomBuilder::new();
    let mut counts = TensorImportCounts::default();
    let mut weight_bytes = 0usize;
    let mut tensor_count = 0usize;

    for tensor in gguf.tensors.iter() {
        let spec = tensor_spec_from_gguf(&gguf, bytes, tensor, &mut counts)?;
        builder = add_imported_tensor(builder, spec, &mut weight_bytes, &mut tensor_count)?;
    }

    if arch == ImportArch::Qwen35 && gguf.tensor_named("output.weight").is_none() {
        let token_embd = gguf.tensor_named("token_embd.weight").ok_or_else(|| {
            "Qwen3.5 GGUF is missing tied-head tensor source 'token_embd.weight'".to_string()
        })?;
        let spec =
            tensor_spec_from_gguf_named(&gguf, bytes, token_embd, "output.weight", &mut counts)?;
        builder = add_imported_tensor(builder, spec, &mut weight_bytes, &mut tensor_count)?;
    }

    let ImportModelConfig {
        layer_count,
        rope_theta,
        rms_norm_eps,
        rope_dimension_count,
        schedule,
    } = read_import_model_config(&gguf, arch)?;

    builder = builder
        .set_layer_count(layer_count)
        .set_rope_theta(rope_theta)
        .set_rms_norm_eps(rms_norm_eps);
    if let Some(rope_dimension_count) = rope_dimension_count {
        builder = builder.set_rope_dimension_count(rope_dimension_count);
    }
    if let Some(entries) = schedule {
        builder = builder.set_schedule_entries(entries);
    }

    let neom = builder
        .build()
        .map_err(|err| format!("failed to build .neom: {err}"))?;

    Ok(ImportedBytes {
        neom,
        tensor_count,
        weight_bytes,
        tokenizer,
        counts,
    })
}

fn add_imported_tensor(
    builder: NeomBuilder,
    spec: TensorSpec,
    weight_bytes: &mut usize,
    tensor_count: &mut usize,
) -> Result<NeomBuilder, String> {
    *weight_bytes = weight_bytes
        .checked_add(spec.data.len())
        .ok_or_else(|| "imported weight byte count overflowed usize".to_string())?;
    *tensor_count = tensor_count
        .checked_add(1)
        .ok_or_else(|| "imported tensor count overflowed usize".to_string())?;
    Ok(builder.add_tensor(spec))
}

struct ImportModelConfig {
    layer_count: u32,
    rope_theta: f32,
    rms_norm_eps: f32,
    rope_dimension_count: Option<u32>,
    schedule: Option<Vec<ScheduleEntrySpec>>,
}

fn read_import_model_config(
    gguf: &GgufFile,
    arch: ImportArch,
) -> Result<ImportModelConfig, String> {
    let prefix = arch.metadata_value();
    let dims = match arch {
        ImportArch::Qwen2 => None,
        ImportArch::Qwen35 => Some(read_qwen35_dims(gguf)?),
    };
    let layer_count = match &dims {
        Some(Qwen35Dims { main_layers, .. }) => *main_layers,
        None => require_arch_metadata_u64(gguf, arch, &format!("{prefix}.block_count"))?,
    };
    let rope_theta = metadata_f32(gguf, &format!("{prefix}.rope.freq_base")).ok_or_else(|| {
        format!(
            "{} GGUF is missing metadata key '{}.rope.freq_base'",
            arch.display_name(),
            prefix
        )
    })?;
    let rms_norm_eps = metadata_f32(gguf, &format!("{prefix}.attention.layer_norm_rms_epsilon"))
        .ok_or_else(|| {
            format!(
                "{} GGUF is missing metadata key '{}.attention.layer_norm_rms_epsilon'",
                arch.display_name(),
                prefix
            )
        })?;
    let layer_count = u32::try_from(layer_count)
        .map_err(|_| format!("{} layer_count does not fit u32", arch.display_name()))?;
    let schedule = match dims.as_ref() {
        Some(dims) => Some(qwen35_schedule_entries(gguf, dims)?),
        None => None,
    };
    let rope_dimension_count = match arch {
        ImportArch::Qwen35 => {
            let value = require_arch_metadata_u64(gguf, arch, "qwen35.rope.dimension_count")?;
            let head_dim = dims.as_ref().expect("Qwen3.5 dimensions").head_dim;
            if value == 0 || !value.is_multiple_of(2) || value > head_dim {
                return Err(format!(
                    "Qwen3.5 GGUF rope.dimension_count {value} must be non-zero, even, and <= attention key_length {head_dim}"
                ));
            }
            Some(
                value
                    .try_into()
                    .map_err(|_| "Qwen3.5 rope.dimension_count does not fit u32".to_string())?,
            )
        }
        ImportArch::Qwen2 => None,
    };
    Ok(ImportModelConfig {
        layer_count,
        rope_theta,
        rms_norm_eps,
        rope_dimension_count,
        schedule,
    })
}

fn tensor_spec_from_gguf(
    gguf: &GgufFile,
    source: &[u8],
    tensor: &GgufTensorInfo,
    counts: &mut TensorImportCounts,
) -> Result<TensorSpec, String> {
    tensor_spec_from_gguf_named(gguf, source, tensor, &tensor.name, counts)
}

fn tensor_spec_from_gguf_named(
    gguf: &GgufFile,
    source: &[u8],
    tensor: &GgufTensorInfo,
    output_name: &str,
    counts: &mut TensorImportCounts,
) -> Result<TensorSpec, String> {
    let raw = gguf
        .tensor_data(source, tensor)
        .map_err(|err| format!("tensor '{}' points outside GGUF data: {err:?}", tensor.name))?;

    match tensor.ty {
        GgmlType::F32 => {
            counts.f32 += 1;
            Ok(typed_tensor_spec(tensor, output_name, Dtype::F32, raw))
        }
        GgmlType::F16 => {
            counts.f16 += 1;
            Ok(typed_tensor_spec(tensor, output_name, Dtype::F16, raw))
        }
        GgmlType::Bf16 => {
            counts.bf16 += 1;
            Ok(typed_tensor_spec(tensor, output_name, Dtype::BF16, raw))
        }
        GgmlType::I32 => {
            counts.i32 += 1;
            Ok(typed_tensor_spec(tensor, output_name, Dtype::I32, raw))
        }
        GgmlType::I8 => {
            counts.i8 += 1;
            Ok(typed_tensor_spec(tensor, output_name, Dtype::I8, raw))
        }
        GgmlType::Q4K => {
            validate_q4_k_layout(tensor, raw)?;
            counts.q4_k += 1;
            Ok(raw_quant_tensor_spec(output_name, raw))
        }
        GgmlType::Q5_0 => {
            validate_q5_0_layout(tensor, raw)?;
            counts.q5_0 += 1;
            Ok(raw_quant_tensor_spec(output_name, raw))
        }
        GgmlType::Q5K => {
            validate_qk_layout(tensor, raw, "Q5_K")?;
            validate_quant_block_bytes(tensor, raw, GgmlType::Q5K.type_size(), "Q5_K")?;
            counts.q5_k += 1;
            Ok(raw_quant_tensor_spec_with_dtype(
                output_name,
                raw,
                Dtype::Q5K,
            ))
        }
        GgmlType::Q6K => {
            validate_qk_layout(tensor, raw, "Q6_K")?;
            validate_quant_block_bytes(tensor, raw, GgmlType::Q6K.type_size(), "Q6_K")?;
            counts.q6_k += 1;
            Ok(raw_quant_tensor_spec(output_name, raw))
        }
        GgmlType::Q8_0 => {
            validate_q8_0_layout(tensor, raw)?;
            counts.q8_0 += 1;
            Ok(raw_quant_tensor_spec(output_name, raw))
        }
        GgmlType::Q8K => {
            validate_qk_layout(tensor, raw, "Q8_K")?;
            validate_quant_block_bytes(tensor, raw, GGML_BLOCK_Q8_K_SIZE, "Q8_K")?;
            counts.q8_k += 1;
            Ok(raw_quant_tensor_spec(output_name, raw))
        }
        other => Err(format!(
            "tensor '{}' uses unsupported GGML type {:?}",
            tensor.name, other
        )),
    }
}

fn typed_tensor_spec(
    tensor: &GgufTensorInfo,
    output_name: &str,
    dtype: Dtype,
    raw: &[u8],
) -> TensorSpec {
    let mut dims = [0u64; 8];
    let rank = tensor_rank(tensor);
    dims[..rank].copy_from_slice(&tensor.dims[..rank]);
    TensorSpec {
        name: output_name.to_string(),
        dtype,
        dims,
        rank: rank as u32,
        data: raw.to_vec(),
    }
}

fn raw_quant_tensor_spec(output_name: &str, raw: &[u8]) -> TensorSpec {
    raw_quant_tensor_spec_with_dtype(output_name, raw, Dtype::U8)
}

fn raw_quant_tensor_spec_with_dtype(output_name: &str, raw: &[u8], dtype: Dtype) -> TensorSpec {
    let mut dims = [0u64; 8];
    dims[0] = raw.len() as u64;
    TensorSpec {
        name: output_name.to_string(),
        dtype,
        dims,
        rank: 1,
        data: raw.to_vec(),
    }
}

fn validate_arch_only(gguf: &GgufFile, arch: ImportArch) -> Result<(), String> {
    let actual = metadata_string(gguf, "general.architecture")
        .ok_or_else(|| "GGUF is missing general.architecture".to_string())?;
    if actual != arch.metadata_value() {
        return Err(format!(
            "GGUF architecture '{actual}' does not match requested '{}'",
            arch.metadata_value()
        ));
    }
    Ok(())
}

/// Dimensional config read from a Qwen2-family GGUF's metadata. The 0.5B,
/// 1.5B, and 3B differ only in these numbers — the forward graph
/// (RoPE/GQA/SwiGLU/RMSNorm) is identical, and the kernel re-derives the same
/// dims data-driven from tensor shapes (F2). The importer validates that the
/// GGUF's tensors are internally consistent with its declared metadata rather
/// than pinning to one model size.
#[derive(Debug)]
struct Qwen2Dims {
    layers: u64,
    hidden: u64,
    intermediate: u64,
    q_heads: u64,
    kv_heads: u64,
    head_dim: u64,
}

fn require_arch_metadata_u64(gguf: &GgufFile, arch: ImportArch, key: &str) -> Result<u64, String> {
    metadata_u64(gguf, key).ok_or_else(|| {
        format!(
            "{} GGUF is missing metadata key '{key}'",
            arch.display_name()
        )
    })
}

fn require_metadata_u64(gguf: &GgufFile, key: &str) -> Result<u64, String> {
    require_arch_metadata_u64(gguf, ImportArch::Qwen2, key)
}

fn read_qwen2_dims(gguf: &GgufFile) -> Result<Qwen2Dims, String> {
    let layers = require_metadata_u64(gguf, "qwen2.block_count")?;
    let hidden = require_metadata_u64(gguf, "qwen2.embedding_length")?;
    let intermediate = require_metadata_u64(gguf, "qwen2.feed_forward_length")?;
    let q_heads = require_metadata_u64(gguf, "qwen2.attention.head_count")?;
    let kv_heads = require_metadata_u64(gguf, "qwen2.attention.head_count_kv")?;

    if q_heads == 0 || kv_heads == 0 {
        return Err(format!(
            "Qwen2 GGUF declares zero heads (head_count={q_heads}, head_count_kv={kv_heads})"
        ));
    }
    // head_dim: prefer the explicit key, else the standard Qwen2 invariant
    // q_heads * head_dim == embedding_length (0.5B: 896/14=64; 1.5B: 1536/12=128).
    let head_dim = match metadata_u64(gguf, "qwen2.attention.key_length") {
        Some(value) if value > 0 => value,
        _ => {
            if hidden % q_heads != 0 {
                return Err(format!(
                    "Qwen2 GGUF embedding_length {hidden} is not divisible by head_count {q_heads}"
                ));
            }
            hidden / q_heads
        }
    };
    // GQA: the query heads must group evenly onto the KV heads.
    if q_heads % kv_heads != 0 {
        return Err(format!(
            "Qwen2 GGUF head_count {q_heads} is not a multiple of head_count_kv {kv_heads}"
        ));
    }
    Ok(Qwen2Dims {
        layers,
        hidden,
        intermediate,
        q_heads,
        kv_heads,
        head_dim,
    })
}

fn validate_qwen2_graph(gguf: &GgufFile, arch: ImportArch, vocab_size: u64) -> Result<(), String> {
    validate_arch_only(gguf, arch)?;

    let Qwen2Dims {
        layers,
        hidden,
        intermediate,
        q_heads,
        kv_heads,
        head_dim,
    } = read_qwen2_dims(gguf)?;
    assert_metadata_at_least(gguf, "qwen2.context_length", QWEN2_MIN_CONTEXT)?;

    expect_tensor(gguf, "token_embd.weight", &[hidden, vocab_size])?;
    expect_tensor(gguf, "output_norm.weight", &[hidden])?;
    expect_tensor(gguf, "output.weight", &[hidden, vocab_size])?;
    if let Some(output_bias) = gguf.tensor_named("output.bias") {
        expect_dims(output_bias, &[vocab_size])?;
        expect_unquantized_vector(output_bias)?;
    }

    let q_proj = q_heads * head_dim;
    let kv_proj = kv_heads * head_dim;

    for layer in 0..layers {
        let prefix = format!("blk.{layer}");
        expect_tensor(gguf, &format!("{prefix}.attn_norm.weight"), &[hidden])?;
        expect_tensor(gguf, &format!("{prefix}.ffn_norm.weight"), &[hidden])?;

        expect_tensor(gguf, &format!("{prefix}.attn_q.weight"), &[hidden, q_proj])?;
        expect_tensor(gguf, &format!("{prefix}.attn_k.weight"), &[hidden, kv_proj])?;
        expect_tensor(gguf, &format!("{prefix}.attn_v.weight"), &[hidden, kv_proj])?;
        expect_tensor(
            gguf,
            &format!("{prefix}.attn_output.weight"),
            &[q_proj, hidden],
        )?;
        expect_tensor(
            gguf,
            &format!("{prefix}.ffn_gate.weight"),
            &[hidden, intermediate],
        )?;
        expect_tensor(
            gguf,
            &format!("{prefix}.ffn_up.weight"),
            &[hidden, intermediate],
        )?;
        expect_tensor(
            gguf,
            &format!("{prefix}.ffn_down.weight"),
            &[intermediate, hidden],
        )?;

        expect_tensor(gguf, &format!("{prefix}.attn_q.bias"), &[q_proj])?;
        expect_tensor(gguf, &format!("{prefix}.attn_k.bias"), &[kv_proj])?;
        expect_tensor(gguf, &format!("{prefix}.attn_v.bias"), &[kv_proj])?;
    }

    Ok(())
}

#[derive(Debug)]
struct Qwen35Dims {
    all_layers: u64,
    main_layers: u64,
    hidden: u64,
    intermediate: u64,
    q_heads: u64,
    kv_heads: u64,
    head_dim: u64,
    ssm_conv_kernel: u64,
    ssm_inner: u64,
    ssm_state: u64,
    ssm_dt_rank: u64,
    ssm_group_count: u64,
    full_attention_interval: u64,
}

fn read_qwen35_dims(gguf: &GgufFile) -> Result<Qwen35Dims, String> {
    let arch = ImportArch::Qwen35;
    let layers = require_arch_metadata_u64(gguf, arch, "qwen35.block_count")?;
    let nextn = metadata_u64(gguf, "qwen35.nextn_predict_layers").unwrap_or(0);
    if layers == 0 || nextn >= layers {
        return Err(format!(
            "Qwen3.5 GGUF declares invalid layer counts (block_count={layers}, nextn_predict_layers={nextn})"
        ));
    }
    let main_layers = layers - nextn;
    let hidden = require_arch_metadata_u64(gguf, arch, "qwen35.embedding_length")?;
    let intermediate = require_arch_metadata_u64(gguf, arch, "qwen35.feed_forward_length")?;
    let q_heads = require_arch_metadata_u64(gguf, arch, "qwen35.attention.head_count")?;
    let kv_heads = require_arch_metadata_u64(gguf, arch, "qwen35.attention.head_count_kv")?;
    if q_heads == 0 || kv_heads == 0 {
        return Err(format!(
            "Qwen3.5 GGUF declares zero attention heads (head_count={q_heads}, head_count_kv={kv_heads})"
        ));
    }
    let head_dim = match metadata_u64(gguf, "qwen35.attention.key_length") {
        Some(value) if value > 0 => value,
        _ => {
            if hidden % q_heads != 0 {
                return Err(format!(
                    "Qwen3.5 GGUF embedding_length {hidden} is not divisible by head_count {q_heads}"
                ));
            }
            hidden / q_heads
        }
    };
    if q_heads % kv_heads != 0 {
        return Err(format!(
            "Qwen3.5 GGUF head_count {q_heads} is not a multiple of head_count_kv {kv_heads}"
        ));
    }
    let ssm_conv_kernel = require_arch_metadata_u64(gguf, arch, "qwen35.ssm.conv_kernel")?;
    let ssm_inner = require_arch_metadata_u64(gguf, arch, "qwen35.ssm.inner_size")?;
    let ssm_state = require_arch_metadata_u64(gguf, arch, "qwen35.ssm.state_size")?;
    let ssm_dt_rank = require_arch_metadata_u64(gguf, arch, "qwen35.ssm.time_step_rank")?;
    let ssm_group_count = require_arch_metadata_u64(gguf, arch, "qwen35.ssm.group_count")?;
    if ssm_conv_kernel <= 1
        || ssm_state == 0
        || ssm_dt_rank == 0
        || ssm_group_count == 0
        || ssm_dt_rank % ssm_group_count != 0
        || ssm_inner % ssm_dt_rank != 0
    {
        return Err(format!(
            "Qwen3.5 GGUF declares invalid GDN geometry (conv_kernel={ssm_conv_kernel}, inner_size={ssm_inner}, state_size={ssm_state}, time_step_rank={ssm_dt_rank}, group_count={ssm_group_count})"
        ));
    }
    let head_v_dim = ssm_inner / ssm_dt_rank;
    if head_v_dim != ssm_state {
        return Err(format!(
            "Qwen3.5 GGUF GDN head_v_dim {head_v_dim} does not match state_size {ssm_state}"
        ));
    }
    let full_attention_interval = metadata_u64(gguf, "qwen35.full_attention_interval").unwrap_or(4);
    if full_attention_interval == 0 {
        return Err("Qwen3.5 GGUF full_attention_interval must be non-zero".to_string());
    }
    Ok(Qwen35Dims {
        all_layers: layers,
        main_layers,
        hidden,
        intermediate,
        q_heads,
        kv_heads,
        head_dim,
        ssm_conv_kernel,
        ssm_inner,
        ssm_state,
        ssm_dt_rank,
        ssm_group_count,
        full_attention_interval,
    })
}

fn qwen35_schedule_entries(
    gguf: &GgufFile,
    dims: &Qwen35Dims,
) -> Result<Vec<ScheduleEntrySpec>, String> {
    let recurrent = metadata_bool_array(gguf, "qwen35.attention.recurrent_layers");
    if let Some(recurrent) = recurrent.as_ref()
        && recurrent.len() < dims.main_layers as usize
    {
        return Err(format!(
            "Qwen3.5 GGUF qwen35.attention.recurrent_layers has {} entries, expected at least {}",
            recurrent.len(),
            dims.main_layers
        ));
    }

    let mut entries = Vec::with_capacity(dims.main_layers as usize);
    let mut gdn_config_ref = 1u32;
    for layer in 0..dims.main_layers {
        let is_recurrent = recurrent
            .as_ref()
            .map(|values| values[layer as usize])
            .unwrap_or_else(|| (layer + 1) % dims.full_attention_interval != 0);
        if is_recurrent {
            entries.push(ScheduleEntrySpec {
                kind: LayerKind::GatedDeltaNetLinear,
                config_ref: gdn_config_ref,
            });
            gdn_config_ref = gdn_config_ref
                .checked_add(1)
                .ok_or_else(|| "Qwen3.5 GDN config_ref overflowed".to_string())?;
        } else {
            entries.push(ScheduleEntrySpec {
                kind: LayerKind::Qwen2AttentionFFN,
                config_ref: 0,
            });
        }
    }
    Ok(entries)
}

fn validate_qwen35_graph(gguf: &GgufFile, arch: ImportArch, vocab_size: u64) -> Result<(), String> {
    validate_arch_only(gguf, arch)?;

    let dims = read_qwen35_dims(gguf)?;
    assert_metadata_at_least(gguf, "qwen35.context_length", QWEN2_MIN_CONTEXT)?;
    if metadata_f32(gguf, "qwen35.rope.freq_base").is_none() {
        return Err("Qwen3.5 GGUF is missing metadata key 'qwen35.rope.freq_base'".to_string());
    }
    if metadata_f32(gguf, "qwen35.attention.layer_norm_rms_epsilon").is_none() {
        return Err(
            "Qwen3.5 GGUF is missing metadata key 'qwen35.attention.layer_norm_rms_epsilon'"
                .to_string(),
        );
    }

    expect_tensor_for_arch(arch, gguf, "token_embd.weight", &[dims.hidden, vocab_size])?;
    expect_tensor_for_arch(arch, gguf, "output_norm.weight", &[dims.hidden])?;
    if let Some(output) = gguf.tensor_named("output.weight") {
        expect_dims(output, &[dims.hidden, vocab_size])?;
    }

    let schedule = qwen35_schedule_entries(gguf, &dims)?;
    let q_proj = dims.q_heads * dims.head_dim * 2;
    let kv_proj = dims.kv_heads * dims.head_dim;
    let key_dim = dims.ssm_group_count * dims.ssm_state;
    let value_dim = dims.ssm_inner;
    let conv_dim = key_dim
        .checked_mul(2)
        .and_then(|value| value.checked_add(value_dim))
        .ok_or_else(|| "Qwen3.5 GDN conv_dim overflowed".to_string())?;

    for layer in 0..dims.main_layers {
        let prefix = format!("blk.{layer}");
        expect_tensor_for_arch(
            arch,
            gguf,
            &format!("{prefix}.attn_norm.weight"),
            &[dims.hidden],
        )?;
        expect_tensor_for_arch(
            arch,
            gguf,
            &format!("{prefix}.post_attention_norm.weight"),
            &[dims.hidden],
        )?;
        expect_tensor_for_arch(
            arch,
            gguf,
            &format!("{prefix}.ffn_gate.weight"),
            &[dims.hidden, dims.intermediate],
        )?;
        expect_tensor_for_arch(
            arch,
            gguf,
            &format!("{prefix}.ffn_up.weight"),
            &[dims.hidden, dims.intermediate],
        )?;
        expect_tensor_for_arch(
            arch,
            gguf,
            &format!("{prefix}.ffn_down.weight"),
            &[dims.intermediate, dims.hidden],
        )?;

        match schedule[layer as usize].kind {
            LayerKind::Qwen2AttentionFFN => {
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_q.weight"),
                    &[dims.hidden, q_proj],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_k.weight"),
                    &[dims.hidden, kv_proj],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_v.weight"),
                    &[dims.hidden, kv_proj],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_output.weight"),
                    &[dims.q_heads * dims.head_dim, dims.hidden],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_q_norm.weight"),
                    &[dims.head_dim],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_k_norm.weight"),
                    &[dims.head_dim],
                )?;
            }
            LayerKind::GatedDeltaNetLinear => {
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_qkv.weight"),
                    &[dims.hidden, conv_dim],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.attn_gate.weight"),
                    &[dims.hidden, value_dim],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_conv1d.weight"),
                    &[dims.ssm_conv_kernel, conv_dim],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_dt.bias"),
                    &[dims.ssm_dt_rank],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_a"),
                    &[dims.ssm_dt_rank],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_beta.weight"),
                    &[dims.hidden, dims.ssm_dt_rank],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_alpha.weight"),
                    &[dims.hidden, dims.ssm_dt_rank],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_norm.weight"),
                    &[dims.ssm_state],
                )?;
                expect_tensor_for_arch(
                    arch,
                    gguf,
                    &format!("{prefix}.ssm_out.weight"),
                    &[value_dim, dims.hidden],
                )?;
            }
        }
    }

    if dims.all_layers > dims.main_layers {
        // MTP tensors may be preserved in the file, but the imported schedule is
        // the main decoder stack only. This mirrors llama.cpp's main graph.
    }
    Ok(())
}

fn expect_tensor(gguf: &GgufFile, name: &str, expected_dims: &[u64]) -> Result<(), String> {
    expect_tensor_for_arch(ImportArch::Qwen2, gguf, name, expected_dims)
}

fn expect_tensor_for_arch(
    arch: ImportArch,
    gguf: &GgufFile,
    name: &str,
    expected_dims: &[u64],
) -> Result<(), String> {
    let tensor = gguf.tensor_named(name).ok_or_else(|| {
        format!(
            "{} GGUF is missing required tensor '{name}'",
            arch.display_name()
        )
    })?;
    expect_dims(tensor, expected_dims)?;
    if expected_dims.len() == 1 {
        expect_unquantized_vector(tensor)?;
    }
    Ok(())
}

fn expect_dims(tensor: &GgufTensorInfo, expected_dims: &[u64]) -> Result<(), String> {
    if tensor_dims(tensor) != expected_dims {
        return Err(format!(
            "tensor '{}' has dims {:?}, expected {:?}",
            tensor.name,
            tensor_dims(tensor),
            expected_dims
        ));
    }
    Ok(())
}

fn expect_unquantized_vector(tensor: &GgufTensorInfo) -> Result<(), String> {
    match tensor.ty {
        GgmlType::F32 | GgmlType::F16 | GgmlType::Bf16 => Ok(()),
        other => Err(format!(
            "tensor '{}' is a vector but uses quantized/non-scalar type {:?}",
            tensor.name, other
        )),
    }
}

fn assert_metadata_at_least(gguf: &GgufFile, key: &str, minimum: u64) -> Result<(), String> {
    let arch = if key.starts_with("qwen35.") {
        ImportArch::Qwen35
    } else {
        ImportArch::Qwen2
    };
    let actual = metadata_u64(gguf, key).ok_or_else(|| {
        format!(
            "{} GGUF is missing metadata key '{key}'",
            arch.display_name()
        )
    })?;
    if actual < minimum {
        return Err(format!(
            "metadata '{key}' is {actual}, expected at least {minimum}"
        ));
    }
    Ok(())
}

fn metadata_bool_array(gguf: &GgufFile, key: &str) -> Option<Vec<bool>> {
    match gguf.metadata_value(key) {
        Some(GgufValue::Array(GgufArray::Scalars {
            element_type: GgufType::Bool,
            bytes,
            len,
        })) if bytes.len() == *len as usize => {
            Some(bytes.iter().map(|value| *value != 0).collect())
        }
        _ => None,
    }
}

fn metadata_string<'a>(gguf: &'a GgufFile, key: &str) -> Option<&'a str> {
    match gguf.metadata_value(key) {
        Some(GgufValue::String(value)) => Some(value),
        _ => None,
    }
}

fn metadata_u64(gguf: &GgufFile, key: &str) -> Option<u64> {
    match gguf.metadata_value(key) {
        Some(GgufValue::Uint8(value)) => Some(*value as u64),
        Some(GgufValue::Int8(value)) if *value >= 0 => Some(*value as u64),
        Some(GgufValue::Uint16(value)) => Some(*value as u64),
        Some(GgufValue::Int16(value)) if *value >= 0 => Some(*value as u64),
        Some(GgufValue::Uint32(value)) => Some(*value as u64),
        Some(GgufValue::Int32(value)) if *value >= 0 => Some(*value as u64),
        Some(GgufValue::Uint64(value)) => Some(*value),
        Some(GgufValue::Int64(value)) if *value >= 0 => Some(*value as u64),
        _ => None,
    }
}

fn metadata_f32(gguf: &GgufFile, key: &str) -> Option<f32> {
    match gguf.metadata_value(key) {
        Some(GgufValue::Float32(value)) => Some(*value),
        _ => None,
    }
}

fn extract_tokenizer(gguf: &GgufFile) -> Result<TokenizerExport, String> {
    let tokens = metadata_string_array(gguf, "tokenizer.ggml.tokens")
        .ok_or_else(|| "GGUF is missing tokenizer.ggml.tokens".to_string())?;
    let merges = metadata_string_array(gguf, "tokenizer.ggml.merges").unwrap_or_default();

    if tokens.is_empty() {
        return Err("GGUF tokenizer has no tokens".to_string());
    }

    // Exercise the host BPE logic on the extracted state so malformed vocab shape fails here,
    // not later when a runtime loader starts using the sidecar.
    build_bpe_tokenizer(&tokens, &merges)?;

    Ok(TokenizerExport {
        tokens,
        merges,
        bos_token_id: metadata_u64(gguf, "tokenizer.ggml.bos_token_id"),
        eos_token_id: metadata_u64(gguf, "tokenizer.ggml.eos_token_id"),
        padding_token_id: metadata_u64(gguf, "tokenizer.ggml.padding_token_id"),
    })
}

fn metadata_string_array(gguf: &GgufFile, key: &str) -> Option<Vec<String>> {
    match gguf.metadata_value(key) {
        Some(GgufValue::Array(GgufArray::Strings(values))) => {
            Some(values.iter().map(|value| (*value).to_string()).collect())
        }
        _ => None,
    }
}

fn build_bpe_tokenizer(tokens: &[String], merges: &[String]) -> Result<BpeTokenizer, String> {
    let token_bytes: Vec<Vec<u8>> = tokens
        .iter()
        .map(|token| token.as_bytes().to_vec())
        .collect();
    let mut vocab = Vec::with_capacity(tokens.len());
    for (id, token) in token_bytes.iter().enumerate() {
        vocab.push(BpeVocabEntry {
            token: token.as_slice(),
            id: id as u32,
        });
    }

    let merge_bytes: Vec<(Vec<u8>, Vec<u8>)> = merges
        .iter()
        .map(|merge| {
            let (left, right) = merge
                .split_once(' ')
                .ok_or_else(|| format!("tokenizer merge '{merge}' is not a two-token BPE merge"))?;
            Ok((left.as_bytes().to_vec(), right.as_bytes().to_vec()))
        })
        .collect::<Result<_, String>>()?;
    let mut merge_entries = Vec::with_capacity(merges.len());
    for (rank, (left, right)) in merge_bytes.iter().enumerate() {
        merge_entries.push(BpeMergeEntry {
            left: left.as_slice(),
            right: right.as_slice(),
            rank: rank as u32,
        });
    }

    BpeTokenizer::from_slices(&vocab, &merge_entries)
        .map_err(|err| format!("tokenizer vocab/merge validation failed: {err:?}"))
}

fn write_tokenizer_export(path: &Path, tokenizer: &TokenizerExport) -> Result<(), String> {
    ensure_parent_dir(path)?;

    let mut out = String::new();
    out.push_str("NEOM_TOKENIZER\t1\n");
    if let Some(id) = tokenizer.bos_token_id {
        out.push_str(&format!("bos\t{id}\n"));
    }
    if let Some(id) = tokenizer.eos_token_id {
        out.push_str(&format!("eos\t{id}\n"));
    }
    if let Some(id) = tokenizer.padding_token_id {
        out.push_str(&format!("pad\t{id}\n"));
    }
    out.push_str(&format!("tokens\t{}\n", tokenizer.tokens.len()));
    for (id, token) in tokenizer.tokens.iter().enumerate() {
        out.push_str(&format!("{id}\t{}\n", hex_encode(token.as_bytes())));
    }
    out.push_str(&format!("merges\t{}\n", tokenizer.merges.len()));
    for (rank, merge) in tokenizer.merges.iter().enumerate() {
        out.push_str(&format!("{rank}\t{}\n", hex_encode(merge.as_bytes())));
    }

    fs::write(path, out).map_err(|err| {
        format!(
            "failed to write tokenizer sidecar '{}': {err}",
            path.display()
        )
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create '{}': {err}", parent.display()))?;
    }
    Ok(())
}

fn validate_q4_k_layout(tensor: &GgufTensorInfo, raw: &[u8]) -> Result<(), String> {
    validate_qk_layout(tensor, raw, "Q4_K")?;
    validate_quant_block_bytes(tensor, raw, GGML_BLOCK_Q4_K_SIZE, "Q4_K")
}

fn validate_q5_0_layout(tensor: &GgufTensorInfo, raw: &[u8]) -> Result<(), String> {
    let elems = tensor_dims(tensor)
        .iter()
        .copied()
        .try_fold(1u64, |acc, dim| {
            acc.checked_mul(dim)
                .ok_or_else(|| format!("tensor '{}' element count overflowed", tensor.name))
        })?;
    if !elems.is_multiple_of(32) {
        return Err(format!(
            "tensor '{}' Q5_0 element count {elems} is not a multiple of 32",
            tensor.name
        ));
    }
    validate_quant_block_bytes(tensor, raw, GgmlType::Q5_0.type_size(), "Q5_0")
}

fn validate_q8_0_layout(tensor: &GgufTensorInfo, raw: &[u8]) -> Result<(), String> {
    let elems = tensor_dims(tensor)
        .iter()
        .copied()
        .try_fold(1u64, |acc, dim| {
            acc.checked_mul(dim)
                .ok_or_else(|| format!("tensor '{}' element count overflowed", tensor.name))
        })?;
    if !elems.is_multiple_of(32) {
        return Err(format!(
            "tensor '{}' Q8_0 element count {elems} is not a multiple of 32",
            tensor.name
        ));
    }
    validate_quant_block_bytes(tensor, raw, GGML_BLOCK_Q8_0_SIZE, "Q8_0")
}

fn validate_qk_layout(tensor: &GgufTensorInfo, raw: &[u8], type_name: &str) -> Result<(), String> {
    let dims = tensor_dims(tensor);
    if dims.is_empty() || !dims[0].is_multiple_of(GGML_QK_K) {
        return Err(format!(
            "tensor '{}' {type_name} row dimension {:?} is not a multiple of {GGML_QK_K}",
            tensor.name,
            dims.first()
        ));
    }
    if raw.is_empty() {
        return Err(format!(
            "tensor '{}' {type_name} payload is empty",
            tensor.name
        ));
    }
    Ok(())
}

fn validate_quant_block_bytes(
    tensor: &GgufTensorInfo,
    raw: &[u8],
    block_size: u64,
    type_name: &str,
) -> Result<(), String> {
    if !(raw.len() as u64).is_multiple_of(block_size) {
        return Err(format!(
            "tensor '{}' {type_name} payload length {} is not a multiple of block size {block_size}",
            tensor.name,
            raw.len()
        ));
    }
    Ok(())
}

fn tensor_rank(tensor: &GgufTensorInfo) -> usize {
    tensor.n_dims as usize
}

fn tensor_dims(tensor: &GgufTensorInfo) -> &[u64] {
    &tensor.dims[..tensor_rank(tensor)]
}

#[cfg(test)]
pub(crate) fn import_gguf_bytes_for_test(bytes: &[u8]) -> Result<Vec<u8>, String> {
    Ok(import_gguf_bytes(bytes, ImportArch::Qwen2, false)?.neom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{
        CONFIG_ROPE_DIMENSION_COUNT_OFF, HEADER_SIZE, HEADER_SIZE_V4, LAYER_DESCRIPTOR_KIND_OFF,
        LAYER_DESCRIPTOR_RESERVED_OFF, LAYER_DESCRIPTOR_SIZE, SCHEDULE_OFF_OFF, read_f32_le,
        read_u32_le, read_u64_le,
    };

    #[test]
    fn tokenizer_export_builds_bpe_state_from_gguf_vocab() {
        let tokens = vec!["h".to_string(), "i".to_string(), "hi".to_string()];
        let merges = vec!["h i".to_string()];
        let tokenizer = build_bpe_tokenizer(&tokens, &merges).expect("bpe tokenizer");

        let encoded = tokenizer.encode("hi").expect("encode");
        assert_eq!(encoded, vec![2]);
        assert_eq!(tokenizer.decode(&encoded).expect("decode"), "hi");
    }

    #[test]
    fn hand_laid_q4_k_gguf_import_preserves_raw_block_and_offsets() {
        let mut block = vec![0u8; GGML_BLOCK_Q4_K_SIZE as usize];
        for (idx, byte) in block.iter_mut().enumerate() {
            *byte = (idx as u8).wrapping_mul(17).wrapping_add(3);
        }

        let gguf = hand_laid_q4_k_gguf(&block);
        let neom = import_gguf_bytes_for_test(&gguf).expect("import");
        validator::validate(&neom).expect("validate imported neom");

        // v2 header: version=2, header_size=108
        assert_eq!(read_u32_le(&neom, 4), 2);
        assert_eq!(read_u32_le(&neom, 8), HEADER_SIZE);
        assert_eq!(read_u32_le(&neom, 16), 1); // tensor_count

        // Tensor table starts after the 108-byte v2 header, 8-byte aligned.
        let tensor_table_off = 112;
        assert_eq!(read_u64_le(&neom, 32), tensor_table_off);
        // After tensor table: 112 + 96 = 208, already 8-byte aligned.
        // Node table (0 nodes) and IO table (0 entries) take no space.
        assert_eq!(read_u64_le(&neom, 56), 208); // string_off = align_up(112+96, 8)
        assert_eq!(read_u64_le(&neom, 72), 4096); // weight_blob_off
        assert_eq!(read_u64_le(&neom, 88), 4096 + block.len() as u64); // total_size

        // Config metadata at v2 extension offsets (96..108)
        assert_eq!(read_u32_le(&neom, 96), 24); // layer_count
        assert_eq!(read_f32_le(&neom, 100), 10000.0); // rope_theta
        assert!((read_f32_le(&neom, 104) - 1.0e-6).abs() < 1e-9); // rms_norm_eps

        let tensor_base = tensor_table_off as usize;
        assert_eq!(read_u32_le(&neom, tensor_base), Dtype::U8 as u32);
        assert_eq!(read_u32_le(&neom, tensor_base + 4), 1);
        assert_eq!(read_u64_le(&neom, tensor_base + 8), block.len() as u64);
        assert_eq!(read_u64_le(&neom, tensor_base + 72), 4096);
        assert_eq!(read_u64_le(&neom, tensor_base + 80), block.len() as u64);

        let imported_block = &neom[4096..4096 + block.len()];
        assert_eq!(imported_block, block.as_slice());
    }

    #[test]
    fn import_rejects_non_qwen_architecture() {
        let gguf = hand_laid_arch_only_gguf("llama");
        let err = import_gguf_bytes_for_test(&gguf).expect_err("arch mismatch");
        assert!(err.contains("architecture 'llama'"));
    }

    /// ISC-3: Independent byte-oracle for v2 config-metadata extension.
    /// Hand-laid fixture of the 3 fields at exact header offsets with known
    /// LE byte encodings, asserted independent of the writer+reader round-trip.
    #[test]
    fn byte_oracle_v2_config_metadata_extension() {
        // Known values: layer_count=24 (u32), rope_theta=10000.0 (f32),
        // rms_norm_eps=1e-6 (f32). Compute oracle bytes from the constants —
        // never hardcode IEEE-754 bit patterns by hand.
        let layer_count_le: [u8; 4] = 24u32.to_le_bytes();
        assert_eq!(layer_count_le, [0x18, 0x00, 0x00, 0x00]);

        let rope_theta_bits = 10000.0f32.to_bits(); // 0x461C4000
        let rope_theta_le: [u8; 4] = rope_theta_bits.to_le_bytes();
        assert_eq!(rope_theta_le, [0x00, 0x40, 0x1C, 0x46]);

        let rms_norm_eps_bits = (1.0e-6f32).to_bits(); // 0x358637BD
        let rms_norm_eps_le: [u8; 4] = rms_norm_eps_bits.to_le_bytes();
        assert_eq!(rms_norm_eps_le, [0xBD, 0x37, 0x86, 0x35]);

        // Verify the hand-laid_q4_k_gguf import produces these exact bytes at
        // the expected v2 header offsets.
        let block = vec![0u8; GGML_BLOCK_Q4_K_SIZE as usize];
        let gguf = hand_laid_q4_k_gguf(&block);
        let neom = import_gguf_bytes_for_test(&gguf).expect("import");

        // Offsets 96..100: layer_count (u32 LE)
        assert_eq!(&neom[96..100], &layer_count_le);
        // Offsets 100..104: rope_theta (f32 LE)
        assert_eq!(&neom[100..104], &rope_theta_le);
        // Offsets 104..108: rms_norm_eps (f32 LE)
        assert_eq!(&neom[104..108], &rms_norm_eps_le);

        // Round-trip: read back the values and verify they match the originals.
        assert_eq!(read_u32_le(&neom, 96), 24);
        assert_eq!(read_f32_le(&neom, 100), 10000.0);
        assert!((read_f32_le(&neom, 104) - 1.0e-6).abs() < 1e-9);
    }

    #[test]
    fn q5_k_import_preserves_raw_quant_blocks() {
        let block = q5_k_fixture_block();
        let gguf = hand_laid_q5_k_gguf(&block);
        let imported =
            import_gguf_bytes(&gguf, ImportArch::Qwen2, false).expect("import q5_K tensor");
        validator::validate(&imported.neom).expect("validate imported neom");

        assert_eq!(imported.tensor_count, 1);
        assert_eq!(imported.counts.q5_k, 1);
        assert_eq!(imported.weight_bytes, GgmlType::Q5K.type_size() as usize);

        let tensor_base = 112usize;
        assert_eq!(read_u32_le(&imported.neom, tensor_base), Dtype::Q5K as u32);
        assert_eq!(read_u32_le(&imported.neom, tensor_base + 4), 1);
        assert_eq!(
            read_u64_le(&imported.neom, tensor_base + 8),
            GgmlType::Q5K.type_size()
        );
        assert_eq!(
            read_u64_le(&imported.neom, tensor_base + 80),
            GgmlType::Q5K.type_size()
        );

        let weight_off = read_u64_le(&imported.neom, tensor_base + 72) as usize;
        assert_eq!(
            &imported.neom[weight_off..weight_off + block.len()],
            block.as_slice()
        );
    }

    /// ISC-4: Additivity — the fixture GGUF with config metadata produces a v2
    /// .neom that validates; a GGUF missing one of the 3 keys fails import.
    #[test]
    fn import_fails_when_config_metadata_key_missing() {
        // Build a minimal GGUF that has architecture + tokenizer but is missing
        // the config-metadata keys (qwen2.block_count, qwen2.rope.freq_base,
        // qwen2.attention.layer_norm_rms_epsilon).
        let block = vec![0u8; GGML_BLOCK_Q4_K_SIZE as usize];
        let mut bytes = Vec::new();
        push_header(&mut bytes, 1, 3);
        push_metadata_string(&mut bytes, "general.architecture", "qwen2");
        push_metadata_string_array(
            &mut bytes,
            "tokenizer.ggml.tokens",
            &["h".to_string(), "i".to_string(), "hi".to_string()],
        );
        push_metadata_string_array(&mut bytes, "tokenizer.ggml.merges", &["h i".to_string()]);

        push_string(&mut bytes, "tensor.q4_k");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&GGML_QK_K.to_le_bytes());
        bytes.extend_from_slice(&GgmlType::Q4K.as_abi().to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());

        pad_to(&mut bytes, 32);
        bytes.extend_from_slice(&block);
        pad_to(&mut bytes, 32);

        let err = import_gguf_bytes_for_test(&bytes).expect_err("should fail without config keys");
        assert!(err.contains("qwen2.block_count"));
    }

    /// The generalized dimension reader recovers the right config for both the
    /// shipped 0.5B and the new 1.5B from GGUF metadata alone (head_dim derived
    /// via the q_heads * head_dim == embedding_length invariant).
    #[test]
    fn read_qwen2_dims_recovers_05b_and_15b() {
        let g05 = metadata_only_qwen2_gguf(24, 896, 4864, 14, 2);
        let parsed = parse_gguf(&g05).expect("parse 0.5B metadata gguf");
        let d = read_qwen2_dims(&parsed).expect("0.5B dims");
        assert_eq!(
            (
                d.layers,
                d.hidden,
                d.intermediate,
                d.q_heads,
                d.kv_heads,
                d.head_dim
            ),
            (24, 896, 4864, 14, 2, 64)
        );

        let g15 = metadata_only_qwen2_gguf(28, 1536, 8960, 12, 2);
        let parsed = parse_gguf(&g15).expect("parse 1.5B metadata gguf");
        let d = read_qwen2_dims(&parsed).expect("1.5B dims");
        assert_eq!(
            (
                d.layers,
                d.hidden,
                d.intermediate,
                d.q_heads,
                d.kv_heads,
                d.head_dim
            ),
            (28, 1536, 8960, 12, 2, 128)
        );
    }

    /// A GGUF whose embedding_length is not divisible by head_count (and lacks
    /// an explicit key_length) is rejected rather than silently mis-derived.
    #[test]
    fn read_qwen2_dims_rejects_inconsistent_head_geometry() {
        let bad = metadata_only_qwen2_gguf(24, 900, 4864, 14, 2); // 900 % 14 != 0
        let parsed = parse_gguf(&bad).expect("parse");
        let err = read_qwen2_dims(&parsed).expect_err("non-divisible hidden");
        assert!(err.contains("not divisible by head_count"));
    }

    #[test]
    fn read_qwen35_dims_derives_main_layers_and_gdn_geometry() {
        let gguf = metadata_only_qwen35_gguf();
        let parsed = parse_gguf(&gguf).expect("parse qwen35 metadata");
        let d = read_qwen35_dims(&parsed).expect("qwen35 dims");

        assert_eq!(d.all_layers, 4);
        assert_eq!(d.main_layers, 4);
        assert_eq!(d.hidden, 4);
        assert_eq!(d.intermediate, 8);
        assert_eq!(d.q_heads, 2);
        assert_eq!(d.kv_heads, 1);
        assert_eq!(d.head_dim, 2);
        assert_eq!(d.ssm_conv_kernel, 2);
        assert_eq!(d.ssm_inner, 4);
        assert_eq!(d.ssm_state, 2);
        assert_eq!(d.ssm_dt_rank, 2);
        assert_eq!(d.ssm_group_count, 1);
        assert_eq!(d.full_attention_interval, 4);
    }

    #[test]
    fn qwen35_import_emits_v4_schedule_and_rope_geometry() {
        let gguf = hand_laid_qwen35_mixed_schedule_gguf(None);
        let imported = import_gguf_bytes(&gguf, ImportArch::Qwen35, true).expect("qwen35 import");
        validator::validate(&imported.neom).expect("validate qwen35 neom");

        assert_eq!(read_u32_le(&imported.neom, 4), 4);
        assert_eq!(read_u32_le(&imported.neom, 8), HEADER_SIZE_V4);
        assert_eq!(read_u32_le(&imported.neom, 96), 4);
        assert_eq!(read_f32_le(&imported.neom, 100), 1_000_000.0);
        assert!((read_f32_le(&imported.neom, 104) - 1.0e-6).abs() < 1e-9);
        assert_eq!(
            read_u32_le(&imported.neom, CONFIG_ROPE_DIMENSION_COUNT_OFF),
            2
        );

        let schedule_off = read_u64_le(&imported.neom, SCHEDULE_OFF_OFF) as usize;
        let expected = [
            (LayerKind::GatedDeltaNetLinear as u32, 1u32),
            (LayerKind::GatedDeltaNetLinear as u32, 2u32),
            (LayerKind::GatedDeltaNetLinear as u32, 3u32),
            (LayerKind::Qwen2AttentionFFN as u32, 0u32),
        ];
        for (idx, (kind, config_ref)) in expected.iter().copied().enumerate() {
            let base = schedule_off + idx * LAYER_DESCRIPTOR_SIZE as usize;
            assert_eq!(
                read_u32_le(&imported.neom, base + LAYER_DESCRIPTOR_KIND_OFF),
                kind,
                "schedule kind[{idx}]"
            );
            assert_eq!(
                read_u32_le(&imported.neom, base + LAYER_DESCRIPTOR_RESERVED_OFF),
                config_ref,
                "schedule config_ref[{idx}]"
            );
        }
    }

    #[test]
    fn qwen35_import_synthesizes_tied_output_weight_when_absent() {
        let gguf = hand_laid_qwen35_mixed_schedule_gguf(Some("output.weight"));
        let parsed = parse_gguf(&gguf).expect("parse qwen35 fixture");
        let token_embd = parsed
            .tensor_named("token_embd.weight")
            .expect("token embedding tensor");
        let token_bytes = parsed
            .tensor_data(&gguf, token_embd)
            .expect("token embedding bytes")
            .len();

        let imported = import_gguf_bytes(&gguf, ImportArch::Qwen35, true).expect("qwen35 import");
        validator::validate(&imported.neom).expect("validate qwen35 neom");

        assert_eq!(imported.tensor_count, parsed.tensors.len() + 1);
        assert_eq!(imported.counts.f32, parsed.tensors.len() + 1);
        assert!(imported.weight_bytes >= token_bytes * 2);
        assert!(String::from_utf8_lossy(&imported.neom).contains("output.weight"));
    }

    #[test]
    fn qwen35_import_rejects_missing_required_gdn_tensor() {
        let gguf = hand_laid_qwen35_mixed_schedule_gguf(Some("blk.1.ssm_beta.weight"));
        let err = match import_gguf_bytes(&gguf, ImportArch::Qwen35, true) {
            Ok(_) => panic!("missing GDN tensor should fail"),
            Err(err) => err,
        };
        assert!(err.contains("Qwen3.5 GGUF is missing required tensor 'blk.1.ssm_beta.weight'"));
    }

    fn metadata_only_qwen2_gguf(layers: u32, hidden: u32, ffn: u32, q: u32, kv: u32) -> Vec<u8> {
        let block = vec![0u8; GGML_BLOCK_Q4_K_SIZE as usize];
        let mut bytes = Vec::new();
        push_header(&mut bytes, 1, 6);
        push_metadata_string(&mut bytes, "general.architecture", "qwen2");
        push_metadata_u32(&mut bytes, "qwen2.block_count", layers);
        push_metadata_u32(&mut bytes, "qwen2.embedding_length", hidden);
        push_metadata_u32(&mut bytes, "qwen2.feed_forward_length", ffn);
        push_metadata_u32(&mut bytes, "qwen2.attention.head_count", q);
        push_metadata_u32(&mut bytes, "qwen2.attention.head_count_kv", kv);
        push_string(&mut bytes, "tensor.q4_k");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&GGML_QK_K.to_le_bytes());
        bytes.extend_from_slice(&GgmlType::Q4K.as_abi().to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        pad_to(&mut bytes, 32);
        bytes.extend_from_slice(&block);
        pad_to(&mut bytes, 32);
        bytes
    }

    fn metadata_only_qwen35_gguf() -> Vec<u8> {
        let mut bytes = Vec::new();
        push_header(&mut bytes, 1, 18);
        push_qwen35_metadata(&mut bytes);
        push_string(&mut bytes, "tensor.f32");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&GgmlType::F32.as_abi().to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        pad_to(&mut bytes, 32);
        bytes.extend_from_slice(&0.0f32.to_le_bytes());
        pad_to(&mut bytes, 32);
        bytes
    }

    #[derive(Clone)]
    struct TestTensor {
        name: String,
        dims: Vec<u64>,
        data: Vec<u8>,
    }

    fn hand_laid_qwen35_mixed_schedule_gguf(skip_tensor: Option<&str>) -> Vec<u8> {
        let hidden = 4u64;
        let vocab = 4u64;
        let intermediate = 8u64;
        let q_heads = 2u64;
        let kv_heads = 1u64;
        let head_dim = 2u64;
        let ssm_conv_kernel = 2u64;
        let ssm_state = 2u64;
        let ssm_dt_rank = 2u64;
        let ssm_group_count = 1u64;
        let ssm_inner = 4u64;
        let key_dim = ssm_group_count * ssm_state;
        let value_dim = ssm_inner;
        let conv_dim = key_dim * 2 + value_dim;

        let mut tensors = Vec::new();
        maybe_push_f32_tensor(
            &mut tensors,
            skip_tensor,
            "token_embd.weight",
            &[hidden, vocab],
        );
        maybe_push_f32_tensor(&mut tensors, skip_tensor, "output_norm.weight", &[hidden]);
        maybe_push_f32_tensor(&mut tensors, skip_tensor, "output.weight", &[hidden, vocab]);

        for layer in 0..4u64 {
            let prefix = format!("blk.{layer}");
            maybe_push_f32_tensor(
                &mut tensors,
                skip_tensor,
                &format!("{prefix}.attn_norm.weight"),
                &[hidden],
            );
            maybe_push_f32_tensor(
                &mut tensors,
                skip_tensor,
                &format!("{prefix}.post_attention_norm.weight"),
                &[hidden],
            );
            maybe_push_f32_tensor(
                &mut tensors,
                skip_tensor,
                &format!("{prefix}.ffn_gate.weight"),
                &[hidden, intermediate],
            );
            maybe_push_f32_tensor(
                &mut tensors,
                skip_tensor,
                &format!("{prefix}.ffn_up.weight"),
                &[hidden, intermediate],
            );
            maybe_push_f32_tensor(
                &mut tensors,
                skip_tensor,
                &format!("{prefix}.ffn_down.weight"),
                &[intermediate, hidden],
            );

            if layer == 3 {
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_q.weight"),
                    &[hidden, q_heads * head_dim * 2],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_k.weight"),
                    &[hidden, kv_heads * head_dim],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_v.weight"),
                    &[hidden, kv_heads * head_dim],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_output.weight"),
                    &[q_heads * head_dim, hidden],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_q_norm.weight"),
                    &[head_dim],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_k_norm.weight"),
                    &[head_dim],
                );
            } else {
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_qkv.weight"),
                    &[hidden, conv_dim],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.attn_gate.weight"),
                    &[hidden, value_dim],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_conv1d.weight"),
                    &[ssm_conv_kernel, conv_dim],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_dt.bias"),
                    &[ssm_dt_rank],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_a"),
                    &[ssm_dt_rank],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_beta.weight"),
                    &[hidden, ssm_dt_rank],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_alpha.weight"),
                    &[hidden, ssm_dt_rank],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_norm.weight"),
                    &[ssm_state],
                );
                maybe_push_f32_tensor(
                    &mut tensors,
                    skip_tensor,
                    &format!("{prefix}.ssm_out.weight"),
                    &[value_dim, hidden],
                );
            }
        }

        write_f32_gguf(tensors)
    }

    fn maybe_push_f32_tensor(
        tensors: &mut Vec<TestTensor>,
        skip_tensor: Option<&str>,
        name: &str,
        dims: &[u64],
    ) {
        if skip_tensor == Some(name) {
            return;
        }
        let elems = dims.iter().copied().product::<u64>() as usize;
        let mut data = Vec::with_capacity(elems * 4);
        for idx in 0..elems {
            let value = (idx as f32 + 1.0) / 1024.0;
            data.extend_from_slice(&value.to_le_bytes());
        }
        tensors.push(TestTensor {
            name: name.to_string(),
            dims: dims.to_vec(),
            data,
        });
    }

    fn write_f32_gguf(tensors: Vec<TestTensor>) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_header(&mut bytes, tensors.len() as u64, 18);
        push_qwen35_metadata(&mut bytes);

        let mut offset = 0u64;
        let mut offsets = Vec::with_capacity(tensors.len());
        for tensor in &tensors {
            offsets.push(offset);
            offset += align_up_u64_for_test(tensor.data.len() as u64, 32);
        }

        for (tensor, offset) in tensors.iter().zip(offsets.iter().copied()) {
            push_string(&mut bytes, &tensor.name);
            bytes.extend_from_slice(&(tensor.dims.len() as u32).to_le_bytes());
            for dim in &tensor.dims {
                bytes.extend_from_slice(&dim.to_le_bytes());
            }
            bytes.extend_from_slice(&GgmlType::F32.as_abi().to_le_bytes());
            bytes.extend_from_slice(&offset.to_le_bytes());
        }

        pad_to(&mut bytes, 32);
        for tensor in &tensors {
            bytes.extend_from_slice(&tensor.data);
            pad_to(&mut bytes, 32);
        }
        bytes
    }

    fn align_up_u64_for_test(value: u64, alignment: u64) -> u64 {
        let mask = alignment - 1;
        (value + mask) & !mask
    }

    fn push_qwen35_metadata(bytes: &mut Vec<u8>) {
        push_metadata_string(bytes, "general.architecture", "qwen35");
        push_metadata_u32(bytes, "qwen35.block_count", 4);
        push_metadata_u32(bytes, "qwen35.embedding_length", 4);
        push_metadata_u32(bytes, "qwen35.feed_forward_length", 8);
        push_metadata_u32(bytes, "qwen35.attention.head_count", 2);
        push_metadata_u32(bytes, "qwen35.attention.head_count_kv", 1);
        push_metadata_u32(bytes, "qwen35.attention.key_length", 2);
        push_metadata_u32(bytes, "qwen35.context_length", QWEN2_MIN_CONTEXT as u32);
        push_metadata_f32(bytes, "qwen35.rope.freq_base", 1_000_000.0);
        push_metadata_u32(bytes, "qwen35.rope.dimension_count", 2);
        push_metadata_f32(bytes, "qwen35.attention.layer_norm_rms_epsilon", 1.0e-6);
        push_metadata_u32(bytes, "qwen35.ssm.conv_kernel", 2);
        push_metadata_u32(bytes, "qwen35.ssm.inner_size", 4);
        push_metadata_u32(bytes, "qwen35.ssm.state_size", 2);
        push_metadata_u32(bytes, "qwen35.ssm.time_step_rank", 2);
        push_metadata_u32(bytes, "qwen35.ssm.group_count", 1);
        push_metadata_u32(bytes, "qwen35.full_attention_interval", 4);
        push_metadata_string_array(
            bytes,
            "tokenizer.ggml.tokens",
            &[
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ],
        );
    }

    fn hand_laid_q4_k_gguf(block: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_header(&mut bytes, 1, 6);
        push_metadata_string(&mut bytes, "general.architecture", "qwen2");
        push_metadata_u32(&mut bytes, "qwen2.block_count", 24);
        push_metadata_f32(&mut bytes, "qwen2.rope.freq_base", 10000.0);
        push_metadata_f32(&mut bytes, "qwen2.attention.layer_norm_rms_epsilon", 1.0e-6);
        push_metadata_string_array(
            &mut bytes,
            "tokenizer.ggml.tokens",
            &["h".to_string(), "i".to_string(), "hi".to_string()],
        );
        push_metadata_string_array(&mut bytes, "tokenizer.ggml.merges", &["h i".to_string()]);

        push_string(&mut bytes, "tensor.q4_k");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&GGML_QK_K.to_le_bytes());
        bytes.extend_from_slice(&GgmlType::Q4K.as_abi().to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());

        pad_to(&mut bytes, 32);
        bytes.extend_from_slice(block);
        pad_to(&mut bytes, 32);
        bytes
    }

    fn hand_laid_q5_k_gguf(block: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_header(&mut bytes, 1, 6);
        push_metadata_string(&mut bytes, "general.architecture", "qwen2");
        push_metadata_u32(&mut bytes, "qwen2.block_count", 24);
        push_metadata_f32(&mut bytes, "qwen2.rope.freq_base", 10000.0);
        push_metadata_f32(&mut bytes, "qwen2.attention.layer_norm_rms_epsilon", 1.0e-6);
        push_metadata_string_array(
            &mut bytes,
            "tokenizer.ggml.tokens",
            &["h".to_string(), "i".to_string(), "hi".to_string()],
        );
        push_metadata_string_array(&mut bytes, "tokenizer.ggml.merges", &["h i".to_string()]);

        push_string(&mut bytes, "tensor.q5_K");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&GGML_QK_K.to_le_bytes());
        bytes.extend_from_slice(&GgmlType::Q5K.as_abi().to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());

        pad_to(&mut bytes, 32);
        bytes.extend_from_slice(block);
        pad_to(&mut bytes, 32);
        bytes
    }

    fn q5_k_fixture_block() -> Vec<u8> {
        let mut block = vec![0u8; GgmlType::Q5K.type_size() as usize];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[2..4].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[4..16].copy_from_slice(&[2, 3, 4, 5, 1, 2, 3, 4, 0x46, 0x57, 0x68, 0x79]);
        for idx in 0..32 {
            block[16 + idx] = match idx % 4 {
                0 => 0b0101_0101,
                1 => 0b1010_1010,
                2 => 0xff,
                _ => 0x00,
            };
        }
        for idx in 0..128 {
            block[48 + idx] = ((((idx + 10) & 0x0f) << 4) | ((idx + 5) & 0x0f)) as u8;
        }
        block
    }

    fn hand_laid_arch_only_gguf(arch: &str) -> Vec<u8> {
        let block = vec![0u8; GGML_BLOCK_Q4_K_SIZE as usize];
        let mut bytes = Vec::new();
        push_header(&mut bytes, 1, 6);
        push_metadata_string(&mut bytes, "general.architecture", arch);
        push_metadata_u32(&mut bytes, "qwen2.block_count", 24);
        push_metadata_f32(&mut bytes, "qwen2.rope.freq_base", 10000.0);
        push_metadata_f32(&mut bytes, "qwen2.attention.layer_norm_rms_epsilon", 1.0e-6);
        push_metadata_string_array(
            &mut bytes,
            "tokenizer.ggml.tokens",
            &["h".to_string(), "i".to_string(), "hi".to_string()],
        );
        push_metadata_string_array(&mut bytes, "tokenizer.ggml.merges", &["h i".to_string()]);
        push_string(&mut bytes, "tensor.q4_k");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&GGML_QK_K.to_le_bytes());
        bytes.extend_from_slice(&GgmlType::Q4K.as_abi().to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        pad_to(&mut bytes, 32);
        bytes.extend_from_slice(&block);
        pad_to(&mut bytes, 32);
        bytes
    }

    fn push_header(bytes: &mut Vec<u8>, tensors: u64, metadata: u64) {
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&tensors.to_le_bytes());
        bytes.extend_from_slice(&metadata.to_le_bytes());
    }

    fn push_metadata_string(bytes: &mut Vec<u8>, key: &str, value: &str) {
        push_string(bytes, key);
        bytes.extend_from_slice(&8u32.to_le_bytes());
        push_string(bytes, value);
    }

    fn push_metadata_u32(bytes: &mut Vec<u8>, key: &str, value: u32) {
        push_string(bytes, key);
        bytes.extend_from_slice(&4i32.to_le_bytes()); // GGUF type Uint32 = 4
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_metadata_f32(bytes: &mut Vec<u8>, key: &str, value: f32) {
        push_string(bytes, key);
        bytes.extend_from_slice(&6i32.to_le_bytes()); // GGUF type Float32 = 6
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }

    fn push_metadata_string_array(bytes: &mut Vec<u8>, key: &str, values: &[String]) {
        push_string(bytes, key);
        bytes.extend_from_slice(&9u32.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&(values.len() as u64).to_le_bytes());
        for value in values {
            push_string(bytes, value);
        }
    }

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }

    fn pad_to(bytes: &mut Vec<u8>, alignment: usize) {
        let pad = (alignment - (bytes.len() % alignment)) % alignment;
        bytes.resize(bytes.len() + pad, 0);
    }
}
