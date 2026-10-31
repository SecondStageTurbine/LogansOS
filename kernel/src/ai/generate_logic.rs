// SPDX-License-Identifier: MPL-2.0
//! Pure generation helpers: seeded sampling, logits slicing, and a host-testable
//! decode loop. Kernel glue that owns tensors and calls `exec::run` lives in
//! `generate.rs`.

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::Ordering;

#[cfg(not(test))]
use super::ops::soft_f32;
#[cfg(test)]
use crate::ops::soft_f32;

/// B1c (F1): the generate bounds. Only the generate syscall arms (and the
/// host KATs) consume them; other `ai-generate` builds compile them unused.
// Not keyed to kernel features: this file is also mounted by the host
// `ai-ops-tests` crate, which does not declare them.
#[allow(dead_code)]
mod bounds {
    /// B1c: the largest `max_new` a `SYS_GENERATE` request may ask for (the cap
    /// `SYS_GENERATE_CONSTRAINED` already has).
    pub const MAX_GENERATE_NEW_TOKENS: usize = 256;
    /// B1c: the longest prompt a `SYS_GENERATE` request may pass. With the cap
    /// above, the 0.5B's KV cache stays near 55 MiB (24 KiB per token).
    pub const MAX_GENERATE_PROMPT_TOKENS: usize = 2048;

    /// The candidate-set limits of `SYS_GENERATE_CONSTRAINED` (unchanged from
    /// Phase 8 A1; B1c moves them here so one policy owns them). Its new-token
    /// cap was already 256, the same as `MAX_GENERATE_NEW_TOKENS`.
    pub const MAX_CONSTRAINED_CANDIDATE_SEQUENCES: usize = 16;
    pub const MAX_CONSTRAINED_CANDIDATE_TOKENS: usize = 1024;
    /// The vocabulary-mask limits of `SYS_GENERATE_MASKED` (unchanged from Arthur A1).
    pub const MAX_GENERATE_MASK_BITS: usize = 262_144;
    pub const MAX_GENERATE_MASK_BYTES: usize = MAX_GENERATE_MASK_BITS / 8;

    /// B1c (F1): the size-bearing fields of one generate request, read from the
    /// registers and the fixed-size descriptor only, before anything else is
    /// copied or allocated.
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub enum GenerateRequest {
        Plain {
            prompt_len: usize,
            max_new: usize,
        },
        Constrained {
            prompt_len: usize,
            max_new: usize,
            sequence_count: usize,
            tokens_len: usize,
        },
        Masked {
            prompt_len: usize,
            disabled: bool,
            mask_bits: usize,
            mask_bytes: usize,
        },
    }

    /// Which bound a refused request broke (every variant maps to `EINVAL`).
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub enum GenerateRequestError {
        Prompt,
        NewTokens,
        Sequences,
        CandidateTokens,
        Mask,
    }

    /// The effective sizes of an admitted request: the KV cache and every buffer
    /// the forward allocates are sized from these and nothing else.
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct GenerateBounds {
        pub prompt_len: usize,
        pub max_new: usize,
    }

    /// B1c (AAG J2): the register-carried bound, the prompt length. The
    /// constrained and masked arms carry their other bounds in a fixed-size
    /// descriptor, so they check this before copying that descriptor, then the
    /// full policy right after it. It is the same prompt rule
    /// `check_generate_request` applies (and `SYS_INFER_RUN`'s inventory route
    /// applies to its token tensor).
    pub const fn check_prompt_len(prompt_len: usize) -> Result<(), GenerateRequestError> {
        if prompt_len == 0 || prompt_len > MAX_GENERATE_PROMPT_TOKENS {
            Err(GenerateRequestError::Prompt)
        } else {
            Ok(())
        }
    }

    /// B1c (F1, AAG J2): the one pre-allocation bounds policy for all three
    /// generate ABIs. The ordering contract at the syscall boundary:
    ///
    /// 1. register-carried bounds first (`SYS_GENERATE`: all of them; the
    ///    other two: the prompt length, `check_prompt_len`);
    /// 2. the only user access before the full policy is the fixed-size
    ///    descriptor read into a stack array (constrained, masked);
    /// 3. then this policy, before any variable-size user copy, any heap
    ///    allocation, and the model lookup.
    ///
    /// So an oversized request never reaches the KV-cache `vec!` (which aborts
    /// the kernel on failure). A masked request generates exactly one token.
    pub const fn check_generate_request(
        request: GenerateRequest,
    ) -> Result<GenerateBounds, GenerateRequestError> {
        let (prompt_len, max_new) = match request {
            GenerateRequest::Plain {
                prompt_len,
                max_new,
            } => (prompt_len, max_new),
            GenerateRequest::Constrained {
                prompt_len,
                max_new,
                sequence_count,
                tokens_len,
            } => {
                if sequence_count == 0 || sequence_count > MAX_CONSTRAINED_CANDIDATE_SEQUENCES {
                    return Err(GenerateRequestError::Sequences);
                }
                if tokens_len == 0 || tokens_len > MAX_CONSTRAINED_CANDIDATE_TOKENS {
                    return Err(GenerateRequestError::CandidateTokens);
                }
                (prompt_len, max_new)
            }
            GenerateRequest::Masked {
                prompt_len,
                disabled,
                mask_bits,
                mask_bytes,
            } => {
                if !disabled
                    && (mask_bits == 0
                        || mask_bits > MAX_GENERATE_MASK_BITS
                        || mask_bytes == 0
                        || mask_bytes > MAX_GENERATE_MASK_BYTES
                        || mask_bytes != mask_bits.div_ceil(8))
                {
                    return Err(GenerateRequestError::Mask);
                }
                (prompt_len, 1)
            }
        };
        if let Err(e) = check_prompt_len(prompt_len) {
            return Err(e);
        }
        if max_new == 0 || max_new > MAX_GENERATE_NEW_TOKENS {
            return Err(GenerateRequestError::NewTokens);
        }
        Ok(GenerateBounds {
            prompt_len,
            max_new,
        })
    }
}
#[allow(unused_imports)]
pub use bounds::*;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GenerateLogicError {
    EmptyPrompt,
    EmptyLogits,
    InvalidLogit,
    InvalidTemperature,
    InvalidTopK,
    InvalidTopP,
    InvalidProbability,
    InvalidShape,
    InvalidPosition,
    InvalidMask,
    ByteLength,
    VocabTooLarge,
}

#[cfg(test)]
#[derive(Copy, Clone, Debug)]
pub struct DecodeConfig {
    pub max_new_tokens: usize,
    pub eos_token_id: Option<u32>,
}

#[derive(Copy, Clone, Debug)]
pub struct SamplingConfig {
    pub greedy: bool,
    pub temperature: f32,
    pub top_k: Option<usize>,
    pub top_p: f32,
}

impl SamplingConfig {
    pub const fn greedy() -> Self {
        Self {
            greedy: true,
            temperature: 1.0,
            top_k: None,
            top_p: 1.0,
        }
    }

    #[allow(dead_code)]
    pub const fn sample(temperature: f32, top_k: Option<usize>, top_p: f32) -> Self {
        Self {
            greedy: false,
            temperature,
            top_k,
            top_p,
        }
    }
}

/// PCG32 XSH-RR, the minimal C PCG generator. It is deterministic and
/// non-cryptographic; do not use it for secrets. The algorithm and seed
/// sequence match Melissa O'Neill's `pcg-c-basic` reference implementation:
/// `pcg32_srandom_r` followed by `pcg32_random_r`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    pub fn seeded(initstate: u64, initseq: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (initseq << 1) | 1,
        };
        let _ = rng.next_u32();
        rng.state = rng.state.wrapping_add(initstate);
        let _ = rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let oldstate = self.state;
        self.state = oldstate
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot = (oldstate >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform-ish f32 in [0, 1), using the top 24 random bits so the largest
    /// draw cannot round up to 1.0 in f32.
    pub fn next_f32(&mut self) -> f32 {
        let bits = self.next_u32() >> 8;
        (bits as f32) * (1.0 / 16_777_216.0)
    }
}

#[derive(Copy, Clone)]
struct Candidate {
    token: u32,
    logit: f32,
    weight: f32,
    keep: bool,
}

pub fn argmax_token(logits: &[f32]) -> Result<u32, GenerateLogicError> {
    if logits.is_empty() {
        return Err(GenerateLogicError::EmptyLogits);
    }
    if logits.len() > u32::MAX as usize {
        return Err(GenerateLogicError::VocabTooLarge);
    }

    let mut best_token = 0usize;
    let mut best = logits[0];
    if !is_finite(best) {
        return Err(GenerateLogicError::InvalidLogit);
    }
    for (token, &logit) in logits.iter().enumerate().skip(1) {
        if !is_finite(logit) {
            return Err(GenerateLogicError::InvalidLogit);
        }
        if logit > best {
            best = logit;
            best_token = token;
        }
    }
    Ok(best_token as u32)
}

/// Greedy argmax constrained by a caller-supplied little-endian token-validity
/// bitmask. `None` is the kill-switch path and is deliberately identical to the
/// unconstrained greedy sampler.
pub fn masked_argmax_token(
    logits: &[f32],
    mask: Option<&[u8]>,
    mask_bits: usize,
) -> Result<u32, GenerateLogicError> {
    let Some(mask) = mask else {
        return argmax_token(logits);
    };
    if logits.is_empty() {
        return Err(GenerateLogicError::EmptyLogits);
    }
    if logits.len() > u32::MAX as usize {
        return Err(GenerateLogicError::VocabTooLarge);
    }
    if mask_bits != logits.len() {
        return Err(GenerateLogicError::InvalidMask);
    }
    let expected_mask_bytes = mask_bits
        .checked_add(7)
        .ok_or(GenerateLogicError::InvalidMask)?
        / 8;
    if mask.len() != expected_mask_bytes {
        return Err(GenerateLogicError::InvalidMask);
    }
    if let Some(&last) = mask.last()
        && !mask_bits.is_multiple_of(8)
    {
        let unused_mask = 0xffu8 << (mask_bits % 8);
        if last & unused_mask != 0 {
            return Err(GenerateLogicError::InvalidMask);
        }
    }

    let mut best_token = None;
    let mut best_logit = f32::NEG_INFINITY;
    for (token, &logit) in logits.iter().enumerate() {
        if !is_finite(logit) {
            return Err(GenerateLogicError::InvalidLogit);
        }
        if !mask_allows_token(mask, token) {
            continue;
        }
        if best_token.is_none() || logit > best_logit {
            best_token = Some(token);
            best_logit = logit;
        }
    }

    best_token
        .map(|token| token as u32)
        .ok_or(GenerateLogicError::InvalidMask)
}

/// Greedy argmax constrained to the next tokens that continue one of the
/// caller-provided candidate sequences.
///
/// This is the sampling-boundary primitive for grammar-guided decode: the
/// forward still produces the full logits vector, and the sampler mechanically
/// ignores every token that would leave the allowed candidate-prefix set.
pub fn constrained_argmax_token(
    logits: &[f32],
    generated_prefix: &[u32],
    candidate_sequences: &[&[u32]],
) -> Result<u32, GenerateLogicError> {
    if logits.is_empty() {
        return Err(GenerateLogicError::EmptyLogits);
    }
    if logits.len() > u32::MAX as usize {
        return Err(GenerateLogicError::VocabTooLarge);
    }
    for &logit in logits {
        if !is_finite(logit) {
            return Err(GenerateLogicError::InvalidLogit);
        }
    }

    let mut best_token = None;
    let mut best_logit = f32::NEG_INFINITY;
    for sequence in candidate_sequences {
        if sequence.len() <= generated_prefix.len() {
            continue;
        }
        let (prefix, suffix) = sequence.split_at(generated_prefix.len());
        if prefix != generated_prefix {
            continue;
        }
        let token = suffix[0];
        let token_idx = token as usize;
        if token_idx >= logits.len() {
            return Err(GenerateLogicError::InvalidShape);
        }
        let logit = logits[token_idx];
        let replace = match best_token {
            Some(best) => logit > best_logit || (logit == best_logit && token < best),
            None => true,
        };
        if replace {
            best_token = Some(token);
            best_logit = logit;
        }
    }

    best_token.ok_or(GenerateLogicError::InvalidProbability)
}

fn mask_allows_token(mask: &[u8], token: usize) -> bool {
    let byte = mask[token / 8];
    let bit = 1u8 << (token % 8);
    byte & bit != 0
}

pub fn sample_next_token(
    logits: &[f32],
    config: SamplingConfig,
    rng: &mut Pcg32,
) -> Result<u32, GenerateLogicError> {
    if config.greedy {
        return argmax_token(logits);
    }
    if logits.is_empty() {
        return Err(GenerateLogicError::EmptyLogits);
    }
    if logits.len() > u32::MAX as usize {
        return Err(GenerateLogicError::VocabTooLarge);
    }
    if !is_finite(config.temperature) || config.temperature <= 0.0 {
        return Err(GenerateLogicError::InvalidTemperature);
    }
    if !is_finite(config.top_p) || config.top_p <= 0.0 || config.top_p > 1.0 {
        return Err(GenerateLogicError::InvalidTopP);
    }
    if matches!(config.top_k, Some(0)) {
        return Err(GenerateLogicError::InvalidTopK);
    }

    let mut candidates = Vec::with_capacity(logits.len());
    for (token, &logit) in logits.iter().enumerate() {
        if !is_finite(logit) {
            return Err(GenerateLogicError::InvalidLogit);
        }
        candidates.push(Candidate {
            token: token as u32,
            logit: logit / config.temperature,
            weight: 0.0,
            keep: true,
        });
    }

    apply_top_k(&mut candidates, config.top_k);
    let mut total = fill_weights(&mut candidates)?;
    if config.top_p < 1.0 {
        apply_top_p(&mut candidates, total, config.top_p)?;
        total = candidates
            .iter()
            .filter(|c| c.keep)
            .fold(0.0f32, |sum, c| sum + c.weight);
        if !is_finite(total) || total <= 0.0 {
            return Err(GenerateLogicError::InvalidProbability);
        }
    }

    let threshold = rng.next_f32() * total;
    let mut cumulative = 0.0f32;
    let mut last_kept = None;
    for candidate in candidates.iter().filter(|c| c.keep) {
        last_kept = Some(candidate.token);
        cumulative += candidate.weight;
        if threshold < cumulative {
            return Ok(candidate.token);
        }
    }
    last_kept.ok_or(GenerateLogicError::InvalidProbability)
}

#[cfg(test)]
pub fn generate_tokens_with<F>(
    prompt: &[u32],
    decode: DecodeConfig,
    sampling: SamplingConfig,
    rng: &mut Pcg32,
    mut logits_for_context: F,
) -> Result<Vec<u32>, GenerateLogicError>
where
    F: FnMut(&[u32], &mut Vec<f32>) -> Result<(), GenerateLogicError>,
{
    if prompt.is_empty() {
        return Err(GenerateLogicError::EmptyPrompt);
    }
    let mut context = prompt.to_vec();
    let mut generated = Vec::new();
    for _ in 0..decode.max_new_tokens {
        let mut logits = Vec::new();
        logits_for_context(&context, &mut logits)?;
        let next = sample_next_token(&logits, sampling, rng)?;
        generated.push(next);
        context.push(next);
        if decode.eos_token_id == Some(next) {
            break;
        }
    }
    Ok(generated)
}

pub fn logits_byte_range(
    rank: usize,
    dims: [u64; 8],
    position: usize,
) -> Result<(usize, usize), GenerateLogicError> {
    match rank {
        1 => {
            let vocab = checked_usize(dims[0])?;
            let len = checked_mul(vocab, 4)?;
            Ok((0, len))
        }
        2 => {
            let seq = checked_usize(dims[0])?;
            let vocab = checked_usize(dims[1])?;
            if position >= seq {
                return Err(GenerateLogicError::InvalidPosition);
            }
            let row = checked_mul(position, vocab)?;
            let start = checked_mul(row, 4)?;
            let len = checked_mul(vocab, 4)?;
            let end = start
                .checked_add(len)
                .ok_or(GenerateLogicError::ByteLength)?;
            Ok((start, end))
        }
        _ => Err(GenerateLogicError::InvalidShape),
    }
}

pub fn read_logits_at_position(
    bytes: &[u8],
    rank: usize,
    dims: [u64; 8],
    position: usize,
) -> Result<Vec<f32>, GenerateLogicError> {
    let (start, end) = logits_byte_range(rank, dims, position)?;
    let row = bytes
        .get(start..end)
        .ok_or(GenerateLogicError::ByteLength)?;
    if row.len() % 4 != 0 {
        return Err(GenerateLogicError::ByteLength);
    }
    let mut logits = Vec::with_capacity(row.len() / 4);
    for chunk in row.chunks_exact(4) {
        logits.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(logits)
}

fn apply_top_k(candidates: &mut [Candidate], top_k: Option<usize>) {
    let Some(k) = top_k else {
        return;
    };
    if k >= candidates.len() {
        return;
    }
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| compare_candidates(&candidates[a], &candidates[b]));
    for idx in order.into_iter().skip(k) {
        candidates[idx].keep = false;
    }
}

fn apply_top_p(
    candidates: &mut [Candidate],
    total: f32,
    top_p: f32,
) -> Result<(), GenerateLogicError> {
    if !is_finite(total) || total <= 0.0 {
        return Err(GenerateLogicError::InvalidProbability);
    }
    let mut order: Vec<usize> = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, c)| c.keep.then_some(idx))
        .collect();
    order.sort_by(|&a, &b| compare_candidates(&candidates[a], &candidates[b]));

    for candidate in candidates.iter_mut() {
        candidate.keep = false;
    }

    let mut cumulative = 0.0f32;
    let mut kept_any = false;
    for idx in order {
        candidates[idx].keep = true;
        kept_any = true;
        cumulative += candidates[idx].weight;
        if cumulative / total >= top_p {
            break;
        }
    }
    if kept_any {
        Ok(())
    } else {
        Err(GenerateLogicError::InvalidProbability)
    }
}

fn fill_weights(candidates: &mut [Candidate]) -> Result<f32, GenerateLogicError> {
    soft_f32::debug_assert_kernel_mxcsr();
    let mut max = None;
    for candidate in candidates.iter().filter(|c| c.keep) {
        max = Some(match max {
            Some(current) if current >= candidate.logit => current,
            _ => candidate.logit,
        });
    }
    let max = max.ok_or(GenerateLogicError::InvalidProbability)?;

    let mut total = 0.0f32;
    for candidate in candidates.iter_mut().filter(|c| c.keep) {
        let shifted = candidate.logit - max;
        let weight = f32::from_bits(soft_f32::exp(shifted.to_bits()));
        if !is_finite(weight) || weight < 0.0 {
            return Err(GenerateLogicError::InvalidProbability);
        }
        candidate.weight = weight;
        total += weight;
    }
    if !is_finite(total) || total <= 0.0 {
        return Err(GenerateLogicError::InvalidProbability);
    }
    Ok(total)
}

fn compare_candidates(lhs: &Candidate, rhs: &Candidate) -> Ordering {
    match rhs.logit.partial_cmp(&lhs.logit).unwrap_or(Ordering::Equal) {
        Ordering::Equal => lhs.token.cmp(&rhs.token),
        order => order,
    }
}

fn is_finite(value: f32) -> bool {
    value.to_bits() & 0x7f80_0000 != 0x7f80_0000
}

fn checked_usize(value: u64) -> Result<usize, GenerateLogicError> {
    usize::try_from(value).map_err(|_| GenerateLogicError::InvalidShape)
}

fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, GenerateLogicError> {
    lhs.checked_mul(rhs).ok_or(GenerateLogicError::ByteLength)
}
