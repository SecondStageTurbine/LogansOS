// SPDX-License-Identifier: MPL-2.0
//! On-demand regression KAT for the shipped Q4_K_M forward pass.
//!
//! Hermetic: skips cleanly (like `real_model_*_when_available`) unless
//! `QUANT_DRIFT_NEOM` points at the real `model-qwen2.5-0.5b.neom` with its
//! `.vocab` sidecar beside it. Then it re-runs the SAME prompts and compares
//! them with `fixtures/quant-drift-kat-2026-11-08.ours.tsv`, the pinned output
//! of the 2026-11-08 quantization-drift run. A forward-pass change that moves a
//! token fails here instead of being re-blessed by review.
//!
//! `QUANT_DRIFT_CORPUS=corpus.u32` additionally checks the pinned PPL window
//! (the KAT's `corpus_tokens` / `corpus_eval_start`) within 0.5% relative. The
//! corpus text is downloaded, never committed.

use std::path::{Path, PathBuf};

use quant_drift_tests::model::Model;
use quant_drift_tests::prompts::PROMPTS;
use quant_drift_tests::report::RefRun;

const MAX_NEW: usize = 32;
const BOOT_PROMPT: [u32; 21] = [
    151644, 8948, 198, 2610, 525, 264, 10950, 17847, 13, 151645, 198, 151644, 872, 198, 14990,
    1879, 151645, 198, 151644, 77091, 198,
];

fn kat_path() -> PathBuf {
    std::env::var("QUANT_DRIFT_KAT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/quant-drift-kat-2026-11-08-control.ours.tsv")
        })
}

#[test]
fn quant_drift_kat_when_available() {
    let Ok(neom_path) = std::env::var("QUANT_DRIFT_NEOM") else {
        eprintln!("SKIP quant_drift_kat_when_available: QUANT_DRIFT_NEOM unset");
        return;
    };
    let neom_path = PathBuf::from(neom_path);
    let (model, vocab) = Model::load(&neom_path).expect("load real .neom");

    let kat_text = std::fs::read_to_string(kat_path()).expect("read KAT reference run");
    let kat = RefRun::from_tsv(&kat_text).expect("parse KAT reference run");
    assert_eq!(
        kat.prompts.len(),
        PROMPTS.len(),
        "KAT covers the fixed prompt set"
    );

    let mut generated_mismatches = Vec::new();
    let mut top1_mismatches = 0usize;
    let mut top1_positions = 0usize;
    for prompt in PROMPTS {
        let ids = model
            .frame_prompt(&vocab, prompt.text)
            .expect("frame prompt");
        let expected = kat.prompt(prompt.id).expect("KAT prompt");
        assert_eq!(ids, expected.ids, "prompt {} token IDs drifted", prompt.id);
        if prompt.id == "boot_hello_world" {
            assert_eq!(ids.as_slice(), &BOOT_PROMPT, "the 21-token boot prompt");
        }

        let generated = model.greedy(&ids, MAX_NEW);
        if generated != expected.generated {
            let position = generated
                .iter()
                .zip(&expected.generated)
                .position(|(got, want)| got != want)
                .unwrap_or(0);
            generated_mismatches.push(format!(
                "{} at index {position}: got {} want {}",
                prompt.id, generated[position], expected.generated[position]
            ));
        }

        let top1 = model.top1_per_position(&ids);
        assert_eq!(top1.len(), expected.top1.len(), "{} top1 length", prompt.id);
        for (got, want) in top1.iter().zip(&expected.top1) {
            top1_positions += 1;
            if got != want {
                top1_mismatches += 1;
            }
        }
        eprintln!(
            "quant-drift {}: prompt={} generated={:?}",
            prompt.id,
            ids.len(),
            generated
        );
    }
    assert!(
        generated_mismatches.is_empty(),
        "greedy decode drifted from the pinned KAT:\n{}",
        generated_mismatches.join("\n")
    );
    assert_eq!(
        top1_mismatches, 0,
        "teacher-forced top-1 drifted from the pinned KAT: {top1_mismatches}/{top1_positions}"
    );

    if let Ok(corpus_path) = std::env::var("QUANT_DRIFT_CORPUS") {
        let bytes = std::fs::read(&corpus_path).expect("read corpus tokens");
        assert_eq!(bytes.len() % 4, 0, "corpus is u32 tokens");
        let corpus: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        assert!(
            corpus.len() >= kat.corpus_window_offset + kat.corpus_tokens,
            "corpus has {} tokens, KAT needs offset {} + {}",
            corpus.len(),
            kat.corpus_window_offset,
            kat.corpus_tokens
        );
        let window =
            &corpus[kat.corpus_window_offset..kat.corpus_window_offset + kat.corpus_tokens];
        let (nll_sum, nll_count) = model.nll_sum(window, kat.corpus_eval_start);
        assert_eq!(nll_count, kat.corpus_nll_count, "corpus NLL target count");
        let ppl = (nll_sum / nll_count as f64).exp();
        let expected_ppl = kat.perplexity();
        let delta = (ppl - expected_ppl).abs() / expected_ppl;
        eprintln!(
            "quant-drift corpus: PPL {ppl:.6} (KAT {expected_ppl:.6}, Δ {:.4}%)",
            delta * 100.0
        );
        assert!(
            delta <= 0.005,
            "corpus PPL drifted from the pinned KAT: {ppl:.6} vs {expected_ppl:.6} ({:.4}%)",
            delta * 100.0
        );
    }
}
