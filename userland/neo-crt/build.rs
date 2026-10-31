// SPDX-License-Identifier: MPL-2.0
//! Compiles neo-crt's C: the musl top half (an explicit file list) plus
<<<<<<< ours
//! `c/neo_bottom.c` and neo-crt's always-locked `c/stdout.c` and
//! `c/stderr.c`, into `libneo_crt_c.a`.
=======
//! `c/neo_bottom.c`, into `libneo_crt_c.a`.
>>>>>>> theirs
//!
//! The compiler is clang (`NEO_CRT_CLANG`, default `clang`), driven with no
//! default flags: every flag that shapes the objects is spelled out below and
//! matches `userland/x86_64-unknown-neoplutarch.json` (static, no PIC, no red
//! zone, small code model). musl's own CFLAGS (its Makefile) supply the rest:
//! C99, freestanding, no builtin headers, excess precision and rounding as
//! musl's math expects.

use std::path::{Path, PathBuf};

/// musl source directories compiled whole (their top-level `.c` files; the
/// per-architecture subdirectories, which hold assembly, are not used).
const WHOLE_DIRS: &[&str] = &[
    "src/string",
    "src/ctype",
    "src/math",
    "src/multibyte",
    "src/stdlib",
];

/// Files left out of the whole directories, and why.
const SKIP: &[&str] = &[
    // compiler_builtins (build-std's `compiler-builtins-mem`) already
    // provides these for every Rust binary; one copy only.
    "src/string/memcpy.c",
    "src/string/memmove.c",
    "src/string/memset.c",
    "src/string/memcmp.c",
    "src/string/bcmp.c",
    "src/string/strlen.c",
];

/// Single files from directories that also hold Linux-facing code.
const FILES: &[&str] = &[
<<<<<<< ours
    // stdio: formatting and scanning over a buffer. No fopen/fdopen/tmpfile:
    // there is no ambient filesystem. The two standard output streams are
    // neo-crt's copies (c/stdout.c, c/stderr.c), locked always.
=======
    // stdio: formatting and scanning over a buffer, and the two standard
    // output streams. No fopen/fdopen/tmpfile: there is no ambient filesystem.
>>>>>>> theirs
    "src/stdio/vfprintf.c",
    "src/stdio/vsnprintf.c",
    "src/stdio/snprintf.c",
    "src/stdio/vsprintf.c",
    "src/stdio/sprintf.c",
    "src/stdio/fprintf.c",
    "src/stdio/printf.c",
    "src/stdio/vprintf.c",
    "src/stdio/vfscanf.c",
    "src/stdio/vsscanf.c",
    "src/stdio/sscanf.c",
    "src/stdio/fputs.c",
    "src/stdio/fputc.c",
    "src/stdio/putc.c",
    "src/stdio/putchar.c",
    "src/stdio/puts.c",
    "src/stdio/fwrite.c",
    "src/stdio/fflush.c",
    "src/stdio/__towrite.c",
    "src/stdio/__overflow.c",
    "src/stdio/__toread.c",
    "src/stdio/__uflow.c",
    "src/stdio/ofl.c",
<<<<<<< ours
=======
    "src/stdio/stdout.c",
    "src/stdio/stderr.c",
>>>>>>> theirs
    // locale: the C locale only.
    "src/locale/c_locale.c",
    "src/locale/__lctrans.c",
    "src/locale/langinfo.c",
    "src/locale/localeconv.c",
    // strerror's message table.
    "src/errno/strerror.c",
    // rand/random/drand48.
    "src/prng/rand.c",
    "src/prng/rand_r.c",
    "src/prng/random.c",
    "src/prng/__rand48_step.c",
    "src/prng/__seed48.c",
    "src/prng/drand48.c",
    "src/prng/lrand48.c",
    "src/prng/mrand48.c",
    "src/prng/seed48.c",
    "src/prng/srand48.c",
    // strtod/strtol and the scanf engines.
    "src/internal/floatscan.c",
    "src/internal/intscan.c",
    "src/internal/shgetc.c",
    // assert()'s failure path: fprintf to stderr, then abort (both neo-crt's).
    "src/exit/assert.c",
    // the generic (soft) fenv: fegetround et al. without x87/SSE control.
    "src/fenv/fenv.c",
    "src/fenv/fesetround.c",
];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let third = manifest.join("../../third_party/ot61-crt/vendor");
    let musl = third.join("musl");
    let generated = third.join("musl-generated");

    let mut sources: Vec<PathBuf> = Vec::new();
    for dir in WHOLE_DIRS {
        let mut files: Vec<PathBuf> = std::fs::read_dir(musl.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "c"))
            .filter(|p| {
                let rel = p.strip_prefix(&musl).unwrap();
                !SKIP.iter().any(|s| Path::new(s) == rel)
            })
            .collect();
        files.sort();
        sources.extend(files);
    }
    sources.extend(FILES.iter().map(|f| musl.join(f)));
    sources.push(manifest.join("c/neo_bottom.c"));
<<<<<<< ours
    sources.push(manifest.join("c/stdout.c"));
    sources.push(manifest.join("c/stderr.c"));
=======
>>>>>>> theirs

    let clang = std::env::var("NEO_CRT_CLANG").unwrap_or_else(|_| "clang".into());
    let mut build = cc::Build::new();
    target_flags(&mut build, &clang);
    build
        // musl's CFLAGS.
        .flag("-std=c99")
        .flag("-nostdinc")
        .flag("-ffreestanding")
        .flag("-frounding-math")
        .flag("-fno-strict-aliasing")
        .flag("-fexcess-precision=standard")
        .flag("-Wno-everything")
        .define("_XOPEN_SOURCE", "700")
        // Include order: neo-crt's overrides first, then musl's own order
        // (arch, internal headers, generated bits, public headers).
        .include(manifest.join("c/override"))
        .include(musl.join("arch/x86_64"))
        .include(musl.join("arch/generic"))
        .include(musl.join("src/include"))
        .include(musl.join("src/internal"))
        .include(&generated)
        .include(musl.join("include"))
        .files(&sources);
    build.compile("neo_crt_c");

    // libc++: its two configuration headers (written by libc++'s CMake
    // upstream) live in cxx/, so every consumer can name one fixed include
    // directory. `__assertion_handler` is the template copied unchanged
    // (CMake substitutes nothing in it); the build stops if the copy drifts.
    let libcxx = third.join("libcxx");
    let cxx_config = manifest.join("cxx");
    let template = std::fs::read(libcxx.join("vendor/llvm/default_assertion_handler.in")).unwrap();
    let copy = std::fs::read(cxx_config.join("__assertion_handler")).unwrap();
    assert!(
        template == copy,
        "cxx/__assertion_handler differs from the vendored libc++ template"
    );
    let builtin = builtin_headers(&clang);

    let mut cxx = cc::Build::new();
    target_flags(&mut cxx, &clang);
    cxx.cpp(true)
        // The C++ library is the libc++ built here, not the host's.
        .cpp_link_stdlib(None)
        .flag("-std=c++23")
        .flag("-nostdinc")
        .flag("-nostdinc++")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-Wno-everything")
        .define("_XOPEN_SOURCE", "700")
        .define("_LIBCPP_BUILDING_LIBRARY", None)
        .define("_LIBCPP_REMOVE_TRANSITIVE_INCLUDES", None)
        .include(&cxx_config)
        .include(libcxx.join("include"))
        .include(libcxx.join("src"))
        .include(&generated)
        .include(musl.join("arch/x86_64"))
        .include(musl.join("arch/generic"))
        .include(musl.join("include"))
        .flag("-isystem")
        .flag(builtin.to_str().unwrap())
        .files(LIBCXX_SOURCES.iter().map(|f| libcxx.join("src").join(f)));
    cxx.compile("neo_crt_cxx");

    // Consumers compiling their own C/C++ against neo-crt read these.
    println!("cargo:include={}", musl.join("include").display());
    println!("cargo:include_arch={}", musl.join("arch/x86_64").display());
    println!(
        "cargo:include_arch_generic={}",
        musl.join("arch/generic").display()
    );
    println!("cargo:include_generated={}", generated.display());
    println!("cargo:include_cxx={}", libcxx.join("include").display());
    println!("cargo:include_cxx_config={}", cxx_config.display());
    println!("cargo:include_builtin={}", builtin.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-changed=cxx");
    println!("cargo:rerun-if-changed=../../third_party/ot61-crt");
    println!("cargo:rerun-if-env-changed=NEO_CRT_CLANG");
}

/// libc++ 18.1.8 library sources for this configuration (`cxx/__config_site`).
/// Left out, as libc++'s own CMake leaves them out under the same options:
/// threads (atomic, barrier, condition_variable*, future, mutex*,
/// shared_mutex, thread), localization and iostreams (fstream, ios*,
/// iostream, locale, ostream, regex, strstream), random_device (random) and
/// the time-zone database (tz, tzdb_list).
const LIBCXX_SOURCES: &[&str] = &[
    "algorithm.cpp",
    "any.cpp",
    "bind.cpp",
    "call_once.cpp",
    "charconv.cpp",
    "chrono.cpp",
    "error_category.cpp",
    "exception.cpp",
    "functional.cpp",
    "hash.cpp",
    "legacy_pointer_safety.cpp",
    "memory.cpp",
    "memory_resource.cpp",
    "new.cpp",
    "new_handler.cpp",
    "new_helpers.cpp",
    "optional.cpp",
    "print.cpp",
    "random_shuffle.cpp",
    "stdexcept.cpp",
    "string.cpp",
    "system_error.cpp",
    "typeinfo.cpp",
    "valarray.cpp",
    "variant.cpp",
    "vector.cpp",
    "verbose_abort.cpp",
    "ryu/d2fixed.cpp",
    "ryu/d2s.cpp",
    "ryu/f2s.cpp",
];

/// The flags that shape every object, C or C++: they match
/// `userland/x86_64-unknown-neoplutarch.json` (static, no PIC, no red zone,
/// small code model).
fn target_flags(build: &mut cc::Build, clang: &str) {
    build
        .compiler(clang)
        .no_default_flags(true)
        .cargo_warnings(false)
        .flag("--target=x86_64-unknown-none-elf")
        .flag("-mno-red-zone")
        .flag("-mcmodel=small")
        .flag("-fno-pic")
        .flag("-fno-pie")
        .flag("-O2")
        .flag("-ffunction-sections")
        .flag("-fdata-sections")
        .flag("-fno-stack-protector");
}

/// clang's own headers (`stddef.h`, `stdarg.h`, the SSE intrinsics), searched
/// after the C library's.
fn builtin_headers(clang: &str) -> PathBuf {
    let out = std::process::Command::new(clang)
        .arg("-print-resource-dir")
        .output()
        .unwrap_or_else(|e| panic!("{clang} -print-resource-dir: {e}"));
    assert!(out.status.success(), "{clang} -print-resource-dir failed");
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim()).join("include")
}
