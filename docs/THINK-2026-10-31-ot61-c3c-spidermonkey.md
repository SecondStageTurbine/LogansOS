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
| `js/src/dtoa.c` | 1 | David M. Gay's dtoa / Lucent permissive notice (the file's header), **not** MPL-2.0 | **yes**: J.c. added this notice to CLAUDE.md's list on 2026-11-08 (`da68b78f`) | yes: `#include`d by `util/DoubleToString.cpp:70`. Had the answer been no, the fallback was `mfbt/double-conversion` (BSD-3-Clause, already linked) in its place. |
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
| `third_party/gemmology`, `third_party/xsimd` | — | MIT; BSD-3-Clause | yes | **no**, by flag: `ENABLE_WASM_MOZ_INTGEMM` is on by default on x86_64, and the generator passes `--disable-wasm-moz-intgemm` (#87). Neither directory is vendored, so a build that wanted them would fail, not link them quietly. |
| `modules/zlib/src` | 2 (`zlib.h`, `zconf.h`) | zlib licence (`zlib.h` header) | yes | headers only; the zlib code itself comes from outside (`libz-sys`, already vendored for L0, or `libz-rs`) |
| `nsprpub` | 536 | MPL-2.0 (`nsprpub/LICENSE`) | yes | **no**: standalone JS doesn't build NSPR unless `--enable-nspr-build` (`build/moz.configure/nspr.configure`) |
| `intl` (ICU and friends) | 1,883 | ICU/Unicode | yes | **no**: intl off |
| `python`, `third_party/python`, `build`, `testing` | — | MPL-2.0 and the Python packages' own | n/a | **no**: generator-time only, never in the product build |
| GPL/LGPL files: `third_party/python/yamllint` (GPL-3.0), `third_party/python/pylru` (GPL-2.0+), `python/mozbuild/mozbuild/action/tooltool.py` (GPL-2.0), `js/src/util/make_unicode.py` and `js/src/devtools/rootAnalysis/run_complete` (GPL-3.0), setuptools' `_vendor/autocommand` (LGPL-3.0), ICU's `config.sub`/`config.guess`/`aclocal.m4`, and the PGO pages `build/pgo/js-input/3d-thingy.html` and `sunspider/date-format-xparb.html` (LGPL-2.1) | — | GPL / LGPL | **no** (never copied) | **not vendored** (#87's cut list). None is on the generator's path. |
| `build/autoconf/config.sub`, `config.guess` | 2 | GPL-3.0 **with the autoconf exception** | **flagged for the PM** | **no**: configure runs them on the CI runners to canonicalise the host triple; never compiled or linked. Fallback if the exception isn't enough: mozbuild canonicalises both triples in Python and the scripts go. |

**One to watch:** `function2` is under the **Boost Software License 1.0** (OSI-approved, permissive, no copyleft).
BSL-1.0 is not on CLAUDE.md's vendoring list (MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0,
Unlicense/CC0). Nothing in `js/src` includes `MoveOnlyFunction.h`, so it should stay out of the C3c-1 build and is
not vendored. If the generator's list ever includes it, that build stops and the question goes to J.c.: add BSL-1.0
to the list, or give `MoveOnlyFunction` a target-cfg replacement. Everything that links is MPL-2.0, BSD, MIT,
Unicode-3.0 or the dtoa notice.

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
| Threads (`js/src/threading/`) | `posix/PosixThread.cpp`, `posix/CpuCount.cpp` | `noop/NoopThread.cpp`, `noop/CpuCount.cpp` (no helper threads) | **POSIX**, on C3b's pthreads, in both steps. WASI's no-op backend makes `JS_NewContext` fail on x86_64. C3c-1 runs **without helper threads**: patch 0003 sets `gCanUseExtraThreads = false`, so the pool never starts (§4, "Single-threaded, enforced"). C3c-2 turns them on. |
| Native stack base (`js/src/util/NativeStack.cpp`) | `pthread_getattr_np` | the frame address at startup, plus a recursion counter (`wasiRecursionDepth`) | **WASI's cluster** (patch 0003): neo-crt has no `pthread_getattr_np`. The recursion bound must fit our thread's stack (trap T4) |
| Interpreter | C++ interpreter, Baseline/Ion JITs | the portable baseline interpreter (PBL), no native code | **PBL.** It generates no machine code, so it fits W^X. |

C3b has since landed, so neo-crt has pthreads, and C3c-1 builds with the POSIX threading backend. The one-thread
rule now comes from helper threads being off (`gCanUseExtraThreads = false`, patch 0003), not from a no-op backend.
C3c-1 needs the neo-crt in #88: libc++ with localization, the POSIX calls SpiderMonkey links, and the stack-protector
canary.

### 3.2 The two steps

- **C3c-1 — one realm, one thread, interpreter only.** `mozjs` with the WASI-shaped configuration on
  `x86_64-unknown-neoplutarch`: no JIT, no shared memory, no intl, POSIX threads with no helper threads
(`gCanUseExtraThreads = false`), GC memory through `posix_memalign`.
  **Gate (`spidermonkey_smoke`, QEMU, `NEO_SMP=4`):** one realm evaluates a fixed script and its result is compared
  byte for byte with a host run of the same `mozjs` version (the oracle); a forced compacting GC moves a known
  object (its address changes, its value doesn't); an allocation loop past the C2.2 quota ends that process only
  (the kernel and a sibling smoke stay up); `Date.now()` returns a value, not a crash (trap T1). A4: no
  syscall site and no `%fs` read that isn't std's.
- **C3c-2 — helper threads.** Turn `gCanUseExtraThreads` back on, so the helper pool starts on the POSIX backend
  C3c-1 already uses (C3b's pthreads, libc++ with threads). **Gate:** ADR-078's "helper threads join": off-thread parse or GC helper tasks run and
  join, and the content process exits cleanly with them running. This step is SMP-touching userland. Its dispatch
  consults `docs/concurrency-pitfalls.md`, and its AAG needs a cross-family lens.

The order is now **C3a → C3b → #88 → C3c-1 → C3c-2**. C3b landed first, so the split no longer works around a
missing pthread layer. What it separates now is turning helper threads on, which needs its own CP table and AAG.

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
| T1 | `PRMJ_Now()` does `MOZ_ALWAYS_TRUE(clock_gettime(CLOCK_REALTIME, …) == 0)`. `neo-crt` returned `EINVAL` for `CLOCK_REALTIME` (C3a known gap 3), so the first `Date` call would **crash the engine**. | `js/src/vm/Time.cpp:30` | **Option (i)** (§6.1, #86): `neo-crt`'s `CLOCK_REALTIME` answers with the process-relative count, read as seconds since 1970. `Date` works and advances, and every date reads as early 1970, never as a wrong "real" date. std is untouched. | The smoke reads `Date.now()` twice. Both values must be at least 0 and below 86,400,000 (one day after the epoch), and the second must be at least the first. A real date (about 1.79e12 ms in 2026) fails the bound, so the check tells "counts from 1970" apart from a real clock. It also stops passing, as it should, the day real dates arrive. |
| T2 | `mozilla::RandomUint64` on Linux makes a **raw `syscall(SYS_getrandom, …)`**, a syscall site of its own that the A4 backstop must reject. The WASI/BSD branch uses `arc4random_buf`. | `mfbt/RandomNum.cpp:30,95,102` | Build with the `arc4random_buf` branch; `neo-crt` provides `arc4random_buf` over std's entropy (`SYS_GETRANDOM` via the sanctioned trampoline). | A4 backstop: the ELF has only std's syscall sites. Smoke: `Math.random()` gives different values across two boots. |
| T3 | GC memory alignment. `posix_memalign(SystemPageSize(), len)` must hand back page-aligned chunks; a `malloc` that only promises 16 bytes will trip `MOZ_ASSERT`s or misplace chunk headers. | `js/src/gc/Memory.cpp:245` | `neo-crt`'s `posix_memalign` honours large alignments (KAT on the host and in the smoke). | Smoke allocates past one GC chunk and forces a GC; a debug build runs with `--enable-debug`'s assertions on once. |
| T4 | **Stack limit.** Linux asks `pthread_getattr_np`, which `neo-crt` doesn't have, and WASI uses the frame address at startup plus a recursion counter. A wrong limit means runaway recursion **overruns the stack** instead of throwing `InternalError: too much recursion`. | `js/src/util/NativeStack.cpp`, `js/public/friend/StackLimits.h`, `js/public/RootingAPI.h` | Patch 0003 (#87) takes WASI's cluster: no native-stack limit, with depth bounded by `wasiRecursionDepth` checks. The bound must fit our thread's stack, which the smoke measures. The `RootingContext` field it adds is in the bindings too (trap B1, #89). | The smoke runs unbounded recursion and expects a catchable `InternalError`, not a guard-page fault. It also reports the depth reached. |
| T5 | **Signal handlers.** SpiderMonkey installs SIGSEGV handlers for wasm bounds checks and JIT traps. With JIT and wasm off they shouldn't be needed, but any leftover `sigaction` is a POSIX surface we don't have. | `js/src/wasm/WasmSignalHandlers.cpp` and friends | Configure wasm off (or its signal handling off); `neo-crt` provides no `sigaction`, so a leftover reference **fails the link**, not the run. | Link failure names the symbol. |
| T6 | **`configure` doesn't know `x86_64-unknown-neoplutarch`.** It parses the triple to set `OS_ARCH`/`OS_TARGET`, which picks the platform sources. | `build/moz.configure/init.configure` | Patch 0002 (#87) adds our OS. Patch 0003 takes WASI's branch only where POSIX's needs what neo-crt lacks (memory, the stack limit, signals, `getpid`, `tzset`), and POSIX elsewhere, threads included. Both are patch files applied to a copy, with the upstream base recorded. | The generator job's output diff (option c). |
| T7 | **Pre-built archive download.** `mozjs-sys`'s `build.rs` tries `archive::download_archive` from GitHub releases before building from source. That's a network fetch and an unaudited binary. | `mozjs-sys/build.rs:78-95` | Option (c) doesn't run this `build.rs`. If (a) is ever used, `MOZJS_FROM_SOURCE=1` and the job runs with the network off. | Generator job runs under `unshare -n` (no network); any fetch fails the job. |
| T8 | **The Python environment.** `mach`/`configure` may try to make a virtualenv and `pip install`. | `python/mozbuild` | Use only `third_party/python` (vendored); network off. | Same `unshare -n` run. |
| T9 | **Unified sources.** SpiderMonkey compiles `Unified_cpp_*.cpp` files that `#include` several `.cpp` files each. A frozen list must keep the same grouping, or a file-local name can collide or change meaning. | `moz.build` `UNIFIED_SOURCES` | The generator emits the unified files themselves, not the raw list. | Byte-diff of the generated unified files. |
| T10 | **`bindgen` output depends on the host's libclang.** | `mozjs-sys/build.rs` (bindgen) | The generator runs upstream's own `build_bindings` with libclang from the runner's `clang`, the same install whose headers it compiles with, commits the output, and diff-checks it like the rest (#87). | Generator diff. |
| T11 | **Exceptions and RTTI.** SpiderMonkey builds with `-fno-exceptions -fno-rtti`, which matches `neo-crt`'s libc++. | `build/moz.configure/flags.configure:101`, `toolchain.configure:3581` | Keep it so; a build that needs RTTI fails the link (no `typeinfo` symbols). | Link failure. |
| T12 | **Static constructors and `atexit`.** `neo-crt` runs `.init_array` but never runs `atexit` handlers (C3a known gap 2). SpiderMonkey needs `JS_ShutDown()` called explicitly. | `mozjs` `JSEngine` | The smoke calls shutdown explicitly. | Smoke ends with the engine shut down and exit code 0. |

**Single-threaded, enforced.** WASI's no-op threading backend is *not* used: with it, `JS_NewContext` fails on
x86_64 (`Runtime.cpp`, `InternalThreadPool.cpp`). Our target keeps the POSIX backend (real `pthread_create` through
neo-crt) and turns helper threads off at their one entry point:
- patch 0003 sets `gCanUseExtraThreads = false` for `__neoplutarch__` (`vm/Runtime.cpp:63`);
- `JSRuntime::init` initialises the helper thread pool only `if (CanUseExtraThreads())` (`vm/Runtime.cpp:176`), so
  `InternalThreadPool::Initialize` never runs;
- everything that would use a helper thread (off-thread parse and compile, parallel and background GC, wasm tiering)
  asks `CanUseExtraThreads()` or the empty pool and runs on the calling thread.

**CP table (C3c-1), for the threads that remain:**

| Class | C3c-1 | Guard / discriminator |
|---|---|---|
| Helper-thread pool | Not started | Smoke: neo-crt's `pthread_create` count is 0 after runtime creation, eval and a forced GC |
| Locks and condvars (`Mutex`, `ConditionVariable`, `LightLock`) | Compiled real (POSIX on neo-crt, C3b), taken by one thread only | Uncontended. C3b's items 20–29 cover the primitives |
| Atomics (`mozilla::Atomic`, `<atomic>`) | Real, one thread | — |
| `MOZ_THREAD_LOCAL` | pthread keys (patch 0002 reports no `__thread`) | C3b items 23 and 29 (keys per thread, destructors) |
| JS on more than one thread | Not allowed: one runtime, one thread | The smoke creates exactly one context |

C3c-2 (helper threads) turns `gCanUseExtraThreads` back on behind its own CP table and a cross-family AAG lens on the
helper-thread seams.

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

**The gate for each step:**

| Step | PR | Gate |
|---|---|---|
| Vendor tree, generator, frozen outputs | #87 | The generator workflow green, with the drift check, the cbindgen check and the bindgen step; push CI; PM review. Docs and tooling only, so no QEMU gate |
| neo-crt for SpiderMonkey | #88 | Close-gate; quick tier with `cxx_runtime_smoke` (items 30 and 31) and `swgl_smoke`; the full four shards before merge |
| The `cc`-only product build | after #89 | Host build of the smoke (`xtask ot61-closure-build --mozjs`); push CI; the generator's drift check unchanged |
| C3c-1 smoke on target | after #89 | Quick tier with `mozjs_smoke` (T1, T3, T4, B1 and the thread count above); the full four shards before merge |
| C3c-2 (helper threads) | later | Its own THINK and CP table, a cross-family AAG lens, and the full four shards |
