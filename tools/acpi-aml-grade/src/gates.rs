// SPDX-License-Identifier: MPL-2.0
//! The OT-86 Fix 1 gates, computed from one run: the stock `acpi` column
//! (the rung-0 rebaseline, findings §4b) against the `acpi+fix1` column.
//! Both columns are graded in the same run by the same handler and judge,
//! so every figure here is reproducible from the committed corpus.

use crate::classify::Class;
use crate::grade_acpi6::{fix1, stock};
use crate::report::{CrateGrade, Mode, Phase};
use crate::summary::Run;
use serde_json::{Value, json};
use std::fmt::Write;

/// The rebaseline figures (findings §4b, `58626216`) the stock column must
/// still reproduce, so a gate is never measured against a moved baseline.
pub const REBASELINE_OBJECT_NOT_OF_EXPECTED_TYPE: usize = 83;
pub const REBASELINE_QEMU_WRONG_ACCESSES: usize = 24;
pub const REBASELINE_PANICS: usize = 27;
pub const REBASELINE_REAL_HW_CLEAN: (usize, usize) = (335, 446);
pub const REBASELINE_REAL_HW_DSDT_CLEAN: (usize, usize) = (3, 7);

/// One column's gate measures.
#[derive(Debug, PartialEq)]
pub struct Measures {
    /// `ObjectNotOfExpectedType` failures in table loads and `_STA`/`_INI`.
    pub object_not_of_expected_type: usize,
    /// Out-of-region accesses on the QEMU sets (the scripted model returns
    /// zero, so each is a wrong answer: the value came from the wrong place).
    pub qemu_wrong_accesses: usize,
    /// Crate panics, every phase including device probes.
    pub panics: usize,
    pub unclassified: usize,
    /// Real hardware `_STA`/`_INI`: (completed with no failure, attempted).
    pub real_hw_clean: (usize, usize),
    /// Real hardware sets whose DSDT loads: (loaded, sets).
    pub real_hw_dsdt_clean: (usize, usize),
    /// The OptiPlex 7080 sets whose DSDT loads, by set.
    pub optiplex_dsdt: Vec<(String, bool)>,
}

/// A stock-clean evaluation (completed, no failure of any class) and what
/// the Fix 1 column made of it.
#[derive(Debug)]
pub struct Change {
    pub set: String,
    pub target: String,
    pub stock: String,
    pub fix1: String,
}

#[derive(Debug)]
pub struct Gates {
    pub stock: Measures,
    pub fix1: Measures,
    /// Stock-clean evaluations the Fix 1 column lost (failed or not reached).
    pub regressions: Vec<Change>,
    /// Stock-clean evaluations whose value the Fix 1 column changed.
    pub value_changes: Vec<Change>,
}

fn grades<'a>(r: &'a Run, krate: &str) -> Vec<&'a CrateGrade> {
    r.grades.iter().filter(|g| g.krate == krate).collect()
}

fn is_qemu(set: &str) -> bool {
    set.starts_with("qemu-")
}

fn clean(g: &CrateGrade, i: usize) -> bool {
    g.evals[i].ok && !g.failures.iter().any(|f| f.eval == i)
}

fn measures(gs: &[&CrateGrade]) -> Measures {
    let mut m = Measures {
        object_not_of_expected_type: 0,
        qemu_wrong_accesses: 0,
        panics: 0,
        unclassified: 0,
        real_hw_clean: (0, 0),
        real_hw_dsdt_clean: (0, 0),
        optiplex_dsdt: Vec::new(),
    };
    for g in gs {
        for f in &g.failures {
            if f.phase != Phase::Probe && f.key == "ObjectNotOfExpectedType" {
                m.object_not_of_expected_type += 1;
            }
            if is_qemu(&g.set) && f.class == Class::OutOfRegionAccess {
                m.qemu_wrong_accesses += 1;
            }
            m.panics += (f.mode == Mode::Panic) as usize;
            m.unclassified += (f.class == Class::Unclassified) as usize;
        }
        if is_qemu(&g.set) {
            continue;
        }
        for (i, e) in g.evals.iter().enumerate() {
            if e.phase == Phase::Sta || e.phase == Phase::Ini {
                m.real_hw_clean.1 += 1;
                m.real_hw_clean.0 += clean(g, i) as usize;
            }
        }
        m.real_hw_dsdt_clean.1 += 1;
        m.real_hw_dsdt_clean.0 += g.dsdt_clean() as usize;
        if g.set.starts_with("optiplex-7080") {
            m.optiplex_dsdt.push((g.set.clone(), g.dsdt_clean()));
        }
    }
    m
}

pub fn gates(r: &Run) -> Gates {
    let s = grades(r, stock::CRATE);
    let f = grades(r, fix1::CRATE);
    let mut regressions = Vec::new();
    let mut value_changes = Vec::new();
    for gs in &s {
        let Some(gf) = f.iter().find(|g| g.set == gs.set) else {
            continue;
        };
        for (i, e) in gs.evals.iter().enumerate() {
            if !clean(gs, i) {
                continue;
            }
            let after = gf
                .evals
                .iter()
                .enumerate()
                .find(|(_, x)| x.phase == e.phase && x.target == e.target);
            let show = |x: Option<&String>| x.cloned().unwrap_or_else(|| "ok".into());
            let change = |fix1: String| Change {
                set: gs.set.clone(),
                target: format!("{} {}", e.phase.name(), e.target),
                stock: show(e.result.as_ref()),
                fix1,
            };
            match after {
                Some((j, x)) if clean(gf, j) => {
                    if x.result != e.result {
                        value_changes.push(change(show(x.result.as_ref())));
                    }
                }
                Some(_) => regressions.push(change("fails".into())),
                None => regressions.push(change("not reached".into())),
            }
        }
    }
    Gates {
        stock: measures(&s),
        fix1: measures(&f),
        regressions,
        value_changes,
    }
}

impl Gates {
    /// The gate verdicts, in the order the dispatch lists them.
    pub fn verdicts(&self) -> Vec<(&'static str, String, String, bool)> {
        let (s, f) = (&self.stock, &self.fix1);
        let frac = |(a, b): (usize, usize)| format!("{a}/{b}");
        let opti = |m: &Measures| {
            m.optiplex_dsdt
                .iter()
                .map(|(set, ok)| format!("{set}: {}", if *ok { "loads" } else { "fails" }))
                .collect::<Vec<_>>()
                .join("; ")
        };
        vec![
            (
                "OptiPlex DSDT loads",
                opti(s),
                opti(f),
                !f.optiplex_dsdt.is_empty() && f.optiplex_dsdt.iter().all(|(_, ok)| *ok),
            ),
            (
                "ObjectNotOfExpectedType (loads + _STA/_INI) -> 0",
                s.object_not_of_expected_type.to_string(),
                f.object_not_of_expected_type.to_string(),
                f.object_not_of_expected_type == 0,
            ),
            (
                "QEMU wrong answers (out-of-region accesses) -> 0",
                s.qemu_wrong_accesses.to_string(),
                f.qemu_wrong_accesses.to_string(),
                f.qemu_wrong_accesses == 0,
            ),
            (
                "no regression on any set (stock-clean evaluations lost)",
                "-".into(),
                self.regressions.len().to_string(),
                self.regressions.is_empty(),
            ),
            (
                "acpi panics <= 27",
                s.panics.to_string(),
                f.panics.to_string(),
                f.panics <= REBASELINE_PANICS,
            ),
            (
                "0 unclassified failures",
                s.unclassified.to_string(),
                f.unclassified.to_string(),
                s.unclassified == 0 && f.unclassified == 0,
            ),
            (
                "real hardware _STA/_INI clean (informational)",
                frac(s.real_hw_clean),
                frac(f.real_hw_clean),
                true,
            ),
            (
                "real hardware DSDT loads (informational)",
                frac(s.real_hw_dsdt_clean),
                frac(f.real_hw_dsdt_clean),
                true,
            ),
        ]
    }

    /// The stock column still reproduces the rebaseline.
    pub fn rebaseline_holds(&self) -> bool {
        self.stock.object_not_of_expected_type == REBASELINE_OBJECT_NOT_OF_EXPECTED_TYPE
            && self.stock.qemu_wrong_accesses == REBASELINE_QEMU_WRONG_ACCESSES
            && self.stock.panics == REBASELINE_PANICS
            && self.stock.real_hw_clean == REBASELINE_REAL_HW_CLEAN
            && self.stock.real_hw_dsdt_clean == REBASELINE_REAL_HW_DSDT_CLEAN
    }

    pub fn to_json(&self) -> Value {
        let change = |c: &Change| json!({"set": c.set, "evaluation": c.target, "stock": c.stock, "fix1": c.fix1});
        json!({
            "rebaseline_reproduced": self.rebaseline_holds(),
            "gates": self.verdicts().iter().map(|(g, s, f, pass)| json!({
                "gate": g, "stock": s, "fix1": f, "pass": pass,
            })).collect::<Vec<_>>(),
            "regressions": self.regressions.iter().map(change).collect::<Vec<_>>(),
            "value_changes": self.value_changes.iter().map(change).collect::<Vec<_>>(),
        })
    }

    pub fn to_markdown(&self, s: &mut String) {
        let _ = writeln!(
            s,
            "\n## OT-86 Fix 1 gates: stock `acpi` (rebaseline) vs `acpi+fix1`\n"
        );
        let _ = writeln!(
            s,
            "Same run, same corpus, same handler and judge. The stock column {} the rung-0 rebaseline (findings §4b).\n",
            if self.rebaseline_holds() {
                "reproduces"
            } else {
                "**does not reproduce**"
            }
        );
        let _ = writeln!(
            s,
            "| Gate | Stock `acpi` | `acpi+fix1` | |\n|---|---|---|---|"
        );
        for (g, st, f, pass) in self.verdicts() {
            let _ = writeln!(
                s,
                "| {g} | {st} | {f} | {} |",
                if g.ends_with("(informational)") {
                    "-"
                } else if pass {
                    "PASS"
                } else {
                    "**FAIL**"
                }
            );
        }
        let _ = writeln!(
            s,
            "\nStock-clean evaluations whose value Fix 1 changed: {}.",
            self.value_changes.len()
        );
        if !self.value_changes.is_empty() {
            let _ = writeln!(
                s,
                "\n| Set | Evaluation | Stock | `acpi+fix1` |\n|---|---|---|---|"
            );
            for c in &self.value_changes {
                let _ = writeln!(
                    s,
                    "| `{}` | `{}` | {} | {} |",
                    c.set, c.target, c.stock, c.fix1
                );
            }
        }
        if !self.regressions.is_empty() {
            let _ = writeln!(
                s,
                "\n**Regressions:**\n\n| Set | Evaluation | Stock | `acpi+fix1` |\n|---|---|---|---|"
            );
            for c in &self.regressions {
                let _ = writeln!(
                    s,
                    "| `{}` | `{}` | {} | {} |",
                    c.set, c.target, c.stock, c.fix1
                );
            }
        }
    }
}
