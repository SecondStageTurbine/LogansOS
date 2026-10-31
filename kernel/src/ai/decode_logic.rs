// SPDX-License-Identifier: MPL-2.0
//! The per-token decode step and the prefill driver, kernel-free.
//!
//! `qwen_inventory::InventoryDecodeState` advances through exactly this code;
//! the host crate `tools/ai-ops-tests` mounts the same file and drives it with
//! the real `forward_logic` layers at Qwen2.5-0.5B shapes. So the prefill
//! lm_head skip (Arthur latency NOTE, fix 3) is proven on the code that ships:
//! a step that skipped a layer, a cache write or the position bump when it skips
//! the logits fails the host KAT (review item 18).
//!
//! The kernel's adapter ([`TokenForward`]) is the embedding + layer loop and
//! the norm + lm_head, the same two calls on every path; nothing about whether
//! logits are wanted reaches it.

extern crate alloc;

use alloc::vec::Vec;

/// One token's forward, split where the prefill skip cuts it.
pub trait TokenForward {
    type Error;
    /// Embed `token` and run every layer at `position`, writing each layer's
    /// K/V (and recurrent) state. Returns the final hidden state.
    fn layers(&mut self, token: u32, position: usize) -> Result<Vec<f32>, Self::Error>;
    /// The output projection of a final hidden state: norm + lm_head.
    fn output(&mut self, hidden: &[f32]) -> Result<Vec<f32>, Self::Error>;
}

/// Where the decode is: the next position, and the context bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeCursor {
    position: usize,
    max_context: usize,
}

impl DecodeCursor {
    pub const fn new(max_context: usize) -> Self {
        Self {
            position: 0,
            max_context,
        }
    }

    /// Positions consumed so far (tokens processed).
    pub const fn position(&self) -> usize {
        self.position
    }

    /// One token: the layer stack always, the output projection only when
    /// `want_logits`, then the next position. Past the context bound it fails
    /// with `full` and changes nothing.
    pub fn step<F: TokenForward>(
        &mut self,
        forward: &mut F,
        token: u32,
        want_logits: bool,
        full: F::Error,
    ) -> Result<Option<Vec<f32>>, F::Error> {
        if self.position >= self.max_context {
            return Err(full);
        }
        let hidden = forward.layers(token, self.position)?;
        let logits = if want_logits {
            Some(forward.output(&hidden)?)
        } else {
            None
        };
        self.position += 1;
        Ok(logits)
    }
}

/// Drive a prompt through `step(token, want_logits)`, asking for logits only on
/// the last token. Every token advances the state identically; the earlier ones
/// skip the output projection, whose result prefill never used. Returns the
/// last token's logits, or an empty vector for an empty prompt (what the
/// per-token loop returned).
pub fn prefill_last_logits<E>(
    prompt: &[u32],
    mut step: impl FnMut(u32, bool) -> Result<Option<Vec<f32>>, E>,
) -> Result<Vec<f32>, E> {
    let mut logits = Vec::new();
    for (idx, &token) in prompt.iter().enumerate() {
        let last = idx + 1 == prompt.len();
        if let Some(out) = step(token, last)? {
            logits = out;
        }
    }
    Ok(logits)
}
