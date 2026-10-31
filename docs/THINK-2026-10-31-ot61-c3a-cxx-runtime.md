<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — OT-61 C3a: the browser bundle's private C/C++ runtime (`neo-crt`)

**Seat:** D. **Date:** 2026-10-31. **Ladder:** `Plans/DISPATCH-2026-10-30-seat-d-browser.md` rung 3 (C3), first half;
ADR-078 §2.1 ("a private static C++/libc++ subset … behind the browser bundle boundary").
**Branch:** `claude/ot61-c3a-crt`, stacked on `claude/ot61-c2.3b-closure` (#245).

## 1. Why this rung comes first

Rung 3 (SpiderMonkey) and rung 4 (SWGL) both compile C++ that expects a C library and a C++ standard library.
Neither exists for `x86_64-unknown-neoplutarch`. The SWGL spike FINDINGS (§2.1) fold SWGL's whole demand into one
row: "C++20, `malloc/realloc/free`, `new/delete`, string/memory, assert, and libm subset", plus a monotonic clock.
SpiderMonkey needs the same and more (threads, its own pthread port). So the runtime is the shared base, and it is
built and gated on its own before either consumer.

It is also the part of rung 3 that doesn't wait on J.c.'s Python question (#249): SpiderMonkey's build
(`mozjs-sys`) runs Python and `make`, but this runtime is compiled by `cc` from Rust build scripts, as C2.3b's
closure was. Nothing here runs third-party build code: the third-party parts are plain C and C++ sources.

## 2. Prior-art survey (standing rule, 2026-10-27)

Liveness checked 2026-10-31 by cloning each upstream at its default branch (`git log -1`). Two hosts
(git.musl-libc.org, sourceware.org) are blocked from this container. For musl I used two independent GitHub mirrors,
which agree on the `v1.2.5` tag. newlib was not reached; it is listed from known licence terms only.

| Candidate | What it is | Licence | Last commit (checked) | Fit | Take |
|---|---|---|---|---|---|
| **musl** 1.2.5 | Small, correct POSIX libc; Linux syscalls confined to a "bottom half" | MIT | master `7813e911` 2026-10-14; tag `v1.2.5` = `0784374d` (2024-02-29) | Its portable top half (string, ctype, math, stdlib, stdio formatting, multibyte) uses no syscalls. libc++ supports it directly (`_LIBCPP_HAS_MUSL_LIBC`). | **Code**: the top half, pristine |
| **wasi-libc** | musl's top half over WASI's capability-based bottom half | Apache-2.0 WITH LLVM-exception / Apache-2.0 / MIT (+ musl MIT) | `d9ad2d11` 2026-10-30 | Closest in *shape*: no ambient authority, capability-handed resources. But wasm32-specific (`bits/`, 32-bit `long`), so its copy of musl is the wrong one for us. | **Design**: the top/bottom split is theirs |
| **picolibc** | Embedded libc (newlib-derived) with tinystdio | BSD-3-Clause and similar | `b5d60504` 2026-10-30 | Good for bare-metal, and libc++ supports it, but it's built with meson (Python) and has no pthread story. | Nothing |
| **LLVM libc** | Per-entrypoint libc; overlay or full build | Apache-2.0 WITH LLVM-exception | llvm-project `29474e1a` 2026-10-30 | The per-entrypoint list would map neatly onto the A4 manifest. But no x86_64 bare-metal config upstream, a CMake + Python header generator, and less mature as a full libc. | Nothing yet; worth revisiting |
| **relibc** | Redox's libc, in Rust | MIT | `650cbecb` 2026-09-27 | A full POSIX personality: the "compat island" ADR-078 §2.1 refutes. | Nothing |
| **newlib** | Embedded libc | Mixed BSD-style; parts carry other terms | not reached (sourceware blocked) | Same build and licence-review cost as picolibc, no gain. | Nothing |
| **libc++** 18.1.8 | LLVM's C++ standard library | Apache-2.0 WITH LLVM-exception | tag `llvmorg-18.1.8` = `3b5b5c1e` (2024-06-15); upstream main `29474e1a` 2026-10-30 | Supports musl, no-exceptions, no-RTTI, no-threads, no-localization builds; hardening modes. Release 18 matches the toolchain's clang 18 (libc++ supports only recent clangs). | **Code**: headers + 30 sources, pristine |
| **libc++abi** / libcxxrt | The C++ ABI library (exceptions, RTTI, guards) | Apache-2.0 WITH LLVM-exception / BSD-2 | (llvm-project) | With no exceptions and no RTTI, the only ABI pieces left are static guards, the virtual-call traps and `__dso_handle`. | Nothing: those few are written in Rust |

**Licence result:** MIT and Apache-2.0 WITH LLVM-exception, both on the standing list. No GPL code was read.
**Freshness:** musl is alive (commits this month), but its last *release* is from 2024-02. LLVM 18 is a closed release
branch, which makes it abandoned by the standing rule's letter. **Seat D maintains our copies**, and the libc++ pin
moves with the toolchain's clang (§7, decision 1).

## 3. Design

```
  C/C++ code (SWGL, SpiderMonkey, the smoke)
      │  libc++ 18.1.8 headers + 30 sources            (third_party/ot61-crt/vendor/libcxx, pristine)
      │  musl 1.2.5 top half: string ctype math stdlib  (third_party/ot61-crt/vendor/musl, pristine)
      │                       multibyte stdio-format …
      ▼
  neo-crt bottom half ── C: c/neo_bottom.c (FILE write hooks, exit; MIT, musl-derived)
                      └─ Rust: src/lib.rs (malloc family, errno, clock_gettime, abort,
                                           __cxa_guard_*, .init_array walk)
      ▼
  the neoplutarch std (overlay) ── its existing PAL syscalls, nothing new
```

- **No raw syscalls.** Everything below musl is Rust on our std, so whatever std may do, the C runtime may do, and
  nothing more. The A4 backstop proves it on every build: the linked ELFs have exactly std's two syscall sites and
  its one `%fs` read (`verify_std_real_syscall_sites`, `verify_std_real_tcb_reads`). The PAL manifest is unchanged.
- **Pristine vendoring.** Whole upstream directories are copied unedited. Which files are compiled is an explicit
  list in `userland/neo-crt/build.rs`, with each exclusion explained. Nothing that calls a Linux syscall is on it.
- **One musl header is replaced.** musl's `src/internal/pthread_impl.h` reads the thread pointer from `%fs`. The
  compiled files need it only for the current locale, so `c/override/pthread_impl.h` gives every thread one
  read-only record (the global C locale). Two guards back this up:
  - `locale_impl.h` is copied verbatim beside the override, because a quoted include finds its sibling first. That
    was a real escape, caught while building: files reaching it through `locale_impl.h` got the real header.
  - An override `pthread_arch.h` is a hard `#error`, so any future route to the real header fails the build.
<<<<<<< ours
- **stdout and stderr are always locked (CP-100).** neo-crt's `c/stdout.c`/`c/stderr.c` are musl's with
  `lock = 0`, and `__lockfile`/`__unlockfile` are Rust, so two threads in `printf` can't share `__fwritex`'s buffer
  unlocked. The on-stack FILEs of `vsnprintf`/`vsscanf` keep `lock = -1` (one thread each). There is no `fopen`.
  musl's futex `__wake` is reached only by its own lock and pthread code, neither compiled, so it aborts with a
  message.
- **`aligned_alloc` (Chad #219).** An alignment that isn't a power of two, 0 included, fails with NULL and
  `EINVAL` (C17/C23: an unsupported alignment fails). A size that isn't a multiple of the alignment is accepted:
  C17 (DR 460) dropped C11's requirement, glibc and musl accept it, and ported C++ relies on it. `memalign` keeps
  the legacy behaviour (an alignment below the minimum is raised to it). Smoke items 58-60.
=======
- **No locked FILEs.** Every FILE neo-crt can make has `lock = -1`: stdout, stderr, and the on-stack FILEs of
  `vsnprintf`/`vsscanf`. There is no `fopen`. musl's lock paths (`__lockfile`, `__wake`) therefore abort with a
  message instead of pretending to lock.
>>>>>>> theirs
- **libc++ configuration** (`userland/neo-crt/cxx/__config_site`, hand-written from libc++'s template):
  - musl as the C library;
  - no exceptions, no RTTI;
  - `_LIBCPP_HAS_NO_THREADS` for now (§6);
  - no filesystem, no `random_device`, no localization or iostreams, no tz database;
  - **hardening `fast` by default**. This is browser code, and a bounds violation becomes `ud2`, which OT-83 contains
    to the one process. The smoke's twin proves the trap.
- **Static constructors.** `neo_crt::init()` walks `.init_array` (lld's `__init_array_start/end`). The Rust `_start`
  must call it first, because nothing else runs C++ constructors. The smoke checks that a global constructor ran.
- **The C++ ABI pieces, in Rust:**
  - `__cxa_guard_acquire/release/abort` (Itanium ABI §3.3.2) use one process-wide std `Mutex` + `Condvar`. Byte 0 is
    "built" (a release store), byte 1 is "being built". The mutex is never held while a constructor runs.
  - `__cxa_pure_virtual` and `__cxa_deleted_virtual` abort with a message.
  - `__cxa_atexit` accepts registrations and never runs them (§6). `__dso_handle` lives in C.
- **The smoke** (`userland/cxx-runtime-smoke`, scenario `cxx_runtime_smoke`) runs 15 checks:
  - **C**, each against an independent oracle: printf conversions written out by hand from C11 7.21.6.1; `strtod`
    bit patterns from CPython's correctly rounded parser; libm exact where IEEE 754 requires it and within one ulp
    of glibc elsewhere; the POSIX `rand48` recurrence computed in the test; allocator sizes, alignment, zeroing and
    overflow; the monotonic clock, with `CLOCK_REALTIME` refused.
  - **C++:** a global constructor ran; containers; `to_chars` shortest round-trip strings; smart pointers;
    `function`, `optional`, `variant`, virtual dispatch; `new(nothrow)` failure; `steady_clock`.
  - **Threads:** six at once for `errno` (must be per thread), malloc (contents intact), and one function-local
    static (built exactly once while all six wait).
  - **The twin** `cxx-hardening-trap` reads `std::vector<int>(4)[4]`. The kernel must see `#UD` and kill only it
    (exit 132), and every frame of both processes must return.

## 4. Kill-switch and the default build

The whole rung is opt-in. `neo-crt` and the smoke are standalone crates (outside the root workspace), built only for
the `cxx_runtime_smoke` scenario and by `cargo xtask ot61-crt-build`. The kernel change is one test-only feature
(`neo-test-cxx-runtime-smoke`) and its harness block, so the default kernel and image are unchanged. Nothing new is
cross-cutting: no syscall, no kernel invariant.

## 5. Concurrency (CP bank consult)

| Class | This change | Guard / discriminator |
|---|---|---|
| Lost wake / double init (static guards) | **introduces** a wait on a condvar for `__cxa_guard_acquire` | Built-once under 6 racing threads (`ok(15)`); falsified by a guard that always says "build" (§8) |
| Spin vs park | `__lock`/`__unlock` (musl internals: `random()` state, not on any hot path) **spin with `yield_now`** | Short sections, rare contention. Recorded here; replace with a futex word if a profile ever shows it |
| Per-thread state | `errno` is a std `thread_local!` (C1.2 keys), pointer stable for the thread's life | 6 threads interleave `errno` writes (`ok(8)`) |
| Allocator under SMP | malloc family on std's global allocator (the PAL heap) | 6 threads, 4000 alloc/free steps each, contents checked (`ok(9)`) |
<<<<<<< ours
| Stdio streams under SMP (CP-100) | **guards**: stdout/stderr locked always (`c/stdout.c`, `c/stderr.c`, `.lock = 0`), `__lockfile` on a std `Mutex`/`Condvar` | A second thread's `fputs` waits while the first holds the lock (`ok(16)`); 6 threads print 600-byte runs at once (`ok(17)`); control: musl's `lock = -1` stream, `FAIL(16)` |
=======
>>>>>>> theirs
| Recursive static init | **left open**: a constructor that needs its own static waits forever (libc++abi aborts instead) | Not reachable in SWGL; noted for the SpiderMonkey rung |

No kernel concurrency code changes.

## 6. Known gaps (each with the rung that closes it)

<<<<<<< ours
1. **No C++ threads, no pthreads. Hard rule: no multi-threaded C++ on this build until C3b's pthread rebuild.**
   libc++ here is `_LIBCPP_HAS_NO_THREADS`: `shared_ptr`'s reference counts aren't atomic, `call_once` isn't
   synchronised, and `std::thread`/`mutex`/`condition_variable` are absent. So C++ code may run on one thread
   only: a C++ object shared between std threads, or C++ entered from two threads at once, is undefined
   behaviour here, even when it compiles. (C is different: the C library's own state is per thread or locked,
   `errno` and the stdio streams included, CP-100.) SpiderMonkey threads through its own POSIX port
   (`js/src/threading/posix`). C3b adds a pthread subset on the C1 primitives std already uses (one
   implementation for std and C++, per ADR-078 A3), then rebuilds libc++ with `_LIBCPP_HAS_THREAD_API_PTHREAD`.
   That rebuild is an ABI change inside the bundle only, and it lifts this rule.
=======
1. **No C++ threads, no pthreads.** `std::thread`/`mutex`/`condition_variable` are absent from this libc++ build.
   SpiderMonkey threads through its own POSIX port (`js/src/threading/posix`). C3 adds a pthread subset on the C1
   primitives std already uses (one implementation for std and C++, per ADR-078 A3), then rebuilds libc++ with
   `_LIBCPP_HAS_THREAD_API_PTHREAD`. That rebuild is an ABI change inside the bundle only.
>>>>>>> theirs
2. **`atexit`/`__cxa_atexit` handlers never run.** Bundle processes end by `exit` or by being killed; SWGL and
   SpiderMonkey shut down explicitly.
3. **No wall clock.** `clock_gettime(CLOCK_REALTIME)` is `EINVAL`, so `std::chrono::system_clock::now()` aborts (a
   `system_error` with exceptions off). This is the same gap as std's `SystemTime`, and it closes with `rtc-epoch`.
4. **`errno` after a failed allocation, as clang sees it.** LLVM models `malloc`/`calloc` as writing no other memory,
   and musl's `__errno_location` is `const`. So a plain read of `errno` right after a failed allocation can fold to
   its old value (this bit the smoke's own check, now a volatile read). The runtime does set it. This is a compiler
   modelling fact for consumers to know about, not a runtime bug.
<<<<<<< ours
5. **The toolchain.** The gate's compiler is pinned in the dev image (`Dockerfile`): Debian trixie's `clang-19`, held
   to major 19 (the image build fails on another), exported as `NEO_CRT_CLANG`. The QEMU gate builds in that image.
   The `crt-build` job runs on the hosted runner and installs `clang-19` itself (Ubuntu noble's package); it checks
   that the Dockerfile pins the same major and fails if apt resolves another. Neither depends on whatever clang a
   runner happens to have (the #261 bounce). Third-party headers
   (musl, libc++) reach the smoke through `-isystem`, so a newer clang's warnings in them can't fail its
   `-Werror`. Checked locally with clang 18.1.3 and 19.1.1.

## 7. Decisions (none blocks this rung)

1. **Compiler pin (done, #261 bounce).** The dev image's packaged clang 19 (§6 item 5). libc++ stays 18.1.8: clang
   supports the previous libc++ release, and moving libc++ to 19 is its own change if ever needed.
2. **Runners need no clang.** `crt-build` installs the pinned major in the job (the public repo has no self-hosted
   QEMU-shard runners, so the job no longer runs in the dev image there).
=======
5. **The toolchain.** The C/C++ compiler is the runner's `clang` (`NEO_CRT_CLANG` overrides it). This container has
   clang 18.1.3, which matches libc++ 18. CI runners need clang ≥ 18 (§7).

## 7. Decisions (none blocks this rung)

1. **Compiler pin.** Use the system clang 18 (as now) and pin libc++ to the matching release, or fetch a pinned LLVM
   release toolchain in CI and move libc++ with it. I recommend the pinned release toolchain before SWGL, so the
   C++ that reaches the browser doesn't depend on whatever clang a runner has.
2. **Runners need `clang`.** The closure-build workflow now also runs `cargo xtask ot61-crt-build`. If a runner lacks
   clang it fails with a clear message. Provisioning is the PL's.
>>>>>>> theirs

## 8. Evidence

- `cxx_runtime_smoke` passes in QEMU (`NEO_SMP=4`): all 15 checks, the twin exits 132, and every frame returns.
- Falsification, each run with one thing broken (all in QEMU):
  - libc++ hardening `NONE` → the twin reads and exits 0, and the scenario fails `hardening_did_not_trap`;
  - `neo_crt::init()` made a no-op → check 10 fails (the global constructor didn't run);
  - `__cxa_guard_acquire` always returning 1 → check 15 fails (the static is built six times).

  The first falsification round found two defects in the discriminators themselves, both fixed:
  - Check 10's global was constant-initialised by clang, so it never went through `.init_array` and passed with no
    constructor run. It now reads a volatile.
  - The smoke's C++ object didn't rebuild when neo-crt's `__config_site` changed. Hardening checks inline into the
    consumer, so a stale object kept the old mode. The smoke's build script now reruns on that file.
- A4: two syscall sites and one `%fs` read in each ELF, all std's.
