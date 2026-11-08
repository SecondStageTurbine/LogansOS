<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 349: #10 design read (SpiderMonkey on neo-crt, interpreter-only)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §349.

**Reviewed:** PR #10 head `c2d5f3ccb79e3fa05c7838771b618ed92f574968` ("THINK C3c §2.2: memory/build and
memory/mozalloc are compiled (MPL)"), one file +199 from merge-base `07d7f6ef9651a143191e2aa8b525713b3fc3bf16`:
`docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md`. `origin/main` at review time is `537aa9bbd`. The PM's decisions are
the 2026-11-08 comments on #10 (§6.1 option (i), §3.3 option (c)); the vendoring pin is `servo/mozjs` @
`138c9470151b8dcbb335f249cb83f9404857baf9`.

**Overall: FAIL.** Item 2 passes. Item 1 fails on a linked file the table calls MPL-2.0: `js/src/dtoa.c` is compiled
through `DoubleToString.cpp` under a Lucent permissive notice that is not on CLAUDE.md's allowed list (no GPL/LGPL is
linked, but the whitelist is not met), and the table also misstates the `gemmology`/`xsimd` link and omits the GPL
cut-list. Items 3, 4 and 5 have design gaps: T1's trap row still names the rejected option (ii); the trap bank's
discriminators are not all in the named gate; the CP table is deferred and single-threaded is *assumed*, not enforced
(on x86_64 the no-op backend makes `JS_NewContext` fail); §7 names only the C3c-1 gate. The architecture is right;
this is a docs fix before the vendor/build, not a redesign.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | §2.2's licence table holds at the pin: every linked part is on the allowed list, nothing GPL/LGPL is linked | **FAIL** — `js/src/dtoa.c` is linked under a Lucent permissive notice that is not on the allowed list; no GPL/LGPL is linked, but the table's `js/src` row says MPL-2.0, its `gemmology`/`xsimd` link cell is wrong, and its vendor cut-list omits the GPL-only files | THINK:56-90; clone at `138c9470`; `js/src/util/DoubleToString.cpp:70`; `js/src/dtoa.c:1-19` (details below) |
| 2 | §3.3 option (c): the product build fetches nothing, and the CI generator job fails if its output differs from the committed files | **PASS** | THINK:140,142-143,157-160,181-188 |
| 3 | §6.1 option (i) is stated as decided, and C1.3's std behaviour is unchanged | **FAIL** — §6.1 says it; T1 still directs option (ii) and its discriminator cannot tell 1970 from a real date | THINK:151 versus 175-180; `docs/ADR-078-ot61-browser-c-ladder.md:33`; `userland/neoplutarch-std-overlay/std/time_neoplutarch.rs:7-10` |
| 4 | The trap list (T1 …) each has a discriminator, and the CP table covers the threads the interpreter can start (single-threaded enforced, not assumed) | **FAIL** — every trap has a discriminator, but most are not in the named gate, the CP table is absent, and the no-op backend does not make the engine single-threaded on x86_64 | THINK:118-122,147-165; `js/src/vm/Runtime.cpp:63-67,176`; `js/src/threading/noop/NoopThread.cpp:30`; `js/src/vm/InternalThreadPool.cpp:134-149,240-242`; `js/src/vm/JSContext.cpp:193-197`; `mozjs/src/rust.rs:350-353` |
| 5 | The next steps name a gate for each rung | **FAIL** — §7 names the C3c-1 smoke gate only; no C3c-2 step/gate, and the vendor/generator and neo-crt steps name no gate | THINK:192-199; §3.2 at THINK:118-126 |

## 1. §2.2 licence table — FAIL: `dtoa.c` is linked under a non-whitelisted licence

The pin is correct. I cloned `servo/mozjs` at
`138c9470151b8dcbb335f249cb83f9404857baf9` (`git clone --filter=blob:none --no-checkout`, then `git fetch --depth 1
origin <sha>`; `git rev-parse HEAD` matched; `config/milestone.txt` reads `153.6.0`). File counts match the table
exactly: `mfbt` 230, `mfbt/double-conversion` 27, `js/src/irregexp` 76, `modules/fdlibm` 77, `mozglue/misc` 90,
`js/src/zydis` 101, `nsprpub` 536, `intl` 1,883. Every licence file named in the table was read from the clone.

**The dispatch's two tests do not fully pass.** No GPL/LGPL part is linked: the GPL files in the tree are
generator/dev-only (`js/src/util/make_unicode.py`, `js/src/devtools/rootAnalysis/run_complete`) or unlinked
(`js/src/vtune`, which is dual BSD/GPLv2 and needs `--enable-vtune`; `js/src/moz.build:333-335`). The one non-list
licence the THINK does flag, `third_party/function2` BSL-1.0, is not linked: the only `MoveOnlyFunction` reference in
the clone is `mfbt/MoveOnlyFunction.h` itself. **But `js/src/dtoa.c` is linked and is not MPL-2.0 or on the allowed
list.** It is included by `js/src/util/DoubleToString.cpp:70` (`#include "dtoa.c"`) and `js/src/util/moz.build:34-37`
compiles `DoubleToString.cpp`; its notice (`js/src/dtoa.c:1-19`) is the Lucent / David M. Gay permissive grant
("Permission to use, copy, modify, and distribute this software for any purpose without fee is hereby granted…"), not
one of MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0 or Unlicense/CC0. That is a linked part outside the
whitelist, so the `js/src` row's "MPL-2.0" claim is wrong. The PM must either add that permissive licence to the list
(no copyleft; the closest precedent is the BSL question) or replace `dtoa.c` before the build.

**Per-row check:**

| Row | Table claim | Checked at the pin | Verdict |
|---|---|---|---|
| `js/src`, `js/public` | MPL-2.0, links | **FAIL — `dtoa.c`.** The engine/public headers are MPL-2.0, but `js/src/dtoa.c` is a linked third-party file under the Lucent permissive notice (`:1-19`), included by `js/src/util/DoubleToString.cpp:70`, which `js/src/util/moz.build:34-37` compiles. `js/src/xsum/xsum.cpp` is MIT-style (permission + notice + disclaimer) and `js/src/util/Unicode.cpp` is public-domain/CC0, both allowed; `vtune/` (dual BSD/GPLv2) is not linked without `MOZ_VTUNE`; `util/make_unicode.py` and `devtools/rootAnalysis/run_complete` are GPL-3.0-or-later but are not linked. | **FAIL** (`dtoa.c` not on the list; GPL-only files still need cutting) |
| `mfbt` | 230, MPL-2.0, links | The 230 includes `mfbt/double-conversion` (27) and `mfbt/tests`; the top-level has `Span.h` (MIT, Microsoft GSL) and `ThreadSafety.h` (BSD-style, WebRTC) in addition to MPL. Both extra licences are allowed. | **PASS** (label incomplete) |
| `mfbt/double-conversion` | 27, BSD-3-Clause, links | `LICENSE` is the V8 3-clause text. | **PASS** |
| `js/src/irregexp` | 76, BSD-3-Clause, links (RegExp) | `LICENSE.v8` is the V8 3-clause text. | **PASS** |
| `modules/fdlibm` | 77, BSD-2-Clause style, links (`Math`) | `src/LICENSE` is the FreeBSD 2-clause text. | **PASS** |
| `mozglue/misc` | 90, MPL-2.0, links | The 90 includes `decimal/Decimal.cpp` and `decimal/Decimal.h` under BSD-3-Clause (Google, `Decimal.cpp:2-25`), plus `decimal/DoubleConversion.h`; the rest is MPL. BSD-3 is allowed. | **PASS** (label incomplete) |
| `memory/mozjemalloc` | MPL-2.0, **no** | `MOZ_MEMORY` is set only by `--enable-jemalloc` (`build/moz.configure/memory.configure:18-27`); `js/moz.configure:1434-1439` excludes mozglue for standalone JS without jemalloc; `memory/build/moz.build:36-49` compiles `Fallback.cpp` when it is off, not `mozjemalloc.cpp`. | **PASS** |
| `memory/build`, `memory/mozalloc` | MPL-2.0, **yes** | `memory/build` builds `Fallback.cpp` (a system-`malloc` wrapper, `Fallback.cpp:12-45`) and `memory/mozalloc` builds `mozalloc.cpp` (`mozalloc.cpp:12-25` maps to libc when `MOZ_MEMORY` is off). The PM's generator probe run 42508546125 emitted unified files for both. Both MPL-2.0. | **PASS** |
| `third_party/fmt` | MIT, yes | `third_party/fmt/LICENSE` is MIT; `mozglue/moz.build:32` builds it; `mfbt/BitSet.h:8`, `mfbt/Casting.h:21-23` (DEBUG) and `mozglue/misc/Debug.h:10` include it. | **PASS** |
| `third_party/function2` | BSL-1.0, expected **no** | BSL-1.0 is not on the allowed list, but nothing includes `mfbt/MoveOnlyFunction.h` (the only reference is the header itself), so it does not link. | **PASS** (not linked) |
| `js/src/zydis` | 101, MIT, **no** | MIT (`Zydis/LICENSE`, `Zycore/LICENSE`); `js/src/moz.build:351-352` adds it only with `JS_JITSPEW`. A debug build defaults `--enable-jitspew` on (`js/moz.configure:516-521`), so the T3 debug-assertion run may link it; MIT is allowed. | **PASS** (conditional) |
| `third_party/gemmology`, `third_party/xsimd` | MIT / BSD-3-Clause, **no** | **They link by default on x86_64.** `ENABLE_WASM_MOZ_INTGEMM` defaults true when `target.cpu` is x86/x86_64/aarch64 (`js/moz.configure:962-984`); `js/src/moz.build:348-349` adds `intgemm`; `js/src/intgemm/moz.build:14-30` compiles the gemmology kernels and includes `xsimd`; `js/src/intgemm/IntegerGemmIntrinsic.cpp:11-13` includes `gemmology_fwd.h` and `fmt/format.h`. Both licences are allowed, so this is not a licence failure, but the "no" is wrong. | **FAIL (link cell)** |
| `modules/zlib/src` | 2 headers, zlib, headers only | Only `zlib.h` and `zconf.h` are present; the code comes from `libz-sys`/`libz-rs` (zlib licence). | **PASS** |
| `nsprpub` | 536, MPL-2.0, **no** | MPL-2.0 (`nsprpub/LICENSE`); standalone JS does not build it without `--enable-nspr-build` (`build/moz.configure/nspr.configure:8-14`). | **PASS** |
| `intl` | 1,883, ICU/Unicode, **no** | Intl is off; ICU/Unicode is allowed anyway. | **PASS** |
| `python`, `third_party/python`, `build`, `testing` | MPL + the packages' own, **no** | Not linked, but the vendor copy would include GPL-only files: `third_party/python/yamllint` (GPL-3.0-or-later, `yamllint-1.37.1.dist-info/METADATA:6`; `yamllint/__init__.py:3-7`), `js/src/util/make_unicode.py` (GPL-3.0-or-later, `:7-18`, and it calls `urlopen` at `:18`), and `js/src/devtools/rootAnalysis/run_complete` (GPL-3.0-or-later, `:6-16`). These must be cut from the vendor tree. | **FAIL (cut-list)** |

**What needs fixing in §2.2 before the vendor PR:**

1. **`js/src/dtoa.c` is linked and is not on the allowed list.** The `js/src` row cannot say "MPL-2.0"; either add the
   Lucent / David M. Gay permissive notice to CLAUDE.md's list (it is non-copyleft and includes a warranty
   disclaimer) or replace `dtoa.c` with an allowed implementation. This is a decision for J.c./the PM, like the BSL
   question.
2. **`gemmology`/`xsimd` are linked by default on our x86_64 target.** Either the generator passes
   `--disable-wasm-moz-intgemm` (or `--wasm-no-experimental`) and the table says so, or the vendor tree keeps them and
   the table says "yes". The 17:58 vendor-plan comment says the vendoring excludes `gemmology`/`xsimd`; with the
   current configuration that build fails on missing `gemmology_fwd.h`/`xsimd` headers. Their licences are fine, so
   keeping them is the smaller correction.
3. **`mfbt` and `mozglue/misc` are mixed-licence directories.** MPL-2.0 is the majority, but `Span.h` (MIT),
   `ThreadSafety.h` (BSD-style) and `mozglue/misc/decimal` (BSD-3) are allowed and should be named, so the build PR's
   "every linked part" claim is literally true. The `mfbt` count of 230 also includes the separately listed
   `mfbt/double-conversion` (27), so it is a directory count, not a disjoint row.
4. **Add a GPL cut-list.** The vendor copy must exclude at least `third_party/python/yamllint`,
   `js/src/util/make_unicode.py` and `js/src/devtools/rootAnalysis/run_complete`; `js/src/vtune` is dual
   BSD/GPLv2 and can be kept under BSD or cut, but it must not be linked without `--enable-vtune`. The table's last
   row should say "generator-time only; GPL-only files cut, not vendored", not just "the packages' own".
5. **Keep `memory/build` and `memory/mozalloc`.** The 17:58 comment still excludes `memory` wholesale. The c2d5f3cc
   correction is right: those two subdirectories are compiled; `memory/mozjemalloc` is not. The vendor PR must include
   the first two and exclude the third.

## 2. §3.3 option (c) — PASS

The design says exactly what the item asks:

- **Product build fetches nothing:** "Product build: `cc` only, from the committed outputs" (THINK:140); "The product
  build runs `cc` only and fetches nothing (the vendoring rule)" (THINK:184). T7 says option (c) does not run the
  `mozjs-sys` `build.rs` that tries the archive download (THINK:157), and T8 confines the Python to vendored
  `third_party/python` with the network off (THINK:158).
- **CI generator fails on drift:** "The CI job re-runs the generator and **fails if its output differs from what's
  committed**, so drift can't hide" (THINK:140); "The generator job **fails if its output differs from what's
  committed**" (THINK:185-186). T9 makes the emitted artifact the unified files themselves (THINK:159), and T10
  pins the bindgen output to clang 19 (THINK:160).

The CI job's commit mechanism (who pushes the frozen outputs, and how the drift job reports on a fork PR) is not
specified, but that is generator-PR implementation detail, not a design failure.

## 3. §6.1 option (i) — FAIL: §6.1 is right, T1 still directs the rejected option (ii)

§6.1 is correct and matches the PM's 2026-11-08 decision: it says "option (i), contained", `neo-crt`'s
`CLOCK_REALTIME` returns the process-relative count from 1970, "std is untouched, so C1.3's stated std behaviour
(`SystemTime::now()` unsupported) stands", and it explicitly records "(ii) … *Not taken*" (THINK:175-180). That is
consistent with C1.3's record (`docs/ADR-078-ot61-browser-c-ladder.md:33`) and the overlay
(`userland/neoplutarch-std-overlay/std/time_neoplutarch.rs:7-10`: `SystemTime` stays std's `unsupported` one and a
process cannot tell `rtc-epoch` from boot-relative realtime).

**But T1 was not updated when §6 was.** The T1 guard still says:

> "… Route std's `SystemTime::now()` to that syscall (already in the A4 manifest, so no new syscall), then
> `neo-crt`'s `CLOCK_REALTIME` reads `SystemTime`. **Decision for the PM (§6).**" (THINK:151)

That is option (ii) — the one §6.1 says is **not taken** — and the decision is no longer pending. A builder reading
the trap bank would modify std's `SystemTime`, which is exactly the C1.3 change the decision forbids. The T1
discriminator is also too weak: "non-negative and non-decreasing" would pass a real date, so it does not distinguish
option (i) from option (ii). #86's `crt_checks.c:303-307` is the right shape: it requires the call to succeed, the
value not to go backwards, and `tv_sec < 3600` (1970's first hour). T1 should point at #86 and that check, and should
say `monotonic_epoch()`, not `SystemTime`.

## 4. Trap bank and CP table — FAIL: no CP table, and single-threaded is not enforced

**Every trap has a discriminator (that half passes).** T1-T12 each have a non-empty "Discriminator" cell
(THINK:151-162). But the sentence above the table says "Each trap names the discriminator that catches it on the
C3c-1 gate" (THINK:147), and the named gate (THINK:118-122) contains only: the fixed-script byte oracle, a forced
compacting GC, an allocation loop past the C2.2 quota, a `Date.now()` "returns a value, not a crash", and A4. It does
not contain T2's `Math.random()` across two boots, T3's "allocate past one GC chunk" plus the debug-assertion run,
T4's unbounded-recursion `InternalError`, or T12's explicit shutdown/exit 0; T5/T11 are link-failure checks and
T6-T10 are generator/build checks, not C3c-1 runtime checks. Either the gate list must grow to include the runtime
discriminators, or the table must say which discriminator belongs to which gate. As written, the trap bank promises
coverage the gate does not run.

**The CP table is absent.** The THINK says "The concurrency pitfall bank applies to **C3c-2**, not C3c-1: C3c-1 is
one thread by construction (no-op threading backend). C3c-2's dispatch carries the CP table" (THINK:164-165). There
is no CP table in the THINK or in PR #10's body. C3c-2 is where the interpreter can start helper threads (off-thread
parse, GC helper tasks, condition variables/mutexes), and the design names no CP rows (at least CP-14 lost wakeup,
CP-33 teardown, CP-61 wait-slot identity, and CP-67 park-kill are the relevant classes) and no discriminators. A
future dispatch carrying the table is not the same as this design covering the threads it introduces.

**Single-threaded is assumed, not enforced — and on x86_64 the no-op backend makes the engine fail to start.** This
is the load-bearing finding. The design says "no-op threads" (THINK:116-117) and "one thread by construction"
(THINK:164), but the no-op backend only makes `Thread::create` return `false`
(`js/src/threading/noop/NoopThread.cpp:30`). It does not make SpiderMonkey believe it is single-threaded:

- `js/src/vm/Runtime.cpp:63-67` sets `gCanUseExtraThreads = true` unless `__wasi__` is defined. Our
  `x86_64-unknown-neoplutarch` target does not define `__wasi__` just by being POSIX-shaped.
- `js/src/vm/Runtime.cpp:176` then calls `EnsureHelperThreadsInitialized()` whenever `CanUseExtraThreads()` is true.
- That reaches `GlobalHelperThreadState::ensureInitialized()` (`js/src/vm/HelperThreads.cpp:159-190`), which builds
  `InternalThreadPool::Initialize(threadCount, lock)`. `threadCount` is `max(cpuCount, 2)` (from
  `ThreadCountForCPUCount`, `HelperThreads.cpp:92-97`), i.e. 2 even with the no-op `GetCPUCount() == 1`.
- `InternalThreadPool::ensureThreadCount` calls `HelperThread::init`, which calls `thread.init(...)`; the no-op
  `Thread::create` returns `false` (`NoopThread.cpp:30`), so `HelperThread::init` returns `false`
  (`InternalThreadPool.cpp:240-242`), `ensureThreadCount` returns `false` (`InternalThreadPool.cpp:134-149`), and
  `JSRuntime::init` returns `false` (`js/src/vm/Runtime.cpp:176-178`).
- `NewContext` then deletes the runtime and returns `nullptr` (`js/src/vm/JSContext.cpp:193-197`), and the safe
  `mozjs` layer unwraps `JS_NewContext` (`mozjs/src/rust.rs:350-353`), so the first `Runtime::new` panics.

So the no-op backend is not "single-threaded by construction"; it is "helper-thread init fails". The design must pick
and state one enforcement:

1. define `__wasi__` for the C3c-1 build (then audit every `__wasi__` branch — `NativeStack.cpp:117-127` is the one
   T4 already says does not fit, and `PlatformMutex.h:10-18` / `PlatformConditionVariable.h:10-18` /
   `ConditionVariable.h:12-14` switch to no-op mutex/condvar), or
2. patch `Runtime.cpp` / add a `__neoplutarch__` branch that sets `gCanUseExtraThreads = false` (or calls
   `js::DisableExtraThreads()` before the first runtime) and patch the source selection in
   `js/src/threading/moz.build:21-33` and `mozglue/misc/moz.build:109-127`, both of which currently key on
   `CONFIG["OS_ARCH"] == "WASI"`.

T6 only names `build/moz.configure/init.configure` (THINK:156). That is not enough: even if `OS_ARCH` is taught the
new triple, the threading and mozglue selections are separate `moz.build` conditions, and `gCanUseExtraThreads` is a
C++ `#ifndef __wasi__`, not an `OS_ARCH` check. The C3c-1 gate should assert the enforcement (e.g.
`CanUseExtraThreads() == false` and no `clone`/`pthread_create` in the ELF), not assume it.

## 5. Next steps and gates — FAIL

§7 (THINK:192-199) has four steps:

1. PM reads it (decided).
2. Vendor `mozjs` at the pin and write the generator job, "commit its first outputs from the CI run" (THINK:195-196).
3. `neo-crt` additions: `arc4random_buf`, the `posix_memalign` check (#65), and the T1 clock (#86)
   (THINK:197-198).
4. `spidermonkey_smoke` and its gate (§3.2) (THINK:199).

Only step 4 names a gate, and it is the C3c-1 gate. The generator's drift gate is described in §3.3/§6.2 but not
named in §7; step 3 names PRs #65/#86 but not their gates; and there is no C3c-2 step or gate in §7 at all. §3.2
does name C3c-2's gate ("ADR-078's 'helper threads join': off-thread parse or GC helper tasks run and join, and the
content process exits cleanly with them running", THINK:123-126), but the next steps do not carry it. If "each rung"
means C3c-1 and C3c-2, §7 names one of the two; if it means each next step, steps 2-3 have no named gate. Either
reading fails the item. Add a C3c-2 step with its gate, and name the generator-drift gate and the #65/#86 gates (or
say they are covered by their own PRs' gates) in §7.

## What holds, and should not be redesigned

- **`servo/mozjs` is the right engine choice.** MPL-2.0, engine 153.6.0, the crate Servo's `script` is written
  against, and the WASI port already has the no-JIT/no-shared-memory/no-threads shape (THINK:30, 96-112). QuickJS/Boa
  would mean rewriting Servo's bindings; the survey's "nothing" for them is right.
- **The WASI GC-memory path is the right A2/A1 fit.** `posix_memalign` + `memset` + `free` over `neo-crt`'s
  `malloc` keeps GC memory under the C2.2 quota and never makes a page executable (THINK:106-110).
- **Option (c) generate-then-freeze is right**, for the reasons in item 2.
- **§6.1 option (i) is right**; #86 implements it and the #86 review already PASSes the code. The fix here is only to
  make T1 agree with §6.1.
- **The two-step split C3c-1 / C3c-2 is right**: it removes the C3b dependency from the first engine gate and keeps
  the helper-thread step SMP-touching and CP/AAG-gated (THINK:114-129).
- **The T1-T12 list is a useful trap bank**; T1, T2, T4, T7 and T8 are the right traps. They need to be wired to the
  gate and joined by the thread-init trap above.

## Recommendation to the PM

Bounce this THINK for a docs-only fix before the vendor PR starts:

1. **T1:** replace the option (ii) guard with option (i) — `neo-crt`'s `CLOCK_REALTIME` reads
   `monotonic_epoch()`; std is untouched; `#86` carries it. Strengthen the discriminator to #86's `crt_checks.c`
   1970-window + advance check (THINK:151).
2. **§2.2:** resolve `js/src/dtoa.c` (add the permissive notice to the allowed list or replace it), correct the
   `gemmology`/`xsimd` link cell (they link by default on x86_64), name the mixed licences in `mfbt`/`mozglue/misc`,
   add the GPL cut-list (`yamllint`, `make_unicode.py`, `run_complete`), and keep `memory/build` + `memory/mozalloc`
   in the vendor tree.
3. **Trap bank / gate:** add the `gCanUseExtraThreads` / no-op-thread-init trap and the exact enforcement
   (`__wasi__` or `DisableExtraThreads`/`__neoplutarch__`), and add its gate assertion. Say which T-rows are
   runtime-gate checks and which are build/generator checks.
4. **CP table:** put the C3c-2 helper-thread CP table (and its discriminators) in this THINK, or make C3c-2's
   dispatch a hard dependency of this design's approval. Do not leave "one thread by construction" as the only
   statement.
5. **§7:** add a C3c-2 step and gate, and name the generator-drift gate and the #65/#86 gates (or explicitly say
   those PRs carry them).

Then the vendor/generator build can start from an accurate table and a single-threaded build that actually starts.

## Verification performed

- `git fetch origin`; fetched `refs/pull/10/head` and confirmed the reviewed head is
  `c2d5f3ccb79e3fa05c7838771b618ed92f574968`; `git diff --stat 07d7f6ef9..refs/pr/10` is one file, +199; `git diff
  --check` is clean. `origin/main` at review time is `537aa9bbd`.
- Cloned `servo/mozjs` at the pin with `git clone --filter=blob:none --no-checkout`, `git fetch --depth 1 origin
  138c9470151b8dcbb335f249cb83f9404857baf9`, `git checkout FETCH_HEAD`; confirmed `git rev-parse HEAD` and
  `config/milestone.txt` = `153.6.0`.
- Read the licence files named in §2.2 and counted each row's files; inspected `moz.build`/`moz.configure` for
  `mozjemalloc`, `memory/build`, `memory/mozalloc`, `fmt`, `function2`, `zydis`, `gemmology`/`xsimd`, `nspr`, `vtune`
  and `intl`. Searched the linked directories for GPL/LGPL text and for linked files with non-MPL headers, which
  found `js/src/dtoa.c` (Lucent permissive, included by `DoubleToString.cpp:70`); also searched the whole tree for
  `MoveOnlyFunction` and `__wasi__`.
- Read `js/src/vm/Runtime.cpp`, `js/src/threading/noop/NoopThread.cpp`, `js/src/threading/moz.build`,
  `js/src/vm/InternalThreadPool.cpp`, `js/src/vm/HelperThreads.cpp`, `js/src/vm/JSContext.cpp`, `mozjs/src/rust.rs`,
  `mozglue/misc/moz.build`, `mozglue/misc/PlatformMutex.h`, `mozglue/misc/PlatformConditionVariable.h`,
  `js/src/threading/ConditionVariable.h`, `js/src/util/NativeStack.cpp`, `js/src/vm/Time.cpp` and
  `mfbt/RandomNum.cpp`.
- Read `docs/ADR-078-ot61-browser-c-ladder.md:33`, `userland/neoplutarch-std-overlay/std/time_neoplutarch.rs:7-10`,
  and `Plans/RETURN-2026-11-08-pr86-review-dexter.md` for the C1.3/#86 clock context.
- Did not run `configure` or build anything. The THINK and the PM's 17:58 comment both say the generator runs on the
  CI runners, never on a seat's machine (J.c., 2026-10-29); this is a design read. No source, THINK or other file
  changed; this commit is the RETURN only. The pre-existing untracked `pr59-review/` directory is not staged and is
  not part of this branch.
- RAG is not available in this worker (`http://127.0.0.1:8404/health` refused the connection); direct reads and
  `grep` supplied the evidence.
