<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 355: #10 narrow re-check (SpiderMonkey on neo-crt)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-355-358.md` §355.

**Reviewed:** PR #10 head `65da764c86ac157d77566f3dd11ec7d6f41e620a` ("THINK C3c: Dexter's design read
(relay 349): licences, T1, threads, gates"), one file `+41/−8` from `c2d5f3ccb79e3fa05c7838771b618ed92f574968`:
`docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md`. `origin/main` at review time is `d5576597d1301b2d66cee938bcbd542118788abe`.
This is a narrow re-check of the four relay-349 FAILs only; the architecture and the parts that passed at
`c2d5f3ccb` are not re-reviewed.

**Overall: FAIL.** Items 2 and 4 pass. Item 1 passes as worded, with one §2.2 cut-list note (two LGPL PGO
inputs are still vendored). Item 3 still fails: the new enforcement paragraph is technically right, but the
design's own §3.1/§3.2 still tell a builder to use the WASI no-op threading backend, which is the exact
selection that makes `JS_NewContext` fail. The fix is a small docs reconciliation, not a redesign.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | §2.2: `dtoa.c` under the allowed notice; `gemmology`/`xsimd` off by the generator flag or marked links; GPL-only files in the cut-list | **PASS** (GPL-only) — `dtoa.c` is on the list, the intgemm flag is real, and every GPL-only file I named is cut. **Note:** the row's wider "GPL/LGPL … not vendored" claim is not literally true: two LGPL-2.1 PGO inputs are vendored and not in #87's cut-list. | THINK:70,81,86-87; `CLAUDE.md:917`; #87 workflow `:76,134`; `VENDORED.md:125` |
| 2 | T1 describes option (i), and its discriminator can tell 1970 from a real date | **PASS** — T1 now states option (i) and #86, and its `Date.now()` bound excludes a 2026 real date | THINK:155; #86 `userland/neo-crt/src/lib.rs:255-286`, `crt_checks.c:303-307` |
| 3 | Single-threaded is enforced, not assumed (how `JS_NewContext` gets past the no-op backend), with a CP table for any remaining thread | **FAIL** — the new paragraph and CP table are added and correct, but §3.1 and §3.2 still say C3c-1 uses the WASI no-op backend and "today's `neo-crt`". That is the selection that fails `JS_NewContext`; the document now says both "no-op" and "not no-op". | THINK:111,115-116,121 versus 168-175,177-185; patch 0003 `Runtime.cpp`; upstream `Runtime.cpp:63-67,176` |
| 4 | §7 names a gate for every step | **PASS** — the new table gives every §7 step a gate, including C3c-2 and the generator drift check | THINK:224-232 |

## 1. §2.2 licence table — PASS as worded, with an LGPL cut-list note

The three checks in the dispatch all hold at the reviewed head:

- **`dtoa.c` is recorded under the allowed notice.** THINK:70 gives the file its own row, names the David M.
  Gay / Lucent permissive notice, says it is **not** MPL-2.0, and records the PL addition. The allow-list entry is
  real: `CLAUDE.md:917` (on `origin/main`, commit `da68b78f2`) names "the dtoa / Lucent permissive notice
  (David M. Gay's `dtoa.c`, as SpiderMonkey links it; PL, 2026-11-08)". The link path is also right: upstream
  `js/src/dtoa.c:1-19` is the Lucent grant, `js/src/util/DoubleToString.cpp:70` is `#include "dtoa.c"`, and
  `js/src/util/moz.build:34-37` compiles `DoubleToString.cpp`.
- **`gemmology`/`xsimd` are turned off by the generator flag.** THINK:81 says the flag is
  `--disable-wasm-moz-intgemm` (#87). That is not a paper claim: `.github/workflows/ot61-mozjs-generate.yml`
  on `origin/claude/ot61-c3c1-mozjs-vendor` (`544cd2410`) passes it in both configure steps (`:76`, `:134`).
  Upstream `js/moz.configure:962-984` confirms `ENABLE_WASM_MOZ_INTGEMM` defaults on for x86_64 and the option
  clears it. Neither directory is in the vendor tree (`git ls-tree` on the #87 branch).
- **The GPL-only files are in the cut-list.** THINK:86 now names the GPL-only files (`yamllint`, `pylru`,
  `tooltool.py`, `make_unicode.py`, `run_complete`) plus the LGPL `autocommand` and ICU's autoconf scripts.
  Each of those paths is absent from #87's vendor tree at `544cd2410`, and `VENDORED.md:125` records the cut
  list. `build/autoconf/config.sub`/`config.guess` are separately called out as GPL-3.0 **with the autoconf
  exception** and flagged for the PM (THINK:87; `VENDORED.md:128-131`), which is the right treatment.

**§2.2 note (not the dispatch's GPL-only check, but the row claims it):** the table row at THINK:86 says
"GPL/LGPL files … not vendored (#87's cut list)". Two LGPL-2.1-only PGO inputs **are** in the #87 vendor tree
and are not in the cut-list:

- `third_party/ot61-mozjs/vendor/mozjs-138c9470/mozjs-sys/mozjs/build/pgo/js-input/3d-thingy.html` —
  "GNU Lesser General Public License … version 2.1" (`:14-18`);
- `third_party/ot61-mozjs/vendor/mozjs-138c9470/mozjs-sys/mozjs/build/pgo/js-input/sunspider/date-format-xparb.html` —
  LGPL-2.1 (`:44-47`).

Neither is GPL-only, so the dispatch's literal item still passes; but `CLAUDE.md` says GPL **and LGPL** code is
never copied, and the row's "GPL/LGPL … not vendored" is therefore inaccurate. Add them to #87's cut-list (they
are PGO benchmark inputs, not generator inputs) or name and flag them. This does not need to block the design
if the cut-list is corrected in #87.

## 2. T1 — PASS: option (i), and the discriminator separates 1970 from a real date

THINK:155 now says exactly what §6.1 decided: **option (i)**, `neo-crt`'s `CLOCK_REALTIME` returns the
process-relative count read as seconds since 1970, std is untouched, and #86 carries it. That matches #86's
implementation:

- `userland/neo-crt/src/lib.rs:255-286` defines `CLOCK_REALTIME = 0`, accepts it in `clock_gettime`, and writes
  the same `monotonic_epoch().elapsed()` value for both clocks (`:273-285`), with the comment that it is 1970
  plus process time and never the real date.
- `userland/cxx-runtime-smoke/c/crt_checks.c:303-307` checks the call succeeds, does not go backwards against
  an earlier monotonic read, and stays inside 1970's first hour (`r.tv_sec >= 0 && r.tv_sec < 3600`).

The T1 discriminator is two `Date.now()` reads, both `>= 0` and `< 86,400,000` ms, the second `>=` the first
(THINK:155). A 2026 real date is about `1.79e12` ms, so it fails the bound; a 1970-plus-uptime value passes.
That is enough to tell option (i) from a real clock. One small mismatch, not a fail: #86's oracle is the first
**hour** (3600 s), while T1 says the first **day** (86,400,000 ms). The trap's bound still discriminates; if the
smoke is meant to use #86's exact check, say "first hour" here too.

## 3. Single-threaded — FAIL: the enforcement paragraph is right, §3.1/§3.2 still prescribe the no-op backend

The added mechanism is correct:

- patch 0003 changes `js/src/vm/Runtime.cpp` from `#ifndef __wasi__` to
  `#if !(defined(__wasi__) || defined(__neoplutarch__))`, so `gCanUseExtraThreads` is **false** for our target
  (patch hunk at `Runtime.cpp:63`; upstream `:63-67`).
- `JSRuntime::init` calls `EnsureHelperThreadsInitialized()` only `if (CanUseExtraThreads())` (upstream
  `Runtime.cpp:176`), so the pool is never created and the no-op `Thread::create` failure is never reached.
- The CP table at THINK:177-185 covers the helper-thread pool (not started), the real POSIX locks/condvars on
  one thread, atomics, `MOZ_THREAD_LOCAL` via pthread keys, and the one-context rule. The smoke's
  `pthread_create` count is the right discriminator for the pool.

**But the document still says the opposite in the design summary.** THINK:111 says for threads "**WASI first**
(C3c-1), **POSIX later** (C3c-2, on C3b)". THINK:115-116 says "The C3a THINK's hard rule … is exactly what the
**no-op threading backend** gives. So the first step can run with **today's `neo-crt`**." THINK:121 says C3c-1 is
"`mozjs` with the WASI-shaped configuration … **no-op threads**". All three are the selection the new paragraph
says is *not* used. A builder reading §3.1/§3.2 before §4 will keep `noop/NoopThread.cpp`, and then
`HelperThread::init`/`InternalThreadPool::ensureThreadCount` return false and `JS_NewContext` returns null —
the exact relay-349 failure.

The fix is confined to §3.1/§3.2: say C3c-1 uses the **POSIX** threading sources (because `OS_ARCH` is
`NeoPlutarch`, not `WASI`; upstream `js/src/threading/moz.build:21-33`) with `gCanUseExtraThreads = false`, and
state the dependency accurately. C3b is already on `main` (`f19aeb9a4`, `abbd20b44`); #88 supplies the POSIX gap
this step needs (`pthread_setname_np`, time/locale, stack canary). The old "C3a → C3c-1, C3b → C3c-2 in
parallel" split at THINK:132-133 should be re-read against that: C3c-1 is no longer independent of the pthread
layer, and C3c-2 is only the `gCanUseExtraThreads = true` flip plus its own CP table/AAG.

## 4. §7 gates — PASS: every step has a gate

The new table (THINK:224-232) gives each §7 step a gate:

- vendor/generator/frozen outputs → #87: generator workflow green, drift check, cbindgen check, bindgen, push
  CI, PM review;
- neo-crt additions → #88: close-gate, quick tier with `cxx_runtime_smoke` items 30/31 and `swgl_smoke`, full
  four shards before merge;
- `cc`-only product build → after #89: `xtask ot61-closure-build --mozjs`, push CI, generator drift check;
- C3c-1 smoke on target → after #89: quick tier with `mozjs_smoke` (T1, T3, T4, B1, thread count), full four
  shards;
- C3c-2 → its own THINK/CP table, cross-family AAG lens, full four shards.

That answers the item. One tier note: #87 changes `.github/workflows/ot61-mozjs-generate.yml`, and
`.github/workflows/` is a `CORE_PATHS` entry (`tools/xtask/src/main.rs:11628`), so `cargo xtask gate-plan` will
put #87 in **full-alone** (four shards), not the "no QEMU gate" in the #87 cell (THINK:228). Correct the cell to
the full tier, or cite the PR's own full run if it already exists. This is a gate-tier detail, not a missing gate,
so the item passes as written.

## What holds

- The dtoa licence resolution, the intgemm flag, and the GPL-only cut-list are correct.
- T1 now matches §6.1 option (i) and #86, and its discriminator is strong enough.
- The `gCanUseExtraThreads = false` enforcement is the right mechanism, and the CP table is the right shape.
- The §7 gate table is complete in structure; only the #87 tier cell needs the full-alone correction.

## Recommendation to the PM

Bounce this THINK for one docs-only fix before the vendor/build continues:

1. **§3.1/§3.2:** replace "WASI first / no-op threads / today's `neo-crt`" at THINK:111,115-116,121 with the
   POSIX backend + `gCanUseExtraThreads = false` wording already at THINK:168-175, and reconcile the
   C3c-1/C3c-2 dependency with C3b (landed) and #88 (the POSIX gap).
2. **§2.2 note:** add the two LGPL-2.1 PGO inputs to #87's cut-list or flag them, so the row's
   "GPL/LGPL … not vendored" is true.
3. **§7 #87:** change "no QEMU gate" to full-alone, because the workflow path is a core path.

No source, THINK, or other file changed; this commit is the RETURN only. The pre-existing untracked
`pr59-review/` directory is not staged and is not part of this branch.

## Verification performed

- `git fetch origin`; `origin/main` = `d5576597d`. Read the dispatch at
  `origin/main:Plans/DISPATCH-2026-11-08-relays-355-358.md`, then `git show 65da764c` and
  `git diff c2d5f3ccb..65da764c -- docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md` (one file, `+41/−8`).
- Read the whole THINK at `65da764c`; cited lines are the reviewed head's lines.
- Read `CLAUDE.md` on `origin/main` at `:917` and its commit `da68b78f2`.
- Used the existing `servo/mozjs` checkout at `138c9470151b8dcbb335f249cb83f9404857baf9`: read
  `js/src/dtoa.c:1-19`, `js/src/util/DoubleToString.cpp:70`, `js/src/util/moz.build:34-37`,
  `js/moz.configure:962-984`, `js/src/threading/moz.build`, `js/src/vm/Runtime.cpp:63-67,176`,
  `js/src/vm/InternalThreadPool.cpp`, `js/src/vm/JSContext.cpp`, and `js/src/vm/HelperThreads.cpp`.
- Read #87 at `origin/claude/ot61-c3c1-mozjs-vendor` (`544cd2410`): its three patches (0001/0002/0003),
  `.github/workflows/ot61-mozjs-generate.yml:60-90,110-160,197-300`, and `VENDORED.md:115-135`. Confirmed the
  `--disable-wasm-moz-intgemm` lines, the absent `gemmology`/`xsimd` directories, and the cut-list paths.
  `git grep` over the #87 vendor tree for GPL/LGPL grant text found the two PGO HTML files above and no
  unlisted GPL-only file; the dual BSD/GPLv2 `vtune` files and the autoconf-exception scripts are documented
  in `VENDORED.md`.
- Read #86 at `refs/pr/86` (`0b650a4d5`): `userland/neo-crt/src/lib.rs:250-286` and
  `userland/cxx-runtime-smoke/c/crt_checks.c:288-312`.
- Read #88 at `refs/pr/88` (`849d23250`) for the POSIX-gap/thread-name context; C3b's landing on `main` is
  `f19aeb9a4` / `abbd20b44` (`userland/neo-crt/src/pthread.rs`).
- Read `tools/xtask/src/main.rs:11595-11629` (`CORE_PATHS`, including `.github/workflows/`) and the
  gate-plan classifier at `:11891-11934`.
- Did not run `configure`, `cargo`, or QEMU: this is a design/docs re-check. RAG was not available in this
  worker (`http://127.0.0.1:8404/health` refused); direct reads and `grep` supplied the evidence.
