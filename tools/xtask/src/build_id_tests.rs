// SPDX-License-Identifier: MPL-2.0
//! The kernel build id xtask passes as NEO_BUILD_ID (`git_build_id`): it must
//! follow the commit, so an incremental build after a new commit does not
//! embed the previous commit's id.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{build_id_from, git_build_id};

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=xtask-test",
            "-c",
            "user.email=xtask-test@invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .expect("git runs");
    assert!(
        status.success(),
        "git {args:?} failed in {}",
        repo.display()
    );
}

/// A fresh repository with one commit of `file.txt`.
fn scratch_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-build-id-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    std::fs::write(dir.join("file.txt"), "one\n").unwrap();
    git(&dir, &["add", "file.txt"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    dir
}

#[test]
fn two_commits_give_two_build_ids() {
    let repo = scratch_repo("commits");
    let first = git_build_id(&repo).expect("build id at the first commit");
    std::fs::write(repo.join("file.txt"), "two\n").unwrap();
    git(&repo, &["commit", "-q", "-am", "two"]);
    let second = git_build_id(&repo).expect("build id at the second commit");
    assert_ne!(first, second);
    assert_eq!(first.len(), 40, "a clean id is the full hash: {first}");
    assert_eq!(second.len(), 40, "a clean id is the full hash: {second}");
    // Back to the first commit (detached HEAD): its id again.
    git(&repo, &["checkout", "-q", "HEAD~1"]);
    assert_eq!(git_build_id(&repo).as_deref(), Some(first.as_str()));
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_tracked_change_marks_the_id_dirty_and_untracked_files_do_not() {
    let repo = scratch_repo("dirty");
    let clean = git_build_id(&repo).unwrap();
    std::fs::write(repo.join("scratch.log"), "untracked\n").unwrap();
    assert_eq!(git_build_id(&repo).as_deref(), Some(clean.as_str()));
    std::fs::write(repo.join("file.txt"), "edited\n").unwrap();
    let dirty = git_build_id(&repo).unwrap();
    assert_eq!(dirty, format!("{}-dirty", &clean[..26]));
    // The log sink keeps 32 bytes: the suffix fits.
    assert_eq!(dirty.len(), 32);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn no_repository_gives_no_id() {
    let missing =
        std::env::temp_dir().join(format!("xtask-build-id-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    assert_eq!(git_build_id(&missing), None);
}

#[test]
fn build_id_from_is_the_hash_or_a_dirty_prefix() {
    let hash = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(build_id_from(hash, false), hash);
    assert_eq!(
        build_id_from(hash, true),
        "0123456789abcdef0123456789-dirty"
    );
}
