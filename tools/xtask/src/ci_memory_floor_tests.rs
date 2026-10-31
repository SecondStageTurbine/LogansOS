// SPDX-License-Identifier: MPL-2.0
//! The `neo-test` memory pre-flight in `.github/workflows/ci.yml` and the
//! "Memory floor" column of the runbook's "Current fleet" table must agree:
//! J.c. puts a runner in the QEMU pool by its documented floor, so a stale
//! table puts a runner on the gate that the pre-flight then fails as infra.

use std::fs;

use super::workspace_root;

const STEP: &str = "- name: Pre-flight — runner memory capacity";

/// The pre-flight's `case "$RUNNER_NAME"` arms, as (runner names, GiB). The
/// default arm's names are `["*"]`.
fn preflight_arms(ci: &str) -> Vec<(Vec<String>, u32)> {
    let step = &ci[ci
        .find(STEP)
        .expect("ci.yml has the memory pre-flight step")..];
    let case = &step[..step.find("esac").expect("the pre-flight's case ends")];
    case.lines()
        .filter_map(|line| {
            let (names, rest) = line.trim().split_once(") REQUIRED_GIB=")?;
            let gib = rest
                .strip_suffix(" ;;")?
                .parse()
                .expect("REQUIRED_GIB is a number");
            Some((names.split('|').map(str::to_owned).collect(), gib))
        })
        .collect()
}

/// The "Current fleet" table's rows, as (runner, floor GiB).
fn documented_floors(runbook: &str) -> Vec<(String, u32)> {
    let section = &runbook[runbook
        .find("## Current fleet")
        .expect("the runbook has a Current fleet section")..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |i| i + 3)];
    section
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            let runner = cells.first()?.strip_prefix('`')?.strip_suffix('`')?;
            let gib = cells.last()?.strip_suffix(" GiB")?.parse().ok()?;
            Some((runner.to_owned(), gib))
        })
        .collect()
}

fn floor_for(arms: &[(Vec<String>, u32)], runner: &str) -> u32 {
    arms.iter()
        .find(|(names, _)| names.iter().any(|n| n == runner || n == "*"))
        .map(|&(_, gib)| gib)
        .expect("the pre-flight has a default arm")
}

#[test]
fn preflight_arms_match_the_documented_floors() {
    let root = workspace_root();
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let runbook = fs::read_to_string(root.join("docs/runbook-ci-runner-capacity.md")).unwrap();
    let arms = preflight_arms(&ci);
    let floors = documented_floors(&runbook);

    assert!(
        matches!(arms.last(), Some((names, 32)) if names == &["*"]),
        "the pre-flight's last arm is the 32 GiB default: {arms:?}"
    );
    assert!(floors.len() >= 2, "the fleet table parsed: {floors:?}");
    for (runner, gib) in &floors {
        assert_eq!(
            floor_for(&arms, runner),
            *gib,
            "`{runner}`: ci.yml's pre-flight and the runbook's floor differ"
        );
    }
    // Every named arm is a runner the table lists, so a retired runner's arm
    // goes when its row does.
    for (names, _) in &arms {
        for name in names.iter().filter(|n| *n != "*") {
            assert!(
                floors.iter().any(|(r, _)| r == name),
                "ci.yml's pre-flight names `{name}`, which the fleet table does not list"
            );
        }
    }
}

#[test]
fn the_parsers_read_a_sample() {
    let ci = format!(
        "{STEP}\n  run: |\n    case \"$RUNNER_NAME\" in\n      a-1|a-2) REQUIRED_GIB=8 ;;\n      *) REQUIRED_GIB=32 ;;\n    esac\n"
    );
    let arms = preflight_arms(&ci);
    assert_eq!(floor_for(&arms, "a-2"), 8);
    assert_eq!(floor_for(&arms, "b-1"), 32);
    let runbook = "## Current fleet (x)\n\n| Runner | Floor |\n|---|---|\n| `a-1` | 8 GiB |\n\n## Next\n| `z` | 1 GiB |\n";
    assert_eq!(documented_floors(runbook), vec![("a-1".to_owned(), 8)]);
}
