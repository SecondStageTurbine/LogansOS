// SPDX-License-Identifier: MPL-2.0
//! The Zenbook timing mode's pure parts
//! (`Plans/DISPATCH-2026-11-09-seat-f-arthur-q8-zenbook-timing.md`): the order
//! the two models run in, the clock arithmetic, the medians and the exact
//! line formats J.c. reads off the screen.
//!
//! Allocation-free and free of `crate::` paths: host-tested through `#[path]`
//! by `tools/ai-ops-tests`. The kernel driver (`quant_timing.rs`) measures,
//! this file only orders and formats.

use core::fmt::{self, Write};

/// The reply cap (the dispatch: greedy, 32 tokens).
#[allow(dead_code)] // the timing image's; the format scenario runs 8
pub const MAX_NEW: usize = 32;

/// What the image's two models need in buddy blocks, measured by
/// `.github/workflows/model-pin.yml` (`neomc inspect`, run 42525785126), and
/// the Q8_0 module's size: the "need" half of the memory discriminator (the
/// census prints the "have"). The timing image's only.
#[allow(dead_code)]
pub const Q4_TENSOR_BYTES: u64 = 761_761_792;
#[allow(dead_code)]
pub const Q8_TENSOR_BYTES: u64 = 1_197_969_408;
#[allow(dead_code)]
pub const Q8_MODULE_BYTES: u64 = 669_799_936;

/// The two models' slots in run order for prompt `prompt` (0-based): Q4, Q8
/// for the first, Q8, Q4 for the second, and so on (Q4, Q8, Q8, Q4, Q4, Q8,
/// ...), so a drift with time (heat, caches) shows as a difference between a
/// prompt's first and second run instead of landing on one model.
pub const fn order(prompt: usize) -> [usize; 2] {
    if prompt.is_multiple_of(2) {
        [0, 1]
    } else {
        [1, 0]
    }
}

/// Microseconds in `cycles` at `hz`; 0 when the clock is uncalibrated.
pub const fn cycles_to_us(cycles: u64, hz: u64) -> u64 {
    if hz == 0 {
        return 0;
    }
    ((cycles as u128 * 1_000_000) / hz as u128) as u64
}

/// One prompt on one model: the time to the first token (a one-token
/// generation: the prefill and the first sample), the time of the full
/// generation, and how many tokens it produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub ttft_us: u64,
    pub total_us: u64,
    pub tokens: usize,
}

impl Run {
    /// The decode: the full generation less its first token.
    pub const fn decode_us(&self) -> u64 {
        self.total_us.saturating_sub(self.ttft_us)
    }

    /// Per generated token after the first; 0 with fewer than two tokens.
    pub const fn us_per_token(&self) -> u64 {
        if self.tokens < 2 {
            0
        } else {
            self.decode_us() / (self.tokens as u64 - 1)
        }
    }
}

/// The median of `values` (the lower middle one for an even count); 0 for
/// none. Sorts `values` in place.
pub fn median(values: &mut [u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

/// `us` as milliseconds with one decimal (`1234567` → `1234.5`), truncated.
pub struct Ms(pub u64);

impl fmt::Display for Ms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.0 / 1000, (self.0 % 1000) / 100)
    }
}

/// FNV-1a over the token ids' little-endian bytes: a run's tokens on one
/// screen line, to compare two runs of a prompt.
pub fn ids_fnv(ids: &[u32]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for id in ids {
        for b in id.to_le_bytes() {
            hash ^= u32::from(b);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    hash
}

/// The header: the clock, the CPUs and the matmul path.
pub fn write_header(
    w: &mut impl Write,
    tsc_hz: u64,
    pit_hz: u64,
    cpus: usize,
    avx2: &str,
    prompts: usize,
    max_new: usize,
) -> fmt::Result {
    write!(
        w,
        "TIMING header tsc_mhz={} pit_check_mhz={} cpus={} avx2={} prompts={} max_new={} order=alternating",
        tsc_hz / 1_000_000,
        pit_hz / 1_000_000,
        cpus,
        avx2,
        prompts,
        max_new
    )
}

/// One prompt on one model (`prompt` is printed 1-based), then its id, the
/// tokens' hash and, where the host's reply is known, whether it matched.
pub fn write_run(
    w: &mut impl Write,
    model: &str,
    prompt: usize,
    id: &str,
    run: &Run,
    ids: u32,
    host_match: Option<bool>,
) -> fmt::Result {
    write!(
        w,
        "TIMING model={} prompt={} prefill_ms={} tokens={} decode_ms={} ms_per_token={} id={} ids_fnv={:08x}",
        model,
        prompt + 1,
        Ms(run.ttft_us),
        run.tokens,
        Ms(run.decode_us()),
        Ms(run.us_per_token()),
        id,
        ids
    )?;
    match host_match {
        Some(true) => w.write_str(" host_match=yes"),
        Some(false) => w.write_str(" host_match=no"),
        None => Ok(()),
    }
}

/// One model's summary over its `runs` (in prompt order): the medians, and
/// its first and last prompt's ms/token (a climb from first to last on both
/// models is heat, not the weights).
pub fn write_summary(w: &mut impl Write, model: &str, runs: &[Run]) -> fmt::Result {
    let mut per_token = [0u64; 16];
    let mut ttft = [0u64; 16];
    let n = runs.len().min(per_token.len());
    for (i, run) in runs.iter().take(n).enumerate() {
        per_token[i] = run.us_per_token();
        ttft[i] = run.ttft_us;
    }
    let first = runs.first().map_or(0, Run::us_per_token);
    let last = runs.last().map_or(0, Run::us_per_token);
    write!(
        w,
        "TIMING summary model={} prompts={} median_ms_per_token={} median_ttft_ms={} first_ms_per_token={} last_ms_per_token={}",
        model,
        n,
        Ms(median(&mut per_token[..n])),
        Ms(median(&mut ttft[..n])),
        Ms(first),
        Ms(last)
    )
}
