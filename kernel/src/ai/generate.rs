// SPDX-License-Identifier: MPL-2.0
//! Autoregressive generation over a validated `.neom` model.
//!
//! This is intentionally correctness-first: each token recomputes the full
//! static context prefix by calling `exec::run`. KV-cache plumbing is deferred.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;

use super::exec::{self, InferenceError};
use super::generate_logic::{GenerateLogicError, read_logits_at_position};
pub use super::generate_logic::{Pcg32, SamplingConfig, sample_next_token};
// The constrained/masked argmax helpers are re-exported only for the Arthur
// grammar/mask syscall paths that consume them; other `ai-generate` builds (e.g.
// the OT-28 KV-persist KAT) compile `generate` without those syscalls, so gating
// the re-export keeps those builds free of `unused_imports`.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
pub use super::generate_logic::{constrained_argmax_token, masked_argmax_token};
use super::model::{Model, NeomDtype};
use super::tensor::{Dtype, Tensor, TensorCreateDesc, TensorError};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GenerateError {
    Logic(GenerateLogicError),
    Inference(InferenceError),
    Tensor(TensorError),
    InvalidModelIo,
    InvalidToken,
    ContextFull,
    OutputCount,
}

impl From<GenerateLogicError> for GenerateError {
    fn from(error: GenerateLogicError) -> Self {
        Self::Logic(error)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GenerateConfig {
    pub max_new_tokens: usize,
    pub eos_token_id: Option<u32>,
    /// `.neom` v1 has static IO shapes, so BUILD-8.1 fills the fixed context
    /// tensor prefix with real tokens and pads the remaining slots.
    pub pad_token_id: u32,
}

pub fn generate(
    model: &Model,
    prompt: &[u32],
    config: GenerateConfig,
    sampling: SamplingConfig,
    rng: &mut Pcg32,
) -> Result<Vec<u32>, GenerateError> {
    if prompt.is_empty() {
        return Err(GenerateError::Logic(GenerateLogicError::EmptyPrompt));
    }
    let max_context = input_context_len(model)?;
    if prompt.len() > max_context {
        return Err(GenerateError::ContextFull);
    }

    let mut context = prompt.to_vec();
    let mut generated = Vec::new();
    for _ in 0..config.max_new_tokens {
        if context.len() >= max_context {
            return Err(GenerateError::ContextFull);
        }
        let logits = next_logits(model, &context, config.pad_token_id)?;
        let next = sample_next_token(&logits, sampling, rng)?;
        if next > i32::MAX as u32 {
            return Err(GenerateError::InvalidToken);
        }
        generated.push(next);
        context.push(next);
        if config.eos_token_id == Some(next) {
            break;
        }
    }
    Ok(generated)
}

pub fn next_logits(
    model: &Model,
    context: &[u32],
    pad_token_id: u32,
) -> Result<Vec<f32>, GenerateError> {
    if context.is_empty() {
        return Err(GenerateError::Logic(GenerateLogicError::EmptyPrompt));
    }
    let max_context = input_context_len(model)?;
    if context.len() > max_context {
        return Err(GenerateError::ContextFull);
    }
    let position = context.len() - 1;
    let input = token_tensor(context, max_context, pad_token_id)?;
    let outputs = exec::run(model, &[input]).map_err(GenerateError::Inference)?;
    if outputs.len() != 1 {
        return Err(GenerateError::OutputCount);
    }
    output_logits(&outputs[0], position)
}

fn input_context_len(model: &Model) -> Result<usize, GenerateError> {
    let mut inputs = model.ios().iter().filter(|io| !io.is_output);
    let input = inputs.next().ok_or(GenerateError::InvalidModelIo)?;
    if inputs.next().is_some() || input.dtype != NeomDtype::I32 || input.shape.rank != 1 {
        return Err(GenerateError::InvalidModelIo);
    }
    usize::try_from(input.shape.dims[0]).map_err(|_| GenerateError::InvalidModelIo)
}

fn token_tensor(
    tokens: &[u32],
    max_context: usize,
    pad_token_id: u32,
) -> Result<Arc<Tensor>, GenerateError> {
    if pad_token_id > i32::MAX as u32 {
        return Err(GenerateError::InvalidToken);
    }
    let byte_len = max_context
        .checked_mul(4)
        .ok_or(GenerateError::InvalidModelIo)?;
    let mut data = Vec::with_capacity(byte_len);
    for idx in 0..max_context {
        let token = tokens.get(idx).copied().unwrap_or(pad_token_id);
        if token > i32::MAX as u32 {
            return Err(GenerateError::InvalidToken);
        }
        data.extend_from_slice(&(token as i32).to_le_bytes());
    }

    let mut dims = [0u64; super::tensor::MAX_RANK];
    dims[0] = max_context as u64;
    let desc = TensorCreateDesc {
        rank: 1,
        dtype: Dtype::I32.as_abi(),
        mutable: 0,
        _pad: 0,
        dims,
        data_ptr: 0,
        data_len: data.len() as u64,
    };
    Tensor::from_desc_and_bytes(desc, &data)
        .map(Arc::new)
        .map_err(GenerateError::Tensor)
}

fn output_logits(output: &Tensor, position: usize) -> Result<Vec<f32>, GenerateError> {
    if output.dtype() != Dtype::F32 {
        return Err(GenerateError::InvalidModelIo);
    }
    let shape = output.shape();
    read_logits_at_position(
        output.bytes(),
        shape.rank() as usize,
        shape.dims(),
        position,
    )
    .map_err(GenerateError::Logic)
}
