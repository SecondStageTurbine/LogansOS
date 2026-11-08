// SPDX-License-Identifier: MPL-2.0
//! Compiles the smoke's own checks against neo-crt's headers, with the same
//! target flags neo-crt uses: `c/crt_checks.c` (musl) and `c/cxx_checks.cpp`
//! (libc++, no exceptions, no RTTI).

/// Third-party headers (musl, libc++) go in with `-isystem`, not `-I`, so a
/// newer clang's warnings in them can't fail this crate's `-Werror`; only the
/// smoke's own code is held to it. The order is kept: libc++, then the C
/// library, then clang's own headers.
fn system_includes(build: &mut cc::Build, dirs: &[String]) {
    for dir in dirs {
        build.flag("-isystem").flag(dir);
    }
}

fn main() {
    let inc = |k: &str| std::env::var(format!("DEP_NEO_CRT_C_{k}")).unwrap();
    let c_headers = [
        inc("INCLUDE_GENERATED"),
        inc("INCLUDE_ARCH"),
        inc("INCLUDE_ARCH_GENERIC"),
        inc("INCLUDE"),
    ];
    let mut c = cc::Build::new();
    c.compiler(std::env::var("NEO_CRT_CLANG").unwrap_or_else(|_| "clang".into()))
        .no_default_flags(true)
        .flag("--target=x86_64-unknown-none-elf")
        .flag("-mno-red-zone")
        .flag("-mcmodel=small")
        .flag("-fno-pic")
        .flag("-fno-pie")
        .flag("-O2")
        .flag("-fno-stack-protector")
        .flag("-std=c11")
        .flag("-nostdinc")
        .flag("-Wall")
        .flag("-Werror")
        .define("_XOPEN_SOURCE", "700");
    system_includes(&mut c, &c_headers);
    c.file("c/crt_checks.c")
        .file("c/pthread_checks.c")
        .compile("crt_checks");
    let mut cxx = cc::Build::new();
    cxx.compiler(std::env::var("NEO_CRT_CLANG").unwrap_or_else(|_| "clang".into()))
        .no_default_flags(true)
        .cpp(true)
        .cpp_link_stdlib(None)
        .flag("--target=x86_64-unknown-none-elf")
        .flag("-mno-red-zone")
        .flag("-mcmodel=small")
        .flag("-fno-pic")
        .flag("-fno-pie")
        .flag("-O2")
        .flag("-fno-stack-protector")
        .flag("-std=c++20")
        .flag("-nostdinc")
        .flag("-nostdinc++")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-Wall")
        .flag("-Werror")
        .define("_XOPEN_SOURCE", "700")
        // libc++'s locale headers call musl's strto*_l and vasprintf, which
        // musl declares only for _GNU_SOURCE (neo-crt's cxx/__config_site).
        .define("_GNU_SOURCE", None);
    system_includes(&mut cxx, &[inc("INCLUDE_CXX_CONFIG"), inc("INCLUDE_CXX")]);
    system_includes(&mut cxx, &c_headers);
    system_includes(&mut cxx, &[inc("INCLUDE_BUILTIN")]);
    cxx.file("c/cxx_checks.cpp")
        .file("c/cxx_thread_checks.cpp")
        .file("c/cxx_stream_checks.cpp")
        .compile("cxx_checks");
    // libc++'s configuration decides code that inlines into this crate's
    // C++ (the hardening checks), so a change to it must rebuild that too.
    println!(
        "cargo:rerun-if-changed={}/__config_site",
        inc("INCLUDE_CXX_CONFIG")
    );
    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-changed=build.rs");
}
