// SPDX-License-Identifier: MPL-2.0
//! The `prefill-perf` overlay's `perf` line: format and the new `sw=`/`at=`
//! shares (Plans/NOTE-2026-10-25-arthur-lever2-design.md, the go/no-go for
//! lever 2 reads them off A8's photo).

use super::prefill_perf_logic::{PerfLine, pct, write_perf_line};

fn line(l: &PerfLine) -> String {
    let mut s = String::new();
    write_perf_line(&mut s, l).unwrap();
    s
}

#[test]
fn perf_line_reports_every_share_of_the_forward() {
    // 1 GHz: 1 ms = 1,000,000 cycles. Forward = 900 + 100 = 1000 units.
    let l = PerfLine {
        tsc_hz: 1_000_000_000,
        prefill_tokens: 25,
        prefill_cycles: 25 * 208_000_000,
        decode_tokens: 7,
        decode_cycles: 7 * 304_000_000,
        layers_cycles: 900_000,
        lm_head_cycles: 100_000,
        matvec_cycles: 170_000,
        swiglu_cycles: 120_000,
        attn_cycles: 350_000,
        par_jobs: 3848,
        par_barrier_cycles: 30_000,
        steals: 0,
        ap_parts: 57_720,
    };
    assert_eq!(
        line(&l),
        "perf pre n=25 ms/t=208 | dec n=7 ms/t=304 | lm=10% mv=17% sw=12% at=35% | par j=3848 bar=3% st=0 ap=57720"
    );
}

#[test]
fn perf_line_before_any_generation_is_all_zero_and_does_not_divide_by_zero() {
    assert_eq!(
        line(&PerfLine::default()),
        "perf pre n=0 ms/t=0 | dec n=0 ms/t=0 | lm=0% mv=0% sw=0% at=0% | par j=0 bar=0% st=0 ap=0"
    );
}

/// Relay #68, item 5: the line stays within the overlay's 192-byte `watchdog::Line` for every
/// input, including counters the relaxed snapshot can leave inconsistent (a part larger than the
/// forward). Every field at `u64::MAX`, then Chad's counterexample (whole of 1), which printed
/// 260 bytes before the clamp.
#[test]
fn perf_line_fits_the_overlay_line_for_every_input() {
    let m = u64::MAX;
    let all_max = PerfLine {
        tsc_hz: m,
        prefill_tokens: m,
        prefill_cycles: m,
        decode_tokens: m,
        decode_cycles: m,
        layers_cycles: m,
        lm_head_cycles: m,
        matvec_cycles: m,
        swiglu_cycles: m,
        attn_cycles: m,
        par_jobs: m,
        par_barrier_cycles: m,
        steals: m,
        ap_parts: m,
    };
    let chad = PerfLine {
        tsc_hz: 1,
        layers_cycles: 1,
        lm_head_cycles: 0,
        ..all_max
    };
    // Every field at its widest printed form: seven-digit counts and ms/t, three-digit shares.
    let widest_input = PerfLine {
        tsc_hz: 1,
        prefill_tokens: 9_999_999,
        decode_tokens: 9_999_999,
        layers_cycles: 0,
        lm_head_cycles: 1,
        ..all_max
    };
    let widest = "perf pre n=9999999 ms/t=9999999 | dec n=9999999 ms/t=9999999 | lm=100% mv=100% sw=100% at=100% | par j=9999999 bar=100% st=9999999 ap=9999999";
    assert_eq!(line(&widest_input), widest);
    assert_eq!(
        line(&chad),
        "perf pre n=9999999 ms/t=1 | dec n=9999999 ms/t=1 | lm=0% mv=100% sw=100% at=100% | par j=9999999 bar=100% st=9999999 ap=9999999"
    );
    for l in [all_max, chad, widest_input] {
        let s = line(&l);
        assert!(s.len() <= widest.len(), "{} bytes: {s}", s.len());
    }
    assert!(widest.len() <= 192, "{}", widest.len());
}

#[test]
fn pct_rounds_down_and_tolerates_a_zero_whole() {
    assert_eq!(pct(1, 3), 33);
    assert_eq!(pct(5, 0), 0);
    assert_eq!(pct(3, 3), 100);
    assert_eq!(pct(5, 1), 100); // a part counted before its whole: clamped
}
