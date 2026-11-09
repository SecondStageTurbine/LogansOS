// SPDX-License-Identifier: MPL-2.0
//! The Zenbook timing mode's order, arithmetic and line formats
//! (`kernel/src/ai/quant_timing_logic.rs`), and its prompt fixture
//! (`kernel/src/ai/quant_timing_prompts.rs`) against the committed KAT.

use super::quant_timing_logic::*;
use super::quant_timing_prompts::PROMPTS;

#[test]
fn the_order_alternates_q4_q8_q8_q4() {
    let seq: Vec<usize> = (0..5).flat_map(order).collect();
    assert_eq!(seq, [0, 1, 1, 0, 0, 1, 1, 0, 0, 1]);
}

#[test]
fn clock_arithmetic_and_ms_format() {
    assert_eq!(cycles_to_us(3_000_000_000, 3_000_000_000), 1_000_000);
    assert_eq!(cycles_to_us(4_500, 3_000_000_000), 1);
    assert_eq!(cycles_to_us(123, 0), 0);
    assert_eq!(format!("{}", Ms(1_234_567)), "1234.5");
    assert_eq!(format!("{}", Ms(999)), "0.9");
    assert_eq!(format!("{}", Ms(0)), "0.0");
    let run = Run {
        ttft_us: 500_000,
        total_us: 3_600_000,
        tokens: 32,
    };
    assert_eq!(run.decode_us(), 3_100_000);
    assert_eq!(run.us_per_token(), 100_000);
    assert_eq!(Run { tokens: 1, ..run }.us_per_token(), 0);
    // A total shorter than its first token (clock noise) never underflows.
    assert_eq!(Run { total_us: 1, ..run }.decode_us(), 0);
}

#[test]
fn median_is_the_lower_middle() {
    assert_eq!(median(&mut [5, 1, 3]), 3);
    assert_eq!(median(&mut [4, 1, 3, 2]), 2);
    assert_eq!(median(&mut []), 0);
}

#[test]
fn lines_have_the_dispatch_format() {
    let mut s = String::new();
    write_header(&mut s, 3_100_000_000, 3_099_000_000, 16, "Yes", 5, 32).unwrap();
    assert_eq!(
        s,
        "TIMING header tsc_mhz=3100 pit_check_mhz=3099 cpus=16 avx2=Yes prompts=5 max_new=32 order=alternating"
    );
    let run = Run {
        ttft_us: 812_345,
        total_us: 4_012_345,
        tokens: 32,
    };
    let mut s = String::new();
    write_run(&mut s, "q8", 1, "desktop_list", &run, 0xdead_beef, None).unwrap();
    assert_eq!(
        s,
        "TIMING model=q8 prompt=2 prefill_ms=812.3 tokens=32 decode_ms=3200.0 ms_per_token=103.2 id=desktop_list ids_fnv=deadbeef"
    );
    let mut s = String::new();
    write_run(&mut s, "q4", 0, "x", &run, 1, Some(true)).unwrap();
    assert!(s.ends_with(" ids_fnv=00000001 host_match=yes"), "{s}");
    let runs = [
        run,
        Run {
            ttft_us: 900_000,
            total_us: 4_000_000,
            tokens: 32,
        },
        Run {
            ttft_us: 700_000,
            total_us: 5_350_000,
            tokens: 32,
        },
    ];
    let mut s = String::new();
    write_summary(&mut s, "q4", &runs).unwrap();
    assert_eq!(
        s,
        "TIMING summary model=q4 prompts=3 median_ms_per_token=103.2 median_ttft_ms=812.3 first_ms_per_token=103.2 last_ms_per_token=150.0"
    );
}

#[test]
fn ids_fnv_is_fnv1a_over_le_bytes() {
    // FNV-1a of no bytes is its offset basis; of [1u32] the four bytes 01 00 00 00.
    assert_eq!(ids_fnv(&[]), 0x811c_9dc5);
    let mut h = 0x811c_9dc5u32;
    for b in [1u8, 0, 0, 0] {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    assert_eq!(ids_fnv(&[1]), h);
    assert_ne!(ids_fnv(&[1, 2]), ids_fnv(&[2, 1]));
}

/// The fixture is the KAT's data, read again here from the committed JSON.
#[test]
fn prompts_are_the_kat_prompts() {
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../quant-drift-tests/fixtures/quant-drift-kat-2026-11-08.json"
    ))
    .unwrap();
    let ids = |key: &str, field: &str| -> Vec<u32> {
        let at = json.find(&format!("\"{key}\": {{\"prompt_ids\"")).unwrap();
        let rest = &json[at..];
        let f = rest.find(&format!("\"{field}\": [")).unwrap() + field.len() + 5;
        let end = rest[f..].find(']').unwrap();
        rest[f..f + end]
            .split(',')
            .map(|t| t.trim().parse().unwrap())
            .collect()
    };
    assert_eq!(PROMPTS.len(), 5);
    for p in &PROMPTS {
        assert_eq!(p.ids, ids(p.id, "prompt_ids").as_slice(), "{}", p.id);
        assert_eq!(
            p.q4_host.as_slice(),
            ids(p.id, "generated_ids").as_slice(),
            "{}",
            p.id
        );
        // The chat template's user turn holds the typed text's tokens.
        assert_eq!(
            &p.ids[..11],
            &[
                151644, 8948, 198, 2610, 525, 264, 10950, 17847, 13, 151645, 198
            ]
        );
    }
    assert_eq!(MAX_NEW, PROMPTS[0].q4_host.len());
}
