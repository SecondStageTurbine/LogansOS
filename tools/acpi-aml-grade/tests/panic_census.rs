// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 2: `third_party/acpi-loganfy/PANIC-CENSUS.md` Part A lists every
//! explicit panic site left in `src/aml`, each classified. This test scans the
//! crate for the same markers and fails unless the census lists exactly the
//! sites found, by file, line and text, and gives each an allowed disposition.

use std::collections::BTreeSet;
use std::path::PathBuf;

const MARKERS: [&str; 9] = [
    "panic!",
    ".unwrap()",
    ".expect(",
    "unreachable!",
    "todo!",
    "unimplemented!",
    "assert!",
    "assert_eq!",
    "assert_ne!",
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/acpi-loganfy")
}

/// Every marker line in `src/aml/*.rs`, outside comments and the trailing
/// `#[cfg(test)] mod tests`, as (`file:line`, trimmed text).
fn scan() -> BTreeSet<(String, String)> {
    let dir = crate_dir().join("src/aml");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    let mut sites = BTreeSet::new();
    for path in files {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t == "#[cfg(test)]" && lines.get(i + 1).is_some_and(|n| n.starts_with("mod tests")) {
                break;
            }
            if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
                continue;
            }
            // `debug_assert` is covered by "assert!" / "assert_eq!" / "assert_ne!".
            if MARKERS.iter().any(|m| t.contains(m)) {
                sites.insert((format!("{name}:{}", i + 1), t.to_string()));
            }
        }
    }
    sites
}

/// Part A rows: `| `file:line` | `code` | disposition | why |`.
fn census() -> Vec<(String, String, String)> {
    let text = std::fs::read_to_string(crate_dir().join("PANIC-CENSUS.md")).unwrap();
    let part_a = text
        .split("## Part A")
        .nth(1)
        .expect("census has Part A")
        .split("\n## ")
        .next()
        .unwrap();
    let mut rows = Vec::new();
    for line in part_a.lines().filter(|l| l.starts_with("| `")) {
        // Split on unescaped `|`.
        let mut cells = Vec::new();
        let mut cur = String::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' if chars.peek() == Some(&'|') => cur.push(chars.next().unwrap()),
                '|' => cells.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            }
        }
        let cell = |i: usize| cells[i].trim().trim_matches('`').to_string();
        rows.push((cell(1), cell(2), cell(3)));
    }
    rows
}

#[test]
fn census_lists_every_explicit_site() {
    let found = scan();
    let rows = census();
    let listed: BTreeSet<_> = rows
        .iter()
        .map(|(loc, code, _)| (loc.clone(), code.clone()))
        .collect();
    assert_eq!(listed.len(), rows.len(), "a census row is duplicated");
    let missing: Vec<_> = found.difference(&listed).collect();
    let stale: Vec<_> = listed.difference(&found).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "PANIC-CENSUS.md Part A is out of date.\nIn the crate, not the census: {missing:#?}\nIn the census, not the crate: {stale:#?}"
    );
    for (loc, _, disposition) in &rows {
        assert!(
            ["unreachable", "host-API"].contains(&disposition.as_str()),
            "{loc}: disposition {disposition:?} is not `unreachable` or `host-API`"
        );
    }
}
