<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 347: #86 (`CLOCK_REALTIME` counted from 1970)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §347.

**Reviewed:** PR #86 head `0b650a4d58300ac1980544247107e29c3eda2f09` ("OT-61 C3c-1 (T1): neo-crt answers
CLOCK_REALTIME, counted from 1970"), one commit on merge-base `103e53481`; `origin/main` at review time is
`e6b3d665c`. The diff against `origin/main` is four files, +37/−13. The PM's decision is C3c THINK §6.1 option (i)
(`claude/ot61-c3c-spidermonkey-think` @ `d7365db41`).

**Overall: PASS.** All four items. No blocking finding. Three non-blocking notes at the end.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs (at `0b650a4d`) |
|---|---|---|---|
| 1 | `CLOCK_REALTIME` returns the process-relative count read from 1970: it advances, and it never claims a real date | **PASS** | `userland/neo-crt/src/lib.rs:255,261-269,274,278-283`; `userland/cxx-runtime-smoke/c/crt_checks.c:303-307` |
| 2 | `CLOCK_MONOTONIC` is unchanged, including `main`'s `monotonic_epoch()` structure that the conflict touched | **PASS** | `userland/neo-crt/src/lib.rs:250-253,256,274-285`; no diff in `monotonic_epoch` against `origin/main` |
| 3 | `crt_checks.c` would fail on the old code (realtime aborting or not advancing) | **PASS** | `crt_checks.c:303` vs old `origin/main:userland/neo-crt/src/lib.rs:262-264`; `crt_checks.c:304-307` |
| 4 | std's `SystemTime::now()` is untouched | **PASS** | `git diff --name-only origin/main...refs/pr/86`: four files, none under the std overlay/PAL; `userland/neoplutarch-std-overlay/std/time_neoplutarch.rs:7-10` |

## 1. `CLOCK_REALTIME`: process-relative, advancing, never a real date

`CLOCK_REALTIME` is defined as 0 at `userland/neo-crt/src/lib.rs:255`, and the guard at `:274` now accepts it as
well as `CLOCK_MONOTONIC`. Both ids then take the same path: `let d = monotonic_epoch().elapsed()` at `:278`, with
`tv_sec`/`tv_nsec` written from `d` at `:280-283`. `monotonic_epoch()` is the first `Instant::now` in the process
(`:250-253`), so the value is the process-relative elapsed count, read as seconds since 1970-01-01 as the doc block
at `:261-269` states.

It advances because `Instant::elapsed()` is monotonic and the nanoseconds are written at full resolution (the
seconds roll over normally). It never claims a real date: the code reads no RTC, no kernel `CLOCK_REALTIME`, and no
wall-clock source at all; the base is zero, not the kernel's realtime. That is option (i) exactly: a caller that
needs a realtime clock keeps running and sees time move, while every date it shows is 1970 plus process time. The
smoke pins the same properties at `crt_checks.c:303-307`: the call succeeds, realtime is not behind the earlier
monotonic read, and it is in 1970's first hour.

## 2. `CLOCK_MONOTONIC` and `monotonic_epoch()` are unchanged

`CLOCK_MONOTONIC` is still 1 (`userland/neo-crt/src/lib.rs:256`). `monotonic_epoch()` is still main's
`OnceLock<Instant>`:

```
250  pub(crate) fn monotonic_epoch() -> Instant {
251      static EPOCH: OnceLock<Instant> = OnceLock::new();
252      *EPOCH.get_or_init(Instant::now)
253  }
```

`git diff origin/main...refs/pr/86 -- userland/neo-crt/src/lib.rs` has no hunk inside that function: the conflict
resolution kept main's structure. The only change to the monotonic path is the guard at `:274` now also accepting
0; `:278-285` has no branch on the clock id, so a `CLOCK_MONOTONIC` read still computes
`monotonic_epoch().elapsed()` and writes the same `Timespec` fields. A `CLOCK_REALTIME`-first process now initialises
the shared epoch at that first clock read (`:258` documents this), which is the "same value" the decision asks for
and leaves the monotonic guarantee—nondecreasing elapsed time—intact.

## 3. The new check fails on the old code

The old code at `origin/main:userland/neo-crt/src/lib.rs:262-264` rejects every id except `CLOCK_MONOTONIC`:

```
if clock != CLOCK_MONOTONIC || ts.is_null() {
    set_errno(EINVAL);
    return -1;
}
```

The new check at `crt_checks.c:303` requires `clock_gettime(CLOCK_REALTIME, &r) == 0`. On that old code the call
returns −1 with `EINVAL`, the `CHECK` macro at `crt_checks.c:59-60` returns item 64, and the smoke fails item c64
(`userland/cxx-runtime-smoke/src/main.rs:110-115`). The two following checks are additional discriminators for a
partial fix: `:304-305` rejects a realtime value behind the earlier monotonic read (a non-advancing/stale clock),
and `:306-307` rejects a real date (seconds ≥ 3600). The old code's abort is enough to fail it.

## 4. std's `SystemTime::now()` is untouched

The PR changes four files — `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md`,
`userland/cxx-runtime-smoke/c/crt_checks.c`, `userland/cxx-runtime-smoke/src/main.rs`, and
`userland/neo-crt/src/lib.rs` — and none under `userland/neoplutarch-std-overlay/` or `userland/neo-std-pal/`.
`userland/neoplutarch-std-overlay/std/time_neoplutarch.rs:7-10` still says `SystemTime` stays std's `unsupported`
one and its `now()` panics; `:18-24` still implements only `Instant::now()` through `rt::monotonic_ns()`. No
`SystemTime::now()` path was added or changed. The `std::chrono::system_clock` call the PR's doc names is libc++'s
C++ clock over libc `clock_gettime`, not std's `SystemTime`.

## Non-blocking notes

1. **The advance oracle is indirect.** `crt_checks.c:303-305` reads realtime once and compares it to an earlier
   monotonic read; it would not catch a realtime clock frozen at exactly that earlier reading (that value passes
   both `:305` and `:307`). The product code's advance comes from `elapsed()` at `lib.rs:278`, and the old code is
   caught at `:303`, so this is a test-strength note, not a defect. A second realtime read after a short delay
   would make "it advances" explicit.
2. **The 1970 test window assumes a short-lived smoke.** `crt_checks.c:306-307` requires `tv_sec < 3600`, which the
   comment above the function (`:290-292`) states as "this process has run for under an hour". The date is still
   correct after an hour (it remains 1970 plus elapsed time), so a smoke delayed past an hour would fail the test,
   not the implementation. Fine for this gate; worth knowing if the timeout budget ever grows.
3. **Two comments still say the bundle has no `CLOCK_REALTIME`.** `userland/neo-crt/src/pthread.rs:432,559-561`
   and `userland/neo-crt/cxx/__config_site:19-21` were written when the C library refused the clock. The pthread
   timed-wait behaviour is intentionally still monotonic-only (and `pthread_checks.c:138` still pins the default
   realtime timed wait at `EINVAL`), but the "there is none" wording is now stale: libc answers it. Comment cleanup
   only; no behaviour change requested in this contained PR.

## Verification performed

- Fetched `refs/pull/86/head` and confirmed the reviewed head is `0b650a4d58300ac1980544247107e29c3eda2f09`;
  one commit from merge-base `103e53481`.
- `git diff --stat/--name-only origin/main...refs/pr/86`: four files, +37/−13; no unrelated hunks. `git diff
  --check` is clean.
- Read the full PR diff and the old `origin/main` versions of `lib.rs` and `crt_checks.c`; read the complete
  `clock_gettime`, `monotonic_epoch`, smoke-check and `c_check` paths.
- Read the C3c THINK §6.1 decision at `d7365db41` to confirm option (i) is the one this build implements.
- Did not run the QEMU `cxx_runtime_smoke`; this review is the static read. The PR's own gate remains its evidence.
