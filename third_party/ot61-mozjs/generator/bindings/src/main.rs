// SPDX-License-Identifier: MPL-2.0
//! Runs upstream mozjs-sys's `build` and `build_bindings` for `JSApi` and
//! `JSGlue` against `NEO_OBJ` (the configured, exported object directory).
//! The current directory must be the patched copy's `mozjs-sys` (upstream's
//! paths are `./src/jsapi.cpp`, `./src/jsglue.cpp`). The environment supplies
//! what cargo would give a build script for our target, and
//! BINDGEN_EXTRA_CLANG_ARGS supplies neo-cc's target flags and headers, so
//! libclang parses with `__neoplutarch__` (trap B1). Outputs land in
//! NEO_OBJ: libjsapi.a, libjsglue.a, jsapi.rs and gluebindings.rs.
#![allow(dead_code, unused_imports, unused_variables, deprecated, unexpected_cfgs)]

include!(concat!(env!("OUT_DIR"), "/upstream_build.rs"));

fn main() {
    let obj = PathBuf::from(env::var("NEO_OBJ").expect("NEO_OBJ must name the object directory"));
    for t in [BuildTarget::JSApi, BuildTarget::JSGlue] {
        build(&obj, t);
        build_bindings(&obj, t);
        println!("neo-mozjs-bindgen: {} and {} written", t.output(), t.output_bindings());
    }
}
