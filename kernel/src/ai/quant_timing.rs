// SPDX-License-Identifier: MPL-2.0
//! The Zenbook timing mode: Arthur's reply speed with the shipped Q4_K_M and
//! with the Q8_0 beside it, in one boot
//! (`Plans/DISPATCH-2026-11-09-seat-f-arthur-q8-zenbook-timing.md`).
//!
//! Each prompt runs once on each model through the product's own generate
//! core (`real_lm_generate`: the persistent-KV greedy loop on the inference
//! stack, the path `sys_generate` takes), in the alternating order of
//! [`logic::order`]. A run is two generations:
//! - one token: the time to the first token (prefill and the first sample);
//! - [`logic::MAX_NEW`] tokens: the whole reply.
//!
//! The decode is the difference. Times come from the TSC at its calibrated
//! rate, printed in the header. Every line goes through `serial_println!`:
//! the boot console on screen (the Zenbook's evidence), and serial in QEMU.
//!
//! Built only with `arthur-quant-timing` (the timing image, never the product
//! image) or the format scenario's `neo-test-arthur-quant-timing`. Off: none
//! of this is compiled.

use super::quant_timing_logic as logic;
use crate::ai::model::Model;
use crate::serial_println;
use alloc::string::String;
use alloc::vec::Vec;

/// One of the two models: its label on the lines, and the model (`None`: it
/// did not load, and its runs are skipped with a line saying so).
pub struct Slot<'a> {
    pub label: &'static str,
    pub model: Option<&'a Model>,
    /// Compare its tokens with the prompt's expected reply.
    pub check: bool,
}

/// One prompt: an id for the line, its token ids, and the reply a checked
/// slot must give (`None`: nothing to compare).
pub struct Prompt<'a> {
    pub id: &'a str,
    pub ids: &'a [u32],
    pub expect: Option<&'a [u32]>,
}

/// What the runs found: how many ran, how many checked replies differed,
/// and whether any generation failed.
#[derive(Default)]
pub struct Outcome {
    pub runs: usize,
    pub mismatches: usize,
    pub failures: usize,
    /// Every run's tokens, per slot and prompt (for the scenario's checks).
    pub tokens: [Vec<Vec<u32>>; 2],
}

/// The calibrated TSC rate: the LAPIC timer's (the image's tick) or, without
/// it, the PIT calibration's; and the PIT one again as the cross-check.
pub fn tsc_hz() -> (u64, u64) {
    #[cfg(feature = "forward-perf")]
    let pit = crate::ai::forward_perf::tsc_hz();
    #[cfg(not(feature = "forward-perf"))]
    let pit = 0u64;
    #[cfg(feature = "lapic-timer")]
    let lapic = crate::arch::x86_64::lapic_timer::tsc_hz();
    #[cfg(not(feature = "lapic-timer"))]
    let lapic = 0u64;
    (if lapic != 0 { lapic } else { pit }, pit)
}

fn rdtsc() -> u64 {
    // SAFETY: RDTSC has no side effects.
    unsafe { core::arch::x86_64::_rdtsc() }
}

fn line(f: impl FnOnce(&mut String) -> core::fmt::Result) {
    let mut s = String::new();
    if f(&mut s).is_ok() {
        serial_println!("{}", s);
    }
}

/// Print the header, run every prompt on both slots in the alternating
/// order, print each run and each slot's summary.
pub fn run(slots: &[Slot<'_>; 2], prompts: &[Prompt<'_>], max_new: usize) -> Outcome {
    let (hz, pit) = tsc_hz();
    let avx2 = crate::ai::ops::qk_matmul::AVX2_CENSUS.decision();
    let avx2 = alloc::format!("{avx2:?}");
    line(|s| {
        logic::write_header(
            s,
            hz,
            pit,
            crate::arch::x86_64::smp::online_cpu_count(),
            &avx2,
            prompts.len(),
            max_new,
        )
    });
    let mut out = Outcome::default();
    let mut runs: [Vec<logic::Run>; 2] = [Vec::new(), Vec::new()];
    for (p, prompt) in prompts.iter().enumerate() {
        for s in logic::order(p) {
            let slot = &slots[s];
            let Some(model) = slot.model else {
                serial_println!(
                    "TIMING model={} prompt={} skipped=not_loaded",
                    slot.label,
                    p + 1
                );
                continue;
            };
            let generate = |n: usize| {
                crate::arch::x86_64::syscall::real_lm_generate(model, prompt.ids, n, None)
            };
            let t0 = rdtsc();
            let first = generate(1);
            let t1 = rdtsc();
            let full = generate(max_new);
            let t2 = rdtsc();
            let (first, tokens) = match (first, full) {
                (Ok((first, _, _)), Ok((tokens, _, _))) => (first, tokens),
                (Err(errno), _) | (_, Err(errno)) => {
                    serial_println!(
                        "TIMING model={} prompt={} failed errno={}",
                        slot.label,
                        p + 1,
                        errno
                    );
                    out.failures += 1;
                    continue;
                }
            };
            // The one-token run's token is the full run's first: the same
            // greedy loop on the same prompt.
            if first.first() != tokens.first() {
                serial_println!(
                    "TIMING model={} prompt={} first_token_differs one={:?} full={:?}",
                    slot.label,
                    p + 1,
                    first.first(),
                    tokens.first()
                );
                out.failures += 1;
            }
            let run = logic::Run {
                ttft_us: logic::cycles_to_us(t1 - t0, hz),
                total_us: logic::cycles_to_us(t2 - t1, hz),
                tokens: tokens.len(),
            };
            let host_match = match (slot.check, prompt.expect) {
                (true, Some(expect)) => Some(tokens.as_slice() == expect),
                _ => None,
            };
            if host_match == Some(false) {
                out.mismatches += 1;
            }
            line(|s| {
                logic::write_run(
                    s,
                    slot.label,
                    p,
                    prompt.id,
                    &run,
                    logic::ids_fnv(&tokens),
                    host_match,
                )
            });
            runs[s].push(run);
            out.tokens[s].push(tokens);
            out.runs += 1;
        }
    }
    for (s, slot) in slots.iter().enumerate() {
        line(|w| logic::write_summary(w, slot.label, &runs[s]));
    }
    out
}

/// The timing image: after `boot_modules::preload` built both models, time
/// them, then hold the screen (no desktop: the boot console is the evidence,
/// and nothing else may draw over it). Never returns; typed `()` so the boot
/// code after its call site stays reachable for every other build.
#[cfg(all(feature = "arthur-quant-timing", not(feature = "neo-test")))]
pub fn run_image() {
    use super::quant_timing_prompts::PROMPTS;
    use crate::boot_model_logic::{QWEN25_05B, QWEN25_05B_Q8_TIMING};
    use crate::boot_modules::preloaded;

    let load = |label: &str, module: &str| {
        let path = alloc::format!("/boot{module}");
        match preloaded(&path) {
            Some(Ok(model)) => {
                serial_println!("TIMING slot model={} path={} loaded=yes", label, module);
                Some(model)
            }
            Some(Err(errno)) => {
                serial_println!(
                    "TIMING slot model={} path={} loaded=no errno={}",
                    label,
                    module,
                    errno
                );
                None
            }
            None => {
                serial_println!(
                    "TIMING slot model={} path={} loaded=no reason=not_preloaded",
                    label,
                    module
                );
                None
            }
        }
    };
    let q4 = load("q4", QWEN25_05B.module);
    let q8 = load("q8", QWEN25_05B_Q8_TIMING.module);
    crate::mm::boot_census("timing_models_built");
    let slots = [
        Slot {
            label: "q4",
            model: q4.as_deref(),
            check: true,
        },
        Slot {
            label: "q8",
            model: q8.as_deref(),
            check: false,
        },
    ];
    // The legend: what each prompt number is, as typed.
    for (n, p) in PROMPTS.iter().enumerate() {
        serial_println!(
            "TIMING prompt={} id={} tokens={} text={:?}",
            n + 1,
            p.id,
            p.ids.len(),
            p.text
        );
    }
    let prompts: Vec<Prompt<'_>> = PROMPTS
        .iter()
        .map(|p| Prompt {
            id: p.id,
            ids: p.ids,
            expect: Some(p.q4_host.as_slice()),
        })
        .collect();
    let out = run(&slots, &prompts, logic::MAX_NEW);
    serial_println!(
        "TIMING done runs={} q4_host_mismatches={} failures={} (photograph this screen)",
        out.runs,
        out.mismatches,
        out.failures
    );
    loop {
        x86_64::instructions::hlt();
    }
}
