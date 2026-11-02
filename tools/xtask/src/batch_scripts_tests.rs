// SPDX-License-Identifier: MPL-2.0
//! OT-58 S2b (docs/THINK-2026-11-01-ot58-tiered-gate.md, 2.4 and 2.5): the
//! batch scripts on a throwaway git repository, and the workflow's batch
//! guards. Dexter's #47 review found a quick batch that could pass for a full
//! one (2.3), a plan step that could hide a failed plan (2.7), and a revert
//! that took another lane's merge with it (2.6); each has a test here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::workspace_root;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "seat")
        .env("GIT_AUTHOR_EMAIL", "seat@invalid")
        .env("GIT_COMMITTER_NAME", "seat")
        .env("GIT_COMMITTER_EMAIL", "seat@invalid")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn script(dir: &Path, name: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new("bash")
        .arg(workspace_root().join("scripts").join(name))
        .args(args)
        .current_dir(dir)
        .env("GIT_COMMITTER_NAME", "seat")
        .env("GIT_COMMITTER_EMAIL", "seat@invalid")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .expect("run script");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn scratch_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ot58-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.name", "seat"]);
    git(&dir, &["config", "user.email", "seat@invalid"]);
    dir
}

/// A commit on a side branch off `from`, adding `file`; returns its SHA.
fn side_commit(dir: &Path, from: &str, branch: &str, file: &str) -> String {
    git(dir, &["checkout", "-q", "-B", branch, from]);
    fs::write(dir.join(file), file).unwrap();
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", file]);
    git(dir, &["rev-parse", "HEAD"])
}

fn assemble(dir: &Path, base: &str, heads: &[&str]) -> (String, String) {
    let mut args = vec![base];
    args.extend_from_slice(heads);
    let (ok, out) = script(dir, "batch-assemble.sh", &args);
    assert!(ok, "batch-assemble: {out}");
    let get = |k: &str| {
        out.lines()
            .find_map(|l| l.strip_prefix(k))
            .expect("assemble output")
            .to_string()
    };
    (get("tree="), get("commit="))
}

/// The assembled batch is the same tree and the same commit on every run, so
/// every job of a batch tests one tree and the PM can re-check it locally.
#[test]
fn batch_assemble_is_deterministic() {
    let dir = scratch_repo("assemble");
    fs::write(dir.join("base"), "base").unwrap();
    git(&dir, &["add", "base"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    let a = side_commit(&dir, &base, "a", "a");
    let b = side_commit(&dir, &base, "b", "b");
    let first = assemble(&dir, &base, &[&a, &b]);
    let second = assemble(&dir, &base, &[&a, &b]);
    assert_eq!(first, second);
    let _ = fs::remove_dir_all(&dir);
}

/// Dexter #47 item 2.6: the revert takes the batch's own merges and nothing
/// else. `main` here is: base, an unrelated PR's merge, the batch (members 1
/// and 2), a second batch (member 1), a docs commit. Reverting the first
/// batch must keep the unrelated merge, the second batch and the docs commit.
#[test]
fn revert_batch_reverts_only_the_batch() {
    let dir = scratch_repo("revert");
    fs::write(dir.join("base"), "base").unwrap();
    git(&dir, &["add", "base"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);

    // Another lane's PR, merged normally after the batch's base.
    let other = side_commit(&dir, &base, "other", "other");
    git(&dir, &["checkout", "-q", "main"]);
    git(
        &dir,
        &["merge", "-q", "--no-ff", "-m", "Merge other", &other],
    );
    let after_other = git(&dir, &["rev-parse", "HEAD"]);

    // The batch, landed on top (main moved docs-only, as THINK 2.4 allows).
    let m1 = side_commit(&dir, &base, "m1", "m1");
    let m2 = side_commit(&dir, &base, "m2", "m2");
    let (_, tip) = assemble(&dir, &after_other, &[&m1, &m2]);
    // A later batch, and a docs commit after it.
    let n1 = side_commit(&dir, &base, "n1", "n1");
    let (_, tip2) = assemble(&dir, &tip, &[&n1]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["reset", "-q", "--hard", &tip2]);
    fs::write(dir.join("docs"), "docs").unwrap();
    git(&dir, &["add", "docs"]);
    git(&dir, &["commit", "-q", "-m", "docs"]);

    let (ok, out) = script(&dir, "revert-batch.sh", &[&base]);
    assert!(ok, "revert-batch: {out}");
    for kept in ["base", "other", "n1", "docs"] {
        assert!(
            dir.join(kept).exists(),
            "{kept} must survive the revert: {out}"
        );
    }
    for gone in ["m1", "m2"] {
        assert!(!dir.join(gone).exists(), "{gone} is the batch's: {out}");
    }
    // One commit, reverting two merges.
    assert_eq!(git(&dir, &["rev-list", "--count", "HEAD^..HEAD"]), "1");
    let msg = git(&dir, &["log", "-1", "--format=%B"]);
    assert_eq!(msg.matches("batch: member").count(), 2, "{msg}");

    // A base with no batch after it is a usage error, not an empty commit.
    let (ok, _) = script(&dir, "revert-batch.sh", &["HEAD"]);
    assert!(!ok);
    let _ = fs::remove_dir_all(&dir);
}

fn workflow() -> String {
    fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).expect("read ci.yml")
}

/// Dexter #47 items 2.3 and 2.4: the plan job decides the batch's tier before
/// anything else, and only `tier: full` can run a batch; a bisect leg is one
/// job of named scenarios; and the report is green only for the full tier.
#[test]
fn a_batch_is_always_the_full_tier() {
    let ci = workflow();
    let tier = &ci[ci.find("      - id: tier\n").expect("tier step")..];
    let tier = &tier[..tier.find("\n      - name:").expect("end of tier step")];
    let batch_at = tier
        .find("if [ -n \"$BATCH$PREFIX$SCENARIOS\" ]")
        .expect("batch branch");
    let plan_at = tier.find("gate-plan").expect("gate-plan call");
    assert!(batch_at < plan_at, "the batch branch must come first");
    assert!(tier.contains("a batch is the full tier; dispatch it with tier: full"));
    assert!(tier.contains("echo 'shards=[\"bisect\"]'"));

    let qemu = &ci[ci.find("\n  neo-test:\n").unwrap()..ci.find("\n  rng-seed-boot:\n").unwrap()];
    assert!(qemu.contains("exec cargo neo-test $SCENARIOS --keep-going --rerun-timeouts"));
    assert!(
        qemu.contains("exec cargo neo-test --shard \"$SHARD/4\" --keep-going --rerun-timeouts")
    );
    // The quick branch can only be taken by the quick shard value.
    assert!(qemu.contains("if [ \"$SHARD\" = quick ]; then"));

    let report = &ci[ci.find("\n  batch-report:\n").expect("batch-report")..];
    assert!(report.contains(
        "[ \"$MODE\" = full ] && [ \"$FULL\" = true ] && [ \"$k\" -eq 4 ] || green=false"
    ));
    assert!(report.contains("Merge evidence: yes"));
    for job in [
        "neo-test",
        "host-checks",
        "build-checks",
        "rng-seed-boot",
        "decoder-fuzz",
    ] {
        assert!(report.contains(&format!("needs.{job}.result")), "{job}");
    }
}

/// Dexter #47 item 2.7: a failed batch plan fails its step. No `| tee` that
/// would return tee's status, and the step runs under `pipefail`.
#[test]
fn a_failed_batch_plan_fails_the_plan_step() {
    let ci = workflow();
    let step = &ci[ci
        .find("      - name: Plan the batch\n")
        .expect("plan step")..];
    let step = &step[..step.find("\n  neo-test:\n").expect("end of plan job")];
    assert!(step.contains("shell: bash"));
    assert!(step.contains("set -euo pipefail"));
    assert!(!step.contains("| tee"));
}
