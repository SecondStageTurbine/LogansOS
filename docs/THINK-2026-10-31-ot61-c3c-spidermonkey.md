<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — OT-61 C3c: SpiderMonkey, interpreter-only and single-threaded first

**Seat:** D. **Date:** 2026-10-31. **Ladder:** `Plans/DISPATCH-2026-10-30-seat-d-browser.md` rung 3 (C3), the engine
half. ADR-078 §3, row C3: *"One realm evaluates JS, GC compacts, helper threads join, OOM/crash kills only the content
process."*
**Builds on:** C3a `neo-crt` (#2, musl top half + libc++). C3b (pthreads, private #263) is a dependency of the
*second* step here, not the first (§3).
**Status:** for the PM's read before any build. Nothing here is code.

## 1. What this rung has to answer

C3a gave the browser bundle a C and C++ runtime. C3c puts the JavaScript engine on it. Servo's `script` crate is
built against SpiderMonkey through `mozjs`, so the engine choice is really "SpiderMonkey via `mozjs`, or rewrite
Servo's bindings". §2 checks that against what else exists. The open questions are then:

1. how the engine is **built** (SpiderMonkey's own Python `configure` + `make`, or a frozen source list compiled by
   `cc`, as C2.3b and C3a were);
2. which **platform port** it gets (Linux/POSIX, or the WASI port that already exists upstream);
3. what it needs from `neo-crt` and the kernel that isn't there yet (§4, the trap bank);
4. how the rung splits so its first gate doesn't wait on C3b.

## 2. Prior-art survey (standing rule, 2026-10-27)

Checked 2026-10-31 by cloning each upstream's default branch (`git clone --depth 1 --filter=blob:none`, then
`git log -1`). Licences were read from each repository's licence file at that head.

| Candidate | What it is | Licence | Last commit (checked) | Fit | Take |
|---|---|---|---|---|---|
| **SpiderMonkey via `servo/mozjs`** | Servo's packaging of SpiderMonkey: `mozjs-sys` (the C++ engine, vendored, plus `bindgen` bindings) and `mozjs` (the safe Rust layer). Engine version **153.6.0** (`config/milestone.txt`). | MPL-2.0 (the workspace and the engine); bundled third-party parts listed in §2.1 | `6e69c16` 2026-10-29 | The engine Servo's `script` is written against, so it is the only choice that keeps Servo's DOM bindings. Already has a **no-JIT, no-shared-memory, no-threads WASI port** (§3). JIT is a cargo feature (`jit`), off unless asked for, which suits A1. | **Code**: the engine sources and `mozjs`; **design**: the WASI configuration |
| **QuickJS** (Bellard) | Small ES2023 engine in C, one file plus libregexp/libunicode | MIT | `2833510` 2026-10-31 | Would build on `neo-crt` in one rung. But Servo's bindings are SpiderMonkey's (rooting, compartments, JSAPI), so using it means rewriting `script`. | **Nothing** for the browser. A useful **fallback** if C3c fails (ADR-078 §5's "bounded WASI-derived path" names the same kind of retreat). |
| **quickjs-ng** | Community fork of QuickJS | MIT | `7b5f588` 2026-10-30 | Same as QuickJS, more active. | Nothing (same reason) |
| **rquickjs** | Rust bindings for QuickJS | MIT | `8836521` 2026-10-28 | Same as QuickJS. | Nothing |
| **Boa** | JS engine in Rust | MIT / Unlicense | `329e897` 2026-10-31 | Pure Rust, would sit on our `std` with no C++ at all. Not what Servo binds to. Conformance behind SpiderMonkey's. | Nothing for the browser; noted as the pure-Rust option if J.c. ever wants a scripting engine outside the browser |
| **rusty_v8** (V8) | Deno's Rust bindings for V8 | MIT (bindings); V8 is BSD-3-Clause | `55ca54e` 2026-10-21 | GN/Ninja build, a much larger C++ surface, and V8's jitless mode is less used. Not what Servo binds to. | Nothing |
| **Starlight** | JS engine in Rust | MPL-2.0 | `503e789` 2021-10-31 | Five years without a commit: abandoned by the standing rule. | Nothing |

**Licence result:** the code we'd take is MPL-2.0, the same as our tree. No GPL code was read.
**Freshness:** `mozjs` is alive (commits this week) and tracks Firefox ESR releases. **Seat D maintains our vendored
copy** and moves it with `mozjs` releases.

### 2.1 What ships inside `mozjs-sys/mozjs/` and has to be checked before vendoring

The vendored tree is 10,443 files. Most of it is build machinery we wouldn't ship: `third_party/python` (2,897
files), `python/mozbuild`, `testing/`, `build/clang-plugin`. With intl off, `intl/icu` and `intl/icu_capi` drop out
too. What's left that links into the engine includes third-party code under its own terms. Before vendoring, the
build PR lists each directory that links into the engine, with its licence file:
- `mfbt/double-conversion` (BSD-3-Clause, `LICENSE`), `modules/fdlibm` (Sun's permissive per-file notice), the
  irregexp port from V8 in `js/src/irregexp` (BSD-3-Clause, `LICENSE.v8`), and `mfbt`/`mozglue` (MPL-2.0). All three
  licence texts were read at `6e69c16`;
- `modules/zlib` (zlib licence), or `libz-rs` with the `libz-rs` feature. Not yet read; the build PR checks it.

The C3c build PR carries this list as its licence table. Any file without a compatible licence is cut from the
source list, not vendored.

### 2.2 The licence table at the vendoring pin (2026-11-08)

**Pin:** `servo/mozjs` @ `138c9470151b8dcbb335f249cb83f9404857baf9` (2026-11-01; `mozjs_sys` 153.6.0-2, engine
153.6.0, `config/milestone.txt`). It moved on from the `6e69c16` read above; the engine version is unchanged. Cloned
with `git clone --depth 1 --filter=blob:none`. Every licence below was read from the file named in the clone, not
through a summarizing fetch.

"Links in C3c-1" is what `moz.build`/`moz.configure` say for an interpreter-only, no-wasm, no-intl, no-jemalloc
standalone engine. The generator's source list (option (c)) is what confirms it; this table is the expectation it is
checked against.

| Directory | Files | Licence (file read) | Compatible? | Links in C3c-1? |
|---|---|---|---|---|
| `js/src` (engine), `js/public` | — | MPL-2.0 (Mozilla headers; top-level `LICENSE` points to `toolkit/content/license.html`) | yes | yes |
| `mfbt` | 230 | MPL-2.0 | yes | yes |
| `mfbt/double-conversion` | 27 | BSD-3-Clause, V8 authors (`mfbt/double-conversion/LICENSE`) | yes | yes |
| `js/src/irregexp` | 76 | BSD-3-Clause, V8 authors (`js/src/irregexp/LICENSE.v8`) | yes | yes (RegExp) |
| `modules/fdlibm` | 77 | BSD-2-Clause style, The FreeBSD Project (`modules/fdlibm/src/LICENSE`) | yes | yes (`Math`) |
| `mozglue/misc` | 90 | MPL-2.0 | yes | yes (timestamps, stack walking helpers) |
| `memory/mozjemalloc` | — | MPL-2.0 | yes | **no**: `js_standalone` without `--enable-jemalloc` (`js/moz.configure:1436`); GC memory goes through `neo-crt`'s `malloc`/`posix_memalign` (trap T3) |
| `memory/build`, `memory/mozalloc` | — | MPL-2.0 | yes | **yes**: the generator probe (configure, run 42508546125) emits unified files for both; they are the allocator wrappers over the system `malloc`, not jemalloc. The licence finding is unchanged. |
| `third_party/fmt` | — | MIT (`third_party/fmt/LICENSE`) | yes | yes: `mozglue/moz.build:32` builds it, and `mfbt/Casting.h`, `mfbt/BitSet.h`, `mozglue/misc/Debug.h` include it |
| `third_party/function2` | — | BSL-1.0 (`third_party/function2/LICENSE.txt`) | **check**: Boost is permissive, but not on CLAUDE.md's list | expected **no**: header-only, reached only through `mfbt/MoveOnlyFunction.h`, which nothing in `js/src` includes |
| `js/src/zydis` | 101 | MIT (`Zydis/LICENSE`, `Zycore/LICENSE`) | yes | **no**: only with `JS_JITSPEW` (`js/src/moz.build:351`) |
| `third_party/gemmology`, `third_party/xsimd` | — | MIT; BSD-3-Clause | yes | **no**: wasm intgemm only (`ENABLE_WASM_MOZ_INTGEMM`) |
| `modules/zlib/src` | 2 (`zlib.h`, `zconf.h`) | zlib licence (`zlib.h` header) | yes | headers only; the zlib code itself comes from outside (`libz-sys`, already vendored for L0, or `libz-rs`) |
| `nsprpub` | 536 | MPL-2.0 (`nsprpub/LICENSE`) | yes | **no**: standalone JS doesn't build NSPR unless `--enable-nspr-build` (`build/moz.configure/nspr.configure`) |
| `intl` (ICU and friends) | 1,883 | ICU/Unicode | yes | **no**: intl off |
| `python`, `third_party/python`, `build`, `testing` | — | MPL-2.0 and the Python packages' own | n/a | **no**: generator-time only, never in the product build |

**One to watch:** `function2` is under the **Boost Software License 1.0** (OSI-approved, permissive, no copyleft).
BSL-1.0 is not on CLAUDE.md's vendoring list (MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0,
Unlicense/CC0). Nothing in `js/src` includes `MoveOnlyFunction.h`, so it should stay out of the C3c-1 build and is
not vendored. If the generator's list ever includes it, that build stops and the question goes to J.c.: add BSL-1.0
to the list, or give `MoveOnlyFunction` a target-cfg replacement. Everything that links is MPL-2.0, BSD or MIT.

## 3. The design: SpiderMonkey's WASI port, re-targeted, in two steps

### 3.1 Why the WASI port is our shape

`mozjs-sys/makefile.cargo` already builds SpiderMonkey for `wasm32-wasip1` with:

```
--enable-portable-baseline-interp --disable-jit --disable-shared-memory --with-sysroot=<wasi-sysroot>
```

and the engine sources carry `__wasi__` branches for exactly the platform surfaces we'd otherwise have to port:

| Surface | Linux port | WASI port | Ours |
|---|---|---|---|
| GC chunk memory (`js/src/gc/Memory.cpp`) | `mmap`/`munmap`, alignment tricks, `madvise` | `posix_memalign` + `memset`, `free`; page protections are no-ops | **WASI.** GC memory comes from `neo-crt`'s `malloc`, so it lands under the C2.2 per-process memory quota (**A2 holds by construction**), and no page is ever made executable (**A1 untouched**). |
| Threads (`js/src/threading/`) | `posix/PosixThread.cpp`, `posix/CpuCount.cpp` | `noop/NoopThread.cpp`, `noop/CpuCount.cpp` (no helper threads) | **WASI first** (C3c-1), **POSIX later** (C3c-2, on C3b) |
| Native stack base (`js/src/util/NativeStack.cpp`) | `pthread_getattr_np` | the frame address at startup, plus a recursion counter (`wasiRecursionDepth`) | **Neither as-is** (trap T4) |
| Interpreter | C++ interpreter, Baseline/Ion JITs | the portable baseline interpreter (PBL), no native code | **PBL.** It generates no machine code, so it fits W^X. |

The C3a THINK's hard rule (single-threaded C++ until C3b) is exactly what the no-op threading backend gives. So the
first step can run with today's `neo-crt`.

### 3.2 The two steps

- **C3c-1 — one realm, one thread, interpreter only.** `mozjs` with the WASI-shaped configuration on
  `x86_64-unknown-neoplutarch`: no JIT, no shared memory, no intl, no-op threads, GC memory through `malloc`.
  **Gate (`spidermonkey_smoke`, QEMU, `NEO_SMP=4`):** one realm evaluates a fixed script and its result is compared
  byte for byte with a host run of the same `mozjs` version (the oracle); a forced compacting GC moves a known
  object (its address changes, its value doesn't); an allocation loop past the C2.2 quota ends that process only
  (the kernel and a sibling smoke stay up); `Date.now()` returns a value, not a crash (trap T1). A4: no
  syscall site and no `%fs` read that isn't std's.
- **C3c-2 — helper threads.** Switch `js/src/threading` to the POSIX backend on C3b's pthread subset; libc++
  rebuilt with threads (C3b). **Gate:** ADR-078's "helper threads join": off-thread parse or GC helper tasks run and
  join, and the content process exits cleanly with them running. This step is SMP-touching userland. Its dispatch
  consults `docs/concurrency-pitfalls.md`, and its AAG needs a cross-family lens.

Splitting it this way changes the dependency order from C3a → C3b → C3c to **C3a → C3c-1**, with **C3b → C3c-2**
running in parallel. The engine port and the pthread layer stop waiting on each other.

### 3.3 How the engine is built

J.c. allowed build-time Python on 2026-10-31 (CLAUDE.md, survey rule §4), on the condition that it fetches nothing
over the network. Three options:

| Option | What runs at build time | For | Against |
|---|---|---|---|
| **(a) `mozjs-sys`'s own `build.rs`** | Python `configure`, `make`, `bindgen` (needs libclang) on every build | Least porting; tracks upstream | `configure` doesn't know our target. It also tries a **pre-built archive download** first (`MOZJS_ARCHIVE` / `archive::download_archive`, trap T7). Every product build runs third-party build code. |
| **(b) frozen source list** | Only `cc` (as C2.3b and C3a) | No third-party build code at product build time; fully auditable | Someone must regenerate the list and `js-confdefs.h` on every `mozjs` bump, by hand |
| **(c) generate, then freeze (recommended)** | CI job: Python `configure` for our configuration, network off, emitting the source list, `js-confdefs.h` and the `bindgen` output. Product build: `cc` only, from the committed outputs. | (b)'s audit property, without hand-maintenance. The CI job re-runs the generator and **fails if its output differs from what's committed**, so drift can't hide. | One more CI job; `configure` must be taught our target (a small patch, §4 T6) |

**Recommendation: (c).** It is the C3a pattern (`cc` from Rust build scripts, nothing third-party running in the
product build) plus a generator that proves the frozen inputs are still what upstream's build would produce.

## 4. Trap bank (seeded before the first build)

Each trap names the discriminator that catches it on the C3c-1 gate.

| # | Trap | Where | Guard | Discriminator |
|---|---|---|---|---|
| T1 | `PRMJ_Now()` does `MOZ_ALWAYS_TRUE(clock_gettime(CLOCK_REALTIME, …) == 0)`. `neo-crt` returns `EINVAL` for `CLOCK_REALTIME` (C3a known gap 3), so the first `Date` call **crashes the engine**. | `js/src/vm/Time.cpp:30` | The kernel already answers `CLOCK_REALTIME` on `browser-runtime` kernels (`sys_clock_gettime_impl`, `kernel/src/arch/x86_64/syscall.rs`): monotonic plus the `rtc-epoch` base, or plus 0 (boot-relative) without it. Only std's overlay refuses it (`SystemTime` is std's `unsupported`). Route std's `SystemTime::now()` to that syscall (already in the A4 manifest, so no new syscall), then `neo-crt`'s `CLOCK_REALTIME` reads `SystemTime`. **Decision for the PM (§6).** | The smoke calls `Date.now()` and checks that the value is non-negative and non-decreasing. |
| T2 | `mozilla::RandomUint64` on Linux makes a **raw `syscall(SYS_getrandom, …)`**, a syscall site of its own that the A4 backstop must reject. The WASI/BSD branch uses `arc4random_buf`. | `mfbt/RandomNum.cpp:30,95,102` | Build with the `arc4random_buf` branch; `neo-crt` provides `arc4random_buf` over std's entropy (`SYS_GETRANDOM` via the sanctioned trampoline). | A4 backstop: the ELF has only std's syscall sites. Smoke: `Math.random()` gives different values across two boots. |
| T3 | GC memory alignment. `posix_memalign(SystemPageSize(), len)` must hand back page-aligned chunks; a `malloc` that only promises 16 bytes will trip `MOZ_ASSERT`s or misplace chunk headers. | `js/src/gc/Memory.cpp:245` | `neo-crt`'s `posix_memalign` honours large alignments (KAT on the host and in the smoke). | Smoke allocates past one GC chunk and forces a GC; a debug build runs with `--enable-debug`'s assertions on once. |
| T4 | **Stack limit.** Linux asks `pthread_getattr_np`, which C3a doesn't have. WASI uses the frame address at startup plus a recursion counter, which relies on `--stack-first` linking. Neither fits us. A wrong limit means runaway recursion **overruns the stack** instead of throwing `InternalError: too much recursion`. | `js/src/util/NativeStack.cpp` | A small `__neoplutarch__` branch: the stack base and size from std's thread record (C1 already tracks stack bounds and guard pages). | The smoke runs unbounded recursion and expects a catchable `InternalError`, not a guard-page fault. |
| T5 | **Signal handlers.** SpiderMonkey installs SIGSEGV handlers for wasm bounds checks and JIT traps. With JIT and wasm off they shouldn't be needed, but any leftover `sigaction` is a POSIX surface we don't have. | `js/src/wasm/WasmSignalHandlers.cpp` and friends | Configure wasm off (or its signal handling off); `neo-crt` provides no `sigaction`, so a leftover reference **fails the link**, not the run. | Link failure names the symbol. |
| T6 | **`configure` doesn't know `x86_64-unknown-neoplutarch`.** It parses the triple to set `OS_ARCH`/`OS_TARGET`, which picks the platform sources. | `build/moz.configure/init.configure` | A small patch adding our OS that selects the WASI-style source choices (threads, memory) and POSIX for the rest. Kept as a single patch file with its upstream base recorded. | The generator job's output diff (option c). |
| T7 | **Pre-built archive download.** `mozjs-sys`'s `build.rs` tries `archive::download_archive` from GitHub releases before building from source. That's a network fetch and an unaudited binary. | `mozjs-sys/build.rs:78-95` | Option (c) doesn't run this `build.rs`. If (a) is ever used, `MOZJS_FROM_SOURCE=1` and the job runs with the network off. | Generator job runs under `unshare -n` (no network); any fetch fails the job. |
| T8 | **The Python environment.** `mach`/`configure` may try to make a virtualenv and `pip install`. | `python/mozbuild` | Use only `third_party/python` (vendored); network off. | Same `unshare -n` run. |
| T9 | **Unified sources.** SpiderMonkey compiles `Unified_cpp_*.cpp` files that `#include` several `.cpp` files each. A frozen list must keep the same grouping, or a file-local name can collide or change meaning. | `moz.build` `UNIFIED_SOURCES` | The generator emits the unified files themselves, not the raw list. | Byte-diff of the generated unified files. |
| T10 | **`bindgen` output depends on the host's libclang.** | `mozjs-sys/build.rs` (bindgen) | Generate the bindings in the generator job with the pinned clang 19, commit them, and diff-check like the rest. | Generator diff. |
| T11 | **Exceptions and RTTI.** SpiderMonkey builds with `-fno-exceptions -fno-rtti`, which matches `neo-crt`'s libc++. | `build/moz.configure/flags.configure:101`, `toolchain.configure:3581` | Keep it so; a build that needs RTTI fails the link (no `typeinfo` symbols). | Link failure. |
| T12 | **Static constructors and `atexit`.** `neo-crt` runs `.init_array` but never runs `atexit` handlers (C3a known gap 2). SpiderMonkey needs `JS_ShutDown()` called explicitly. | `mozjs` `JSEngine` | The smoke calls shutdown explicitly. | Smoke ends with the engine shut down and exit code 0. |

The concurrency pitfall bank applies to **C3c-2**, not C3c-1: C3c-1 is one thread by construction (no-op threading
backend). C3c-2's dispatch carries the CP table.

## 5. Kill-switch

The whole rung is opt-in, like C3a: `mozjs` and its smoke are standalone crates outside the root workspace, built
only by the `spidermonkey_smoke` scenario and the CI generator/build jobs. The default kernel and userland builds are
bit-for-bit unchanged. JIT stays a cargo feature that **no** build enables; A1 stays reserved.

## 6. Decisions (PM, 2026-11-08, on #10)

1. **The wall clock (T1): option (i), contained.** `neo-crt`'s `CLOCK_REALTIME` returns its process-relative
   count, read from 1970. Time advances, and every date reads as 1970, never a wrong "real" one. std is untouched,
   so C1.3's stated std behaviour (`SystemTime::now()` unsupported) stands. The kernel side needs nothing:
   `SYS_CLOCK_GETTIME(CLOCK_REALTIME)` already answers on `browser-runtime` kernels (OT-85). Real dates come later,
   in one place, when the bundle can trust `rtc-epoch`. **#86** carries it (it replaces #21).
   *Not taken:* (ii), letting std's `SystemTime::now()` read the kernel's realtime, which would change C1.3.
2. **The build (§3.3): option (c), generate then freeze.**
   - A CI generator job runs `configure` for our configuration with the **network off**, and commits the frozen
     source list (the unified files themselves, T9), `js-confdefs.h` and the `bindgen` output (T10, pinned clang).
   - The product build runs `cc` only and fetches nothing (the vendoring rule).
   - The generator job **fails if its output differs from what's committed**, so the frozen files can't drift
     silently.
   - The generator, like every third-party build script, runs on the CI runners, never on a seat's machine
     (J.c., 2026-10-29).
3. **Order: C3c-1 goes ahead now, beside rung 4.** C3c-1 needs only C3a (landed); it touches none of rung 4's
   files.

## 7. Next steps

1. ~~The PM reads this THINK~~: decided (§6).
2. Vendor `mozjs` at the §2.2 pin (`servo/mozjs` @ `138c9470`) under `third_party/ot61-mozjs/`, with §2.2's
   licence table. Write the generator job (option (c)) and commit its first outputs from the CI run.
3. `neo-crt` additions: `arc4random_buf` and the page-aligned `posix_memalign` check (landed as #65), and the T1
   clock (#86).
4. `spidermonkey_smoke` and its gate (§3.2).
