// SPDX-License-Identifier: MPL-2.0
//! ADR-028 D1/D2 — `.neom` v1/v2 binary format constants, types, and helpers.
//! All multi-byte fields are little-endian. All offsets are absolute from
//! the start of the file.

use std::io::{self, Write};

// ── Magic & version ──────────────────────────────────────────────────────────

pub const NEOM_MAGIC: [u8; 4] = *b"NEOM";
/// v1 — original layout (96-byte header).
pub const NEOM_VERSION_V1: u32 = 1;
/// v2 — extended header with config metadata (108-byte header). Remains the
/// default written version: the OT-16 v3 schedule section is *additive*, emitted
/// only when a layer-kind schedule is requested (existing v1/v2 files unaffected).
pub const NEOM_VERSION: u32 = 2;
/// v3 — OT-16 B1: adds the layer-kind schedule section (124-byte header). A new
/// architecture becomes a new `.neom` schedule, not a kernel branch (ANSPEC
/// §19.2). The forward-graph walker reads this schedule; on a v1/v2 file it
/// synthesizes a homogeneous `n × Qwen2AttentionFFN` schedule for compat.
pub const NEOM_VERSION_V3: u32 = 3;
/// v4 — OT-16 B3: appends the attention RoPE dimension count so models with
/// partial rotary heads remain data-driven in ring 0.
pub const NEOM_VERSION_V4: u32 = 4;

// ── Alignment constants ──────────────────────────────────────────────────────

/// Page size for weight blob alignment (ADR-028 D1).
pub const PAGE_SIZE: usize = 4096;
/// Per-weight alignment within the blob.
pub const WEIGHT_ALIGN: usize = 64;

// ── Struct sizes (repr(C), validated against ADR-028) ────────────────────────

/// v1 header size (96 bytes).
pub const HEADER_SIZE_V1: u32 = 96;
/// v2 header size — 12-byte config-metadata extension (108 bytes).
pub const HEADER_SIZE: u32 = 108;
/// Size of the v2-only config-metadata extension block.
pub const CONFIG_META_SIZE: u32 = 12;
pub const TENSOR_ENTRY_SIZE: u32 = 96;
pub const NODE_ENTRY_SIZE: u32 = 48;
pub const IO_ENTRY_SIZE: u32 = 80;

// ── Config metadata offsets (v2 header, relative to header start) ────────────

/// Offset of `layer_count` (u32 LE) in v2 header.
pub const CONFIG_LAYER_COUNT_OFF: usize = 96;
/// Offset of `rope_theta` (f32 LE) in v2 header.
pub const CONFIG_ROPE_THETA_OFF: usize = 100;
/// Offset of `rms_norm_eps` (f32 LE) in v2 header.
pub const CONFIG_RMS_NORM_EPS_OFF: usize = 104;

// ── v3 schedule-section descriptor (OT-16 B1) ────────────────────────────────
// The v3 header appends a 16-byte descriptor after the v2 config block; it
// points at a LayerDescriptor table (the layer-kind schedule). All fields LE,
// offsets absolute from file start. See ADR-028 (v3 amendment) for the oracle.

/// v3 header size — v2 (108) + 16-byte schedule descriptor (124 bytes).
pub const HEADER_SIZE_V3: u32 = 124;
/// Size of the v3-only schedule-section descriptor block.
pub const SCHEDULE_DESC_SIZE: u32 = 16;
/// Offset of the schedule `layer_count` (u32 LE) in the v3 header.
pub const SCHEDULE_LAYER_COUNT_OFF: usize = 108;
/// Offset of the schedule `version` (u32 LE) in the v3 header.
pub const SCHEDULE_VERSION_OFF: usize = 112;
/// Offset of the schedule table absolute `offset` (u64 LE) in the v3 header.
pub const SCHEDULE_OFF_OFF: usize = 116;

// ── v4 attention-geometry extension (OT-16 B3) ──────────────────────────────

/// v4 header size — v3 (124) + `rope_dimension_count` (u32).
pub const HEADER_SIZE_V4: u32 = 128;
/// Offset of the number of dimensions per attention head transformed by RoPE.
pub const CONFIG_ROPE_DIMENSION_COUNT_OFF: usize = 124;

/// Current layer-kind schedule format version (independent of the `.neom`
/// container version).
pub const SCHEDULE_VERSION: u32 = 1;
/// LayerDescriptor entry size: `kind` (u32) + `reserved` (u32).
pub const LAYER_DESCRIPTOR_SIZE: u32 = 8;
/// Offset of `kind` (u32 LE) within a LayerDescriptor entry.
pub const LAYER_DESCRIPTOR_KIND_OFF: usize = 0;
/// Offset of `reserved` (u32 LE, must be 0 in B1) within a LayerDescriptor entry.
pub const LAYER_DESCRIPTOR_RESERVED_OFF: usize = 4;
/// Maximum layers a schedule may declare (matches kernel `MAX_LAYERS`).
pub const SCHEDULE_MAX_LAYERS: u32 = 128;

/// Layer-kind for the data-driven forward-graph schedule (OT-16). B1 implements
/// only `Qwen2AttentionFFN`; `GatedDeltaNetLinear` is declared for B2 but is
/// **rejected at load** in B1 (never stub-executed).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum LayerKind {
    Qwen2AttentionFFN = 0,
    GatedDeltaNetLinear = 1,
}

impl LayerKind {
    /// Parse an on-disk `kind` ABI value. Unknown values are rejected (the
    /// known-kinds-only load-time invariant); `GatedDeltaNetLinear` parses here
    /// but is rejected by the walker's load-time validation in B1.
    pub fn from_abi(value: u32) -> Result<Self, NeomError> {
        match value {
            0 => Ok(Self::Qwen2AttentionFFN),
            1 => Ok(Self::GatedDeltaNetLinear),
            _ => Err(NeomError::UnknownLayerKind(value)),
        }
    }
}

// ── Dtype ABI (must match kernel ai/tensor_logic.rs) ────────────────────────

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Dtype {
    F32 = 0,
    F16 = 1,
    BF16 = 2,
    I32 = 3,
    I8 = 4,
    U8 = 5,
    Bool = 6,
    Q5K = 7,
}

impl Dtype {
    pub const fn size_bytes(self) -> u64 {
        match self {
            Self::F32 | Self::I32 => 4,
            Self::F16 | Self::BF16 => 2,
            Self::I8 | Self::U8 | Self::Bool | Self::Q5K => 1,
        }
    }

    pub fn from_abi(value: u32) -> Result<Self, NeomError> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::BF16),
            3 => Ok(Self::I32),
            4 => Ok(Self::I8),
            5 => Ok(Self::U8),
            6 => Ok(Self::Bool),
            7 => Ok(Self::Q5K),
            _ => Err(NeomError::InvalidDtype(value)),
        }
    }
}

// ── OpCode (ADR-028 D3) ──────────────────────────────────────────────────────

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum OpCode {
    Identity = 0,
    Add = 1,
    Mul = 2,
    MatMul = 3,
    Relu = 4,
    Gelu = 5,
    Softmax = 6,
    LayerNorm = 7,
    RmsNorm = 8,
    Embedding = 9,
    Reshape = 10,
    Transpose = 11,
}

impl OpCode {
    pub fn from_abi(value: u32) -> Result<Self, NeomError> {
        match value {
            0 => Ok(Self::Identity),
            1 => Ok(Self::Add),
            2 => Ok(Self::Mul),
            3 => Ok(Self::MatMul),
            4 => Ok(Self::Relu),
            5 => Ok(Self::Gelu),
            6 => Ok(Self::Softmax),
            7 => Ok(Self::LayerNorm),
            8 => Ok(Self::RmsNorm),
            9 => Ok(Self::Embedding),
            10 => Ok(Self::Reshape),
            11 => Ok(Self::Transpose),
            _ => Err(NeomError::UnknownOpCode(value)),
        }
    }
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum NeomError {
    InvalidMagic,
    InvalidVersion(u32),
    NonZeroFlags(u32),
    BadHeaderSize(u32),
    TotalSizeMismatch {
        header: u64,
        file: u64,
    },
    TableOverflow {
        table: &'static str,
    },
    WeightBlobNotPageAligned(u64),
    WeightOffNot64Aligned {
        tensor_idx: usize,
        off: u64,
    },
    WeightRangeOutsideBlob {
        tensor_idx: usize,
    },
    InvalidDtype(u32),
    RankTooLarge(u32),
    ZeroDimension {
        tensor_idx: usize,
        dim_idx: usize,
    },
    WeightLenMismatch {
        tensor_idx: usize,
        expected: u64,
        actual: u64,
    },
    StringOffsetOutOfRange {
        off: u32,
        len: u32,
    },
    AttrOffsetOutOfRange {
        node_idx: usize,
    },
    ValueRefOutOfRange {
        node_idx: usize,
        ref_idx: usize,
        value: u32,
    },
    InputOutputRefOutOfRange {
        value_index: u32,
    },
    CyclicGraph,
    UnknownOpCode(u32),
    /// v3 schedule: a LayerDescriptor `kind` value is not a known `LayerKind`.
    UnknownLayerKind(u32),
    /// v3 schedule requested without the v2 config block (a schedule needs a model).
    ScheduleRequiresConfig,
    /// v3 schedule table failed a structural invariant.
    InvalidSchedule(&'static str),
    IoEntryValueIndexOutOfRange {
        entry_idx: usize,
        value_index: u32,
    },
    IoDtypeInvalid(u32),
    IoRankTooLarge(u32),
    IoZeroDimension {
        entry_idx: usize,
    },
    NodeReservedNonZero {
        node_idx: usize,
    },
    NodeIoRefOverflow {
        node_idx: usize,
    },
    IoIsOutputBadValue(u32),
    IOError,
}

impl std::fmt::Display for NeomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "invalid magic (expected NEOM)"),
            Self::InvalidVersion(v) => write!(f, "unsupported version {v}"),
            Self::NonZeroFlags(fl) => write!(f, "reserved flags must be 0, got {fl}"),
            Self::BadHeaderSize(s) => write!(
                f,
                "header_size must be 96 (v1), 108 (v2), 124 (v3), or 128 (v4), got {s}"
            ),
            Self::TotalSizeMismatch { header, file } => {
                write!(f, "total_size {header} != file length {file}")
            }
            Self::TableOverflow { table } => write!(f, "{table} extends past total_size"),
            Self::WeightBlobNotPageAligned(off) => {
                write!(f, "weight_blob_off 0x{off:X} is not page-aligned (4096)")
            }
            Self::WeightOffNot64Aligned { tensor_idx, off } => {
                write!(
                    f,
                    "tensor[{tensor_idx}] weight_off 0x{off:X} is not 64-byte aligned"
                )
            }
            Self::WeightRangeOutsideBlob { tensor_idx } => {
                write!(
                    f,
                    "tensor[{tensor_idx}] weight range extends past weight blob"
                )
            }
            Self::InvalidDtype(d) => write!(f, "invalid dtype ABI value {d}"),
            Self::RankTooLarge(r) => write!(f, "rank {r} exceeds maximum 8"),
            Self::ZeroDimension {
                tensor_idx,
                dim_idx,
            } => {
                write!(f, "tensor[{tensor_idx}] dims[{dim_idx}] is zero")
            }
            Self::WeightLenMismatch {
                tensor_idx,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "tensor[{tensor_idx}] weight_len {actual} != expected {expected}"
                )
            }
            Self::StringOffsetOutOfRange { off, len } => {
                write!(
                    f,
                    "string heap offset 0x{off:X} + len {len} exceeds string_len"
                )
            }
            Self::AttrOffsetOutOfRange { node_idx } => {
                write!(
                    f,
                    "node[{node_idx}] attr offset/len out of string heap range"
                )
            }
            Self::ValueRefOutOfRange {
                node_idx,
                ref_idx,
                value,
            } => {
                write!(
                    f,
                    "node[{node_idx}] value ref[{ref_idx}] = {value} out of range"
                )
            }
            Self::InputOutputRefOutOfRange { value_index } => {
                write!(
                    f,
                    "IO entry value_index {value_index} out of graph value range"
                )
            }
            Self::CyclicGraph => write!(f, "node graph contains a cycle (not a DAG)"),
            Self::UnknownOpCode(op) => write!(f, "unknown opcode {op}"),
            Self::UnknownLayerKind(k) => write!(f, "unknown layer kind {k}"),
            Self::ScheduleRequiresConfig => {
                write!(
                    f,
                    "v3 schedule requires the v2 config block (layer_count/rope_theta/rms_norm_eps)"
                )
            }
            Self::InvalidSchedule(reason) => write!(f, "invalid v3 schedule: {reason}"),
            Self::IoEntryValueIndexOutOfRange {
                entry_idx,
                value_index,
            } => {
                write!(
                    f,
                    "io_entry[{entry_idx}] value_index {value_index} out of range"
                )
            }
            Self::IoDtypeInvalid(d) => write!(f, "IO entry has invalid dtype {d}"),
            Self::IoRankTooLarge(r) => write!(f, "IO entry rank {r} exceeds 8"),
            Self::IoZeroDimension { entry_idx } => {
                write!(f, "io_entry[{entry_idx}] has a zero dimension")
            }
            Self::NodeReservedNonZero { node_idx } => {
                write!(f, "node[{node_idx}] reserved field is non-zero")
            }
            Self::NodeIoRefOverflow { node_idx } => {
                write!(
                    f,
                    "node[{node_idx}] input/output ref array extends past node table"
                )
            }
            Self::IoIsOutputBadValue(v) => {
                write!(f, "IO entry is_output must be 0 or 1, got {v}")
            }
            Self::IOError => write!(f, "I/O error"),
        }
    }
}

impl std::error::Error for NeomError {}

impl From<std::io::Error> for NeomError {
    fn from(_: std::io::Error) -> Self {
        Self::IOError
    }
}

pub type NeomResult<T> = Result<T, NeomError>;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Align `value` up to the next multiple of `align`.
#[inline]
pub const fn align_up(value: u64, align: u64) -> u64 {
    value.div_ceil(align) * align
}

/// Compute element count for a shape (product of active dims).
pub fn shape_element_count(rank: usize, dims: &[u64; 8]) -> NeomResult<u64> {
    let mut count = 1u64;
    for &dim in dims.iter().take(rank) {
        count = count.checked_mul(dim).ok_or({
            // Use a generic overflow error; caller context disambiguates.
            NeomError::WeightLenMismatch {
                tensor_idx: 0,
                expected: 0,
                actual: u64::MAX,
            }
        })?;
    }
    Ok(count)
}

/// Write a u32 in little-endian to the buffer at the given offset.
pub fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    let b = value.to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&b);
}

/// Write a u64 in little-endian to the buffer at the given offset.
pub fn write_u64_le(buf: &mut [u8], offset: usize, value: u64) {
    let b = value.to_le_bytes();
    buf[offset..offset + 8].copy_from_slice(&b);
}

/// Write an f32 in little-endian to the buffer at the given offset.
pub fn write_f32_le(buf: &mut [u8], offset: usize, value: f32) {
    let b = value.to_bits().to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&b);
}

/// Read a u32 in little-endian from the buffer.
pub fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&buf[offset..offset + 4]);
    u32::from_le_bytes(b)
}

/// Read a u64 in little-endian from the buffer.
pub fn read_u64_le(buf: &[u8], offset: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[offset..offset + 8]);
    u64::from_le_bytes(b)
}

/// Read an f32 in little-endian from the buffer.
pub fn read_f32_le(buf: &[u8], offset: usize) -> f32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&buf[offset..offset + 4]);
    f32::from_bits(u32::from_le_bytes(b))
}

/// Write bytes to a file, padding the file to `total_size` with zeros.
pub fn write_file(buf: &[u8], path: &std::path::Path) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(buf)?;
    file.flush()?;
    Ok(())
}
