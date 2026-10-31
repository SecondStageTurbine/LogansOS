// SPDX-License-Identifier: MPL-2.0
//! The forward-pass driver: synchronous graph execution for validated `.neom`
//! models, carrying the whole policy arc per op — placement (§9.9), eviction
//! under pressure (§9.10), proactive retain/cold-demote at `W = 1` lookahead
//! (§9.11), and the per-device need-notes that drive the residency daemon
//! (§9.12). BUILD-C1 split: this file owns the pass (value table, lookahead,
//! op loop, the boundary demote seam); `exec_ops.rs` owns per-op shape rules;
//! `exec_dispatch.rs` owns the place/room/dispatch glue for one op.

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::accel::{AccelDriver, AccelError};

use super::evict;
use super::exec_dispatch::{OpLookahead, Target, allocate_result, view};
use super::exec_ops;
use super::model::{Model, NeomDtype, ShapeMeta};
use super::model_logic::NodeMeta;
use super::ops::{self, OpKind, TensorView};
use super::predictor;
use super::tensor::{Dtype, Shape, Tensor, TensorError};

// The P1-gate migration metric — feature-gated scenario consumers (Rule 5).
#[allow(unused_imports)]
pub use super::exec_dispatch::{migrated_bytes, migration_count, reset_migration_counter};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum InferenceError {
    InvalidInput,
    InvalidGraph,
    UnsupportedOp,
    Tensor(TensorError),
    Op(ops::OpError),
    Accel(AccelError),
    /// CP-66: the admission scheduler withdrew the request (its process is
    /// being torn down); nothing ran.
    Cancelled,
}

/// The default inference path: residency-aware placement with LRU eviction
/// under pressure and the proactive retain/cold-demote decisions at `W = 1`
/// (the always-on substrate). Strict superset all the way down (ledger I-2).
pub fn run(model: &Model, inputs: &[Arc<Tensor>]) -> Result<Vec<Arc<Tensor>>, InferenceError> {
    // Arthur perf lane (OT-7): time the real forward at TSC (cycle) granularity —
    // this is the central forward EVERY caller funnels through (the generate/
    // next_logits path AND the sys_infer_run syscall path), so it captures the
    // showcase forward regardless of which syscall drove it. `read_tsc` is a pure
    // register read (no memory effect), so it does NOT change the logits — the
    // fast-on ≡ scalar kill-switch is unperturbed by measurement.
    #[cfg(feature = "forward-perf")]
    {
        let t0 = crate::ai::forward_perf::read_tsc();
        let out = run_inner(model, inputs, &Target::Placed { lookahead: 1 });
        crate::ai::forward_perf::record_forward(
            crate::ai::forward_perf::read_tsc().wrapping_sub(t0),
        );
        out
    }
    #[cfg(not(feature = "forward-perf"))]
    run_inner(model, inputs, &Target::Placed { lookahead: 1 })
}

/// The placed path with every proactive decision disabled (`W = 0`) — the
/// §9.10 reactive layer, bit for bit. The reactive-eviction gate and the
/// proactive gates' baseline (the I-2 kill-switch entry point).
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn run_reactive(
    model: &Model,
    inputs: &[Arc<Tensor>],
) -> Result<Vec<Arc<Tensor>>, InferenceError> {
    run_inner(model, inputs, &Target::Placed { lookahead: 0 })
}

/// The legacy fixed-driver path — cross-device equivalence scenarios and the
/// P1 gate's thrash baseline. Pre-P1 behavior, plus the migration counters.
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn run_with_driver(
    model: &Model,
    inputs: &[Arc<Tensor>],
    driver: &dyn AccelDriver,
) -> Result<Vec<Arc<Tensor>>, InferenceError> {
    run_inner(model, inputs, &Target::Fixed(driver))
}

fn run_inner(
    model: &Model,
    inputs: &[Arc<Tensor>],
    target: &Target,
) -> Result<Vec<Arc<Tensor>>, InferenceError> {
    if inputs.len() != model.input_count() as usize {
        return Err(InferenceError::InvalidInput);
    }
    let value_count =
        usize::try_from(model.value_count()).map_err(|_| InferenceError::InvalidGraph)?;
    let mut values: Vec<Option<Arc<Tensor>>> = Vec::with_capacity(value_count);
    values.resize_with(value_count, || None);

    for (idx, key) in model.weight_slots().iter().enumerate() {
        if idx >= value_count || values[idx].is_some() {
            return Err(InferenceError::InvalidGraph);
        }
        // BUILD-O1b: mint the weight for the pass (the I-9 pin — strong ≥ 2
        // for exactly the pass duration; the daemon skips it as referenced).
        // A stale key here means the model outlived its weights — a kernel
        // bug surfaced as a clean error, never a stale deref (I-13).
        let Some(weight) = crate::ai::arena::mint(*key) else {
            return Err(InferenceError::InvalidGraph);
        };
        values[idx] = Some(weight);
    }

    let mut input_idx = 0usize;
    for io in model.ios().iter().filter(|io| !io.is_output) {
        let Some(input) = inputs.get(input_idx) else {
            return Err(InferenceError::InvalidInput);
        };
        validate_io_tensor(io.dtype, io.shape, input)?;
        let value = value_index(io.value_index, value_count)?;
        if values[value].is_some() {
            return Err(InferenceError::InvalidGraph);
        }
        values[value] = Some(input.clone());
        input_idx += 1;
    }
    if input_idx != inputs.len() {
        return Err(InferenceError::InvalidInput);
    }

    // §9.11: the structural lookahead. Per op, compute from the remaining
    // graph which value indices are predicted-hot (consumed within `W`) and
    // which are dead to the pass (safe to proactively demote).
    let lookahead = match target {
        Target::Placed { lookahead } => *lookahead,
        Target::Fixed(_) => 0,
    };
    let nodes = model.nodes();
    let output_values: BTreeSet<u32> = model
        .ios()
        .iter()
        .filter(|io| io.is_output)
        .map(|io| io.value_index)
        .collect();
    for (idx, node) in nodes.iter().enumerate() {
        let la = if lookahead > 0 {
            let remaining: Vec<&[u32]> = nodes[idx + 1..]
                .iter()
                .map(|n| n.inputs.as_slice())
                .collect();
            let future = predictor::future_use_values(&remaining);
            let mut demote_ok = BTreeSet::new();
            for value in 0..value_count as u32 {
                if !future.contains(&value) && !output_values.contains(&value) {
                    demote_ok.insert(value);
                }
            }
            OpLookahead {
                active: true,
                last_op: idx + 1 == nodes.len(),
                hot: predictor::hot_values(&remaining, lookahead),
                demote_ok,
            }
        } else {
            OpLookahead::INERT
        };
        execute_node(model, node, &mut values, target, &la)?;

        // §9.11: the proactive demote seam — at the op boundary, after the
        // node's operand references have dropped (an op's own operands are
        // never demotable at its own seam). Banks one placement's worth of
        // headroom (the per-device hint) so the next placement avoids an
        // eviction round-trip. W=0 and the fixed path never reach it; the
        // final op has no seam.
        if la.active && !la.last_op {
            evict::bank_headroom_all(&mut values, &la.hot, &la.demote_ok);
        }
    }

    let mut outputs = Vec::with_capacity(model.output_count() as usize);
    for io in model.ios().iter().filter(|io| io.is_output) {
        let value = value_index(io.value_index, value_count)?;
        let Some(output) = values[value].as_ref() else {
            return Err(InferenceError::InvalidGraph);
        };
        validate_io_tensor(io.dtype, io.shape, output)?;
        outputs.push(output.clone());
    }
    if outputs.len() != model.output_count() as usize {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(outputs)
}

/// Execute one node: gather operands, validate shape/dtype (`exec_ops`), and
/// hand the op to the dispatch glue. The views handed to the driver are
/// always all operands in order — the BUILD-C1 observation that collapsed the
/// former per-op match into `exec_ops::output_shape_and_dtype`.
fn execute_node(
    model: &Model,
    node: &NodeMeta,
    values: &mut [Option<Arc<Tensor>>],
    target: &Target,
    la: &OpLookahead,
) -> Result<(), InferenceError> {
    if node.outputs.len() != 1 {
        return Err(InferenceError::InvalidGraph);
    }
    let output_value = value_index(node.outputs[0], values.len())?;
    if values[output_value].is_some() {
        return Err(InferenceError::InvalidGraph);
    }

    let mut inputs = Vec::with_capacity(node.inputs.len());
    for value_ref in &node.inputs {
        let value = value_index(*value_ref, values.len())?;
        let Some(tensor) = values[value].as_ref() else {
            return Err(InferenceError::InvalidGraph);
        };
        inputs.push(tensor.clone());
    }

    let op = OpKind::from_abi(node.op).ok_or(InferenceError::UnsupportedOp)?;
    let (shape, dtype) = exec_ops::output_shape_and_dtype(op, model, node, &inputs)?;
    let views: Vec<TensorView<'_>> = inputs.iter().map(|t| view(t)).collect();
    let output = allocate_result(target, values, la, &inputs, op, shape, dtype, &views)?;

    if let Some(expected) = exec_ops::io_shape_for_value(model, node.outputs[0])
        && output.shape() != expected
    {
        return Err(InferenceError::InvalidGraph);
    }
    values[output_value] = Some(output);
    Ok(())
}

fn validate_io_tensor(
    dtype: NeomDtype,
    shape: ShapeMeta,
    tensor: &Tensor,
) -> Result<(), InferenceError> {
    if tensor.dtype() != dtype_from_meta(dtype) || tensor.shape() != shape_from_meta(shape)? {
        return Err(InferenceError::InvalidInput);
    }
    Ok(())
}

fn dtype_from_meta(dtype: NeomDtype) -> Dtype {
    match dtype {
        NeomDtype::F32 => Dtype::F32,
        NeomDtype::F16 => Dtype::F16,
        NeomDtype::BF16 => Dtype::BF16,
        NeomDtype::I32 => Dtype::I32,
        NeomDtype::I8 => Dtype::I8,
        NeomDtype::U8 => Dtype::U8,
        NeomDtype::Bool => Dtype::Bool,
        NeomDtype::Q5K => Dtype::U8,
    }
}

fn shape_from_meta(shape: ShapeMeta) -> Result<Shape, InferenceError> {
    Shape::new(shape.rank, shape.dims).map_err(InferenceError::Tensor)
}

fn value_index(value: u32, value_count: usize) -> Result<usize, InferenceError> {
    let value = usize::try_from(value).map_err(|_| InferenceError::InvalidGraph)?;
    if value >= value_count {
        return Err(InferenceError::InvalidGraph);
    }
    Ok(value)
}
