// SPDX-License-Identifier: MPL-2.0
//! Emit the linker script as an absolute path so rust-lld finds it
//! regardless of the CWD it's invoked from. Cargo's `-Clink-arg=-Tlinker.ld`
//! doesn't work here because rustc spawns lld from the target build dir,
//! not from the package root.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linker_script = manifest_dir.join("linker.ld");

    println!("cargo:rerun-if-changed=linker.ld");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none") {
        println!("cargo:rustc-link-arg=-T{}", linker_script.display());
    }
    println!("cargo:rerun-if-env-changed=NEO_BUILD_ID");
    // Without NEO_BUILD_ID (a plain `cargo build`; xtask always passes it),
    // the id is HEAD, so rerun when HEAD moves.
    watch_git_head(&manifest_dir);

    let build_id = env::var("NEO_BUILD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_head)
        .unwrap_or_else(|| {
            // git did not answer (none installed, or a tree it does not
            // trust): then no git path above was named, and nothing would
            // rerun this script once git works, so the unknown id would
            // stick. A path that never exists makes cargo rerun it on every
            // build until git answers.
            if let Ok(out_dir) = env::var("OUT_DIR") {
                let retry = PathBuf::from(out_dir).join("build-id-retry-until-git-answers");
                println!("cargo:rerun-if-changed={}", retry.display());
            }
            "unknown-build-id".to_string()
        });
    println!("cargo:rustc-env=NEO_BUILD_ID={build_id}");
}

/// `cargo:rerun-if-changed` for the files a new commit, checkout or reset
/// rewrites: this work tree's HEAD and HEAD reflog, the ref HEAD names, and
/// packed-refs (`git rev-parse --git-path` resolves them in a linked
/// worktree too). Only paths that exist are named: a missing one would make
/// cargo rerun this script, and so rebuild the kernel, on every build.
fn watch_git_head(manifest_dir: &Path) {
    let mut paths = vec![
        "HEAD".to_string(),
        "logs/HEAD".to_string(),
        "packed-refs".to_string(),
    ];
    if let Some(name) = git_output(manifest_dir, &["rev-parse", "--symbolic-full-name", "HEAD"])
        && name.starts_with("refs/")
    {
        paths.push(name);
    }
    for path in paths {
        let Some(found) = git_output(manifest_dir, &["rev-parse", "--git-path", &path]) else {
            continue;
        };
        let found = manifest_dir.join(found);
        if found.exists() {
            println!("cargo:rerun-if-changed={}", found.display());
        }
    }
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn git_head() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8(output.stdout).ok()?;
    let hash = hash.trim();
    if hash.is_empty() {
        None
    } else {
        Some(hash.to_string())
    }
}
