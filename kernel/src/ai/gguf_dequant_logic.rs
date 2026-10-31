// SPDX-License-Identifier: MPL-2.0
//! GGUF quantized tensor dequantization into the existing f32 op path.
//!
//! This module is deliberately pure logic: GGUF remains an importer format, and
//! the output is ordinary f32 data for the existing op kernels to consume.
#![allow(dead_code)] // default-off BUILD-8.3 integration surface; host KATs consume it first.

extern crate alloc;

use alloc::vec::Vec;

use super::gguf_logic::{
    BLOCK_Q4_K_SIZE, BLOCK_Q8_0_SIZE, GGML_MAX_DIMS, GgmlType, GgufError, GgufFile, GgufTensorInfo,
    QK8_0,
};
use super::ops::q4_k::{self, Q4KError};

const BLOCK_Q5_0_SIZE: usize = 22;
const QK5_0: usize = 32;
const BLOCK_Q5_K_SIZE: usize = 176;
const BLOCK_Q6_K_SIZE: usize = 210;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GgufDequantError {
    Gguf(GgufError),
    Q4K(Q4KError),
    TensorNotFound,
    InvalidTensor,
    UnsupportedTensorType,
}

impl From<GgufError> for GgufDequantError {
    fn from(err: GgufError) -> Self {
        Self::Gguf(err)
    }
}

impl From<Q4KError> for GgufDequantError {
    fn from(err: Q4KError) -> Self {
        Self::Q4K(err)
    }
}

pub fn dequantize_named_tensor_to_f32(
    file: &GgufFile,
    bytes: &[u8],
    name: &str,
) -> Result<Vec<f32>, GgufDequantError> {
    let tensor = file
        .tensor_named(name)
        .ok_or(GgufDequantError::TensorNotFound)?;
    dequantize_tensor_to_f32(file, bytes, tensor)
}

pub fn dequantize_tensor_to_f32(
    file: &GgufFile,
    bytes: &[u8],
    tensor: &GgufTensorInfo,
) -> Result<Vec<f32>, GgufDequantError> {
    let data = file.tensor_data(bytes, tensor)?;
    dequantize_tensor_data_to_f32(tensor, data)
}

pub fn dequantize_tensor_data_to_f32(
    tensor: &GgufTensorInfo,
    data: &[u8],
) -> Result<Vec<f32>, GgufDequantError> {
    let element_count = checked_usize(tensor_element_count(tensor)?)?;
    match tensor.ty {
        GgmlType::Q4K => dequantize_q4_k_tensor(data, element_count),
        GgmlType::Q5_0 => dequantize_q5_0_tensor(data, element_count),
        GgmlType::Q5K => dequantize_q5_k_tensor(data, element_count),
        GgmlType::Q6K => dequantize_q6_k_tensor(data, element_count),
        GgmlType::Q8_0 => dequantize_q8_0_tensor(data, element_count),
        _ => Err(GgufDequantError::UnsupportedTensorType),
    }
}

pub fn tensor_element_count(tensor: &GgufTensorInfo) -> Result<u64, GgufDequantError> {
    if tensor.n_dims == 0 || tensor.n_dims as usize > GGML_MAX_DIMS {
        return Err(GgufDequantError::InvalidTensor);
    }
    let mut elements = 1u64;
    let mut idx = 0usize;
    while idx < tensor.n_dims as usize {
        let dim = tensor.dims[idx];
        if dim == 0 {
            return Err(GgufDequantError::InvalidTensor);
        }
        elements = elements
            .checked_mul(dim)
            .ok_or(GgufDequantError::InvalidTensor)?;
        idx += 1;
    }
    Ok(elements)
}

fn dequantize_q4_k_tensor(data: &[u8], element_count: usize) -> Result<Vec<f32>, GgufDequantError> {
    if !data.len().is_multiple_of(BLOCK_Q4_K_SIZE) {
        return Err(GgufDequantError::InvalidTensor);
    }
    let blocks = data.len() / BLOCK_Q4_K_SIZE;
    if blocks
        .checked_mul(q4_k::Q4_K_VALUES)
        .ok_or(GgufDequantError::InvalidTensor)?
        != element_count
    {
        return Err(GgufDequantError::InvalidTensor);
    }

    let mut out = Vec::with_capacity(element_count);
    for block in data.chunks_exact(BLOCK_Q4_K_SIZE) {
        let dequantized = q4_k::dequantize_q4_k(block)?;
        out.extend_from_slice(&dequantized);
    }
    Ok(out)
}

fn dequantize_q5_0_tensor(data: &[u8], element_count: usize) -> Result<Vec<f32>, GgufDequantError> {
    if !data.len().is_multiple_of(BLOCK_Q5_0_SIZE) {
        return Err(GgufDequantError::InvalidTensor);
    }
    let blocks = data.len() / BLOCK_Q5_0_SIZE;
    if blocks
        .checked_mul(QK5_0)
        .ok_or(GgufDequantError::InvalidTensor)?
        != element_count
    {
        return Err(GgufDequantError::InvalidTensor);
    }

    let mut out = Vec::with_capacity(element_count);
    for block in data.chunks_exact(BLOCK_Q5_0_SIZE) {
        let d = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let qh = u32::from_le_bytes([block[2], block[3], block[4], block[5]]);
        let qs = &block[6..22];
        let mut idx = 0usize;
        while idx < 16 {
            let high = ((qh >> idx) & 1) << 4;
            let quant = ((qs[idx] & 0x0f) as u32 | high) as i32 - 16;
            out.push(quant as f32 * d);
            idx += 1;
        }
        idx = 0;
        while idx < 16 {
            let high = ((qh >> (idx + 16)) & 1) << 4;
            let quant = ((qs[idx] >> 4) as u32 | high) as i32 - 16;
            out.push(quant as f32 * d);
            idx += 1;
        }
    }
    Ok(out)
}

fn dequantize_q5_k_tensor(data: &[u8], element_count: usize) -> Result<Vec<f32>, GgufDequantError> {
    if !data.len().is_multiple_of(BLOCK_Q5_K_SIZE) {
        return Err(GgufDequantError::InvalidTensor);
    }
    let blocks = data.len() / BLOCK_Q5_K_SIZE;
    if blocks
        .checked_mul(super::gguf_logic::QK_K)
        .ok_or(GgufDequantError::InvalidTensor)?
        != element_count
    {
        return Err(GgufDequantError::InvalidTensor);
    }

    let mut out = Vec::with_capacity(element_count);
    for block in data.chunks_exact(BLOCK_Q5_K_SIZE) {
        let d = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let dmin = f16_bits_to_f32(u16::from_le_bytes([block[2], block[3]]));
        let scales = &block[4..16];
        let qh = &block[16..48];
        let ql = &block[48..176];

        let mut scale_idx = 0usize;
        let mut quant_idx = 0usize;
        let mut high_low = 1u8;
        let mut high_high = 2u8;
        while scale_idx < 8 {
            let (scale_low, min_low) = q4_k::get_scale_min_k4(scale_idx, scales)?;
            let (scale_high, min_high) = q4_k::get_scale_min_k4(scale_idx + 1, scales)?;
            let d_low = d * scale_low as f32;
            let m_low = dmin * min_low as f32;
            let d_high = d * scale_high as f32;
            let m_high = dmin * min_high as f32;

            let mut lane = 0usize;
            while lane < 32 {
                let high = if qh[lane] & high_low != 0 { 16 } else { 0 };
                out.push(d_low * ((ql[quant_idx + lane] & 0x0f) + high) as f32 - m_low);
                lane += 1;
            }
            lane = 0;
            while lane < 32 {
                let high = if qh[lane] & high_high != 0 { 16 } else { 0 };
                out.push(d_high * ((ql[quant_idx + lane] >> 4) + high) as f32 - m_high);
                lane += 1;
            }

            quant_idx += 32;
            scale_idx += 2;
            high_low <<= 2;
            high_high <<= 2;
        }
    }
    Ok(out)
}

fn dequantize_q6_k_tensor(data: &[u8], element_count: usize) -> Result<Vec<f32>, GgufDequantError> {
    if !data.len().is_multiple_of(BLOCK_Q6_K_SIZE) {
        return Err(GgufDequantError::InvalidTensor);
    }
    let blocks = data.len() / BLOCK_Q6_K_SIZE;
    if blocks
        .checked_mul(super::gguf_logic::QK_K)
        .ok_or(GgufDequantError::InvalidTensor)?
        != element_count
    {
        return Err(GgufDequantError::InvalidTensor);
    }

    let mut out = Vec::with_capacity(element_count);
    for block in data.chunks_exact(BLOCK_Q6_K_SIZE) {
        let ql = &block[0..128];
        let qh = &block[128..192];
        let scales = &block[192..208];
        let d = f16_bits_to_f32(u16::from_le_bytes([block[208], block[209]]));

        let mut ql_base = 0usize;
        let mut qh_base = 0usize;
        let mut scale_base = 0usize;
        while ql_base < 128 {
            let mut values = [0.0f32; 128];
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
                values[lane] = d * scales[scale_base + scale_pair] as i8 as f32 * q1 as f32;
                values[lane + 32] =
                    d * scales[scale_base + scale_pair + 2] as i8 as f32 * q2 as f32;
                values[lane + 64] =
                    d * scales[scale_base + scale_pair + 4] as i8 as f32 * q3 as f32;
                values[lane + 96] =
                    d * scales[scale_base + scale_pair + 6] as i8 as f32 * q4 as f32;
                lane += 1;
            }
            out.extend_from_slice(&values);
            ql_base += 64;
            qh_base += 32;
            scale_base += 8;
        }
    }
    Ok(out)
}

fn dequantize_q8_0_tensor(data: &[u8], element_count: usize) -> Result<Vec<f32>, GgufDequantError> {
    if !data.len().is_multiple_of(BLOCK_Q8_0_SIZE) {
        return Err(GgufDequantError::InvalidTensor);
    }
    let blocks = data.len() / BLOCK_Q8_0_SIZE;
    if blocks
        .checked_mul(QK8_0)
        .ok_or(GgufDequantError::InvalidTensor)?
        != element_count
    {
        return Err(GgufDequantError::InvalidTensor);
    }

    let mut out = Vec::with_capacity(element_count);
    for block in data.chunks_exact(BLOCK_Q8_0_SIZE) {
        let d = f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let mut idx = 0usize;
        while idx < QK8_0 {
            out.push(block[2 + idx] as i8 as f32 * d);
            idx += 1;
        }
    }
    Ok(out)
}

fn checked_usize(value: u64) -> Result<usize, GgufDequantError> {
    usize::try_from(value).map_err(|_| GgufDequantError::InvalidTensor)
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
