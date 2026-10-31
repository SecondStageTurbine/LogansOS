// SPDX-License-Identifier: MPL-2.0
//! Pure .neom parser and graph validator.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

pub const NEOM_MAGIC: [u8; 4] = *b"NEOM";
pub const NEOM_VERSION_V1: u32 = 1;
pub const NEOM_VERSION: u32 = 2;
pub const NEOM_HEADER_SIZE_V1: usize = 96;
pub const NEOM_HEADER_SIZE: usize = 108;
pub const NEOM_CONFIG_LAYER_COUNT_OFF: usize = 96;
pub const NEOM_CONFIG_ROPE_THETA_OFF: usize = 100;
pub const NEOM_CONFIG_RMS_NORM_EPS_OFF: usize = 104;
// v3 (OT-16 B1): layer-kind schedule section. Additive over v2 (124-byte header).
pub const NEOM_VERSION_V3: u32 = 3;
pub const NEOM_HEADER_SIZE_V3: usize = 124;
// v4 (OT-16 B3): persists the per-head RoPE dimension count.
pub const NEOM_VERSION_V4: u32 = 4;
pub const NEOM_HEADER_SIZE_V4: usize = 128;
pub const NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF: usize = 124;
pub const NEOM_SCHEDULE_LAYER_COUNT_OFF: usize = 108;
pub const NEOM_SCHEDULE_VERSION_OFF: usize = 112;
pub const NEOM_SCHEDULE_OFF_OFF: usize = 116;
pub const NEOM_SCHEDULE_VERSION: u32 = 1;
pub const NEOM_LAYER_DESCRIPTOR_SIZE: usize = 8;
pub const NEOM_LAYER_DESCRIPTOR_KIND_OFF: usize = 0;
pub const NEOM_LAYER_DESCRIPTOR_RESERVED_OFF: usize = 4;
pub const NEOM_SCHEDULE_MAX_LAYERS: u32 = 128;
pub const NEOM_TENSOR_ENTRY_SIZE: usize = 96;
pub const NEOM_NODE_ENTRY_SIZE: usize = 48;
pub const NEOM_IO_ENTRY_SIZE: usize = 80;
pub const NEOM_TABLE_ALIGNMENT: u64 = 8;
pub const NEOM_WEIGHT_ALIGNMENT: u64 = 4096;
pub const NEOM_WEIGHT_ENTRY_ALIGNMENT: u64 = 64;
pub const NEOM_MAX_FILE_SIZE: usize = 2 * 1024 * 1024 * 1024;
pub const NEOM_MAX_TENSORS: usize = 4096;
pub const NEOM_MAX_NODES: usize = 4096;
pub const NEOM_MAX_IO: usize = 4096;
pub const NEOM_MAX_REFS_PER_NODE: usize = 64;
pub const NEOM_MAX_VALUES: usize = NEOM_MAX_TENSORS + NEOM_MAX_IO + NEOM_MAX_NODES * 4;
pub const MAX_RANK: usize = 8;
pub const QWEN2_DEFAULT_CONTEXT: usize = 4096;
const GGML_Q4_K_BLOCK_VALUES: u64 = 256;
const GGML_Q4_K_BLOCK_BYTES: u64 = 144;
const GGML_Q5_0_BLOCK_VALUES: u64 = 32;
const GGML_Q5_0_BLOCK_BYTES: u64 = 22;
const GGML_Q6_K_BLOCK_VALUES: u64 = 256;
const GGML_Q6_K_BLOCK_BYTES: u64 = 210;
const GGML_Q8_0_BLOCK_VALUES: u64 = 32;
const GGML_Q8_0_BLOCK_BYTES: u64 = 34;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ModelFormatError {
    BadMagic,
    BadVersion,
    InvalidHeader,
    InvalidBounds,
    InvalidTensor,
    InvalidNode,
    InvalidIo,
    InvalidGraph,
    InvalidConfig,
    UnsupportedOp,
    /// v3 schedule: `schedule.version` is outside the supported range (invariant 1).
    UnsupportedScheduleVersion,
    /// v3 schedule: a LayerDescriptor `kind` value is not a known `LayerKind`
    /// (invariant 3, known-kinds-only).
    UnknownLayerKind,
    /// v3 schedule: a declared-but-unimplemented kind (B1-only; B2 accepts
    /// GatedDeltaNetLinear at load. Kept for backward compat with v3 files that
    /// may carry this error from older tooling).
    #[allow(dead_code)]
    LayerKindUnimplemented,
    /// v3 schedule: layer_count 0 or > MAX_LAYERS, mismatch vs the model config,
    /// a non-zero reserved field, or the table extends past the file (invariants
    /// 2, 4, 5, 7).
    InvalidSchedule,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NeomDtype {
    F32,
    F16,
    BF16,
    I32,
    I8,
    U8,
    Bool,
    /// Raw GGML `block_q5_K` bytes. The tensor shape remains the opaque byte
    /// payload shape; this tag disambiguates Q5_K from equal-density Q5_0.
    Q5K,
}

impl NeomDtype {
    pub const fn from_abi(value: u32) -> Result<Self, ModelFormatError> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::BF16),
            3 => Ok(Self::I32),
            4 => Ok(Self::I8),
            5 => Ok(Self::U8),
            6 => Ok(Self::Bool),
            7 => Ok(Self::Q5K),
            _ => Err(ModelFormatError::InvalidTensor),
        }
    }

    pub const fn as_abi(self) -> u32 {
        match self {
            Self::F32 => 0,
            Self::F16 => 1,
            Self::BF16 => 2,
            Self::I32 => 3,
            Self::I8 => 4,
            Self::U8 => 5,
            Self::Bool => 6,
            Self::Q5K => 7,
        }
    }

    pub const fn size_bytes(self) -> u64 {
        match self {
            Self::F32 | Self::I32 => 4,
            Self::F16 | Self::BF16 => 2,
            Self::I8 | Self::U8 | Self::Bool | Self::Q5K => 1,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ShapeMeta {
    pub rank: u32,
    pub dims: [u64; MAX_RANK],
}

impl ShapeMeta {
    fn parse(rank: u32, dims: [u64; MAX_RANK]) -> Result<Self, ModelFormatError> {
        if rank as usize > MAX_RANK {
            return Err(ModelFormatError::InvalidTensor);
        }
        let rank_usize = rank as usize;
        let mut idx = 0usize;
        while idx < rank_usize {
            if dims[idx] == 0 {
                return Err(ModelFormatError::InvalidTensor);
            }
            idx += 1;
        }
        while idx < MAX_RANK {
            if dims[idx] != 0 {
                return Err(ModelFormatError::InvalidTensor);
            }
            idx += 1;
        }
        Ok(Self { rank, dims })
    }

    fn byte_len(self, dtype: NeomDtype) -> Result<u64, ModelFormatError> {
        let mut elements = 1u64;
        let mut idx = 0usize;
        while idx < self.rank as usize {
            elements = elements
                .checked_mul(self.dims[idx])
                .ok_or(ModelFormatError::InvalidTensor)?;
            idx += 1;
        }
        let bytes = elements
            .checked_mul(dtype.size_bytes())
            .ok_or(ModelFormatError::InvalidTensor)?;
        if bytes == 0 {
            return Err(ModelFormatError::InvalidTensor);
        }
        Ok(bytes)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Header {
    pub version: u32,
    pub header_size: u32,
    pub tensor_count: u32,
    pub node_count: u32,
    pub input_count: u32,
    pub output_count: u32,
    pub tensor_table_off: u64,
    pub node_table_off: u64,
    pub io_table_off: u64,
    pub string_off: u64,
    pub string_len: u64,
    pub weight_blob_off: u64,
    pub weight_blob_len: u64,
    pub total_size: u64,
    pub config: Option<HeaderConfig>,
    /// v3-only: the layer-kind schedule descriptor. `None` for v1/v2 (the walker
    /// synthesizes a homogeneous Qwen2 schedule for those).
    pub schedule: Option<ScheduleRef>,
}

/// The v3 schedule-section descriptor read from the header (offsets 108..124).
/// The LayerDescriptor table it points at is parsed + validated by
/// `parse_schedule` (the 7 load-time invariants).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ScheduleRef {
    pub version: u32,
    pub layer_count: u32,
    pub off: u64,
}

/// A validated layer-kind for the data-driven forward-graph schedule (OT-16).
/// B1 implemented only `Qwen2AttentionFFN`; B2 accepts `GatedDeltaNetLinear`
/// at load, and B2b executes it through the shared walker dispatch.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum LayerKind {
    Qwen2AttentionFFN,
    GatedDeltaNetLinear,
}

impl LayerKind {
    pub const fn from_abi(value: u32) -> Result<Self, ModelFormatError> {
        match value {
            0 => Ok(Self::Qwen2AttentionFFN),
            1 => Ok(Self::GatedDeltaNetLinear),
            _ => Err(ModelFormatError::UnknownLayerKind),
        }
    }
}

/// The validated layer-kind schedule (an ordered list, one kind per layer). A
/// typed table the walker walks sequentially — not a graph with control flow.
/// B2 adds `config_ref` for GDN layers (the per-layer config-ref from the
/// LayerDescriptor `reserved` field, ADR-072 Decision 2).
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct LayerSchedule {
    pub version: u32,
    pub layers: Vec<LayerEntry>,
}

/// A single entry in the layer-kind schedule. B2 extends this with the
/// per-layer config-ref for GDN layers.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct LayerEntry {
    pub kind: LayerKind,
    /// B2: for `GatedDeltaNetLinear`, indexes into the GDN config table.
    /// For `Qwen2AttentionFFN`, must be 0 (global config suffices).
    pub config_ref: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct HeaderConfig {
    pub layer_count: u32,
    pub rope_theta_bits: u32,
    pub rms_norm_eps_bits: u32,
    /// v4-only partial-RoPE geometry. v1/v2/v3 derive full-head rotation for
    /// backward compatibility with existing Qwen2 artifacts.
    pub rope_dimension_count: Option<u32>,
}

impl HeaderConfig {
    pub fn rope_theta(self) -> f32 {
        f32::from_bits(self.rope_theta_bits)
    }

    pub fn rms_norm_eps(self) -> f32 {
        f32::from_bits(self.rms_norm_eps_bits)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ModelConfig {
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub layer_count: usize,
    pub q_heads: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
    pub rope_dimension_count: usize,
    pub q_projection_size: usize,
    pub context: usize,
    pub vocab: usize,
    pub rope_theta: f32,
    pub rms_norm_eps: f32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TensorMeta {
    pub dtype: NeomDtype,
    pub shape: ShapeMeta,
    pub weight_off: u64,
    pub weight_len: u64,
    pub name_off: u32,
    pub name_len: u32,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct NodeMeta {
    pub op: u32,
    pub inputs: Vec<u32>,
    pub outputs: Vec<u32>,
    pub attr_off: u32,
    pub attr_len: u32,
    pub name_off: u32,
    pub name_len: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct IoMeta {
    pub value_index: u32,
    pub dtype: NeomDtype,
    pub shape: ShapeMeta,
    pub is_output: bool,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ValidatedNeom {
    pub header: Header,
    pub tensors: Vec<TensorMeta>,
    pub nodes: Vec<NodeMeta>,
    pub ios: Vec<IoMeta>,
    pub value_count: u32,
}

#[derive(Copy, Clone)]
struct Range {
    start: usize,
    end: usize,
}

impl Range {
    fn new(start: u64, len: u64, total: usize) -> Result<Self, ModelFormatError> {
        let start = usize::try_from(start).map_err(|_| ModelFormatError::InvalidBounds)?;
        let len = usize::try_from(len).map_err(|_| ModelFormatError::InvalidBounds)?;
        let end = start
            .checked_add(len)
            .ok_or(ModelFormatError::InvalidBounds)?;
        if end > total {
            return Err(ModelFormatError::InvalidBounds);
        }
        Ok(Self { start, end })
    }

    const fn len(self) -> usize {
        self.end - self.start
    }

    const fn contains_region(self, off: usize, len: usize) -> bool {
        match off.checked_add(len) {
            Some(end) => off >= self.start && end <= self.end,
            None => false,
        }
    }

    const fn disjoint(self, other: Self) -> bool {
        self.end <= other.start || other.end <= self.start
    }
}

pub fn validate_neom(bytes: &[u8]) -> Result<ValidatedNeom, ModelFormatError> {
    let header = parse_header(bytes)?;
    // v3 (OT-16): reject a malformed/hostile layer-kind schedule at load, before
    // any forward executes. The synthesized v1/v2 path needs no load check.
    if header.schedule.is_some() {
        let layer_count = header
            .config
            .map(|c| c.layer_count as usize)
            .ok_or(ModelFormatError::InvalidSchedule)?;
        parse_schedule(bytes, &header, layer_count)?;
    }
    let total = checked_usize(header.total_size)?;
    let tensor_count = checked_usize(header.tensor_count as u64)?;
    let node_count = checked_usize(header.node_count as u64)?;
    let input_count = checked_usize(header.input_count as u64)?;
    let output_count = checked_usize(header.output_count as u64)?;
    let io_count = input_count
        .checked_add(output_count)
        .ok_or(ModelFormatError::InvalidHeader)?;
    if tensor_count > NEOM_MAX_TENSORS
        || node_count > NEOM_MAX_NODES
        || io_count > NEOM_MAX_IO
        || total > NEOM_MAX_FILE_SIZE
    {
        return Err(ModelFormatError::InvalidHeader);
    }

    let tensor_table_len = checked_mul(tensor_count, NEOM_TENSOR_ENTRY_SIZE)?;
    let io_table_len = checked_mul(io_count, NEOM_IO_ENTRY_SIZE)?;
    let tensor_range = Range::new(header.tensor_table_off, tensor_table_len as u64, total)?;
    let io_range = Range::new(header.io_table_off, io_table_len as u64, total)?;
    let string_range = Range::new(header.string_off, header.string_len, total)?;
    let weight_range = Range::new(header.weight_blob_off, header.weight_blob_len, total)?;
    let node_start = checked_usize(header.node_table_off)?;
    let node_end = checked_usize(header.io_table_off)?;
    if node_start > node_end || node_end > total {
        return Err(ModelFormatError::InvalidBounds);
    }
    let node_range = Range {
        start: node_start,
        end: node_end,
    };
    let node_entries_len = checked_mul(node_count, NEOM_NODE_ENTRY_SIZE)?;
    if node_entries_len > node_range.len() {
        return Err(ModelFormatError::InvalidBounds);
    }
    if header.tensor_table_off < header.header_size as u64
        || header.node_table_off < header.header_size as u64
        || header.io_table_off < header.header_size as u64
        || header.string_off < header.header_size as u64
    {
        return Err(ModelFormatError::InvalidBounds);
    }
    if header.tensor_table_off % NEOM_TABLE_ALIGNMENT != 0
        || header.node_table_off % NEOM_TABLE_ALIGNMENT != 0
        || header.io_table_off % NEOM_TABLE_ALIGNMENT != 0
        || header.string_off % NEOM_TABLE_ALIGNMENT != 0
        || header.weight_blob_off % NEOM_WEIGHT_ALIGNMENT != 0
    {
        return Err(ModelFormatError::InvalidBounds);
    }
    let mut ranges = vec![
        tensor_range,
        node_range,
        io_range,
        string_range,
        weight_range,
    ];
    // v3 (OT-16): the layer-kind schedule table must also be disjoint from every
    // other region (B1c — an AAG completeness fold-in; benign in B1 since the
    // reads are bounds-checked and only all-zero descriptors survive, but a live
    // requirement once `reserved` becomes a B2 config-ref).
    if let Some(sched) = header.schedule {
        let table_len = checked_mul(
            checked_usize(sched.layer_count as u64)?,
            NEOM_LAYER_DESCRIPTOR_SIZE,
        )?;
        ranges.push(Range::new(sched.off, table_len as u64, total)?);
    }
    validate_disjoint(&ranges)?;

    let tensors = parse_tensors(bytes, header, tensor_range, string_range, weight_range)?;
    let nodes = parse_nodes(bytes, node_count, node_range, string_range)?;
    let ios = parse_ios(bytes, input_count, output_count, io_range)?;
    let value_count = validate_graph(&tensors, &nodes, &ios, input_count)?;
    Ok(ValidatedNeom {
        header,
        tensors,
        nodes,
        ios,
        value_count,
    })
}

pub fn derive_model_config(
    bytes: &[u8],
    model: &ValidatedNeom,
) -> Result<Option<ModelConfig>, ModelFormatError> {
    let Some(header_config) = model.header.config else {
        return Ok(None);
    };

    let dims = match derive_qwen_config_dims_from_rank2(bytes, model) {
        Ok(dims) => dims,
        Err(_) => match derive_qwen35_config_dims(bytes, model) {
            Ok(dims) => dims,
            Err(_) => derive_qwen_config_dims_from_raw_quant(bytes, model)?,
        },
    };
    let (q_heads, kv_heads, head_dim) = if let (Some(q_heads), Some(kv_heads), Some(head_dim)) =
        (dims.q_heads, dims.kv_heads, dims.head_dim)
    {
        if q_heads == 0
            || kv_heads == 0
            || head_dim == 0
            || q_heads
                .checked_mul(head_dim)
                .ok_or(ModelFormatError::InvalidConfig)?
                != dims.q_projection
            || kv_heads
                .checked_mul(head_dim)
                .ok_or(ModelFormatError::InvalidConfig)?
                != dims.kv_projection
        {
            return Err(ModelFormatError::InvalidConfig);
        }
        (q_heads, kv_heads, head_dim)
    } else {
        infer_qwen2_heads(dims.q_projection, dims.kv_projection, dims.hidden_size)?
    };
    let rope_theta = header_config.rope_theta();
    let rms_norm_eps = header_config.rms_norm_eps();
    let rope_dimension_count = match header_config.rope_dimension_count {
        Some(value) => checked_config_usize(value as u64)?,
        None => head_dim,
    };
    if header_config.layer_count == 0
        || dims.hidden_size == 0
        || dims.intermediate_size == 0
        || dims.vocab == 0
        || !rope_theta.is_finite()
        || rope_theta <= 0.0
        || !rms_norm_eps.is_finite()
        || rms_norm_eps <= 0.0
        || rope_dimension_count == 0
        || rope_dimension_count > head_dim
        || !rope_dimension_count.is_multiple_of(2)
    {
        return Err(ModelFormatError::InvalidConfig);
    }

    Ok(Some(ModelConfig {
        hidden_size: dims.hidden_size,
        intermediate_size: dims.intermediate_size,
        layer_count: checked_config_usize(header_config.layer_count as u64)?,
        q_heads,
        kv_heads,
        head_dim,
        rope_dimension_count,
        q_projection_size: dims.q_projection_size,
        context: QWEN2_DEFAULT_CONTEXT,
        vocab: dims.vocab,
        rope_theta,
        rms_norm_eps,
    }))
}

struct QwenConfigDims {
    hidden_size: usize,
    intermediate_size: usize,
    vocab: usize,
    q_projection: usize,
    kv_projection: usize,
    q_projection_size: usize,
    q_heads: Option<usize>,
    kv_heads: Option<usize>,
    head_dim: Option<usize>,
}

fn derive_qwen_config_dims_from_rank2(
    bytes: &[u8],
    model: &ValidatedNeom,
) -> Result<QwenConfigDims, ModelFormatError> {
    let token_embd = rank2_tensor(bytes, model, "token_embd.weight")?;
    let hidden_size = checked_config_usize(token_embd[0])?;
    let vocab = checked_config_usize(token_embd[1])?;

    let attn_q = rank2_tensor(bytes, model, "blk.0.attn_q.weight")?;
    let attn_k = rank2_tensor(bytes, model, "blk.0.attn_k.weight")?;
    let ffn_gate = rank2_tensor(bytes, model, "blk.0.ffn_gate.weight")?;
    let ffn_up = rank2_tensor(bytes, model, "blk.0.ffn_up.weight")?;
    let ffn_down = rank2_tensor(bytes, model, "blk.0.ffn_down.weight")?;

    if checked_config_usize(attn_q[0])? != hidden_size
        || checked_config_usize(attn_k[0])? != hidden_size
        || checked_config_usize(ffn_gate[0])? != hidden_size
        || checked_config_usize(ffn_up[0])? != hidden_size
    {
        return Err(ModelFormatError::InvalidConfig);
    }
    let q_projection = checked_config_usize(attn_q[1])?;
    let kv_projection = checked_config_usize(attn_k[1])?;
    let intermediate_size = checked_config_usize(ffn_gate[1])?;
    if checked_config_usize(ffn_up[1])? != intermediate_size
        || checked_config_usize(ffn_down[0])? != intermediate_size
        || checked_config_usize(ffn_down[1])? != hidden_size
    {
        return Err(ModelFormatError::InvalidConfig);
    }

    Ok(QwenConfigDims {
        hidden_size,
        intermediate_size,
        vocab,
        q_projection,
        kv_projection,
        q_projection_size: q_projection,
        q_heads: None,
        kv_heads: None,
        head_dim: None,
    })
}

fn derive_qwen35_config_dims(
    bytes: &[u8],
    model: &ValidatedNeom,
) -> Result<QwenConfigDims, ModelFormatError> {
    let output_norm = rank1_tensor(bytes, model, "output_norm.weight")?;
    let hidden_size = checked_config_usize(output_norm[0])?;
    let token_embd = tensor_element_candidates(bytes, model, "token_embd.weight")?;
    let vocab = first_factor(&token_embd, hidden_size, hidden_size, usize::MAX)
        .ok_or(ModelFormatError::InvalidConfig)?;

    let ffn_gate = tensor_element_candidates(bytes, model, "blk.0.ffn_gate.weight")?;
    let ffn_up = tensor_element_candidates(bytes, model, "blk.0.ffn_up.weight")?;
    let ffn_down = tensor_element_candidates(bytes, model, "blk.0.ffn_down.weight")?;
    let intermediate_size = first_factor(
        &ffn_gate,
        hidden_size,
        hidden_size.saturating_add(1),
        usize::MAX,
    )
    .ok_or(ModelFormatError::InvalidConfig)?;
    if !contains_product(&ffn_up, hidden_size, intermediate_size)
        || !contains_product(&ffn_down, hidden_size, intermediate_size)
    {
        return Err(ModelFormatError::InvalidConfig);
    }

    let attn_q = first_tensor_element_candidates_with_suffix(bytes, model, ".attn_q.weight")?;
    let attn_k = first_tensor_element_candidates_with_suffix(bytes, model, ".attn_k.weight")?;
    let attn_q_norm = first_rank1_tensor_with_suffix(bytes, model, ".attn_q_norm.weight")?;
    let head_dim = checked_config_usize(attn_q_norm[0])?;
    let q_projection_full = first_factor(&attn_q, hidden_size, hidden_size, usize::MAX)
        .ok_or(ModelFormatError::InvalidConfig)?;
    if q_projection_full % 2 != 0 {
        return Err(ModelFormatError::InvalidConfig);
    }
    let q_projection = q_projection_full / 2;
    let kv_projection = first_factor(&attn_k, hidden_size, 1, q_projection)
        .ok_or(ModelFormatError::InvalidConfig)?;
    if !q_projection.is_multiple_of(head_dim) || !kv_projection.is_multiple_of(head_dim) {
        return Err(ModelFormatError::InvalidConfig);
    }

    Ok(QwenConfigDims {
        hidden_size,
        intermediate_size,
        vocab,
        q_projection,
        kv_projection,
        q_projection_size: q_projection_full,
        q_heads: Some(q_projection / head_dim),
        kv_heads: Some(kv_projection / head_dim),
        head_dim: Some(head_dim),
    })
}

fn derive_qwen_config_dims_from_raw_quant(
    bytes: &[u8],
    model: &ValidatedNeom,
) -> Result<QwenConfigDims, ModelFormatError> {
    let token_embd = raw_quant_tensor_element_candidates(bytes, model, "token_embd.weight")?;
    let attn_q = raw_quant_tensor_element_candidates(bytes, model, "blk.0.attn_q.weight")?;
    let attn_k = raw_quant_tensor_element_candidates(bytes, model, "blk.0.attn_k.weight")?;
    let ffn_gate = raw_quant_tensor_element_candidates(bytes, model, "blk.0.ffn_gate.weight")?;
    let ffn_up = raw_quant_tensor_element_candidates(bytes, model, "blk.0.ffn_up.weight")?;
    let ffn_down = raw_quant_tensor_element_candidates(bytes, model, "blk.0.ffn_down.weight")?;

    let mut idx = 0usize;
    while idx < attn_q.len {
        let Some(hidden_size) = square_root_if_square(attn_q.values[idx]) else {
            idx += 1;
            continue;
        };
        let Some(kv_projection) = first_factor(&attn_k, hidden_size, 1, hidden_size) else {
            idx += 1;
            continue;
        };
        let Some(intermediate_size) = first_factor(
            &ffn_gate,
            hidden_size,
            hidden_size.saturating_add(1),
            usize::MAX,
        ) else {
            idx += 1;
            continue;
        };
        if !contains_product(&ffn_up, hidden_size, intermediate_size)
            || !contains_product(&ffn_down, hidden_size, intermediate_size)
        {
            idx += 1;
            continue;
        }
        let Some(vocab) = first_factor(&token_embd, hidden_size, hidden_size, usize::MAX) else {
            idx += 1;
            continue;
        };
        return Ok(QwenConfigDims {
            hidden_size,
            intermediate_size,
            vocab,
            q_projection: hidden_size,
            kv_projection,
            q_projection_size: hidden_size,
            q_heads: None,
            kv_heads: None,
            head_dim: None,
        });
    }

    Err(ModelFormatError::InvalidConfig)
}

pub fn weight_slice<'a>(
    bytes: &'a [u8],
    _header: &Header,
    tensor: &TensorMeta,
) -> Result<&'a [u8], ModelFormatError> {
    let start = checked_usize(tensor.weight_off)?;
    let len = checked_usize(tensor.weight_len)?;
    let end = start
        .checked_add(len)
        .ok_or(ModelFormatError::InvalidBounds)?;
    bytes.get(start..end).ok_or(ModelFormatError::InvalidBounds)
}

pub fn tensor_name<'a>(
    bytes: &'a [u8],
    header: &Header,
    tensor: &TensorMeta,
) -> Result<&'a str, ModelFormatError> {
    let string_start = checked_usize(header.string_off)?;
    let string_len = checked_usize(header.string_len)?;
    let name_off = checked_usize(tensor.name_off as u64)?;
    let name_len = checked_usize(tensor.name_len as u64)?;
    if name_off > string_len {
        return Err(ModelFormatError::InvalidBounds);
    }
    let start = string_start
        .checked_add(name_off)
        .ok_or(ModelFormatError::InvalidBounds)?;
    let end = start
        .checked_add(name_len)
        .ok_or(ModelFormatError::InvalidBounds)?;
    let string_end = string_start
        .checked_add(string_len)
        .ok_or(ModelFormatError::InvalidBounds)?;
    if end > string_end {
        return Err(ModelFormatError::InvalidBounds);
    }
    let slice = bytes
        .get(start..end)
        .ok_or(ModelFormatError::InvalidBounds)?;
    core::str::from_utf8(slice).map_err(|_| ModelFormatError::InvalidBounds)
}

fn rank2_tensor(
    bytes: &[u8],
    model: &ValidatedNeom,
    name: &str,
) -> Result<[u64; 2], ModelFormatError> {
    let tensor = tensor_named(bytes, model, name).ok_or(ModelFormatError::InvalidConfig)?;
    rank2_dims(tensor)
}

fn rank1_tensor(
    bytes: &[u8],
    model: &ValidatedNeom,
    name: &str,
) -> Result<[u64; 1], ModelFormatError> {
    let tensor = tensor_named(bytes, model, name).ok_or(ModelFormatError::InvalidConfig)?;
    rank1_dims(tensor)
}

fn first_tensor_element_candidates_with_suffix(
    bytes: &[u8],
    model: &ValidatedNeom,
    suffix: &str,
) -> Result<QuantElementCandidates, ModelFormatError> {
    let tensor = model
        .tensors
        .iter()
        .find(|tensor| {
            matches!(tensor_name(bytes, &model.header, tensor), Ok(found) if found.ends_with(suffix))
        })
        .ok_or(ModelFormatError::InvalidConfig)?;
    tensor_element_candidates_from_meta(tensor)
}

fn first_rank1_tensor_with_suffix(
    bytes: &[u8],
    model: &ValidatedNeom,
    suffix: &str,
) -> Result<[u64; 1], ModelFormatError> {
    let tensor = model
        .tensors
        .iter()
        .find(|tensor| {
            matches!(tensor_name(bytes, &model.header, tensor), Ok(found) if found.ends_with(suffix))
        })
        .ok_or(ModelFormatError::InvalidConfig)?;
    rank1_dims(tensor)
}

fn tensor_named<'a>(bytes: &[u8], model: &'a ValidatedNeom, name: &str) -> Option<&'a TensorMeta> {
    model.tensors.iter().find(
        |tensor| matches!(tensor_name(bytes, &model.header, tensor), Ok(found) if found == name),
    )
}

fn rank2_dims(tensor: &TensorMeta) -> Result<[u64; 2], ModelFormatError> {
    if tensor.shape.rank != 2 {
        return Err(ModelFormatError::InvalidConfig);
    }
    Ok([tensor.shape.dims[0], tensor.shape.dims[1]])
}

fn rank1_dims(tensor: &TensorMeta) -> Result<[u64; 1], ModelFormatError> {
    if tensor.shape.rank != 1 {
        return Err(ModelFormatError::InvalidConfig);
    }
    Ok([tensor.shape.dims[0]])
}

struct QuantElementCandidates {
    values: [u64; 4],
    len: usize,
}

fn raw_quant_tensor_element_candidates(
    bytes: &[u8],
    model: &ValidatedNeom,
    name: &str,
) -> Result<QuantElementCandidates, ModelFormatError> {
    let tensor = tensor_named(bytes, model, name).ok_or(ModelFormatError::InvalidConfig)?;
    if tensor.dtype != NeomDtype::U8
        || tensor.shape.rank != 1
        || tensor.shape.dims[0] != tensor.weight_len
    {
        return Err(ModelFormatError::InvalidConfig);
    }

    let mut out = QuantElementCandidates {
        values: [0; 4],
        len: 0,
    };
    push_quant_elements(
        &mut out,
        tensor.weight_len,
        GGML_Q4_K_BLOCK_BYTES,
        GGML_Q4_K_BLOCK_VALUES,
    )?;
    push_quant_elements(
        &mut out,
        tensor.weight_len,
        GGML_Q5_0_BLOCK_BYTES,
        GGML_Q5_0_BLOCK_VALUES,
    )?;
    push_quant_elements(
        &mut out,
        tensor.weight_len,
        GGML_Q6_K_BLOCK_BYTES,
        GGML_Q6_K_BLOCK_VALUES,
    )?;
    push_quant_elements(
        &mut out,
        tensor.weight_len,
        GGML_Q8_0_BLOCK_BYTES,
        GGML_Q8_0_BLOCK_VALUES,
    )?;
    if out.len == 0 {
        return Err(ModelFormatError::InvalidConfig);
    }
    Ok(out)
}

fn tensor_element_candidates(
    bytes: &[u8],
    model: &ValidatedNeom,
    name: &str,
) -> Result<QuantElementCandidates, ModelFormatError> {
    let tensor = tensor_named(bytes, model, name).ok_or(ModelFormatError::InvalidConfig)?;
    tensor_element_candidates_from_meta(tensor)
}

fn tensor_element_candidates_from_meta(
    tensor: &TensorMeta,
) -> Result<QuantElementCandidates, ModelFormatError> {
    if tensor.shape.rank == 2 {
        let elements = tensor.shape.dims[0]
            .checked_mul(tensor.shape.dims[1])
            .ok_or(ModelFormatError::InvalidConfig)?;
        return Ok(QuantElementCandidates {
            values: [elements, 0, 0, 0],
            len: 1,
        });
    }
    if tensor.dtype != NeomDtype::U8
        || tensor.shape.rank != 1
        || tensor.shape.dims[0] != tensor.weight_len
    {
        return Err(ModelFormatError::InvalidConfig);
    }

    quant_element_candidates_from_byte_len(tensor.weight_len)
}

fn quant_element_candidates_from_byte_len(
    byte_len: u64,
) -> Result<QuantElementCandidates, ModelFormatError> {
    let mut out = QuantElementCandidates {
        values: [0; 4],
        len: 0,
    };
    push_quant_elements(
        &mut out,
        byte_len,
        GGML_Q4_K_BLOCK_BYTES,
        GGML_Q4_K_BLOCK_VALUES,
    )?;
    push_quant_elements(
        &mut out,
        byte_len,
        GGML_Q5_0_BLOCK_BYTES,
        GGML_Q5_0_BLOCK_VALUES,
    )?;
    push_quant_elements(
        &mut out,
        byte_len,
        GGML_Q6_K_BLOCK_BYTES,
        GGML_Q6_K_BLOCK_VALUES,
    )?;
    push_quant_elements(
        &mut out,
        byte_len,
        GGML_Q8_0_BLOCK_BYTES,
        GGML_Q8_0_BLOCK_VALUES,
    )?;
    if out.len == 0 {
        return Err(ModelFormatError::InvalidConfig);
    }
    Ok(out)
}

fn push_quant_elements(
    out: &mut QuantElementCandidates,
    byte_len: u64,
    block_bytes: u64,
    block_values: u64,
) -> Result<(), ModelFormatError> {
    if byte_len.is_multiple_of(block_bytes) {
        let elements = byte_len
            .checked_div(block_bytes)
            .and_then(|blocks| blocks.checked_mul(block_values))
            .ok_or(ModelFormatError::InvalidConfig)?;
        out.values[out.len] = elements;
        out.len += 1;
    }
    Ok(())
}

fn first_factor(
    candidates: &QuantElementCandidates,
    factor: usize,
    min: usize,
    max: usize,
) -> Option<usize> {
    let factor = factor as u64;
    let mut idx = 0usize;
    while idx < candidates.len {
        let value = candidates.values[idx];
        if value.is_multiple_of(factor) {
            let quotient = value / factor;
            if quotient >= min as u64 && quotient <= max as u64 {
                return usize::try_from(quotient).ok();
            }
        }
        idx += 1;
    }
    None
}

fn contains_product(candidates: &QuantElementCandidates, lhs: usize, rhs: usize) -> bool {
    let Some(product) = (lhs as u64).checked_mul(rhs as u64) else {
        return false;
    };
    let mut idx = 0usize;
    while idx < candidates.len {
        if candidates.values[idx] == product {
            return true;
        }
        idx += 1;
    }
    false
}

fn square_root_if_square(value: u64) -> Option<usize> {
    let mut lo = 1u64;
    let mut hi = value.min(u32::MAX as u64);
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let square = mid.checked_mul(mid)?;
        if square == value {
            return usize::try_from(mid).ok();
        }
        if square < value {
            lo = mid + 1;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    None
}

fn infer_qwen2_heads(
    q_projection: usize,
    kv_projection: usize,
    hidden_size: usize,
) -> Result<(usize, usize, usize), ModelFormatError> {
    if q_projection != hidden_size || kv_projection == 0 || kv_projection > q_projection {
        return Err(ModelFormatError::InvalidConfig);
    }

    for head_dim in [128usize, 64usize] {
        if q_projection.is_multiple_of(head_dim) && kv_projection.is_multiple_of(head_dim) {
            let q_heads = q_projection / head_dim;
            let kv_heads = kv_projection / head_dim;
            if kv_heads >= 2 && q_heads >= kv_heads {
                return Ok((q_heads, kv_heads, head_dim));
            }
        }
    }
    Err(ModelFormatError::InvalidConfig)
}

fn checked_config_usize(value: u64) -> Result<usize, ModelFormatError> {
    let value = usize::try_from(value).map_err(|_| ModelFormatError::InvalidConfig)?;
    if value == 0 {
        return Err(ModelFormatError::InvalidConfig);
    }
    Ok(value)
}

/// Reads the declared file size from a complete `.neom` header without
/// requiring the full model bytes. The syscall loader uses this to allocate
/// once before reading large models.
pub fn neom_total_size_hint(header: &[u8]) -> Result<usize, ModelFormatError> {
    if header.len() < NEOM_HEADER_SIZE_V1 {
        return Err(ModelFormatError::InvalidHeader);
    }
    if header.get(0..4) != Some(&NEOM_MAGIC) {
        return Err(ModelFormatError::BadMagic);
    }
    let version = read_u32(header, 4)?;
    let header_size = read_u32(header, 8)? as usize;
    match (version, header_size) {
        (NEOM_VERSION_V1, NEOM_HEADER_SIZE_V1)
        | (NEOM_VERSION, NEOM_HEADER_SIZE)
        | (NEOM_VERSION_V3, NEOM_HEADER_SIZE_V3)
        | (NEOM_VERSION_V4, NEOM_HEADER_SIZE_V4) => {}
        _ if matches!(
            version,
            NEOM_VERSION_V1 | NEOM_VERSION | NEOM_VERSION_V3 | NEOM_VERSION_V4
        ) =>
        {
            return Err(ModelFormatError::InvalidHeader);
        }
        _ => return Err(ModelFormatError::BadVersion),
    }
    if header.len() < header_size || read_u32(header, 12)? != 0 {
        return Err(ModelFormatError::InvalidHeader);
    }
    let total_size =
        usize::try_from(read_u64(header, 88)?).map_err(|_| ModelFormatError::InvalidHeader)?;
    if total_size < header_size || total_size > NEOM_MAX_FILE_SIZE {
        return Err(ModelFormatError::InvalidHeader);
    }
    Ok(total_size)
}

fn parse_header(bytes: &[u8]) -> Result<Header, ModelFormatError> {
    if bytes.len() < NEOM_HEADER_SIZE_V1 {
        return Err(ModelFormatError::InvalidHeader);
    }
    if bytes.get(0..4) != Some(&NEOM_MAGIC) {
        return Err(ModelFormatError::BadMagic);
    }
    let version = read_u32(bytes, 4)?;
    let header_size = read_u32(bytes, 8)?;
    match (version, header_size as usize) {
        (NEOM_VERSION_V1, NEOM_HEADER_SIZE_V1) => {}
        (NEOM_VERSION, NEOM_HEADER_SIZE) => {}
        (NEOM_VERSION_V3, NEOM_HEADER_SIZE_V3) => {}
        (NEOM_VERSION_V4, NEOM_HEADER_SIZE_V4) => {}
        _ if version == NEOM_VERSION_V1
            || version == NEOM_VERSION
            || version == NEOM_VERSION_V3
            || version == NEOM_VERSION_V4 =>
        {
            return Err(ModelFormatError::InvalidHeader);
        }
        _ => return Err(ModelFormatError::BadVersion),
    }
    if bytes.len() < header_size as usize || read_u32(bytes, 12)? != 0 {
        return Err(ModelFormatError::InvalidHeader);
    }
    let total_size = read_u64(bytes, 88)?;
    if total_size != bytes.len() as u64 || bytes.len() > NEOM_MAX_FILE_SIZE {
        return Err(ModelFormatError::InvalidHeader);
    }
    let config = if matches!(version, NEOM_VERSION | NEOM_VERSION_V3 | NEOM_VERSION_V4) {
        Some(HeaderConfig {
            layer_count: read_u32(bytes, NEOM_CONFIG_LAYER_COUNT_OFF)?,
            rope_theta_bits: read_u32(bytes, NEOM_CONFIG_ROPE_THETA_OFF)?,
            rms_norm_eps_bits: read_u32(bytes, NEOM_CONFIG_RMS_NORM_EPS_OFF)?,
            rope_dimension_count: if version == NEOM_VERSION_V4 {
                Some(read_u32(bytes, NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF)?)
            } else {
                None
            },
        })
    } else {
        None
    };
    // v3 schedule descriptor (offsets 108..124). Bytes are guaranteed present:
    // header_size == 124 was checked above and `bytes.len() >= header_size`.
    let schedule = if version == NEOM_VERSION_V3 || version == NEOM_VERSION_V4 {
        Some(ScheduleRef {
            layer_count: read_u32(bytes, NEOM_SCHEDULE_LAYER_COUNT_OFF)?,
            version: read_u32(bytes, NEOM_SCHEDULE_VERSION_OFF)?,
            off: read_u64(bytes, NEOM_SCHEDULE_OFF_OFF)?,
        })
    } else {
        None
    };
    Ok(Header {
        version,
        header_size,
        tensor_count: read_u32(bytes, 16)?,
        node_count: read_u32(bytes, 20)?,
        input_count: read_u32(bytes, 24)?,
        output_count: read_u32(bytes, 28)?,
        tensor_table_off: read_u64(bytes, 32)?,
        node_table_off: read_u64(bytes, 40)?,
        io_table_off: read_u64(bytes, 48)?,
        string_off: read_u64(bytes, 56)?,
        string_len: read_u64(bytes, 64)?,
        weight_blob_off: read_u64(bytes, 72)?,
        weight_blob_len: read_u64(bytes, 80)?,
        total_size,
        config,
        schedule,
    })
}

/// Parse + validate the v3 layer-kind schedule (ISC-3 — the 7 load-time
/// invariants, THINK §2.2). A malformed or hostile schedule is rejected here as
/// data; it never reaches the walker or ring-0 forward. `model_layer_count` is
/// derived from the model config — the schedule must describe exactly that many
/// layers.
pub fn parse_schedule(
    bytes: &[u8],
    header: &Header,
    model_layer_count: usize,
) -> Result<LayerSchedule, ModelFormatError> {
    let sched = header.schedule.ok_or(ModelFormatError::InvalidSchedule)?;

    // Invariant 1 — version gate.
    if sched.version != NEOM_SCHEDULE_VERSION {
        return Err(ModelFormatError::UnsupportedScheduleVersion);
    }
    // Invariant 2 — layer-count bounds: 0 < n <= MAX_LAYERS.
    if sched.layer_count == 0 || sched.layer_count > NEOM_SCHEDULE_MAX_LAYERS {
        return Err(ModelFormatError::InvalidSchedule);
    }
    let layer_count = checked_usize(sched.layer_count as u64)?;
    // Invariants 4/5 — config consistency + weight presence: the schedule must
    // describe exactly the model's layers (for B1's homogeneous Qwen2, matching
    // layer_count + the existing per-tensor loader validation is sufficient; a
    // per-kind weight-presence check lands in B2 with GDN's distinct tensors).
    if layer_count != model_layer_count {
        return Err(ModelFormatError::InvalidSchedule);
    }
    // Invariant 7 — a static in-file table, not control flow: the descriptor
    // table must lie within the file, 8-aligned, after the header. Range::new
    // bounds-checks against total_size.
    let table_len = checked_mul(layer_count, NEOM_LAYER_DESCRIPTOR_SIZE)?;
    let total = checked_usize(header.total_size)?;
    let table = Range::new(sched.off, table_len as u64, total)?;
    if sched.off < header.header_size as u64 || sched.off % NEOM_TABLE_ALIGNMENT != 0 {
        return Err(ModelFormatError::InvalidSchedule);
    }

    let mut layers = Vec::with_capacity(layer_count);
    let mut idx = 0usize;
    while idx < layer_count {
        let base = table.start + idx * NEOM_LAYER_DESCRIPTOR_SIZE;
        // Invariant 3 — known kinds only (from_abi rejects unknown values).
        let kind = LayerKind::from_abi(read_u32(bytes, base + NEOM_LAYER_DESCRIPTOR_KIND_OFF)?)?;
        // B2: the reserved field is the per-layer GDN config-ref (ADR-072 Decision 2).
        let config_ref = read_u32(bytes, base + NEOM_LAYER_DESCRIPTOR_RESERVED_OFF)?;
        // B2: GatedDeltaNetLinear is now implemented — accept at load.
        layers.push(LayerEntry { kind, config_ref });
        idx += 1;
    }
    // Invariant 6 — state budget: B2 computes the total GDN recurrent-state size
    // from the config-refs and rejects at load if it overflows the model's declared
    // memory budget. (The budget check is in the caller; parse_schedule returns the
    // raw schedule + config-refs for the caller to validate.)

    Ok(LayerSchedule {
        version: sched.version,
        layers,
    })
}

/// Resolve the layer-kind schedule the walker will execute: parse + validate the
/// explicit v3 schedule, or synthesize a homogeneous `n x Qwen2AttentionFFN`
/// schedule for a v1/v2 file (so the shipped 0.5B/1.5B/3B `.neom` files run
/// unchanged under the walker). Called once before the forward loop.
// Consumed by the walker (model.rs, feature-gated) and the host schedule KATs;
// unconditional allow so it is warning-clean in every config, incl. the host
// test crates that mount this file without the `ot16-graph-walker` feature.
#[allow(dead_code)]
pub fn resolve_schedule(
    bytes: &[u8],
    header: &Header,
    model_layer_count: usize,
) -> Result<LayerSchedule, ModelFormatError> {
    if header.schedule.is_some() {
        parse_schedule(bytes, header, model_layer_count)
    } else {
        if model_layer_count == 0 || model_layer_count > NEOM_SCHEDULE_MAX_LAYERS as usize {
            return Err(ModelFormatError::InvalidSchedule);
        }
        Ok(LayerSchedule {
            version: NEOM_SCHEDULE_VERSION,
            layers: vec![
                LayerEntry {
                    kind: LayerKind::Qwen2AttentionFFN,
                    config_ref: 0
                };
                model_layer_count
            ],
        })
    }
}

/// Walk a layer-kind schedule, threading `hidden` through the appropriate layer
/// runner for each kind, in schedule order. This is the SINGLE dispatch used by
/// BOTH the ring-0 walker (qwen_inventory) and the host KATs. B2b adds
/// `GatedDeltaNetLinear` execution via `run_gdn_layer`; the caller owns the
/// recurrent GDN state beside the KV cache so it persists across token decode.
#[allow(dead_code)] // used by the walker (feature-gated) and the host KATs
pub fn walk_qwen2_schedule<F, G, E>(
    schedule: &LayerSchedule,
    initial: Vec<f32>,
    mut run_qwen2_layer: F,
    mut run_gdn_layer: G,
) -> Result<Vec<f32>, E>
where
    F: FnMut(usize, &[f32]) -> Result<Vec<f32>, E>,
    G: FnMut(usize, u32, &[f32]) -> Result<Vec<f32>, E>,
{
    let mut hidden = initial;
    let mut layer_idx = 0usize;
    while layer_idx < schedule.layers.len() {
        let entry = schedule.layers[layer_idx];
        match entry.kind {
            LayerKind::Qwen2AttentionFFN => {
                hidden = run_qwen2_layer(layer_idx, &hidden)?;
            }
            LayerKind::GatedDeltaNetLinear => {
                hidden = run_gdn_layer(layer_idx, entry.config_ref, &hidden)?;
            }
        }
        layer_idx += 1;
    }
    Ok(hidden)
}

fn parse_tensors(
    bytes: &[u8],
    header: Header,
    tensor_range: Range,
    string_range: Range,
    weight_range: Range,
) -> Result<Vec<TensorMeta>, ModelFormatError> {
    let count = checked_usize(header.tensor_count as u64)?;
    let mut tensors = Vec::with_capacity(count);
    let mut idx = 0usize;
    while idx < count {
        let base = tensor_range.start + idx * NEOM_TENSOR_ENTRY_SIZE;
        let dtype = NeomDtype::from_abi(read_u32(bytes, base)?)?;
        let rank = read_u32(bytes, base + 4)?;
        let dims = read_dims(bytes, base + 8)?;
        let shape = ShapeMeta::parse(rank, dims)?;
        let weight_off = read_u64(bytes, base + 72)?;
        let weight_len = read_u64(bytes, base + 80)?;
        let name_off = read_u32(bytes, base + 88)?;
        let name_len = read_u32(bytes, base + 92)?;
        let expected = shape.byte_len(dtype)?;
        if expected != weight_len {
            return Err(ModelFormatError::InvalidTensor);
        }
        let weight_start_usize = checked_usize(weight_off)?;
        let weight_len_usize = checked_usize(weight_len)?;
        if !weight_range.contains_region(weight_start_usize, weight_len_usize) {
            return Err(ModelFormatError::InvalidTensor);
        }
        if weight_off % NEOM_WEIGHT_ENTRY_ALIGNMENT != 0 {
            return Err(ModelFormatError::InvalidTensor);
        }
        validate_string_ref(bytes, string_range, name_off, name_len)?;
        tensors.push(TensorMeta {
            dtype,
            shape,
            weight_off,
            weight_len,
            name_off,
            name_len,
        });
        idx += 1;
    }
    Ok(tensors)
}

fn parse_nodes(
    bytes: &[u8],
    node_count: usize,
    node_range: Range,
    string_range: Range,
) -> Result<Vec<NodeMeta>, ModelFormatError> {
    let mut nodes = Vec::with_capacity(node_count);
    let refs_base = node_count
        .checked_mul(NEOM_NODE_ENTRY_SIZE)
        .ok_or(ModelFormatError::InvalidBounds)?;
    let mut idx = 0usize;
    while idx < node_count {
        let base = node_range.start + idx * NEOM_NODE_ENTRY_SIZE;
        let op = read_u32(bytes, base)?;
        if op > 11 {
            return Err(ModelFormatError::UnsupportedOp);
        }
        let input_count = checked_usize(read_u32(bytes, base + 4)? as u64)?;
        let output_count = checked_usize(read_u32(bytes, base + 8)? as u64)?;
        let inputs_off = read_u32(bytes, base + 12)?;
        let outputs_off = read_u32(bytes, base + 16)?;
        let attr_off = read_u32(bytes, base + 20)?;
        let attr_len = read_u32(bytes, base + 24)?;
        let name_off = read_u32(bytes, base + 28)?;
        let name_len = read_u32(bytes, base + 32)?;
        let reserved = bytes
            .get(base + 36..base + 48)
            .ok_or(ModelFormatError::InvalidNode)?;
        if input_count == 0
            || output_count == 0
            || input_count > NEOM_MAX_REFS_PER_NODE
            || output_count > NEOM_MAX_REFS_PER_NODE
            || reserved.iter().any(|b| *b != 0)
        {
            return Err(ModelFormatError::InvalidNode);
        }
        validate_string_ref(bytes, string_range, attr_off, attr_len)?;
        validate_string_ref(bytes, string_range, name_off, name_len)?;
        let inputs = read_node_refs(bytes, node_range, refs_base, inputs_off, input_count)?;
        let outputs = read_node_refs(bytes, node_range, refs_base, outputs_off, output_count)?;
        nodes.push(NodeMeta {
            op,
            inputs,
            outputs,
            attr_off,
            attr_len,
            name_off,
            name_len,
        });
        idx += 1;
    }
    Ok(nodes)
}

fn parse_ios(
    bytes: &[u8],
    input_count: usize,
    output_count: usize,
    io_range: Range,
) -> Result<Vec<IoMeta>, ModelFormatError> {
    let total = input_count
        .checked_add(output_count)
        .ok_or(ModelFormatError::InvalidIo)?;
    let mut ios = Vec::with_capacity(total);
    let mut idx = 0usize;
    while idx < total {
        let base = io_range.start + idx * NEOM_IO_ENTRY_SIZE;
        let value_index = read_u32(bytes, base)?;
        let dtype = NeomDtype::from_abi(read_u32(bytes, base + 4)?)
            .map_err(|_| ModelFormatError::InvalidIo)?;
        let rank = read_u32(bytes, base + 8)?;
        let is_output_raw = read_u32(bytes, base + 12)?;
        if is_output_raw > 1 {
            return Err(ModelFormatError::InvalidIo);
        }
        let shape = ShapeMeta::parse(rank, read_dims(bytes, base + 16)?)
            .map_err(|_| ModelFormatError::InvalidIo)?;
        let expected_output = idx >= input_count;
        if (is_output_raw != 0) != expected_output {
            return Err(ModelFormatError::InvalidIo);
        }
        ios.push(IoMeta {
            value_index,
            dtype,
            shape,
            is_output: is_output_raw != 0,
        });
        idx += 1;
    }
    Ok(ios)
}

fn validate_graph(
    tensors: &[TensorMeta],
    nodes: &[NodeMeta],
    ios: &[IoMeta],
    input_count: usize,
) -> Result<u32, ModelFormatError> {
    let total_node_outputs = nodes.iter().try_fold(0usize, |acc, node| {
        acc.checked_add(node.outputs.len())
            .ok_or(ModelFormatError::InvalidGraph)
    })?;
    let value_count = tensors
        .len()
        .checked_add(input_count)
        .and_then(|v| v.checked_add(total_node_outputs))
        .ok_or(ModelFormatError::InvalidGraph)?;
    if value_count == 0 || value_count > NEOM_MAX_VALUES {
        return Err(ModelFormatError::InvalidGraph);
    }
    let mut known = vec![false; value_count];
    let mut idx = 0usize;
    while idx < tensors.len() {
        known[idx] = true;
        idx += 1;
    }
    let mut io_idx = 0usize;
    while io_idx < input_count {
        let value = checked_usize(ios[io_idx].value_index as u64)?;
        if value >= value_count || known[value] {
            return Err(ModelFormatError::InvalidGraph);
        }
        known[value] = true;
        io_idx += 1;
    }
    for node in nodes {
        for input in &node.inputs {
            let value = checked_usize(*input as u64)?;
            if value >= value_count || !known[value] {
                return Err(ModelFormatError::InvalidGraph);
            }
        }
        for output in &node.outputs {
            let value = checked_usize(*output as u64)?;
            if value >= value_count || known[value] {
                return Err(ModelFormatError::InvalidGraph);
            }
            known[value] = true;
        }
    }
    let mut output_idx = input_count;
    while output_idx < ios.len() {
        let value = checked_usize(ios[output_idx].value_index as u64)?;
        if value >= value_count || !known[value] {
            return Err(ModelFormatError::InvalidGraph);
        }
        output_idx += 1;
    }
    let count = u32::try_from(value_count).map_err(|_| ModelFormatError::InvalidGraph)?;
    Ok(count)
}

fn validate_disjoint(ranges: &[Range]) -> Result<(), ModelFormatError> {
    let mut i = 0usize;
    while i < ranges.len() {
        let mut j = i + 1;
        while j < ranges.len() {
            if ranges[i].len() != 0 && ranges[j].len() != 0 && !ranges[i].disjoint(ranges[j]) {
                return Err(ModelFormatError::InvalidBounds);
            }
            j += 1;
        }
        i += 1;
    }
    Ok(())
}

fn validate_string_ref(
    bytes: &[u8],
    string_range: Range,
    off: u32,
    len: u32,
) -> Result<(), ModelFormatError> {
    let rel_off = checked_usize(off as u64)?;
    let len = checked_usize(len as u64)?;
    if rel_off > string_range.len() {
        return Err(ModelFormatError::InvalidBounds);
    }
    let start = string_range
        .start
        .checked_add(rel_off)
        .ok_or(ModelFormatError::InvalidBounds)?;
    if !string_range.contains_region(start, len) {
        return Err(ModelFormatError::InvalidBounds);
    }
    let slice = bytes
        .get(start..start + len)
        .ok_or(ModelFormatError::InvalidBounds)?;
    core::str::from_utf8(slice).map_err(|_| ModelFormatError::InvalidBounds)?;
    Ok(())
}

fn read_node_refs(
    bytes: &[u8],
    node_range: Range,
    refs_base: usize,
    off: u32,
    count: usize,
) -> Result<Vec<u32>, ModelFormatError> {
    let start = checked_usize(off as u64)?;
    let refs_start = node_range
        .start
        .checked_add(refs_base)
        .ok_or(ModelFormatError::InvalidNode)?;
    if start < refs_start || start % 4 != 0 {
        return Err(ModelFormatError::InvalidNode);
    }
    let byte_len = checked_mul(count, 4)?;
    if !node_range.contains_region(start, byte_len) {
        return Err(ModelFormatError::InvalidNode);
    }
    let mut refs = Vec::with_capacity(count);
    let mut idx = 0usize;
    while idx < count {
        refs.push(read_u32(bytes, start + idx * 4)?);
        idx += 1;
    }
    Ok(refs)
}

fn read_dims(bytes: &[u8], off: usize) -> Result<[u64; MAX_RANK], ModelFormatError> {
    let mut dims = [0u64; MAX_RANK];
    let mut idx = 0usize;
    while idx < MAX_RANK {
        dims[idx] = read_u64(bytes, off + idx * 8)?;
        idx += 1;
    }
    Ok(dims)
}

fn read_u32(bytes: &[u8], off: usize) -> Result<u32, ModelFormatError> {
    let slice = bytes
        .get(off..off + 4)
        .ok_or(ModelFormatError::InvalidBounds)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], off: usize) -> Result<u64, ModelFormatError> {
    let slice = bytes
        .get(off..off + 8)
        .ok_or(ModelFormatError::InvalidBounds)?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn checked_usize(value: u64) -> Result<usize, ModelFormatError> {
    usize::try_from(value).map_err(|_| ModelFormatError::InvalidBounds)
}

fn checked_mul(a: usize, b: usize) -> Result<usize, ModelFormatError> {
    a.checked_mul(b).ok_or(ModelFormatError::InvalidBounds)
}
