// SPDX-License-Identifier: MPL-2.0
//! Pure GGUF v3 parser and quantized-block extraction.
//!
//! This module intentionally parses GGUF as an importer format. It does not
//! make GGUF native and it does not dequantize. The block constants are pinned
//! to ggml-org/llama.cpp commit 5f04dc7:
//! `gguf.h` (`GGUF_VERSION`/metadata ABI), `ggml.h` (`ggml_type` ABI), and
//! `ggml-common.h` (`block_q4_K`/`block_q8_0` static_asserts).
#![allow(dead_code)] // default-off importer surface; host KATs consume it first.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::str;

pub const GGUF_MAGIC: [u8; 4] = *b"GGUF";
pub const GGUF_VERSION: u32 = 3;
pub const GGUF_DEFAULT_ALIGNMENT: usize = 32;
pub const GGML_MAX_DIMS: usize = 4;
pub const GGML_MAX_NAME: usize = 64;

pub const QK8_0: usize = 32;
pub const BLOCK_Q8_0_SIZE: usize = 34;
pub const QK_K: usize = 256;
pub const K_SCALE_SIZE: usize = 12;
pub const BLOCK_Q4_K_SIZE: usize = 144;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GgufError {
    BadMagic,
    BadVersion,
    InvalidHeader,
    InvalidType,
    InvalidMetadata,
    DuplicateKey,
    DuplicateTensor,
    InvalidTensor,
    UnsupportedTensorType,
    InvalidBounds,
    InvalidUtf8,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GgufType {
    Uint8,
    Int8,
    Uint16,
    Int16,
    Uint32,
    Int32,
    Float32,
    Bool,
    String,
    Array,
    Uint64,
    Int64,
    Float64,
}

impl GgufType {
    pub const fn from_abi(value: i32) -> Result<Self, GgufError> {
        match value {
            0 => Ok(Self::Uint8),
            1 => Ok(Self::Int8),
            2 => Ok(Self::Uint16),
            3 => Ok(Self::Int16),
            4 => Ok(Self::Uint32),
            5 => Ok(Self::Int32),
            6 => Ok(Self::Float32),
            7 => Ok(Self::Bool),
            8 => Ok(Self::String),
            9 => Ok(Self::Array),
            10 => Ok(Self::Uint64),
            11 => Ok(Self::Int64),
            12 => Ok(Self::Float64),
            _ => Err(GgufError::InvalidType),
        }
    }

    pub const fn scalar_size(self) -> Option<usize> {
        match self {
            Self::Uint8 | Self::Int8 | Self::Bool => Some(1),
            Self::Uint16 | Self::Int16 => Some(2),
            Self::Uint32 | Self::Int32 | Self::Float32 => Some(4),
            Self::Uint64 | Self::Int64 | Self::Float64 => Some(8),
            Self::String | Self::Array => None,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum GgufArray {
    Scalars {
        element_type: GgufType,
        bytes: Vec<u8>,
        len: u64,
    },
    Strings(Vec<String>),
}

#[derive(Clone, PartialEq, Debug)]
pub enum GgufValue {
    Uint8(u8),
    Int8(i8),
    Uint16(u16),
    Int16(i16),
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
    Array(GgufArray),
}

#[derive(Clone, PartialEq, Debug)]
pub struct GgufMetadata {
    pub key: String,
    pub value: GgufValue,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GgmlType {
    F32,
    F16,
    Q4_0,
    Q4_1,
    Q5_0,
    Q5_1,
    Q8_0,
    Q8_1,
    Q2K,
    Q3K,
    Q4K,
    Q5K,
    Q6K,
    Q8K,
    I8,
    I16,
    I32,
    I64,
    F64,
    Bf16,
}

impl GgmlType {
    pub const fn from_abi(value: i32) -> Result<Self, GgufError> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::Q4_0),
            3 => Ok(Self::Q4_1),
            6 => Ok(Self::Q5_0),
            7 => Ok(Self::Q5_1),
            8 => Ok(Self::Q8_0),
            9 => Ok(Self::Q8_1),
            10 => Ok(Self::Q2K),
            11 => Ok(Self::Q3K),
            12 => Ok(Self::Q4K),
            13 => Ok(Self::Q5K),
            14 => Ok(Self::Q6K),
            15 => Ok(Self::Q8K),
            24 => Ok(Self::I8),
            25 => Ok(Self::I16),
            26 => Ok(Self::I32),
            27 => Ok(Self::I64),
            28 => Ok(Self::F64),
            30 => Ok(Self::Bf16),
            _ => Err(GgufError::UnsupportedTensorType),
        }
    }

    pub const fn as_abi(self) -> i32 {
        match self {
            Self::F32 => 0,
            Self::F16 => 1,
            Self::Q4_0 => 2,
            Self::Q4_1 => 3,
            Self::Q5_0 => 6,
            Self::Q5_1 => 7,
            Self::Q8_0 => 8,
            Self::Q8_1 => 9,
            Self::Q2K => 10,
            Self::Q3K => 11,
            Self::Q4K => 12,
            Self::Q5K => 13,
            Self::Q6K => 14,
            Self::Q8K => 15,
            Self::I8 => 24,
            Self::I16 => 25,
            Self::I32 => 26,
            Self::I64 => 27,
            Self::F64 => 28,
            Self::Bf16 => 30,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F16 => "f16",
            Self::Q4_0 => "q4_0",
            Self::Q4_1 => "q4_1",
            Self::Q5_0 => "q5_0",
            Self::Q5_1 => "q5_1",
            Self::Q8_0 => "q8_0",
            Self::Q8_1 => "q8_1",
            Self::Q2K => "q2_K",
            Self::Q3K => "q3_K",
            Self::Q4K => "q4_K",
            Self::Q5K => "q5_K",
            Self::Q6K => "q6_K",
            Self::Q8K => "q8_K",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F64 => "f64",
            Self::Bf16 => "bf16",
        }
    }

    pub const fn block_size(self) -> u64 {
        match self {
            Self::Q4_0 | Self::Q4_1 | Self::Q5_0 | Self::Q5_1 | Self::Q8_0 | Self::Q8_1 => 32,
            Self::Q2K | Self::Q3K | Self::Q4K | Self::Q5K | Self::Q6K | Self::Q8K => QK_K as u64,
            _ => 1,
        }
    }

    pub const fn type_size(self) -> u64 {
        match self {
            Self::F32 | Self::I32 => 4,
            Self::F16 | Self::Bf16 | Self::I16 => 2,
            Self::I8 => 1,
            Self::I64 | Self::F64 => 8,
            Self::Q4_0 => 18,
            Self::Q4_1 => 20,
            Self::Q5_0 => 22,
            Self::Q5_1 => 24,
            Self::Q8_0 => BLOCK_Q8_0_SIZE as u64,
            Self::Q8_1 => 36,
            Self::Q2K => 84,
            Self::Q3K => 110,
            Self::Q4K => BLOCK_Q4_K_SIZE as u64,
            Self::Q5K => 176,
            Self::Q6K => 210,
            Self::Q8K => 292,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct GgufTensorInfo {
    pub name: String,
    pub n_dims: u32,
    pub dims: [u64; GGML_MAX_DIMS],
    pub ty: GgmlType,
    pub offset: u64,
    pub byte_len: u64,
    pub data_start: u64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct GgufFile {
    pub version: u32,
    pub alignment: usize,
    pub data_start: u64,
    pub metadata: Vec<GgufMetadata>,
    pub tensors: Vec<GgufTensorInfo>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct BlockQ4K {
    pub d: u16,
    pub dmin: u16,
    pub scales: [u8; K_SCALE_SIZE],
    pub qs: [u8; QK_K / 2],
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct BlockQ8_0 {
    pub d: u16,
    pub qs: [i8; QK8_0],
}

impl GgufFile {
    pub fn metadata_value(&self, key: &str) -> Option<&GgufValue> {
        self.metadata
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }

    pub fn metadata_u32(&self, key: &str) -> Option<u32> {
        match self.metadata_value(key) {
            Some(GgufValue::Uint32(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn metadata_string(&self, key: &str) -> Option<&str> {
        match self.metadata_value(key) {
            Some(GgufValue::String(value)) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn metadata_string_array(&self, key: &str) -> Option<&[String]> {
        match self.metadata_value(key) {
            Some(GgufValue::Array(GgufArray::Strings(values))) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn tensor_named(&self, name: &str) -> Option<&GgufTensorInfo> {
        self.tensors.iter().find(|tensor| tensor.name == name)
    }

    pub fn tensor_data<'a>(
        &self,
        bytes: &'a [u8],
        tensor: &GgufTensorInfo,
    ) -> Result<&'a [u8], GgufError> {
        let start = checked_usize(tensor.data_start)?;
        let len = checked_usize(tensor.byte_len)?;
        let end = start.checked_add(len).ok_or(GgufError::InvalidBounds)?;
        bytes.get(start..end).ok_or(GgufError::InvalidBounds)
    }

    pub fn q4_k_blocks_for_tensor(
        &self,
        bytes: &[u8],
        tensor: &GgufTensorInfo,
    ) -> Result<Vec<BlockQ4K>, GgufError> {
        if tensor.ty != GgmlType::Q4K {
            return Err(GgufError::InvalidTensor);
        }
        parse_q4_k_blocks(self.tensor_data(bytes, tensor)?)
    }

    pub fn q8_0_blocks_for_tensor(
        &self,
        bytes: &[u8],
        tensor: &GgufTensorInfo,
    ) -> Result<Vec<BlockQ8_0>, GgufError> {
        if tensor.ty != GgmlType::Q8_0 {
            return Err(GgufError::InvalidTensor);
        }
        parse_q8_0_blocks(self.tensor_data(bytes, tensor)?)
    }
}

pub fn parse_gguf(bytes: &[u8]) -> Result<GgufFile, GgufError> {
    let mut reader = Reader::new(bytes);
    if reader.read_exact(4)? != GGUF_MAGIC {
        return Err(GgufError::BadMagic);
    }
    let version = reader.read_u32()?;
    if version != GGUF_VERSION {
        return Err(GgufError::BadVersion);
    }
    let tensor_count = checked_count(reader.read_i64()?)?;
    let metadata_count = checked_count(reader.read_i64()?)?;

    let mut metadata = Vec::with_capacity(metadata_count);
    let mut idx = 0usize;
    while idx < metadata_count {
        let key = reader.read_string()?;
        if metadata.iter().any(|entry: &GgufMetadata| entry.key == key) {
            return Err(GgufError::DuplicateKey);
        }
        let value_type = GgufType::from_abi(reader.read_i32()?)?;
        let value = if value_type == GgufType::Array {
            let element_type = GgufType::from_abi(reader.read_i32()?)?;
            let len = reader.read_u64()?;
            GgufValue::Array(reader.read_array(element_type, len)?)
        } else {
            reader.read_scalar(value_type)?
        };
        metadata.push(GgufMetadata { key, value });
        idx += 1;
    }

    let alignment = metadata
        .iter()
        .find_map(|entry| match (&entry.key[..], &entry.value) {
            ("general.alignment", GgufValue::Uint32(value)) => Some(*value as usize),
            _ => None,
        })
        .unwrap_or(GGUF_DEFAULT_ALIGNMENT);
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(GgufError::InvalidMetadata);
    }

    let mut tensors = Vec::with_capacity(tensor_count);
    let mut tensor_idx = 0usize;
    while tensor_idx < tensor_count {
        let name = reader.read_string()?;
        if name.len() >= GGML_MAX_NAME {
            return Err(GgufError::InvalidTensor);
        }
        if tensors
            .iter()
            .any(|tensor: &GgufTensorInfo| tensor.name == name)
        {
            return Err(GgufError::DuplicateTensor);
        }
        let n_dims = reader.read_u32()?;
        if n_dims as usize > GGML_MAX_DIMS || n_dims == 0 {
            return Err(GgufError::InvalidTensor);
        }
        let mut dims = [1u64; GGML_MAX_DIMS];
        let mut dim_idx = 0usize;
        while dim_idx < n_dims as usize {
            let dim = reader.read_i64()?;
            if dim <= 0 {
                return Err(GgufError::InvalidTensor);
            }
            dims[dim_idx] = dim as u64;
            dim_idx += 1;
        }
        let ty = GgmlType::from_abi(reader.read_i32()?)?;
        let block_size = ty.block_size();
        if block_size == 0 || dims[0] % block_size != 0 {
            return Err(GgufError::InvalidTensor);
        }
        let byte_len = tensor_byte_len(ty, dims)?;
        let offset = reader.read_u64()?;
        tensors.push(GgufTensorInfo {
            name,
            n_dims,
            dims,
            ty,
            offset,
            byte_len,
            data_start: 0,
        });
        tensor_idx += 1;
    }

    let data_start = align_up(reader.position(), alignment)?;
    if data_start > bytes.len() {
        return Err(GgufError::InvalidBounds);
    }

    let mut expected_offset = 0u64;
    for tensor in &mut tensors {
        if tensor.offset != expected_offset {
            return Err(GgufError::InvalidTensor);
        }
        let absolute = checked_add_u64(data_start as u64, tensor.offset)?;
        let end = checked_add_u64(absolute, tensor.byte_len)?;
        if end > bytes.len() as u64 {
            return Err(GgufError::InvalidBounds);
        }
        tensor.data_start = absolute;
        expected_offset = checked_add_u64(
            expected_offset,
            align_up_u64(tensor.byte_len, alignment as u64)?,
        )?;
    }

    let data_end = checked_add_u64(data_start as u64, expected_offset)?;
    if data_end > bytes.len() as u64 {
        return Err(GgufError::InvalidBounds);
    }

    Ok(GgufFile {
        version,
        alignment,
        data_start: data_start as u64,
        metadata,
        tensors,
    })
}

pub fn parse_q4_k_blocks(bytes: &[u8]) -> Result<Vec<BlockQ4K>, GgufError> {
    if !bytes.len().is_multiple_of(BLOCK_Q4_K_SIZE) {
        return Err(GgufError::InvalidTensor);
    }
    let mut blocks = Vec::with_capacity(bytes.len() / BLOCK_Q4_K_SIZE);
    for raw in bytes.chunks_exact(BLOCK_Q4_K_SIZE) {
        let mut scales = [0u8; K_SCALE_SIZE];
        scales.copy_from_slice(&raw[4..16]);
        let mut qs = [0u8; QK_K / 2];
        qs.copy_from_slice(&raw[16..144]);
        blocks.push(BlockQ4K {
            d: u16::from_le_bytes([raw[0], raw[1]]),
            dmin: u16::from_le_bytes([raw[2], raw[3]]),
            scales,
            qs,
        });
    }
    Ok(blocks)
}

pub fn parse_q8_0_blocks(bytes: &[u8]) -> Result<Vec<BlockQ8_0>, GgufError> {
    if !bytes.len().is_multiple_of(BLOCK_Q8_0_SIZE) {
        return Err(GgufError::InvalidTensor);
    }
    let mut blocks = Vec::with_capacity(bytes.len() / BLOCK_Q8_0_SIZE);
    for raw in bytes.chunks_exact(BLOCK_Q8_0_SIZE) {
        let mut qs = [0i8; QK8_0];
        let mut idx = 0usize;
        while idx < QK8_0 {
            qs[idx] = raw[2 + idx] as i8;
            idx += 1;
        }
        blocks.push(BlockQ8_0 {
            d: u16::from_le_bytes([raw[0], raw[1]]),
            qs,
        });
    }
    Ok(blocks)
}

fn tensor_byte_len(ty: GgmlType, dims: [u64; GGML_MAX_DIMS]) -> Result<u64, GgufError> {
    let mut elements = 1u64;
    let mut idx = 0usize;
    while idx < GGML_MAX_DIMS {
        elements = elements
            .checked_mul(dims[idx])
            .ok_or(GgufError::InvalidTensor)?;
        idx += 1;
    }
    let block_size = ty.block_size();
    if elements == 0 || !elements.is_multiple_of(block_size) {
        return Err(GgufError::InvalidTensor);
    }
    elements
        .checked_div(block_size)
        .and_then(|blocks| blocks.checked_mul(ty.type_size()))
        .ok_or(GgufError::InvalidTensor)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    const fn position(&self) -> usize {
        self.pos
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], GgufError> {
        let end = self.pos.checked_add(len).ok_or(GgufError::InvalidBounds)?;
        let out = self
            .bytes
            .get(self.pos..end)
            .ok_or(GgufError::InvalidBounds)?;
        self.pos = end;
        Ok(out)
    }

    fn read_u8(&mut self) -> Result<u8, GgufError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_i8(&mut self) -> Result<i8, GgufError> {
        Ok(self.read_u8()? as i8)
    }

    fn read_u16(&mut self) -> Result<u16, GgufError> {
        let raw = self.read_exact(2)?;
        Ok(u16::from_le_bytes([raw[0], raw[1]]))
    }

    fn read_i16(&mut self) -> Result<i16, GgufError> {
        Ok(self.read_u16()? as i16)
    }

    fn read_u32(&mut self) -> Result<u32, GgufError> {
        let raw = self.read_exact(4)?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn read_i32(&mut self) -> Result<i32, GgufError> {
        Ok(self.read_u32()? as i32)
    }

    fn read_f32(&mut self) -> Result<f32, GgufError> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    fn read_u64(&mut self) -> Result<u64, GgufError> {
        let raw = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
        ]))
    }

    fn read_i64(&mut self) -> Result<i64, GgufError> {
        Ok(self.read_u64()? as i64)
    }

    fn read_f64(&mut self) -> Result<f64, GgufError> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    fn read_string(&mut self) -> Result<String, GgufError> {
        let len = checked_usize(self.read_u64()?)?;
        let raw = self.read_exact(len)?;
        let text = str::from_utf8(raw).map_err(|_| GgufError::InvalidUtf8)?;
        Ok(String::from(text))
    }

    fn read_scalar(&mut self, ty: GgufType) -> Result<GgufValue, GgufError> {
        match ty {
            GgufType::Uint8 => Ok(GgufValue::Uint8(self.read_u8()?)),
            GgufType::Int8 => Ok(GgufValue::Int8(self.read_i8()?)),
            GgufType::Uint16 => Ok(GgufValue::Uint16(self.read_u16()?)),
            GgufType::Int16 => Ok(GgufValue::Int16(self.read_i16()?)),
            GgufType::Uint32 => Ok(GgufValue::Uint32(self.read_u32()?)),
            GgufType::Int32 => Ok(GgufValue::Int32(self.read_i32()?)),
            GgufType::Float32 => Ok(GgufValue::Float32(self.read_f32()?)),
            GgufType::Bool => Ok(GgufValue::Bool(self.read_u8()? != 0)),
            GgufType::String => Ok(GgufValue::String(self.read_string()?)),
            GgufType::Uint64 => Ok(GgufValue::Uint64(self.read_u64()?)),
            GgufType::Int64 => Ok(GgufValue::Int64(self.read_i64()?)),
            GgufType::Float64 => Ok(GgufValue::Float64(self.read_f64()?)),
            GgufType::Array => Err(GgufError::InvalidMetadata),
        }
    }

    fn read_array(&mut self, element_type: GgufType, len: u64) -> Result<GgufArray, GgufError> {
        if element_type == GgufType::Array {
            return Err(GgufError::InvalidMetadata);
        }
        if element_type == GgufType::String {
            let count = checked_usize(len)?;
            let mut values = Vec::with_capacity(count);
            let mut idx = 0usize;
            while idx < count {
                values.push(self.read_string()?);
                idx += 1;
            }
            return Ok(GgufArray::Strings(values));
        }
        let size = element_type
            .scalar_size()
            .ok_or(GgufError::InvalidMetadata)?;
        let byte_len = checked_mul(checked_usize(len)?, size)?;
        let bytes = self.read_exact(byte_len)?.to_vec();
        Ok(GgufArray::Scalars {
            element_type,
            bytes,
            len,
        })
    }
}

fn checked_count(value: i64) -> Result<usize, GgufError> {
    if value < 0 {
        return Err(GgufError::InvalidHeader);
    }
    checked_usize(value as u64)
}

fn checked_usize(value: u64) -> Result<usize, GgufError> {
    usize::try_from(value).map_err(|_| GgufError::InvalidBounds)
}

fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, GgufError> {
    lhs.checked_mul(rhs).ok_or(GgufError::InvalidBounds)
}

fn checked_add_u64(lhs: u64, rhs: u64) -> Result<u64, GgufError> {
    lhs.checked_add(rhs).ok_or(GgufError::InvalidBounds)
}

fn align_up(value: usize, alignment: usize) -> Result<usize, GgufError> {
    let add = alignment.checked_sub(1).ok_or(GgufError::InvalidMetadata)?;
    let rounded = value.checked_add(add).ok_or(GgufError::InvalidBounds)?;
    Ok(rounded & !add)
}

fn align_up_u64(value: u64, alignment: u64) -> Result<u64, GgufError> {
    let add = alignment.checked_sub(1).ok_or(GgufError::InvalidMetadata)?;
    let rounded = value.checked_add(add).ok_or(GgufError::InvalidBounds)?;
    Ok(rounded & !add)
}
