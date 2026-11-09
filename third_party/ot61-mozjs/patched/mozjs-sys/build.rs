// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-1: `mozjs-sys`'s product build script, std only.
//!
//! Upstream's `build.rs` runs configure, make and bindgen. None of that runs
//! here (`docs/THINK-2026-11-08-ot61-c3c1-product-build.md` §2). The CI
//! generator (`.github/workflows/ot61-mozjs-generate.yml`) ran them once,
//! network off, and froze what they made in `third_party/ot61-mozjs/generated/`.
//! This script only compiles C and C++, with the frozen command lines:
//!
//! 1. **Source view** (`OUT_DIR/src`): the compiled directories of the vendored
//!    tree, hard-linked (copied where links fail), with `generated/patched/`
//!    laid over them and `patched/deleted.txt` honoured.
//! 2. **Object view** (`OUT_DIR/obj`): `generated/obj/` with its path tokens
//!    replaced, `dist/include` (links into the source and object views, so a
//!    patched header wins) and `dist/system_wrappers` (one template).
//! 3. **Compile** every entry of `compile_commands.json` and
//!    `glue_commands.json` through `generator/neo-cc`/`neo-c++`, the same
//!    wrapper, flags and pinned clang the generator used.
//! 4. **Archive** with the pinned clang's `llvm-ar` into `libjs_static.a`,
//!    `libjsapi.a` and `libjsglue.a`, and put the frozen bindings where
//!    `src/lib.rs` and `src/glue.rs` include them.
//!
//! It runs no Python, configure, make, bindgen, cbindgen, and no network.
//! The C++ runtime (libc++, musl) comes from `userland/neo-crt`, which the
//! final binary links; this crate emits no `c++` link line.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;

/// The vendored crates' root, under the repository root.
const VENDOR: &str = "third_party/ot61-mozjs/vendor/mozjs-138c9470";
/// The generator's frozen outputs, under the repository root.
const GENERATED: &str = "third_party/ot61-mozjs/generated";
/// The generator's compiler wrapper and its headers, under the repository root.
const GENERATOR: &str = "third_party/ot61-mozjs/generator";

/// The directories the compiles read, relative to the vendored crates' root.
/// The generator's frozen set names nothing outside them (checked in
/// `check_tokens`).
const SOURCE_DIRS: &[&str] = &[
    "mozjs-sys/mozjs/build",
    "mozjs-sys/mozjs/config",
    "mozjs-sys/mozjs/js/public",
    "mozjs-sys/mozjs/js/src",
    "mozjs-sys/mozjs/memory",
    "mozjs-sys/mozjs/mfbt",
    "mozjs-sys/mozjs/modules/fdlibm",
    "mozjs-sys/mozjs/mozglue",
    "mozjs-sys/mozjs/third_party/fmt",
    "mozjs-sys/mozjs/toolkit",
    "mozjs-sys/src",
];

/// The clang major the generator ran (`NEO_CLANG_MAJOR` in the workflow,
/// `NEO_CRT_CLANG_MAJOR` in the CI image).
const PINNED_CLANG_MAJOR: &str = "19";

/// `dist/system_wrappers/<name>`: every wrapper is this (`freeze.py` checks).
const WRAPPER: &str = "#pragma GCC system_header\n\
#pragma GCC visibility push(default)\n\
#include_next <{name}>\n\
#pragma GCC visibility pop\n";

/// One compile: the compiler, its arguments, its directory and its object.
struct Job {
    exe: PathBuf,
    args: Vec<String>,
    dir: PathBuf,
    obj: PathBuf,
}

fn main() {
    // Upstream's other features change configure's options. The frozen set is
    // one configuration: no JIT, no intl, release, zlib from libz-sys.
    for f in ["JIT", "INTL", "DEBUGMOZJS", "PROFILEMOZJS", "JITSPEW", "LIBZ_RS"] {
        if env::var_os(format!("CARGO_FEATURE_{f}")).is_some() {
            panic!("mozjs-sys: feature {f} isn't in the frozen configuration");
        }
    }

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo = manifest
        .join("../../../..")
        .canonicalize()
        .expect("mozjs-sys: repository root");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let vendor = repo.join(VENDOR);
    let generated = repo.join(GENERATED);
    let generator = repo.join(GENERATOR);

    for p in [&generated, &generator, &manifest.join("build.rs")] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    for d in SOURCE_DIRS {
        println!("cargo:rerun-if-changed={}", vendor.join(d).display());
    }
    for v in ["NEO_CLANG", "NEO_CXX_CONFIG", "NUM_JOBS"] {
        println!("cargo:rerun-if-env-changed={v}");
    }

    let src_view = out.join("src");
    let obj_view = out.join("obj");
    let objects = out.join("objects");
    for d in [&src_view, &obj_view, &objects] {
        remove_dir_all(d);
    }

    // The path tokens freeze.py wrote.
    let tokens = [
        ("@CRATES@", src_view.clone()),
        ("@SRC@", src_view.join("mozjs-sys/mozjs")),
        ("@OBJ@", obj_view.clone()),
        ("@REPO@", repo.clone()),
    ];
    let subst = |s: &str| -> String {
        let mut s = s.to_string();
        for (t, p) in &tokens {
            s = s.replace(t, p.to_str().expect("mozjs-sys: non-UTF-8 path"));
        }
        s
    };
    check_tokens(&generated);

    // 1. Source view.
    for d in SOURCE_DIRS {
        mirror(&vendor.join(d), &src_view.join(d));
    }
    let patched = generated.join("patched");
    let mut overlaid = 0;
    walk(&patched, &mut |p| {
        let rel = p.strip_prefix(&patched).unwrap();
        if rel == Path::new("deleted.txt") || !in_source_dirs(rel) {
            return;
        }
        let dst = src_view.join(rel);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        let _ = fs::remove_file(&dst);
        fs::copy(p, &dst).unwrap_or_else(|e| panic!("mozjs-sys: copy {}: {e}", p.display()));
        overlaid += 1;
    });
    for line in read(&patched.join("deleted.txt")).lines() {
        let rel = line.trim();
        if !rel.is_empty() && in_source_dirs(Path::new(rel)) {
            let _ = fs::remove_file(src_view.join(rel));
        }
    }

    // 2. Object view.
    let frozen_obj = generated.join("obj");
    walk(&frozen_obj, &mut |p| {
        let dst = obj_view.join(p.strip_prefix(&frozen_obj).unwrap());
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::write(&dst, subst(&read(p))).unwrap();
    });
    let dist_include = obj_view.join("dist/include");
    for line in read(&generated.join("dist_include.txt")).lines() {
        let Some((name, target)) = line.split_once(" -> ") else {
            continue;
        };
        let dst = dist_include.join(name);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        link_or_copy_file(Path::new(&subst(target)), &dst);
    }
    let wrappers = obj_view.join("dist/system_wrappers");
    for name in read(&generated.join("system_wrappers.txt")).lines() {
        if name.is_empty() {
            continue;
        }
        let dst = wrappers.join(name);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::write(&dst, WRAPPER.replace("{name}", name)).unwrap();
    }

    // 3. Compile.
    let clang = clang();
    let mut jobs = Vec::new();
    let mut names = BTreeSet::new();
    for e in entries(&generated.join("compile_commands.json")) {
        let args: Vec<String> = e.arguments.iter().map(|a| subst(a)).collect();
        let name = object_name(&e.file);
        assert!(names.insert(name.clone()), "mozjs-sys: two objects named {name}");
        jobs.push(Job {
            exe: PathBuf::from(&args[0]),
            args: args[1..].to_vec(),
            dir: PathBuf::from(subst(&e.directory)),
            obj: objects.join("js_static").join(name),
        });
    }
    let n_static = jobs.len();
    for e in entries(&generated.join("glue_commands.json")) {
        let lib = e.file.trim_end_matches(".cpp").to_string();
        let wrapper = if e.cxx { "neo-c++" } else { "neo-cc" };
        jobs.push(Job {
            exe: generator.join(wrapper),
            args: e.arguments.iter().map(|a| subst(a)).collect(),
            dir: PathBuf::from(subst(&e.directory)),
            obj: objects.join(&lib).join(format!("{lib}.o")),
        });
    }
    compile(&jobs, &repo, &clang);

    // 4. Archive, link lines, bindings.
    let ar = llvm_ar(&clang);
    let libs = out.join("lib");
    remove_dir_all(&libs);
    fs::create_dir_all(&libs).unwrap();
    for lib in ["js_static", "jsapi", "jsglue"] {
        let members: Vec<&PathBuf> = jobs
            .iter()
            .map(|j| &j.obj)
            .filter(|o| o.starts_with(objects.join(lib)))
            .collect();
        assert!(!members.is_empty(), "mozjs-sys: lib{lib}.a would be empty");
        let status = Command::new(&ar)
            .arg("crs")
            .arg(libs.join(format!("lib{lib}.a")))
            .args(&members)
            .status()
            .unwrap_or_else(|e| panic!("mozjs-sys: run {}: {e}", ar.display()));
        assert!(status.success(), "mozjs-sys: {} failed for lib{lib}.a", ar.display());
    }
    println!("cargo:rustc-link-search=native={}", libs.display());
    println!("cargo:rustc-link-lib=static=js_static");
    println!("cargo:rustc-link-lib=static=jsapi");
    println!("cargo:rustc-link-lib=static=jsglue");

    let build = out.join("build");
    fs::create_dir_all(&build).unwrap();
    for f in ["jsapi.rs", "gluebindings.rs"] {
        fs::copy(generated.join("bindings").join(f), build.join(f)).unwrap();
    }
    println!(
        "cargo:warning=mozjs-sys: {n_static} SpiderMonkey objects + jsapi + jsglue from the frozen \
         set ({overlaid} patched files), clang {}",
        clang.display()
    );
}

/// The clang to compile with: `NEO_CLANG`, else `clang-<pinned major>` if it
/// runs, else `clang`. A different major warns: the frozen flags were made for
/// the pinned one.
fn clang() -> PathBuf {
    let pinned = format!("clang-{PINNED_CLANG_MAJOR}");
    let clang = env::var_os("NEO_CLANG").map(PathBuf::from).unwrap_or_else(|| {
        if Command::new(&pinned).arg("--version").output().is_ok() {
            PathBuf::from(&pinned)
        } else {
            PathBuf::from("clang")
        }
    });
    let out = Command::new(&clang)
        .arg("-dumpversion")
        .output()
        .unwrap_or_else(|e| panic!("mozjs-sys: run {}: {e}", clang.display()));
    let version = String::from_utf8_lossy(&out.stdout);
    if version.split('.').next() != Some(PINNED_CLANG_MAJOR) {
        println!(
            "cargo:warning=mozjs-sys: {} is clang {}, the generator pinned {PINNED_CLANG_MAJOR}",
            clang.display(),
            version.trim()
        );
    }
    clang
}

/// The pinned clang's own `llvm-ar`.
fn llvm_ar(clang: &Path) -> PathBuf {
    let out = Command::new(clang).arg("-print-prog-name=llvm-ar").output().unwrap();
    let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if p.is_absolute() {
        p
    } else {
        PathBuf::from("llvm-ar")
    }
}

/// Runs the compiles, `NUM_JOBS` at a time. Every failure is printed, then
/// the build fails naming them.
fn compile(jobs: &[Job], repo: &Path, clang: &Path) {
    let n = env::var("NUM_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| thread::available_parallelism().map_or(1, |n| n.get()));
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    thread::scope(|s| {
        for _ in 0..n.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(i) else { break };
                fs::create_dir_all(job.obj.parent().unwrap()).unwrap();
                // Directories with no generated files aren't in the object view.
                fs::create_dir_all(&job.dir).unwrap();
                let mut cmd = Command::new(&job.exe);
                cmd.args(&job.args)
                    .arg("-c")
                    .arg("-o")
                    .arg(&job.obj)
                    .current_dir(&job.dir)
                    .env("NEO_REPO", repo)
                    .env("NEO_CLANG", clang)
                    .env_remove("NEO_CC_LOG");
                match cmd.output() {
                    Ok(o) if o.status.success() => {}
                    Ok(o) => {
                        eprintln!(
                            "mozjs-sys: compile failed: {}\n{}",
                            job.obj.display(),
                            String::from_utf8_lossy(&o.stderr)
                        );
                        failed.lock().unwrap().push(job.obj.clone());
                    }
                    Err(e) => {
                        eprintln!("mozjs-sys: run {}: {e}", job.exe.display());
                        failed.lock().unwrap().push(job.obj.clone());
                    }
                }
            });
        }
    });
    let failed = failed.into_inner().unwrap();
    assert!(
        failed.is_empty(),
        "mozjs-sys: {} compiles failed: {failed:?}",
        failed.len()
    );
}

/// One entry of `compile_commands.json` or `glue_commands.json`.
struct Entry {
    directory: String,
    file: String,
    arguments: Vec<String>,
    cxx: bool,
}

/// Reads the frozen command files. They are flat JSON written by `freeze.py`
/// (`json.dump`): an array of objects with string, bool and string-array
/// values, which `json` below parses.
fn entries(path: &Path) -> Vec<Entry> {
    let v = json::parse(&read(path)).unwrap_or_else(|e| panic!("mozjs-sys: {}: {e}", path.display()));
    let json::Value::Array(items) = v else {
        panic!("mozjs-sys: {} isn't an array", path.display())
    };
    items
        .into_iter()
        .map(|item| {
            let json::Value::Object(fields) = item else {
                panic!("mozjs-sys: {} has a non-object entry", path.display())
            };
            let mut e = Entry {
                directory: String::new(),
                file: String::new(),
                arguments: vec![],
                cxx: false,
            };
            for (k, v) in fields {
                match (k.as_str(), v) {
                    ("directory", json::Value::String(s)) => e.directory = s,
                    ("file", json::Value::String(s)) => e.file = s,
                    ("cxx", json::Value::Bool(b)) => e.cxx = b,
                    ("arguments", json::Value::Array(a)) => {
                        e.arguments = a
                            .into_iter()
                            .map(|x| match x {
                                json::Value::String(s) => s,
                                _ => panic!("mozjs-sys: {}: non-string argument", path.display()),
                            })
                            .collect()
                    }
                    (k, _) => panic!("mozjs-sys: {}: unexpected field {k}", path.display()),
                }
            }
            assert!(
                !e.file.is_empty() && !e.arguments.is_empty(),
                "mozjs-sys: {}: empty entry",
                path.display()
            );
            if e.directory.is_empty() {
                e.directory = "@OBJ@".to_string();
            }
            e
        })
        .collect()
}

/// `@OBJ@/js/src/Unified_cpp_js_src0.cpp` -> `OBJ_js_src_Unified_cpp_js_src0.o`.
fn object_name(file: &str) -> String {
    let rel = file.replace('@', "");
    let stem = rel.rsplit_once('.').map_or(rel.as_str(), |(s, _)| s);
    format!("{}.o", stem.replace(['/', '-'], "_"))
}

/// Fails if the frozen set names a path the source view doesn't hold, so a
/// new generated reference can't silently resolve to nothing.
fn check_tokens(generated: &Path) {
    let mut bad = BTreeSet::new();
    walk(generated, &mut |p| {
        let Ok(text) = fs::read_to_string(p) else { return };
        for (token, base) in [("@SRC@/", "mozjs-sys/mozjs/"), ("@CRATES@/", "")] {
            for rest in text.split(token).skip(1) {
                let end = rest
                    .find(|c: char| c.is_whitespace() || "\"'>".contains(c))
                    .unwrap_or(rest.len());
                let rel = format!("{base}{}", &rest[..end]);
                if !in_source_dirs(Path::new(&rel)) {
                    bad.insert(rel);
                }
            }
        }
    });
    assert!(
        bad.is_empty(),
        "mozjs-sys: the frozen set names paths outside SOURCE_DIRS: {bad:?}"
    );
}

fn in_source_dirs(rel: &Path) -> bool {
    SOURCE_DIRS.iter().any(|d| rel.starts_with(d))
}

/// Mirrors `src` into `dst`: hard links where the filesystem allows, copies
/// otherwise.
fn mirror(src: &Path, dst: &Path) {
    walk(src, &mut |p| {
        let to = dst.join(p.strip_prefix(src).unwrap());
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        if fs::hard_link(p, &to).is_err() {
            fs::copy(p, &to).unwrap_or_else(|e| panic!("mozjs-sys: copy {}: {e}", p.display()));
        }
    });
}

/// `dist/include/<name>` -> its target: a symlink on Unix, a copy elsewhere.
fn link_or_copy_file(target: &Path, dst: &Path) {
    assert!(
        target.is_file(),
        "mozjs-sys: dist/include target {} is missing",
        target.display()
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, dst).unwrap();
    #[cfg(not(unix))]
    fs::copy(target, dst).unwrap();
}

/// Calls `f` on every file under `dir`, in sorted order.
fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("mozjs-sys: read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        let ft = fs::symlink_metadata(&p).unwrap().file_type();
        if ft.is_dir() {
            walk(&p, f);
        } else {
            f(&p);
        }
    }
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("mozjs-sys: read {}: {e}", p.display()))
}

fn remove_dir_all(p: &Path) {
    match fs::remove_dir_all(p) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => panic!("mozjs-sys: remove {}: {e}", p.display()),
    }
}

/// A small JSON reader for the frozen command files (no crates: this build
/// script has no build-dependencies). It handles what `json.dump` writes:
/// objects, arrays, strings with escapes, `true`/`false`/`null` and numbers.
mod json {
    pub enum Value {
        Null,
        Bool(bool),
        Number(()),
        String(String),
        Array(Vec<Value>),
        Object(Vec<(String, Value)>),
    }

    pub fn parse(s: &str) -> Result<Value, String> {
        let mut p = Parser { b: s.as_bytes(), i: 0 };
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(format!("trailing data at byte {}", p.i));
        }
        Ok(v)
    }

    struct Parser<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Parser<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
        }

        fn eat(&mut self, c: u8) -> Result<(), String> {
            self.ws();
            if self.b.get(self.i) == Some(&c) {
                self.i += 1;
                Ok(())
            } else {
                Err(format!("expected '{}' at byte {}", c as char, self.i))
            }
        }

        fn value(&mut self) -> Result<Value, String> {
            self.ws();
            match self.b.get(self.i) {
                Some(b'{') => {
                    self.i += 1;
                    let mut fields = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Value::Object(fields));
                    }
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.eat(b':')?;
                        fields.push((k, self.value()?));
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Value::Object(fields));
                            }
                            _ => return Err(format!("bad object at byte {}", self.i)),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut items = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Value::Array(items));
                    }
                    loop {
                        items.push(self.value()?);
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Value::Array(items));
                            }
                            _ => return Err(format!("bad array at byte {}", self.i)),
                        }
                    }
                }
                Some(b'"') => Ok(Value::String(self.string()?)),
                Some(b't') if self.b[self.i..].starts_with(b"true") => {
                    self.i += 4;
                    Ok(Value::Bool(true))
                }
                Some(b'f') if self.b[self.i..].starts_with(b"false") => {
                    self.i += 5;
                    Ok(Value::Bool(false))
                }
                Some(b'n') if self.b[self.i..].starts_with(b"null") => {
                    self.i += 4;
                    Ok(Value::Null)
                }
                Some(c) if c.is_ascii_digit() || *c == b'-' => {
                    while self.i < self.b.len() && b"+-.eE0123456789".contains(&self.b[self.i]) {
                        self.i += 1;
                    }
                    Ok(Value::Number(()))
                }
                _ => Err(format!("unexpected input at byte {}", self.i)),
            }
        }

        fn string(&mut self) -> Result<String, String> {
            if self.b.get(self.i) != Some(&b'"') {
                return Err(format!("expected a string at byte {}", self.i));
            }
            self.i += 1;
            let mut out = String::new();
            loop {
                let start = self.i;
                while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\' {
                    self.i += 1;
                }
                out.push_str(std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?);
                match self.b.get(self.i) {
                    Some(b'"') => {
                        self.i += 1;
                        return Ok(out);
                    }
                    Some(b'\\') => {
                        let c = *self.b.get(self.i + 1).ok_or("unterminated escape")?;
                        self.i += 2;
                        match c {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{8}'),
                            b'f' => out.push('\u{c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let mut cp = self.hex4()?;
                                if (0xD800..0xDC00).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                }
                                out.push(char::from_u32(cp).ok_or("bad \\u escape")?);
                            }
                            _ => return Err(format!("bad escape at byte {}", self.i)),
                        }
                    }
                    _ => return Err("unterminated string".to_string()),
                }
            }
        }

        fn hex4(&mut self) -> Result<u32, String> {
            let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
            self.i += 4;
            u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16).map_err(|e| e.to_string())
        }
    }
}
