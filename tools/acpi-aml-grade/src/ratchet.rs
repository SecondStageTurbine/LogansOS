// SPDX-License-Identifier: MPL-2.0
//! The `acpi+fix1` regression ratchet (OT-86): numbers only go up.
//!
//! `report/ratchet.json` holds, per corpus set, the floor the patched crate
//! has reached. A run regresses if, on any set, any of these goes down:
//! - the DSDT, or all tables, no longer load clean;
//! - a table that loaded no longer does;
//! - an evaluation (`_STA`/`_INI`/probe call) that completed no longer does;
//! - an evaluation that completed with no failure of any class now has one
//!   (an out-of-region access shows up here);
//! - the crate-panic count rises.
//!
//! Completions are compared per target, not as totals, so a gain in one place
//! cannot hide a loss in another. Failure counts per class are deliberately
//! not ratcheted: fixing an early failure lets evaluation reach further, and
//! what it finds there (the next layer) is progress, not a regression. The
//! per-target "clean" sets catch a target that was clean getting worse.
//!
//! The floor only moves through `ACPI_GRADE_UPDATE_RATCHET=1`, which refuses
//! to lower anything; lowering it takes a hand edit that review sees.

use crate::report::{CrateGrade, Mode, Phase};
use crate::summary::Run;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn counts<'a>(it: impl Iterator<Item = &'a str>) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    for t in it {
        *m.entry(t.to_string()).or_insert(0) += 1;
    }
    m
}

fn floor_of(g: &CrateGrade) -> Value {
    let failed: std::collections::BTreeSet<usize> = g.failures.iter().map(|f| f.eval).collect();
    let evals = || {
        g.evals
            .iter()
            .enumerate()
            .filter(|(_, e)| !matches!(e.phase, Phase::Load | Phase::Setup))
    };
    json!({
        "dsdt_parse_clean": g.dsdt_clean(),
        "all_tables_parse_clean": g.all_tables_clean(),
        "tables_loaded": counts(g.evals.iter().filter(|e| e.phase == Phase::Load && e.ok).map(|e| e.target.as_str())),
        "completed": counts(evals().filter(|(_, e)| e.ok).map(|(_, e)| e.target.as_str())),
        "completed_clean": counts(evals().filter(|(i, e)| e.ok && !failed.contains(i)).map(|(_, e)| e.target.as_str())),
        "panics": g.failures.iter().filter(|f| f.mode == Mode::Panic).count(),
    })
}

/// The floor `krate` reaches on this run, per set.
pub fn snapshot(r: &Run, krate: &str) -> Value {
    let mut sets = serde_json::Map::new();
    for g in r.grades.iter().filter(|g| g.krate == krate) {
        sets.insert(g.set.clone(), floor_of(g));
    }
    json!({ "crate": krate, "sets": sets })
}

/// Every way `now` falls below `floor`. Empty means no regression. A set the
/// floor has never seen is reported too, so adding a set forces a ratchet
/// update in the same commit.
pub fn regressions(floor: &Value, now: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let empty = serde_json::Map::new();
    let fs = floor["sets"].as_object().unwrap_or(&empty);
    let ns = now["sets"].as_object().unwrap_or(&empty);
    for name in ns.keys() {
        if !fs.contains_key(name) {
            out.push(format!(
                "{name}: no floor recorded for this set (update the ratchet)"
            ));
        }
    }
    for (name, f) in fs {
        let Some(n) = ns.get(name) else {
            out.push(format!("{name}: set no longer graded"));
            continue;
        };
        for flag in ["dsdt_parse_clean", "all_tables_parse_clean"] {
            if f[flag] == true && n[flag] != true {
                out.push(format!("{name}: {flag} was true, now false"));
            }
        }
        for key in ["tables_loaded", "completed", "completed_clean"] {
            let nk = n[key].as_object().unwrap_or(&empty);
            for (target, c) in f[key].as_object().unwrap_or(&empty) {
                let was = c.as_u64().unwrap_or(0);
                let now_c = nk.get(target).and_then(Value::as_u64).unwrap_or(0);
                if now_c < was {
                    out.push(format!("{name}: {key} `{target}` {was} -> {now_c}"));
                }
            }
        }
        let (fp, np) = (
            f["panics"].as_u64().unwrap_or(0),
            n["panics"].as_u64().unwrap_or(0),
        );
        if np > fp {
            out.push(format!("{name}: panics {fp} -> {np}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::regressions;
    use serde_json::json;

    fn set(
        completed: serde_json::Value,
        clean: serde_json::Value,
        panics: u64,
    ) -> serde_json::Value {
        json!({"sets": {"s": {
            "dsdt_parse_clean": true, "all_tables_parse_clean": false,
            "tables_loaded": {"DSDT": 1}, "completed": completed, "completed_clean": clean, "panics": panics,
        }}})
    }

    #[test]
    fn a_loss_hidden_behind_a_gain_is_still_a_regression() {
        let floor = set(json!({"\\A._STA": 1}), json!({"\\A._STA": 1}), 2);
        let now = set(json!({"\\B._STA": 1, "\\C._STA": 1}), json!({}), 2);
        let r = regressions(&floor, &now);
        assert!(
            r.iter().any(|x| x.contains("completed `\\A._STA` 1 -> 0")),
            "{r:?}"
        );
        assert!(
            r.iter().any(|x| x.contains("completed_clean `\\A._STA`")),
            "{r:?}"
        );
    }

    #[test]
    fn gains_and_fewer_panics_are_not_regressions() {
        let floor = set(json!({"\\A._STA": 1}), json!({}), 3);
        let now = set(
            json!({"\\A._STA": 1, "\\B._STA": 1}),
            json!({"\\A._STA": 1}),
            1,
        );
        assert_eq!(regressions(&floor, &now), Vec::<String>::new());
    }

    #[test]
    fn more_panics_or_a_lost_clean_dsdt_regress() {
        let floor = set(json!({}), json!({}), 1);
        let mut now = set(json!({}), json!({}), 2);
        now["sets"]["s"]["dsdt_parse_clean"] = json!(false);
        let r = regressions(&floor, &now);
        assert!(r.iter().any(|x| x.contains("panics 1 -> 2")), "{r:?}");
        assert!(r.iter().any(|x| x.contains("dsdt_parse_clean")), "{r:?}");
    }
}
