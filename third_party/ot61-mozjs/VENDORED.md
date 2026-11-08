<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `ot61-mozjs`: SpiderMonkey, as Servo packages it (OT-61 C3c-1)

The JavaScript engine for the browser: **SpiderMonkey 153.6.0** in Servo's packaging, `servo/mozjs`. That is
`mozjs-sys`, the C++ engine with its build tree and `bindgen` bindings, and `mozjs`, the safe Rust layer Servo's
`script` is written against.

The rung's design and the PM's decisions are in `docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md` (#10): one realm,
one thread, interpreter only, and build option (c).

## Provenance

| | |
|---|---|
| Upstream | `https://github.com/servo/mozjs` |
| Commit | `138c9470151b8dcbb335f249cb83f9404857baf9` (2026-11-01) |
| Crate versions | `mozjs_sys` 153.6.0-2; engine 153.6.0 (`mozjs-sys/mozjs/config/milestone.txt`) |
| Fetched | `git clone --depth 1 --filter=blob:none`, then `git ls-files` copied with `tar` (no summarizing fetch) |
| Maintainer of our copy | Seat D. It moves with `mozjs` releases, and each move re-runs the §2.2 licence table and the generator. |

## Layout

- `vendor/mozjs-138c9470/` is upstream at that commit, **unmodified**, minus the paths below. Nothing in it is
  edited. The SPDX check skips `third_party/*/vendor/*` for that reason.
- `patches/` holds our changes, applied in order onto a copy at generation time, never to `vendor/`:
  - `0001-mfbt-drop-function2.patch` deletes `mfbt/MoveOnlyFunction.h` and its two `mfbt/moz.build` export lines.
    `third_party/function2` is under the Boost Software License 1.0, which is not on CLAUDE.md's vendoring list,
    so it isn't vendored. mozbuild checks that every exported file exists, so the export has to go too. Nothing
    in `js/src`, `mfbt` or `mozglue` includes `MoveOnlyFunction.h` (THINK §2.2).
  - `0002-configure-neoplutarch-target.patch` teaches configure our triple, `x86_64-unknown-neoplutarch` (THINK
    trap T6). `config.sub` passes it through, `split_triplet` maps it to kernel and OS `NeoPlutarch`, and the
    kernel's preprocessor check is `__neoplutarch__`. Like WASI, it is accepted only for `--enable-project=js`.
  - `0003-neoplutarch-source-choices.patch`: where upstream picks code with `__wasi__`, our target takes WASI's
    branch only where the POSIX one needs something neo-crt doesn't have:
    - `mmap`: GC chunks, ArrayBuffers, `TaggedAnonymousMemory`, executable memory, `MmapFaultHandler`;
    - `getpid`: assertions, the base profiler;
    - `tzset`: `DateTime`, two testing functions;
    - signals: `WasmSignalHandlers`, plus no helper threads in `Runtime.cpp`;
    - the native-stack base (no `pthread_getattr_np`): the stack-limit cluster in `StackLimits.h`,
      `NativeStackLimits.h`, `RootingAPI.h`, `NativeStack.cpp`, `jsapi.cpp`, `FrontendContext` and `JSContext.cpp`;
    - `arc4random_buf` for `RandomNum`.

    Everything else (atomics, `ThreadLocal`, mutexes, condition variables) stays POSIX. `Poison.cpp` gets its own
    small `__neoplutarch__` branch, because WASI's calls a wasm builtin.
  - `0004-jsglue-neoplutarch.patch`: `mozjs-sys/src/jsglue.cpp` (the Rust glue, compiled outside SpiderMonkey's own
    build) picks `<malloc.h>` and `malloc_usable_size` by platform. Our target joins the Linux/WASI branch, since
    musl has the header and neo-crt defines the function.
- `generator/neo-cc` (and `neo-c++`, a symlink) is the compiler the generator gives configure for our target:
  neo-crt's own compile line (clang for `x86_64-unknown-none-elf` over musl's and libc++'s headers) with
  `__neoplutarch__` predefined. configure's test programs link against a stub that defines exactly the symbols in
  `generator/neo-crt-symbols.txt`: neo-crt's, plus Rust's `compiler_builtins`. So a function check passes only
  for a function a LogansOS binary really has.
- Nothing builds from this tree yet. The CI generator job (option (c), next) runs `configure` here with the network
  off, on the CI runners only (J.c., 2026-10-29), and commits its frozen outputs. The product build then runs `cc`
  over those outputs. No other build reads this tree, and the default kernel, image and userland are unchanged.

## Also vendored: `vendor/encoding_c_mem-0.2.6/`

`mfbt/Latin1.h` includes `encoding_rs_mem.h`, the C header of the `encoding_c_mem` crate (`mozjs-sys/Cargo.toml`:
`encoding_c_mem = "0.2.6"`). `mozjs-sys`'s `build.rs` puts the crate's `include/` on the compile line, and the generator
does the same. The crate is also what links those functions in, so the product build needs its Rust side too.

| | |
|---|---|
| Upstream | `https://github.com/hsivonen/encoding_c_mem`, crates.io `encoding_c_mem` 0.2.6 |
| Fetched | `https://static.crates.io/crates/encoding_c_mem/encoding_c_mem-0.2.6.crate`, sha256 `3a80a16821fe8c7cab96e0c67b57cd7090e021e9615e6ce6ab0cf866c44ed1f0`, unpacked unmodified |
| Licence | Apache-2.0 OR MIT (`LICENSE-APACHE`, `LICENSE-MIT`, `COPYRIGHT`) |
| Freshness | 0.2.6 (2020-05-13) is the latest release. It's a small, finished binding by `encoding_rs`'s author, and Firefox still links it. Seat D maintains our copy. |

## `mozjs-extracted-crates/` is vendored too

The first version of this copy left `mozjs-extracted-crates/` out as release tooling. That was wrong:
`mozjs_normalizer_glue` is a required dependency of `mozjs-sys` (`String.prototype.normalize`), and the ICU4X crates
it uses live there. It is now copied unmodified: 204 files, each checked against upstream's index.
- **Licences:** MPL-2.0 (the `*_glue` crates and `unicode_bidi_ffi`), Unicode-3.0 (`icu_*`), Apache-2.0 OR MIT
  (`utf8_iter`, `utf16_iter`).
- **Built for C3c-1:** only `normalizer_glue` and what it uses (`icu_normalizer` and its data, `icu_collections`,
  `utf16_iter`). The rest are intl-only.

`normalizer_glue.h`, which upstream's `build.rs` writes with cbindgen, is in `generator/include/`. It's written by
hand from the crate's `lib.rs` and `cbindgen.toml` until the generator produces it and checks the two match.

## libc++ with localization

SpiderMonkey's headers and irregexp use `<ostream>`/`<sstream>`/`<iomanip>`, so it needs neo-crt's libc++ with
localization. That change is its own PR (`claude/ot61-c3a-localization`; C3a THINK §10). Until it lands, the
generator's compile probe points `neo-cc` (`NEO_CXX_CONFIG`) at a copy of `__config_site` with localization on.

## The frozen outputs: `generated/`

This is what the product build reads instead of running `configure`: option (c), THINK #10 §6. CI writes it
(`generator/freeze.py`) after `configure` and `make export` for `x86_64-unknown-neoplutarch`, with the network off.
Every generator run freezes the outputs afresh, diffs them against this copy, and fails on any difference. A change
to the vendored tree, the patches, `neo-cc` or neo-crt's symbols therefore has to come with a matching `generated/`.
- **`compile_commands.json`:** one entry per object. There are 167: 81 unified files and the sources no unified
  file includes, the same set `make` compiled in the probe. Each entry carries its directory, source and compiler
  arguments.
- **`obj/`:** the generated headers and unified sources, 96 files. Among them are `js-confdefs.h`, `js-config.h`,
  `selfhosted.out.h` and the `jit/*Generated.h` tables.
- **`dist_include.txt`:** the 396 links of `dist/include`, as `name -> source`.
- **`system_wrappers.txt`:** the 1,061 names in `dist/system_wrappers`. Each wrapper is the same four lines
  (`#pragma GCC system_header`, `visibility push(default)`, `#include_next`, `visibility pop`).

Paths are `@SRC@` (the patched source root), `@OBJ@` and `@REPO@`. The first copy is from run 42518384967 at
`6b70a914`: a gzip'd tar with sha256 `e29faac123278a3b69bd3d748a20226a741fd3bbc0204f5e430ae6cd56f63975`, taken from
the job log and checked against that hash.

## Left out of the copy

| Path | Why |
|---|---|
| `mozjs-sys/mozjs/third_party/function2/` | BSL-1.0, not on the vendoring list; reached only through `MoveOnlyFunction.h` (patch 0001) |
| `mozjs-sys/mozjs/js/src/zydis/` | The x86 disassembler, built only with `JS_JITSPEW` (`js/src/moz.build:351`); C3c-1 has no JIT |
| `mozjs-sys/mozjs/third_party/gemmology/`, `third_party/xsimd/` | wasm intgemm only (`ENABLE_WASM_MOZ_INTGEMM`); wasm is off |
| `mozjs-sys/mozjs/nsprpub/` | Standalone JS doesn't build NSPR unless `--enable-nspr-build` |
| `.gitattributes` (the repository's own) | Git metadata. It sets `text=auto eol=lf`, but upstream stores four files with CRLF in its own index. Our `vendor/.gitattributes` (`* -text`) keeps every blob byte-identical to upstream's |
| `android-build`, `Cross.toml`, `shell.nix`, `publish_crates_io.py`, `PULL_REQUEST_TEMPLATE.md` | Upstream's release and CI tooling |
| GPL- and LGPL-licensed files (Dexter's #10 design read, then a sweep of the whole copy): `third_party/python/yamllint` (GPL-3.0), `third_party/python/pylru` (GPL-2.0+), `python/mozbuild/mozbuild/action/tooltool.py` (GPL-2.0), `js/src/util/make_unicode.py` and `js/src/devtools/rootAnalysis/run_complete` (GPL-3.0), `third_party/python/setuptools/setuptools/_vendor/autocommand` (LGPL-3.0), and ICU's unused autoconf files `intl/icu/source/config.sub`, `config.guess`, `aclocal.m4` (GPL with the autoconf exception) | CLAUDE.md: GPL/LGPL code is never copied. None is on the generator's path: mach's site files list `yamllint` and `pylru` as `.pth` entries, which Python skips when the directory is absent, and `tooltool` and `autocommand` serve commands we never run |
| 10 binaries: setuptools' Windows launchers (`*.exe`), `config/external/icu/data/icudt78l.dat` (ICU data; intl is off), `config/tests/test.manifest.jar`, and `python/mozperftest/mozperftest/tests/fetched_artifact.zip` | Executables or data we don't run or link |

**Kept, and flagged for the PM:** `build/autoconf/config.sub` and `config.guess` are GPL-3.0 **with the autoconf
exception**, which permits them in a configure system under any licence. configure runs them on the CI runners to
canonicalise the host triple, and patch 0002 passes ours straight through. They are never compiled or linked, and
nothing in the product reads them. If the exception isn't enough, the fallback is a small patch: mozbuild
canonicalises both triples in Python and drops the two scripts.
`js/src/vtune/` and `mozglue/baseprofiler/core/vtune/ittnotify.h` are dual BSD/GPLv2 (Intel's notice). They're taken
as BSD, and they aren't compiled without `--enable-vtune`.

**Also kept, and flagged:** three pure-Python wheels (`pip`, `setuptools`, `wheel`, all `py3-none-any`) under
`mozjs-sys/mozjs/third_party/python/_venv/wheels/`. mach uses them to make its virtualenv without the network. They
run only in the generator job, never in the product build. If the generator works without them, they go.

## Licences of what links (C3c-1)

All licences were read from the files in the clone. The THINK's §2.2 is the full table.

| Directory | Licence |
|---|---|
| `js/src`, `js/public`, `mfbt`, `mozglue/misc` | MPL-2.0 |
| `mfbt/double-conversion`, `js/src/irregexp` | BSD-3-Clause, the V8 authors (`LICENSE`, `LICENSE.v8`) |
| `modules/fdlibm` | BSD-2-Clause style, The FreeBSD Project (`modules/fdlibm/src/LICENSE`) |
| `third_party/fmt` | MIT (`third_party/fmt/LICENSE`) |
| zlib | headers only here (zlib licence); the code is the `libz-sys` copy L0 already vendors |

The generator-time trees (`python/`, `third_party/python/`, `build/`, `testing/`) carry MPL-2.0 and each Python
package's own licence. They are never linked.
