// SPDX-License-Identifier: MPL-2.0
use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linker_script = manifest_dir.join("linker.ld");

    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rustc-link-arg=-T{}", linker_script.display());
    println!("cargo:rustc-link-arg=--gc-sections");
}
