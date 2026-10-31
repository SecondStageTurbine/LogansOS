// SPDX-License-Identifier: MPL-2.0
//! Pure tensor descriptor validation shared by kernel code and host tests.

pub const MAX_RANK: usize = 8;
pub const PAGE_SIZE: usize = 4096;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TensorError {
    InvalidShape,
    InvalidDtype,
    InvalidLength,
    InvalidDevice,
    PermissionDenied,
    OutOfMemory,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Dtype {
    F32,
    F16,
    BF16,
    I32,
    I8,
    U8,
    Bool,
}

impl Dtype {
    pub const F32_ABI: u32 = 0;
    pub const F16_ABI: u32 = 1;
    pub const BF16_ABI: u32 = 2;
    pub const I32_ABI: u32 = 3;
    pub const I8_ABI: u32 = 4;
    pub const U8_ABI: u32 = 5;
    pub const BOOL_ABI: u32 = 6;

    pub const fn from_abi(value: u32) -> Result<Self, TensorError> {
        match value {
            Self::F32_ABI => Ok(Self::F32),
            Self::F16_ABI => Ok(Self::F16),
            Self::BF16_ABI => Ok(Self::BF16),
            Self::I32_ABI => Ok(Self::I32),
            Self::I8_ABI => Ok(Self::I8),
            Self::U8_ABI => Ok(Self::U8),
            Self::BOOL_ABI => Ok(Self::Bool),
            _ => Err(TensorError::InvalidDtype),
        }
    }

    pub const fn as_abi(self) -> u32 {
        match self {
            Self::F32 => Self::F32_ABI,
            Self::F16 => Self::F16_ABI,
            Self::BF16 => Self::BF16_ABI,
            Self::I32 => Self::I32_ABI,
            Self::I8 => Self::I8_ABI,
            Self::U8 => Self::U8_ABI,
            Self::Bool => Self::BOOL_ABI,
        }
    }

    pub const fn size_bytes(self) -> usize {
        match self {
            Self::F32 | Self::I32 => 4,
            Self::F16 | Self::BF16 => 2,
            Self::I8 | Self::U8 | Self::Bool => 1,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Device {
    Cpu,
    Accel(u32),
}

impl Device {
    pub const CPU_ABI: u32 = 0;
    pub const ACCEL_ABI_BASE: u32 = 0x8000_0000;

    pub const fn as_abi(self) -> u32 {
        match self {
            Self::Cpu => Self::CPU_ABI,
            Self::Accel(id) => Self::ACCEL_ABI_BASE | id,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Shape {
    rank: u8,
    dims: [u64; MAX_RANK],
}

impl Shape {
    pub const EMPTY: Self = Self {
        rank: 0,
        dims: [0; MAX_RANK],
    };

    pub fn new(rank: u32, dims: [u64; MAX_RANK]) -> Result<Self, TensorError> {
        if rank as usize > MAX_RANK {
            return Err(TensorError::InvalidShape);
        }

        let mut normalized = [0u64; MAX_RANK];
        let rank_usize = rank as usize;
        let mut idx = 0usize;
        while idx < rank_usize {
            if dims[idx] == 0 {
                return Err(TensorError::InvalidShape);
            }
            normalized[idx] = dims[idx];
            idx += 1;
        }

        Ok(Self {
            rank: rank as u8,
            dims: normalized,
        })
    }

    pub const fn rank(&self) -> u8 {
        self.rank
    }

    pub const fn dims(&self) -> [u64; MAX_RANK] {
        self.dims
    }

    pub const fn dim(&self, idx: usize) -> u64 {
        self.dims[idx]
    }

    pub fn num_elements(&self) -> Result<u64, TensorError> {
        let mut elements = 1u64;
        let mut idx = 0usize;
        while idx < self.rank as usize {
            let Some(next) = elements.checked_mul(self.dims[idx]) else {
                return Err(TensorError::InvalidLength);
            };
            elements = next;
            idx += 1;
        }
        Ok(elements)
    }

    pub fn byte_len(&self, dtype: Dtype) -> Result<usize, TensorError> {
        let bytes_per_element = dtype.size_bytes() as u64;
        let Some(byte_len) = self.num_elements()?.checked_mul(bytes_per_element) else {
            return Err(TensorError::InvalidLength);
        };
        if byte_len == 0 || byte_len > usize::MAX as u64 {
            return Err(TensorError::InvalidLength);
        }
        Ok(byte_len as usize)
    }
}

pub fn validate_data_len(shape: Shape, dtype: Dtype, data_len: u64) -> Result<usize, TensorError> {
    let expected = shape.byte_len(dtype)?;
    if data_len != expected as u64 {
        return Err(TensorError::InvalidLength);
    }
    Ok(expected)
}

pub fn order_for_len(len: usize) -> Result<usize, TensorError> {
    if len == 0 {
        return Err(TensorError::InvalidLength);
    }
    let Some(rounded) = len.checked_add(PAGE_SIZE - 1) else {
        return Err(TensorError::InvalidLength);
    };
    let pages = rounded / PAGE_SIZE;
    let mut order = 0usize;
    let mut capacity = 1usize;
    while capacity < pages {
        let Some(next) = capacity.checked_mul(2) else {
            return Err(TensorError::InvalidLength);
        };
        capacity = next;
        order += 1;
    }
    Ok(order)
}
