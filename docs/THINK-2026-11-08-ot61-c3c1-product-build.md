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
- **The patched files.** The ~28 files patches 0001–0003 touch are frozen whole under `generated/patched/`. That way
  the product build overlays them on the vendored tree instead of running `git apply`. The job checks they equal
  vendor-plus-patches.

## 2. The product crate

`third_party/ot61-mozjs/patched/mozjs-sys/` is a copy of upstream's crate (`Cargo.toml`, `src/`). It follows the
`ot61-browser/patched/` precedent. Its `build.rs` is ours:
1. **Source view.** In `OUT_DIR/src`, mirror the compiled directories of the vendored tree (`js/src`, `js/public`,
   `mfbt`, `mozglue`, `memory`, `modules/fdlibm`, `modules/zlib/src`, `third_party/fmt`, `config`, `build`; about
   65 MB), with `generated/patched/` laid over them. These are hard links where the filesystem allows, copies
   otherwise.
2. **Object view.** In `OUT_DIR/obj`, write `generated/obj/` with `@SRC@`/`@OBJ@`/`@REPO@` replaced. Make
   `dist/include` from `dist_include.txt`, pointing at the source view, so patched headers win. Write each
   `dist/system_wrappers/<name>` from the one template.
3. **Compile.** Compile the 167 entries of `compile_commands.json` with the frozen arguments, through `neo-cc`'s
   flags (the target ABI last). Then compile `src/jsapi.cpp` and `src/jsglue.cpp` the same way.
4. **Archive and link.** Archive the objects into `libjs_static.a`, `libjsapi.a` and `libjsglue.a`, include the
   frozen bindings, and emit the same `cargo:rustc-link-lib` lines upstream does.

**Nothing in it runs Python, configure, make, bindgen or the network.**

## 3. The Rust closure

`mozjs_sys` needs:
- `encoding_c`, `encoding_c_mem` and `encoding_rs`;
- `mozjs_normalizer_glue`, which brings `icu_normalizer` and its data, `icu_collections`, `utf16_iter` and
  `smallvec` (all in `mozjs-extracted-crates/` or on crates.io);
- one zlib, of `libz-sys`/`libz-rs-sys`. #81's L0 closure has `libz-sys-1.1.31` and a `zlib-rs` stub; C3c-1 takes
  the same one.
- `libc`: our target's, as the rest of the bundle uses.

`mozjs` (the safe layer) adds little more. All of it is vendored like #81's closure (`.cargo-checksum.json` per
crate, so `vendor-verify` covers it), with the licence of each in the PR.

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

1. #88 (neo-crt) lands, then #87 (vendor, generator, frozen outputs).
2. The generator gains bindgen and the patched-file freeze. That's one PR; gate: generator green with drift check.
3. The product crate and the Rust closure. Gate: `xtask ot61-closure-build --mozjs` builds and links the smoke
   on the host, and push CI passes.
4. The smoke on target. Gate: quick tier with `mozjs_smoke`, and the full four shards before merge (it's
   kernel-touching only through the scenario).

Each step's failure is diagnosable from its own output:
- **step 2:** the generator's drift diff;
- **step 3:** the link gap list (now Rust crates only, #87) and the linker's undefined symbols;
- **step 4:** the smoke prints each check before its value.

## 6. Concurrency

One thread, one runtime. `gCanUseExtraThreads` is false (patch 0003), so there are no helper threads. The pitfall
bank's classes are N/A until a helper thread exists. The canary and TLS notes are #88's and patch 0002's.
