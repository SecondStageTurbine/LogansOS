// SPDX-License-Identifier: MPL-2.0
//! Corpus-level roll-up: the headline metrics, the ranked gap list, and the
//! corpus manifest (which sets are present, which reference machines are
//! still missing). Rendered as JSON (machine-readable, the gate's evidence)
//! and Markdown (for the findings doc).

use crate::classify::Class;
use crate::corpus::CorpusSet;
use crate::grade_acpi6::{fix1, stock};
use crate::grade_aml016;
use crate::report::{CrateGrade, Phase};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write;

/// The reference machines every complete corpus must cover. A set counts
/// for a machine when its directory name starts with the prefix.
pub const REFERENCE_MACHINES: &[(&str, &str)] = &[
    ("qemu-q35-", "QEMU q35 (SeaBIOS / OVMF) - always available"),
    (
        "zenbook-s14-ux5406aa",
        "ASUS Zenbook S14 UX5406AA (Windows 11 dual-boot; write-guard target)",
    ),
    (
        "optiplex-7080",
        "Dell OptiPlex 7080 (bare-metal Arthur target)",
    ),
    (
        "lenovo-thinkcentre-m710q",
        "Lenovo ThinkCentre M710q (Gen9 GPU target): not delivered, the PL does not have it (2026-10-22)",
    ),
];

/// Headline groups. "Attempted" counts differ by crate because a table that
/// fails to load contributes no `_STA`/`_INI` to that crate's namespace, so
/// the percentages are read beside the attempted counts, never alone.
type Pick = fn(&str) -> bool;
pub const GROUPS: [(&str, Pick); 3] = [
    ("all sets", |_| true),
    ("QEMU", |s| s.starts_with("qemu-")),
    ("real hardware", |s| !s.starts_with("qemu-")),
];

/// The graded crates, in report order: the two stock crates, then `acpi`
/// with the Fix 1 patches (OT-86 L3).
pub const CRATES: [(&str, &str); 3] = [
    (grade_aml016::CRATE, grade_aml016::VERSION),
    (stock::CRATE, stock::VERSION),
    (fix1::CRATE, fix1::VERSION),
];

pub struct Run {
    pub sets: Vec<CorpusSet>,
    pub grades: Vec<CrateGrade>,
}

pub fn run(sets: Vec<CorpusSet>) -> Run {
    let mut grades = Vec::new();
    for s in &sets {
        grades.push(grade_aml016::grade(s));
        grades.push(stock::grade(s));
        grades.push(fix1::grade(s));
    }
    Run { sets, grades }
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        "n/a".into()
    } else {
        format!("{:.1}%", 100.0 * n as f64 / d as f64)
    }
}

#[derive(Default)]
struct CrateTotals {
    sets: usize,
    dsdt_clean: usize,
    all_clean: usize,
    evals: usize,
    completed: usize,
    completed_clean: usize,
    by_class: BTreeMap<&'static str, usize>,
}

fn totals(grades: &[&CrateGrade]) -> CrateTotals {
    let mut t = CrateTotals::default();
    for g in grades {
        t.sets += 1;
        t.dsdt_clean += g.dsdt_clean() as usize;
        t.all_clean += g.all_tables_clean() as usize;
        for (i, e) in g.evals.iter().enumerate() {
            if e.phase != Phase::Sta && e.phase != Phase::Ini {
                continue;
            }
            t.evals += 1;
            t.completed += e.ok as usize;
            let tainted = g.failures.iter().any(|f| f.eval == i);
            t.completed_clean += (e.ok && !tainted) as usize;
        }
        // Device probes are reported in their own table, not ranked.
        for f in g.failures.iter().filter(|f| f.phase != Phase::Probe) {
            *t.by_class.entry(f.class.name()).or_default() += 1;
        }
    }
    t
}

struct Gap {
    krate: &'static str,
    class: Class,
    key: String,
    count: usize,
    sets: Vec<String>,
    example: String,
}

fn gaps(grades: &[CrateGrade]) -> Vec<Gap> {
    let mut m: BTreeMap<(&'static str, Class, String), Gap> = BTreeMap::new();
    for g in grades {
        // Device probes are reported in their own table, not ranked.
        for f in g.failures.iter().filter(|f| f.phase != Phase::Probe) {
            let e = m
                .entry((g.krate, f.class, f.key.clone()))
                .or_insert_with(|| Gap {
                    krate: g.krate,
                    class: f.class,
                    key: f.key.clone(),
                    count: 0,
                    sets: Vec::new(),
                    example: format!("{} {} - {}", f.phase.name(), f.target, f.detail),
                });
            e.count += 1;
            if !e.sets.contains(&g.set) {
                e.sets.push(g.set.clone());
            }
        }
    }
    let mut v: Vec<Gap> = m.into_values().collect();
    v.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.krate.cmp(b.krate))
            .then(a.key.cmp(&b.key))
    });
    v
}

fn provenance(set: &CorpusSet) -> Value {
    std::fs::read_to_string(set.dir.join("provenance.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

pub fn to_json(r: &Run) -> Value {
    let crates = CRATES;
    let mut headline = Vec::new();
    for (group, pick) in GROUPS {
        for (c, ver) in crates {
            let gs: Vec<&CrateGrade> = r
                .grades
                .iter()
                .filter(|g| g.krate == c && pick(&g.set))
                .collect();
            if gs.is_empty() {
                continue;
            }
            let t = totals(&gs);
            headline.push(json!({
                "group": group, "crate": c, "version": ver,
                "sets": t.sets,
                "dsdt_parse_clean": {"sets": t.dsdt_clean, "pct": pct(t.dsdt_clean, t.sets)},
                "all_tables_parse_clean": {"sets": t.all_clean, "pct": pct(t.all_clean, t.sets)},
                "sta_ini_evaluations": {
                    "attempted": t.evals,
                    "completed": t.completed, "completed_pct": pct(t.completed, t.evals),
                    "completed_without_any_failure": t.completed_clean,
                    "completed_without_any_failure_pct": pct(t.completed_clean, t.evals),
                },
                "failures_by_class": t.by_class,
            }));
        }
    }
    let machines: Vec<Value> = REFERENCE_MACHINES
        .iter()
        .map(|(prefix, label)| {
            let present: Vec<&str> =
                r.sets.iter().filter(|s| s.name.starts_with(prefix)).map(|s| s.name.as_str()).collect();
            json!({"machine": label, "set_prefix": prefix, "present": !present.is_empty(), "sets": present})
        })
        .collect();
    let other: Vec<&str> = r
        .sets
        .iter()
        .filter(|s| {
            !REFERENCE_MACHINES
                .iter()
                .any(|(p, _)| s.name.starts_with(p))
        })
        .map(|s| s.name.as_str())
        .collect();
    json!({
        "rung": "ACPI/AML rung 0 - stock-crate grade",
        "pinned": crates.iter().map(|(c, v)| format!("{c} ={v}")).collect::<Vec<_>>(),
        "scripted_model": "zero-initialised byte-granular write-back store per address space; 1 ms fake clock per query; mutexes always granted",
        "access_budget_per_evaluation": crate::recorder::ACCESS_BUDGET,
        "corpus": {
            "reference_machines": machines,
            "other_project_machines": other,
            "sets": r.sets.iter().map(|s| json!({
                "set": s.name,
                "tables": s.tables.iter().map(|t| format!("{} ({}, {} B)", t.file, t.signature, t.bytes.len())).collect::<Vec<_>>(),
                "sha256sums_verified": s.sums_ok,
                "sha256sums_problems": s.sums_detail.iter().filter(|d| *d != "no SHA256SUMS").collect::<Vec<_>>(),
                "provenance": provenance(s),
            })).collect::<Vec<_>>(),
        },
        "headline": headline,
        "fix1_gates": crate::gates::gates(r).to_json(),
        "ranked_gaps": gaps(&r.grades).iter().enumerate().map(|(i, g)| json!({
            "rank": i + 1, "crate": g.krate, "class": g.class.name(), "mode": g.key,
            "occurrences": g.count, "sets": g.sets, "example": g.example,
        })).collect::<Vec<_>>(),
        "per_set": r.grades.iter().map(|g| g.to_json()).collect::<Vec<_>>(),
    })
}

pub fn to_markdown(r: &Run) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "<!-- SPDX-License-Identifier: CC-BY-4.0 -->");
    let _ = writeln!(
        s,
        "<!-- GENERATED by `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` - do not edit; the test suite checks it. -->"
    );
    let _ = writeln!(s, "# ACPI/AML rung 0 - coverage report\n");
    let _ = writeln!(
        s,
        "Pinned stock crates: `aml ={}`, `acpi ={}` (unmodified). `acpi+fix1` is `acpi` {} vendored in `third_party/acpi-loganfy` with the OT-86 Fix 1 patches.\n",
        grade_aml016::VERSION,
        stock::VERSION,
        fix1::VERSION
    );
    let _ = writeln!(s, "## Corpus\n");
    let _ = writeln!(s, "| Reference machine | Present | Sets |\n|---|---|---|");
    for (prefix, label) in REFERENCE_MACHINES {
        let present: Vec<&str> = r
            .sets
            .iter()
            .filter(|x| x.name.starts_with(prefix))
            .map(|x| x.name.as_str())
            .collect();
        let _ = writeln!(
            s,
            "| {label} | {} | {} |",
            if present.is_empty() {
                "**MISSING**"
            } else {
                "yes"
            },
            if present.is_empty() {
                "-".to_string()
            } else {
                present.join(", ")
            }
        );
    }
    let other: Vec<&str> = r
        .sets
        .iter()
        .filter(|x| {
            !REFERENCE_MACHINES
                .iter()
                .any(|(p, _)| x.name.starts_with(p))
        })
        .map(|x| x.name.as_str())
        .collect();
    let _ = writeln!(
        s,
        "| Other project-owned machines | {} | {} |",
        if other.is_empty() { "none yet" } else { "yes" },
        if other.is_empty() {
            "-".to_string()
        } else {
            other.join(", ")
        }
    );
    let _ = writeln!(
        s,
        "\n| Set | Tables | SHA256SUMS | XSDT entries / delivered / excluded by rule / **missing** | SSDT numbering gaps |\n|---|---|---|---|---|"
    );
    for set in &r.sets {
        let sigs: Vec<&str> = set.tables.iter().map(|t| t.signature.as_str()).collect();
        let x = &provenance(set)["xsdt_completeness"];
        let (xsdt, gaps) = if x.is_object() {
            let missing = x["missing_from_delivery"].as_i64().unwrap_or(0);
            (
                format!(
                    "{} / {} / {} / {}",
                    x["xsdt_entries"],
                    x["delivered_xsdt_tables"],
                    x["excluded_by_rule"],
                    if missing > 0 {
                        format!("**{missing}**")
                    } else {
                        "0".into()
                    }
                ),
                x["ssdt_numbering_gaps"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .filter(|g| !g.is_empty())
                    .unwrap_or_else(|| "-".into()),
            )
        } else {
            (
                "extracted from guest RAM (complete by construction)".into(),
                "-".into(),
            )
        };
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {xsdt} | {gaps} |",
            set.name,
            sigs.join(" "),
            match set.sums_ok {
                Some(true) => "verified",
                Some(false) => "**MISMATCH**",
                None => "absent",
            }
        );
    }
    let _ = writeln!(s, "\n## Headline metrics\n");
    let _ = writeln!(
        s,
        "Percentages are read beside \"reached\": a table that fails to load contributes no `_STA`/`_INI` to that crate's namespace.\n"
    );
    let _ = writeln!(
        s,
        "| Group | Crate | Sets | DSDT parses clean | All DSDT+SSDT parse clean | `_STA`/`_INI` reached | completing | ...and with no failure of any class |\n|---|---|---|---|---|---|---|---|"
    );
    for (group, pick) in GROUPS {
        for (c, ver) in CRATES {
            let gs: Vec<&CrateGrade> = r
                .grades
                .iter()
                .filter(|g| g.krate == c && pick(&g.set))
                .collect();
            if gs.is_empty() {
                continue;
            }
            let t = totals(&gs);
            let _ = writeln!(
                s,
                "| {group} | `{c}` {ver} | {} | {}/{} = {} | {}/{} = {} | {} | {}/{} = {} | {}/{} = {} |",
                t.sets,
                t.dsdt_clean,
                t.sets,
                pct(t.dsdt_clean, t.sets),
                t.all_clean,
                t.sets,
                pct(t.all_clean, t.sets),
                t.evals,
                t.completed,
                t.evals,
                pct(t.completed, t.evals),
                t.completed_clean,
                t.evals,
                pct(t.completed_clean, t.evals)
            );
        }
    }
    crate::gates::gates(r).to_markdown(&mut s);
    let _ = writeln!(
        s,
        "\n## Ranked gap list (distinct failure modes, by occurrences)\n"
    );
    let _ = writeln!(
        s,
        "| # | Crate | Class | Failure mode | Occurrences | Sets | Example |\n|---|---|---|---|---|---|---|"
    );
    for (i, g) in gaps(&r.grades).iter().enumerate() {
        let _ = writeln!(
            s,
            "| {} | `{}` | {} | {} | {} | {} | {} |",
            i + 1,
            g.krate,
            g.class.name(),
            g.key,
            g.count,
            g.sets.len(),
            g.example.replace('|', "\\|")
        );
    }
    for set in r.sets.iter().filter(|x| !x.probes.is_empty()) {
        let _ = writeln!(s, "\n## Device probes: `{}`\n", set.name);
        let _ = writeln!(
            s,
            "Results under the scripted zero model: a method that branches on firmware NVS state takes its all-zero branch.\n"
        );
        let _ = writeln!(
            s,
            "| Device | Call | `aml` 0.16.4 | `acpi` 6.1.1 | `acpi+fix1` 6.1.1 |\n|---|---|---|---|---|"
        );
        let cell = |krate: &str, target: &str| -> String {
            let Some(g) = r
                .grades
                .iter()
                .find(|g| g.set == set.name && g.krate == krate)
            else {
                return "-".into();
            };
            let Some((i, e)) = g
                .evals
                .iter()
                .enumerate()
                .find(|(_, e)| e.phase == Phase::Probe && e.target == target)
            else {
                return "not reached".into();
            };
            let text = if e.ok {
                format!("ok: {}", e.result.clone().unwrap_or_default())
            } else {
                g.failures
                    .iter()
                    .find(|f| f.eval == i)
                    .map(|f| format!("**FAIL** {}: {}", f.class.name(), f.key))
                    .unwrap_or_else(|| "**FAIL**".into())
            };
            crate::probe::clip(text, 140).replace('|', "\\|")
        };
        for p in &set.probes {
            for m in &p.methods {
                let mut calls = vec![format!("{}.{m}", p.path)];
                if m == "_CRS" {
                    calls.push(format!("{}.{m} -> resource_descriptor_list", p.path));
                }
                for c in calls {
                    let short = c
                        .strip_prefix(&format!("{}.", p.path))
                        .unwrap_or(&c)
                        .to_string();
                    let _ = writeln!(
                        s,
                        "| `{}` ({}) | `{short}` | {} | {} | {} |",
                        p.path,
                        p.label,
                        cell(grade_aml016::CRATE, &c),
                        cell(stock::CRATE, &c),
                        cell(fix1::CRATE, &c)
                    );
                }
            }
        }
    }
    let _ = writeln!(s, "\n## Per set\n");
    for g in &r.grades {
        let (st, so) = count(g, Phase::Sta);
        let (it, io) = count(g, Phase::Ini);
        let _ = writeln!(
            s,
            "- `{}` / `{}` {}: DSDT {}, all tables {}, `_STA` {so}/{st}, `_INI` {io}/{it}, {} failures, {} handler accesses; stock init: {}",
            g.set,
            g.krate,
            g.version,
            if g.dsdt_clean() { "clean" } else { "FAILED" },
            if g.all_tables_clean() {
                "clean"
            } else {
                "FAILED"
            },
            g.failures.len(),
            g.accesses.len(),
            g.stock_init
        );
    }
    s
}

fn count(g: &CrateGrade, p: Phase) -> (usize, usize) {
    let all: Vec<_> = g.evals.iter().filter(|e| e.phase == p).collect();
    (all.len(), all.iter().filter(|e| e.ok).count())
}
