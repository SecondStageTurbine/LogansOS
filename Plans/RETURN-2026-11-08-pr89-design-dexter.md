<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 353: #89 C3c-1 product-build design read

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §353.

**Reviewed:** PR #89 head `103d91222b61b22ea3dfb60e7c8e0c6fb62973ae` ("THINK: the C3c-1 product build (SpiderMonkey
with cc only)"), one file +93 on merge-base `537aa9bbd74992fcebe4204a217197c6814c7ea1`:
`docs/THINK-2026-11-08-ot61-c3c1-product-build.md`. `origin/main` at review time is `069b45a3c4cb28e551a8fd753af8578c2653fbdf`.
Docs only. The design it continues is #10 at `refs/pr/10` `c2d5f3ccb79e3fa05c7838771b618ed92f574968`; the generator
work it builds on is #87 at `refs/pr/87` `db24be390e0196255c5e5bcb8b0637a374c95923`.

**Overall: FAIL.** Item 2 passes. Item 1's "`cc` only" statement is in the THINK but is not yet true of the design:
the closure includes `mozjs_normalizer_glue`, whose build script runs `cbindgen`, and the copied
`mozjs-sys/Cargo.toml` retains build-dependencies (`bindgen`, `cc`, `walkdir`, `flate2`, `tar`). Item 3 fails the
dispatch's own test: no Rust crate is named with its licence, and the closure is incomplete (it omits `write16`,
`utf8_iter`, `icu_provider`, `zerovec`, `displaydoc`, `log` and `num-traits`). Item 4 fails narrowly: step 1 names no
gate. `dtoa.c` is open per the dispatch and is noted, not failed. Neither the architecture nor the order is wrong;
this is a design-completeness fix before the product crate and generator step are built.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs (THINK at `103d91222`; support at `refs/pr/87`) |
|---|---|---|---|
| 1 | The product build runs `cc` only, over the frozen sources and `generated/`: no Python, configure, make, bindgen or network | **FAIL (design gap)** — `THINK:49` states it, but `THINK:55,61-62` puts `mozjs_normalizer_glue` in the closure, and that crate's build script runs `cbindgen`; `THINK:35` copies `mozjs-sys/Cargo.toml`, which still declares `bindgen`, `cc`, `walkdir`, `flate2` and `tar` as build-dependencies | THINK:35-49,53-62; `third_party/ot61-mozjs/vendor/mozjs-138c9470/mozjs-extracted-crates/normalizer_glue/build.rs:1-8`; same crate `Cargo.toml:16-17`; `.../mozjs-sys/Cargo.toml:66-71` |
| 2 | The drift check covers the bindgen output and the patched files, as well as configure's output | **PASS** | THINK:18,20-24,29-31,79; `generator/freeze.py:10-22,192-201`; `.github/workflows/ot61-mozjs-generate.yml:239-269` |
| 3 | Every crate in the Rust closure is named with its licence, and each licence is allowed; `dtoa.c` is a note, not a fail | **FAIL** — the THINK names some crates but no licences, and the closure is incomplete; the licences that can be identified are all allowed | THINK:51-62; `mozjs/Cargo.toml:14,26-31,41-43`; `mozjs-sys/Cargo.toml:43-71`; `normalizer_glue/Cargo.toml:10-17` |
| 4 | Each step names its gate | **FAIL (narrow)** — steps 2, 3 and 4 name theirs; step 1 ("#88 lands, then #87") names none | THINK:78-83 |

## 1. `cc`-only product build — FAIL: the design's own closure runs `cbindgen`

`THINK:49` is the right rule: "Nothing in it runs Python, configure, make, bindgen or the network." §2's four steps
are the right shape: source view (`THINK:37-40`), object view (`THINK:41-43`), compile the 167 frozen entries plus
`jsapi.cpp`/`jsglue.cpp` (`THINK:44-45`), archive and link (`THINK:46-47`). The compile step uses the frozen
`compile_commands.json` arguments, whose compiler is `@REPO@/third_party/ot61-mozjs/generator/neo-c++`, i.e. the
`neo-cc` wrapper over clang (`generator/neo-cc:41-52`). So the statement is right for the product crate's own
`build.rs`.

**But the product build is not just that `build.rs`.** §3's closure includes `mozjs_normalizer_glue`
(`THINK:55,61-62`). That crate is a path dependency of `mozjs_sys` (`mozjs-sys/Cargo.toml:53`) and it has a build
script that runs `cbindgen`:

```
normalizer_glue/build.rs:1  fn main() {
normalizer_glue/build.rs:6      cbindgen::generate(".")
normalizer_glue/build.rs:8          .write_to_file(include_dir.join("normalizer_glue.h"));
normalizer_glue/Cargo.toml:16  [build-dependencies.cbindgen]
normalizer_glue/Cargo.toml:17  version = "0.29.4"
```

Cargo runs that build script when the product crate is built, so the product build would run a third-party header
generator, not `cc` only. The design already has the frozen header in the repo
(`third_party/ot61-mozjs/generator/include/normalizer_glue.h`; #87 checks it against cbindgen at
`.github/workflows/ot61-mozjs-generate.yml:272-294`). The design must say how the product build gets that header
without the crate's build script: patch `mozjs_normalizer_glue` to `build = false` and drop the `cbindgen`
build-dependency, or generate a `patched/normalizer_glue/` copy in the style of `ot61-browser/patched/`. `THINK:35`
mentions only a patched `mozjs-sys`, so this is currently an unnamed third-party step and an unnamed closure crate.

The same paragraph's "copy of upstream's crate (`Cargo.toml`, `src/`)" leaves the copied manifest's build-dependencies
in place:

```
mozjs-sys/Cargo.toml:66  [build-dependencies]
mozjs-sys/Cargo.toml:67  bindgen.workspace = true
mozjs-sys/Cargo.toml:68  cc.workspace = true
mozjs-sys/Cargo.toml:69  walkdir = "2"
mozjs-sys/Cargo.toml:70  flate2 = "1"
mozjs-sys/Cargo.toml:71  tar = "0.4"
```

Even with `build.rs` replaced by ours, Cargo builds every declared build-dependency. `bindgen`, `cc`, `flate2` and `tar`
are not in #81's closure, so the product build would need a new build-dependency closure (and `bindgen` is exactly
the crate the `cc`-only plan is trying to avoid). The copy's `repository.workspace`, `license.workspace` and
`edition.workspace` fields (`mozjs-sys/Cargo.toml:4,8,10`) also need a workspace root, and the path dependencies
(`:52-56`) need re-pointing from `../mozjs-extracted-crates/` to the vendored tree; the THINK does not say the
manifest is normalised. The fix is to state it: the copied `Cargo.toml` is ours, the `[build-dependencies]` block is
removed (the new `build.rs` uses only `std` and invokes `neo-cc`), the workspace fields are resolved, and the path
dependencies point at `vendor/mozjs-138c9470/mozjs-extracted-crates/`.

**A second build-tool gap:** `neo-cc` requires `NEO_REPO` (`generator/neo-cc:19`), and its C++ branch uses
`NEO_CXX_CONFIG` (`:50-54`). The design's object view replaces `@REPO@` in the compile arguments, but it does not say
the product `build.rs` sets `NEO_REPO` (and the libc++ config) for the wrapper. Without that, the compile stops at
`NEO_REPO must name the repository root`. This is a one-line design addition, not a redesign.

**A third compile-input gap:** the 167 frozen entries do not include `src/jsapi.cpp` or `src/jsglue.cpp`. `THINK:45`
says to compile them "the same way", but their flags come from upstream's `get_common_cc` in
`mozjs-sys/build.rs:343-352,548-634`, not from `compile_commands.json`. Freeze those two compile entries (or their
flags) in the generator output, or the product `build.rs` has to re-derive them from upstream's build script — the
third-party build code §2 is replacing. This is the same class as the bindgen output: a build input that must be
frozen or it will drift.

## 2. Drift check — PASS

The design covers all three parts the item names:

- **Bindgen output.** `THINK:20-24`: the generator runs upstream's `build_bindings` for `JSApi` and `JSGlue`, network
  off, with `neo-cc`'s flags and `-D__neoplutarch__`, and freezes the two `.rs` files under `generated/bindings/`;
  "The drift check covers them like the rest." The code matches: `freeze.py:192-201` writes
  `bindings/jsapi.rs` and `bindings/gluebindings.rs` and fails if either is missing, and
  `.github/workflows/ot61-mozjs-generate.yml:239-269` diffs the whole fresh freeze against the committed
  `generated/` and exits 1 on any difference. The workflow's bindgen step also greps for `wasiRecursionDepth` and
  fails trap B1 (`:202-237`).
- **Patched files.** `THINK:29-31`: the ~28 files patches 0001–0003 touch are frozen whole under
  `generated/patched/`, overlaid instead of `git apply`, and "The job checks they equal vendor-plus-patches." The
  29 files in the three patches at `refs/pr/87` match "~28". Because the freeze writes them under `generated/` and
  the job diffs the whole directory, a patched-file change is a drift failure like any other. This is the new part
  of step 2 (`THINK:79`); it is not yet in #87's `freeze.py`, which is why the design is the right place to specify
  it.
- **Configure's output.** The existing `freeze.py` covers `compile_commands.json`, `obj/` (including
  `js-confdefs.h`), `dist_include.txt` and `system_wrappers.txt` (`freeze.py:10-22`), and the workflow diff is the
  same `diff -r` at `:257`. The THINK builds "on #87's job" (`THINK:18`) and says the drift check covers the new
  outputs "like the rest" (`THINK:24`), so the item is satisfied.

Two non-blocking wording notes: (a) `THINK:79` says "The generator gains bindgen", but #87 already has the bindgen
step at `db24be390`; the remaining work is the patched-file freeze. (b) Patch 0001 deletes
`mfbt/MoveOnlyFunction.h`, so "frozen whole" cannot represent a deletion; the file is not compiled and nothing
includes it, so this is a bookkeeping note, not a gate failure. The design should say the overlay is "the patched
files that exist, with 0001's deletion handled by the cut-list/`function2` removal" so step 2's implementer does not
try to freeze a deleted path.

## 3. Rust closure and licences — FAIL: no licence per crate, and the closure is incomplete

`THINK:51-62` names these crates: `encoding_c`, `encoding_c_mem`, `encoding_rs`; `mozjs_normalizer_glue`,
`icu_normalizer` and its data, `icu_collections`, `utf16_iter`, `smallvec`; one of `libz-sys`/`libz-rs-sys`; `libc`;
and says "`mozjs` (the safe layer) adds little more." It then says "with the licence of each in the PR" (`THINK:62`).
The PR is docs-only and contains no licence table, so the dispatch's "named with its licence" is not met. The
licences that can be checked are all allowed, so this is a table to add, not a rejected dependency:

| Crate (as named or needed) | Licence at the pin / crate | On CLAUDE.md's allowed list? | In the THINK? |
|---|---|---|---|
| `encoding_c` 0.9.8 | Apache-2.0 OR MIT (crates.io metadata) | yes | named, no licence |
| `encoding_c_mem` 0.2.6 | Apache-2.0 OR MIT (#87 `VENDORED.md`) | yes | named, no licence |
| `encoding_rs` 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | yes | named, no licence |
| `mozjs_normalizer_glue` 153.6.0 | MPL-2.0 (`normalizer_glue/Cargo.toml:8`) | yes | named, no licence |
| `icu_normalizer` / data 153.6.0 | Unicode-3.0 (`icu_normalizer`/`icu_normalizer_data` manifests) | yes | named, no licence |
| `icu_collections` 153.6.0 | Unicode-3.0 | yes | named, no licence |
| `utf16_iter` 153.6.0 | Apache-2.0 OR MIT | yes | named, no licence |
| `smallvec` 1.16.4 | MIT OR Apache-2.0 | yes | named, no licence |
| `libz-sys` 1.1.31 / `libz-rs-sys` 0.6 | MIT OR Apache-2.0 / Zlib | yes | named as a choice, no licence |
| `libc` 0.2.191 | MIT OR Apache-2.0 | yes | named, no licence |
| `mozjs` 0.26.14 | MPL-2.0 (`mozjs/Cargo.toml:7` inherits workspace) | yes | "adds little more", no licence |
| `log` 0.4.35 | MIT OR Apache-2.0 (`mozjs/Cargo.toml:28`) | yes | **omitted** |
| `num-traits` 0.2.19 | MIT OR Apache-2.0 (`mozjs/Cargo.toml:31`) | yes | **omitted** |
| `write16` 1.0 | Apache-2.0 OR MIT (`normalizer_glue/Cargo.toml:12`) | yes | **omitted** |
| `utf8_iter` 1.1.0 (ICU default feature) | Apache-2.0 OR MIT | yes | **omitted** |
| `icu_provider`, `zerovec`, `displaydoc` (ICU deps) | Unicode-3.0 / Unicode-3.0 / MIT OR Apache-2.0 | yes | **omitted** |
| `cbindgen` 0.29.4 (build-dep) | MPL-2.0 | yes | **omitted** |
| `bindgen` 0.72.1, `cc` 1.3, `walkdir` 2.5.0, `flate2`, `tar` (build-deps) | BSD-3-Clause / MIT OR Apache-2.0 / Unlicense OR MIT / MIT OR Apache-2.0 / MIT OR Apache-2.0 | yes | **omitted** |

The omitted list is not theoretical: `mozjs/Cargo.toml:28,31` are direct dependencies of the safe layer; `THINK:61`
says the safe layer is included. `write16` is a direct dependency of `normalizer_glue` (`Cargo.toml:12`), and
`icu_normalizer`'s default features turn on `utf8_iter` and `utf16_iter` (`icu_normalizer/Cargo.toml` features
`default`), so `utf8_iter` is in the closure. `normalizer_glue` also has the `cbindgen` build-dependency
(`Cargo.toml:16-17`). All of these are allowed, but the PR's table must name them, or the "every crate ... with its
licence" test has no content.

**The safe layer's features are part of the closure and are not stated.** `mozjs/Cargo.toml:14` is
`default = ["jit", "libz-sys", "intl"]`. If the smoke uses `mozjs` with defaults, it turns JIT and intl on, which is
the opposite of C3c-1's "no JIT, no intl" decision (#10 §3.2). If it uses `mozjs` with `default-features = false`, the
design must say so and choose `libz-sys` or `libz-rs` (and `libz-sys` needs its `stock-zlib` feature per
`mozjs-sys/Cargo.toml:61-64`). The same choice applies to `mozjs_sys`'s `libz-*` feature. `THINK:57-58` says "C3c-1
takes the same one" as #81 but does not name which, and `THINK:61` does not name the feature set. This is the one
place where a wrong default would produce a green build of the wrong engine, so it belongs in the design.

**`dtoa.c` (open; not counted as a fail, per the dispatch).** The product build still compiles
`js/src/util/DoubleToString.cpp` (it is one of the non-unified entries in
`generated/compile_commands.json`), and that file includes `dtoa.c` at `DoubleToString.cpp:70`. `dtoa.c`'s notice
(`js/src/dtoa.c:1-19`) is the Lucent/David M. Gay permissive grant, which is not on CLAUDE.md's allowed list. J.c.'s
decision is still pending. The #89 THINK should record that open item next to §2's source view, so the product build
does not look licence-clear when it is not.

## 4. Gates — FAIL (narrow): step 1 names none

`THINK:76-83` is "Order and gates":

1. `THINK:78` — "#88 (neo-crt) lands, then #87 (vendor, generator, frozen outputs)." No gate.
2. `THINK:79` — "gate: generator green with drift check."
3. `THINK:80-81` — "Gate: `xtask ot61-closure-build --mozjs` builds and links the smoke on the host, and push CI
   passes."
4. `THINK:82-83` — "Gate: quick tier with `mozjs_smoke`, and the full four shards before merge (it's kernel-touching
   only through the scenario)."

Steps 2–4 are adequate. Step 1 is the same shape the PM failed in #10's item 5: the vendor/generator and neo-crt
steps name no gate of their own. Here the gates exist (#87's generator-green + drift, #88's own PR gate), but the
design does not name them, so a reader cannot tell whether step 1 is "wait for two green PRs" or "do the vendor and
generator work again". Add "gate: #88's merged tree plus #87's generator job green with the drift check" to step 1
(or mark it explicitly as a prerequisite, not a gated step).

## Verdict

**FAIL, docs fix before step 3.** The freeze-then-`cc` architecture and the four-step order are right. The failing
items are completeness, not direction:

1. Name the `mozjs_normalizer_glue`/`cbindgen` patch and remove the copied `mozjs-sys` build-dependencies (plus
   resolve the copied manifest's workspace/path fields).
2. Add the licence table for the full Rust closure, including the safe layer's `log`/`num-traits`, `write16`,
   `utf8_iter`, the ICU provider/zerovec/displaydoc set, and the build-dependencies; state the `default-features`
   and zlib choice.
3. Name step 1's gate.
4. Note `dtoa.c`'s pending decision.

No code or gate was run: the PR is one `docs/` file, and the branch is docs-only, so the push-CI paths-ignore applies.
The review used the #89 head tree, the #87 generator tree at `db24be390`, and the #10 design tree at `c2d5f3cc`; no
clone of `servo/mozjs` was needed beyond the vendored copy in #87.
