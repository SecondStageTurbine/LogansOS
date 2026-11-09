<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the C3c-1 product build (SpiderMonkey with `cc` only)

The next rung after #87 (generator and frozen outputs) and #88 (neo-crt for SpiderMonkey). The design is
`docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md` (#10). Option (c) there says:
- the CI generator commits the frozen source list, `js-confdefs.h` and **bindgen output**;
- the product build runs `cc` only.

This note says how. Upstream's `mozjs-sys/build.rs` does four things in one build script:
1. runs `configure` and `make`;
2. compiles `src/jsapi.cpp` and `src/jsglue.cpp`;
3. runs bindgen twice (on `jsapi` and `jsglue`);
4. links `jsapi`, `jsglue` and `js_static`.

Steps 1 and 3 can't run in our product build: `configure` is third-party build code (CI only, J.c. 2026-10-29), and
bindgen needs libclang.

## 1. What the generator adds (on #87's job)

- **Bindgen output.** The job runs upstream's `build_bindings` configuration for `JSApi` and `JSGlue`, network off:
  - same allowlists and callbacks;
  - our target's clang arguments: `neo-cc`'s flags, the frozen include paths, `-D__neoplutarch__`.

  It freezes the two `.rs` files under `generated/bindings/`. The drift check covers them like the rest.
  - **Trap B1:** `RootingAPI.h` gains `wasiRecursionDepth` under `__neoplutarch__` (patch 0003). Bindings made
    without our define would get `JS::RootingContext`'s layout wrong, silently. The bindgen run must use the same
    defines as the C++ compile. A layout test (bindgen's own `size_of`/`offset_of` asserts) runs on target in the
    smoke.
  **Done in #87:** upstream's own `build`/`build_bindings` run with libclang from the pinned clang. The step fails if
  `jsapi.rs` lacks `wasiRecursionDepth`.
- **The patched files.** The 29 files patches 0001–0004 touch are frozen whole under `generated/patched/`, with
  `deleted.txt`. The product build overlays them on the vendored tree instead of running `git apply`. They are a
  fresh `git apply` on every run, so the drift check proves they equal vendor-plus-patches. **Done in #87.**
- **Still to add:** `glue_commands.json` (the `jsapi.cpp`/`jsglue.cpp` compiles, §2) and the patched-crate check
  (§2).

## 2. The product crates: `cc` only

Three upstream crates have build scripts or build-dependencies that run what the product build must not:
- `mozjs-sys`: configure, make, bindgen;
- `mozjs`: `[build-dependencies]` `cc` and `bindgen`, though it has no `build.rs`;
- `mozjs_normalizer_glue`: its `build.rs` runs cbindgen.

Each gets a patched copy under `third_party/ot61-mozjs/patched/`, following the `ot61-browser/patched/` precedent.
A copy holds only its manifest, and for `mozjs-sys` its `build.rs`. Its `[lib] path` is the vendored crate's own
`src/lib.rs`, so there is no second copy of the sources to drift. In all three manifests:
- the `[build-dependencies]` blocks are gone;
- the `*.workspace = true` fields are written out (MPL-2.0, edition 2021);
- path dependencies point at `vendor/mozjs-138c9470/mozjs-extracted-crates/` or the other patched copies.

`mozjs-sys`'s features that change configure's options (`jit`, `intl`, `debugmozjs`, `profilemozjs`, `jitspew`,
`libz-rs`) stay declared, because `src/` tests them with `cfg`, but they are empty, and `build.rs` fails if one is on.
Their optional dependencies are gone. `mozjs`'s default features are `["libz-sys"]`.

A check in the generator job (`generator/check_patched_crates.py`) holds each copy to that: only the allowed files;
`[lib] path` at the vendored `src/lib.rs`; upstream's name, version, licence and edition; no build-dependencies;
and upstream's dependencies and version requirements, less the dropped optional ones.

- **`mozjs_normalizer_glue`:** `build = false`, and no `links` key. Its one output was `normalizer_glue.h`, which is
  frozen in `generator/include/` and checked against cbindgen 0.29.4 on every generator run (#87).
- **`mozjs`:** the manifest only. It has no build script.
- **`mozjs-sys`:** a new `build.rs` that uses only `std`. It:
  1. **Source view.** In `OUT_DIR/src`, mirrors the compiled directories of the vendored tree (`js/src`, `js/public`,
     `mfbt`, `mozglue`, `memory`, `modules/fdlibm`, `third_party/fmt`, `config`, `build`, `toolkit`, and
     `mozjs-sys/src`; about 65 MB), with `generated/patched/` laid over them and `patched/deleted.txt` honoured.
     These are hard links where the filesystem allows, copies otherwise; an overlaid file replaces its link, never
     writes through it. The script fails if the frozen set names a path outside these directories. (zlib's headers
     come from the vendored tree through `neo-cc`.)
  2. **Object view.** In `OUT_DIR/obj`, writes `generated/obj/` with `@SRC@`/`@OBJ@`/`@REPO@` replaced. Makes
     `dist/include` from `dist_include.txt`, pointing into the source view so patched headers win, and writes each
     `dist/system_wrappers/<name>` from its one template.
  3. **Compile** by running `generator/neo-cc`/`neo-c++` as a process (no `cc` crate), with `NEO_REPO`,
     `NEO_CLANG` (the pinned major) and `NEO_CXX_CONFIG` set. That is the same wrapper, flags and pinned clang the
     generator used. It compiles the 167 entries of `compile_commands.json`, then `src/jsapi.cpp` and
     `src/jsglue.cpp` from `generated/glue_commands.json`.

     That second file is the one new generator output this needs. In the bindgen step, `neo-cc` appends each
     command line it runs to a log (`NEO_CC_LOG`), and `freeze.py` tokenises the two compiles into
     `glue_commands.json`. Their flags therefore come from upstream's `get_common_cc`, recorded, and are never
     re-derived.
  4. **Archive** with the pinned `llvm-ar` into `libjs_static.a`, `libjsapi.a` and `libjsglue.a`. Emit upstream's
     `cargo:rustc-link-lib` lines, and include `generated/bindings/jsapi.rs` and `gluebindings.rs`.

**Nothing in these three crates runs Python, configure, make, bindgen, cbindgen or the network.** The one other
build script in the closure is `libz-sys`'s: it compiles its bundled zlib C sources with `cc`, which is compiling C,
and it fetches nothing. It's the same copy #81's L0 closure already vendors and builds.

## 3. The Rust closure, with licences

Resolved with `cargo tree -p mozjs --no-default-features --features libz-sys -e normal` (no JIT, no intl). There are
46 crates, and every licence is on CLAUDE.md's list (Unicode-3.0 included, 2026-10-31). The product lockfile pins
the exact versions; those below are the current compatible releases.

| Crate | Version | Licence | Role |
|---|---|---|---|
| `mozjs`, `mozjs_sys`, `mozjs_normalizer_glue` | 0.26.14, 153.6.0-2, 153.6.0 | MPL-2.0 | the engine's Rust layer (patched copies, §2) |
| `mozjs_icu_normalizer`, `mozjs_icu_normalizer_data`, `mozjs_icu_collections` | 153.6.0 | Unicode-3.0 | `String.prototype.normalize` (vendored, `mozjs-extracted-crates/`) |
| `mozjs_utf8_iter`, `mozjs_utf16_iter` | 153.6.0 | Apache-2.0 OR MIT | same |
| `icu_casemap`, `icu_collections`, `icu_locale_core`, `icu_properties`, `icu_provider` | 2.1.x–2.4.x | Unicode-3.0 | ICU4X, through the normalizer |
| `litemap`, `potential_utf`, `tinystr`, `writeable`, `yoke`, `zerofrom`, `zerotrie`, `zerovec` | 0.1–0.11 | Unicode-3.0 | ICU4X's data structures |
| `yoke-derive`, `zerofrom-derive`, `zerovec-derive` | 0.1–0.11 | Unicode-3.0 | proc-macros (build time) |
| `encoding_c`, `encoding_c_mem` | 0.9.8, 0.2.6 | Apache-2.0 OR MIT | `mozjs-sys`'s encoding FFI |
| `encoding_rs` | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | the encodings |
| `libz-sys` | 1.1.32 (#81 has 1.1.31) | MIT OR Apache-2.0; its bundled zlib under the zlib licence | zlib for `Compression.cpp` |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | `mozjs`'s FFI types (our target's) |
| `log`, `num-traits`, `smallvec`, `cfg-if`, `scopeguard`, `stable_deref_trait`, `simdutf8`, `write16` | — | MIT OR Apache-2.0 | utilities |
| `core_detect`, `multiversion`, `multiversion_no_op` | — | MIT OR Apache-2.0 | `encoding_rs`'s SIMD dispatch |
| `displaydoc`, `multiversion-macros`, `rustversion`, `proc-macro2`, `quote`, `syn` | — | MIT OR Apache-2.0 | proc-macros and their support (build time) |
| `synstructure` | 0.14.0 | MIT | proc-macro support (build time) |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | `proc-macro2` (build time) |

They are vendored like #81's closure, with a `.cargo-checksum.json` per crate so `vendor-verify` covers them. Each
crate's licence goes in the PR table. `js/src/dtoa.c` (David Gay's Lucent notice) is allowed since J.c.'s ruling of
2026-11-08, and is recorded in #87's `VENDORED.md`.

## 4. The smoke (C3c-1's acceptance)

`userland/mozjs-smoke` (bin), a real-std process. It:
- creates a runtime and context with one realm;
- evaluates `1 + 1`, a closure, a regexp (irregexp), `String.prototype.normalize` (`normalizer_glue`) and a
  `Date` from a fixed time value (UTC);
- forces a GC past one chunk (trap T3);
- checks each result against a hand-written value.

Scenario `mozjs_smoke` lives in a new kernel feature and needs no kernel code change. The bindings' layout asserts
run there too (trap B1).

## 5. Order and gates

1. **#88 (neo-crt) lands, then #87.**
   - #88's gate: the close-gate; a quick tier green on its head (run 42525170759 at `849d2325`: `cxx_runtime_smoke`
     items 30–31 and 108–110, `swgl_smoke`); Dexter's narrow re-check of relay 351; the full four shards at landing.
   - #87's gate: the generator workflow green on its head, with the drift check, the cbindgen check and the
     bindgen step's trap-B1 check; push CI; Dexter's re-check of relay 354.
   - The PM's merge-check confirms the order: #87's generator reads #88's neo-crt (`neo-crt-symbols.txt`,
     `__config_site`). After #88 lands, #87 re-points both at `main`, and the generator re-runs green before #87
     merges.
2. **The generator's last additions.** Already in #87: bindgen, and the patched-file freeze. Still to add:
   `glue_commands.json` (§2 step 3) and the check that the patched crate copies differ from upstream only in
   their manifests and `mozjs-sys/build.rs`. Gate: generator green with the drift check.
3. **The product crates and the Rust closure.** Gate:
   - `xtask ot61-closure-build --mozjs` builds and links the smoke on the host;
   - the build log shows no build script ran other than `mozjs-sys`'s and `libz-sys`'s;
   - push CI passes.
4. **The smoke on target.** Gate: a quick tier with `mozjs_smoke`, then the full four shards before merge. It's
   kernel-touching only through the scenario.

Each step's failure is diagnosable from its own output:
- **step 2:** the generator's drift diff;
- **step 3:** the link gap list (now Rust crates only, #87) and the linker's undefined symbols;
- **step 4:** the smoke prints each check before its value.

## 6. Concurrency

One thread, one runtime. `gCanUseExtraThreads` is false (patch 0003), so there are no helper threads. The pitfall
bank's classes are N/A until a helper thread exists. The canary and TLS notes are #88's and patch 0002's.
