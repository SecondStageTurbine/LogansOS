<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 356: #88 narrow re-check (thread names, the canary, the head gate)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-355-358.md` §356, its three checklist items.
**Reviewed:** PR #88 head `849d232506ab41ca82c1a0de5393a0e6004e453d` (`OT-61 C3c-1: thread names are stored, not ignored`). My relay-351 head `1d77e6cb` was re-based as `272f4b9ff` (byte-identical for the files that review covered); `341df2c3` and `849d2325` are the two fix commits on top. Own diff against base `65af2ff45184af05122a028869c7953904038f97`: 54 files, +2493/−14, including `third_party/ot61-crt` (41 vendored musl files) from the earlier heads. `origin/main` when the review began was `d5576597d1301b2d66cee938bcbd542118788abe`; it moved three commits during the review (to `ef8e1a51eb78c9ca2bc4f1c1826afe1c6bb4c5d9`, which includes the private #264 re-stack), so this RETURN branch is rebased onto `ef8e1a51e` at push. PR #88 itself still has base `65af2ff45` and is not rebased on that new main; item 3 below is its own head's gate at `849d2325`, not a rebased tree.
This is a narrow re-check of the three §356 items only; the item-level findings of relay 351 that already passed are not re-reviewed, apart from confirming the two fix commits did not disturb them.

**Overall: PASS.** The thread name is now stored, read back, and exercised by a required smoke check; the canary is still seeded before anything C or C++ runs in the program that calls it; and the head's own 4-shard PR gate is green.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| 1 | `pthread_setname_np` stores the name (≤ 15 bytes plus NUL, truncated), `pthread_getname_np` returns it, and a check covers the round trip | **PASS** | `userland/neo-crt/src/pthread.rs:103-110,165-169,227-231,237-273`; `userland/cxx-runtime-smoke/c/crt_posix_checks.c:60-72`; `userland/cxx-runtime-smoke/src/main.rs:332-336`; `tools/xtask/src/main.rs:19568` |
| 2 | The stack-canary change (`341df2c3`, seeded explicitly rather than in init) still seeds before any protected function runs | **PASS** | `userland/cxx-runtime-smoke/src/main.rs:203-206`; `userland/neo-crt/src/lib.rs:456-470,472-490`; `userland/neo-crt/c/neo_posix.c:105-113`; `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:303-310` (ordering caveat below) |
| 3 | The head's own 4-shard PR run is green | **PASS** | PR `CI` run [`42525174819`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42525174819) at `849d2325`: all four shards and every host job `success`, run `success` at 23:11:24Z |

## 1. Thread names — PASS

`PThread` gained `name: Mutex<[u8; NAME_LEN]>` with `NAME_LEN = 16` (15 bytes plus the NUL), initialised empty in both record constructors — `pthread_create` (`pthread.rs:165-169`) and the lazy `pthread_self` record (`:227-231`).

- **Store.** `pthread_setname_np` (`:243-254`) validates the two pointers, takes the C string as bytes, cuts the length with `min(NAME_LEN - 1)`, zeroes the record's 16 bytes, copies the first `n` bytes, and returns `0`. A 20-byte name therefore stores exactly the first 15 bytes; it does not overrun the `[u8; 16]`, and the NUL is always present. This is the truncation the dispatch asks for, and the reason for it (SpiderMonkey's `MOZ_RELEASE_ASSERT(!rv)` in `PosixThread.cpp`) is recorded in the code comment (`:237-242`) and in THINK §10 (`docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:298-302`).
- **Read back.** `pthread_getname_np` (`:259-273`) copies the record's name under its mutex, finds the first NUL (`position`, falling back to the last byte), returns `ERANGE` when `len < n + 1`, otherwise copies `n` bytes and appends the NUL. A never-named thread reads `""`. `ERANGE` is the local `34`, musl's value (`:110`).
- **Check covers the round trip.** The smoke's POSIX check runs on the calling thread (`crt_posix_checks.c:62-72`): item 108 sets `"js-helper"` and requires `getname` to return it; item 109 sets a 20-byte name and requires the 15-byte cut `"abcdefghijklmno"`; item 110 requires `ERANGE` for a 4-byte buffer. A failure returns the item number from `crt_check_posix` and fails smoke item 31, whose required output line was updated to include `thread names` (`src/main.rs:332-336`, `tools/xtask/src/main.rs:19568`). The head gate passing item 31 therefore means 108-110 all returned 0.

## 2. The canary is seeded before protected code runs — PASS, with the intended ordering caveat

`341df2c3` moved the seeding out of `neo_crt::init()` into `seed_stack_canary()` (`lib.rs:456-470`): it draws a `usize` from `arc4random_buf` (the kernel CSPRNG through the std overlay), masks it with `& !0xff`, and writes `__stack_chk_guard`. `init()` (`:472-490`) now only walks `.init_array`; it no longer asks for entropy, which is what stopped `swgl_smoke` aborting on a kernel without `kernel-rng`.

The ordering holds in the program that calls it: `cxx-runtime-smoke`'s `_start` calls `neo_crt::seed_stack_canary()` as its first statement (`src/main.rs:205`), before `neo_crt::init()` on the next line (`:206`), and `init` is what runs the C/C++ static constructors. So no constructor or other C/C++ frame exists before the write. The canary is written before any protected function can run, and THINK §10 records the contract for the next consumer (`docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:303-310`): a program with stack-protected code calls it first, before `init()`.

The caveat is that **no in-tree program currently compiles with the protector**, so the ordering is proven by construction and by the call site, not by a protected frame in this gate: `neo-crt`'s C, the smoke's C/C++, and SWGL's C++ are all built with `-fno-stack-protector` (`userland/neo-crt/build.rs:317`, `userland/cxx-runtime-smoke/build.rs:33,55`, `tools/xtask/src/main.rs:8927`). The smoke's item 107 checks the canary's value shape (`crt_posix_checks.c:57-59`), and item 31 runs that check; neither proves a protected frame used it. That matches the PM's change: the canary exists for SpiderMonkey's `-fstack-protector-strong` bundle, which is documented to call `seed_stack_canary()` first, and the Rust target itself has no stack protector. The other in-tree neo-crt users, `cxx-hardening-trap` (`src/trap.rs:23`) and `swgl-smoke` (`userland/swgl-smoke/src/main.rs:140`), call `init()` without seeding, but neither has protected C/C++ under the flags above, so neither runs a protected function with the fixed default. No fail.

## 3. The head's own 4-shard PR run is green — PASS

Run [`42525174819`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42525174819) is the `pull_request` `CI` run for `849d2325` (created 21:26:00Z; run `success`, shard 2/4's completion at 23:11:24Z was the last job). Job results from the run's API record:

| Job | Result |
|---|---|
| `Nightly Skip Check` | success |
| `Host Tests, Format & Lint` | success |
| `Build Checks (product build, feature clippy, SPDX, identity)` | success |
| `Production RNG seed boot (no neo-test)` | success |
| `Product kernel boot, WHPX XSAVE CPUID (no neo-test)` | success |
| `Decoder Fuzz (inflate-zlib, inflate-raw, png, jpeg)` | success |
| `QEMU Integration Tests (shard 1/4)` | success |
| `QEMU Integration Tests (shard 2/4)` | success |
| `QEMU Integration Tests (shard 3/4)` | success |
| `QEMU Integration Tests (shard 4/4)` | success |

`Batch report` is `skipped` (this is not a batch run). As corroboration, the `workflow_dispatch` full-tier run on the same head, [`42525170759`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42525170759), is also `success` with all four shards green (21:25:57Z–22:46:22Z). The previous head's two red shards (`42516902770`/`42516949510`, shards 3/4 and 4/4) were the failures `341df2c3` names — the `swgl_smoke` abort and the padded `memcmp` in smoke item 101 — and neither is red here.

## Verification performed

- `git fetch origin +pull/88/head:refs/pr/88`; `refs/pr/88` is `849d2325`; `git merge-base origin/main 849d2325` is `65af2ff45`; `git diff --shortstat` on that range is 54 files, +2493/−14.
- Read both fix commits in full (`341df2c3`, `849d2325`) and the current `pthread.rs`, `lib.rs`, `crt_posix_checks.c`, smoke `_start`, and xtask's required-line list at the head.
- Confirmed the smoke's item 31 calls `crt_check_posix` (`src/main.rs:332-336`), that its required output line includes `thread names` (`tools/xtask/src/main.rs:19568`), and that a failed `CHECK` returns its number so item 31 fails rather than passing.
- Confirmed the protector flags: `-fno-stack-protector` in `userland/neo-crt/build.rs:317`, `userland/cxx-runtime-smoke/build.rs:33,55`, and `tools/xtask/src/main.rs:8927`; the Rust target spec has no protector setting.
- Queried the GitHub Actions API for the head's runs and job conclusions; did not rely on the earlier review's run citations.
- RAG health `http://127.0.0.1:8404/health` is down in this session; I used `git show`/`git grep`/`read` (not a blocker).
- Working tree: this RETURN only on `dexter/pr88-recheck`; the pre-existing untracked `pr59-review/` directory is untouched.

## Recommendation to the PM

**PASS.** All three §356 items pass at `849d2325`. The narrow relay-351 FAIL is closed by a real store (not a success-without-work stub), the canary move keeps the seeding ahead of the constructors in the smoke and is documented for SpiderMonkey, and the PR's own gate is green. The PR is ready for the PM's merge check on this head.
