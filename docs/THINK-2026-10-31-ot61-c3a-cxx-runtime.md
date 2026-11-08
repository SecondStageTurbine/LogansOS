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
- **stdout and stderr are always locked (CP-106).** neo-crt's `c/stdout.c`/`c/stderr.c` are musl's with
  `lock = 0`, and `__lockfile`/`__unlockfile` are Rust, so two threads in `printf` can't share `__fwritex`'s buffer
  unlocked. The on-stack FILEs of `vsnprintf`/`vsscanf` keep `lock = -1` (one thread each). There is no `fopen`.
  musl's futex `__wake` is reached only by its own lock and pthread code, neither compiled, so it aborts with a
  message.
- **`aligned_alloc` (Chad #219).** An alignment that isn't a power of two, 0 included, fails with NULL and
  `EINVAL` (C17/C23: an unsupported alignment fails). A size that isn't a multiple of the alignment is accepted:
  C17 (DR 460) dropped C11's requirement, glibc and musl accept it, and ported C++ relies on it. `memalign` keeps
  the legacy behaviour (an alignment below the minimum is raised to it). Smoke items 58-60.
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
| Stdio streams under SMP (CP-106) | **guards**: stdout/stderr locked always (`c/stdout.c`, `c/stderr.c`, `.lock = 0`), `__lockfile` on a std `Mutex`/`Condvar` | A second thread's `fputs` waits while the first holds the lock (`ok(16)`); 6 threads print 600-byte runs at once (`ok(17)`); control: musl's `lock = -1` stream, `FAIL(16)` |
| Recursive static init | **left open**: a constructor that needs its own static waits forever (libc++abi aborts instead) | Not reachable in SWGL; noted for the SpiderMonkey rung |

No kernel concurrency code changes.

## 6. Known gaps (each with the rung that closes it)

1. **No C++ threads, no pthreads** in C3a. **Closed by C3b (§9).**
2. **`atexit`/`__cxa_atexit` handlers never run.** Bundle processes end by `exit` or by being killed; SWGL and
   SpiderMonkey shut down explicitly.
3. **No wall clock.** `clock_gettime(CLOCK_REALTIME)` is `EINVAL`, so `std::chrono::system_clock::now()` aborts (a
   `system_error` with exceptions off). This is the same gap as std's `SystemTime`, and it closes with `rtc-epoch`.
4. **`errno` after a failed allocation, as clang sees it.** LLVM models `malloc`/`calloc` as writing no other memory,
   and musl's `__errno_location` is `const`. So a plain read of `errno` right after a failed allocation can fold to
   its old value (this bit the smoke's own check, now a volatile read). The runtime does set it. This is a compiler
   modelling fact for consumers to know about, not a runtime bug.
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

## 9. C3b: POSIX threads, and libc++ with threads

**Branch:** `claude/ot61-c3b-pthreads`, stacked on C3a.

**What it adds.** `userland/neo-crt/src/pthread.rs` provides the pthread subset that SpiderMonkey's POSIX
threading layer (`js/src/threading/posix`) and libc++'s pthread thread API call:
- create/join/detach/self/equal and the stack-size and detach-state attributes;
- mutexes: normal, recursive and error-checking;
- condition variables, with `pthread_condattr_setclock(CLOCK_MONOTONIC)`;
- keys with destructors at thread exit, once, rwlocks, `sched_yield`, `nanosleep`/`usleep`, and `sysconf` for the
  CPU count and page size.

libc++ is rebuilt with `_LIBCPP_HAS_THREAD_API_PTHREAD`, plus its atomic, barrier, condition_variable, future,
mutex, shared_mutex and thread sources.

**Still one threading implementation (ADR-078 A3).**
- Threads are `std::thread`.
- Every block is a std `Mutex`/`Condvar`, so the C1.1 futex underneath.
- Keys sit on a std `thread_local!`.

No syscall is added, and the A4 backstop still shows exactly std's two syscall sites and one `%fs` read.

**How state lives in C memory.** musl's `pthread_mutex_t`/`pthread_cond_t`/`pthread_rwlock_t` are caller-owned and
start all-zero. The first pointer of each holds a boxed state, built on first use and installed by
compare-and-swap, so two first users agree on one box. `*_destroy` frees it.

**Condition-variable wakeups aren't lost** (`neo-crt/src/seqcond.rs`). A sequence number lives under the condvar's
own mutex, and two rules hold it together:
- A waiter reads the number *before* releasing the user's mutex, and a signaller must take the same mutex to bump
  it. So a signal sent after the release can't land before the wait.
- A signaller **wakes while still holding that mutex**. A thread can only record a number, and so only start waiting,
  while it holds the mutex, so every waiter the wake can reach recorded the old number and returns to its caller.
  The first version bumped, released, then woke. A thread arriving in that gap recorded the new number and parked,
  and our kernel wakes futex waiters lowest queue slot first, so it could be the one woken: it saw no change and slept
  again, and the waiter the signal was for slept on (Chad's review of #67, item 2).

**Clocks.** POSIX's default condvar clock is `CLOCK_REALTIME`, which the bundle doesn't have, so a timed wait on a
default condvar is `EINVAL`. Timed waits work on `CLOCK_MONOTONIC` (`pthread_condattr_setclock`, which SpiderMonkey
uses) and through `pthread_cond_clockwait`. libc++ uses the latter for every `condition_variable` timeout once
`_LIBCPP_HAS_COND_CLOCKWAIT` is set in `__config_site` (upstream sets it only for glibc and Android). musl 1.2.5
doesn't declare that call, so `neo-crt/cxx/pthread.h` adds the declaration and then includes musl's header
(`#include_next`).

**Checks** (`cxx_runtime_smoke`, items 20–29 (16–17 are C3a's CP-106 checks, 18 is C3a's errno check, 19 is C3c-1's); all pass in QEMU, `NEO_SMP=4`):
- six threads updating a counter under a pthread mutex, yielding inside the critical section, lose no update; join
  returns each start routine's value;
- recursive, error-checking and normal mutex semantics (`EPERM`, `EDEADLK`, `EBUSY`);
- condvar wake; a monotonic timed wait times out no earlier than its deadline; the realtime default is refused;
- keys: each thread sees only its own value, each thread's destructor runs once at its exit, and a deleted key
  refuses values; a plain `std::thread` that sets a key runs its destructor once at exit too (item 29);
- `pthread_once` under six threads; rwlock sharing and exclusion;
- libc++: `std::thread` + `std::mutex` (lost-update check), `condition_variable::wait_for` timeout and wake,
  `call_once`, `shared_mutex`, `atomic::wait`/`notify_one`.

**Falsified** (results in the RETURN): with `pthread_mutex_lock`/`unlock` as no-ops, the lost-update checks fail;
with key destructors skipped, item 23 fails; without `Slots`'s `Drop`, item 29 fails (its destructor count is 0, as
Chad's probe found). The condvar's wake order has a host KAT (`tools/neo-crt-logic-tests`): a model condvar that
wakes the newest waiter first, as our futex queue can, with a late waiter arriving during the wake. The fixed order
gets the signal to the waiter it was for, and a control with the old order loses it every time.

**CP bank (C3b):**

| Class | This change | Guard / discriminator |
|---|---|---|
| Lost wakeup (condvar) | Introduces a condvar on std's | Sequence read before the user mutex is released, and the wake sent under the sequence lock (above); host KAT `signal_reaches_the_waiter_it_was_for` with its control; items 22 and 27 |
| Lazy init race | Boxed state installed on first use | CAS install, loser frees its box |
| Lock-holder preemption / spin | None: every wait blocks on the futex | — |
| Thread-exit ordering | Key destructors run on the exiting thread: before std's TLS teardown on `pthread_create`'s threads (4 rounds), and from `Slots`'s TLS `Drop` on any other thread that set a key (one round) | Items 23 and 29 |
| Join vs detach vs exit | `pthread_t` is an `Arc` record shared by the creator and the thread | Join returns values (item 20) |

**Not here** (each with the consumer that will need it):
- `pthread_exit` and cancellation (SpiderMonkey uses neither);
- `pthread_mutex_timedlock` (POSIX measures it on the realtime clock, so `EINVAL`);
- `pthread_getattr_np`/stack bounds (SpiderMonkey's stack limit; comes with C3 proper);
- C++ `thread_local` stays unusable on this target: it compiles to `%fs` accesses. SpiderMonkey must take its
  pthread-key path (`MOZ_THREAD_LOCAL` without compiler TLS).

## 10. C3c-1 amendment: localization and iostreams on (2026-11-08)

§3 configured libc++ with no localization or iostreams. SpiderMonkey needs them:
- `mfbt/Maybe.h`, `Array.h`, `ToString.h`, `mozglue/misc/TimeStamp.h` and `js/public/RegExpFlags.h` include
  `<ostream>`/`<sstream>`, and `Maybe.h` writes to the stream in non-dependent code;
- irregexp's imported V8 printers use `<iostream>`/`<iomanip>`.

Upstream's WASI build never meets this, because wasi-sdk's libc++ has localization. The other route was patching
about 15 SpiderMonkey files, irregexp's V8 imports included, and re-doing that on every `mozjs` move. The PM was told
on #87.

**What changes:**
- **`cxx/__config_site`:** drops `_LIBCPP_HAS_NO_LOCALIZATION`. Filesystem, `random_device` and the tz database stay off.
- **libc++ sources added:** `ios.cpp`, `ios.instantiations.cpp`, `iostream.cpp`, `locale.cpp`, `ostream.cpp`,
  `regex.cpp` and `strstream.cpp`. `fstream.cpp` stays out, because there is no filesystem.
- **musl files added, all from the same tag:**
  - locale: `locale_map`, `__mo_lookup`, `newlocale`, `freelocale`, `duplocale`, `uselocale`, `setlocale`,
    `strcoll`, `strxfrm`, `wcscoll`, `wcsxfrm`, `strtod_l`;
  - stdio: `getc`, `ungetc`, `fgetwc`, `getwc`, `ungetwc`, `fputwc`, `vasprintf`;
  - time: `strftime` and its three date helpers. `src/time/` is newly vendored whole; the rest of it isn't compiled.
- **neo-crt's own:**
  - `c/stdin.c`: musl's `stdin`, locked always, and every read is end-of-file, because the bundle has no input.
  - `neo_bottom.c`: `__tm_to_tzname` returns `"UTC"`, because there are no time zones. `__map_file` returns NULL; it
    is never reached, since `getenv` returns NULL and musl therefore never looks for locale files.
- **Locales:** still only `"C"` and C.UTF-8 in practice.
- **`_GNU_SOURCE`:** code that includes the locale or stream headers must define it, because musl declares
  `strto*_l` and `vasprintf` only then. libc++'s own build and the smoke define it.

**Smoke item 30** covers the following, each expected value worked by hand:
- `ostringstream` formatting (`"42 1.5 ff ****7"`);
- `istringstream` extraction and its failbit;
- `std::locale().name() == "C"`;
- `put_time` over `strftime` (`"2026-11-08 09:05 UTC"`);
- `regex_match`;
- `std::cin` at end-of-file.

**Cost:** about 7 more libc++ objects and 23 musl objects in every C++-using bundle. A Rust-only bundle doesn't link
neo-crt.

**Kill-switch:** none at run time. This is a build configuration: reverting `__config_site` and the source lists gives
back §3's runtime bit for bit. Everything §3 already compiled is unchanged. Items 1-29 still pass and keep their
expected values.

**Also for C3c-1: the POSIX calls SpiderMonkey links against** (`c/neo_posix.c`, the musl files named in `build.rs`).
The generator's link probe (#87) listed every symbol SpiderMonkey's 213 objects need that neo-crt didn't define.
Apart from the Rust crates the product link supplies (`encoding_c_mem`, `normalizer_glue`, zlib, `mozjs-sys`'s own
hooks), these are now here:
- **Time.**
  - musl's `gmtime`, `gmtime_r` and `timegm`.
  - `localtime`/`localtime_r` and `mktime` are UTC, because the bundle has no time zones.
  - `time()` reads `CLOCK_REALTIME`. That gives a time once C1.3's clock is in (#86) and returns -1 until then.
- **Files.**
  - `fopen` and `remove` refuse with `ENOENT`; `fstat`, `fcntl` and `close` refuse with `EBADF`; `memfd_create`
    refuses with `ENOSYS`.
  - `fclose` flushes the standard stream it's given; there is no other kind.
  - musl's `ferror`, `fileno`, `setvbuf`, `perror` and `fread` work on the standard streams.
- **`getpid`** is std's `process::id`.
- **`pthread_setname_np`** is accepted and ignored.
- **The stack protector.** SpiderMonkey's configure turns on `-fstack-protector-strong`.
  - `neo_crt::seed_stack_canary()` randomises `__stack_chk_guard`, with the low byte zero. A program with
    stack-protected code calls it first, before `init()` and before any C runs.
  - It draws from the kernel CSPRNG. `init()` doesn't call it, so a bundle with no stack-protected code (SWGL)
    needs no entropy to start.
  - The first quick-tier run had seeded it inside `init()`. That aborted `swgl_smoke`, whose test kernel has no
    `kernel-rng`, by std's no-weaker-fallback rule.
  - `__stack_chk_fail` reports and aborts.

Smoke item 31 checks these with hand-worked values (1971-01-01 is 31,536,000 s and a Friday).
