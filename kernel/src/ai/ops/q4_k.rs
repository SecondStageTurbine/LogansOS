// SPDX-License-Identifier: MPL-2.0
//! Raw-byte Q4_K dequantization helpers.
#![allow(dead_code)] // BUILD-8.3a host-KAT surface; 8.3 integration wires the consumer.

pub const Q4_K_VALUES: usize = 256;
pub const Q4_K_SCALE_BYTES: usize = 12;
pub const Q4_K_QUANT_BYTES: usize = Q4_K_VALUES / 2;
pub const Q4_K_BLOCK_BYTES: usize = 2 + 2 + Q4_K_SCALE_BYTES + Q4_K_QUANT_BYTES;

const D_OFFSET: usize = 0;
const DMIN_OFFSET: usize = 2;
const SCALES_OFFSET: usize = 4;
const QS_OFFSET: usize = SCALES_OFFSET + Q4_K_SCALE_BYTES;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Q4KError {
    Length,
    ScaleIndex,
}

pub fn dequantize_q4_k(block: &[u8]) -> Result<[f32; Q4_K_VALUES], Q4KError> {
    if block.len() != Q4_K_BLOCK_BYTES {
        return Err(Q4KError::Length);
    }

    let d = f16_bits_to_f32(read_u16_le(block, D_OFFSET));
    let dmin = f16_bits_to_f32(read_u16_le(block, DMIN_OFFSET));
    let scales = &block[SCALES_OFFSET..QS_OFFSET];
    let qs = &block[QS_OFFSET..Q4_K_BLOCK_BYTES];
    let mut out = [0.0f32; Q4_K_VALUES];

    let mut scale_idx = 0usize;
    let mut quant_idx = 0usize;
    let mut out_idx = 0usize;
    while out_idx < Q4_K_VALUES {
        let (sc1, m1) = get_scale_min_k4(scale_idx, scales)?;
        let (sc2, m2) = get_scale_min_k4(scale_idx + 1, scales)?;
        let d1 = d * sc1 as f32;
        let min1 = dmin * m1 as f32;
        let d2 = d * sc2 as f32;
        let min2 = dmin * m2 as f32;

        let mut lane = 0usize;
        while lane < 32 {
            out[out_idx + lane] = d1 * (qs[quant_idx + lane] & 0x0f) as f32 - min1;
            lane += 1;
        }

        lane = 0;
        while lane < 32 {
            out[out_idx + 32 + lane] = d2 * (qs[quant_idx + lane] >> 4) as f32 - min2;
            lane += 1;
        }

        quant_idx += 32;
        scale_idx += 2;
        out_idx += 64;
    }

    Ok(out)
}

pub fn get_scale_min_k4(index: usize, scales: &[u8]) -> Result<(u8, u8), Q4KError> {
    if index >= 8 || scales.len() != Q4_K_SCALE_BYTES {
        return Err(Q4KError::ScaleIndex);
    }

    if index < 4 {
        Ok((scales[index] & 63, scales[index + 4] & 63))
    } else {
        Ok((
            (scales[index + 4] & 0x0f) | ((scales[index - 4] >> 6) << 4),
            (scales[index + 4] >> 4) | ((scales[index] >> 6) << 4),
        ))
    }
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
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
