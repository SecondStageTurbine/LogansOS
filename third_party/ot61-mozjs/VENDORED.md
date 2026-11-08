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
- `generator/neo-cc` (and `neo-c++`, a symlink) is the compiler the generator gives configure for our target:
  neo-crt's own compile line (clang for `x86_64-unknown-none-elf` over musl's and libc++'s headers) with
  `__neoplutarch__` predefined. configure's test programs link against a stub that defines exactly the symbols in
  `generator/neo-crt-symbols.txt`: neo-crt's, plus Rust's `compiler_builtins`. So a function check passes only
  for a function a LogansOS binary really has.
- Nothing builds from this tree yet. The CI generator job (option (c), next) runs `configure` here with the network
  off, on the CI runners only (J.c., 2026-10-29), and commits its frozen outputs. The product build then runs `cc`
  over those outputs. No other build reads this tree, and the default kernel, image and userland are unchanged.

## Left out of the copy

| Path | Why |
|---|---|
| `mozjs-sys/mozjs/third_party/function2/` | BSL-1.0, not on the vendoring list; reached only through `MoveOnlyFunction.h` (patch 0001) |
| `mozjs-sys/mozjs/js/src/zydis/` | The x86 disassembler, built only with `JS_JITSPEW` (`js/src/moz.build:351`); C3c-1 has no JIT |
| `mozjs-sys/mozjs/third_party/gemmology/`, `third_party/xsimd/` | wasm intgemm only (`ENABLE_WASM_MOZ_INTGEMM`); wasm is off |
| `mozjs-sys/mozjs/nsprpub/` | Standalone JS doesn't build NSPR unless `--enable-nspr-build` |
| `.gitattributes` (the repository's own) | Git metadata. It sets `text=auto eol=lf`, but upstream stores four files with CRLF in its own index. Our `vendor/.gitattributes` (`* -text`) keeps every blob byte-identical to upstream's |
| `mozjs-extracted-crates/`, `android-build`, `Cross.toml`, `shell.nix`, `publish_crates_io.py`, `PULL_REQUEST_TEMPLATE.md` | Upstream's release and CI tooling |
| 10 binaries: setuptools' Windows launchers (`*.exe`), `config/external/icu/data/icudt78l.dat` (ICU data; intl is off), `config/tests/test.manifest.jar`, and `python/mozperftest/mozperftest/tests/fetched_artifact.zip` | Executables or data we don't run or link |

**Kept, and flagged:** three pure-Python wheels (`pip`, `setuptools`, `wheel`, all `py3-none-any`) under
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
