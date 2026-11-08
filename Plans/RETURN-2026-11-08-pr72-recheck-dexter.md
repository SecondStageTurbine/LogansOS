<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 325: #72 re-check (stdio chosen at spawn, revision 2)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-325-328.md` §325.
**Reviewed:** PR #72 head `e49233bf62c37db7af42722f2f053209caa93919` (`docs/THINK-2026-11-08-l1a1-stdio-selection.md`,
revision 2; `b3cd3418` is revision 2, `e49233bf` is the handle-kind-20 line). Branch `dexter/pr72-recheck`, cut from
`origin/main` `7072cf58270044fa99d93ccc5e3f5d02eabb8976`.

**Overall: PASS.** All five prior recommendation points are answered, handle kind 20 does not collide with anything on
main, and the two earlier passes (set-before-run, endpoint counting) still hold. Two non-blocking notes: the THINK's
`[2a]` citation still names `71bc2dca`, where `KIND_PIPE` is 19 — the 20 is at #56's post-#8-merge head `1e98d65e`;
and `SYS_STDIO_INFO`'s `len`-vs-`out` error order is not written down (the `SYS_HANDLE_INFO` precedent checks `len`
first).

The section's opening line still names the old `Plans/RETURN-2026-11-08-pr72-design-dexter.md` path; the section's
close line (and the relay) name `Plans/RETURN-2026-11-08-pr72-recheck-dexter.md`, which is this file.

## 1. The five recommendation points

| # | Point | Verdict | THINK lines |
|---|---|---|---|
| 1 | Term page feature + positive term leg | **PASS** | 37-39, 99, 101, 225-226, 253, 313-316 |
| 2 | Two SDK-ABI rows + range note | **PASS** | 203-205 |
| 3 | `SYS_STDIO_INFO` error, all-none and term-identity cases | **PASS** | 90-91, 169-192, 201, 303-307 |
| 4 | `HandleKind::Pipe` from `cap_semantic`, `expect` before `need`, zero-length write | **PASS** | 28-30, 60-62, 144, 159-162 |
| 5 | Scenario stand-in, 5-arg dispatch, serial observation, late-control witness | **PASS** | 253-262, 272-273, 318-325 |

### 1 — PASS: the term page has a feature and a positive leg

The scenario feature now carries both gates: `neo-test-pipe-stdio = ["neo-test", "pipe",
"atrium-compositor-g1"]` (THINK:253). The term role is a `Handle::Buffer` of exactly 4096 bytes with
`MAP_READ | MAP_WRITE` (THINK:99), and §2.2 says a term index in a kernel without `atrium-compositor-g1` is `EINVAL`
(THINK:101). §2.5 repeats that the term page also needs `atrium-compositor-g1`, and that the term field of
`SYS_STDIO_INFO` is always `0xFFFF` without it (THINK:225-226). Leg 9 is the positive term-page leg: the terminal writes
`NTP1` and the 80 × 25 size before the spawn, the probe maps it with `SYS_STDIO_TERM_MAP` and writes the magic and size
back, and a reused slot is `EBADF` (THINK:313-316). The earlier `pipe`-only feature set that could not compile the term
arm is gone.

### 2 — PASS: the SDK-ABI rows and the range note are named

The THINK adds three rows — 445 / `0x1BD`, 446 / `0x1BE`, 447 / `0x1BF` — in the Pipes section, with their arguments,
returns, errors and features, and changes the range note's "0x1BD-0x1BF empty" to "populated" (THINK:203-205). The
request was two rows; the third is the new `SYS_STDIO_TERM_MAP` that the term-identity fix needs, so this is a superset,
not a miss. On #56 the Pipes section ends at row 444 / `0x1BC` and the note says `0x1BD–0x1BF empty`
(`refs/pr/56:docs/SDK-ABI.md:361-363,504`), so 445-447 are the next rows. The xtask drift guard is named
(`sdk_abi_doc_covers_every_kernel_syscall`), so the host gate will enforce the rows.

### 3 — PASS: the error, all-none and term-identity cases are defined

- **Errors:** `len != 8` is `EINVAL`, and an `out` that fails `validate_user_buf` or faults in the copy is `EFAULT`
  (THINK:176-177). This is the `SYS_HANDLE_INFO` shape (`kernel/src/arch/x86_64/syscall.rs:7836-7859` at the THINK's
  base `13eed2c5`). Leg 7 pins `len = 7` → −22 and `out = 1` → −14 (THINK:306).
- **All-none:** `0x00FF_FFFF` is `Stdio::NONE`, so the child is the same as one spawned by `SYS_SPAWN`, and
  `SYS_STDIO_INFO` reports the no-selection row with flags 0 (THINK:90-91, 201); leg 7 spawns the probe that way and
  checks the row (THINK:307).
- **Term identity:** `SYS_STDIO_INFO` reports a slot only while it still holds the selected `CapId`, else `0xFFFF`,
  with flags still saying a stdio was selected (THINK:180-183). The new `SYS_STDIO_TERM_MAP(role)` checks the
  `StdioRef` `CapId` first under the table lock before mapping, so a different buffer that happens to reuse the slot
  is `EBADF` (THINK:184-192); leg 9's `T` case exercises exactly that (THINK:316).

The report is advisory and the map is the enforced path, which is the right split: `SYS_STDIO_INFO` can go stale
between the report and the use, and the `CapId` check at the use catches it.

### 4 — PASS: the pipe end, the lookup order and the zero-length write are fixed

- **`HandleKind::Pipe` end:** the end is the `cap_semantic` `READ`/`WRITE` bit, and `flags` is 0 for a pipe
  (THINK:28-30, 209). The record has separate `cap_meta` and `cap_semantic` fields
  (`refs/pr/56:kernel/src/sched/handle_info_logic.rs:18-20`), and `handle_info` fills `cap_semantic` from
  `CapRights.semantic` (`refs/pr/56:kernel/src/sched/process.rs:1235-1238`), so the std overlay can tell the two ends
  apart without a new flag.
- **`expect` before `need`:** `pipe_lookup` gains `expect: Option<CapId>`, compared before the kind and the rights,
  under the same table lock (THINK:60-62, 159-162). Leg 5(b)/(c) covers a read end and a write end reused into the
  stdout slot, both −9, not −1 (THINK:288-293).
- **Zero-length write:** `SYS_WRITE(1, buf, 0)` returns 0 before any lookup, today's contract and POSIX's, never
  `EPIPE` (THINK:144). That preserves the current `sys_write_impl` behaviour.

### 5 — PASS: the scenario is implementable and the control proves its window

- **No kernel stand-in / 5-arg shim:** the terminal side is real ring-3 `neo-init` code under the `pipe-stdio-test`
  arm, making the real syscalls through the real six-argument dispatcher, so the 2a `syscall_dispatch4_for_test` limit
  no longer applies (THINK:254-255).
- **Stand-in:** the child is the real `neo-shell`; the new `neo-stdio-probe` module is the probe / shell-child
  (THINK:259-262). It has a named home and a defined handle layout, so the legs are no longer "the smallest existing
  module, chosen with the code".
- **Serial observation:** the terminal reports each leg as a `PIPE_STDIO <leg> ...` line on its own serial fd 1, checked
  with `require_serial_sequence`; xtask then checks that no `neo> ` and no `pipe-ok` reached serial before the leg-4
  marker with `require_serial_absent` on that prefix (THINK:257-258, 272-273). This is the xtask-side observation, and
  it is paired with the positive pipe read in leg 1, so "prompt on serial" and "prompt nowhere" are distinguishable.
- **Late-control witness:** the control stores the stdio after `try_push_ready`, spins with a 1 s bound until the
  child's first syscall has entered (a per-pid counter that exists only in the control), and counts a hit. If the
  window was hit, the terminal reports `pipe_stdio_prompt_not_in_pipe`; if it was not, the scenario reports
  `pipe_stdio_late_window_not_hit`; either way the control fails with a named reason (THINK:318-325). That is the
  witness the prior read asked for, not a bare yield.

## 2. Handle kind 20 does not collide on main — PASS

On `origin/main` `7072cf58`, the highest kind is `KIND_POWER_CONTROL = 19`
(`kernel/src/sched/handle_info_logic.rs:46`); there is no `KIND_PIPE` and no kind 20. The libneo decoder maps 19 to
`PowerControl` and every other code to `Unknown` (`userland/libneo/src/handle_info.rs:64-65`), so 20 is free. #56's
current head has `KIND_PIPE = 20` (`refs/pr/56:kernel/src/sched/handle_info_logic.rs:48`), after it merged #8.

**Note (citation, not a collision):** the THINK says `[2a]` code is at `71bc2dca` (THINK:11), but at that commit
`KIND_PIPE` is still `19` (`71bc2dca:kernel/src/sched/handle_info_logic.rs:47`), which collides with main's
`PowerControl = 19`. The THINK's `20` matches #56's post-merge head `1e98d65e` (the merge of `origin/main` `8c3ac35`,
which brought #8 in). Update THINK:11 and the §1.5 parenthetical to the head that will land, or land the
`19 → 20` change explicitly; otherwise the design's citation and the code disagree even though main is clear.

## 3. Items 2 and 3 still hold — PASS

- **Item 2 (set before the child can run).** The THINK still installs the inherited handles, records the child-side
  `CapId`s into a local `Stdio` after the install block, and stores it before `register(process)` and
  `try_push_ready` (THINK:117-120, 246). On #56's current head the install block is `process.rs:4454-4472`,
  `register(process)` is `:4474`, and `try_push_ready` is `:4507` — the field can be set after the install block
  (where the `handles` lock is dropped) and before `register`, so the ordering still closes the window.
- **Item 3 (endpoint counting).** The THINK's counts are unchanged (THINK:131-139): `HandleTable::install` calls
  `retain_pipe` (`refs/pr/56:kernel/src/sched/process.rs:2091-2095`), `Detached::release` calls `release_pipe`
  (`:1877-1883`), a failed spawn's `HandleTable::drop` drains (`:1710-1713`), process teardown drains through
  `teardown` (`:3300-3308`), and `drain` fires the removed handles' wait keys (`:2315-2332`). The child's copy is one
  more endpoint, the parent's copy keeps the pipe from early EOF, and the failed-spawn path releases the same way.
  The #8 merge changed none of these paths.

## Verification performed

- Fetched PR #72 (`refs/pr/72` = `e49233bf`) and PR #56 (`refs/pr/56` = `1e98d65e`), plus `origin/main`
  (`7072cf58`). Read `docs/THINK-2026-11-08-l1a1-stdio-selection.md` at `e49233bf` in full, with line numbers.
- Read `origin/main`'s `kernel/src/sched/handle_info_logic.rs`, `userland/libneo/src/handle_info.rs` and
  `userland/neoplutarch-std-overlay/std/os_neoplutarch.rs` for the kind registry; read #56's
  `handle_info_logic.rs`, `process.rs`, `syscall.rs`, `docs/SDK-ABI.md` at both `71bc2dca` and the current head.
- Re-read the prior RETURN, `Plans/RETURN-2026-11-08-pr72-design-dexter.md`, for the five recommendation points and
  the two earlier passes.
- No build or test run: this is a design re-read and the THINK says the 2b code is not written. No source, THINK or
  other file changed; this commit is the RETURN only. The pre-existing untracked `pr59-review/` directory is not
  staged and is not part of this branch.
- RAG is unavailable in this worker (`http://127.0.0.1:8404/health` did not answer); used `git show` and `grep`.

## Recommendation to the PM

Proceed to the 2b build. The five prior findings are answered in the THINK, the kind number is free on main, and the
two earlier passes still hold. Before the build, make the two small edits: update the `[2a]` citation to #56's landing
head (the current `71bc2dca` still has `KIND_PIPE = 19`), and write down the `SYS_STDIO_INFO` error precedence
(`len` first, then `out`, per `SYS_HANDLE_INFO`) so the oracle's "both bad" input has one defined errno.
