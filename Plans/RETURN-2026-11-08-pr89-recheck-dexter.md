<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 362: #89 re-check (`cc` only, licences, step 1's gate)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-359-362.md` §362.

**Reviewed:** PR #89, branch `claude/ot61-c3c1-product-build-think`, at head
`f52503e77c1fbf9d9bd50749ad30f5360313e5bc` ("THINK C3c-1 product build: Dexter's design read (relay 353)").
One file, +149 on merge-base `537aa9bbd74992fcebe4204a217197c6814c7ea1`:
`docs/THINK-2026-11-08-ot61-c3c1-product-build.md`. `origin/main` at review time is
`1fb76e0140dc3d977827b15660b0d522c2a3220c`.

The design's support tree is #87. The dispatch's §361 names `bbdd374c1`, but #87 has moved to
`3fc087fb8` ("OT-61 C3c-1 generator: tokenise the crate root (@CRATES@); a stricter leak check"); the
closure and the `normalizer_glue` header are unchanged between `db24be390` (the #89 THINK's citation),
`bbdd374c1` and `3fc087fb8` (`git diff db24be390 3fc087fb8 -- third_party/ot61-mozjs/vendor` changes no
manifest, and the only generator change is the workflow). I used `3fc087fb8` for the manifests, the frozen
header and the cbindgen check. CLAUDE.md's allowed-licence list is read at `1fb76e014` (lines 915-918).

**Overall: PASS, all three items.** `mozjs_normalizer_glue` now has no build script that runs cbindgen; every
crate the product closure actually resolves is named with an allowed licence; step 1 names both prerequisite
gates. Two non-blocking wording notes are in §Notes: the closure count says 46 but the table and the real closure
both have 48 crates, and the step-3 gate's "no build script other than mozjs-sys's and libz-sys's" is literally
false for the closure's Rust-only build scripts.

| # | Dispatch item | Verdict | Enforcing refs (THINK at `f52503e77`; support at `3fc087fb8`) |
|---|---|---|---|
| 1 | "`cc` only" is now true: `mozjs_normalizer_glue` is either frozen or built without Python, configure or bindgen | **PASS** | THINK:44-49,54-55,77; `.../normalizer_glue/Cargo.toml:3,16-17`; `generator/include/normalizer_glue.h`; `ot61-mozjs-generate.yml:309-331` |
| 2 | Every crate in the Rust closure is named with an allowed licence | **PASS** (per crate below) | THINK:83-85,89-103; `cargo tree -p mozjs --no-default-features --features libz-sys -e normal` at `3fc087fb8`; `CLAUDE.md:915-918` |
| 3 | Step 1 names its gate | **PASS** | THINK:123-130 |

## 1. `mozjs_normalizer_glue` no longer runs cbindgen — PASS

The upstream crate is the blocker from relay 353: its manifest declares `links = "normalizer_glue"`
(`.../normalizer_glue/Cargo.toml:3`) and a `cbindgen 0.29.4` build-dependency (`:16-17`), and its `build.rs` runs
`cbindgen::generate` to write `include/normalizer_glue.h`. The head now removes that path from the product build:

- **The patched copy.** THINK:44-49 says each of the three upstream crates gets a copy under
  `third_party/ot61-mozjs/patched/`, and that in all three manifests "the `[build-dependencies]` blocks are gone".
  That removes the `cbindgen` build-dependency, so Cargo has nothing to build for it.
- **`build = false` and no `links`.** THINK:54-55: "`mozjs_normalizer_glue`: `build = false`, and no `links` key."
  `build = false` disables Cargo's automatic `build.rs` discovery even though the patched copy still contains the
  upstream `build.rs`; removing `links` also drops the native-link declaration that upstream's build script served.
  The crate's one generated output, `normalizer_glue.h`, is frozen at
  `third_party/ot61-mozjs/generator/include/normalizer_glue.h`.
- **The frozen header is still checked, in CI only.** THINK:55 says it is "checked against cbindgen 0.29.4 on every
  generator run (#87)". That is present at `3fc087fb8`:
  `ot61-mozjs-generate.yml:314-331` installs `cbindgen --version 0.29.4 --locked`, runs it over the vendored
  crate's `src/lib.rs`, strips comments, and diffs the result against the committed header. That is the generator
  job, not the product build, so it does not add a cbindgen step to `cc`.
- **The blanket statement.** THINK:77 says nothing in the three crates runs Python, configure, make, bindgen,
  cbindgen or the network. For `mozjs_normalizer_glue`, the patched copy's only replaced inputs are its manifest
  and the frozen header, so the statement is true of this crate.

PASS: there is no product-build path in `mozjs_normalizer_glue` that runs cbindgen, Python, configure or bindgen.
The cbindgen invocation that remains is the CI generator's drift check, which is the allowed place for it.

## 2. Every crate in the closure, with its licence — PASS per crate

The THINK gives the exact resolution command (THINK:83), `cargo tree -p mozjs --no-default-features --features
libz-sys -e normal`, which is the no-JIT/no-intl feature set the design chose. Running it at `3fc087fb8` resolves
**48** crates; the table at THINK:89-103 names the same 48, with no missing and no extra crate (I compared the
table's backticked names to `cargo tree`'s package set). Every declared licence below is on CLAUDE.md's list
(MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0, dtoa/Lucent, Unlicense/CC0 at `CLAUDE.md:915-918`).

| Crate | Version | Declared licence | Verdict |
|---|---|---|---|
| `mozjs` | 0.26.14 | MPL-2.0 | PASS — allowed |
| `mozjs_sys` | 153.6.0-2 | MPL-2.0 | PASS — allowed |
| `mozjs_normalizer_glue` | 153.6.0 | MPL-2.0 | PASS — allowed |
| `mozjs_icu_normalizer` | 153.6.0 | Unicode-3.0 | PASS — allowed |
| `mozjs_icu_normalizer_data` | 153.6.0 | Unicode-3.0 | PASS — allowed |
| `mozjs_icu_collections` | 153.6.0 | Unicode-3.0 | PASS — allowed |
| `mozjs_utf8_iter` | 153.6.0 | Apache-2.0 OR MIT | PASS — allowed |
| `mozjs_utf16_iter` | 153.6.0 | Apache-2.0 OR MIT | PASS — allowed |
| `icu_casemap` | 2.1.1 | Unicode-3.0 | PASS — allowed |
| `icu_collections` | 2.1.1 | Unicode-3.0 | PASS — allowed |
| `icu_locale_core` | 2.4.0 | Unicode-3.0 | PASS — allowed |
| `icu_properties` | 2.1.2 | Unicode-3.0 | PASS — allowed |
| `icu_provider` | 2.4.0 | Unicode-3.0 | PASS — allowed |
| `litemap` | 0.8.3 | Unicode-3.0 | PASS — allowed |
| `potential_utf` | 0.1.6 | Unicode-3.0 | PASS — allowed |
| `tinystr` | 0.8.5 | Unicode-3.0 | PASS — allowed |
| `writeable` | 0.6.4 | Unicode-3.0 | PASS — allowed |
| `yoke` | 0.8.4 | Unicode-3.0 | PASS — allowed |
| `zerofrom` | 0.1.8 | Unicode-3.0 | PASS — allowed |
| `zerotrie` | 0.2.5 | Unicode-3.0 | PASS — allowed |
| `zerovec` | 0.11.9 | Unicode-3.0 | PASS — allowed |
| `yoke-derive` | 0.8.4 | Unicode-3.0 | PASS — allowed |
| `zerofrom-derive` | 0.1.8 | Unicode-3.0 | PASS — allowed |
| `zerovec-derive` | 0.11.7 | Unicode-3.0 | PASS — allowed |
| `encoding_c` | 0.9.8 | Apache-2.0 OR MIT | PASS — allowed |
| `encoding_c_mem` | 0.2.6 | Apache-2.0 OR MIT | PASS — allowed |
| `encoding_rs` | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | PASS — allowed |
| `libz-sys` | 1.1.32 | MIT OR Apache-2.0; bundled zlib under the zlib licence | PASS — allowed |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | PASS — allowed |
| `log` | 0.4.35 | MIT OR Apache-2.0 | PASS — allowed |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 | PASS — allowed |
| `smallvec` | 1.16.5 | MIT OR Apache-2.0 | PASS — allowed |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | PASS — allowed |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 | PASS — allowed |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 | PASS — allowed |
| `simdutf8` | 0.1.5 | MIT OR Apache-2.0 | PASS — allowed |
| `write16` | 1.0.0 | Apache-2.0 OR MIT | PASS — allowed |
| `core_detect` | 1.0.0 | MIT/Apache-2.0 (the slash form of MIT OR Apache-2.0) | PASS — allowed |
| `multiversion` | 0.9.2 | MIT OR Apache-2.0 | PASS — allowed |
| `multiversion_no_op` | 1.0.0 | Apache-2.0 OR MIT | PASS — allowed |
| `displaydoc` | 0.2.7 | MIT OR Apache-2.0 | PASS — allowed |
| `multiversion-macros` | 0.9.2 | MIT OR Apache-2.0 | PASS — allowed |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | PASS — allowed |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | PASS — allowed |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | PASS — allowed |
| `syn` | 3.0.9 | MIT OR Apache-2.0 | PASS — allowed |
| `synstructure` | 0.14.0 | MIT | PASS — allowed |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | PASS — allowed |

The versions are the current compatible resolutions, as the THINK says; the product lockfile pins them. The table's
ranges cover them. `dtoa.c` is not a Rust crate and is not in this checklist, but the THINK's §3 note is accurate:
J.c.'s 2026-11-08 ruling puts the dtoa/Lucent notice on the allowed list (`CLAUDE.md:916-917`), and #87 records it
at `third_party/ot61-mozjs/VENDORED.md:147-148` (the `js/src/dtoa.c` row and the `DoubleToString.cpp:70` link).

No crate in the resolved closure has a GPL/LGPL/AGPL licence, and none is unlicensed or outside the allowed list.

## 3. Step 1 names its gate — PASS

THINK:123-130 is now explicit gate-by-gate for the first step:

- **#88 (neo-crt) gate** (THINK:124-125): the close-gate; a quick tier green on its head (run `42525170759` at
  `849d2325`, naming `cxx_runtime_smoke` items 30-31 and 108-110 and `swgl_smoke`); Dexter's narrow re-check of
  relay 351; the full four shards at landing.
- **#87 gate** (THINK:126-127): the generator workflow green on its head, with the drift check, the cbindgen check
  and the bindgen step's trap-B1 check; push CI; Dexter's re-check of relay 354.
- **The order is part of the gate** (THINK:128-130): #87's generator reads #88's neo-crt, so #88 lands first, #87
  re-points both inputs at `main`, and the generator re-runs green before #87 merges.

That is a gate for step 1 on both sides, not a bare "wait for two green PRs". PASS.

## Notes (non-blocking)

- **Closure count (THINK:83-84):** the sentence says "There are 46 crates", but the table at THINK:89-103 has 48
  names and `cargo tree` resolves 48. Every crate is named, so item 2 passes; the number should be corrected to
  48 when the THINK is next touched.
- **Step 3's build-script gate (THINK:136):** "the build log shows no build script ran other than `mozjs-sys`'s and
  `libz-sys`'s" is literally false for this closure. Cargo also runs the Rust-only build scripts of `encoding_c`
  (`build.rs` emits `cargo:include-dir`), `encoding_c_mem` (same), `encoding_rs` (`rustc-check-cfg`/cfg),
  `libc`, `num-traits`, `mozjs_icu_normalizer_data` (an `ICU4X_DATA_DIR` cfg probe), `proc-macro2`, `quote` and
  `rustversion`. None runs Python, configure or bindgen, so the `cc`-only property is unaffected and item 1 passes;
  the step-3 gate should say "no non-Rust build step" or name the allow-list, or it will fail on a correct build.
- **Build-dependency scope (THINK:83-85):** the 48-row table is the `-e normal` closure, which is what the THINK's
  command resolves and what item 2's per-crate table checks. `libz-sys`'s build-dependencies are outside it: `cc`,
  `pkg-config` and `vcpkg` are already recorded with allowed licences in #81's
  `third_party/ot61-browser/VENDORED.md` (lines 229, 364, 517); `cc`'s own `find-msvc-tools` and `shlex` (and the
  optional `jobserver`) are not in the #89 table. If the PM wants the full build closure named in #89 rather than
  inherited from #81, those are the remaining rows; all are MIT/Apache-2.0, so the licence verdict does not change.
- **No CI run was needed:** #89 is one `docs/` file, so the push-CI paths-ignore applies. The only commands run were
  read-only (`git show`/`diff`, `cargo tree`/`metadata` against #87's tree at `3fc087fb8`).

## Verdict

**PASS, all three §362 items** at `f52503e77`: `mozjs_normalizer_glue` is built without a cbindgen/Python/configure
build step, all 48 closure crates are named with an allowed licence, and step 1 names its gate. The notes above
are wording/scope fixes, not design gaps and not merge blockers.
