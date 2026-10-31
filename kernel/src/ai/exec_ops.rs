// SPDX-License-Identifier: MPL-2.0
//! Per-op shape/dtype validation for the executor (BUILD-C1 split of the
//! former `exec.rs` op match). Pure shape arithmetic over the op vocabulary:
//! given an op and its operand tensors, validate arity/dtype/rank and produce
//! the output `(Shape, Dtype)`. No placement, no dispatch, no residency — the
//! glue lives in `exec_dispatch.rs`, the pass driver in `exec.rs`.
//!
//! The consolidation observation that earned this file: every op's dispatch
//! views are simply *all operands in order*, so the only per-op variance is
//! this function — the former 130-line match collapsed into the uniform
//! driver in `exec.rs::execute_node` plus this validation.

extern crate alloc;

use alloc::sync::Arc;

use super::exec::InferenceError;
use super::model::Model;
use super::model_logic::NodeMeta;
use super::ops::{self, OpKind};
use super::tensor::{Dtype, Shape, Tensor};

/// Validate `op`'s operands and compute the output shape/dtype. Exactly the
/// arity, dtype, rank, and broadcast rules the per-op match enforced — one op
/// per arm, no behavior change.
pub(super) fn output_shape_and_dtype(
    op: OpKind,
    model: &Model,
    node: &NodeMeta,
    inputs: &[Arc<Tensor>],
) -> Result<(Shape, Dtype), InferenceError> {
    match op {
        OpKind::Identity | OpKind::Relu | OpKind::Softmax | OpKind::Gelu => {
            require_inputs(inputs, 1)?;
            require_f32(&inputs[0])?;
            Ok((inputs[0].shape(), inputs[0].dtype()))
        }
        OpKind::Reshape => {
            require_inputs(inputs, 1)?;
            require_f32(&inputs[0])?;
            let target =
                io_shape_for_value(model, node.outputs[0]).unwrap_or_else(|| inputs[0].shape());
            require_same_elements(inputs[0].shape_ref(), &target)?;
            Ok((target, inputs[0].dtype()))
        }
        OpKind::Transpose => {
            require_inputs(inputs, 1)?;
            require_f32(&inputs[0])?;
            require_rank2(inputs[0].shape_ref())?;
            let shape = shape2(inputs[0].shape_ref().dim(1), inputs[0].shape_ref().dim(0))?;
            Ok((shape, inputs[0].dtype()))
        }
        OpKind::Add | OpKind::Mul => {
            require_inputs(inputs, 2)?;
            require_f32(&inputs[0])?;
            require_f32(&inputs[1])?;
            let shape = broadcast_shape(inputs[0].shape_ref(), inputs[1].shape_ref())?;
            Ok((shape, Dtype::F32))
        }
        OpKind::Matmul => {
            require_inputs(inputs, 2)?;
            require_f32(&inputs[0])?;
            require_f32(&inputs[1])?;
            let (lhs, rhs) = (inputs[0].shape_ref(), inputs[1].shape_ref());
            require_rank2(lhs)?;
            require_rank2(rhs)?;
            if lhs.dim(1) != rhs.dim(0) {
                return Err(InferenceError::Op(ops::OpError::Shape));
            }
            Ok((shape2(lhs.dim(0), rhs.dim(1))?, Dtype::F32))
        }
        OpKind::LayerNorm => {
            require_input_count_one_of(inputs, 1, 3)?;
            require_f32(&inputs[0])?;
            if inputs.len() == 3 {
                require_f32(&inputs[1])?;
                require_f32(&inputs[2])?;
            }
            Ok((inputs[0].shape(), inputs[0].dtype()))
        }
        OpKind::RmsNorm => {
            require_input_range(inputs, 1, 2)?;
            require_f32(&inputs[0])?;
            if inputs.len() == 2 {
                require_f32(&inputs[1])?;
            }
            Ok((inputs[0].shape(), inputs[0].dtype()))
        }
        OpKind::Embedding => {
            require_inputs(inputs, 2)?;
            require_i32(&inputs[0])?;
            require_f32(&inputs[1])?;
            require_rank1(inputs[0].shape_ref())?;
            require_rank2(inputs[1].shape_ref())?;
            let shape = shape2(inputs[0].shape_ref().dim(0), inputs[1].shape_ref().dim(1))?;
            Ok((shape, Dtype::F32))
        }
        OpKind::Silu | OpKind::Swiglu | OpKind::Rope | OpKind::GqaAttention => {
            Err(InferenceError::UnsupportedOp)
        }
    }
}

/// The declared IO shape for `value`, if the model names one — used for the
/// Reshape target and the post-op output check in `exec.rs`.
pub(super) fn io_shape_for_value(model: &Model, value: u32) -> Option<Shape> {
    model
        .ios()
        .iter()
        .find(|io| io.value_index == value)
        .and_then(|io| Shape::new(io.shape.rank, io.shape.dims).ok())
}

fn require_inputs(inputs: &[Arc<Tensor>], expected: usize) -> Result<(), InferenceError> {
    if inputs.len() == expected {
        Ok(())
    } else {
        Err(InferenceError::InvalidGraph)
    }
}

fn require_input_range(
    inputs: &[Arc<Tensor>],
    min: usize,
    max: usize,
) -> Result<(), InferenceError> {
    if (min..=max).contains(&inputs.len()) {
        Ok(())
    } else {
        Err(InferenceError::InvalidGraph)
    }
}

fn require_input_count_one_of(
    inputs: &[Arc<Tensor>],
    first: usize,
    second: usize,
) -> Result<(), InferenceError> {
    if inputs.len() == first || inputs.len() == second {
        Ok(())
    } else {
        Err(InferenceError::InvalidGraph)
    }
}

fn require_f32(tensor: &Tensor) -> Result<(), InferenceError> {
    if tensor.dtype() == Dtype::F32 {
        Ok(())
    } else {
        Err(InferenceError::Op(ops::OpError::Dtype))
    }
}

fn require_i32(tensor: &Tensor) -> Result<(), InferenceError> {
    if tensor.dtype() == Dtype::I32 {
        Ok(())
    } else {
        Err(InferenceError::Op(ops::OpError::Dtype))
    }
}

fn require_rank1(shape: &Shape) -> Result<(), InferenceError> {
    if shape.rank() == 1 {
        Ok(())
    } else {
        Err(InferenceError::Op(ops::OpError::Shape))
    }
}

fn require_rank2(shape: &Shape) -> Result<(), InferenceError> {
    if shape.rank() == 2 {
        Ok(())
    } else {
        Err(InferenceError::Op(ops::OpError::Shape))
    }
}

fn require_same_elements(lhs: &Shape, rhs: &Shape) -> Result<(), InferenceError> {
    let lhs = lhs.num_elements().map_err(InferenceError::Tensor)?;
    let rhs = rhs.num_elements().map_err(InferenceError::Tensor)?;
    if lhs == rhs {
        Ok(())
    } else {
        Err(InferenceError::Op(ops::OpError::Shape))
    }
}

fn broadcast_shape(lhs: &Shape, rhs: &Shape) -> Result<Shape, InferenceError> {
    require_rank2(lhs)?;
    require_rank2(rhs)?;
    let rows = broadcast_dim(lhs.dim(0), rhs.dim(0))?;
    let cols = broadcast_dim(lhs.dim(1), rhs.dim(1))?;
    shape2(rows, cols)
}

fn broadcast_dim(lhs: u64, rhs: u64) -> Result<u64, InferenceError> {
    if lhs == rhs {
        Ok(lhs)
    } else if lhs == 1 {
        Ok(rhs)
    } else if rhs == 1 {
        Ok(lhs)
    } else {
        Err(InferenceError::Op(ops::OpError::Shape))
    }
}

fn shape2(rows: u64, cols: u64) -> Result<Shape, InferenceError> {
    let mut dims = [0u64; super::tensor::MAX_RANK];
    dims[0] = rows;
    dims[1] = cols;
    Shape::new(2, dims).map_err(InferenceError::Tensor)
}
