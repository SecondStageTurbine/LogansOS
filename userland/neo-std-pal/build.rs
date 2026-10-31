// SPDX-License-Identifier: MPL-2.0
//! OT-61 C0 — A4: the **build-breaking PAL manifest gate**.
//!
//! The anti-compat-island guard, enforced structurally: this build script runs
//! the checker in `a4_gate.rs` over the PAL crate's own source AND the rust-src
//! std overlay (`userland/neoplutarch-std-overlay/`: its `std/*.rs` modules and
//! the scripts that patch rust-src), and **fails the build** on any syscall
//! surface outside `tools/ot61-pal-manifest.toml` `[gate].allowed_syscalls`.
//! Adding a syscall therefore REQUIRES adding it to the manifest (with its
//! capability provenance) in the same change. The rules (G1 named, G2 value vs
//! the kernel table, G3 one trampoline, G4 rax operand, G5 call sites) are
//! documented in `a4_gate.rs`; the committed negative tests that prove each
//! rule bites are in `tools/xtask/src/ot61_c05_tests.rs`.

#[path = "a4_gate.rs"]
mod a4_gate;

use std::path::{Path, PathBuf};

fn main() {
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let manifest_path = crate_dir.join("../../tools/ot61-pal-manifest.toml");
    let kernel_syscall_path = crate_dir.join("../../kernel/src/arch/x86_64/syscall.rs");
    let roots = [
        crate_dir.join("src"),
        crate_dir.join("../neoplutarch-std-overlay"),
    ];

    println!("cargo:rerun-if-changed={}", manifest_path.display());
    println!("cargo:rerun-if-changed={}", kernel_syscall_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        crate_dir.join("a4_gate.rs").display()
    );
    for root in &roots {
        println!("cargo:rerun-if-changed={}", root.display());
    }

    let read = |p: &Path| {
        std::fs::read_to_string(p)
            .unwrap_or_else(|e| panic!("A4 gate: cannot read {}: {e}", p.display()))
    };
    let sources: Vec<a4_gate::Source> = roots
        .iter()
        .flat_map(|r| walk(r))
        .map(|p| a4_gate::Source {
            path: p.display().to_string(),
            text: read(&p),
        })
        .collect();
    if sources.is_empty() {
        panic!("A4 gate: scanned 0 files — the PAL/overlay paths moved?");
    }

    match a4_gate::check(&sources, &read(&manifest_path), &read(&kernel_syscall_path)) {
        Ok(s) => println!(
            "cargo:warning=A4 PAL-manifest gate OK — {} files, {} syscall4 trampoline(s), {} fn tcb read(s), syscalls {:?} ⊆ [gate].allowed_syscalls (G1-G7)",
            sources.len(),
            s.trampolines,
            s.tcb_reads,
            s.referenced
        ),
        Err(errs) => panic!(
            "A4 gate FAILED — off-manifest syscall surface in the PAL or the std overlay.\n  {}\n\
             Add each syscall to tools/ot61-pal-manifest.toml WITH its capability provenance + a \
             negative gate in the same change (the anti-compat-island rule), or remove it.",
            errs.join("\n  ")
        ),
    }
}

/// Every `.rs` / `.sh` / `.py` file under `dir`, sorted (deterministic logs).
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if p
                .extension()
                .is_some_and(|x| x == "rs" || x == "sh" || x == "py")
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}
