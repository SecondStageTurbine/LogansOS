// SPDX-License-Identifier: MPL-2.0
//! The rung-0 gate, as tests: the corpus is intact, both stock crates (and
//! `acpi` with the Fix 1 patches) run over every set end to end, every
//! failure is classified, and the committed coverage report is exactly what
//! the harness produces now.

use acpi_aml_grade::classify::Class;
use acpi_aml_grade::corpus::load_corpus;
use acpi_aml_grade::summary;
use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ratchet_floor() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("tools/acpi-aml-grade/report/ratchet.json"))
            .expect("tools/acpi-aml-grade/report/ratchet.json"),
    )
    .unwrap()
}

/// Corpus sets the committed ratchet names but the tree does not carry. The
/// real-machine sets are kept in the private repo only, so a public checkout
/// has just the QEMU sets: whole-corpus figures (the rebaseline, the committed
/// report) are checked only when nothing is missing, and per-set checks run
/// on every set that is present. The QEMU sets may never be missing.
fn missing_sets() -> Vec<String> {
    let floor = ratchet_floor();
    let missing: Vec<String> = floor["sets"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|n| !repo().join("fixtures/acpi").join(n).is_dir())
        .cloned()
        .collect();
    assert!(
        missing.iter().all(|n| !n.starts_with("qemu-")),
        "a QEMU corpus set is missing: {missing:?}"
    );
    if !missing.is_empty() {
        eprintln!("note: real-machine corpus sets absent (kept in the private repo): {missing:?}");
    }
    missing
}

#[test]
fn corpus_sets_are_intact_and_carry_provenance() {
    let sets = load_corpus(&repo().join("fixtures/acpi")).unwrap();
    assert!(!sets.is_empty(), "corpus is empty");
    assert!(
        sets.iter().any(|s| s.name.starts_with("qemu-q35-")),
        "the QEMU set is mandatory"
    );
    for s in &sets {
        assert!(
            s.forbidden.is_empty(),
            "{}: licence tables (MSDM/SLIC) must never be committed: {:?}",
            s.name,
            s.forbidden
        );
        assert_eq!(
            s.sums_ok,
            Some(true),
            "{}: SHA256SUMS {:?}",
            s.name,
            s.sums_detail
        );
        assert!(s.dsdt().is_some(), "{}: no DSDT", s.name);
        assert!(s.find("FACP").is_some(), "{}: no FADT", s.name);
        let prov: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(s.dir.join("provenance.json")).unwrap())
                .unwrap();
        for key in [
            "set",
            "machine",
            "firmware",
            "firmware_version",
            "extraction_tool",
            "extraction_os",
            "date",
        ] {
            assert!(
                prov.get(key).is_some(),
                "{}: provenance lacks `{key}`",
                s.name
            );
        }
        assert_eq!(
            prov["set"],
            s.name.as_str(),
            "{}: provenance names another set",
            s.name
        );
        // A delivered (ingested) set must account for every XSDT entry: a
        // table the dump left out is counted, never silently absent.
        if prov.get("ingested_by").is_some() && s.find("XSDT").is_some() {
            let x = &prov["xsdt_completeness"];
            assert!(x.is_object(), "{}: no xsdt_completeness record", s.name);
            let entries = x["xsdt_entries"].as_i64().unwrap();
            let sum = x["delivered_xsdt_tables"].as_i64().unwrap()
                + x["excluded_by_rule"].as_i64().unwrap()
                + x["missing_from_delivery"].as_i64().unwrap();
            assert_eq!(entries, sum, "{}: XSDT accounting does not add up", s.name);
        }
        // A bad table checksum on real firmware is a finding, not corpus
        // damage: it must match what provenance recorded at extraction.
        let recorded: std::collections::BTreeMap<String, bool> = prov["tables"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| {
                Some((
                    t["file"].as_str()?.to_string(),
                    t.get("checksum_ok")?.as_bool()?,
                ))
            })
            .collect();
        for t in &s.tables {
            if t.signature != "RSDP" && t.signature != "FACS" {
                let ok = t.declared_len() == t.bytes.len()
                    && t.bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b)) == 0;
                let want = recorded.get(&t.file).copied();
                assert_eq!(
                    Some(ok),
                    want,
                    "{}/{}: checksum vs provenance",
                    s.name,
                    t.file
                );
            }
        }
    }
}

#[test]
fn every_set_graded_by_every_crate_and_every_failure_classified() {
    let run = summary::run(load_corpus(&repo().join("fixtures/acpi")).unwrap());
    assert_eq!(run.grades.len(), run.sets.len() * summary::CRATES.len());
    for g in &run.grades {
        assert!(
            !g.evals.is_empty(),
            "{} / {}: nothing attempted",
            g.set,
            g.krate
        );
        for f in &g.failures {
            assert_ne!(
                f.class,
                Class::Unclassified,
                "{} / {}: unclassified failure at {} {}: {} ({})",
                g.set,
                g.krate,
                f.phase.name(),
                f.target,
                f.raw,
                f.detail
            );
            assert!(
                !f.key.is_empty(),
                "{} / {}: failure without a named mode",
                g.set,
                g.krate
            );
        }
    }
}

#[test]
fn fix1_gates_hold_against_the_rebaseline() {
    let run = summary::run(load_corpus(&repo().join("fixtures/acpi")).unwrap());
    let g = acpi_aml_grade::gates::gates(&run);
    let missing = missing_sets();
    if missing.is_empty() {
        assert!(
            g.rebaseline_holds(),
            "the stock column no longer reproduces the rebaseline: {:?}",
            g.stock
        );
    } else {
        // The rebaseline totals span the whole corpus; only its QEMU figure
        // can be checked without the real-machine sets.
        eprintln!("SKIP: whole-corpus rebaseline totals (real-machine sets absent)");
        assert_eq!(
            g.stock.qemu_wrong_accesses,
            acpi_aml_grade::gates::REBASELINE_QEMU_WRONG_ACCESSES,
            "the stock column no longer reproduces the QEMU rebaseline: {:?}",
            g.stock
        );
    }
    for (gate, stock, fix1, pass) in g.verdicts() {
        if gate == "OptiPlex DSDT loads" && missing.iter().any(|n| n.starts_with("optiplex-7080")) {
            eprintln!("SKIP gate `{gate}`: OptiPlex sets absent");
            continue;
        }
        assert!(
            pass,
            "Fix 1 gate failed: {gate}: stock {stock}, fix1 {fix1}"
        );
    }
    assert!(g.regressions.is_empty(), "{:?}", g.regressions);
}

#[test]
fn fix1_never_regresses() {
    let run = summary::run(load_corpus(&repo().join("fixtures/acpi")).unwrap());
    let mut floor = ratchet_floor();
    // A set absent from the tree is not graded; hold the present sets to
    // their floors.
    for name in missing_sets() {
        floor["sets"].as_object_mut().unwrap().remove(&name);
    }
    let now = acpi_aml_grade::ratchet::snapshot(&run, acpi_aml_grade::grade_acpi6::fix1::CRATE);
    let lost = acpi_aml_grade::ratchet::regressions(&floor, &now);
    assert!(
        lost.is_empty(),
        "acpi+fix1 regressed below its ratchet (raise it only with ACPI_GRADE_UPDATE_RATCHET=1, never lower it):\n  {}",
        lost.join("\n  ")
    );
}

#[test]
fn committed_report_matches_the_harness() {
    if !missing_sets().is_empty() {
        eprintln!("SKIP: the committed report covers the full corpus (real-machine sets absent)");
        return;
    }
    let run = summary::run(load_corpus(&repo().join("fixtures/acpi")).unwrap());
    let json = serde_json::to_string_pretty(&summary::to_json(&run)).unwrap() + "\n";
    let md = summary::to_markdown(&run);
    let dir = repo().join("tools/acpi-aml-grade/report");
    let hint = "regenerate from the repo root with `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade`";
    assert_eq!(
        std::fs::read_to_string(dir.join("coverage.json")).unwrap(),
        json,
        "coverage.json is stale; {hint}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("coverage.md")).unwrap(),
        md,
        "coverage.md is stale; {hint}"
    );
}

#[test]
fn licence_tables_are_refused_by_the_loader() {
    let dir = std::env::temp_dir().join(format!("acpi-aml-grade-msdm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut msdm = vec![0u8; 85];
    msdm[..4].copy_from_slice(b"MSDM");
    msdm[4..8].copy_from_slice(&85u32.to_le_bytes());
    std::fs::write(dir.join("msdm.dat"), &msdm).unwrap();
    let mut slic = msdm.clone();
    slic[..4].copy_from_slice(b"SLIC");
    // A Linux-style name: the signature, not the file name, decides.
    std::fs::write(dir.join("OEMX"), &slic).unwrap();
    let set = acpi_aml_grade::corpus::load_set(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(set.tables.is_empty(), "a licence table was loaded");
    assert_eq!(
        set.forbidden,
        vec!["OEMX (SLIC)".to_string(), "msdm.dat (MSDM)".to_string()]
    );
}
