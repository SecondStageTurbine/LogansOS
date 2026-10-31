// SPDX-License-Identifier: MPL-2.0
//! Grade both stock interpreters over the corpus and write the report.
//!
//! `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade -- [corpus-dir] [out-dir]`
//! (PowerShell: `$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`)
//! (defaults: `fixtures/acpi`, `tools/acpi-aml-grade/report`; run from the
//! repo root). Writes `coverage.json` and `coverage.md`.

use std::path::Path;

fn main() {
    // Both stock crates panic on firmware-reachable paths, and the harness
    // turns each panic into a classified failure with `catch_unwind`. The
    // workspace profiles set `panic = "abort"` (kernel needs), under which
    // the first crate panic would kill the run, so refuse to start.
    if cfg!(panic = "abort") {
        eprintln!(
            "acpi-aml-grade must be built with unwinding: run\n  \
             CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade\n\
             (PowerShell: $env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade)\n\
             (`cargo test -p acpi-aml-grade` always unwinds and checks the committed report)"
        );
        std::process::exit(2);
    }
    let args: Vec<String> = std::env::args().collect();
    let root = args.get(1).map(String::as_str).unwrap_or("fixtures/acpi");
    let out = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("tools/acpi-aml-grade/report");
    let sets = acpi_aml_grade::corpus::load_corpus(Path::new(root)).expect("read corpus");
    let run = acpi_aml_grade::summary::run(sets);
    std::fs::create_dir_all(out).expect("create report dir");
    let json = serde_json::to_string_pretty(&acpi_aml_grade::summary::to_json(&run)).unwrap();
    std::fs::write(Path::new(out).join("coverage.json"), json + "\n").expect("write coverage.json");
    std::fs::write(
        Path::new(out).join("coverage.md"),
        acpi_aml_grade::summary::to_markdown(&run),
    )
    .expect("write coverage.md");
    // The patched crate's regression floor moves only on request, and never down.
    if std::env::var_os("ACPI_GRADE_UPDATE_RATCHET").is_some() {
        let path = Path::new(out).join("ratchet.json");
        let now = acpi_aml_grade::ratchet::snapshot(&run, acpi_aml_grade::grade_acpi6::fix1::CRATE);
        if let Some(floor) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        {
            let lost: Vec<String> = acpi_aml_grade::ratchet::regressions(&floor, &now)
                .into_iter()
                .filter(|r| !r.contains("no floor recorded"))
                .collect();
            if !lost.is_empty() {
                eprintln!("refusing to lower the ratchet:");
                for l in &lost {
                    eprintln!("  {l}");
                }
                std::process::exit(1);
            }
        }
        let text = serde_json::to_string_pretty(&now).unwrap() + "\n";
        std::fs::write(&path, text).expect("write ratchet.json");
        eprintln!("ratchet updated: {}", path.display());
    }
    let unclassified = run
        .grades
        .iter()
        .flat_map(|g| &g.failures)
        .filter(|f| f.class == acpi_aml_grade::classify::Class::Unclassified)
        .count();
    eprintln!(
        "graded {} sets x {} crates; {unclassified} unclassified failures",
        run.sets.len(),
        acpi_aml_grade::summary::CRATES.len()
    );
    if unclassified > 0 {
        std::process::exit(1);
    }
}
