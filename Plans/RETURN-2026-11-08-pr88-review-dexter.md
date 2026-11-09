<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 351: #88 (neo-crt for SpiderMonkey: libc++ localization, the POSIX gap, the stack protector)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §351, its six checklist items.
**Reviewed:** PR #88 head `1d77e6cb3cb22e680b12a8cf51f83cc57fe9a355` (`claude/ot61-c3a-localization`), own diff against its base `537aa9bbd74992fcebe4204a217197c6814c7ea1`: 54 files, +2414/−14. 41 files are under `third_party/ot61-crt` (`VENDORED.md` plus 40 musl `src/time/` files). `origin/main` at review is `d0dd0136160f3bb88683c92aed74d193f614ea17`; #88 is not rebased on it.
**Overall: FAIL (narrow), #88.2 only.** Licences, the C-locale design, the stack-protector seeding/failure path and the opt-in/default-build claim hold. The one failing item is a new stub that returns success without doing the work: `pthread_setname_np` returns `0` and ignores the name. It is documented in THINK §10:298, but §351.2 accepts only an implementation or a stub that fails safely — and an error return here would trip SpiderMonkey's own `MOZ_RELEASE_ASSERT(!rv)`, so this needs a PM decision, not a silent pass. Separately, the head's own QEMU gate is red (shards 3/4 and 4/4), so this is not merge evidence even after the narrow fix. Details and line references below.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| 1 | Every newly vendored file under `third_party/ot61-crt` has its licence recorded, on CLAUDE.md's allowed list | **PASS** (per source: musl pass; libc++ pass) | `third_party/ot61-crt/VENDORED.md:10-13,17-24`; `CLAUDE.md:915-918`; `vendor/musl/COPYRIGHT`; `vendor/libcxx/LICENSE.TXT` |
| 2 | Each of the 47 link-gap symbols is implemented or a documented stub that fails safely; name any success-without-work stub | **FAIL** (narrow, one stub) | `userland/neo-crt/src/pthread.rs:228-233`; `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:298`; `third_party/ot61-mozjs/.../PosixThread.cpp:110-119` (#87) |
| 3 | `__stack_chk_guard` seeded from the kernel RNG before any protected function runs; `__stack_chk_fail` stops the process | **PASS** | `userland/neo-crt/src/lib.rs:459-481`; `userland/neoplutarch-std-overlay/std/random_neoplutarch.rs:20-31`; `userland/neo-crt/c/neo_posix.c:105-118`; `userland/cxx-runtime-smoke/src/main.rs:203-205`; `userland/cxx-runtime-smoke/src/trap.rs:22-23` |
| 4 | Localization stays in the "C" locale: no locale files are read, nothing fetches | **PASS** | `userland/neo-crt/cxx/__config_site:26-33`; `userland/neo-crt/build.rs:75-92`; `userland/neo-crt/src/lib.rs:390-394`; `userland/neo-crt/c/neo_bottom.c:127-142`; `c/neo_posix.c:52-58` |
| 5 | The smoke checks would fail on the old code for each new piece | **PASS** for the three pieces (coverage note 3) | `tools/xtask/src/main.rs:19465-19466`; `userland/cxx-runtime-smoke/c/crt_posix_checks.c:26-53`; `c/cxx_stream_checks.cpp:26-64` |
| 6 | Off by default; other `neo-crt` users build unchanged | **PASS** (one scope note 4) | `Cargo.toml:84-96`; `userland/neo-crt/Cargo.toml:12-21`; `tools/xtask/src/main.rs:8901-8913`; CI job `neo-crt` and `SWGL S1 smoke` at head |

## #88.1 PASS — every new vendored file is covered by a recorded, allowed licence

The 41 files under `third_party/ot61-crt` in this diff are `VENDORED.md` plus **40 newly added musl `src/time/` files**. There are no other new vendor files; the seven libc++ sources this PR starts compiling (`ios.cpp`, `ios.instantiations.cpp`, `iostream.cpp`, `locale.cpp`, `ostream.cpp`, `regex.cpp`, `strstream.cpp`) were already vendored on `main`.

- **musl 1.2.5 — PASS.** `VENDORED.md:10-12` records the source, the tag pin `v1.2.5 = 0784374d561435f7c787a555aeab8ede699ed298`, the MIT licence and `vendor/musl/COPYRIGHT`. `VENDORED.md:20-24` explicitly records that `src/time/` was added for C3c-1 from the same tag. MIT is on CLAUDE.md's list (`CLAUDE.md:915-918`). I cloned `github.com/kraj/musl` at `v1.2.5` and ran `diff -r /tmp/musl-v1.2.5/src/time <PR>/third_party/ot61-crt/vendor/musl/src/time`: **no differences**; all 40 files are byte-identical to the tag.
- **libc++ 18.1.8 — PASS.** `VENDORED.md:13` records Apache-2.0 WITH LLVM-exception and `vendor/libcxx/LICENSE.TXT`; the dispatch says that licence is fine. The seven files newly added to `LIBCXX_SOURCES` (`userland/neo-crt/build.rs:251-259`) are already in the pinned tree, so this PR adds no new libc++ file.
- **First-party files.** `c/neo_posix.c` is `MPL-2.0` (`:1`); `c/stdin.c` is `MIT` and names musl's `stdin.c` plus the MIT COPYRIGHT (`:1-9`). No GPL/LGPL text is linked or copied.

**Per source: musl PASS; libc++ PASS; first-party PASS.**

## #88.2 FAIL (narrow) — `pthread_setname_np` is a documented success-without-work stub

The probe's literal 47-line output is not committed to the repo; the split comes from the PR body and commit `1d77e6cb` (20 C-ABI symbols closed, 27 Rust-crate symbols left to the product link) and from run [`42516423240`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42516423240). The C-ABI symbols this PR closes are either implemented or honest refusals:

- **Time:** musl's `gmtime`/`gmtime_r`/`timegm` (`build.rs:101-103`); `localtime`/`localtime_r`/`mktime` are UTC (`c/neo_posix.c:36-50`); `time()` reads `CLOCK_REALTIME` and returns `-1` until #86 (`:28-34`).
- **Files:** `fopen`/`remove` return `ENOENT`; `fstat`/`fcntl`/`close` return `EBADF`; `memfd_create` returns `ENOSYS` (`c/neo_posix.c:53-103`). `fclose` flushes a permanent standard stream (`:70-73`).
- **Stdio:** musl's `ferror`, `fileno`, `setvbuf`, `perror`, `fread` are compiled (`build.rs:104-109`); `stdin` is always locked and every read is EOF (`c/stdin.c`).
- **`getpid`:** `std::process::id()` (`src/lib.rs:449-454`).
- **`pthread_setname_np`:** `userland/neo-crt/src/pthread.rs:228-233` takes the thread and name and returns `0` without writing anything. THINK §10:298 calls it "accepted and ignored".

That last one is the named exception §351.2 asks for. Whether or not the probe counted it among the 20 C names, it is a new C-ABI symbol this PR adds for the same consumer. It is not an implementation, and it is not a stub that fails safely by the item's definition: it returns success while doing nothing. It is at least **documented** in the THINK and in the code, so it is not silent in the source; but a caller sees `0` and must assume the name was set.

**Do not simply change it to an error return.** SpiderMonkey calls it here:

`third_party/ot61-mozjs/vendor/mozjs-138c9470/mozjs-sys/mozjs/js/src/threading/posix/PosixThread.cpp:110-119` (from #87)

and ends the function with `MOZ_RELEASE_ASSERT(!rv)`. An `ENOSYS`/`EINVAL` return would abort a SpiderMonkey process that tries to name a thread. The real choices are (a) implement a name store (the target has no TLS ABI, but a process/thread-record name would satisfy the call), or (b) record the accepted no-op as an explicit exception and have the PM waive this one. The current code is an honest, safe no-op for the consumer; it is not the "error return" the checklist demands, so this item fails narrowly.

The **27 remaining link-gap symbols** are not neo-crt's: the PR body and run [`42516423240`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42516423240) name them as the Rust crates the product link supplies — zlib, `encoding_c_mem`, `normalizer_glue`, and `mozjs-sys`'s hook. They are not POSIX libc functions and this PR does not claim to define them.

**Other success-without-work stubs, for completeness (not introduced by this PR):** `atexit` and `__cxa_atexit` return `0` and never run their handlers (`src/lib.rs:421-437`); THINK §6 item 2 records that as a C3a known gap. `__munmap` returns `0` without unmapping (`c/neo_bottom.c:127-133`), but it is unreachable because `__map_file` always returns `NULL` (`:135-142`) and `getenv` is always `NULL` (`src/lib.rs:390-394`). Neither is one of the 47.

## #88.3 PASS — the canary is seeded before any C code, and failure aborts

`neo_crt::init()` (`src/lib.rs:459-481`) is the first thing both smoke entry points call (`userland/cxx-runtime-smoke/src/main.rs:203-205`, `userland/cxx-runtime-smoke/src/trap.rs:22-23`). It seeds the canary **before** walking `.init_array`:

- `arc4random_buf((&raw mut g).cast(), size_of::<usize>())` (`lib.rs:467-470`) goes to `DefaultRandomSource` (`src/lib.rs:224-236`), whose neoplutarch overlay is `SYS_GETRANDOM` on the kernel CSPRNG (`userland/neoplutarch-std-overlay/std/random_neoplutarch.rs:20-31`). This is the kernel's RNG, not a userspace PRNG and not a constant.
- The value is masked with `g & !0xff` (`lib.rs:470`), so the low byte is zero; it is written to the C global before any `.init_array` constructor runs (`:472-481`) and before threads start.
- The C side has `uintptr_t __stack_chk_guard = 0x595e9fbd94fda700;` and `_Noreturn void __stack_chk_fail(void)` at `c/neo_posix.c:105-118`; the failure path writes `neo-crt: stack smashing detected`, flushes, and calls `abort()`. That `abort` is `std::process::abort()` (`src/lib.rs:383-386`), which the std overlay routes to `SYS_EXIT(134)` (`userland/neoplutarch-std-overlay/std/neoplutarch_rt.rs:127-132`). It stops the process.
- Smoke item 31 checks that the global differs from the fixed default, has low byte zero and is nonzero (`c/crt_posix_checks.c:50-52`). A 64-bit `movq __stack_chk_guard(%rip)` is visible in the built smoke; clang uses the global canary for `x86_64-unknown-none-elf` because the target has no TLS ABI.

The one caveat is the fixed default: a program that never calls `init()` would run with a known canary. That is the documented contract (`src/lib.rs:456-458`), and both smoke entry points and the C3a design call `init()` first. **PASS.**

## #88.4 PASS — localization cannot read a locale file, and nothing fetches

`cxx/__config_site:26-33` drops `_LIBCPP_HAS_NO_LOCALIZATION` but keeps `_LIBCPP_HAS_NO_FILESYSTEM`, `_LIBCPP_HAS_NO_RANDOM_DEVICE` and `_LIBCPP_HAS_NO_TIME_ZONE_DATABASE`. The locale implementation that musl compiles is `locale_map.c` + `newlocale.c` + `setlocale.c` (`build.rs:75-92`). In `__get_locale` the only route to a file is `getenv("MUSL_LOCPATH")`; neo-crt's `getenv` always returns `NULL` (`src/lib.rs:390-394`), so `path` stays null and the `for (path)` block is not entered. Even if it were reached, `__map_file` returns `NULL` without opening anything (`c/neo_bottom.c:135-142`), and `fopen` returns `NULL`/`ENOENT` (`c/neo_posix.c:52-58`). There is no network code in this diff. The C locale's name is checked at `c/cxx_stream_checks.cpp:42`; `put_time`'s `%Z` is `"UTC"` via `__tm_to_tzname` (`c/neo_bottom.c:144-151`) and checked at `cxx_stream_checks.cpp:44-55`. **PASS.**

## #88.5 PASS (coverage note) — the two new checks are real discriminators, but not every new symbol is exercised

The xtask harness requires the exact new lines:

- `tools/xtask/src/main.rs:19465` — `ok(30) C++: ostringstream/istringstream, the C locale, put_time, regex, cin at EOF`
- `:19466` — `ok(31) POSIX for SpiderMonkey: UTC time, no files, fileno/ferror, getpid, stack canary`

On the base `537aa9bb`, `crt_posix_checks.c` does not compile/link because `gmtime_r`, `localtime_r`, `mktime`, `strftime`, `fopen`, `fileno`, `ferror`, `getpid` and `__stack_chk_guard` are absent; `cxx_stream_checks.cpp` does not compile because the old `_LIBCPP_HAS_NO_LOCALIZATION`/source list has no `<sstream>`/`<locale>` implementation. So the two new checks fail on the old code as a whole, and the old output cannot satisfy the new exact-line requirements.

**Coverage note.** Item 31 exercises only the time helpers, `fopen`, `fileno`/`ferror`, `getpid` and the canary value; it does not exercise `remove`, `fstat`, `fcntl`, `close`, `memfd_create`, `fclose`, `setvbuf`, `perror`, `fread`, `time()`, `pthread_setname_np`, or the `__stack_chk_fail` path. A regression in those stubs would not fail this smoke. That does not contradict "the smoke would fail on the old code" for the three pieces, but it is not per-symbol coverage.

## #88.6 PASS (scope note) — default build unchanged; the other C++ user still builds

`userland/neo-crt` is not a root-workspace member and is explicitly excluded (`Cargo.toml:84-96`); `cxx-runtime-smoke` is built only by `cargo xtask ot61-crt-build` / the `cxx_runtime_smoke` scenario (`tools/xtask/src/main.rs:8901-8913`, `:2438-2439`). The default kernel, image and userland do not read this tree; the only xtask change is the scenario's required-line list (`:19465-19466`), which is test-only. The head CI host job `neo-crt C/C++ runtime (x86_64-unknown-neoplutarch)` and `SWGL S1 smoke over neo-crt` both succeeded (run [`42516900732`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42516900732)), so the other neo-crt user builds.

Scope note: localization is **not** a per-consumer feature flag. Dropping `_LIBCPP_HAS_NO_LOCALIZATION` and adding seven libc++ sources changes every C++ neo-crt link, which is the cost THINK §10:277-278 records ("about 7 more libc++ objects and 23 musl objects in every C++-using bundle"). The default product is unchanged because it does not link neo-crt; a C++ user does get the larger runtime. If "off by default" was meant to require a per-bundle opt-in, that is not what the code does.

## Verification performed

- `git fetch origin +pull/88/head:refs/pr/88`; head `1d77e6cb` matches the dispatch; base `537aa9bb`; `git merge-base` confirms the diff is the PR's own two commits.
- `git diff --stat 537aa9bb...1d77e6cb`: 54 files, +2414/−14; every changed first-party file read, plus all 40 new musl files' source context.
- Cloned musl `v1.2.5` at `0784374d` and `diff -r`'d `src/time` against the PR tree: identical.
- Built the head locally with `cargo xtask ot61-crt-build` (clang 18.1.3, host): `neo-crt` and `cxx-runtime-smoke` compiled. The built ELF has exactly two `syscall` instructions, one in `neo_std_pal::syscall4` and one in `std::sys::neoplutarch_rt::syscall4`, and one `%fs:` read in `std::sys::neoplutarch_rt::tls::tcb`; the canary is a `movq __stack_chk_guard(%rip)` reference. (My A4 invocation used a shared `CARGO_TARGET_DIR`, so xtask looked for the ELF under the worktree; the head CI's `neo-crt` job is the authoritative A4 evidence and is green.)
- Head check-runs: `Build Checks`, `Host Tests, Format & Lint`, `Decoder Fuzz`, `Production RNG seed boot`, `Product kernel boot` all success; `neo-crt` and `SWGL S1` host jobs success.
- **Head QEMU gate is red.** At the time of writing, PR `CI` run [`42516902770`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42516902770) and its rerun [`42516949510`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42516949510) both have **shards 3/4 and 4/4 failed**; shards 1/2 were still in progress. The Actions log endpoint returns 403 to this session, so I could not read the failing scenario from the serial log; the PM should get it or rerun before merge. Note that `gate-plan` at this head is `full-alone` (because `tools/xtask/src/main.rs` is a core path), not the "quick tier" the PR body calls it, so the 4-shard run is the relevant one.
- RAG health: `http://127.0.0.1:8404/health` is down in this session; I used `git show`/`grep`/`read` (not a blocker).
- Working tree: this RETURN only on `dexter/pr88-review`; the pre-existing untracked `pr59-review/` directory is untouched.

## Recommendation to the PM

**FAIL (narrow) — #88.2.** Fix or explicitly waive `pthread_setname_np`. The safe implementation is a name store on the thread record; the cheapest accepted alternative is a PM-recorded exception that the no-op `0` is deliberate because SpiderMonkey's `MOZ_RELEASE_ASSERT(!rv)` requires success, with THINK §10:298 saying so. If the PM waives it, the other five items pass.

Do **not** merge on the current head regardless: its own QEMU shards 3/4 and 4/4 are red, and that run is the PR's full-alone gate. Re-run the 4-shard gate after the #88.2 decision and before the PM's merge check; this RETURN is a review, not merge evidence.
