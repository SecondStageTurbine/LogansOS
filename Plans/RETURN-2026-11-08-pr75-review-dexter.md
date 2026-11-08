<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 344: #75 (key destructors at most `PTHREAD_DESTRUCTOR_ITERATIONS`)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §344, which runs `Plans/DISPATCH-2026-11-08-relays-315-321.md` §318 as written.
**Reviewed:** PR #75 head `efb63b9da280a349ca835c01ba04c075b6f2c530` (`claude/ot61-c3b-dtor-cap`), base/merge-base `13eed2c54caf76cbef78dad762467df68611546a` (the PR's `main` base). `origin/main` is now `018ddef0c`; nothing in `userland/neo-crt/src/pthread.rs` or `userland/cxx-runtime-smoke/c/pthread_checks.c` changed between the base and current `main`, so the PR diff is the whole change.
**Overall: PASS on all four checklist items.** The two exit paths now share one per-thread round count, a re-arming destructor runs exactly 4 times, the value left after round 4 is reclaimed with the TLS block and is not run a fifth time, the new p36 KAT pins 4 and would fail on the base code, and no other `pthread.rs` behaviour changed. Two non-blocking notes at the end (quick-tier coverage of the KAT; the leftover value is reclaimed by the `Slots` drop rather than explicitly zeroed).

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| 1 | One pass counter covers the wrapper's `run_key_destructors` and the TLS `Drop`; an always-re-arming destructor runs exactly 4 times | **PASS** | `pthread.rs:629-646`; `:659-667`; `:681-684`; `:757-770`; wrapper call `:151`; TLS teardown `thread_neoplutarch.rs:100-101` |
| 2 | A value left after the last pass is dropped, not leaked or re-run | **PASS** | `pthread.rs:639-646`, `:659-667`, `:671-679`; std key-layer `tls.rs:401-429`, `:433-436` |
| 3 | The KAT pins 4 and fails on the old code | **PASS** (not executed by the head's quick tier — note 1) | `pthread_checks.c:161-175`, `:198-203`; call at `main.rs:291-295`; base code `origin/main:userland/neo-crt/src/pthread.rs:642-647,735-741` |
| 4 | Nothing else in `pthread.rs` changed behaviour | **PASS** | diff is confined to the destructor section: `pthread.rs:629-646,659-667,682-684,757-801` |

## 1. PASS — one per-thread round count now covers both exit paths

`Slots` carries the count (`pthread.rs:629-635`), and `next_round` is the single gate: it returns `true` at most `DESTRUCTOR_ITERATIONS` (4) times per thread, then `false` (`:639-646`). Both paths go through it:

- `run_key_destructors`, the `pthread_create` wrapper's exit pass, counts in the thread's `Slots.rounds` before each round (`:757-770`, closure at `:760-763`).
- `Slots::drop`, the TLS teardown pass, destructures the same `rounds` field and calls `next_round(rounds)` for its own rounds (`:659-667`).
- The thread-local starts at `rounds: 0` (`:681-684`); it is per-thread because `Slots` is the `thread_local!`.

The ordering that makes the two paths meet is real: the wrapper calls `run_key_destructors()` at `pthread.rs:151` before the spawn closure returns; `thread_start` then calls `tls::thread_exit_cleanup()` (`userland/neoplutarch-std-overlay/std/thread_neoplutarch.rs:100-101`), which runs std's key destructors and drops the `SLOTS` thread-local (`tls.rs:401-429,433-436`), and only then `SYS_THREAD_EXIT` (`thread_neoplutarch.rs:102`). `pthread_join` waits for that exit, so the KAT observes both paths.

Trace of the always-re-arming destructor: rounds 1–4 each take the value, call the destructor, and the destructor re-arms it; the fifth `next_round` returns `false` at `pthread.rs:640-644`, so `run_key_destructors` stops with the value still set. `Slots::drop` then calls the same `next_round` with `rounds == 4` and runs no round, so no fifth call. Exactly 4 in all.

## 2. PASS — the leftover value is reclaimed with the TLS block, not run again

`take_value` clears the slot before the destructor is called (`pthread.rs:671-679`), so the fourth call sees the value and leaves the re-armed one. After that, `next_round` refuses the fifth round (`:639-646`) and `Slots::drop` does not take the slot (`:659-667`), so the value is not passed to a destructor again. The whole `Slots` is then dropped by std's TLS key layer: `run_destructors_on` clears the std key before calling the `SLOTS` destructor (`tls.rs:409-420`), and std's `destroy_value` drops the value box after that destructor returns (`rust-src:library/std/src/sys/thread_local/os.rs:222-243`); the box's `Drop` frees the allocation (`os.rs:139-150`). `values` is an inline array in `Slots` (`pthread.rs:630`), not a separate allocation, so it is reclaimed with that block. The pointer the C code stored is caller-owned and POSIX does not free it; no slot storage is leaked. "Dropped" here is the drop of `Slots` (and therefore of `values`), not an explicit zeroing of the slot — see note 2.

## 3. PASS — the KAT pins 4, and the base code gives 5

The new C case is `pthread_checks.c:161-175` (a re-arming destructor and a worker that sets the key) and `:198-203`: create the re-arming key, run `set_rearm` on a `pthread_create` thread, join, then `CHECK(36, ..., rearm_runs == 4)`. `pt_check_keys` is reached as C check 23 from `cxx-runtime-smoke` (`main.rs:291-295`); a nonzero C item fails the process and the harness exits nonzero, so p36 is a real discriminator, not a printed note.

On the base code (`origin/main:userland/neo-crt/src/pthread.rs`):

- `run_key_destructors` (`:735-741`) runs four rounds and the always-re-arming destructor runs four times;
- `Slots::drop` (`:642-647`) then runs its own `destructor_rounds`: round 1 takes the value left by round 4 and calls the destructor a **fifth** time; the destructor's `pthread_setspecific` fails because std's `SLOTS` key is already in its destructor (`destroy_value` sets the sentinel-1 "destructor running" value before dropping the value, so `SLOTS.try_with` returns `None`; `pthread.rs:747-750` turns that into `ENOMEM`), and round 2 has nothing to take;
- `rearm_runs` is therefore 5, and `rearm_runs == 4` fails. The new code returns 4 as item 1's trace shows.

Note 1: `cxx_runtime_smoke` is not in `SMOKE_SCENARIOS`, and the PR's added lines do not name the scenario, so the head's quick-tier CI (`42453384265`) did not execute p36; it will run in the full-tier batch, which is the merge evidence. The head's OT-61 closure build (`42453367062`) compiled the runtime and the C file green.

## 4. PASS — the change is confined to the destructor logic

The full diff against the PR base is two files, `userland/neo-crt/src/pthread.rs` (+69/−18) and `userland/cxx-runtime-smoke/c/pthread_checks.c` (+26). In `pthread.rs` the hunks are exactly:

- the `Slots.rounds` field (`:629-635`);
- `next_round` (`:639-646`);
- the `Drop` body (`:659-667`);
- the `SLOTS` initialiser's `rounds: 0` (`:681-684`);
- `run_key_destructors` (`:757-770`);
- `destructor_rounds`'s new `next_round` parameter and `while next_round()` loop (`:776-801`).

No other function in `pthread.rs` is touched: create/join/detach/self/equal, the mutex/cond/rwlock/once code and the key create/delete/get/set paths are byte-identical to the base. `destructor_rounds` still clears before the destructor, still stops after a round that ran none, and the only callers are the two exit paths. The one extra semantic detail is that an empty round now consumes one of the four counts; that cannot skip a pending destructor, because an empty round means no live value exists, and a value set after the cap is intentionally not run (the POSIX cap this PR is about).

## Verification performed

- `git fetch origin +pull/75/head:refs/pr/75`: head `efb63b9d` (matches the dispatch; PR base `13eed2c5`).
- `git diff --stat origin/main...refs/pr/75`: 2 files, +77/−18. Read the full diff and the head versions of `pthread.rs`, `pthread_checks.c`, `cxx-runtime-smoke/src/main.rs`, and the std TLS overlay files (`thread_neoplutarch.rs`, `tls.rs`).
- Confirmed the base-to-`origin/main` diff for the two touched files is empty (`git diff 13eed2c5..origin/main -- userland/neo-crt/src/pthread.rs userland/cxx-runtime-smoke/c/pthread_checks.c`), so the review applies to current `main`.
- Read the rust-src std key-layer implementation to confirm `try_with` returns `None` while a thread-local's destructor is running (`library/std/src/sys/thread_local/os.rs:222-243`, sentinel set at `:236` before the drop at `:237`), which is why the base code's fifth call cannot re-arm.
- CI at the head: run `42453384265` (quick tier) is green on `efb63b9d`; OT-61 closure build `42453367062` is green (both via the GitHub API).
- No QEMU binary in this worker (`qemu-system-x86_64` absent), so I did not boot `cxx_runtime_smoke`; this RETURN is a review, not the merge gate.
- RAG: no response at `http://127.0.0.1:8404/health`; used git/grep/read, not a blocker.
- Working tree: only this RETURN on `dexter/pr75-review`; the pre-existing untracked `pr59-review/` directory is untouched.

## Non-blocking notes

1. **Quick-tier coverage.** `cxx_runtime_smoke` is a default/full-suite scenario, not one of the 31 `SMOKE_SCENARIOS`, and the PR adds no line naming `"cxx_runtime_smoke"` or its feature, so the head's quick-tier QEMU job does not exercise p36. The full-tier batch will, because it runs the whole default suite. If the PM wants the discriminator observed before the batch, dispatch `cargo neo-test cxx_runtime_smoke` (or the full tier) on `efb63b9d`.
2. **"Dropped" is the `Slots` drop, not a slot zeroing.** A value left after round 4 stays in `values` until `Slots` is dropped by std's TLS teardown; the array is inline and is reclaimed then, and the C pointer is caller-owned (POSIX does not free it). That is the correct POSIX outcome, but if the intent is "the slot is null after the last pass", the code does not do that explicitly.

## Recommendation to the PM

**PASS — merge #75 on this evidence.** The cap is enforced by one per-thread count across both exit paths, the leftover value is not re-run, and p36 is a real old-code-fails discriminator. The two notes are coverage/wording, not defects; the KAT's execution evidence is the full-tier batch.
