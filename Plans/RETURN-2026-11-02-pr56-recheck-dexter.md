<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 281, Section 281: re-check of PR #56 F1/F2 (closed-end wake; write readiness)

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-02-relays-277-281.md`, Section 281.
**Branch:** `dexter/pr56-recheck`, cut from `origin/main` @ `d92da6479`; this RETURN is docs-only.
**PR head reviewed:** `bea025782` (`refs/pull/56/head` == `origin/claude/l1a1-pipe-wiring`). This is the
dispatch head. It is `a897a92fc` plus the libneo ABI-module commit; the design/code fixes for F1/F2 are in
`a897a92fc`. My earlier review was at `9194cdcbc`.
**Prior report:** `Plans/RETURN-2026-11-02-pr56-pipe2-review-dexter.md` on `dexter/pr56-review`.

**Verdict.** **F1 PASS. F2 PASS.** No blocking new finding. The two fixed FAILs are correctly implemented,
the closed-end outcome is reached on both the blocking syscall path and the `SYS_WAIT_ANY` path, and the
write-end readiness predicate can no longer report ready for a non-blocking write that then returns `EAGAIN`.

## Checklist (as requested: PASS/FAIL, enforcing `file:line`)

| # | Item | Verdict |
|---|---|---|
| F1 | **Lifetime/lost wake — last close of an end under a parked sibling wakes it, and a read/write through the closed end has a defined outcome** | **PASS** |
| F2 | **Readiness — a write end reports ready only when a non-blocking write necessarily makes progress** | **PASS** |

## F1 — PASS

- The last close of either end now sets that end's closed flag and returns
  `Wake { readers: true, writers: true }`: `kernel/src/ipc/pipe_logic.rs:226-255` (closed flags at `:232-238`,
  the read buffer is dropped at `:245-248`, both sides at `:252-255`). `Wakes::take` drains **both** token
  lists (`kernel/src/ipc/pipe.rs:73-84`), and `PipeObject::release` fires them after dropping the pipe guard
  (`:174-187`; `:95-110`). A thread parked on the end a sibling closed is therefore woken, not left asleep.
- The retry has a defined outcome: `Pipe::write` refuses a closed write end before any blocking decision
  (`pipe_logic.rs:272-274`), `Pipe::read` refuses a closed read end (`:317-319`), and `errno_of` maps
  `PipeError::EndClosed` to `EBADF` (`pipe.rs:396-401`). A blocking write that stored bytes before the close
  still returns that count, not the error (`kernel/src/arch/x86_64/syscall.rs:2314-2319`), which is the
  contract the THINK records (`docs/THINK-2026-11-02-l1a1-pipe-wiring.md:50-56`).
- The same-end wake also covers a `SYS_WAIT_ANY` waiter: removal fires the pipe's key
  (`kernel/src/sched/process.rs:2077-2087`), and the re-scan's `pipe_lookup` returns `EBADF` once the handle is
  gone (`syscall.rs:6538-6544`), so that path cannot re-park on a handle nothing will fire.
- Tests are present and exercise the actual dispatcher: KAT `a_closed_end_refuses_its_own_read_and_write`
  (`tools/pipe-logic-tests/src/lib.rs:301-318`); scenario legs `close_parked_read` / `close_parked_write`
  (`kernel/src/test_harness_pipe.rs:668-719`), checked against `EBADF` (-9) and `arc=1` in the expected serial
  (`tools/xtask/src/main.rs:16829-16834`). I ran the host KAT suite at `bea025782`: 22/22 pass.

## F2 — PASS

- `Pipe::writable` now requires room for `min(PIPE_ATOMIC, capacity)` bytes, or no reader (EPIPE):
  `kernel/src/ipc/pipe_logic.rs:169-176`. `PipeObject::ready(Write)` uses exactly that predicate
  (`kernel/src/ipc/pipe.rs:156-162`).
- No spin: if readiness is true and a reader remains, then `space >= min(512, capacity)`. For a write of
  length `L <= 512` and `L <= capacity`, `space >= L`, so it is stored whole (`pipe_logic.rs:282-284`). For any
  larger or non-atomic write, `space >= 1`, so it stores at least one byte (`:286-292`). If no reader remains,
  the write returns `BrokenPipe`/`EPIPE`, not `EAGAIN` (`:275-277`). Therefore a `SYS_WAIT_ANY`-ready write
  followed by `SYS_PIPE_WRITE(..., PIPE_NONBLOCK)` cannot return `EAGAIN`.
- The reviewer's concrete case is pinned: capacity 512, 400 buffered, 300-byte non-blocking write is refused
  by readiness (`tools/pipe-logic-tests/src/lib.rs:325-340`). The scenario's `wait_write` leg shows a 1-byte
  read on a full 16-byte pipe wakes the waiter, which re-parks (`early=false`), then a drain completes it
  (`kernel/src/test_harness_pipe.rs:611-666`; expected strings `tools/xtask/src/main.rs:16827-16828`).
- The PM's latest note reports the gate green on `bea0257` (run 41455350038), with `pipe_core` and
  `pipe_park_kill` passing. I did not rerun the QEMU gate; this re-check is code/design plus host KATs.

## Anything new (one line)

**Non-blocking note:** for a capacity below `PIPE_ATOMIC` (512), `writable`'s threshold is the whole capacity,
so a wait on such a pipe is ready only when it is empty; no spin and no correctness defect (step 2a's pipes are
4 KiB/64 KiB), but a small pipe's write-end wait is conservative because the wait item carries no length.

## What I checked / did not check

Inspected at `bea025782`: `kernel/src/ipc/pipe_logic.rs`, `kernel/src/ipc/pipe.rs`, the pipe syscalls and
`SYS_WAIT_ANY` arm in `kernel/src/arch/x86_64/syscall.rs`, the handle removal/release path in
`kernel/src/sched/process.rs`, `kernel/src/sched/wait_any.rs`, `kernel/src/test_harness_pipe.rs`, the
`pipe-logic-tests` KATs, the `xtask` expected serials, and the THINK. Ran `cargo test -p pipe-logic-tests` at
the reviewed head (22 pass). Did not rerun the 4-shard QEMU gate or the three controls; the pre-merge AAG for
the thread park/teardown surface remains the PM's gate.
