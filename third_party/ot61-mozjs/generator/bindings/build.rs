// SPDX-License-Identifier: MPL-2.0
//! Copies upstream mozjs-sys's build.rs (`NEO_MOZJS_SYS`: the patched copy's
//! `mozjs-sys` directory) into OUT_DIR with its `fn main()` renamed, so
//! src/main.rs can include it and call its functions unchanged.

use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("NEO_MOZJS_SYS").expect("NEO_MOZJS_SYS must name mozjs-sys"));
    let src = fs::read_to_string(dir.join("build.rs")).expect("read upstream build.rs");
    let n = src.matches("\nfn main() {").count();
    assert_eq!(n, 1, "upstream build.rs should have exactly one `fn main() {{`");
    let out = src.replacen("\nfn main() {", "\n#[allow(dead_code)]\nfn upstream_main() {", 1);
    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("upstream_build.rs");
    fs::write(dest, out).unwrap();
    println!("cargo:rerun-if-env-changed=NEO_MOZJS_SYS");
    println!("cargo:rerun-if-changed={}", dir.join("build.rs").display());
}
