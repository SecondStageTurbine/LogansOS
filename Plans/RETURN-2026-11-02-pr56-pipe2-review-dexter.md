<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 273, Section 273: design read of PR #56 (L1a.1 step 2, the kernel pipe object)

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-02-relays-272-273.md`, Section 273.
**Branch:** `dexter/pr56-review`, cut from `origin/main` @ `c0b0e249b`; this RETURN is docs-only.
**PR head reviewed:** `9194cdcbc` (`refs/pull/56/head` == `origin/claude/l1a1-pipe-wiring`), 5 commits.
**PR base:** `647f370de`.
**Design read:** `docs/THINK-2026-11-02-l1a1-pipe-wiring.md` at `9194cdcbc`.

**Dispatch-head discrepancy.** Section 273 names head `a2ff8630`. That object is not in the repository
(`git cat-file -t a2ff8630` → `fatal: Not a valid object name`), and it is not in `refs/pull/56/head` or the
`claude/l1a1-pipe-wiring` branch. The head I fetched and reviewed is `9194cdcbc`; the SHA mapping is unknown
because the old object was not preserved by the force-push/rewrite. Every `file:line` below is at `9194cdcbc`.

**Verdict.** The core design is sound: per-handle endpoint counting, the token park under the pipe guard, and the
CP-14 / CP-33 / CP-61 / CP-67 rows are implemented and exercised. I found **three design gaps that should be
resolved before this is called step 2 complete** (F1 close-while-parked; F2 write-end readiness vs the atomic rule;
F3 the missing ABI byte oracle), plus a missing kill-switch gate leg (F4) and several documentation/rights notes.
This is a design review, not the AAG: the PR touches thread park/teardown, so the pre-merge AAG still needs the
cross-family lens the THINK names (GPT-6.1-Sol, high; CLAUDE.md SMP-core rule). I did not run the QEMU gate.

## Checklist (as requested: PASS/FAIL, enforcing `file:line`, concrete input on FAIL)

| # | Item | Verdict |
|---|---|---|
| 1a | **Rights — read/write are end-bound and enforced** | **PASS** |
| 1b | **Rights — derive/grant/spawn cannot widen an end's right** | **PASS** |
| 1c | **Rights — `SYS_WAIT_ANY` on an end with no semantic right** | **NOTE (fail-open on a rightless handle)** |
| 1d | **Rights — `REVOKE` on a pipe handle** | **NOTE (deliberate? every other `full_*_rights` has it)** |
| 2a | **Lifetime — process death / kill releases endpoints and wakes the other side** | **PASS** |
| 2b | **Lifetime — close of an end while a sibling thread is parked on that same end** | **FAIL (F1)** |
| 3a | **Blocking/wakeup — no lost wake at empty→non-empty or full→non-full** | **PASS** |
| 3b | **Blocking/wakeup — `SYS_WAIT_ANY` write-end readiness matches the atomic-write rule** | **FAIL (F2)** |
| 4 | **Bounded buffers** | **PASS (NOTE F6: unbounded pre-check walk)** |
| 5 | **EOF/EPIPE as step 1 defined, plus the pinned zero-length contract** | **PASS** |
| 6a | **Syscall ABI — numbers free, pack matches libneo, `WaitItem` oracle** | **PASS** |
| 6b | **Syscall ABI — an independent byte oracle for the new ABI** | **FAIL (F3)** |
| 7a | **Kill-switch — `pipe` off is a complete compile-time off path** | **PASS** |
| 7b | **Kill-switch — the claimed "pipe on, nothing creates a pipe" gate leg** | **FAIL (F4)** |

**PASS evidence.** 1a/1b: `pipe_lookup` requires `USE` plus the end's semantic right (`syscall.rs:2138-2165`);
the wrong end is `EPERM` (the `pipe_core` create leg's `read_end_write=-1` (`test_harness_pipe.rs:228-236`, print at `:369`));
grant intersects rights (`process.rs:3961-3964`), derive requires a subset (`:3463`), `with_cap` preserves `end`
(`:1299-1307`), and spawn's generic `GRANT` path clones with the same rights/end (`syscall.rs:1987-1997`).
2a: `retain_pipe`/`release_pipe` (`process.rs:2054-2075`), `Detached::release` (`:1842-1848`),
`HandleTable::Drop` (`:1675-1685`), `notify_removed` (`:2079-2088`), last-end wakes (`pipe_logic.rs:197-229`),
and the CP-67 anchor (`thread.rs:191-196`, `scheduler.rs:1706-1740`); the `pipe_core` two-writers leg and the
`pipe_park_kill` read/write legs are the tests. 3a: the check + token + park is one critical section
(`pipe.rs:226-341`, `Wakes` `:73-110`), with `try_block_current_with_token_and_guard` (`scheduler.rs:2206-2290`)
and `token_still_parked` (`scheduler.rs:2685-2692`); CP-14/CP-33/CP-61/CP-67 scenarios, the three control features named in the
`xtask` scenario comments (`tools/xtask/src/main.rs:4830-4860`), and the `require_observed_pass` strings
(`:16799-16840`). 4: capacity 1..=64 KiB (`pipe_logic.rs:24-30,102-115`),
fixed token arrays (`PARK_MAX` at `pipe.rs:38`; `Inner`'s arrays at `:43-44`; `Wakes` at `:56-62`), fixed 512-byte stack chunks (`syscall.rs:2221`), and
`try_reserve_exact` for storage (`pipe.rs:127-131`). 5: `close_endpoint`/`read`/`write`
(`pipe_logic.rs:197-330`), the zero-length contract (`:236-258`, `:278-297`), and the host KATs in
`tools/pipe-logic-tests`. 6a: the syscall numbers are free (grep of all remote refs: the `0x1B7–0x1B9` slots are taken, e.g.
`claude/a1a-apps` has `SYS_TRY_SEND`/`SYS_TRY_GRANT`/`SYS_TRY_RECV_GRANT` there and `import/pr220` uses
`0x1B7`/`0x1B8` under other names; no ref uses `0x1BA–0x1BC`), and the `SYS_PIPE_CREATE` pack / libneo split
agree; the `WaitItem` oracle is `tools/atrium-logic-tests/src/lib.rs:135-136`. 7a: the cfg-off cites are in F4.

## Findings

### F1 (FAIL, lifetime + lost wake) — closing an end while a sibling thread is parked on it does not wake the parked side, and a closed end is not a read/write outcome

`Pipe::close_endpoint` for the **last read** endpoint sets `read_closed`, drops the buffer, and returns
`Wake { readers: false, writers: true }` (`kernel/src/ipc/pipe_logic.rs:213-220`). The last **write** endpoint
returns `Wake { readers: true, writers: false }` (`:222-225`). Neither drains the token list of the end being
closed (`kernel/src/ipc/pipe.rs:174-187` → `Wakes::fire`, `:98-110`). `Pipe::read` never checks `read_closed`
(`pipe_logic.rs:286-294`: it returns `WouldBlock` whenever `len == 0 && writers > 0`) and `Pipe::write` never
checks `write_closed` (it checks `readers == 0` only, `:244-245`, and the atomic-space test, `:250-254`).
`SYS_CLOSE` on a pipe handle goes through `HandleTable::remove` → `Detached` → `release_pipe`
(`kernel/src/sched/process.rs:2066-2075`, `:1842-1848`); nothing there knows about a parked read/write.

Concrete interleaving (two threads of one process; the handle table is per process):
1. Thread A: `SYS_PIPE_READ(h=3, buf, 8, 0)`; the pipe is empty and a writer remains, so A parks
   (`pipe.rs:226-252` → `park`, `:293-341`). A's token is in `inner.readers`.
2. Thread B: `SYS_CLOSE(3)`; this is the last read endpoint → `read_closed = true`, the buffer is dropped, and
   `Wakes::fire` wakes only the **writers** (`pipe_logic.rs:213-220`). A's token is not drained and A is not
   woken.
3. A writer still holds the write end, so even if A were woken, `read` would see `writers > 0` and return
   `WouldBlock`, not `Eof`. But `Pipe::write` sees `readers == 0` and returns `BrokenPipe`
   (`pipe_logic.rs:244-245`), so no writer can ever write to wake A. A sleeps until **all** writers close
   (`close_endpoint(Write)` last → `Wake { readers: true }`) or the process is killed.

The same shape exists for a parked **writer** and a sibling close of the last write end: `close_endpoint(Write)`
wakes readers, not writers, and `write` does not consult `write_closed`, so the writer re-parks.

This is not the kill path (that is CP-33/CP-67 and is covered); it is the ordinary `SYS_CLOSE` lifetime path the
dispatch asks about. If the project already treats a close during an in-flight sibling syscall as undefined, then
this is a documentation/test gap rather than a code bug; neither the THINK nor the SDK-ABI says so, and no test
covers it. Otherwise fix by one of:
- (a) make the last close of an end wake **both** sides and make `read`/`write` return a defined closed-end
  outcome (`Eof`/`EPIPE`, or a new `EBADF`) when `read_closed`/`write_closed`; or
- (b) hold an endpoint reference for the duration of a blocking syscall (the Linux file-ref model), so the close
  cannot reach 0 until the syscall returns; or
- (c) explicitly document "a handle must not be closed while a sibling is blocked on it" and enforce it (hard to
  detect; (a) is the smallest correct change). Existing tests do not cover this: `pipe_core`'s rebind leg closes a
  handle while the thread is in `SYS_WAIT_ANY` (key-based, so `notify_removed` does fire,
  `process.rs:2079-2088`), and `pipe_park_kill` kills the process, which zombifies the thread, rather than
  closing the handle.

### F2 (FAIL, readiness) — `SYS_WAIT_ANY` on a write end is "ready" when any space exists, but an atomic write can still return `EAGAIN`; a wait/try loop spins

`Pipe::writable()` is `self.len < self.capacity || self.readers == 0` (`pipe_logic.rs:145-147`) and
`PipeObject::ready(Write)` uses it (`pipe.rs:156-163`). The atomic-write rule parks a write of at most
`PIPE_ATOMIC = 512` bytes when the free space is smaller than the write, even though the pipe is not full
(`pipe_logic.rs:250-254`). `SYS_PIPE_WRITE` with `PIPE_NONBLOCK` returns `EAGAIN` in that case
(`syscall.rs:2280-2330`).

Concrete input: a pipe of capacity 512 with 400 bytes buffered (112 free). `SYS_WAIT_ANY` on the write end returns
bit 0 set (`writable()` true), then `SYS_PIPE_WRITE(h, buf, 300, PIPE_NONBLOCK)` returns `EAGAIN` (`300 <= 512`
and `300 <= capacity`, but `space 112 < 300`). The wait item is immediately ready again, so a caller that loops
`WAIT_ANY` → non-blocking write spins forever on a pipe that is not full and has no reader activity. The read-end
predicate is fine; only the write end is affected. Fix: make the write-end readiness `space >= PIPE_ATOMIC` (the
level-triggered predicate for a <=512-byte write), or carry a requested length in the wait item and use
`space >= len`; at minimum, correct the `writable()` doc ("a write would not block" is false under the atomic rule)
and document that a write-end `WAIT_ANY` is only a "some space" hint. The terminal's actual use is a read-end wait,
so this is not on the step-2a terminal path, but the design claims end-aware readiness for both ends.

### F3 (FAIL, ABI artifact/docs) — no independent byte oracle for the new pipe ABI, and the SDK-ABI rows sit under the wrong section/range

What is pinned: `WAIT_KIND_PIPE == 4` is asserted against the kernel constant
(`tools/atrium-logic-tests/src/lib.rs:135-136`). What is not: the `SYS_PIPE_CREATE` packed return
`rd | wr << 32` (`kernel/src/arch/x86_64/syscall.rs:2204-2215`; split at `userland/libneo/src/lib.rs:3072-3078`)
and the `PIPE_NONBLOCK = 1` flag (`syscall.rs:376`, `lib.rs:305`). Both halves are in-tree and can agree while
both are wrong; that is exactly the failure the byte-oracle rule exists to catch. A hand-laid fixture should exist
independent of the packer/splitter: e.g. `(rd, wr) = (0x1122_3344, 0x5566_7788)` → `0x5566_7788_1122_3344`, and
`PIPE_NONBLOCK == 1`, asserted in a host KAT (the pack can be factored into a pure helper if it is currently
inline in the syscall).

Documentation: the new rows are under `### Atrium: compositor, GPU, input (0x1A0–0x1B6)`
(`docs/SDK-ABI.md:306`) although the pipe is not Atrium and the range ends at 0x1B6
(`docs/SDK-ABI.md:335-337`). The `## Reserved syscall ranges` table (`docs/SDK-ABI.md:450-462`) has no
`0x1B7–0x1BF` row for the pipe range and #15's `0x1B7–0x1B9`. Add a pipe/base-extension row and a separate
heading (or extend the range description). This is a spec-consistency gap, not a code defect.

### F4 (FAIL, kill-switch claim) — the feature-off path is clean, but the claimed "pipe on, nothing creates a pipe" gate leg does not exist

Feature off is complete: no `Handle::Pipe` variant (`process.rs:1042-1052`), no syscall constants/arms
(`syscall.rs:361-376`, `:1330-1336`), no `Thread::pipe_anchor` (`thread.rs:191-196`), no `HandleTable`
retain/release/notify arms; `SYS_PIPE_*` fall to `_ => ENOSYS`. `WAIT_KIND_PIPE` remains a constant
(`wait_any_logic.rs:18`) and the `SYS_WAIT_ANY` arm is `cfg(feature = "pipe")` (`syscall.rs:6537-6543`), so kind 4
is `EINVAL` with pipe off — a clean fail-closed, though the THINK's "no wait kind" (`:100-101`) is imprecise.
The THINK's "With `pipe` on and nothing creating a pipe, every existing scenario's captures are unchanged (a leg in
the gate, OT-88 §3.2's kill-switch leg)" (`docs/THINK:100-102`) has no corresponding scenario/leg in the diff:
`tools/xtask/src/main.rs` adds only `pipe_core` and `pipe_park_kill` (~`:4830-4860`). Add a `pipe`-on-unused run
(e.g. build an existing scenario with `NEO_TEST_EXTRA_FEATURES=pipe`) and assert the same capture, or drop the
claim. Note `pipe` also enables `wait-multiplex` (`kernel/Cargo.toml:503-506`), so that run should confirm both.

### F5 (NOTE, rights) — `full_pipe_rights` omits `REVOKE`

`full_pipe_rights` grants `USE | GRANT | DERIVE` + the end's semantic right (`process.rs:239-250`), while
`full_channel_rights`/`full_socket_rights` use `full_meta_rights()` (`:280-289`, `:298-303`), which includes
`REVOKE` (`:280-282`). A process holding a pipe end therefore cannot `SYS_REVOKE` a copy it granted:
`revoke_handle` refuses with `EPERM` at `!root.rights().has_meta(REVOKE)` (`:3574`). Concrete: parent creates a
pipe, grants the write end to a child, then calls `SYS_REVOKE` on its own write handle → `EPERM`. If pipe grants
are deliberately irrevocable, say so in §2a; otherwise add `meta_rights::REVOKE`.

### F6 (NOTE, bounds) — `SYS_PIPE_READ` pre-checks the whole user `len`, which is user-controlled up to 128 TiB

`sys_pipe_read_impl` calls `check_user_writable(buf, len)` before the pipe lookup (`syscall.rs:2228-2234`).
`len` is only bounded by `USER_HALF_LIMIT` (`:1544`) and `check_user_writable` walks every 4 KiB page
(`:1598-1615`). A pipe can deliver at most `capacity <= 64 KiB`, so the full-range walk is unnecessary; a caller
with a large mapped buffer and a large `len` makes the kernel walk O(len/4K) pages before returning. Bound the
check to `min(len, capacity + PIPE_CHUNK)` (or the first chunk) — the copy still fault-checks each chunk. Not a
correctness bug; worth fixing while the syscall is new.

### F7 (NOTE, design-doc drift)

- `docs/THINK:57` says the pipe fires the wait key "under its own lock (the order `PipeObject.state` →
  `WAIT_TABLE` → scheduler)". The code fires **after** dropping the pipe guard (`pipe.rs:98-110`, `Wakes::fire`
  says "The caller has dropped the pipe's guard"), which is safe because the wait is level-triggered and the
  caller re-scans after registering (`syscall.rs:6630-6665`); the actual order is "state change under the lock,
  fire after release", which is better for lock order. The doc should describe the code (or the code should match
  the doc, but the doc's order is not needed).
- `docs/THINK:69` says `wait_any::pipe_key(ptr, end)`; the code has `PipeObject::wait_key` (`pipe.rs:149`); there
  is no `wait_any::pipe_key`.
- `pipe.rs:124-126` says `PipeObject::new` returns "one read end and one write end"; it returns **zero**
  endpoints (`Pipe::new_unattached`, `pipe_logic.rs:119-125`), and the caller's handle installs retain. The test
  harness relies on the zero counts (`test_harness_pipe.rs:343`, the `pipe_core` round leg's `counts=0/0/0` at `:412`). Fix the comment.
- `docs/THINK:100-101` "no wait kind" vs the unconditional `WAIT_KIND_PIPE` constant (`wait_any_logic.rs:18`).

## The PR's §6 questions

1. **`TokenList` bound = 8.** Adequate for step 2a's consumer (a terminal has one reader and one writer), and it
   matches the kernel's bounded-waiter style (`WAITERS_MAX = 16`). Two caveats: a blocking read/write that finds
   the list full returns `EAGAIN`, which is a documented bounded-resource outcome rather than POSIX blocking
   behaviour; and because the list is per pipe, a process that holds an end can park 8 threads and make another
   process's blocking call `EAGAIN` (a scoped cross-process DoS between processes that already share the pipe).
   Recommend 8 for 2a, document both caveats, and revisit if a workload needs more. It must not become "wake a
   live waiter to make room" — the purge-dead-tokens-then-`EAGAIN` rule (`pipe.rs:293-341`) is the right shape.
2. **`PIPE_ATOMIC = 512`.** Correct (POSIX's minimum `PIPE_BUF`), and it is also the stack chunk
   (`PIPE_CHUNK`, `syscall.rs:2221`), so a <=512-byte write is one atomic `Pipe::write` call and a larger write is
   chunked/interleavable as POSIX allows. Recommend 512. The one consequence to pin with the fix in F2 is that
   "any space" is no longer "a write will not block", so the write-end readiness predicate needs the atomic
   threshold.

## What I checked / did not check

Read: the THINK, the full diff `origin/main...pr56`, `kernel/src/ipc/pipe.rs` and `pipe_logic.rs`, the pipe arms of
`syscall.rs` (lookup/create/read/write/`WAIT_ANY`), `process.rs` (Handle::Pipe, retain/release/notify, Detached
drop order, grant/derive/spawn rights, CAP_GRAPH), `scheduler.rs` (token park/wake, `with_pipe_anchor`,
`token_still_parked`), `thread.rs`, `wait_any.rs`, `test_harness_pipe.rs`, the `xtask` scenarios/controls and
expected serial strings, `docs/SDK-ABI.md`, `userland/libneo`, and the host KATs. I grepped all remote refs for
`0x1B7–0x1BF`: the `0x1B7–0x1B9` slots are taken (e.g. `claude/a1a-apps` has
`SYS_TRY_SEND`/`SYS_TRY_GRANT`/`SYS_TRY_RECV_GRANT` there; `import/pr220` uses `0x1B7`/`0x1B8` under other
names), and only this branch uses `0x1BA–0x1BC` (the THINK's claim holds at this head).

I did **not** run the 4-shard QEMU gate, the controls, or a host build. The scenario/control wiring and expected
strings are present and internally consistent; the gate result is the PM's verify-before-merge step. The
pre-merge AAG for the thread-park/teardown surface is still required (cross-family lens, GPT-6.1-Sol high per the
THINK), and F1/F2 should be settled before the merge so the AAG reviews the final shape.
