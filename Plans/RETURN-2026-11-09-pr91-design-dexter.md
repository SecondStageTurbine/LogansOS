<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 367: #91 re-check (C3c-1 product build THINK, "as built" §2 and relay-362 notes)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-359-362.md` §367.

**Reviewed:** PR #91, branch `claude/ot61-c3c1-think-followup`, at the dispatched head
`c0be27dd2fb1f8742fb7780a841c7449891f73d8` ("THINK C3c-1 product build: step 3 needs #81 (libc, libz-sys) and
patch 0005; its gate passed on the try branch"). The branch's range `145059d5b..c0be27dd` is two commits, one
file, +38/-13: `docs/THINK-2026-11-08-ot61-c3c1-product-build.md`. `origin/main` at review time is
`d247d28375733ef163f0cbbc4f5ccc48fb69759d`, ahead of the branch base `145059d5b`; the branch's own change is
still the one docs file.

The "as built" §2 is checked against `claude/ot61-c3c1-product-crates` at
`2761bb71bd3cb40b993a12a746e3fd5029e6c34f` ("OT-61 C3c-1: the product build's patched crates (mozjs, mozjs-sys,
normalizer_glue)"). For the build-script allow-list I also read `claude/ot61-c3c1-mozjs-try` at
`ee40505ee8a56c4c51b89a254679fd2759982553`, the try branch §5 cites.

**Overall: PASS, both items.** The three patched copies hold only their manifests plus the one
`mozjs-sys/build.rs` that §2 itself names; the product build runs no Python, configure, bindgen or network step.
Every relay-362 note has a corresponding fix at `c0be27dd`. Two non-blocking wording points are in §Notes.

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | "As built" §2 matches `claude/ot61-c3c1-product-crates`: patched copies manifest-only; nothing runs Python, configure, bindgen or the network | **PASS** | THINK:44-57,59-91; `third_party/ot61-mozjs/patched/` (4 files under `2761bb71b`); `patched/mozjs/Cargo.toml:22,28`; `patched/normalizer_glue/Cargo.toml:21,24`; `patched/mozjs-sys/Cargo.toml:24,28`; `patched/mozjs-sys/build.rs:2,23,301-309`; `generator/check_patched_crates.py:58-102`; `ot61-mozjs-generate.yml:333-337` |
| 2 | Every relay-362 note is answered | **PASS** | THINK:95-96 (48); THINK:84-91 and :152-155 (build-script gate/allow-list); THINK:117-120 (build-dependencies); note 4 was a review-process statement |

## 1. "As built" §2 matches the product-crates branch — PASS

### 1.1 The patched copies hold only their manifests (plus the one named `build.rs`)

`third_party/ot61-mozjs/patched/` at `2761bb71b` has exactly four files:

```
patched/mozjs/Cargo.toml
patched/mozjs-sys/Cargo.toml
patched/mozjs-sys/build.rs
patched/normalizer_glue/Cargo.toml
```

There is no `src/` copy under any of them, which is what THINK:44-46 says: "A copy holds only its manifest, and
for `mozjs-sys` its `build.rs`. Its `[lib] path` is the vendored crate's own `src/lib.rs`". The three manifests
point at the vendored sources, and all three targets exist at `2761bb71b`:

- `patched/mozjs/Cargo.toml:28` -> `../../vendor/mozjs-138c9470/mozjs/src/lib.rs`;
- `patched/mozjs-sys/Cargo.toml:28` -> `../../vendor/mozjs-138c9470/mozjs-sys/src/lib.rs`;
- `patched/normalizer_glue/Cargo.toml:24` ->
  `../../vendor/mozjs-138c9470/mozjs-extracted-crates/normalizer_glue/src/lib.rs`.

The rest of the §2 manifest claims also hold at that head. `mozjs` has `build = false`
(`patched/mozjs/Cargo.toml:22`) and `default = ["libz-sys"]` (`:32`); `normalizer_glue` has `build = false` and
no `links` (`patched/normalizer_glue/Cargo.toml:21`), and only its manifest is present; `mozjs-sys` has
`build = "build.rs"` (`patched/mozjs-sys/Cargo.toml:24`), the six configure-option features are empty
(`:34-37,39-40`), and its dependencies are upstream's minus the dropped optional ones (`:44-52`). None of the three
manifests has a `[build-dependencies]` block, and the workspace fields are written out (MPL-2.0, edition 2021).
The `build.rs` rejects a feature outside the frozen configuration at `patched/mozjs-sys/build.rs:82-85`.

### 1.2 Nothing in the product build runs Python, configure, bindgen or the network

`patched/mozjs-sys/build.rs` says and does this at `:2` ("std only") and `:23` ("It runs no Python, configure,
make, bindgen, cbindgen, and no network"). Statically, the whole file imports only `std` (`:27-35`); it executes
just the compiler wrapper, `clang`, and `llvm-ar` (`:301-309`, `:210-227`). The compile set is the already-frozen
`compile_commands.json` (167 entries; I counted them in the committed file) plus the two
`glue_commands.json` entries, both through `generator/neo-cc`/`neo-c++` (`:185-207`). No `configure`, `make`,
`bindgen`, `cbindgen`, Python or network call appears in the build.

The one Python edge in that path is `generator/neo-cc:42-45`: the wrapper appends its caller's arguments to
`NEO_CC_LOG` with `python3` when that variable is set. The product build closes it explicitly:
`patched/mozjs-sys/build.rs:309` does `.env_remove("NEO_CC_LOG")` on every compile, so the logging branch cannot
run. (The log is the generator's tool, not the product build's.)

`generator/check_patched_crates.py` is Python, but it runs in the generator job only
(`.github/workflows/ot61-mozjs-generate.yml:333-337`), not in `mozjs-sys`'s build. I ran it against the four
copied files and the vendored manifests at `2761bb71b`; it exits 0 and prints
`check_patched_crates: 3 copies differ from upstream only in their manifests (and mozjs-sys's build.rs)`. The
check enforces the allowed-file set (`check_patched_crates.py:65-71`), the vendored `[lib] path` (`:78-80`), no
`[build-dependencies]` (`:81-82`), the build key (`:83-85`) and the dependency/version set (`:86-97`). That is
the check THINK:55-57 describes.

The other build scripts in the product closure are also clean of those steps. The try branch's allow-list
(`tools/xtask/src/main.rs:8998-9019`) names `neo-crt`, `neo-std-pal`, `mozjs_sys`, `libz-sys`, `encoding_c`,
`encoding_c_mem`, `encoding_rs`, `libc`, `mozjs_icu_normalizer_data`, `num-traits`, `proc-macro2`, `quote` and
`rustversion`, which is the list THINK:84-91 describes. `neo-crt/build.rs` only compiles musl and libc++ with
`cc`/clang; `libz-sys`'s build script compiles its bundled zlib; the rest only probe rustc or print metadata.
`verify_mozjs_build_scripts` (`tools/xtask/src/main.rs:9021-9075`) fails the gate on any other build script, so
a future one that did run Python/configure/bindgen/network could not slip in silently.

## 2. Every relay-362 note is answered — PASS

Relay-362's four notes are at `Plans/RETURN-2026-11-08-pr89-recheck-dexter.md` §"Notes (non-blocking)". Each is
answered at `c0be27dd`:

1. **Closure count 46 -> 48.** THINK:95-96 now says "There are 48 crates", matching the table and the resolved
   closure. ANSWERED.
2. **The step-3 build-script gate was literally false.** The gate at THINK:154 now says "xtask's list of the
   build scripts that ran holds only the allowed ones (§2)", and §2 itself names the allow-list at THINK:84-91.
   That is the fix the note asked for (name the allow-list, or say "no non-Rust build step"). ANSWERED. The try
   branch's `MOZJS_BUILD_SCRIPTS` and `verify_mozjs_build_scripts` implement it
   (`tools/xtask/src/main.rs:8998-9019,9021-9075`).
3. **The build-dependency scope was not named.** THINK:117-120 now names the build-dependencies that come on top
   of the `-e normal` closure: `libz-sys`'s `cc`, `pkg-config` and `vcpkg`, and `cc`'s `find-msvc-tools` and
   `shlex`, all MIT OR Apache-2.0. The note's optional `jobserver` is correctly absent: `cc`'s default features
   are empty (`third_party/ot61-swgl/vendor/cc-1.2.63/Cargo.toml:42-47`), `libz-sys` does not enable `parallel`,
   and `jobserver` is not in the smoke's `Cargo.lock`. ANSWERED.
4. **No CI run was needed** was a review-process statement, not a THINK defect; it remains true for this
   docs-only PR. No change was required, and none is missing.

The new head commit also adds the step-3 prerequisites at THINK:157-162 (#81's patched `libc`, #81's `libz-sys`
1.1.31 / `pkg-config` / `vcpkg`, and patch 0005 for the duplicate `abort`), which is consistent with the
product-crates and try-branch state.

## Notes (non-blocking)

- **`LIBZ_SYS_STATIC=1` (THINK:85-86).** The no-system-probe property is real, but the mechanism named is not the
  one used: the smoke enables libz-sys's `static` feature
  (`userland/mozjs-smoke/Cargo.toml:31`), and `LIBZ_SYS_STATIC` is never set anywhere in the branches I read.
  libz-sys's `should_link_static` reads the env value first and falls back to `cfg!(feature = "static")`
  (`third_party/ot61-browser/vendor/libz-sys-1.1.31/build.rs:260-271`), so the build does skip pkg-config. The
  line should say "with the `static` feature" rather than "`LIBZ_SYS_STATIC=1`".
- **`NEO_CXX_CONFIG` (THINK:72-73).** The build script sets `NEO_REPO` and `NEO_CLANG` explicitly
  (`patched/mozjs-sys/build.rs:307-308`) but leaves `NEO_CXX_CONFIG` to the wrapper's default
  (`generator/neo-cc:63`, `$repo/userland/neo-crt/cxx`). On the try branch that default is the same
  localization-on `__config_site` the generator's `$cfg` copy has, so the build is consistent; the sentence
  would be precise as "with `NEO_REPO` and `NEO_CLANG` set (and `NEO_CXX_CONFIG` honoured when set)".

## No CI run

#91 is one `docs/` file, so push CI's paths-ignore applies (`.md` / `docs/**`). Everything here was read-only:
`git show`/`ls-tree`/`grep`, one `cargo`-free Python run of the generator's own check script over the extracted
four-file copy, and `git diff`. No repo build or test was run or needed.

## Verdict

**PASS, both §367 items** at `c0be27dd2`: the "as built" §2 matches `claude/ot61-c3c1-product-crates`
(`2761bb71b`) — the patched copies are manifest-only plus the one `mozjs-sys/build.rs`, and nothing in the
product build runs Python, configure, bindgen or the network — and all four relay-362 notes are answered. The
two wording notes above do not change the design or the `cc`-only property.
