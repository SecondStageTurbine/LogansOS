// SPDX-License-Identifier: MPL-2.0
//! Governed Qwen2 KV-cache allocation for the default-off GGUF generation path.
//!
//! BUILD-8.5 uses the ratified Option A shape: pre-reserve fixed-size K and V
//! tensors per layer, register them in the existing arena at generation start,
//! and keep `used_len` as controller-local state. The arena sees ordinary
//! fixed-footprint production tensors, so I-9, I-13, and reservation do not
//! need a new invariant.
#![allow(dead_code)] // default-off integration surface; host KATs exercise the logic first.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;

use super::forward_logic::{ForwardLayerError, Qwen2LayerConfig};
use super::kv_layout;
use super::tensor::{Dtype, Shape, Tensor, TensorError};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GovernedKvCacheError {
    Forward(ForwardLayerError),
    Tensor(TensorError),
    Stale,
}

impl From<ForwardLayerError> for GovernedKvCacheError {
    fn from(err: ForwardLayerError) -> Self {
        Self::Forward(err)
    }
}

impl From<TensorError> for GovernedKvCacheError {
    fn from(err: TensorError) -> Self {
        Self::Tensor(err)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GovernedKvLayer {
    pub k: crate::ai::arena::SlotKey,
    pub v: crate::ai::arena::SlotKey,
    used_len: usize,
}

impl GovernedKvLayer {
    pub const fn used_len(&self) -> usize {
        self.used_len
    }
}

pub struct GovernedKvCache {
    layers: Vec<GovernedKvLayer>,
    config: Qwen2LayerConfig,
    max_context: usize,
}

impl GovernedKvCache {
    pub fn pre_reserve(
        layer_count: usize,
        config: Qwen2LayerConfig,
        max_context: usize,
    ) -> Result<Self, GovernedKvCacheError> {
        if layer_count == 0
            || max_context == 0
            || config.kv_heads == 0
            || config.head_dim == 0
            || config.kv_len() == 0
        {
            return Err(ForwardLayerError::Shape.into());
        }

        let mut layers = Vec::with_capacity(layer_count);
        let shape = kv_shape(config, max_context)?;
        let home = crate::ai::arena::default_home();
        let release_partial = |allocated: &Vec<GovernedKvLayer>| {
            for layer in allocated {
                crate::ai::arena::release(layer.k);
                crate::ai::arena::release(layer.v);
            }
        };

        let mut idx = 0usize;
        while idx < layer_count {
            let k = match allocate_slot(shape, home) {
                Ok(key) => key,
                Err(err) => {
                    release_partial(&layers);
                    return Err(err);
                }
            };
            let v = match allocate_slot(shape, home) {
                Ok(key) => key,
                Err(err) => {
                    crate::ai::arena::release(k);
                    release_partial(&layers);
                    return Err(err);
                }
            };
            layers.push(GovernedKvLayer { k, v, used_len: 0 });
            idx += 1;
        }

        Ok(Self {
            layers,
            config,
            max_context,
        })
    }

    pub const fn max_context(&self) -> usize {
        self.max_context
    }

    pub fn layers(&self) -> &[GovernedKvLayer] {
        &self.layers
    }

    pub fn append_rope_kv(
        &mut self,
        layer_idx: usize,
        position: u64,
        k_rope: &[f32],
        v: &[f32],
    ) -> Result<(), GovernedKvCacheError> {
        if k_rope.len() != self.config.kv_len() || v.len() != self.config.kv_len() {
            return Err(ForwardLayerError::Shape.into());
        }
        let layer = self
            .layers
            .get_mut(layer_idx)
            .ok_or(ForwardLayerError::Shape)?;
        let position = usize::try_from(position).map_err(|_| ForwardLayerError::CachePosition)?;
        if position != layer.used_len {
            return Err(ForwardLayerError::CachePosition.into());
        }
        if layer.used_len >= self.max_context {
            return Err(ForwardLayerError::CacheFull.into());
        }

        write_slot(layer.k, self.config, self.max_context, position, k_rope)?;
        write_slot(layer.v, self.config, self.max_context, position, v)?;
        layer.used_len += 1;
        Ok(())
    }

    pub fn active_key(&self, layer_idx: usize) -> Result<Vec<f32>, GovernedKvCacheError> {
        let layer = self.layers.get(layer_idx).ok_or(ForwardLayerError::Shape)?;
        read_active_slot(layer.k, self.config, self.max_context, layer.used_len)
    }

    pub fn active_value(&self, layer_idx: usize) -> Result<Vec<f32>, GovernedKvCacheError> {
        let layer = self.layers.get(layer_idx).ok_or(ForwardLayerError::Shape)?;
        read_active_slot(layer.v, self.config, self.max_context, layer.used_len)
    }
}

impl Drop for GovernedKvCache {
    fn drop(&mut self) {
        for layer in &self.layers {
            crate::ai::arena::release(layer.k);
            crate::ai::arena::release(layer.v);
        }
    }
}

fn allocate_slot(
    shape: Shape,
    home: super::tensor::Device,
) -> Result<crate::ai::arena::SlotKey, GovernedKvCacheError> {
    let tensor = Tensor::allocate_writable_on(home, shape, Dtype::F32)?;
    Ok(crate::ai::arena::alloc(Arc::new(tensor), home))
}

fn write_slot(
    key: crate::ai::arena::SlotKey,
    config: Qwen2LayerConfig,
    max_context: usize,
    position: usize,
    values: &[f32],
) -> Result<(), GovernedKvCacheError> {
    let tensor = crate::ai::arena::mint(key).ok_or(GovernedKvCacheError::Stale)?;
    kv_layout::visit_token(config.kv_heads, config.head_dim, |head, dim, src| {
        let dst = kv_layout::storage_index(head, position, dim, max_context, config.head_dim)?;
        write_f32(&tensor, dst, values[src])?;
        Ok::<(), GovernedKvCacheError>(())
    })?;
    Ok(())
}

fn read_active_slot(
    key: crate::ai::arena::SlotKey,
    config: Qwen2LayerConfig,
    max_context: usize,
    used_len: usize,
) -> Result<Vec<f32>, GovernedKvCacheError> {
    if used_len == 0 {
        return Err(ForwardLayerError::Shape.into());
    }
    let tensor = crate::ai::arena::mint(key).ok_or(GovernedKvCacheError::Stale)?;
    let bytes = tensor.bytes();
    let mut out = Vec::with_capacity(checked_mul(used_len, config.kv_len())?);
    kv_layout::visit_active_prefix(
        config.kv_heads,
        used_len,
        max_context,
        config.head_dim,
        |_, _, _, src| {
            out.push(read_f32(bytes, src)?);
            Ok::<(), GovernedKvCacheError>(())
        },
    )?;
    Ok(out)
}

fn kv_shape(config: Qwen2LayerConfig, max_context: usize) -> Result<Shape, ForwardLayerError> {
    let mut dims = [0u64; super::tensor::MAX_RANK];
    dims[0] = config.kv_heads as u64;
    dims[1] = max_context as u64;
    dims[2] = config.head_dim as u64;
    Ok(Shape::new(3, dims)?)
}

fn read_f32(bytes: &[u8], idx: usize) -> Result<f32, ForwardLayerError> {
    let off = checked_mul(idx, 4)?;
    let slice = bytes.get(off..off + 4).ok_or(ForwardLayerError::Shape)?;
    Ok(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn write_f32(tensor: &Tensor, idx: usize, value: f32) -> Result<(), GovernedKvCacheError> {
    let off = checked_mul(idx, 4)?;
    // OT-70: shared `Arc<Tensor>` — write through `write_bytes`, never `&mut`.
    tensor
        .write_bytes(off, &value.to_le_bytes())
        .map_err(|err| match err {
            TensorError::InvalidLength => ForwardLayerError::Shape.into(),
            err => err.into(),
        })
}

fn checked_add(lhs: usize, rhs: usize) -> Result<usize, ForwardLayerError> {
    lhs.checked_add(rhs).ok_or(ForwardLayerError::Shape)
}

fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, ForwardLayerError> {
    lhs.checked_mul(rhs).ok_or(ForwardLayerError::Shape)
}
