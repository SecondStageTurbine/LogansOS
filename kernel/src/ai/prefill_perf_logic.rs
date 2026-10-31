// SPDX-License-Identifier: MPL-2.0
//! The `prefill-perf` overlay's `perf` line, as a pure function of the
//! counters, so the host tests (`tools/ai-ops-tests`) check its format.
//! `prefill_perf::write_line` reads the counters and calls it.

use core::fmt::{self, Write};

/// One generation's counters, in TSC cycles where they are times.
#[derive(Clone, Copy, Debug, Default)]
pub struct PerfLine {
    pub tsc_hz: u64,
    pub prefill_tokens: u64,
    pub prefill_cycles: u64,
    pub decode_tokens: u64,
    pub decode_cycles: u64,
    /// The 24-layer stack and the `lm_head`: together, the forward.
    pub layers_cycles: u64,
    pub lm_head_cycles: u64,
    /// Quantized matvecs, fork-join included.
    pub matvec_cycles: u64,
    /// The SwiGLU element-wise step (`swiglu_vec`).
    pub swiglu_cycles: u64,
    /// The attention core per decode step: the KV-cache append plus the
    /// in-place `gqa_causal_attention_last_strided` (lever 1c).
    pub attn_cycles: u64,
    pub par_jobs: u64,
    pub par_barrier_cycles: u64,
    pub steals: u64,
    pub ap_parts: u64,
}

fn ms(cycles: u64, hz: u64) -> u64 {
    cycles.saturating_mul(1000) / hz.max(1)
}

/// `part` as a percentage of `whole`, clamped to 0..=100. The relaxed counter snapshot is not
/// transactional (a piece can be counted before its containing forward is), so a part can read
/// larger than its whole.
pub fn pct(part: u64, whole: u64) -> u64 {
    part.saturating_mul(100)
        .checked_div(whole)
        .unwrap_or(0)
        .min(100)
}

/// The largest count the line prints; anything above shows as this. It keeps the line within the
/// overlay's 192 bytes for any counter values (relay #68, item 5).
pub const DISPLAY_MAX: u64 = 9_999_999;

fn cap(n: u64) -> u64 {
    n.min(DISPLAY_MAX)
}

/// `perf pre n=… ms/t=… | dec n=… ms/t=… | lm=…% mv=…% sw=…% at=…% | par j=… bar=…% st=… ap=…`.
/// Every share is of the forward (layers + `lm_head`), clamped to 0..=100; what `mv`, `sw`,
/// `at` and `bar` leave is the rest of the serial work (norms, RoPE, residuals). Counts saturate
/// at [`DISPLAY_MAX`], so the line is at most 141 bytes.
pub fn write_perf_line(out: &mut dyn Write, l: &PerfLine) -> fmt::Result {
    let forward = l.layers_cycles.saturating_add(l.lm_head_cycles);
    write!(
        out,
        "perf pre n={} ms/t={} | dec n={} ms/t={} | lm={}% mv={}% sw={}% at={}% | par j={} bar={}% st={} ap={}",
        cap(l.prefill_tokens),
        cap(ms(l.prefill_cycles, l.tsc_hz) / l.prefill_tokens.max(1)),
        cap(l.decode_tokens),
        cap(ms(l.decode_cycles, l.tsc_hz) / l.decode_tokens.max(1)),
        pct(l.lm_head_cycles, forward),
        pct(l.matvec_cycles, forward),
        pct(l.swiglu_cycles, forward),
        pct(l.attn_cycles, forward),
        cap(l.par_jobs),
        pct(l.par_barrier_cycles, forward),
        cap(l.steals),
        cap(l.ap_parts)
    )
}
